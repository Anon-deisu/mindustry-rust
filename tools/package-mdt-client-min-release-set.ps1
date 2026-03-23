<#
.SYNOPSIS
Builds the Windows `core/devtools` release set and optional release gate.

.DESCRIPTION
Stages both Windows package variants, writes the zip artifacts, and optionally
runs the release verification flow.

.PARAMETER Verify
Runs the default release gate after packaging. By default this is a clean
hard gate unless `-AllowLegacyStageWarning` is supplied.

.PARAMETER CleanLegacyStage
Removes legacy single-directory stage outputs before packaging.

.PARAMETER AllowLegacyStageWarning
Downgrades legacy stage detection from a hard failure to a warning when
`-Verify` is used.

.PARAMETER BenchWorldStreamHex
Optional world-stream sample to embed into the `devtools` package.

.PARAMETER VerifyRustWorkspaces
Runs split-workspace Rust verification as part of `-Verify`.

.PARAMETER RustWorkspaceMetadataOnly
When used with `-VerifyRustWorkspaces`, only checks workspace membership via
`cargo metadata` and skips `cargo test --no-run`.

.PARAMETER SkipPreflight
Skips release preflight (`check-mdt-release-prereqs.ps1`) when `-Verify` is
used. Default behavior keeps preflight enforcement on.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -Verify -AnimatePlayer

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -CleanLegacyStage -BenchWorldStreamHex .\build\archipelago-6567-world-stream.hex -Verify -AnimatePlayer
#>
param(
    [string]$BenchWorldStreamHex = '',
    [string]$Server = '127.0.0.1:6567',
    [int]$DurationMs = 3000,
    [int]$TickMs = 200,
    [int]$BenchFrames = 30,
    [switch]$AnimatePlayer,
    [switch]$CleanLegacyStage,
    [switch]$AllowLegacyStageWarning,
    [switch]$VerifyRustWorkspaces,
    [switch]$RustWorkspaceMetadataOnly,
    [switch]$SkipPreflight,
    [switch]$Verify
)

$ErrorActionPreference = 'Stop'
$removedRPlus2Params = @(
    '-AllowTransitionalFixturePaths',
    '-TransitionalFixtureFallbackWaiverTicket'
)
$blockedRemovedParams = @($args | Where-Object { $removedRPlus2Params -contains $_ })
if ($blockedRemovedParams.Count -gt 0) {
    throw "removed R+2 parameter(s): $($blockedRemovedParams -join ', '). Transitional fixture fallback is hard-removed; use canonical fixtures only."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$packageScript = Join-Path $PSScriptRoot 'package-mdt-client-min-online.ps1'
$verifyScript = Join-Path $PSScriptRoot 'verify-mdt-client-min-release-set.ps1'
$cleanupScript = Join-Path $PSScriptRoot 'clean-legacy-mdt-package-dirs.ps1'

if (-not (Test-Path $packageScript)) {
    throw "package script not found: $packageScript"
}

if ($Verify -and -not (Test-Path $verifyScript)) {
    throw "verify script not found: $verifyScript"
}

if ($CleanLegacyStage -and -not (Test-Path $cleanupScript)) {
    throw "cleanup script not found: $cleanupScript"
}

$coreStageDir = Join-Path $repoRoot 'build\windows\mdt-client-min-online-core'
$devtoolsStageDir = Join-Path $repoRoot 'build\windows\mdt-client-min-online-devtools'
$coreZipPath = Join-Path $repoRoot 'build\windows\mdt-client-min-online-windows.zip'
$devtoolsZipPath = Join-Path $repoRoot 'build\windows\mdt-client-min-online-devtools.zip'

if ($CleanLegacyStage) {
    & $cleanupScript
}

& $packageScript -StageDir $coreStageDir -CreateZip -ZipPath $coreZipPath

if (-not [string]::IsNullOrWhiteSpace($BenchWorldStreamHex)) {
    & $packageScript `
        -StageDir $devtoolsStageDir `
        -IncludeBenchTools `
        -CreateZip `
        -ZipPath $devtoolsZipPath `
        -BenchWorldStreamHex $BenchWorldStreamHex
} else {
    & $packageScript `
        -StageDir $devtoolsStageDir `
        -IncludeBenchTools `
        -CreateZip `
        -ZipPath $devtoolsZipPath
}

Write-Output "staged_windows_release_set: core_stage=$coreStageDir core_zip=$coreZipPath devtools_stage=$devtoolsStageDir devtools_zip=$devtoolsZipPath"

if ($Verify) {
    $verifyArgs = @{
        Server = $Server
        DurationMs = $DurationMs
        TickMs = $TickMs
        BenchFrames = $BenchFrames
        SkipPackage = $true
    }
    if (-not [string]::IsNullOrWhiteSpace($BenchWorldStreamHex)) {
        $verifyArgs['BenchWorldStreamHex'] = $BenchWorldStreamHex
        $verifyArgs['ExpectStagedSampleWorld'] = $true
    }
    if ($AnimatePlayer) {
        $verifyArgs['AnimatePlayer'] = $true
    }
    if (-not $AllowLegacyStageWarning) {
        $verifyArgs['FailOnLegacyStage'] = $true
    }
    if ($VerifyRustWorkspaces) {
        $verifyArgs['VerifyRustWorkspaces'] = $true
        if ($RustWorkspaceMetadataOnly) {
            $verifyArgs['RustWorkspaceMetadataOnly'] = $true
        }
    }
    if ($SkipPreflight) {
        $verifyArgs['SkipPreflight'] = $true
    }
    & $verifyScript @verifyArgs
}
