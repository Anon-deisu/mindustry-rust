<#
.SYNOPSIS
Copies the current Rust handoff surface into the fixed `mindustry-rust` checkout.

.DESCRIPTION
Reads `tools/mindustry-rust-target.json`, verifies the target checkout points at
the expected repository, and copies the current handoff include manifest into that
checkout. This prevents future upload work from rediscovering the target repo
or reassembling the sync list by hand.

.PARAMETER SourceRoot
Source monorepo root. Defaults to the parent directory of `tools/`.

.PARAMETER TargetCheckout
Target repo checkout path. Resolution order is: explicit `-TargetCheckout`,
source-repo local `git config mdt.targetcheckout`, then
`$env:MDT_TARGET_CHECKOUT`.

.PARAMETER Stage
Stages only the declared handoff paths inside the target checkout after copying.

.PARAMETER ValidateManifest
Validates the handoff manifest against the source workspace and exits without
touching a target checkout.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\sync-mindustry-rust-handoff.ps1

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\sync-mindustry-rust-handoff.ps1 -Stage

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\sync-mindustry-rust-handoff.ps1 -ValidateManifest
#>
[CmdletBinding()]
param(
    [string]$SourceRoot,
    [string]$TargetCheckout,
    [switch]$Stage,
    [switch]$ValidateManifest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$configPath = Join-Path $scriptDir "mindustry-rust-target.json"
$targetCheckoutResolutionOrder = "Target checkout resolution order: -TargetCheckout > git config --local mdt.targetcheckout (source workspace) > MDT_TARGET_CHECKOUT."

if (!(Test-Path $configPath)) {
    throw "Missing target config: $configPath"
}

$config = Get-Content -Raw -Path $configPath | ConvertFrom-Json

if ([string]::IsNullOrWhiteSpace($SourceRoot)) {
    $SourceRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
} else {
    $SourceRoot = (Resolve-Path $SourceRoot).Path
}

function Normalize-RelativePath([string]$RelativePath, [string]$FieldName) {
    if ([string]::IsNullOrWhiteSpace($RelativePath)) {
        throw "Manifest field '$FieldName' contains an empty path."
    }

    $normalized = $RelativePath.Trim() -replace "\\", "/"
    while ($normalized.StartsWith("./")) {
        $normalized = $normalized.Substring(2)
    }

    if ([string]::IsNullOrWhiteSpace($normalized)) {
        throw "Manifest field '$FieldName' contains an empty path."
    }

    if ([System.IO.Path]::IsPathRooted($normalized)) {
        throw "Manifest field '$FieldName' must use relative paths only: $RelativePath"
    }

    return $normalized
}

function Get-ManifestStringList([object]$ManifestValue, [string]$FieldName) {
    $result = @()
    foreach ($item in @($ManifestValue)) {
        if ($null -eq $item) {
            throw "Manifest field '$FieldName' contains a null entry."
        }
        $result += (Normalize-RelativePath -RelativePath ([string]$item) -FieldName $FieldName)
    }
    return $result
}

function Get-ValidatedHandoffManifest([string]$ManifestPath, [string]$ResolvedSourceRoot) {
    if (!(Test-Path $ManifestPath -PathType Leaf)) {
        throw "Missing handoff manifest: $ManifestPath"
    }

    $manifest = Get-Content -Raw -Path $ManifestPath | ConvertFrom-Json

    $files = Get-ManifestStringList -ManifestValue $manifest.files -FieldName "files"
    $directories = Get-ManifestStringList -ManifestValue $manifest.directories -FieldName "directories"
    $obsoletePaths = Get-ManifestStringList -ManifestValue $manifest.obsolete_paths -FieldName "obsolete_paths"

    $mappedFiles = @()
    foreach ($entry in @($manifest.mapped_files)) {
        if ($null -eq $entry) {
            throw "Manifest field 'mapped_files' contains a null entry."
        }

        $sourceRelativePath = Normalize-RelativePath -RelativePath ([string]$entry.source) -FieldName "mapped_files.source"
        $targetRelativePath = Normalize-RelativePath -RelativePath ([string]$entry.target) -FieldName "mapped_files.target"
        $mappedFiles += [pscustomobject]@{
            Source = $sourceRelativePath
            Target = $targetRelativePath
        }
    }

    $seenTargetPaths = @{}
    foreach ($relativePath in @($files + $directories + @($mappedFiles | ForEach-Object { $_.Target }))) {
        $normalizedTarget = $relativePath.ToLowerInvariant()
        if ($seenTargetPaths.ContainsKey($normalizedTarget)) {
            throw "Duplicate target path in handoff manifest: $relativePath"
        }
        $seenTargetPaths[$normalizedTarget] = $true
    }

    foreach ($relativePath in $files) {
        $sourcePath = Join-Path $ResolvedSourceRoot $relativePath
        if (!(Test-Path $sourcePath -PathType Leaf)) {
            throw "Manifest file path is missing from the source workspace: $relativePath"
        }
    }

    foreach ($relativePath in $directories) {
        $sourcePath = Join-Path $ResolvedSourceRoot $relativePath
        if (!(Test-Path $sourcePath -PathType Container)) {
            throw "Manifest directory path is missing from the source workspace: $relativePath"
        }
    }

    foreach ($mappedFile in $mappedFiles) {
        $sourcePath = Join-Path $ResolvedSourceRoot $mappedFile.Source
        if (!(Test-Path $sourcePath -PathType Leaf)) {
            throw "Manifest mapped source file is missing from the source workspace: $($mappedFile.Source)"
        }
    }

    return [pscustomobject]@{
        Path          = $ManifestPath
        Files         = @($files)
        Directories   = @($directories)
        MappedFiles   = @($mappedFiles)
        ObsoletePaths = @($obsoletePaths)
    }
}

$manifestRelativePath = if ($config.PSObject.Properties.Name -contains "handoff_manifest" -and -not [string]::IsNullOrWhiteSpace($config.handoff_manifest)) {
    $config.handoff_manifest
} else {
    "tools/mindustry-rust-handoff-manifest.json"
}
$manifestPath = if ([System.IO.Path]::IsPathRooted($manifestRelativePath)) {
    $manifestRelativePath
} else {
    Join-Path $SourceRoot $manifestRelativePath
}
$handoffManifest = Get-ValidatedHandoffManifest -ManifestPath $manifestPath -ResolvedSourceRoot $SourceRoot

if ($ValidateManifest) {
    [pscustomobject]@{
        SourceRoot       = $SourceRoot
        ManifestPath     = $handoffManifest.Path
        Files            = $handoffManifest.Files.Count
        Directories      = $handoffManifest.Directories.Count
        MappedFiles      = $handoffManifest.MappedFiles.Count
        ObsoletePaths    = $handoffManifest.ObsoletePaths.Count
        TargetValidation = $false
        Staged           = $false
    }
    return
}

if ([string]::IsNullOrWhiteSpace($TargetCheckout)) {
    $gitConfiguredCheckout = (& git -C $SourceRoot config --local --get mdt.targetcheckout 2>$null)
    if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($gitConfiguredCheckout)) {
        $TargetCheckout = $gitConfiguredCheckout
    } elseif (-not [string]::IsNullOrWhiteSpace($env:MDT_TARGET_CHECKOUT)) {
        $TargetCheckout = $env:MDT_TARGET_CHECKOUT
    } else {
        throw "TargetCheckout is not configured. Run this script from the source workspace (not the target repo), or pass -TargetCheckout explicitly. $targetCheckoutResolutionOrder"
    }
}

if (!(Test-Path $TargetCheckout)) {
    throw "Target checkout does not exist: $TargetCheckout"
}

$normalizedSourceRoot = [System.IO.Path]::GetFullPath($SourceRoot).TrimEnd('\')
$normalizedTargetCheckout = [System.IO.Path]::GetFullPath($TargetCheckout).TrimEnd('\')

if ($normalizedSourceRoot.ToLowerInvariant() -eq $normalizedTargetCheckout.ToLowerInvariant()) {
    throw "SourceRoot and TargetCheckout resolve to the same directory: $SourceRoot. This usually means the script is running in the target repo. Run it from the source workspace or pass -SourceRoot explicitly. $targetCheckoutResolutionOrder"
}

$originUrl = git -C $TargetCheckout remote get-url origin 2>$null
if ($LASTEXITCODE -ne 0) {
    throw "Failed to read target checkout remote: $TargetCheckout"
}

$expectedRepo = $config.target_repo
$normalizedOrigin = $originUrl.TrimEnd("/")
$normalizedExpected = $expectedRepo.TrimEnd("/")

if ($normalizedOrigin -ne $normalizedExpected -and $normalizedOrigin -ne "$normalizedExpected.git") {
    throw "Target checkout origin mismatch. expected=$expectedRepo actual=$originUrl"
}

function Copy-RelativeFile([string]$RelativePath) {
    $sourcePath = Join-Path $SourceRoot $RelativePath
    $targetPath = Join-Path $TargetCheckout $RelativePath
    if (!(Test-Path $sourcePath)) {
        throw "Missing source file: $sourcePath"
    }
    $parent = Split-Path -Parent $targetPath
    if (!(Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    Copy-Item -Path $sourcePath -Destination $targetPath -Force
}

function Copy-RelativeDirectory([string]$RelativePath) {
    $sourcePath = Join-Path $SourceRoot $RelativePath
    $targetPath = Join-Path $TargetCheckout $RelativePath
    if (!(Test-Path $sourcePath)) {
        throw "Missing source directory: $sourcePath"
    }
    $parent = Split-Path -Parent $targetPath
    if (!(Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    if (!(Test-Path $targetPath)) {
        New-Item -ItemType Directory -Path $targetPath -Force | Out-Null
    }
    Copy-Item -Path (Join-Path $sourcePath "*") -Destination $targetPath -Recurse -Force
}

function Copy-MappedFile([string]$SourceRelativePath, [string]$TargetRelativePath) {
    $sourcePath = Join-Path $SourceRoot $SourceRelativePath
    $targetPath = Join-Path $TargetCheckout $TargetRelativePath
    if (!(Test-Path $sourcePath)) {
        throw "Missing source file: $sourcePath"
    }
    $parent = Split-Path -Parent $targetPath
    if (!(Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    Copy-Item -Path $sourcePath -Destination $targetPath -Force
}

function Remove-RelativePathIfPresent([string]$RelativePath) {
    $targetPath = Join-Path $TargetCheckout $RelativePath
    if (Test-Path $targetPath) {
        Remove-Item -Path $targetPath -Force
    }
}

foreach ($relativePath in $handoffManifest.Files) {
    Copy-RelativeFile $relativePath
}

foreach ($relativePath in $handoffManifest.Directories) {
    Copy-RelativeDirectory $relativePath
}

foreach ($mappedFile in $handoffManifest.MappedFiles) {
    Copy-MappedFile -SourceRelativePath $mappedFile.Source -TargetRelativePath $mappedFile.Target
}

foreach ($relativePath in $handoffManifest.ObsoletePaths) {
    Remove-RelativePathIfPresent $relativePath
}

if ($Stage) {
    $stageExistingPaths = @()
    foreach ($relativePath in @(
            $handoffManifest.Files +
            $handoffManifest.Directories +
            @($handoffManifest.MappedFiles | ForEach-Object { $_.Target })
        )) {
        if (Test-Path (Join-Path $TargetCheckout $relativePath)) {
            $stageExistingPaths += $relativePath
        }
    }

    if ($stageExistingPaths.Count -gt 0) {
        git -C $TargetCheckout add -- @stageExistingPaths
        if ($LASTEXITCODE -ne 0) {
            throw "git add failed for copied handoff paths in target checkout: $TargetCheckout"
        }
    }

    $stageDeletedPaths = @()
    foreach ($relativePath in $handoffManifest.ObsoletePaths) {
        $trackedPath = (& git -C $TargetCheckout ls-files -- $relativePath 2>$null)
        if (-not [string]::IsNullOrWhiteSpace(($trackedPath | Out-String))) {
            $stageDeletedPaths += $relativePath
        }
    }

    if ($stageDeletedPaths.Count -gt 0) {
        git -C $TargetCheckout add -u -- @stageDeletedPaths
        if ($LASTEXITCODE -ne 0) {
            throw "git add failed for removed obsolete paths in target checkout: $TargetCheckout"
        }
    }
}

[pscustomobject]@{
    TargetRepo          = $expectedRepo
    TargetCheckout      = $TargetCheckout
    ManifestPath        = $handoffManifest.Path
    FilesCopied         = $handoffManifest.Files.Count
    DirectoriesCopied   = $handoffManifest.Directories.Count
    MappedFilesCopied   = $handoffManifest.MappedFiles.Count
    Staged              = [bool]$Stage
}
