[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $SourceDirectory,

    [Parameter(Mandatory)]
    [string] $DestinationDirectory,

    [Parameter(Mandatory)]
    [string] $ManifestPath,

    [string] $SmokeTestModel
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$source = (Resolve-Path -LiteralPath $SourceDirectory).Path
$manifestFile = (Resolve-Path -LiteralPath $ManifestPath).Path
$destination = if ([System.IO.Path]::IsPathRooted($DestinationDirectory)) {
    [System.IO.Path]::GetFullPath($DestinationDirectory)
}
else {
    [System.IO.Path]::GetFullPath((Join-Path $root $DestinationDirectory))
}
$rootPrefix = $root.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $destination.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Minimized runtime output must stay inside the repository: $destination"
}
if ($source.Equals($destination, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Runtime source and destination must be different directories."
}

$manifest = Get-Content -LiteralPath $manifestFile -Raw | ConvertFrom-Json
if ($manifest.schema_version -ne 1 -or $null -eq $manifest.runtime -or $null -eq $manifest.runtime.files) {
    throw "Unsupported trusted runtime manifest: $manifestFile"
}

function Assert-TrustedFile {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] $Specification
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Trusted runtime component is missing: $Path"
    }
    if ((Get-Item -LiteralPath $Path).Length -ne [long] $Specification.bytes) {
        throw "Trusted runtime component size mismatch: $Path"
    }
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
    if ($actualHash -ne ([string] $Specification.sha256).ToLowerInvariant()) {
        throw "Trusted runtime component SHA-256 mismatch: $Path"
    }
}

function Get-PeImports {
    param([Parameter(Mandatory)] [string] $Path)

    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        if ($stream.Length -lt 256) {
            throw "PE file is unexpectedly small: $Path"
        }
        $stream.Position = 0
        if ($reader.ReadUInt16() -ne 0x5A4D) {
            throw "File does not have an MZ header: $Path"
        }
        $stream.Position = 0x3C
        $peOffset = $reader.ReadUInt32()
        if ($peOffset -gt ($stream.Length - 256)) {
            throw "PE header offset is invalid: $Path"
        }
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x00004550) {
            throw "File does not have a PE header: $Path"
        }

        $stream.Position = $peOffset + 6
        $numberOfSections = $reader.ReadUInt16()
        $stream.Position = $peOffset + 20
        $sizeOfOptionalHeader = $reader.ReadUInt16()
        $optionalHeader = $peOffset + 24
        $stream.Position = $optionalHeader
        $magic = $reader.ReadUInt16()
        $dataDirectoryOffset = switch ($magic) {
            0x010B { 96 }
            0x020B { 112 }
            default { throw "Unsupported PE optional-header magic 0x$($magic.ToString('X')): $Path" }
        }

        $sectionHeaders = $optionalHeader + $sizeOfOptionalHeader
        $sections = @()
        for ($index = 0; $index -lt $numberOfSections; $index++) {
            $section = $sectionHeaders + (40 * $index)
            $stream.Position = $section + 8
            $virtualSize = $reader.ReadUInt32()
            $virtualAddress = $reader.ReadUInt32()
            $rawSize = $reader.ReadUInt32()
            $rawAddress = $reader.ReadUInt32()
            $sections += [pscustomobject]@{
                VirtualSize    = [uint64] $virtualSize
                VirtualAddress = [uint64] $virtualAddress
                RawSize        = [uint64] $rawSize
                RawAddress     = [uint64] $rawAddress
            }
        }

        function Convert-RvaToFileOffset {
            param([Parameter(Mandatory)] [uint64] $Rva)
            foreach ($section in $sections) {
                $size = [Math]::Max($section.VirtualSize, $section.RawSize)
                if ($Rva -ge $section.VirtualAddress -and $Rva -lt ($section.VirtualAddress + $size)) {
                    return [uint64] ($section.RawAddress + ($Rva - $section.VirtualAddress))
                }
            }
            if ($Rva -lt [uint64] $sectionHeaders) {
                return $Rva
            }
            throw "Could not map PE RVA 0x$($Rva.ToString('X')) in $Path"
        }

        function Read-AsciiZ {
            param([Parameter(Mandatory)] [uint64] $Offset)
            if ($Offset -ge [uint64] $stream.Length) {
                throw "PE string offset is outside the file: $Path"
            }
            $stream.Position = [long] $Offset
            $bytes = [System.Collections.Generic.List[byte]]::new()
            while ($stream.Position -lt $stream.Length) {
                $value = $reader.ReadByte()
                if ($value -eq 0) {
                    return [System.Text.Encoding]::ASCII.GetString($bytes.ToArray())
                }
                $bytes.Add($value)
                if ($bytes.Count -gt 4096) {
                    throw "PE import name is unexpectedly long: $Path"
                }
            }
            throw "PE import name is not null terminated: $Path"
        }

        $stream.Position = $optionalHeader + $dataDirectoryOffset + 8
        $importRva = $reader.ReadUInt32()
        $importSize = $reader.ReadUInt32()
        if ($importRva -eq 0 -or $importSize -eq 0) {
            return @()
        }

        $descriptorOffset = Convert-RvaToFileOffset -Rva $importRva
        $imports = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
        $maximumDescriptors = [Math]::Max(1, [Math]::Ceiling($importSize / 20.0))
        for ($index = 0; $index -lt $maximumDescriptors; $index++) {
            $offset = $descriptorOffset + (20 * $index)
            if (($offset + 20) -gt [uint64] $stream.Length) {
                throw "PE import descriptor is outside the file: $Path"
            }
            $stream.Position = [long] $offset
            $originalFirstThunk = $reader.ReadUInt32()
            $timeDateStamp = $reader.ReadUInt32()
            $forwarderChain = $reader.ReadUInt32()
            $nameRva = $reader.ReadUInt32()
            $firstThunk = $reader.ReadUInt32()
            if (($originalFirstThunk -bor $timeDateStamp -bor $forwarderChain -bor $nameRva -bor $firstThunk) -eq 0) {
                break
            }
            if ($nameRva -eq 0) {
                throw "PE import descriptor has no DLL name: $Path"
            }
            $nameOffset = Convert-RvaToFileOffset -Rva $nameRva
            $null = $imports.Add((Read-AsciiZ -Offset $nameOffset).ToLowerInvariant())
        }
        return @($imports | Sort-Object)
    }
    finally {
        $reader.Dispose()
        $stream.Dispose()
    }
}

$specifications = @($manifest.runtime.files)
$expectedNames = @($specifications | ForEach-Object { ([string] $_.path).ToLowerInvariant() })
if (($expectedNames | Sort-Object -Unique).Count -ne $expectedNames.Count) {
    throw "Trusted runtime manifest contains duplicate paths."
}
foreach ($specification in $specifications) {
    if ((Split-Path -Leaf $specification.path) -ne $specification.path) {
        throw "Nested runtime paths are not allowed: $($specification.path)"
    }
    if ([System.IO.Path]::GetExtension($specification.path).ToLowerInvariant() -notin @(".exe", ".dll")) {
        throw "Runtime manifest may contain only EXE and DLL files: $($specification.path)"
    }
}

$manifestExecutables = @($specifications | Where-Object { $_.path -like "*.exe" } | ForEach-Object { $_.path })
if ($manifestExecutables.Count -ne 1 -or $manifestExecutables[0] -ne "llama-server.exe") {
    throw "The minimized runtime must contain only llama-server.exe; found: $($manifestExecutables -join ', ')"
}
$unrelatedImplementations = @($specifications |
    Where-Object { $_.path -like "*-impl.dll" -and $_.path -ne "llama-server-impl.dll" } |
    ForEach-Object { $_.path })
if ($unrelatedImplementations.Count -gt 0) {
    throw "Unrelated llama.cpp tool implementations remain in the runtime manifest: $($unrelatedImplementations -join ', ')"
}

$sourceCpuBackends = @(Get-ChildItem -LiteralPath $source -File -Filter "ggml-cpu-*.dll" |
    ForEach-Object { $_.Name.ToLowerInvariant() } |
    Sort-Object)
$manifestCpuBackends = @($expectedNames | Where-Object { $_ -like "ggml-cpu-*.dll" } | Sort-Object)
if ($sourceCpuBackends.Count -eq 0 -or ($sourceCpuBackends -join "`n") -ne ($manifestCpuBackends -join "`n")) {
    throw "The minimized runtime must retain every pinned CPU backend for portable x64 dispatch."
}

$sourceLocalFiles = @{}
foreach ($file in Get-ChildItem -LiteralPath $source -File |
    Where-Object { $_.Extension.ToLowerInvariant() -in @(".exe", ".dll") }) {
    $sourceLocalFiles[$file.Name.ToLowerInvariant()] = $file.FullName
}
$expectedSet = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($expectedName in $expectedNames) {
    $null = $expectedSet.Add($expectedName)
}
foreach ($specification in $specifications) {
    $sourcePath = Join-Path $source $specification.path
    Assert-TrustedFile -Path $sourcePath -Specification $specification
    foreach ($import in Get-PeImports -Path $sourcePath) {
        if ($sourceLocalFiles.ContainsKey($import) -and -not $expectedSet.Contains($import)) {
            throw "Static PE dependency '$import' required by '$($specification.path)' is omitted from the runtime manifest."
        }
    }
}

if (Test-Path -LiteralPath $destination) {
    $resolvedDestination = (Resolve-Path -LiteralPath $destination).Path
    if (-not $resolvedDestination.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to replace runtime output outside the repository: $resolvedDestination"
    }
    Remove-Item -LiteralPath $resolvedDestination -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $destination | Out-Null
foreach ($specification in $specifications) {
    Copy-Item -LiteralPath (Join-Path $source $specification.path) -Destination (Join-Path $destination $specification.path)
}

$actualNames = @(Get-ChildItem -LiteralPath $destination -File |
    ForEach-Object { $_.Name.ToLowerInvariant() } |
    Sort-Object)
if (($actualNames -join "`n") -ne (@($expectedNames | Sort-Object) -join "`n")) {
    throw "Minimized runtime inventory does not exactly match the trusted manifest."
}

$server = Join-Path $destination "llama-server.exe"
$versionOutput = & $server --version 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($versionOutput)) {
    throw "Minimized llama-server failed its Windows loader/version smoke test."
}

if (-not [string]::IsNullOrWhiteSpace($SmokeTestModel)) {
    $model = (Resolve-Path -LiteralPath $SmokeTestModel).Path
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = ([System.Net.IPEndPoint] $listener.LocalEndpoint).Port
    $listener.Stop()

    $process = Start-Process -FilePath $server `
        -ArgumentList @("--model", "`"$model`"", "--host", "127.0.0.1", "--port", [string] $port, "--ctx-size", "512", "--threads", "1") `
        -WorkingDirectory $destination -WindowStyle Hidden -PassThru
    try {
        $ready = $false
        for ($attempt = 0; $attempt -lt 240; $attempt++) {
            if ($process.HasExited) {
                break
            }
            try {
                $client = [System.Net.Sockets.TcpClient]::new()
                $client.Connect("127.0.0.1", $port)
                $client.Dispose()
                $ready = $true
                break
            }
            catch {
                Start-Sleep -Milliseconds 250
            }
        }
        if (-not $ready) {
            $exitDescription = if ($process.HasExited) { "exit code $($process.ExitCode)" } else { "startup timeout" }
            throw "Minimized llama-server failed its model-loading smoke test: $exitDescription"
        }

        $loadedRuntimeNames = @((Get-Process -Id $process.Id).Modules |
            Where-Object {
                [System.IO.Path]::GetDirectoryName($_.FileName).Equals($destination, [System.StringComparison]::OrdinalIgnoreCase)
            } |
            ForEach-Object { $_.ModuleName.ToLowerInvariant() } |
            Sort-Object -Unique)
        foreach ($loadedName in $loadedRuntimeNames) {
            if (-not $expectedSet.Contains($loadedName)) {
                throw "Model-loading smoke test loaded an untrusted local module: $loadedName"
            }
        }
        foreach ($dynamicDependency in @("ggml-rpc.dll", "mtmd.dll")) {
            if ($dynamicDependency -notin $loadedRuntimeNames) {
                throw "Expected dynamic server dependency was not exercised by the model smoke test: $dynamicDependency"
            }
        }
        if (@($loadedRuntimeNames | Where-Object { $_ -like "ggml-cpu-*.dll" }).Count -ne 1) {
            throw "Model smoke test did not select exactly one portable CPU backend."
        }
    }
    finally {
        if ($null -ne $process -and -not $process.HasExited) {
            Stop-Process -Id $process.Id -Force
            $process.WaitForExit()
        }
    }
}

$totalBytes = (Get-ChildItem -LiteralPath $destination -File | Measure-Object -Property Length -Sum).Sum
Write-Host "Verified minimized llama.cpp server runtime: $($specifications.Count) files, $totalBytes bytes."
