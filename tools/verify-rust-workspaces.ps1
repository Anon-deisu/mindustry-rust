<#
.SYNOPSIS
Verifies Rust split-workspace coverage for this repository.

.DESCRIPTION
Checks each required Rust workspace manifest, validates workspace membership
via `cargo metadata`, and (optionally) runs `cargo test --workspace --no-run`
for each workspace manifest so coverage is explicit across split workspaces.
Can also run Gradle `verifyMdtRemoteFreshness` so remote manifest/codegen
freshness is explicit in the same verification flow.

.PARAMETER MetadataOnly
Only run workspace membership checks via `cargo metadata`.

.PARAMETER VerifyRemoteFreshness
Runs Gradle `verifyMdtRemoteFreshness` before Rust workspace checks.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1 -MetadataOnly

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1 -MetadataOnly -VerifyRemoteFreshness
#>
param(
    [switch]$MetadataOnly,
    [switch]$VerifyRemoteFreshness
)

$ErrorActionPreference = 'Stop'

function Parse-WorkspacePackageName {
    param(
        [string]$WorkspaceMemberId
    )

    $match = [regex]::Match($WorkspaceMemberId, '/([^/#]+)#')
    if ($match.Success) {
        return $match.Groups[1].Value
    }

    throw "unable to parse workspace member id: $WorkspaceMemberId"
}

function Assert-ContainsAll {
    param(
        [string[]]$Actual,
        [string[]]$Expected,
        [string]$Label
    )

    foreach ($value in $Expected) {
        if ($Actual -notcontains $value) {
            throw "$Label missing expected member: $value"
        }
    }
}

function Find-GradleWrapperPath {
    param(
        [string]$RepoRoot
    )

    foreach ($candidate in @('gradlew.bat', 'gradlew')) {
        $wrapperPath = Join-Path $RepoRoot $candidate
        if (Test-Path $wrapperPath) {
            return $wrapperPath
        }
    }

    return $null
}

function Invoke-RemoteFreshnessGradleTask {
    param(
        [string]$GradleWrapperPath
    )

    $savedErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        if ($GradleWrapperPath.EndsWith('.bat', [System.StringComparison]::OrdinalIgnoreCase) -or
            $GradleWrapperPath.EndsWith('.cmd', [System.StringComparison]::OrdinalIgnoreCase)) {
            $quotedWrapperPath = '"' + $GradleWrapperPath + '"'
            $commandLine = "$quotedWrapperPath -PnoLocalArc verifyMdtRemoteFreshness --stacktrace"
            $output = @(& cmd.exe /c $commandLine 2>&1)
        } else {
            $output = @(& $GradleWrapperPath -PnoLocalArc verifyMdtRemoteFreshness --stacktrace 2>&1)
        }

        return [ordered]@{
            exit_code = $LASTEXITCODE
            output = $output
        }
    }
    finally {
        $ErrorActionPreference = $savedErrorActionPreference
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$rustRoot = Join-Path $repoRoot 'rust'

$workspaceChecks = @(
    @{
        name = 'rust-root'
        manifest = Join-Path $rustRoot 'Cargo.toml'
        expectedMembers = @('mdt-protocol', 'mdt-remote', 'mdt-typeio', 'mdt-world')
    },
    @{
        name = 'mdt-input'
        manifest = Join-Path $rustRoot 'mdt-input\Cargo.toml'
        expectedMembers = @('mdt-input')
    },
    @{
        name = 'mdt-client-min'
        manifest = Join-Path $rustRoot 'mdt-client-min\Cargo.toml'
        expectedMembers = @('mdt-client-min')
    },
    @{
        name = 'mdt-render-ui'
        manifest = Join-Path $rustRoot 'mdt-render-ui\Cargo.toml'
        expectedMembers = @('mdt-render-ui')
    }
)

$summary = @()
$remoteFreshnessVerified = $false
$remoteFreshnessStatus = 'not_requested'

Push-Location $repoRoot
try {
    if ($VerifyRemoteFreshness) {
        $gradleWrapperPath = Find-GradleWrapperPath -RepoRoot $repoRoot
        if ([string]::IsNullOrWhiteSpace($gradleWrapperPath)) {
            $remoteFreshnessStatus = 'failed_wrapper_missing'
            throw 'verifyMdtRemoteFreshness requested but no Gradle wrapper was found at the repository root.'
        } else {
            Write-Output "remote_freshness_check: status=running wrapper=$gradleWrapperPath task=verifyMdtRemoteFreshness"
            $remoteFreshnessResult = Invoke-RemoteFreshnessGradleTask -GradleWrapperPath $gradleWrapperPath
            if ($remoteFreshnessResult.exit_code -ne 0) {
                throw "verifyMdtRemoteFreshness failed via $gradleWrapperPath`n$($remoteFreshnessResult.output | Out-String)"
            }

            $remoteFreshnessVerified = $true
            $remoteFreshnessStatus = 'ok'
            Write-Output "remote_freshness_check: status=ok wrapper=$gradleWrapperPath task=verifyMdtRemoteFreshness"
        }
    }

    foreach ($check in $workspaceChecks) {
        $manifest = $check.manifest
        if (-not (Test-Path $manifest)) {
            throw "workspace manifest not found: $manifest"
        }

        $metadataJson = & cargo metadata --format-version 1 --no-deps --manifest-path $manifest 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "cargo metadata failed for $($check.name): $metadataJson"
        }

        $metadata = $metadataJson | ConvertFrom-Json
        $workspaceMembers = @($metadata.workspace_members | ForEach-Object {
                Parse-WorkspacePackageName -WorkspaceMemberId $_
            })

        Assert-ContainsAll -Actual $workspaceMembers -Expected $check.expectedMembers -Label "$($check.name) workspace_members"

        if (-not $MetadataOnly) {
            & cargo test --workspace --manifest-path $manifest --no-run
            if ($LASTEXITCODE -ne 0) {
                throw "cargo test --no-run failed for $($check.name) ($manifest)"
            }
        }

        $summary += [ordered]@{
            workspace = $check.name
            manifest = $manifest
            members = ($workspaceMembers -join ',')
            verified_tests = [bool](-not $MetadataOnly)
        }
    }
}
finally {
    Pop-Location
}

$summaryText = $summary | ForEach-Object {
    "$($_.workspace)|members=$($_.members)|tests=$($_.verified_tests)"
}

Write-Output ("verified_rust_workspaces: metadata_only={0} remote_freshness_checked={1} remote_freshness_status={2} checks={3} results={4}" -f ([bool]$MetadataOnly), ([bool]$remoteFreshnessVerified), $remoteFreshnessStatus, $workspaceChecks.Count, ($summaryText -join '; '))
