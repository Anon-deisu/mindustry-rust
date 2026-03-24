# Parity Gap Shard Session/World (2026-03-24g)

Purpose: freeze the highest-signal remaining session/snapshot/world gaps so later workers can read one short shard instead of re-auditing from scratch.

## High

- `entitySnapshot` still stops at parsed-family projection instead of Java `readSyncEntity()` live apply.
  - Java anchor: `core/src/mindustry/core/NetClient.java` `readSyncEntity(...)`
  - Rust anchor: `rust/mdt-client-min/src/client_session.rs`
  - Rust anchor: `rust/mdt-client-min/src/session_state.rs`
  - Current state: Rust parses many vanilla families and writes `EntityTableProjection` / semantic mirrors, but still does not do `EntityMapping.map -> readSync -> snapSync -> add`.

- `blockSnapshot` still stops at authoritative building projection instead of Java `tile.build.readSync(..., version)`.
  - Java anchor: `core/src/mindustry/core/NetClient.java` `blockSnapshot(...)`
  - Rust anchor: `rust/mdt-client-min/src/client_session.rs`
  - Rust anchor: `rust/mdt-client-min/src/session_state.rs`
  - Current state: low-risk base/tail folds are broad, but runtime building ownership/apply depth is still missing.

- loaded-world/session activation is still split across shallow bootstrap helpers instead of Java `NetworkIO.loadWorld(...) -> finishConnecting()`.
  - Java anchor: `core/src/mindustry/net/NetworkIO.java` `loadWorld(...)`
  - Java anchor: `core/src/mindustry/core/NetClient.java` `finishConnecting()`
  - Rust anchor: `rust/mdt-client-min/src/bootstrap_flow.rs`
  - Rust anchor: `rust/mdt-client-min/src/client_session.rs`
  - Rust anchor: `rust/mdt-world/src/lib.rs`
  - Current state: Rust has metadata/bootstrap summaries, post-load world graph helpers, `mark_client_loaded()`, and deferred replay, but not one integrated live-apply activation path.

## Medium

- `.msav -> post_load_world()` now has useful query helpers, but still not Java live world/entity instantiate semantics.
  - Java anchor: `core/src/mindustry/net/NetworkIO.java` `loadWorld(...)`
  - Rust anchor: `rust/mdt-world/src/save_post_load.rs`
  - Rust anchor: `rust/mdt-world/src/lib.rs`

- hidden/entity lifecycle is safer now, but still below Java `handleSyncHidden()` / live group ownership depth.
  - Java anchor: `core/src/mindustry/core/NetClient.java` `hiddenSnapshot(...)`
  - Rust anchor: `rust/mdt-client-min/src/snapshot_ingest.rs`
  - Rust anchor: `rust/mdt-client-min/src/session_state.rs`

## Low

- bootstrap/post-load observability is stronger than before and should not be re-audited as if absent.
  - `stateSnapshot` wave-advance live signal is landed.
  - loaded-world configured tail folds listed in `release-unfinished-current-20260324f.md` are landed.
  - `.msav` post-load graph/team-plan/marker/static-fog query helpers are landed.

## Immediate Next 3

- `U1` typed `entitySnapshot` runtime apply for already parseable `Player` / `Unit` families.
- `U3` stronger `blockSnapshot` runtime building model on top of existing parsed base/tail fields.
- `U6` serial `finishConnecting` / `clientLoaded` lifecycle tightening without mixing snapshot ownership rewrites.
