# mindustry-rust Handoff

## Goal

This note defines the current minimal sync surface for handing the Rust client
work to the target repository:

- delivery scope: minimal compatibility client
- not a claim of full parity with the Java desktop client
- single upload target: `https://github.com/Anon-deisu/mindustry-rust`
- this mirrored copy exists in the target repo for traceability; most target-repo
  contributors do not run the sync command from this checkout

Architecture boundary source of truth:

- `rust/ARCHITECTURE.md` owns crate responsibilities, dependency direction, fixture policy,
  and split-workspace rules
- `rust/README.md` owns the Rust docs entry point
- `rust/WORKSPACES.md` owns the split-workspace topology summary
- `rust/WORKSPACE_RUNBOOK.md` owns day-to-day command entry points for the split workspaces
- `rust/FIXTURE_PATHS.md` owns canonical-vs-test-vs-mirror fixture path policy
- this handoff note only owns sync/upload policy

- target repo: `https://github.com/Anon-deisu/mindustry-rust`
- current source workspace: `<source_repo_root>`
- machine-readable target anchor: `tools/mindustry-rust-target.json`
- machine-readable handoff include manifest: `tools/mindustry-rust-handoff-manifest.json`
- quick lookup command: `powershell -ExecutionPolicy Bypass -File .\tools\get-mindustry-rust-target.ps1`
- sync command: `powershell -ExecutionPolicy Bypass -File .\tools\sync-mindustry-rust-handoff.ps1 -Stage`
- manifest self-check: `powershell -ExecutionPolicy Bypass -File .\tools\sync-mindustry-rust-handoff.ps1 -ValidateManifest`
- sync guard: the sync script now rejects `SourceRoot == TargetCheckout`; if you run it outside the source workspace, pass `-SourceRoot` explicitly

Canonical fixture layout (source + target):

- `fixtures/remote/remote-manifest-v1.json`
- `fixtures/world-streams/archipelago-6567-world-stream.hex`

Current monorepo rule:

- treat `fixtures/...` as the canonical primary fixture path
- keep `rust/fixtures/...` only as a non-release mirror copy
- release/handoff docs must point to `fixtures/...` as the default path
- current release-facing scripts are `canonical_only`; they do not consume
  transitional fallback paths anymore

Resolved target-repo parity-fixture rule:

- keep Rust-consumed parity fixtures under source-compatible
  `tests/src/test/resources/...` paths in `mindustry-rust`
- reason: current Rust crates still hardcode those paths through
  `include_str!(...)`, `fs::read_to_string(...)`, and CLI/demo defaults
- do not migrate parity fixtures into a Rust-owned fixture root until those
  code paths are first abstracted away

## Include

The canonical include list now lives only in
`tools/mindustry-rust-handoff-manifest.json`, consumed directly by
`tools/sync-mindustry-rust-handoff.ps1`.

The manifest currently covers these categories:

- Rust workspace roots, crate manifests, split-workspace lockfiles, and selected `src/` trees
- handoff/release tooling and the target-repo root `README.md` mapping
- reference-only `audit/ci-gate-plan.md`
- build-required crate-local metadata
- parity fixtures under `tests/src/test/resources/...`
- canonical runtime fixtures under `fixtures/...`
- transitional fixture mirrors under `rust/fixtures/...`

If the sync surface changes, update the manifest first; the script and stage set
will follow it automatically.

## Do Not Sync As-Is

Do not copy these local outputs into the target repo as committed artifacts:

- `rust/**/target/`
- `.gradle/`
- `.gradle-project-cache/`
- `.gradle-user/`
- `build/`
- `build/windows/`
- `build/mdt-remote/`
- `build/archipelago-6567-world-stream.hex`
- `gradle-9.3.1-bin.zip`
- `tmp-maps-out.txt`
- `tmp-maps-err.txt`

## Runtime Artifacts Re-Homed

The canonical repo-owned locations are:

- `fixtures/remote/remote-manifest-v1.json`
- `fixtures/world-streams/archipelago-6567-world-stream.hex`

Transitional mirror locations (non-canonical repository mirrors only):

- `rust/fixtures/remote/remote-manifest-v1.json`
- `rust/fixtures/world-streams/archipelago-6567-world-stream.hex`

The old `build/` copies should now be treated as regeneration outputs, not as
the canonical paths to sync into the target repo. `rust/fixtures/...` is also
non-canonical and transitional; handoff sync should use `fixtures/...` first,
and release-facing automation should stay on canonical paths only.

## Workspace Note

The current source tree is still split:

- root `rust/Cargo.toml` only covers `mdt-protocol`, `mdt-remote`, `mdt-typeio`,
  and `mdt-world`
- `mdt-input`, `mdt-client-min`, and `mdt-render-ui` currently carry their own
  `[workspace]` sections and lockfiles

Do not assume `cargo test --workspace --manifest-path rust/Cargo.toml` covers
the release binaries. The current Windows release scripts build those crates by
their own manifest paths.

## Current Windows Release Policy

- primary artifact: `mdt-client-min-online-windows.zip`
- secondary artifact: `mdt-client-min-online-devtools.zip`
- default release gate:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -Verify -AnimatePlayer
```

- optional staged sample world:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -BenchWorldStreamHex .\fixtures\world-streams\archipelago-6567-world-stream.hex -Verify -AnimatePlayer
```

## Upload Layout Decision

Target repo layout is now fixed as:

- keep `tools/README.md` and `tools/WINDOWS-RELEASE.md` as the detailed entry
  points
- sync `tools/mindustry-rust-repo-README.md` to target-repo root `README.md`
  as the top-level entry point
