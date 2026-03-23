# Rust Workspace Docs

This directory contains the Rust-side Mindustry compatibility work.

Current delivery scope:

- minimal compatibility client
- releaseable split-workspace toolchain
- ongoing parity work against the Java baseline

Current non-goal:

- claiming full Java desktop parity

## Start Here

- [ARCHITECTURE.md](ARCHITECTURE.md): crate responsibilities, dependency direction, and hot files
- [WORKSPACES.md](WORKSPACES.md): split-workspace map and the right command for each workspace
- [WORKSPACE_RUNBOOK.md](WORKSPACE_RUNBOOK.md): day-to-day verification entry points
- [FIXTURE_PATHS.md](FIXTURE_PATHS.md): canonical fixture policy

## Crates

- `mdt-protocol`: packet framing, framework messages, transport-level codec work
- `mdt-remote`: remote manifest, registry, and codegen inputs
- `mdt-typeio`: `TypeIO` object decoding and bounded semantic helpers
- `mdt-world`: world stream, snapshot parsing, and loaded-world model
- `mdt-input`: input intent and build-plan editing helpers
- `mdt-client-min`: minimal compatibility client session/runtime orchestration
- `mdt-render-ui`: HUD/presenter/view-side projection output

## Split Workspace Reminder

Do not assume `cargo test --workspace --manifest-path rust/Cargo.toml` covers every Rust crate.

- root workspace: `mdt-protocol`, `mdt-remote`, `mdt-typeio`, `mdt-world`
- standalone workspaces: `mdt-input`, `mdt-client-min`, `mdt-render-ui`

Use [WORKSPACES.md](WORKSPACES.md) or [WORKSPACE_RUNBOOK.md](WORKSPACE_RUNBOOK.md) before running broad verification.
