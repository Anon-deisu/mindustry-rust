<#
.SYNOPSIS
Verifies Rust split-workspace coverage for this repository.

.DESCRIPTION
Checks each required Rust workspace manifest, validates workspace membership
via `cargo metadata`, and (optionally) runs `cargo test --workspace --no-run`
for each workspace manifest so coverage is explicit across split workspaces.

.PARAMETER MetadataOnly
Only run workspace membership checks via `cargo metadata`.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1 -MetadataOnly
#>
param(
    [switch]$MetadataOnly
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

Push-Location $repoRoot
try {
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

Write-Output ("verified_rust_workspaces: metadata_only={0} checks={1} results={2}" -f ([bool]$MetadataOnly), $workspaceChecks.Count, ($summaryText -join '; '))
