<#
.SYNOPSIS
Stages a single Windows package for `mdt-client-min-online`.

.DESCRIPTION
Builds the packaged client, writes the selected stage directory, and optionally
includes bench tooling and a zip artifact.

.PARAMETER StageDir
Overrides the output stage directory. Defaults to the `core` or `devtools`
stage path based on `-IncludeBenchTools`.

.PARAMETER IncludeBenchTools
Stages the `devtools` variant with bench-only artifacts.

.PARAMETER BenchWorldStreamHex
Optional world-stream sample to stage into `devtools\sample-world-stream.hex`.

.PARAMETER CreateZip
Also writes a zip artifact for the staged package.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-online.ps1

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-online.ps1 -IncludeBenchTools -BenchWorldStreamHex .\fixtures\world-streams\archipelago-6567-world-stream.hex
#>
param(
    [string]$StageDir = '',
    [string]$ManifestPath = '',
    [switch]$IncludeBenchTools,
    [string]$BenchWorldStreamHex = '',
    [switch]$CreateZip,
    [string]$ZipPath = ''
)

$ErrorActionPreference = 'Stop'

function Remove-StagePath {
    param(
        [string]$Path,
        [string]$Label
    )

    try {
        Remove-Item -Path $Path -Recurse -Force
    } catch [System.IO.IOException] {
        throw "$Label is locked: $Path. Close any running packaged client/bench process and retry."
    }
}

function Copy-StageFile {
    param(
        [string]$Source,
        [string]$Destination,
        [string]$Label
    )

    try {
        Copy-Item $Source $Destination -Force
    } catch [System.IO.IOException] {
        throw "$Label is locked or in use: $Destination. Close any running packaged client/bench process and retry."
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

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$crateManifest = Join-Path $repoRoot 'rust\mdt-client-min\Cargo.toml'
$renderUiManifest = Join-Path $repoRoot 'rust\mdt-render-ui\Cargo.toml'
$renderUiDevtoolBins = @(
    'mdt-render-ui-window-bench',
    'mdt-render-ui-ascii',
    'mdt-render-ui-window',
    'mdt-render-ui-window-demo'
)
$transitionalRemoteManifestPath = Join-Path $repoRoot 'rust\fixtures\remote\remote-manifest-v1.json'
$transitionalWorldStreamFixturePath = Join-Path $repoRoot 'rust\fixtures\world-streams\archipelago-6567-world-stream.hex'
$legacyBuildRemoteManifestPath = Join-Path $repoRoot 'build\mdt-remote\remote-manifest-v1.json'
$legacyBuildWorldStreamPath = Join-Path $repoRoot 'build\archipelago-6567-world-stream.hex'
$defaultManifestCandidates = @(
    (Join-Path $repoRoot 'fixtures\remote\remote-manifest-v1.json')
)

if ([string]::IsNullOrWhiteSpace($StageDir)) {
    $StageDir = if ($IncludeBenchTools) {
        Join-Path $repoRoot 'build\windows\mdt-client-min-online-devtools'
    } else {
        Join-Path $repoRoot 'build\windows\mdt-client-min-online-core'
    }
}

if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    $ManifestPath = Select-FirstExistingPath -Candidates $defaultManifestCandidates
}

if (((Normalize-PathForComparison -Path $ManifestPath) -eq (Normalize-PathForComparison -Path $transitionalRemoteManifestPath)) -or
    ((Normalize-PathForComparison -Path $ManifestPath) -eq (Normalize-PathForComparison -Path $legacyBuildRemoteManifestPath))) {
    throw "non-canonical manifest path is not allowed at R+2: $ManifestPath; use fixtures\\remote\\remote-manifest-v1.json"
}

if ((-not [string]::IsNullOrWhiteSpace($BenchWorldStreamHex)) -and `
    (((Normalize-PathForComparison -Path $BenchWorldStreamHex) -eq (Normalize-PathForComparison -Path $transitionalWorldStreamFixturePath)) -or
     ((Normalize-PathForComparison -Path $BenchWorldStreamHex) -eq (Normalize-PathForComparison -Path $legacyBuildWorldStreamPath)))) {
    throw "non-canonical fixture path is not allowed at R+2: $BenchWorldStreamHex; use fixtures\\world-streams\\archipelago-6567-world-stream.hex"
}

if (-not (Test-Path $crateManifest)) {
    throw "crate manifest not found: $crateManifest"
}

if (-not (Test-Path $ManifestPath)) {
    throw "remote manifest not found. Checked: $($defaultManifestCandidates -join ', ')"
}

if ($CreateZip -and [string]::IsNullOrWhiteSpace($ZipPath)) {
    $zipName = if ($IncludeBenchTools) {
        'mdt-client-min-online-devtools.zip'
    } else {
        'mdt-client-min-online-windows.zip'
    }
    $ZipPath = Join-Path $repoRoot "build\windows\$zipName"
}

Push-Location $repoRoot
try {
    cargo build --release --manifest-path $crateManifest --bin mdt-client-min-online
    if ($IncludeBenchTools) {
        foreach ($renderUiBin in $renderUiDevtoolBins) {
            cargo build --release --manifest-path $renderUiManifest --bin $renderUiBin
        }
    }
} finally {
    Pop-Location
}

$exeSource = Join-Path $repoRoot 'rust\mdt-client-min\target\release\mdt-client-min-online.exe'
if (-not (Test-Path $exeSource)) {
    throw "built executable not found: $exeSource"
}

$renderUiExeSources = @{}
foreach ($renderUiBin in $renderUiDevtoolBins) {
    $renderUiExeSources[$renderUiBin] = Join-Path $repoRoot "rust\mdt-render-ui\target\release\$renderUiBin.exe"
}
if ($IncludeBenchTools) {
    foreach ($renderUiBin in $renderUiDevtoolBins) {
        $renderUiExeSource = $renderUiExeSources[$renderUiBin]
        if (-not (Test-Path $renderUiExeSource)) {
            throw "devtools executable not found: $renderUiExeSource"
        }
    }
}

if (Test-Path $StageDir) {
    Remove-StagePath -Path $StageDir -Label 'stage dir'
}

New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
$exeTarget = Join-Path $StageDir 'mdt-client-min-online.exe'
$manifestTarget = Join-Path $StageDir 'remote-manifest-v1.json'
$readmeTarget = Join-Path $StageDir 'README.txt'
$runExampleTarget = Join-Path $StageDir 'run-localhost-example.ps1'
$runExampleCmdTarget = Join-Path $StageDir 'run-localhost-example.cmd'
$packageManifestTarget = Join-Path $StageDir 'PACKAGE-MANIFEST.json'
$devtoolsDir = Join-Path $StageDir 'devtools'

if (Test-Path $devtoolsDir) {
    Remove-StagePath -Path $devtoolsDir -Label 'devtools stage dir'
}
if (Test-Path $packageManifestTarget) {
    Remove-StagePath -Path $packageManifestTarget -Label 'package manifest'
}

Copy-StageFile -Source $exeSource -Destination $exeTarget -Label 'packaged client exe'
Copy-StageFile -Source $ManifestPath -Destination $manifestTarget -Label 'remote manifest'

$readmeBenchSection = if ($IncludeBenchTools) {
@"

Package role:
- secondary devtools / bench artifact
- primary bench entrypoint: .\devtools\run-bench-example.ps1

Optional devtools included in .\devtools:
- mdt-render-ui-window-bench.exe
- mdt-render-ui-ascii.exe
- mdt-render-ui-window.exe
- mdt-render-ui-window-demo.exe
- run-bench-example.ps1
"@
} else {
@"

Package role:
- primary Windows release artifact
- default entrypoint: .\run-localhost-example.cmd

Optional devtools are excluded from the default first-release package.
Re-run package-mdt-client-min-online.ps1 with -IncludeBenchTools to stage bench-only tools under .\devtools.
"@
}

@"
mdt-client-min-online Windows package

Files:
- mdt-client-min-online.exe
- remote-manifest-v1.json
- run-localhost-example.ps1
- run-localhost-example.cmd

Example:
.\mdt-client-min-online.exe --manifest .\remote-manifest-v1.json --server 127.0.0.1:6567 --name rustbot --locale en_US --duration-ms 5000 --tick-ms 200

Release scope:
- minimal compatibility client
- not a full replacement for the Java desktop client

Smoke prerequisite:
- `run-localhost-example.*` expects a reachable Mindustry server at the selected `--server` address
$readmeBenchSection
"@ | Set-Content -Path $readmeTarget -Encoding ASCII

@"
param(
    [string]`$Server = '127.0.0.1:6567',
    [string]`$Name = 'rustbot',
    [int]`$DurationMs = 5000,
    [int]`$TickMs = 200,
    [switch]`$RenderAscii,
    [Parameter(ValueFromRemainingArguments = `$true)]
    [string[]]`$ExtraArgs
)

`$ErrorActionPreference = 'Stop'
`$exe = Join-Path `$PSScriptRoot 'mdt-client-min-online.exe'
`$manifest = Join-Path `$PSScriptRoot 'remote-manifest-v1.json'
`$args = @(
    '--manifest', `$manifest,
    '--server', `$Server,
    '--name', `$Name,
    '--locale', 'en_US',
    '--duration-ms', `$DurationMs,
    '--tick-ms', `$TickMs,
    '--max-recv-packets', '64'
)
if (`$RenderAscii) {
    `$args += '--render-ascii-on-world-ready'
}
if (`$ExtraArgs) {
    `$args += `$ExtraArgs
}
& `$exe @args
"@ | Set-Content -Path $runExampleTarget -Encoding ASCII

@"
@echo off
setlocal
set SERVER=127.0.0.1:6567
set NAME=rustbot
set DURATION_MS=5000
set TICK_MS=200

"%~dp0mdt-client-min-online.exe" --manifest "%~dp0remote-manifest-v1.json" --server %SERVER% --name %NAME% --locale en_US --duration-ms %DURATION_MS% --tick-ms %TICK_MS% --max-recv-packets 64 %*
"@ | Set-Content -Path $runExampleCmdTarget -Encoding ASCII

$devtoolFiles = @()
if ($IncludeBenchTools) {
    New-Item -ItemType Directory -Force -Path $devtoolsDir | Out-Null
    $runBenchTarget = Join-Path $devtoolsDir 'run-bench-example.ps1'
    foreach ($renderUiBin in $renderUiDevtoolBins) {
        $renderUiExeName = "$renderUiBin.exe"
        $renderUiExeTarget = Join-Path $devtoolsDir $renderUiExeName
        Copy-StageFile -Source $renderUiExeSources[$renderUiBin] -Destination $renderUiExeTarget -Label "packaged devtools exe ($renderUiBin)"
        $devtoolFiles += "devtools\$renderUiExeName"
    }

    $sampleWorldTarget = ''
    if (-not [string]::IsNullOrWhiteSpace($BenchWorldStreamHex)) {
        if (-not (Test-Path $BenchWorldStreamHex)) {
            throw "bench world-stream hex not found: $BenchWorldStreamHex"
        }
        $sampleWorldTarget = Join-Path $devtoolsDir 'sample-world-stream.hex'
        Copy-StageFile -Source $BenchWorldStreamHex -Destination $sampleWorldTarget -Label 'staged sample world stream'
        $devtoolFiles += 'devtools\sample-world-stream.hex'
    }

@"
param(
    [string]`$WorldStreamHex = '',
    [int]`$Frames = 300,
    [string]`$MaxViewTiles = '64:32',
    [switch]`$AnimatePlayer
)

`$ErrorActionPreference = 'Stop'
`$exe = Join-Path `$PSScriptRoot 'mdt-render-ui-window-bench.exe'
if ([string]::IsNullOrWhiteSpace(`$WorldStreamHex)) {
    `$candidate = Join-Path `$PSScriptRoot 'sample-world-stream.hex'
    if (Test-Path `$candidate) {
        `$WorldStreamHex = `$candidate
    } else {
        throw 'provide -WorldStreamHex or stage a sample-world-stream.hex'
    }
}

`$args = @(
    '--world-stream-hex', `$WorldStreamHex,
    '--frames', `$Frames,
    '--max-view-tiles', `$MaxViewTiles
)
if (`$AnimatePlayer) {
    `$args += '--animate-player'
}
& `$exe @args
"@ | Set-Content -Path $runBenchTarget -Encoding ASCII

    $devtoolFiles += 'devtools\run-bench-example.ps1'
}

$packageManifest = [ordered]@{
    package_name = 'mdt-client-min-online'
    package_role = if ($IncludeBenchTools) { 'devtools' } else { 'core' }
    artifact_tier = if ($IncludeBenchTools) { 'secondary' } else { 'primary' }
    stage_dir = $StageDir
    include_bench_tools = [bool]$IncludeBenchTools
    zip_path = if ($CreateZip) { $ZipPath } else { '' }
    entrypoint = if ($IncludeBenchTools) { 'devtools\run-bench-example.ps1' } else { 'run-localhost-example.cmd' }
    core_files = @(
        'mdt-client-min-online.exe',
        'remote-manifest-v1.json',
        'README.txt',
        'run-localhost-example.ps1',
        'run-localhost-example.cmd'
    )
    devtool_files = $devtoolFiles
}
$packageManifest | ConvertTo-Json -Depth 4 | Set-Content -Path $packageManifestTarget -Encoding ASCII

if ($CreateZip) {
    if (Test-Path $ZipPath) {
        Remove-StagePath -Path $ZipPath -Label 'zip artifact'
    }
    try {
        Compress-Archive -Path (Join-Path $StageDir '*') -DestinationPath $ZipPath
    } catch [System.IO.IOException] {
        throw "zip artifact is locked or in use: $ZipPath. Close any running process using the zip and retry."
    }
}

Write-Output "staged_windows_package: dir=$StageDir exe=$exeTarget manifest=$manifestTarget readme=$readmeTarget run_script=$runExampleTarget run_cmd=$runExampleCmdTarget package_manifest=$packageManifestTarget include_bench_tools=$([bool]$IncludeBenchTools) zip_path=$ZipPath"
