<#
.SYNOPSIS
Removes legacy Windows package stage directories.

.DESCRIPTION
Deletes old single-directory staging outputs that should not coexist with the
current `core/devtools` release layout.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\clean-legacy-mdt-package-dirs.ps1

.EXAMPLE
powershell -NoProfile -ExecutionPolicy Bypass -Command "& '.\tools\clean-legacy-mdt-package-dirs.ps1' -Confirm:`$false"
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param()

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$legacyStageDirs = @(
    (Join-Path $repoRoot 'build\windows\mdt-client-min-online'),
    (Join-Path $repoRoot '-StageDir')
)

$removed = @()
$missing = @()

foreach ($path in $legacyStageDirs) {
    if (-not (Test-Path $path)) {
        $missing += $path
        continue
    }

    if ($PSCmdlet.ShouldProcess($path, 'Remove legacy stage dir')) {
        Remove-Item -Path $path -Recurse -Force
        $removed += $path
    }
}

Write-Output "cleaned_legacy_stage_dirs: removed=$($removed -join ';') missing=$($missing -join ';')"
