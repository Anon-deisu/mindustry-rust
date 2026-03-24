# Rust Workspace Map

The Rust tree is intentionally split across multiple workspaces.

Scope reminder:

- external wording stays at `minimal compatibility client`
- green workspace checks do not imply full Java desktop parity

## Root Workspace

Manifest:

- `rust/Cargo.toml`

Members:

- `mdt-protocol`
- `mdt-remote`
- `mdt-typeio`
- `mdt-world`

Typical command:

```powershell
cargo test --workspace --manifest-path .\rust\Cargo.toml
```

## Standalone Workspaces

`mdt-input`

- manifest: `rust/mdt-input/Cargo.toml`
- command: `cargo test --manifest-path .\rust\mdt-input\Cargo.toml`

`mdt-client-min`

- manifest: `rust/mdt-client-min/Cargo.toml`
- command: `cargo test --manifest-path .\rust\mdt-client-min\Cargo.toml`

`mdt-render-ui`

- manifest: `rust/mdt-render-ui/Cargo.toml`
- command: `cargo test --manifest-path .\rust\mdt-render-ui\Cargo.toml`

## Responsibility Map

Use this as the default routing rule before adding code:

- `mdt-protocol`: packet framing, framework messages, transport byte-level codec work
- `mdt-remote`: remote manifest, registry, codegen input/output
- `mdt-typeio`: `TypeIO` object decoding and bounded object-semantic helpers
- `mdt-world`: world-stream parsing, snapshot/world model, loaded-world tails/markers/custom chunks
- `mdt-input`: input intent mapping, live intent state, build-plan editing helpers
- `mdt-client-min`: session lifecycle, network loops, runtime observability, CLI/runtime orchestration
- `mdt-render-ui`: render projection, HUD/view DTOs, ASCII/window presenters

Hard guardrails:

- `mdt-world` must not depend on `mdt-render-ui` or `mdt-client-min`
- `mdt-input` should stay free of session/runtime transport logic
- `mdt-render-ui` should consume stable projection/model inputs, not absorb network/session behavior
- `mdt-client-min` is the integration layer, but narrow reusable logic should still be pushed down first

## Verification Order

When a change is local to one crate, start with the narrowest matching command above.

When a change crosses workspace boundaries, use this escalation order:

1. focused crate test
2. `powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1`
3. `powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -Verify`

## Release-Oriented Verification

For the current full Rust verification entry point, use:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1
```

For the current Windows release verification entry point, use:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -Verify
```

## Guardrail

The release chain currently depends on all four command surfaces above.
Passing one workspace does not imply the others are green.

If the source tree is later synced into the handoff repo, rerun the same verification commands there before push.
