[CmdletBinding()]
param(
    [string] $Version,

    [switch] $UnsignedDevelopment,

    [string] $SigningCertificateThumbprint = $env:PRIVATE_TRANSLATOR_SIGNING_CERT_THUMBPRINT,

    [string] $TimestampUrl = $env:PRIVATE_TRANSLATOR_RFC3161_TIMESTAMP_URL,

    [string] $SignToolPath = $env:PRIVATE_TRANSLATOR_SIGNTOOL
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$dist = Join-Path $root "dist"
$cargoManifest = Get-Content -LiteralPath (Join-Path $root "Cargo.toml") -Raw
$versionMatch = [regex]::Match($cargoManifest, '(?m)^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) {
    throw "Could not read the package version from Cargo.toml."
}
$manifestVersion = $versionMatch.Groups[1].Value
if (-not $Version) {
    $Version = $manifestVersion
}
elseif ($Version -cne $manifestVersion) {
    throw "Requested release version '$Version' does not match Cargo.toml version '$manifestVersion'."
}
if ($Version -notmatch '^[0-9A-Za-z][0-9A-Za-z.+-]*$') {
    throw "Release version contains unsafe path characters: $Version"
}
$componentManifest = Get-Content -LiteralPath (Join-Path $root "packaging\component-manifest.json") -Raw | ConvertFrom-Json
if ([string] $componentManifest.application.version -cne $Version) {
    throw "Component manifest version '$($componentManifest.application.version)' does not match Cargo.toml version '$Version'."
}
$trustedManifest = Get-Content -LiteralPath (Join-Path $root "packaging\trusted-artifacts.json") -Raw | ConvertFrom-Json
if ($trustedManifest.schema_version -ne 1 -or @($trustedManifest.licenses).Count -ne 2) {
    throw "The trusted artifact manifest or its license inventory is invalid."
}
. (Join-Path $PSScriptRoot "package-policy.ps1")

$bundleBaseName = "PrivateTranslator-v$Version-Windows-x64"
$releaseGitCommit = & (Join-Path $PSScriptRoot "assert-release-tag-available.ps1") `
    -Version $Version `
    -RepositoryRoot $root `
    -UnsignedDevelopment:$UnsignedDevelopment
if ($UnsignedDevelopment) {
    $gitCommand = Get-Command git -ErrorAction SilentlyContinue
    if ($null -eq $gitCommand) {
        throw "Release provenance requires git to resolve the current commit."
    }
    $commitProbe = @(& $gitCommand.Source -C $root rev-parse --verify HEAD 2>&1)
    $commitProbeExit = $LASTEXITCODE
    $releaseGitCommit = ($commitProbe -join "`n").Trim().ToLowerInvariant()
    if ($commitProbeExit -ne 0 -or $releaseGitCommit -notmatch '^[0-9a-f]{40}$') {
        throw "Could not resolve the Git commit for unsigned development provenance."
    }
}

if ($UnsignedDevelopment) {
    $publicLikeOutputs = @(
        Join-Path $dist $bundleBaseName
        Join-Path $dist "$bundleBaseName.zip"
        Join-Path $dist "$bundleBaseName.zip.sha256"
    ) | Where-Object { Test-Path -LiteralPath $_ }
    if (@($publicLikeOutputs).Count -gt 0) {
        $formattedPaths = @($publicLikeOutputs | ForEach-Object { "  - $_" }) -join [Environment]::NewLine
        throw @"
Unsigned development release refused because same-version public-looking outputs already exist:
$formattedPaths
Move these legacy outputs to dist\quarantine (or produce a verified signed public release) before creating an unsigned development artifact.
"@
    }
}

function Resolve-SignTool {
    param([string] $RequestedPath)

    if (-not [string]::IsNullOrWhiteSpace($RequestedPath)) {
        if (-not (Test-Path -LiteralPath $RequestedPath -PathType Leaf)) {
            throw "PRIVATE_TRANSLATOR_SIGNTOOL does not point to a file: $RequestedPath"
        }
        return (Resolve-Path -LiteralPath $RequestedPath).Path
    }

    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    $sdkRoots = @()
    if (-not [string]::IsNullOrWhiteSpace($env:WindowsSdkVerBinPath)) {
        $sdkRoots += $env:WindowsSdkVerBinPath
    }
    if (-not [string]::IsNullOrWhiteSpace(${env:ProgramFiles(x86)})) {
        $sdkRoots += Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    }
    foreach ($sdkRoot in $sdkRoots | Select-Object -Unique) {
        if (-not (Test-Path -LiteralPath $sdkRoot -PathType Container)) {
            continue
        }
        $candidate = Get-ChildItem -LiteralPath $sdkRoot -Recurse -File -Filter "signtool.exe" -ErrorAction SilentlyContinue |
            Where-Object { $_.Directory.Name -eq "x64" } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($null -ne $candidate) {
            return $candidate.FullName
        }
    }
    throw "signtool.exe was not found. Install the Windows SDK or set PRIVATE_TRANSLATOR_SIGNTOOL."
}

function Resolve-SigningCertificate {
    param([Parameter(Mandatory)] [string] $Thumbprint)

    $normalized = ($Thumbprint -replace '[^0-9A-Fa-f]', '').ToUpperInvariant()
    if ($normalized -notmatch '^[0-9A-F]{40}$') {
        throw "PRIVATE_TRANSLATOR_SIGNING_CERT_THUMBPRINT must be a SHA-1 certificate thumbprint."
    }

    foreach ($location in @("CurrentUser", "LocalMachine")) {
        $certificate = Get-ChildItem -LiteralPath "Cert:\$location\My" -ErrorAction SilentlyContinue |
            Where-Object { $_.Thumbprint -eq $normalized } |
            Select-Object -First 1
        if ($null -eq $certificate) {
            continue
        }
        if (-not $certificate.HasPrivateKey) {
            throw "Signing certificate $normalized in $location\My has no private key."
        }
        $hasCodeSigningUsage = @($certificate.EnhancedKeyUsageList |
            Where-Object { $_.ObjectId.Value -eq "1.3.6.1.5.5.7.3.3" }).Count -gt 0
        if (-not $hasCodeSigningUsage) {
            throw "Signing certificate $normalized is not valid for Code Signing."
        }
        $now = Get-Date
        if ($certificate.NotBefore -gt $now -or $certificate.NotAfter -le $now) {
            throw "Signing certificate $normalized is not currently valid."
        }
        return [pscustomobject]@{
            Certificate = $certificate
            Location    = $location
            Thumbprint  = $normalized
        }
    }
    throw "Signing certificate $normalized was not found in CurrentUser\My or LocalMachine\My."
}

function Assert-UnsignedExecutable {
    param([Parameter(Mandatory)] [string] $Path)

    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::NotSigned) {
        throw "Unsigned-development executable has unexpected Authenticode status '$($signature.Status)': $Path"
    }
}

function Assert-TrustedReleaseFile {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] $Specification
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf) -or
        (Get-Item -LiteralPath $Path).Length -ne [long] $Specification.bytes) {
        throw "Trusted release file is missing or has the wrong size: $Path"
    }
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
    if ($actualHash -ne ([string] $Specification.sha256).ToLowerInvariant()) {
        throw "Trusted release file SHA-256 mismatch: $Path"
    }
}

function Assert-SignedExecutable {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $ExpectedThumbprint,
        [Parameter(Mandatory)] [string] $SignTool
    )

    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid) {
        throw "Authenticode validation failed with status '$($signature.Status)': $Path"
    }
    if ($null -eq $signature.SignerCertificate -or
        $signature.SignerCertificate.Thumbprint -ne $ExpectedThumbprint) {
        throw "Executable signer does not match the requested certificate: $Path"
    }
    if ($null -eq $signature.TimeStamperCertificate) {
        throw "Executable has no verifiable RFC 3161 timestamp: $Path"
    }

    & $SignTool verify /pa /all /v $Path
    if ($LASTEXITCODE -ne 0) {
        throw "signtool verification failed with exit code $LASTEXITCODE`: $Path"
    }
}

$signTool = $null
$signingCertificate = $null
if (-not $UnsignedDevelopment) {
    if ([string]::IsNullOrWhiteSpace($SigningCertificateThumbprint)) {
        throw "Public release creation requires PRIVATE_TRANSLATOR_SIGNING_CERT_THUMBPRINT. Use -UnsignedDevelopment only for local development artifacts."
    }
    if ([string]::IsNullOrWhiteSpace($TimestampUrl) -or $TimestampUrl -notmatch '^https://') {
        throw "Public release creation requires an HTTPS RFC 3161 URL in PRIVATE_TRANSLATOR_RFC3161_TIMESTAMP_URL."
    }
    $signingCertificate = Resolve-SigningCertificate -Thumbprint $SigningCertificateThumbprint
    $signTool = Resolve-SignTool -RequestedPath $SignToolPath
}

if (-not $env:CARGO_HOME) {
    $env:CARGO_HOME = Join-Path $root ".cache\cargo"
}
& (Join-Path $PSScriptRoot "check.ps1")
cargo build --manifest-path (Join-Path $root "Cargo.toml") --locked --release
if ($LASTEXITCODE -ne 0) {
    throw "Fresh locked release build failed with exit code $LASTEXITCODE."
}

& (Join-Path $PSScriptRoot "package.ps1") -RequireModelSmokeTest

$bundleName = if ($UnsignedDevelopment) { "$bundleBaseName-UNSIGNED-DEVELOPMENT" } else { $bundleBaseName }
$bundle = Join-Path $dist $bundleName
$archive = Join-Path $dist "$bundleName.zip"
$sidecar = "$archive.sha256"
$distPrefix = $dist.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar

foreach ($path in @($bundle, $archive, $sidecar)) {
    if (Test-Path -LiteralPath $path) {
        $resolvedPath = (Resolve-Path -LiteralPath $path).Path
        if (-not $resolvedPath.StartsWith($distPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to replace release output outside dist: $resolvedPath"
        }
        Remove-Item -LiteralPath $resolvedPath -Recurse -Force
    }
}

$runtimeDestination = Join-Path $bundle "runtime\llama.cpp"
$licenseDestination = Join-Path $bundle "licenses"
$configDestination = Join-Path $bundle "configs"
New-Item -ItemType Directory -Force -Path $runtimeDestination, $licenseDestination, $configDestination | Out-Null

$liteExecutable = Join-Path $bundle "PrivateTranslator-Lite.exe"
$qualityExecutable = Join-Path $bundle "PrivateTranslator-Quality.exe"
$modelInstallerExecutable = Join-Path $bundle "Install-Model.exe"
Copy-Item -LiteralPath (Join-Path $dist "PrivateTranslator-Lite\PrivateTranslator-Lite.exe") -Destination $liteExecutable
Copy-Item -LiteralPath (Join-Path $dist "PrivateTranslator-Quality\PrivateTranslator-Quality.exe") -Destination $qualityExecutable
Copy-Item -LiteralPath (Join-Path $dist "PrivateTranslator-Lite\Install-Model.exe") -Destination $modelInstallerExecutable
Copy-Item -Path (Join-Path $dist "PrivateTranslator-Lite\runtime\llama.cpp\*") -Destination $runtimeDestination -Recurse
Copy-Item -Path (Join-Path $dist "PrivateTranslator-Lite\licenses\*") -Destination $licenseDestination
Copy-Item -LiteralPath (Join-Path $root "configs\lite.json") -Destination $configDestination
Copy-Item -LiteralPath (Join-Path $root "configs\quality.json") -Destination $configDestination
Copy-Item -LiteralPath (Join-Path $root "LICENSE") -Destination $bundle
Copy-Item -LiteralPath (Join-Path $root "THIRD_PARTY_NOTICES.md") -Destination $bundle
$releaseComponentManifest = Get-Content -LiteralPath (Join-Path $root "packaging\component-manifest.json") -Raw | ConvertFrom-Json
$releaseComponentManifest.application | Add-Member -NotePropertyName "git_commit" -NotePropertyValue $releaseGitCommit -Force
$releaseComponentManifest | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath (Join-Path $bundle "component-manifest.json") -Encoding utf8
Copy-Item -LiteralPath (Join-Path $root "packaging\trusted-artifacts.json") -Destination $bundle
Copy-Item -LiteralPath (Join-Path $root "packaging\README-Release.txt") -Destination (Join-Path $bundle "README.txt")
Copy-Item -LiteralPath (Join-Path $root "packaging\README-Release.ko.txt") -Destination (Join-Path $bundle "README.ko.txt")

$bundledLicenseNames = @(Get-ChildItem -LiteralPath $licenseDestination -File |
    ForEach-Object { $_.Name.ToLowerInvariant() } |
    Sort-Object)
$trustedLicenseNames = @($trustedManifest.licenses |
    ForEach-Object { ([string] $_.path).ToLowerInvariant() } |
    Sort-Object)
if (($bundledLicenseNames -join "`n") -ne ($trustedLicenseNames -join "`n")) {
    throw "Release license inventory does not exactly match the trusted artifact manifest."
}
foreach ($license in $trustedManifest.licenses) {
    Assert-TrustedReleaseFile -Path (Join-Path $licenseDestination $license.path) -Specification $license
}

& (Join-Path $PSScriptRoot "generate-supply-chain.ps1") `
    -OutputDirectory (Join-Path $bundle "supply-chain") `
    -GitCommit $releaseGitCommit

$releaseExecutables = @($liteExecutable, $qualityExecutable, $modelInstallerExecutable)
$unsignedBinaryHashes = @($releaseExecutables | ForEach-Object {
    (Get-FileHash -Algorithm SHA256 -LiteralPath $_).Hash
} | Select-Object -Unique)
if ($unsignedBinaryHashes.Count -ne 1) {
    throw "Lite, Quality, and Install-Model executables must start as byte-for-byte copies of the same Rust binary."
}
if ($UnsignedDevelopment) {
    foreach ($executable in $releaseExecutables) {
        Assert-UnsignedExecutable -Path $executable
    }
    @(
        "UNSIGNED DEVELOPMENT ARTIFACT"
        ""
        "This archive is created only for local validation."
        "Do not publish or redistribute it as a Private Translator release."
    ) | Set-Content -LiteralPath (Join-Path $bundle "UNSIGNED-DEVELOPMENT.txt") -Encoding ascii
}
else {
    foreach ($executable in $releaseExecutables) {
        $signArguments = @("sign", "/fd", "SHA256", "/tr", $TimestampUrl, "/td", "SHA256", "/sha1", $signingCertificate.Thumbprint, "/s", "My")
        if ($signingCertificate.Location -eq "LocalMachine") {
            $signArguments += "/sm"
        }
        $signArguments += $executable
        & $signTool @signArguments
        if ($LASTEXITCODE -ne 0) {
            throw "signtool signing failed with exit code $LASTEXITCODE`: $executable"
        }
        Assert-SignedExecutable -Path $executable -ExpectedThumbprint $signingCertificate.Thumbprint -SignTool $signTool
    }
}

$bundledModels = @(Get-ChildItem -LiteralPath $bundle -Recurse -File -Filter "*.gguf")
if ($bundledModels.Count -ne 0) {
    throw "Thin release archives must not contain GGUF model files."
}

$runtimeFiles = @(Get-ChildItem -LiteralPath $runtimeDestination -File)
if (@($runtimeFiles | Where-Object { $_.Extension -eq ".exe" }).Count -ne 1 -or
    @($runtimeFiles | Where-Object { $_.Name -eq "llama-server.exe" }).Count -ne 1) {
    throw "Release runtime must contain llama-server.exe and no other executables."
}
$expectedApplicationExecutables = @(
    "Install-Model.exe",
    "PrivateTranslator-Lite.exe",
    "PrivateTranslator-Quality.exe"
) | Sort-Object
$actualApplicationExecutables = @(Get-ChildItem -LiteralPath $bundle -File -Filter "*.exe" |
    ForEach-Object { $_.Name } |
    Sort-Object)
if (($expectedApplicationExecutables -join "`n") -cne ($actualApplicationExecutables -join "`n")) {
    throw "Release root must contain exactly the signed Lite, Quality, and Install-Model executables."
}
$allReleaseExecutables = @(Get-ChildItem -LiteralPath $bundle -Recurse -File -Filter "*.exe")
if ($allReleaseExecutables.Count -ne 4) {
    throw "Release must contain exactly three signed application executables and the pinned llama-server.exe."
}

Assert-NoExecutableScripts -Path $bundle

Compress-Archive -Path $bundle -DestinationPath $archive -CompressionLevel Optimal
Assert-ZipContainsNoExecutableScripts -Path $archive
$archiveLength = (Get-Item -LiteralPath $archive).Length
if ($archiveLength -gt 150MB) {
    throw "Thin release archive exceeds the 150 MB size gate: $archiveLength bytes"
}
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
"$hash  $([System.IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath $sidecar -Encoding ascii

Get-Item -LiteralPath $archive, $sidecar | Select-Object FullName, Length
if ($UnsignedDevelopment) {
    Write-Warning "Created an explicitly marked unsigned development archive. It is not eligible for public release."
}
else {
    Write-Host "Created a signed public release. Lite, Quality, and Install-Model executables have valid Authenticode signatures and RFC 3161 timestamps."
}
