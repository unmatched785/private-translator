$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$assertScript = Join-Path $PSScriptRoot "assert-release-tag-available.ps1"
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("private-translator-release-tag-test-" + [guid]::NewGuid().ToString("N"))
$repository = Join-Path $testRoot "worktree"
$version = "9.8.7-test"
$tagName = "v$version"
$remoteTagAbsent = {
    [pscustomobject]@{ ExitCode = 2; Output = @() }
}
$remoteTagPresent = {
    [pscustomobject]@{ ExitCode = 0; Output = @("test refs/tags/v9.8.7-test") }
}
$remoteLookupFailed = {
    [pscustomobject]@{ ExitCode = 128; Output = @("simulated network failure") }
}

function Invoke-GitTestCommand {
    param(
        [Parameter(Mandatory)] [string] $WorkingDirectory,
        [Parameter(Mandatory)] [string[]] $Arguments
    )

    Push-Location $WorkingDirectory
    try {
        $output = @(& git @Arguments 2>&1)
        if ($LASTEXITCODE -ne 0) {
            throw "Test setup git command failed: git $($Arguments -join ' ')`n$($output -join "`n")"
        }
    }
    finally {
        Pop-Location
    }
}

function Assert-ThrowsLike {
    param(
        [Parameter(Mandatory)] [scriptblock] $Action,
        [Parameter(Mandatory)] [string] $Pattern
    )

    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -notlike $Pattern) {
            throw "Expected error like '$Pattern', got: $($_.Exception.Message)"
        }
        return
    }
    throw "Expected action to fail with an error like '$Pattern'."
}

New-Item -ItemType Directory -Force -Path $testRoot, $repository | Out-Null
try {
    # Unsigned development packaging must be usable without a Git repository or network.
    & $assertScript -Version $version -RepositoryRoot $testRoot -UnsignedDevelopment

    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("init")
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("config", "user.name", "Release invariant test")
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("config", "user.email", "release-invariant@example.invalid")
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("commit", "--allow-empty", "-m", "test root")
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("remote", "add", "origin", "https://example.invalid/private-translator.git")

    Assert-ThrowsLike -Pattern "*requires a local annotated tag*" -Action {
        & $assertScript -Version $version -RepositoryRoot $repository -RemoteTagProbeForTesting $remoteTagAbsent
    }

    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("tag", $tagName)
    Assert-ThrowsLike -Pattern "*lightweight tags are forbidden*" -Action {
        & $assertScript -Version $version -RepositoryRoot $repository -RemoteTagProbeForTesting $remoteTagAbsent
    }
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("tag", "--delete", $tagName)

    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("tag", "--annotate", $tagName, "-m", "release tag")
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("commit", "--allow-empty", "-m", "newer commit")
    Assert-ThrowsLike -Pattern "*must point exactly to HEAD*" -Action {
        & $assertScript -Version $version -RepositoryRoot $repository -RemoteTagProbeForTesting $remoteTagAbsent
    }
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("tag", "--delete", $tagName)
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("tag", "--annotate", $tagName, "-m", "release tag")
    $expectedCommit = (& git -C $repository rev-parse HEAD).Trim().ToLowerInvariant()
    $actualCommit = (& $assertScript -Version $version -RepositoryRoot $repository -RemoteTagProbeForTesting $remoteTagAbsent).Trim().ToLowerInvariant()
    if ($actualCommit -cne $expectedCommit) {
        throw "Tag assertion returned commit '$actualCommit', expected '$expectedCommit'."
    }

    Set-Content -LiteralPath (Join-Path $repository "dirty.txt") -Value "dirty" -Encoding ascii
    Assert-ThrowsLike -Pattern "*requires a clean worktree*" -Action {
        & $assertScript -Version $version -RepositoryRoot $repository -RemoteTagProbeForTesting $remoteTagAbsent
    }
    Remove-Item -LiteralPath (Join-Path $repository "dirty.txt") -Force

    Assert-ThrowsLike -Pattern "*already exists on origin*" -Action {
        & $assertScript -Version $version -RepositoryRoot $repository -RemoteTagProbeForTesting $remoteTagPresent
    }

    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("tag", "--delete", $tagName)
    Invoke-GitTestCommand -WorkingDirectory $repository -Arguments @("tag", "--annotate", "v9.8.8-test", "-m", "network failure tag")
    Assert-ThrowsLike -Pattern "*Could not prove*absent on origin*" -Action {
        & $assertScript -Version "9.8.8-test" -RepositoryRoot $repository -RemoteTagProbeForTesting $remoteLookupFailed
    }

    Write-Host "Release tag availability tests passed."
}
finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
