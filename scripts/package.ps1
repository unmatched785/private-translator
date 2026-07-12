$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$binary = Join-Path $root "target\release\private-translator.exe"
$runtime = Join-Path $root "runtime\llama.cpp"
$dist = Join-Path $root "dist"
$trustedManifestPath = Join-Path $root "packaging\trusted-artifacts.json"
$modelInstaller = Join-Path $root "packaging\Install-Model.cmd"

foreach ($required in @($binary, $runtime, $trustedManifestPath, $modelInstaller)) {
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

$expectedRuntimeNames = @($trustedManifest.runtime.files |
    ForEach-Object { $_.path.ToLowerInvariant() } |
    Sort-Object)
$actualRuntimeNames = @(Get-ChildItem -LiteralPath $runtime -File |
    Where-Object { $_.Extension -in @(".exe", ".dll") } |
    ForEach-Object { $_.Name.ToLowerInvariant() } |
    Sort-Object)
if (($expectedRuntimeNames -join "`n") -ne ($actualRuntimeNames -join "`n")) {
    throw "Runtime EXE/DLL inventory does not match the trusted artifact manifest."
}
foreach ($specification in $trustedManifest.runtime.files) {
    if ((Split-Path -Leaf $specification.path) -ne $specification.path) {
        throw "Nested runtime paths are not allowed in the trusted artifact manifest."
    }
    Assert-TrustedFile -Path (Join-Path $runtime $specification.path) -Specification $specification
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

    Copy-Item -LiteralPath $binary -Destination (Join-Path $destination $Executable) -Force
    Copy-Item -Path (Join-Path $runtime "*") -Destination $runtimeDestination -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $root "packaging\$Readme") -Destination (Join-Path $destination "README.txt") -Force
    Copy-Item -LiteralPath (Join-Path $root "packaging\$KoreanReadme") -Destination (Join-Path $destination "README.ko.txt") -Force
    Copy-Item -LiteralPath (Join-Path $root "LICENSE") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $root "packaging\component-manifest.json") -Destination $destination -Force
    Copy-Item -LiteralPath $trustedManifestPath -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $root "THIRD_PARTY_NOTICES.md") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $root "configs\$Config") -Destination $configDestination -Force
    Copy-Item -LiteralPath (Join-Path $root "runtime\licenses\llama.cpp-LICENSE") -Destination $licenseDestination -Force
    Copy-Item -LiteralPath (Join-Path $root "runtime\licenses\Hy-MT2-LICENSE") -Destination $licenseDestination -Force
    Copy-Item -LiteralPath $modelInstaller -Destination $destination -Force

    $bundledModels = @(Get-ChildItem -LiteralPath $destination -Recurse -File -Filter "*.gguf")
    if ($bundledModels.Count -ne 0) {
        throw "Thin packages must not contain GGUF model files."
    }
}

New-Item -ItemType Directory -Force -Path $dist | Out-Null
New-PortableProfile -Folder "PrivateTranslator-Lite" -Executable "PrivateTranslator-Lite.exe" -Readme "README-Lite.txt" -KoreanReadme "README-Lite.ko.txt" -Config "lite.json"
New-PortableProfile -Folder "PrivateTranslator-Quality" -Executable "PrivateTranslator-Quality.exe" -Readme "README-Quality.txt" -KoreanReadme "README-Quality.ko.txt" -Config "quality.json"

Get-Item -LiteralPath @(
    (Join-Path $dist "PrivateTranslator-Lite\PrivateTranslator-Lite.exe"),
    (Join-Path $dist "PrivateTranslator-Quality\PrivateTranslator-Quality.exe")
) | Select-Object FullName, Length
