Set-StrictMode -Version Latest

function Test-ExecutableScriptPath {
    param([Parameter(Mandatory)] [string] $Path)

    $extension = [System.IO.Path]::GetExtension($Path).ToLowerInvariant()
    return @(
        ".bat", ".cmd", ".hta", ".js", ".jse", ".ps1", ".psd1", ".psm1",
        ".py", ".pyw", ".sh", ".vbe", ".vbs", ".wsf", ".wsh"
    ) -contains $extension
}

function Assert-NoExecutableScripts {
    param([Parameter(Mandatory)] [string] $Path)

    $scripts = @(Get-ChildItem -LiteralPath $Path -Recurse -File | Where-Object {
        Test-ExecutableScriptPath -Path $_.Name
    })
    if ($scripts.Count -ne 0) {
        $names = @($scripts | ForEach-Object { $_.FullName }) -join [Environment]::NewLine
        throw "Package contains forbidden executable scripts:`n$names"
    }
}

function Assert-ZipContainsNoExecutableScripts {
    param([Parameter(Mandatory)] [string] $Path)

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($Path)
    try {
        $scriptEntries = @($zip.Entries | Where-Object {
            Test-ExecutableScriptPath -Path $_.FullName
        })
        if ($scriptEntries.Count -ne 0) {
            $names = @($scriptEntries | ForEach-Object { $_.FullName }) -join [Environment]::NewLine
            throw "Final release ZIP contains forbidden executable scripts:`n$names"
        }
    }
    finally {
        $zip.Dispose()
    }
}
