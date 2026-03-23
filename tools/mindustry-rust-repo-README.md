# mindustry-rust

Tracked delivery repository for the Rust Mindustry client work.

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

Repository policy:

- this repository is the tracked upload target
- detailed release and handoff rules live under `tools/`
- only if you also maintain an upstream source workspace: run the handoff sync helper there with an explicit source root or configured `mdt.targetcheckout`; in this target repo it is normally not a self-update command
