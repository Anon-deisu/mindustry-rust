<#
.SYNOPSIS
Copies the current Rust handoff surface into the fixed `mindustry-rust` checkout.

.DESCRIPTION
Reads `tools/mindustry-rust-target.json`, verifies the target checkout points at
the expected repository, and copies the current handoff include set into that
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

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\sync-mindustry-rust-handoff.ps1

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\sync-mindustry-rust-handoff.ps1 -Stage
#>
[CmdletBinding()]
param(
    [string]$SourceRoot,
    [string]$TargetCheckout,
    [switch]$Stage
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

$files = @(
    "rust/Cargo.toml",
    "rust/Cargo.lock",
    "rust/mdt-protocol/Cargo.toml",
    "rust/mdt-typeio/Cargo.toml",
    "rust/mdt-world/Cargo.toml",
    "rust/mdt-remote/Cargo.toml",
    "rust/mdt-input/Cargo.toml",
    "rust/mdt-input/Cargo.lock",
    "rust/mdt-client-min/Cargo.toml",
    "rust/mdt-client-min/Cargo.lock",
    "rust/mdt-client-min/assets/version.properties",
    "rust/mdt-render-ui/Cargo.toml",
    "rust/mdt-render-ui/Cargo.lock",
    "tools/check-mdt-release-prereqs.ps1",
    "tools/package-mdt-client-min-online.ps1",
    "tools/package-mdt-client-min-release-set.ps1",
    "tools/verify-mdt-client-min-release-set.ps1",
    "tools/verify-rust-workspaces.ps1",
    "tools/clean-legacy-mdt-package-dirs.ps1",
    "tools/WINDOWS-RELEASE.md",
    "tools/README.md",
    "tools/MINDUSTRY-RUST-HANDOFF.md",
    "tools/mindustry-rust-target.json",
    "tools/get-mindustry-rust-target.ps1",
    "tools/mindustry-rust-repo-README.md",
    "tools/sync-mindustry-rust-handoff.ps1",
    "audit/ci-gate-plan.md",
    "tests/src/test/resources/connect-packet.hex",
    "tests/src/test/resources/control-packet-goldens.txt",
    "tests/src/test/resources/framework-message-goldens.txt",
    "tests/src/test/resources/payload-campaign-compound-goldens.txt",
    "tests/src/test/resources/snapshot-goldens.txt",
    "tests/src/test/resources/typeio-goldens.txt",
    "tests/src/test/resources/world-stream.hex",
    "fixtures/remote/remote-manifest-v1.json",
    "fixtures/world-streams/archipelago-6567-world-stream.hex",
    "rust/fixtures/remote/remote-manifest-v1.json",
    "rust/fixtures/world-streams/archipelago-6567-world-stream.hex"
)

$dirs = @(
    "rust/mdt-protocol/src",
    "rust/mdt-typeio/src",
    "rust/mdt-world/src",
    "rust/mdt-remote/src",
    "rust/mdt-input/src",
    "rust/mdt-client-min/src",
    "rust/mdt-render-ui/src"
)

$obsoletePaths = @(
    "core/assets/version.properties"
)

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

foreach ($relativePath in $files) {
    Copy-RelativeFile $relativePath
}

foreach ($relativePath in $dirs) {
    Copy-RelativeDirectory $relativePath
}

Copy-MappedFile -SourceRelativePath "tools/mindustry-rust-repo-README.md" -TargetRelativePath "README.md"

foreach ($relativePath in $obsoletePaths) {
    Remove-RelativePathIfPresent $relativePath
}

if ($Stage) {
    $stageExistingPaths = @()
    foreach ($relativePath in @($files + $dirs + @("README.md"))) {
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
    foreach ($relativePath in $obsoletePaths) {
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
    FilesCopied         = $files.Count
    DirectoriesCopied   = $dirs.Count
    Staged              = [bool]$Stage
}
