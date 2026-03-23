<#
.SYNOPSIS
Checks release prerequisites for `mdt-client-min-online` packaging and verify flow.

.DESCRIPTION
Validates required release scripts and canonical fixture paths. This script is
non-destructive and intended as a fast preflight check before running release
packaging/verification.

.PARAMETER Server
Server address that will be used by release verification smoke checks.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\check-mdt-release-prereqs.ps1
#>
param(
    [string]$Server = '127.0.0.1:6567'
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

$requiredScripts = @(
    (Join-Path $PSScriptRoot 'package-mdt-client-min-online.ps1'),
    (Join-Path $PSScriptRoot 'package-mdt-client-min-release-set.ps1'),
    (Join-Path $PSScriptRoot 'verify-mdt-client-min-release-set.ps1'),
    (Join-Path $PSScriptRoot 'clean-legacy-mdt-package-dirs.ps1')
)

$canonicalFixtures = [ordered]@{
    remote_manifest = (Join-Path $repoRoot 'fixtures\remote\remote-manifest-v1.json')
    world_stream_hex = (Join-Path $repoRoot 'fixtures\world-streams\archipelago-6567-world-stream.hex')
}

$issues = New-Object System.Collections.Generic.List[string]

Write-Output "release_prereq_check: repo_root=$repoRoot server=$Server fixture_policy=canonical_only"
Write-Output "server_prereq: verify flow runs live smoke against --server $Server; ensure a reachable Mindustry server is listening."

foreach ($scriptPath in $requiredScripts) {
    $exists = Test-Path $scriptPath
    Write-Output "check_script: path=$scriptPath status=$(if($exists){'ok'}else{'missing'})"
    if (-not $exists) {
        $issues.Add("missing release script: $scriptPath")
    }
}

foreach ($key in $canonicalFixtures.Keys) {
    $canonicalPath = $canonicalFixtures[$key]
    $hasCanonical = Test-Path $canonicalPath
    $canonicalStatus = if ($hasCanonical) { 'ok' } else { 'missing' }

    Write-Output ("check_fixture: key={0} canonical={1} canonical_path={2}" -f `
        $key, `
        $canonicalStatus, `
        $canonicalPath)

    if (-not $hasCanonical) {
        $issues.Add("missing canonical fixture ($key): $canonicalPath")
    }
}

if ($issues.Count -gt 0) {
    Write-Output "release_prereq_result: status=failed issue_count=$($issues.Count)"
    foreach ($issue in $issues) {
        Write-Output "release_prereq_issue: $issue"
    }
    exit 1
}

Write-Output "release_prereq_result: status=ok issue_count=0"
