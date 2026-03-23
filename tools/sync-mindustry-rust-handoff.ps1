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
Target repo checkout path. Defaults to the value from
`tools/mindustry-rust-target.json`.

.PARAMETER Stage
Runs `git add .` inside the target checkout after copying.

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
    $TargetCheckout = $config.default_checkout_path
}

if (!(Test-Path $TargetCheckout)) {
    throw "Target checkout does not exist: $TargetCheckout"
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
    "rust/mdt-render-ui/Cargo.toml",
    "rust/mdt-render-ui/Cargo.lock",
    "tools/package-mdt-client-min-online.ps1",
    "tools/package-mdt-client-min-release-set.ps1",
    "tools/verify-mdt-client-min-release-set.ps1",
    "tools/clean-legacy-mdt-package-dirs.ps1",
    "tools/WINDOWS-RELEASE.md",
    "tools/README.md",
    "tools/MINDUSTRY-RUST-HANDOFF.md",
    "tools/mindustry-rust-target.json",
    "tools/get-mindustry-rust-target.ps1",
    "tools/sync-mindustry-rust-handoff.ps1",
    "tests/src/test/resources/connect-packet.hex",
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

foreach ($relativePath in $files) {
    Copy-RelativeFile $relativePath
}

foreach ($relativePath in $dirs) {
    Copy-RelativeDirectory $relativePath
}

if ($Stage) {
    git -C $TargetCheckout add .
    if ($LASTEXITCODE -ne 0) {
        throw "git add failed in target checkout: $TargetCheckout"
    }
}

[pscustomobject]@{
    TargetRepo          = $expectedRepo
    TargetCheckout      = $TargetCheckout
    FilesCopied         = $files.Count
    DirectoriesCopied   = $dirs.Count
    Staged              = [bool]$Stage
}
