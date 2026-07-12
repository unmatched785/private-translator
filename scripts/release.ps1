[CmdletBinding()]
param(
    [string] $Version
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$dist = Join-Path $root "dist"
$cargoManifest = Get-Content -LiteralPath (Join-Path $root "Cargo.toml") -Raw
if (-not $Version) {
    $versionMatch = [regex]::Match($cargoManifest, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $versionMatch.Success) {
        throw "Could not read the package version from Cargo.toml."
    }
    $Version = $versionMatch.Groups[1].Value
}

& (Join-Path $PSScriptRoot "package.ps1")

$bundleName = "PrivateTranslator-v$Version-Windows-x64"
$bundle = Join-Path $dist $bundleName
$archive = Join-Path $dist "$bundleName.zip"
$sidecar = "$archive.sha256"

foreach ($path in @($bundle, $archive, $sidecar)) {
    if (Test-Path -LiteralPath $path) {
        Remove-Item -LiteralPath $path -Recurse -Force
    }
}

$runtimeDestination = Join-Path $bundle "runtime\llama.cpp"
$modelDestinationDirectory = Join-Path $bundle "models"
$licenseDestination = Join-Path $bundle "licenses"
$configDestination = Join-Path $bundle "configs"
New-Item -ItemType Directory -Force -Path $runtimeDestination, $modelDestinationDirectory, $licenseDestination, $configDestination | Out-Null

Copy-Item -LiteralPath (Join-Path $dist "PrivateTranslator-Lite\PrivateTranslator-Lite.exe") -Destination $bundle
Copy-Item -LiteralPath (Join-Path $dist "PrivateTranslator-Quality\PrivateTranslator-Quality.exe") -Destination $bundle
Copy-Item -Path (Join-Path $root "runtime\llama.cpp\*") -Destination $runtimeDestination -Recurse
Copy-Item -Path (Join-Path $root "runtime\licenses\*") -Destination $licenseDestination
Copy-Item -LiteralPath (Join-Path $root "configs\lite.json") -Destination $configDestination
Copy-Item -LiteralPath (Join-Path $root "configs\quality.json") -Destination $configDestination
Copy-Item -LiteralPath (Join-Path $root "LICENSE") -Destination $bundle
Copy-Item -LiteralPath (Join-Path $root "THIRD_PARTY_NOTICES.md") -Destination $bundle
Copy-Item -LiteralPath (Join-Path $root "packaging\component-manifest.json") -Destination $bundle
Copy-Item -LiteralPath (Join-Path $root "packaging\trusted-artifacts.json") -Destination $bundle
Copy-Item -LiteralPath (Join-Path $root "packaging\README-Release.txt") -Destination (Join-Path $bundle "README.txt")
Copy-Item -LiteralPath (Join-Path $root "packaging\README-Release.ko.txt") -Destination (Join-Path $bundle "README.ko.txt")

$modelSource = Join-Path $root "models\Hy-MT2-1.8B-Q4_K_M.gguf"
$modelDestination = Join-Path $modelDestinationDirectory "Hy-MT2-1.8B-Q4_K_M.gguf"
New-Item -ItemType HardLink -Path $modelDestination -Target $modelSource | Out-Null

Compress-Archive -Path $bundle -DestinationPath $archive -CompressionLevel NoCompression
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
"$hash  $([System.IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath $sidecar -Encoding ascii

Get-Item -LiteralPath $archive, $sidecar | Select-Object FullName, Length
