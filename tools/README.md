# Tools

## Windows Release

- [`WINDOWS-RELEASE.md`](WINDOWS-RELEASE.md): default Windows release command, artifact policy, release gate, localhost-server prerequisite, legacy cleanup flow
- [`package-mdt-client-min-release-set.ps1`](package-mdt-client-min-release-set.ps1): builds `core/devtools` stage directories and zip artifacts; use `-Verify` for the default release gate (preflight enforced by default)
- [`verify-mdt-client-min-release-set.ps1`](verify-mdt-client-min-release-set.ps1): validates staged artifacts, manifests, core smoke, and devtools bench; now runs preflight first unless `-SkipPreflight` is explicitly set
- [`check-mdt-release-prereqs.ps1`](check-mdt-release-prereqs.ps1): non-destructive preflight for required release scripts, canonical fixtures, and localhost-server prerequisite
- [`verify-rust-workspaces.ps1`](verify-rust-workspaces.ps1): verifies split Rust workspace coverage (`rust` root + `mdt-input` + `mdt-client-min` + `mdt-render-ui`)
- [`clean-legacy-mdt-package-dirs.ps1`](clean-legacy-mdt-package-dirs.ps1): removes legacy single-directory staging outputs before hard-gated release verification
- [`package-mdt-client-min-online.ps1`](package-mdt-client-min-online.ps1): stages a single `core` or `devtools` package; `-IncludeBenchTools` now stages runnable `mdt-render-ui-window-bench/ascii/window/window-demo` binaries under `devtools`

Release scope for these tools: Rust minimal compatibility client, not full Java
desktop-client parity.

## CI Release Gate Governance

Reference-only note: this section documents the current source-monorepo gate
policy and runbook. The target `mindustry-rust` repo tracks these documents,
but does not automatically imply the workflow is already wired there.

- Required protected-branch check context: `Rust Release Gate / rust-release-gate`
- Owner roles:
  - Primary: `ReleaseOperator`
  - Backup: `ReleaseManager`
  - Server-smoke triage: `QA`
- Skip policy: CI workflow does not use preflight/workspace bypass flags.
- Waiver policy: exception-only, needs `ReleaseManager` + `QA` approval with
  issue ID and max `24h` expiry.
- Full policy/runbook: [`../audit/ci-gate-plan.md`](../audit/ci-gate-plan.md)

## Handoff

- [`MINDUSTRY-RUST-HANDOFF.md`](MINDUSTRY-RUST-HANDOFF.md): current minimal sync surface for handing Rust client work to the target `mindustry-rust` repo
- [`mindustry-rust-target.json`](mindustry-rust-target.json): machine-readable single upload target record for future sync/push work
- [`get-mindustry-rust-target.ps1`](get-mindustry-rust-target.ps1): prints the fixed upload target, effective checkout resolution, and sync strategy
- [`sync-mindustry-rust-handoff.ps1`](sync-mindustry-rust-handoff.ps1): copies the handoff include set into the fixed `mindustry-rust` checkout after verifying the checkout remote
- [`mindustry-rust-repo-README.md`](mindustry-rust-repo-README.md): target-repo root README template synced as `README.md`
- Handoff fixture rule: runtime fixtures are canonical under `fixtures/...`; Rust-consumed parity fixtures stay source-compatible under `tests/src/test/resources/...` for now
- Sync guard: `sync-mindustry-rust-handoff.ps1` now hard-fails when `SourceRoot` and `TargetCheckout` resolve to the same directory, to avoid accidentally running it inside the target repo as a self-copy no-op
