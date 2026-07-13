[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $Version,

    [Parameter(Mandatory)]
    [string] $RepositoryRoot,

    [switch] $UnsignedDevelopment,

    [scriptblock] $RemoteTagProbeForTesting
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ($UnsignedDevelopment) {
    return
}

if ($Version -notmatch '^[0-9A-Za-z][0-9A-Za-z.+-]*$') {
    throw "Release version contains unsafe tag characters: $Version"
}
if (-not (Test-Path -LiteralPath $RepositoryRoot -PathType Container)) {
    throw "Release repository root does not exist: $RepositoryRoot"
}

$gitCommand = Get-Command git -ErrorAction SilentlyContinue
if ($null -eq $gitCommand) {
    throw "Public release creation requires git so tag reuse can be checked locally and on origin."
}

$tagName = "v$Version"
$tagRef = "refs/tags/$tagName"

Push-Location $RepositoryRoot
try {
    $repositoryProbe = @(& $gitCommand.Source rev-parse --is-inside-work-tree 2>&1)
    $repositoryProbeExit = $LASTEXITCODE
    if ($repositoryProbeExit -ne 0 -or ($repositoryProbe -join "`n").Trim() -ne "true") {
        throw "Public release tag verification requires a valid Git worktree: $RepositoryRoot"
    }

    $worktreeProbe = @(& $gitCommand.Source status --porcelain=v1 --untracked-files=all 2>&1)
    $worktreeProbeExit = $LASTEXITCODE
    if ($worktreeProbeExit -ne 0) {
        $detail = ($worktreeProbe -join "`n").Trim()
        throw "Could not verify that the public release worktree is clean (git exit $worktreeProbeExit): $detail"
    }
    if (@($worktreeProbe).Count -ne 0) {
        throw "Public release creation requires a clean worktree. Commit or remove every tracked and untracked change first."
    }

    $headProbe = @(& $gitCommand.Source rev-parse --verify HEAD 2>&1)
    $headProbeExit = $LASTEXITCODE
    $headCommit = ($headProbe -join "`n").Trim()
    if ($headProbeExit -ne 0 -or $headCommit -notmatch '^[0-9a-fA-F]{40}$') {
        throw "Could not resolve the exact HEAD commit for public release tag '$tagName'."
    }

    $localProbe = @(& $gitCommand.Source tag --list $tagName 2>&1)
    $localProbeExit = $LASTEXITCODE
    if ($localProbeExit -ne 0) {
        $detail = ($localProbe -join "`n").Trim()
        throw "Could not verify local public release tag '$tagName' (git exit $localProbeExit): $detail"
    }
    if (@($localProbe).Count -eq 0) {
        throw "Public release creation requires a local annotated tag '$tagName' that points exactly to HEAD."
    }
    if (@($localProbe).Count -ne 1 -or ($localProbe -join "`n").Trim() -cne $tagName) {
        throw "Local public release tag lookup for '$tagName' returned an ambiguous result."
    }

    $tagTypeProbe = @(& $gitCommand.Source cat-file -t $tagRef 2>&1)
    $tagTypeProbeExit = $LASTEXITCODE
    if ($tagTypeProbeExit -ne 0 -or ($tagTypeProbe -join "`n").Trim() -ne "tag") {
        throw "Public release tag '$tagName' must be an annotated or signed tag; lightweight tags are forbidden."
    }

    $tagCommitProbe = @(& $gitCommand.Source rev-list -n 1 $tagRef 2>&1)
    $tagCommitProbeExit = $LASTEXITCODE
    $tagCommit = ($tagCommitProbe -join "`n").Trim()
    if ($tagCommitProbeExit -ne 0 -or $tagCommit -cne $headCommit) {
        throw "Public release tag '$tagName' must point exactly to HEAD ($headCommit), but resolves to '$tagCommit'."
    }

    $originProbe = @(& $gitCommand.Source remote get-url origin 2>&1)
    $originProbeExit = $LASTEXITCODE
    if ($originProbeExit -ne 0 -or [string]::IsNullOrWhiteSpace(($originProbe -join "`n").Trim())) {
        $detail = ($originProbe -join "`n").Trim()
        throw "Could not resolve the origin remote while checking public release tag '$tagName' (git exit $originProbeExit): $detail"
    }

    if ($null -eq $RemoteTagProbeForTesting) {
        $remoteProbe = @(& $gitCommand.Source ls-remote --exit-code --tags origin $tagRef 2>&1)
        $remoteProbeExit = $LASTEXITCODE
    }
    else {
        $remoteProbeResult = & $RemoteTagProbeForTesting $tagRef
        if ($null -eq $remoteProbeResult -or $null -eq $remoteProbeResult.ExitCode) {
            throw "Test remote tag probe returned no exit code."
        }
        $remoteProbe = @($remoteProbeResult.Output)
        $remoteProbeExit = [int] $remoteProbeResult.ExitCode
    }
    if ($remoteProbeExit -eq 0) {
        throw "Public release tag '$tagName' already exists on origin. Version reuse is forbidden."
    }
    if ($remoteProbeExit -ne 2) {
        $detail = ($remoteProbe -join "`n").Trim()
        throw "Could not prove that public release tag '$tagName' is absent on origin (git exit $remoteProbeExit): $detail"
    }

    $headCommit.ToLowerInvariant()
}
finally {
    Pop-Location
}
