$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "package-policy.ps1")

function Assert-Rejected {
    param(
        [Parameter(Mandatory)] [scriptblock] $Action,
        [Parameter(Mandatory)] [string] $ExpectedMessage
    )

    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -notlike "*$ExpectedMessage*") {
            throw
        }
        return
    }
    throw "Package policy test expected rejection containing: $ExpectedMessage"
}

$temporaryRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$temporary = Join-Path $temporaryRoot "private-translator-package-policy-$([guid]::NewGuid())"
$archive = Join-Path $temporaryRoot "private-translator-package-policy-$([guid]::NewGuid()).zip"
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    Set-Content -LiteralPath (Join-Path $temporary "README.txt") -Value "safe fixture" -Encoding ascii
    Assert-NoExecutableScripts -Path $temporary

    $forbidden = Join-Path $temporary "Install-Model.cmd"
    Set-Content -LiteralPath $forbidden -Value "@echo off" -Encoding ascii
    Assert-Rejected -ExpectedMessage "forbidden executable scripts" -Action {
        Assert-NoExecutableScripts -Path $temporary
    }

    Compress-Archive -Path $temporary -DestinationPath $archive
    Assert-Rejected -ExpectedMessage "forbidden executable scripts" -Action {
        Assert-ZipContainsNoExecutableScripts -Path $archive
    }
}
finally {
    $resolvedTemporary = [System.IO.Path]::GetFullPath($temporary)
    if (-not $resolvedTemporary.StartsWith($temporaryRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clean package policy fixture outside the temporary directory."
    }
    if (Test-Path -LiteralPath $temporary) {
        Remove-Item -LiteralPath $temporary -Recurse -Force
    }
    if (Test-Path -LiteralPath $archive) {
        Remove-Item -LiteralPath $archive -Force
    }
}

Write-Host "Package policy tests passed."
