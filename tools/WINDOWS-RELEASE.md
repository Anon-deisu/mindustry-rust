# MDT Windows Release

This release flow packages the Rust **minimal compatibility client**. It does
not imply parity with the full Java desktop client.

## Default Command

Run these commands from the repository root. Use this command for the default
Windows release flow:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -Verify -AnimatePlayer
```

This command:

- builds and stages the `core` package
- builds and stages the `devtools` package
- writes both zip artifacts
- requires a clean workspace for release gate verification
- expects a reachable Mindustry server at `127.0.0.1:6567` unless `-Server` is overridden

## Optional Sample World

If you want the devtools package to include a staged `sample-world-stream.hex`, pass the real dumped world stream explicitly:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -BenchWorldStreamHex .\fixtures\world-streams\archipelago-6567-world-stream.hex -Verify -AnimatePlayer
```

Without `-BenchWorldStreamHex`, the devtools package still passes release verification, but it does not embed `sample-world-stream.hex`.

## Artifact Policy

- Primary artifact: `build\windows\mdt-client-min-online-windows.zip`
- Secondary artifact: `build\windows\mdt-client-min-online-devtools.zip`
- Default first-release package is `core`
- `devtools` is bench/tooling only
- `mdt-render-ui-ascii/window/window-demo` are staged only in `devtools` (not in the default `core` package)

## Release Gate

`-Verify` on [`package-mdt-client-min-release-set.ps1`](package-mdt-client-min-release-set.ps1) is the default release gate.

The gate checks:

- release preflight (`check-mdt-release-prereqs.ps1`) before packaging/smoke
- remote freshness is now part of Gradle `check` by default, and can still be checked explicitly with `.\gradlew -PnoLocalArc verifyMdtRemoteFreshness`
- `core/devtools` stage directories exist
- both zip artifacts exist
- `PACKAGE-MANIFEST.json` metadata is correct
- core `.cmd` smoke reaches `WorldStreamReady`, `PlayerSpawned`, an ASCII scene, and final packet summary
- devtools bench smoke returns `bench_window:`
- devtools manifest includes runnable `mdt-render-ui-window-bench/ascii/window/window-demo` binaries

The core smoke is a real localhost integration check. If no compatible server
is listening at the selected `-Server`, verification fails.

Preflight enforcement is on by default for release-set verification. Use
`-SkipPreflight` only for exceptional troubleshooting flows.

If you need to bypass the default Gradle `check` remote-freshness guard for
local troubleshooting, run Gradle with `-PremoteFreshnessOnCheck=false`.

If you want the PowerShell release gate to invoke the same freshness check
explicitly, use:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -Verify -VerifyRustWorkspaces -VerifyRemoteFreshness -AnimatePlayer
```

This routes the check through `tools\verify-rust-workspaces.ps1`, which now
emits `remote_freshness_check: ...` and `verified_rust_workspaces: remote_freshness_checked=...`.
If the current checkout does not include a Gradle wrapper, the same path fails
explicitly instead of silently skipping the freshness check.

R+2 policy is canonical-only fixtures:

- transitional fallback switches are removed from release-facing scripts
- preflight fails when canonical fixtures are missing
- explicit transitional fixture path usage is a hard failure in release scripts

Policy markers in release logs:

- `release_prereq_check: ... fixture_policy=canonical_only`
- `verified_windows_release_set: ... fixture_policy=canonical_only ...`

## Legacy Stage Policy

By default, `-Verify` runs as a hard gate and fails if legacy stage directories are present.

Legacy cleanup command:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "& '.\tools\clean-legacy-mdt-package-dirs.ps1' -Confirm:`$false"
```

Compatibility override:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -Verify -AnimatePlayer -AllowLegacyStageWarning
```

Use `-AllowLegacyStageWarning` only when you explicitly need warning-compatible verification.

## Outputs

- `build\windows\mdt-client-min-online-core`
- `build\windows\mdt-client-min-online-devtools`
- `build\windows\mdt-client-min-online-windows.zip`
- `build\windows\mdt-client-min-online-devtools.zip`

## Self-Describing Metadata

Each staged package writes `PACKAGE-MANIFEST.json` with:

- `package_role`
- `artifact_tier`
- `entrypoint`
- `core_files`
- `devtool_files`
