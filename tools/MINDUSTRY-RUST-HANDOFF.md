# mindustry-rust Handoff

## Goal

This note defines the current minimal sync surface for handing the Rust client
work to the target repository:

- delivery scope: minimal compatibility client
- not a claim of full parity with the Java desktop client
- single upload target: `https://github.com/Anon-deisu/mindustry-rust`

- target repo: `https://github.com/Anon-deisu/mindustry-rust`
- current source workspace: `<source_repo_root>` (example: `D:\MDT\mindustry`)
- machine-readable target anchor: `tools/mindustry-rust-target.json`
- quick lookup command: `powershell -ExecutionPolicy Bypass -File .\tools\get-mindustry-rust-target.ps1`

Canonical fixture layout (source + target):

- `fixtures/remote/remote-manifest-v1.json`
- `fixtures/world-streams/archipelago-6567-world-stream.hex`

Current monorepo rule:

- treat `fixtures/...` as the canonical primary fixture path
- keep `rust/fixtures/...` only as a non-release mirror copy
- release/handoff docs must point to `fixtures/...` as the default path
- current release-facing scripts are `canonical_only`; they do not consume
  transitional fallback paths anymore

## Include

Sync these areas first:

- `rust/`
  - root workspace `Cargo.toml`
  - root workspace `Cargo.lock`
  - crate manifests and `src/` trees for:
    - `mdt-protocol`
    - `mdt-typeio`
    - `mdt-world`
    - `mdt-input`
    - `mdt-remote`
    - `mdt-client-min`
    - `mdt-render-ui`
  - split-workspace lockfiles still needed by current release scripts:
    - `mdt-input/Cargo.lock`
    - `mdt-client-min/Cargo.lock`
    - `mdt-render-ui/Cargo.lock`
- `tools/`
  - `package-mdt-client-min-online.ps1`
  - `package-mdt-client-min-release-set.ps1`
  - `verify-mdt-client-min-release-set.ps1`
  - `clean-legacy-mdt-package-dirs.ps1`
  - `WINDOWS-RELEASE.md`
  - `README.md`
- Rust/Java parity fixtures still needed by the client work:
  - `tests/src/test/resources/connect-packet.hex`
  - `tests/src/test/resources/payload-campaign-compound-goldens.txt`
  - `tests/src/test/resources/snapshot-goldens.txt`
  - `tests/src/test/resources/typeio-goldens.txt`
  - `tests/src/test/resources/world-stream.hex`
- Repo-owned runtime fixtures (canonical primary path):
  - `fixtures/remote/remote-manifest-v1.json`
  - `fixtures/world-streams/archipelago-6567-world-stream.hex`
- Transitional fixture mirror (non-canonical, not used by current release scripts):
  - `rust/fixtures/remote/remote-manifest-v1.json`
  - `rust/fixtures/world-streams/archipelago-6567-world-stream.hex`

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

## Open Decisions Before Upload

- Decide whether the target repo keeps Java parity fixtures under the same paths or moves them into a Rust-owned fixture directory.
- Decide whether the target repo wants the same `tools/README.md` and `WINDOWS-RELEASE.md` layout or a repo-root delivery doc.
