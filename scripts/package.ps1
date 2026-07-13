[CmdletBinding()]
param(
    [switch] $RequireModelSmokeTest
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$binary = Join-Path $root "target\release\private-translator.exe"
$runtime = Join-Path $root "runtime\llama.cpp"
$dist = Join-Path $root "dist"
$trustedManifestPath = Join-Path $root "packaging\trusted-artifacts.json"
$componentManifestPath = Join-Path $root "packaging\component-manifest.json"
$runtimeMinimizer = Join-Path $PSScriptRoot "minimize-runtime.ps1"
$packagePolicy = Join-Path $PSScriptRoot "package-policy.ps1"

foreach ($required in @($binary, $runtime, $trustedManifestPath, $componentManifestPath, $runtimeMinimizer, $packagePolicy)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Required build component is missing: $required"
    }
}

function Assert-TrustedFile {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] $Specification
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Trusted component is missing: $Path"
    }
    $actualBytes = (Get-Item -LiteralPath $Path).Length
    if ($actualBytes -ne [long] $Specification.bytes) {
        throw "Trusted component size mismatch: $Path"
    }
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
    if ($actualHash -ne $Specification.sha256.ToLowerInvariant()) {
        throw "Trusted component SHA-256 mismatch: $Path"
    }
}

$trustedManifest = Get-Content -LiteralPath $trustedManifestPath -Raw | ConvertFrom-Json
if ($trustedManifest.schema_version -ne 1) {
    throw "Unsupported trusted artifact manifest version: $($trustedManifest.schema_version)"
}

$modelSpecification = $trustedManifest.models |
    Where-Object { $_.id -eq "hy-mt2-1.8b-q4" } |
    Select-Object -First 1
if ($null -eq $modelSpecification) {
    throw "The Lite model is missing from the trusted artifact manifest."
}
if ($modelSpecification.channel -ne "stable" -or
    $modelSpecification.revision -notmatch '^[0-9a-f]{40}$' -or
    $modelSpecification.download_url -notmatch '^https://huggingface\.co/') {
    throw "The Lite model must use a pinned stable official download."
}

foreach ($specification in $trustedManifest.runtime.files) {
    if ((Split-Path -Leaf $specification.path) -ne $specification.path) {
        throw "Nested runtime paths are not allowed in the trusted artifact manifest."
    }
    Assert-TrustedFile -Path (Join-Path $runtime $specification.path) -Specification $specification
}

. $packagePolicy

$componentManifest = Get-Content -LiteralPath $componentManifestPath -Raw | ConvertFrom-Json
if ($componentManifest.runtime.packaged_entrypoint -ne "llama-server.exe" -or
    [int] $componentManifest.runtime.packaged_file_count -ne @($trustedManifest.runtime.files).Count) {
    throw "Component manifest runtime packaging policy does not match the trusted runtime inventory."
}
$trustedLicenses = @($trustedManifest.licenses)
if ($trustedLicenses.Count -ne 2) {
    throw "The trusted artifact manifest must contain exactly the llama.cpp and Hy-MT2 licenses."
}
$expectedLicenseNames = @($trustedLicenses | ForEach-Object { ([string] $_.path).ToLowerInvariant() } | Sort-Object)
$actualLicenseNames = @(Get-ChildItem -LiteralPath (Join-Path $root "runtime\licenses") -File |
    ForEach-Object { $_.Name.ToLowerInvariant() } |
    Sort-Object)
if (($expectedLicenseNames -join "`n") -ne ($actualLicenseNames -join "`n")) {
    throw "Runtime license inventory does not exactly match the trusted artifact manifest."
}
foreach ($license in $trustedLicenses) {
    if ((Split-Path -Leaf $license.path) -ne $license.path) {
        throw "Nested license paths are not allowed in the trusted artifact manifest."
    }
    Assert-TrustedFile -Path (Join-Path $root "runtime\licenses\$($license.path)") -Specification $license
}
$llamaLicense = $trustedLicenses | Where-Object { $_.id -eq "llama.cpp" } | Select-Object -First 1
$modelLicense = $trustedLicenses | Where-Object { $_.id -eq "hy-mt2-1.8b" } | Select-Object -First 1
if ($null -eq $llamaLicense -or $null -eq $modelLicense -or
    $componentManifest.runtime.license.sha256 -ne $llamaLicense.sha256 -or
    $componentManifest.runtime.license.source -ne $llamaLicense.source -or
    $componentManifest.lite_model.license_sha256 -ne $modelLicense.sha256 -or
    $componentManifest.lite_model.license_source -ne $modelLicense.source) {
    throw "Component and trusted manifest license metadata do not match."
}
$smokeTestModel = Join-Path $root "models\$($componentManifest.lite_model.file)"
if (-not (Test-Path -LiteralPath $smokeTestModel -PathType Leaf)) {
    if ($RequireModelSmokeTest) {
        throw "A full release requires the pinned local model for the llama-server loader smoke test: $smokeTestModel"
    }
    $smokeTestModel = $null
}

function New-PortableProfile {
    param(
        [Parameter(Mandatory)] [string] $Folder,
        [Parameter(Mandatory)] [string] $Executable,
        [Parameter(Mandatory)] [string] $Readme,
        [Parameter(Mandatory)] [string] $KoreanReadme,
        [Parameter(Mandatory)] [string] $Config
    )

    $destination = Join-Path $dist $Folder
    $runtimeDestination = Join-Path $destination "runtime\llama.cpp"
    $licenseDestination = Join-Path $destination "licenses"
    $configDestination = Join-Path $destination "configs"

    if (Test-Path -LiteralPath $destination) {
        Remove-Item -LiteralPath $destination -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $runtimeDestination | Out-Null
    New-Item -ItemType Directory -Force -Path $licenseDestination | Out-Null
    New-Item -ItemType Directory -Force -Path $configDestination | Out-Null

    $launcher = Join-Path $destination $Executable
    $installer = Join-Path $destination "Install-Model.exe"
    Copy-Item -LiteralPath $binary -Destination $launcher -Force
    Copy-Item -LiteralPath $binary -Destination $installer -Force
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $launcher).Hash -cne
        (Get-FileHash -Algorithm SHA256 -LiteralPath $installer).Hash) {
        throw "Install-Model.exe must be a byte-for-byte copy of the Private Translator binary."
    }
    $minimizerArguments = @{
        SourceDirectory      = $runtime
        DestinationDirectory = $runtimeDestination
        ManifestPath         = $trustedManifestPath
    }
    if ($null -ne $smokeTestModel) {
        $minimizerArguments.SmokeTestModel = $smokeTestModel
    }
    & $runtimeMinimizer @minimizerArguments
    Copy-Item -LiteralPath (Join-Path $root "packaging\$Readme") -Destination (Join-Path $destination "README.txt") -Force
    Copy-Item -LiteralPath (Join-Path $root "packaging\$KoreanReadme") -Destination (Join-Path $destination "README.ko.txt") -Force
    Copy-Item -LiteralPath (Join-Path $root "LICENSE") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $root "packaging\component-manifest.json") -Destination $destination -Force
    Copy-Item -LiteralPath $trustedManifestPath -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $root "THIRD_PARTY_NOTICES.md") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $root "configs\$Config") -Destination $configDestination -Force
    foreach ($license in $trustedLicenses) {
        Copy-Item -LiteralPath (Join-Path $root "runtime\licenses\$($license.path)") -Destination $licenseDestination -Force
    }
    $bundledModels = @(Get-ChildItem -LiteralPath $destination -Recurse -File -Filter "*.gguf")
    if ($bundledModels.Count -ne 0) {
        throw "Thin packages must not contain GGUF model files."
    }
    Assert-NoExecutableScripts -Path $destination
}

New-Item -ItemType Directory -Force -Path $dist | Out-Null
New-PortableProfile -Folder "PrivateTranslator-Lite" -Executable "PrivateTranslator-Lite.exe" -Readme "README-Lite.txt" -KoreanReadme "README-Lite.ko.txt" -Config "lite.json"
New-PortableProfile -Folder "PrivateTranslator-Quality" -Executable "PrivateTranslator-Quality.exe" -Readme "README-Quality.txt" -KoreanReadme "README-Quality.ko.txt" -Config "quality.json"

Get-Item -LiteralPath @(
    (Join-Path $dist "PrivateTranslator-Lite\PrivateTranslator-Lite.exe"),
    (Join-Path $dist "PrivateTranslator-Lite\Install-Model.exe"),
    (Join-Path $dist "PrivateTranslator-Quality\PrivateTranslator-Quality.exe"),
    (Join-Path $dist "PrivateTranslator-Quality\Install-Model.exe")
) | Select-Object FullName, Length
