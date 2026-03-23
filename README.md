# mindustry-rust

Rust Mindustry client work tracked out of the source monorepo.

Current delivery scope:

- Rust minimal compatibility client
- release/runtime fixtures under `fixtures/...`
- Rust-consumed parity fixtures kept under `tests/src/test/resources/...`
- not a claim of full Java desktop parity

Primary entry points:

- `tools/README.md`
- `tools/WINDOWS-RELEASE.md`
- `tools/MINDUSTRY-RUST-HANDOFF.md`
- `tools/get-mindustry-rust-target.ps1`

Target repo policy:

- upload target is this repository
- do not push the source monorepo history to upstream `Anuken/Mindustry`
- use `powershell -ExecutionPolicy Bypass -File .\tools\sync-mindustry-rust-handoff.ps1 -Stage` from the source workspace to refresh the tracked handoff surface
