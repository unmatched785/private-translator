$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$binary = Join-Path $root "target\release\private-translator.exe"
$runtime = Join-Path $root "runtime\llama.cpp"
$model = Join-Path $root "models\Hy-MT2-1.8B-Q4_K_M.gguf"
$dist = Join-Path $root "dist"

foreach ($required in @($binary, $runtime, $model)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Required build component is missing: $required"
    }
}

function New-PortableProfile {
    param(
        [Parameter(Mandatory)] [string] $Folder,
        [Parameter(Mandatory)] [string] $Executable,
        [Parameter(Mandatory)] [string] $Readme,
        [Parameter(Mandatory)] [string] $Config
    )

    $destination = Join-Path $dist $Folder
    $runtimeDestination = Join-Path $destination "runtime\llama.cpp"
    $modelDestinationDirectory = Join-Path $destination "models"
    $licenseDestination = Join-Path $destination "licenses"
    $configDestination = Join-Path $destination "configs"

    New-Item -ItemType Directory -Force -Path $runtimeDestination | Out-Null
    New-Item -ItemType Directory -Force -Path $modelDestinationDirectory | Out-Null
    New-Item -ItemType Directory -Force -Path $licenseDestination | Out-Null
    New-Item -ItemType Directory -Force -Path $configDestination | Out-Null

    Copy-Item -LiteralPath $binary -Destination (Join-Path $destination $Executable) -Force
    Copy-Item -Path (Join-Path $runtime "*") -Destination $runtimeDestination -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $root "packaging\$Readme") -Destination (Join-Path $destination "README.txt") -Force
    Copy-Item -LiteralPath (Join-Path $root "packaging\component-manifest.json") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $root "THIRD_PARTY_NOTICES.md") -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $root "configs\$Config") -Destination $configDestination -Force
    Copy-Item -LiteralPath (Join-Path $root "runtime\licenses\llama.cpp-LICENSE") -Destination $licenseDestination -Force
    Copy-Item -LiteralPath (Join-Path $root "runtime\licenses\Hy-MT2-LICENSE") -Destination $licenseDestination -Force

    $modelDestination = Join-Path $modelDestinationDirectory "Hy-MT2-1.8B-Q4_K_M.gguf"
    if (Test-Path -LiteralPath $modelDestination) {
        $sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $model).Hash
        $destinationHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $modelDestination).Hash
        if ($sourceHash -ne $destinationHash) {
            throw "Existing packaged model has a different hash: $modelDestination"
        }
    } else {
        New-Item -ItemType HardLink -Path $modelDestination -Target $model | Out-Null
    }
}

New-Item -ItemType Directory -Force -Path $dist | Out-Null
New-PortableProfile -Folder "PrivateTranslator-Lite" -Executable "PrivateTranslator-Lite.exe" -Readme "README-Lite.txt" -Config "lite.json"
New-PortableProfile -Folder "PrivateTranslator-Quality" -Executable "PrivateTranslator-Quality.exe" -Readme "README-Quality.txt" -Config "quality.json"

Get-ChildItem -Path $dist -Filter "PrivateTranslator-*.exe" -Recurse |
    Select-Object FullName, Length
