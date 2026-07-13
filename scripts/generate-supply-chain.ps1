[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $OutputDirectory,

    [string] $Target = "x86_64-pc-windows-msvc",

    [string] $GitCommit
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrWhiteSpace($GitCommit)) {
    $gitCommand = Get-Command git -ErrorAction SilentlyContinue
    if ($null -eq $gitCommand) {
        throw "Supply-chain generation requires git to record the exact application commit."
    }
    $gitProbe = @(& $gitCommand.Source -C $root rev-parse --verify HEAD 2>&1)
    $gitProbeExit = $LASTEXITCODE
    $GitCommit = ($gitProbe -join "`n").Trim()
    if ($gitProbeExit -ne 0) {
        throw "Could not resolve the Git commit for supply-chain metadata."
    }
}
$GitCommit = $GitCommit.Trim().ToLowerInvariant()
if ($GitCommit -notmatch '^[0-9a-f]{40}$') {
    throw "Supply-chain Git commit must be a full 40-character hexadecimal object ID."
}

$output = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    [System.IO.Path]::GetFullPath($OutputDirectory)
}
else {
    [System.IO.Path]::GetFullPath((Join-Path $root $OutputDirectory))
}
$rootPrefix = $root.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $output.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Supply-chain output must stay inside the repository: $output"
}

if (-not $env:CARGO_HOME) {
    $env:CARGO_HOME = Join-Path $root ".cache\cargo"
}

$metadataText = & cargo metadata --manifest-path (Join-Path $root "Cargo.toml") --locked --offline --format-version 1 --filter-platform $Target | Out-String
if ($LASTEXITCODE -ne 0) {
    throw "cargo metadata failed with exit code $LASTEXITCODE. Build the locked Windows target first so every crate source is available offline."
}
$metadata = $metadataText | ConvertFrom-Json
if ($null -eq $metadata.resolve -or [string]::IsNullOrWhiteSpace($metadata.resolve.root)) {
    throw "cargo metadata did not return a resolved root package."
}

$packagesById = @{}
foreach ($package in $metadata.packages) {
    $packagesById[$package.id] = $package
}
$nodesById = @{}
foreach ($node in $metadata.resolve.nodes) {
    $nodesById[$node.id] = $node
}
if (-not $packagesById.ContainsKey($metadata.resolve.root)) {
    throw "The resolved root package is missing from cargo metadata."
}

$reachable = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
$pending = [System.Collections.Generic.Queue[string]]::new()
$pending.Enqueue($metadata.resolve.root)
while ($pending.Count -gt 0) {
    $packageId = $pending.Dequeue()
    if (-not $reachable.Add($packageId)) {
        continue
    }
    if (-not $nodesById.ContainsKey($packageId)) {
        throw "Resolved package has no dependency node: $packageId"
    }
    foreach ($dependency in $nodesById[$packageId].deps) {
        $releaseKinds = @($dependency.dep_kinds | Where-Object { [string] $_.kind -ne "dev" })
        if ($releaseKinds.Count -gt 0) {
            $pending.Enqueue($dependency.pkg)
        }
    }
}

$rootPackage = $packagesById[$metadata.resolve.root]
$dependencyPackages = @($reachable |
    Where-Object { $_ -ne $metadata.resolve.root } |
    ForEach-Object { $packagesById[$_] } |
    Sort-Object name, version, id)
if ($dependencyPackages.Count -eq 0) {
    throw "No Rust dependencies were found for the Windows release target."
}

if (Test-Path -LiteralPath $output) {
    $resolvedOutput = (Resolve-Path -LiteralPath $output).Path
    if (-not $resolvedOutput.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to replace supply-chain output outside the repository: $resolvedOutput"
    }
    Remove-Item -LiteralPath $resolvedOutput -Recurse -Force
}

$rustLicenseRoot = Join-Path $output "licenses\rust"
New-Item -ItemType Directory -Force -Path $rustLicenseRoot | Out-Null

function Get-SafePackageDirectory {
    param(
        [Parameter(Mandatory)] $Package,
        [Parameter(Mandatory)] [AllowEmptyCollection()] [System.Collections.Generic.HashSet[string]] $UsedNames
    )

    $baseName = "$($Package.name)-$($Package.version)" -replace '[^A-Za-z0-9._-]', '_'
    if ($UsedNames.Add($baseName)) {
        return $baseName
    }

    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $idBytes = [System.Text.Encoding]::UTF8.GetBytes([string] $Package.id)
        $suffix = ([System.BitConverter]::ToString($sha256.ComputeHash($idBytes))).Replace("-", "").Substring(0, 8).ToLowerInvariant()
    }
    finally {
        $sha256.Dispose()
    }
    $uniqueName = "$baseName-$suffix"
    if (-not $UsedNames.Add($uniqueName)) {
        throw "Could not create a unique license directory for $($Package.id)."
    }
    return $uniqueName
}

function Get-PackageLicenseFiles {
    param([Parameter(Mandatory)] $Package)

    $crateDirectory = Split-Path -Parent $Package.manifest_path
    if (-not (Test-Path -LiteralPath $crateDirectory -PathType Container)) {
        throw "Crate source directory is missing for $($Package.name) $($Package.version): $crateDirectory"
    }

    $candidates = @()
    if ($null -ne $Package.license_file -and -not [string]::IsNullOrWhiteSpace([string] $Package.license_file)) {
        $licenseFile = [string] $Package.license_file
        if (-not [System.IO.Path]::IsPathRooted($licenseFile)) {
            $licenseFile = Join-Path $crateDirectory $licenseFile
        }
        if (-not (Test-Path -LiteralPath $licenseFile -PathType Leaf)) {
            throw "Declared license_file is missing for $($Package.name) $($Package.version): $licenseFile"
        }
        $candidates += Get-Item -LiteralPath $licenseFile
    }

    $candidates += @(Get-ChildItem -LiteralPath $crateDirectory -File |
        Where-Object { $_.Name -match '^(?i)(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)([._-].*)?$' })

    $unique = @{}
    foreach ($candidate in $candidates) {
        $unique[$candidate.FullName.ToLowerInvariant()] = $candidate
    }
    return @($unique.Values | Sort-Object Name, FullName)
}

function ConvertTo-PurlPart {
    param([Parameter(Mandatory)] [string] $Value)
    return [System.Uri]::EscapeDataString($Value).Replace("%2F", "/")
}

$usedDirectories = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
$inventoryDependencies = @()
$sbomComponents = @()
$bomRefById = @{}
$usedBomRefs = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)

foreach ($package in @($rootPackage) + $dependencyPackages) {
    $bomRef = "pkg:cargo/$(ConvertTo-PurlPart $package.name)@$($package.version)"
    if (-not $usedBomRefs.Add($bomRef)) {
        throw "Cargo metadata produced a duplicate CycloneDX package identity: $bomRef"
    }
    $bomRefById[$package.id] = $bomRef
}

foreach ($package in $dependencyPackages) {
    if ([string]::IsNullOrWhiteSpace([string] $package.license)) {
        throw "Rust dependency has no SPDX license expression: $($package.name) $($package.version)"
    }

    $licenseFiles = @(Get-PackageLicenseFiles -Package $package)
    if ($licenseFiles.Count -eq 0) {
        throw "Rust dependency has no packaged license text: $($package.name) $($package.version)"
    }

    $directoryName = Get-SafePackageDirectory -Package $package -UsedNames $usedDirectories
    $packageLicenseDirectory = Join-Path $rustLicenseRoot $directoryName
    New-Item -ItemType Directory -Force -Path $packageLicenseDirectory | Out-Null
    $relativeLicensePaths = @()
    foreach ($licenseFile in $licenseFiles) {
        $destination = Join-Path $packageLicenseDirectory $licenseFile.Name
        Copy-Item -LiteralPath $licenseFile.FullName -Destination $destination -Force
        $copiedLicense = Get-Item -LiteralPath $destination
        if ($copiedLicense.Length -eq 0) {
            throw "Copied license file is empty: $destination"
        }
        if ($copiedLicense.LastWriteTimeUtc -lt [datetime] "1980-01-01T00:00:00Z") {
            $copiedLicense.LastWriteTimeUtc = [datetime] "1980-01-01T00:00:00Z"
        }
        $relativeLicensePaths += [ordered]@{
            path   = "licenses/rust/$directoryName/$($licenseFile.Name)"
            bytes  = $copiedLicense.Length
            sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash.ToLowerInvariant()
        }
    }

    $inventoryDependencies += [ordered]@{
        name          = $package.name
        version       = $package.version
        package_id    = $package.id
        license       = $package.license
        license_files = @($relativeLicensePaths | Sort-Object path)
        source        = $package.source
        repository    = $package.repository
    }

    $component = [ordered]@{
        type     = "library"
        'bom-ref' = $bomRefById[$package.id]
        name     = $package.name
        version  = $package.version
        scope    = "required"
        licenses = @([ordered]@{ expression = $package.license })
        purl     = $bomRefById[$package.id]
    }
    if (-not [string]::IsNullOrWhiteSpace([string] $package.repository)) {
        $component.externalReferences = @([ordered]@{
            type = "vcs"
            url  = $package.repository
        })
    }
    $sbomComponents += $component
}

$componentManifest = Get-Content -LiteralPath (Join-Path $root "packaging\component-manifest.json") -Raw | ConvertFrom-Json
$trustedManifest = Get-Content -LiteralPath (Join-Path $root "packaging\trusted-artifacts.json") -Raw | ConvertFrom-Json
$modelSpecification = $trustedManifest.models |
    Where-Object { $_.id -eq "hy-mt2-1.8b-q4" } |
    Select-Object -First 1
$llamaLicense = $trustedManifest.licenses | Where-Object { $_.id -eq "llama.cpp" } | Select-Object -First 1
$modelLicense = $trustedManifest.licenses | Where-Object { $_.id -eq "hy-mt2-1.8b" } | Select-Object -First 1
if ($null -eq $modelSpecification -or $null -eq $llamaLicense -or $null -eq $modelLicense -or
    @($trustedManifest.licenses).Count -ne 2) {
    throw "The pinned model or upstream license metadata is missing from the trusted artifact manifest."
}
if ($componentManifest.runtime.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or
    $modelSpecification.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or
    [string] $componentManifest.application.version -cne [string] $rootPackage.version -or
    $componentManifest.runtime.packaged_entrypoint -ne "llama-server.exe" -or
    [int] $componentManifest.runtime.packaged_file_count -ne @($trustedManifest.runtime.files).Count -or
    $componentManifest.lite_model.sha256 -ne $modelSpecification.sha256 -or
    $componentManifest.lite_model.revision -ne $modelSpecification.revision -or
    $componentManifest.lite_model.license -ne $modelSpecification.license -or
    $componentManifest.runtime.license.sha256 -ne $llamaLicense.sha256 -or
    $componentManifest.runtime.license.source -ne $llamaLicense.source -or
    $componentManifest.runtime.license.spdx -ne $llamaLicense.license -or
    $componentManifest.lite_model.license_sha256 -ne $modelLicense.sha256 -or
    $componentManifest.lite_model.license_source -ne $modelLicense.source -or
    $componentManifest.lite_model.license -ne $modelLicense.license) {
    throw "Runtime or model supply-chain metadata does not match the trusted manifests."
}

$runtimeBomRef = "pkg:github/ggml-org/llama.cpp@$($componentManifest.runtime.tag)"
$modelBomRef = "pkg:generic/tencent-hy-mt2-1.8b@$($modelSpecification.revision)?filename=$([System.Uri]::EscapeDataString($modelSpecification.file))"
$runtimeDownloadUrl = "https://github.com/ggml-org/llama.cpp/releases/download/$($componentManifest.runtime.tag)/$($componentManifest.runtime.asset)"

$sbomComponents += [ordered]@{
    type               = "library"
    'bom-ref'          = $runtimeBomRef
    name               = "llama.cpp"
    version            = $componentManifest.runtime.tag
    scope              = "required"
    hashes             = @([ordered]@{
        alg     = "SHA-256"
        content = ([string] $componentManifest.runtime.sha256).ToLowerInvariant()
    })
    licenses           = @([ordered]@{ expression = $componentManifest.runtime.license.spdx })
    purl               = $runtimeBomRef
    externalReferences = @(
        [ordered]@{ type = "distribution"; url = $runtimeDownloadUrl },
        [ordered]@{ type = "website"; url = $componentManifest.runtime.source }
    )
    properties         = @(
        [ordered]@{ name = "private-translator:upstream-archive"; value = $componentManifest.runtime.asset },
        [ordered]@{ name = "private-translator:commit"; value = $componentManifest.runtime.commit },
        [ordered]@{ name = "private-translator:packaged-file-count"; value = [string] $componentManifest.runtime.packaged_file_count },
        [ordered]@{ name = "private-translator:bundled"; value = "true" }
    )
}

$sbomComponents += [ordered]@{
    type               = "machine-learning-model"
    'bom-ref'          = $modelBomRef
    name               = $componentManifest.lite_model.name
    version            = $modelSpecification.revision
    scope              = "optional"
    hashes             = @([ordered]@{
        alg     = "SHA-256"
        content = ([string] $modelSpecification.sha256).ToLowerInvariant()
    })
    licenses           = @([ordered]@{ expression = $modelSpecification.license })
    purl               = $modelBomRef
    externalReferences = @(
        [ordered]@{ type = "distribution"; url = $modelSpecification.download_url },
        [ordered]@{ type = "website"; url = $modelSpecification.source }
    )
    properties         = @(
        [ordered]@{ name = "private-translator:bytes"; value = [string] $modelSpecification.bytes },
        [ordered]@{ name = "private-translator:revision"; value = $modelSpecification.revision },
        [ordered]@{ name = "private-translator:bundled"; value = "false" },
        [ordered]@{ name = "private-translator:installation"; value = "explicit setup download or local import" }
    )
}

$inventory = [ordered]@{
    schema_version = 1
    generated_by   = "scripts/generate-supply-chain.ps1"
    target         = $Target
    application    = [ordered]@{
        name       = $rootPackage.name
        version    = $rootPackage.version
        license    = $rootPackage.license
        git_commit = $GitCommit
    }
    dependencies   = $inventoryDependencies
}

$sbomDependencies = @()
foreach ($packageId in @($reachable | Sort-Object { $packagesById[$_].name }, { $packagesById[$_].version }, { $_ })) {
    $node = $nodesById[$packageId]
    $dependsOn = @($node.deps |
        Where-Object { @($_.dep_kinds | Where-Object { [string] $_.kind -ne "dev" }).Count -gt 0 } |
        ForEach-Object { $_.pkg } |
        Where-Object { $reachable.Contains($_) } |
        ForEach-Object { $bomRefById[$_] } |
        Sort-Object -Unique)
    $sbomDependencies += [ordered]@{
        ref       = $bomRefById[$packageId]
        dependsOn = $dependsOn
    }
}
$rootDependency = $sbomDependencies | Where-Object { $_["ref"] -eq $bomRefById[$rootPackage.id] } | Select-Object -First 1
if ($null -eq $rootDependency) {
    throw "CycloneDX dependency graph has no application root."
}
$rootDependency["dependsOn"] = @($rootDependency["dependsOn"] + $runtimeBomRef + $modelBomRef | Sort-Object -Unique)
$sbomDependencies += [ordered]@{ ref = $runtimeBomRef; dependsOn = @() }
$sbomDependencies += [ordered]@{ ref = $modelBomRef; dependsOn = @() }

$rootComponent = [ordered]@{
    type      = "application"
    'bom-ref' = $bomRefById[$rootPackage.id]
    name      = $rootPackage.name
    version   = $rootPackage.version
    licenses  = @([ordered]@{ expression = $rootPackage.license })
    purl      = $bomRefById[$rootPackage.id]
    properties = @([ordered]@{
        name  = "private-translator:git-commit"
        value = $GitCommit
    })
}
$sbom = [ordered]@{
    '$schema'    = "http://cyclonedx.org/schema/bom-1.5.schema.json"
    bomFormat    = "CycloneDX"
    specVersion  = "1.5"
    version      = 1
    metadata     = [ordered]@{
        component  = $rootComponent
        properties = @(
            [ordered]@{
                name  = "private-translator:release-target"
                value = $Target
            },
            [ordered]@{
                name  = "private-translator:git-commit"
                value = $GitCommit
            }
        )
    }
    components   = $sbomComponents
    dependencies = $sbomDependencies
}

$inventoryPath = Join-Path $output "rust-dependency-licenses.json"
$sbomPath = Join-Path $output "private-translator.cdx.json"
$inventory | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $inventoryPath -Encoding utf8
$sbom | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $sbomPath -Encoding utf8

# Parse the emitted files again so truncation or serialization regressions fail the release.
$inventoryCheck = Get-Content -LiteralPath $inventoryPath -Raw | ConvertFrom-Json
$sbomCheck = Get-Content -LiteralPath $sbomPath -Raw | ConvertFrom-Json
if (@($inventoryCheck.dependencies).Count -ne $dependencyPackages.Count) {
    throw "Rust license inventory count changed during serialization."
}
if ([string] $inventoryCheck.application.git_commit -cne $GitCommit) {
    throw "Rust license inventory lost the exact application Git commit during serialization."
}
$sbomCommitProperties = @($sbomCheck.metadata.properties |
    Where-Object { $_.name -eq "private-translator:git-commit" -and $_.value -ceq $GitCommit })
if ($sbomCommitProperties.Count -ne 1) {
    throw "CycloneDX metadata does not contain the exact application Git commit."
}
if ($sbomCheck.bomFormat -ne "CycloneDX" -or $sbomCheck.specVersion -ne "1.5" -or
    @($sbomCheck.components).Count -ne ($dependencyPackages.Count + 2) -or
    @($sbomCheck.components | Where-Object { $_.'bom-ref' -in @($runtimeBomRef, $modelBomRef) }).Count -ne 2) {
    throw "CycloneDX SBOM validation failed."
}

Get-Item -LiteralPath $inventoryPath, $sbomPath | Select-Object FullName, Length
Write-Host "Collected $($dependencyPackages.Count) locked Windows Rust dependencies and their upstream license files."
