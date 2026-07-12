[CmdletBinding()]
param(
    [switch] $Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$componentManifestPath = Join-Path $root "packaging\component-manifest.json"
$trustedManifestPath = Join-Path $root "packaging\trusted-artifacts.json"
$componentManifest = Get-Content -LiteralPath $componentManifestPath -Raw | ConvertFrom-Json
$trustedManifest = Get-Content -LiteralPath $trustedManifestPath -Raw | ConvertFrom-Json

if ($trustedManifest.schema_version -ne 1) {
    throw "Unsupported trusted artifact manifest version: $($trustedManifest.schema_version)"
}

$cacheDirectory = Join-Path $root ".cache\bootstrap"
$runtimeDirectory = Join-Path $root "runtime\llama.cpp"
$licenseDirectory = Join-Path $root "runtime\licenses"
$modelDirectory = Join-Path $root "models"
$modelPath = Join-Path $modelDirectory $componentManifest.lite_model.file
$runtimeArchive = Join-Path $cacheDirectory $componentManifest.runtime.asset
$runtimeDownloadUrl = "https://github.com/ggml-org/llama.cpp/releases/download/$($componentManifest.runtime.tag)/$($componentManifest.runtime.asset)"
$modelDownloadUrl = $componentManifest.lite_model.download_url

function Get-RemoteFile {
    param(
        [Parameter(Mandatory)] [string] $Uri,
        [Parameter(Mandatory)] [string] $Destination
    )

    $parent = Split-Path -Parent $Destination
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    if (Test-Path -LiteralPath $Destination) {
        Remove-Item -LiteralPath $Destination -Force
    }

    $curl = Get-Command curl.exe -ErrorAction SilentlyContinue
    if ($null -ne $curl) {
        & $curl.Source --location --fail --retry 3 --output $Destination $Uri
        if ($LASTEXITCODE -ne 0) {
            throw "Download failed with exit code $LASTEXITCODE`: $Uri"
        }
        return
    }

    Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $Destination
}

function Test-FileSpecification {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] $Specification
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $false
    }
    if ((Get-Item -LiteralPath $Path).Length -ne [long] $Specification.bytes) {
        return $false
    }
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
    return $actualHash -eq $Specification.sha256.ToLowerInvariant()
}

function Assert-FileSpecification {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] $Specification
    )

    if (-not (Test-FileSpecification -Path $Path -Specification $Specification)) {
        throw "Artifact failed size or SHA-256 verification: $Path"
    }
}

function Test-Runtime {
    if (-not (Test-Path -LiteralPath $runtimeDirectory -PathType Container)) {
        return $false
    }
    $expectedNames = @($trustedManifest.runtime.files | ForEach-Object { $_.path.ToLowerInvariant() } | Sort-Object)
    $actualNames = @(Get-ChildItem -LiteralPath $runtimeDirectory -File |
        Where-Object { $_.Extension -in @(".exe", ".dll") } |
        ForEach-Object { $_.Name.ToLowerInvariant() } |
        Sort-Object)
    if (($expectedNames -join "`n") -ne ($actualNames -join "`n")) {
        return $false
    }
    foreach ($specification in $trustedManifest.runtime.files) {
        if (-not (Test-FileSpecification -Path (Join-Path $runtimeDirectory $specification.path) -Specification $specification)) {
            return $false
        }
    }
    return $true
}

New-Item -ItemType Directory -Force -Path $cacheDirectory, $modelDirectory, $licenseDirectory | Out-Null

if ($Force -or -not (Test-Runtime)) {
    Write-Host "Preparing pinned llama.cpp runtime..."
    $archiveIsTrusted = $false
    if (Test-Path -LiteralPath $runtimeArchive -PathType Leaf) {
        $archiveHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $runtimeArchive).Hash.ToLowerInvariant()
        $archiveIsTrusted = $archiveHash -eq $componentManifest.runtime.sha256.ToLowerInvariant()
    }
    if (-not $archiveIsTrusted) {
        Get-RemoteFile -Uri $runtimeDownloadUrl -Destination $runtimeArchive
    }
    $archiveHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $runtimeArchive).Hash.ToLowerInvariant()
    if ($archiveHash -ne $componentManifest.runtime.sha256.ToLowerInvariant()) {
        throw "llama.cpp archive failed SHA-256 verification."
    }

    $extractDirectory = Join-Path $cacheDirectory "llama-$($componentManifest.runtime.tag)-extracted"
    if (Test-Path -LiteralPath $extractDirectory) {
        Remove-Item -LiteralPath $extractDirectory -Recurse -Force
    }
    Expand-Archive -LiteralPath $runtimeArchive -DestinationPath $extractDirectory -Force
    $serverCandidates = @(Get-ChildItem -LiteralPath $extractDirectory -Recurse -File -Filter "llama-server.exe")
    if ($serverCandidates.Count -ne 1) {
        throw "Expected exactly one llama-server.exe in the pinned runtime archive."
    }
    $runtimeSource = $serverCandidates[0].Directory.FullName

    if (Test-Path -LiteralPath $runtimeDirectory) {
        Remove-Item -LiteralPath $runtimeDirectory -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $runtimeDirectory | Out-Null
    foreach ($specification in $trustedManifest.runtime.files) {
        if ((Split-Path -Leaf $specification.path) -ne $specification.path) {
            throw "Nested runtime paths are not allowed in the trusted manifest."
        }
        $source = Join-Path $runtimeSource $specification.path
        Assert-FileSpecification -Path $source -Specification $specification
        Copy-Item -LiteralPath $source -Destination (Join-Path $runtimeDirectory $specification.path)
    }
}

$modelSpecification = $trustedManifest.models |
    Where-Object { $_.id -eq "hy-mt2-1.8b-q4" } |
    Select-Object -First 1
if ($null -eq $modelSpecification) {
    throw "The Lite model is missing from the trusted artifact manifest."
}

if ($Force -or -not (Test-FileSpecification -Path $modelPath -Specification $modelSpecification)) {
    if ((Test-Path -LiteralPath $modelPath) -and -not $Force) {
        throw "The existing model failed verification. Review it, then rerun with -Force to replace it."
    }
    Write-Host "Downloading the pinned Hy-MT2 model (about 1.13 GB)..."
    $modelDownload = Join-Path $cacheDirectory "$($componentManifest.lite_model.file).download"
    Get-RemoteFile -Uri $modelDownloadUrl -Destination $modelDownload
    Assert-FileSpecification -Path $modelDownload -Specification $modelSpecification
    Move-Item -LiteralPath $modelDownload -Destination $modelPath -Force
}

$licenses = @(
    @{
        Uri = "https://raw.githubusercontent.com/ggml-org/llama.cpp/$($componentManifest.runtime.tag)/LICENSE"
        Destination = Join-Path $licenseDirectory "llama.cpp-LICENSE"
    },
    @{
        Uri = "https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF/resolve/main/LICENSE.txt?download=true"
        Destination = Join-Path $licenseDirectory "Hy-MT2-LICENSE"
    }
)
foreach ($license in $licenses) {
    if ($Force -or -not (Test-Path -LiteralPath $license.Destination -PathType Leaf)) {
        Get-RemoteFile -Uri $license.Uri -Destination $license.Destination
    }
    if ((Get-Item -LiteralPath $license.Destination).Length -lt 500) {
        throw "Downloaded license file is unexpectedly small: $($license.Destination)"
    }
}

if (-not (Test-Runtime)) {
    throw "Runtime verification failed after installation."
}
Assert-FileSpecification -Path $modelPath -Specification $modelSpecification

Write-Host "Bootstrap complete. The pinned runtime, model, and licenses are verified."
