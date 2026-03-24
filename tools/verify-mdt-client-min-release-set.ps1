<#
.SYNOPSIS
Verifies the staged Windows release set for `mdt-client-min-online`.

.DESCRIPTION
Validates stage directories, zip artifacts, package manifests, core smoke, and
devtools bench smoke. Can optionally fail on legacy stage residues.

.PARAMETER FailOnLegacyStage
Treats legacy single-directory staging outputs as a hard failure.

.PARAMETER SkipPackage
Skips the packaging step and verifies the already-staged release set.

.PARAMETER ExpectStagedSampleWorld
Requires `devtools\sample-world-stream.hex` to be present in the staged
manifest.

.PARAMETER VerifyRustWorkspaces
Runs split-workspace Rust verification before smoke checks.

.PARAMETER RustWorkspaceMetadataOnly
When used with `-VerifyRustWorkspaces`, only checks workspace membership via
`cargo metadata` and skips `cargo test --no-run`.

.PARAMETER VerifyRemoteFreshness
When used with `-VerifyRustWorkspaces` or standalone, asks the workspace
verification script to run Gradle `verifyMdtRemoteFreshness` before smoke
checks.

.PARAMETER SkipPreflight
Skips release preflight (`check-mdt-release-prereqs.ps1`). Use only for
special-case verification workflows that intentionally bypass canonical
prerequisite enforcement.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\verify-mdt-client-min-release-set.ps1 -AnimatePlayer

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\verify-mdt-client-min-release-set.ps1 -SkipPackage -AnimatePlayer -FailOnLegacyStage
#>
param(
    [string]$Server = '127.0.0.1:6567',
    [string]$BenchWorldStreamHex = '',
    [int]$DurationMs = 3000,
    [int]$TickMs = 200,
    [int]$BenchFrames = 30,
    [switch]$AnimatePlayer,
    [switch]$ExpectStagedSampleWorld,
    [switch]$FailOnLegacyStage,
    [switch]$VerifyRustWorkspaces,
    [switch]$RustWorkspaceMetadataOnly,
    [switch]$VerifyRemoteFreshness,
    [switch]$SkipPreflight,
    [switch]$SkipPackage
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
$releaseSetScript = Join-Path $PSScriptRoot 'package-mdt-client-min-release-set.ps1'
$coreStageDir = Join-Path $repoRoot 'build\windows\mdt-client-min-online-core'
$devtoolsStageDir = Join-Path $repoRoot 'build\windows\mdt-client-min-online-devtools'
$coreZipPath = Join-Path $repoRoot 'build\windows\mdt-client-min-online-windows.zip'
$devtoolsZipPath = Join-Path $repoRoot 'build\windows\mdt-client-min-online-devtools.zip'
$cleanupScript = Join-Path $PSScriptRoot 'clean-legacy-mdt-package-dirs.ps1'
$preflightScript = Join-Path $PSScriptRoot 'check-mdt-release-prereqs.ps1'
$workspaceVerifyScript = Join-Path $PSScriptRoot 'verify-rust-workspaces.ps1'
$transitionalWorldStreamFixturePath = Join-Path $repoRoot 'rust\fixtures\world-streams\archipelago-6567-world-stream.hex'
$legacyBuildWorldStreamPath = Join-Path $repoRoot 'build\archipelago-6567-world-stream.hex'
$legacyStageDirs = @(
    (Join-Path $repoRoot 'build\windows\mdt-client-min-online'),
    (Join-Path $repoRoot '-StageDir')
)

function Assert-PathExists {
    param(
        [string]$Path,
        [string]$Label
    )

    if (-not (Test-Path $Path)) {
        throw "$Label not found: $Path"
    }
}

function Assert-ContainsAll {
    param(
        [string[]]$Actual,
        [string[]]$Expected,
        [string]$Label
    )

    foreach ($item in $Expected) {
        if ($Actual -notcontains $item) {
            throw "$Label missing item: $item"
        }
    }
}

function Select-FirstExistingPath {
    param(
        [string[]]$Candidates
    )

    foreach ($candidate in $Candidates) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    return ''
}

function Normalize-PathForComparison {
    param(
        [string]$Path
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return ''
    }

    return [System.IO.Path]::GetFullPath($Path).TrimEnd('\').ToLowerInvariant()
}

if ([string]::IsNullOrWhiteSpace($BenchWorldStreamHex)) {
    $BenchWorldStreamHex = Select-FirstExistingPath -Candidates @(
        (Join-Path $repoRoot 'fixtures\world-streams\archipelago-6567-world-stream.hex')
    )
}

if ((-not [string]::IsNullOrWhiteSpace($BenchWorldStreamHex)) -and `
    (((Normalize-PathForComparison -Path $BenchWorldStreamHex) -eq (Normalize-PathForComparison -Path $transitionalWorldStreamFixturePath)) -or
     ((Normalize-PathForComparison -Path $BenchWorldStreamHex) -eq (Normalize-PathForComparison -Path $legacyBuildWorldStreamPath)))) {
    throw "non-canonical fixture path is not allowed at R+2: $BenchWorldStreamHex; use fixtures\\world-streams\\archipelago-6567-world-stream.hex"
}

if (-not $SkipPreflight) {
    if (-not (Test-Path $preflightScript)) {
        throw "preflight script not found: $preflightScript"
    }

    $preflightOutput = @(& $preflightScript -Server $Server 2>&1)
    $preflightOutput | ForEach-Object { Write-Output $_ }
}

if (-not $SkipPackage) {
    if ([string]::IsNullOrWhiteSpace($BenchWorldStreamHex)) {
        & $releaseSetScript
    } else {
        & $releaseSetScript -BenchWorldStreamHex $BenchWorldStreamHex
    }
}

if ($VerifyRustWorkspaces -or $VerifyRemoteFreshness) {
    if (-not (Test-Path $workspaceVerifyScript)) {
        throw "workspace verify script not found: $workspaceVerifyScript"
    }

    $workspaceArgs = @{}
    if ($RustWorkspaceMetadataOnly) {
        $workspaceArgs['MetadataOnly'] = $true
    }
    if ($VerifyRemoteFreshness) {
        $workspaceArgs['VerifyRemoteFreshness'] = $true
    }
    & $workspaceVerifyScript @workspaceArgs
}

$coreManifestPath = Join-Path $coreStageDir 'PACKAGE-MANIFEST.json'
$devtoolsManifestPath = Join-Path $devtoolsStageDir 'PACKAGE-MANIFEST.json'

Assert-PathExists -Path $coreStageDir -Label 'core stage dir'
Assert-PathExists -Path $devtoolsStageDir -Label 'devtools stage dir'
Assert-PathExists -Path $coreZipPath -Label 'core zip'
Assert-PathExists -Path $devtoolsZipPath -Label 'devtools zip'
Assert-PathExists -Path $coreManifestPath -Label 'core package manifest'
Assert-PathExists -Path $devtoolsManifestPath -Label 'devtools package manifest'

$legacyStageHits = @($legacyStageDirs | Where-Object { Test-Path $_ })
if ($legacyStageHits.Count -gt 0) {
    $legacyMessage = "legacy stage dirs still present: " + ($legacyStageHits -join ', ')
    if ($FailOnLegacyStage) {
        $cleanupHint = if (Test-Path $cleanupScript) {
            "; cleanup with: powershell -ExecutionPolicy Bypass -File $cleanupScript"
        } else {
            ''
        }
        throw ($legacyMessage + $cleanupHint)
    }
    Write-Warning $legacyMessage
}

$coreManifest = Get-Content -Path $coreManifestPath -Raw | ConvertFrom-Json
$devtoolsManifest = Get-Content -Path $devtoolsManifestPath -Raw | ConvertFrom-Json

if ([bool]$coreManifest.include_bench_tools) {
    throw 'core package manifest unexpectedly enables bench tools'
}

if (-not [bool]$devtoolsManifest.include_bench_tools) {
    throw 'devtools package manifest unexpectedly disables bench tools'
}

if ($coreManifest.zip_path -ne $coreZipPath) {
    throw "core package manifest zip_path mismatch: $($coreManifest.zip_path)"
}

if ($devtoolsManifest.zip_path -ne $devtoolsZipPath) {
    throw "devtools package manifest zip_path mismatch: $($devtoolsManifest.zip_path)"
}

if ($coreManifest.package_role -ne 'core') {
    throw "core package manifest package_role mismatch: $($coreManifest.package_role)"
}

if ($devtoolsManifest.package_role -ne 'devtools') {
    throw "devtools package manifest package_role mismatch: $($devtoolsManifest.package_role)"
}

if ($coreManifest.artifact_tier -ne 'primary') {
    throw "core package manifest artifact_tier mismatch: $($coreManifest.artifact_tier)"
}

if ($devtoolsManifest.artifact_tier -ne 'secondary') {
    throw "devtools package manifest artifact_tier mismatch: $($devtoolsManifest.artifact_tier)"
}

if ($coreManifest.entrypoint -ne 'run-localhost-example.cmd') {
    throw "core package manifest entrypoint mismatch: $($coreManifest.entrypoint)"
}

if ($devtoolsManifest.entrypoint -ne 'devtools\run-bench-example.ps1') {
    throw "devtools package manifest entrypoint mismatch: $($devtoolsManifest.entrypoint)"
}

$expectedCoreFiles = @(
    'mdt-client-min-online.exe',
    'remote-manifest-v1.json',
    'README.txt',
    'run-localhost-example.ps1',
    'run-localhost-example.cmd'
)
Assert-ContainsAll -Actual $coreManifest.core_files -Expected $expectedCoreFiles -Label 'core package manifest core_files'

if ($coreManifest.devtool_files.Count -ne 0) {
    throw 'core package manifest unexpectedly contains devtool files'
}

$expectedDevtoolFiles = @(
    'devtools\mdt-render-ui-window-bench.exe',
    'devtools\mdt-render-ui-ascii.exe',
    'devtools\mdt-render-ui-window.exe',
    'devtools\mdt-render-ui-window-demo.exe',
    'devtools\run-bench-example.ps1'
)
if ($ExpectStagedSampleWorld) {
    $expectedDevtoolFiles += 'devtools\sample-world-stream.hex'
}
Assert-ContainsAll -Actual $devtoolsManifest.devtool_files -Expected $expectedDevtoolFiles -Label 'devtools package manifest devtool_files'

Push-Location $coreStageDir
try {
    $coreOutput = & cmd.exe /c "call run-localhost-example.cmd --server $Server --name rustbot-release-verify --duration-ms $DurationMs --tick-ms $TickMs --render-ascii-on-world-ready" 2>&1
    $coreExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}

if ($coreExitCode -ne 0) {
    throw "core smoke failed with exit code $coreExitCode"
}

$coreText = ($coreOutput | Out-String)
foreach ($needle in @('WorldStreamReady', 'PlayerSpawned', 'final: packets_seen=')) {
    if (-not $coreText.Contains($needle)) {
        throw "core smoke output missing marker: $needle"
    }
}
if ((-not $coreText.Contains('ascii_scene:')) -and (-not $coreText.Contains('ascii_scene_final:'))) {
    throw 'core smoke output missing ascii scene marker'
}

$benchScript = Join-Path $devtoolsStageDir 'devtools\run-bench-example.ps1'
Assert-PathExists -Path $benchScript -Label 'devtools bench script'

$benchArgs = @{
    Frames = $BenchFrames
}
if ($AnimatePlayer) {
    $benchArgs['AnimatePlayer'] = $true
}
if (-not [string]::IsNullOrWhiteSpace($BenchWorldStreamHex)) {
    $benchArgs['WorldStreamHex'] = $BenchWorldStreamHex
}

$benchOutput = & $benchScript @benchArgs 2>&1
$benchExitCode = $LASTEXITCODE
if ($benchExitCode -ne 0) {
    throw "devtools bench smoke failed with exit code $benchExitCode"
}

$benchText = ($benchOutput | Out-String)
foreach ($needle in @('bench_window:', "frames=$BenchFrames")) {
    if (-not $benchText.Contains($needle)) {
        throw "devtools bench output missing marker: $needle"
    }
}

Write-Output "verified_windows_release_set: core_stage=$coreStageDir devtools_stage=$devtoolsStageDir core_zip=$coreZipPath devtools_zip=$devtoolsZipPath server=$Server duration_ms=$DurationMs tick_ms=$TickMs bench_frames=$BenchFrames animate_player=$([bool]$AnimatePlayer) preflight_enforced=$([bool](-not $SkipPreflight)) fixture_policy=canonical_only verify_rust_workspaces=$([bool]$VerifyRustWorkspaces) verify_remote_freshness=$([bool]$VerifyRemoteFreshness) rust_workspace_metadata_only=$([bool]$RustWorkspaceMetadataOnly) legacy_stage_state=$(if($legacyStageHits.Count -gt 0){'warning'}else{'clean'})"
