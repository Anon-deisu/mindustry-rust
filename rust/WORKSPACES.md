# Rust Workspace Map

The Rust tree is intentionally split across multiple workspaces.

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
