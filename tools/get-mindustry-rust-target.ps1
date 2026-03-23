<#
.SYNOPSIS
Shows the fixed upload target for Rust Mindustry handoff work.

.DESCRIPTION
Reads `tools/mindustry-rust-target.json` and prints the canonical target repo,
effective target checkout resolution, and sync strategy so future upload work does not need
to rediscover the destination.

.PARAMETER Json
Outputs the raw config as JSON.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\get-mindustry-rust-target.ps1

.EXAMPLE
powershell -ExecutionPolicy Bypass -File .\tools\get-mindustry-rust-target.ps1 -Json
#>
[CmdletBinding()]
param(
    [switch]$Json
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$configPath = Join-Path $scriptDir "mindustry-rust-target.json"

if (!(Test-Path $configPath)) {
    throw "Missing target config: $configPath"
}

$config = Get-Content -Raw -Path $configPath | ConvertFrom-Json
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
$gitConfiguredCheckout = (& git -C $repoRoot config --local --get mdt.targetcheckout 2>$null)
if ($LASTEXITCODE -ne 0) {
    $gitConfiguredCheckout = ''
}
$envConfiguredCheckout = $env:MDT_TARGET_CHECKOUT
$effectiveCheckout = if (-not [string]::IsNullOrWhiteSpace($gitConfiguredCheckout)) {
    $gitConfiguredCheckout
} elseif (-not [string]::IsNullOrWhiteSpace($envConfiguredCheckout)) {
    $envConfiguredCheckout
} else {
    ''
}

if ($Json) {
    $config | ConvertTo-Json -Depth 8
    exit 0
}

[pscustomobject]@{
    TargetRepo              = $config.target_repo
    DefaultBranch           = $config.default_branch
    EffectiveCheckoutPath   = $effectiveCheckout
    CheckoutResolutionOrder = ($config.target_checkout_resolution_order -join " -> ")
    CheckoutHint            = $config.target_checkout_hint
    SyncStrategy            = $config.sync_strategy
    CanonicalFixtures       = ($config.canonical_fixture_paths -join ", ")
    ParityFixtureRoot       = $config.canonical_parity_fixture_root
}
