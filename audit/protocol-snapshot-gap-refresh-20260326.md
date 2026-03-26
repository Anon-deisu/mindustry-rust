# Protocol / Snapshot / World Sync Gap Refresh (2026-03-26)

## Scope

This refresh re-reads these audit baselines against the current Rust code:

- `RUST_RELEASE_AUDIT_FINDINGS.md`
- `RUST_RELEASE_AUDIT_CONTINUATION.md`
- `audit/remote-packet-coverage.md`
- `audit/client-snapshot-parity.md`
- `audit/world-sync-gap-refresh-20260324d.md`

The goal is to keep only the gaps that are still real on 2026-03-26, then group them by parallel write lane.

## Bottom Line

- The Rust stack is no longer blocked on missing packet ids or missing basic decode paths.
- The main remaining risk is semantic depth: many paths now have decode, observability, and lightweight projections, but still do not have Java-equivalent live apply or business behavior.
- The highest-risk area remains snapshot convergence.
- World sync is no longer "cannot parse msav/world stream"; it is now "cannot fully apply/save/synchronize world state with one authoritative runtime model".
- `worldDataBegin` / `client_loaded` / deferred replay remain a serial owner lane and should not be split across concurrent writers.

## Lane A: Snapshot Semantic Apply

### A1. `entitySnapshot` is still a bounded family parser, not Java-equivalent `readSyncEntity`

Current Rust behavior:

- `entitySnapshot` handling applies a whitelist of parseable row families: player, alpha, building, mech, missile, payload, building-tether, fire, puddle, weather, and world-label.
- Evidence: `rust/mdt-client-min/src/client_session.rs:6185-6325`
- For the currently covered unit families, the runtime typed mirror is no longer a pure thin semantic subset: bounded `runtime_sync` fields now retain `ammo_bits/elevation_bits/flag_bits`, mech rows also retain `base_rotation_bits`, and the same path carries a bounded carried-item stack mirror.
- Evidence: `rust/mdt-client-min/src/client_session.rs:7436-7572`

Why this is still a gap:

- This is still `try_parse known shapes + apply lightweight table rows`, not generic entity instantiation/readSync/add semantics.
- Unknown or drifted class families still remain fail-closed or projection-only.
- Long-session convergence can still diverge from Java live entity state.

Parallel lane:

- Write scope:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/session_state.rs`
- Do not touch:
  - `worldDataBegin`
  - `client_loaded`

### A2. `stateSnapshot` is still authority/business projection, not live game-state apply

Current Rust behavior:

- `snapshot_ingest.rs` parses `stateSnapshot`, parses `coreData`, updates authority projection, updates business projection, and records parse-fail telemetry.
- Evidence: `rust/mdt-client-min/src/snapshot_ingest.rs:58-131`, `rust/mdt-client-min/src/snapshot_ingest.rs:227-260`
- Reload clears the entire snapshot authority/business surface.
- Evidence: `rust/mdt-client-min/src/client_session.rs:8681-8718`

Why this is still a gap:

- Rust can summarize wave, pause, game over, and core inventory state.
- Rust still does not apply those semantics into a Java-equivalent live world/game-state system.

Parallel lane:

- Write scope:
  - `rust/mdt-client-min/src/snapshot_ingest.rs`
  - `rust/mdt-client-min/src/session_state.rs`
- Keep this lane projection/apply-focused only.

### A3. `blockSnapshot` and `hiddenSnapshot` are still lightweight apply, not Java world-sync parity

Current Rust behavior:

- `blockSnapshot` parses envelope plus first-entry fixed-prefix/base facts, then writes a lightweight building-table projection.
- Evidence: `rust/mdt-client-min/src/snapshot_ingest.rs:134-201`
- `hiddenSnapshot` parses the current hidden id set and applies bounded hidden projection updates.
- Evidence: `rust/mdt-client-min/src/snapshot_ingest.rs:203-223`

Why this is still a gap:

- `blockSnapshot` is still not Java `tile.build.readSync(...)`.
- `hiddenSnapshot` is still not Java `handleSyncHidden()` depth.
- Runtime tables and world truth can still drift after world-stream baseline plus later authoritative deltas.

Parallel lane:

- Write scope:
  - `rust/mdt-client-min/src/snapshot_ingest.rs`
  - `rust/mdt-client-min/src/session_state.rs`
- Do not modify reload/deferred queue ownership here.

## Lane B: Protocol Business Semantics

### B1. `tileConfig` is still bounded TypeIO parse plus bounded business projection

Current Rust behavior:

- `decode_tile_config_payload(...)` reads `build_pos` plus one TypeIO object and treats trailing bytes as parse failure.
- Evidence: `rust/mdt-client-min/src/client_session.rs:13451-13509`
- Outbound `queue_tile_config(...)` records local intent; inbound apply still flows through the current bounded authority/business path.
- Evidence: `rust/mdt-client-min/src/client_session.rs:2076-2145`

Why this is still a gap:

- This is enough for minimal compatibility and current configured-block coverage.
- It is still not Java-equivalent authoritative rollback/apply behavior.

Parallel lane:

- Write scope:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/session_state.rs`

### B2. `effect(..., data)` is still bounded decode/projection/overlay, not general effect execution

Current Rust behavior:

- `mdt-typeio` exposes effect-safe object decode.
- Evidence: `rust/mdt-typeio/src/object.rs:698-729`
- `client_session.rs` derives `EffectBusinessProjection` from the decoded object.
- Evidence: `rust/mdt-client-min/src/client_session.rs:11452-12220`
- Runtime effect rendering is still based on selected effect ids and selected line/position projections.
- Evidence: `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs:232-270`, `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs:400-460`

Why this is still a gap:

- Rust can observe and render selected high-signal effect data.
- Rust still does not implement Java-equivalent general effect business semantics.

Parallel lane:

- Write scope:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`
  - `rust/mdt-typeio/src/object.rs`

### B3. `setRules` / `setObjectives` / `setRule` remain raw JSON/string projection paths

Current Rust behavior:

- These packets decode JSON/string payloads and patch projections directly.
- Evidence: `rust/mdt-client-min/src/client_session.rs:5132-5215`, `rust/mdt-client-min/src/client_session.rs:11351-11370`

Why this is still a gap:

- Good enough for minimal compatibility.
- Still not full Java `Rules` / `MapObjectives` object semantics.

Parallel lane:

- Write scope:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/session_state.rs`

### B4. Custom/binary/logic packet families still stop at queue + dispatch + observability

Current Rust behavior:

- Packet-id classification is now registry-based instead of a local hand-written chain: `mdt-remote` exposes typed packet registries, and `mdt-client-min` consumes those through combined/custom-channel registries for well-known, high-frequency, inbound, and custom families.
- Evidence: `rust/mdt-client-min/src/packet_registry.rs:230-370`
- Evidence: `rust/mdt-remote/src/lib.rs:1721-1989`
- Rust also exposes outbound logic-data queueing and matching bounded dispatch surfaces on top of that typed classification layer.
- Evidence: `rust/mdt-client-min/src/client_session.rs:1840-1864`

Why this is still a gap:

- Packet presence is no longer the problem.
- Java-equivalent business integration for custom/mod/plugin logic is still shallow.

Parallel lane:

- Write scope:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/typed_remote_dispatch.rs`

## Lane C: World Sync / Save / Post-Load

### C1. `mdt-world` still lacks a public save writer / regionized write pipeline

Current Rust behavior:

- `post_load_world()` returns `SavePostLoadWorldObservation`.
- Evidence: `rust/mdt-world/src/lib.rs:1129-1165`
- The visible write path in current code is still CLI-side file output helper code.
- Evidence: `rust/mdt-world/src/main.rs:729-740`

Why this is still a gap:

- The world layer is still parse-first/observation-first.
- There is still no public writer pipeline comparable to Java save-version write flow.

Parallel lane:

- Write scope:
  - `rust/mdt-world/src/lib.rs`

### C2. `post_load_world()` now exposes readiness/query surfaces, but still cannot seed live runtime apply

Current Rust behavior:

- Save11 tests show runtime apply, ownership, and batch-plan surfaces exist.
- Evidence: `rust/mdt-world/src/lib.rs:40134-40295`
- `runtime_seed_surface()` is now a public bounded query surface for readiness, blocked/awaiting/deferred state, and next-batch summaries rather than a test-only fact.
- Evidence: `rust/mdt-world/src/lib.rs:40420-40660`
- The same test block still shows `can_seed_runtime_apply == false`.
- Evidence: `rust/mdt-world/src/lib.rs:40268-40295`
- Save6 legacy runtime-world semantics are still blocked.
- Evidence: `rust/mdt-world/src/lib.rs:40422-40485`

Why this is still a gap:

- `mdt-world` has moved beyond passive observation and now exposes consumable readiness/query surfaces.
- It still has not crossed into direct live-runtime seeding.

Parallel lane:

- Write scope:
  - `rust/mdt-world/src/lib.rs`

### C3. Loaded-world baseline and runtime authority are still dual-source

Current Rust behavior:

- `begin_world_data_reload()` clears loaded world, pending world stream, deferred packets, snapshot surfaces, and many runtime authority projections in one reset path.
- Evidence: `rust/mdt-client-min/src/client_session.rs:8623-8735`

Why this is still a gap:

- Baseline world, snapshot tables, and runtime business projections still coexist as separate state surfaces.
- This keeps truth-source convergence risky around `blockSnapshot`, `tileConfig`, `constructFinish`, reload, and reconnect boundaries.

Parallel lane:

- Write scope:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/session_state.rs`
  - optional `rust/mdt-world/src/lib.rs`

## Lane S: Serial Owner Lane

Do not split these across concurrent workers:

- `worldDataBegin`
- `client_loaded`
- deferred inbound replay
- loading-time low-priority drop policy
- reconnect/reload reset path

Why:

- This area owns reset of loaded world, deferred queues, replay queues, snapshot surfaces, and readiness anchors in one path.
- Evidence: `rust/mdt-client-min/src/client_session.rs:8623-8735`
- Tests explicitly pin deferred/drop/reload behavior here.
- Evidence: `rust/mdt-client-min/src/client_session.rs:42292-42520`

## Recommended Parallel Split

### Lane A1

- Topic: `entitySnapshot` breadth and semantic apply
- Files:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/session_state.rs`

### Lane A2

- Topic: `stateSnapshot` / `blockSnapshot` / `hiddenSnapshot` semantic deepen
- Files:
  - `rust/mdt-client-min/src/snapshot_ingest.rs`
  - `rust/mdt-client-min/src/session_state.rs`

### Lane B1

- Topic: `tileConfig` authoritative rollback/business semantics
- Files:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/session_state.rs`

### Lane B2

- Topic: `effect(..., data)` bounded semantic executor
- Files:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`
  - `rust/mdt-typeio/src/object.rs`

### Lane B3

- Topic: rules/objectives and custom/logic packet deeper business integration
- Files:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/session_state.rs`
  - `rust/mdt-client-min/src/typed_remote_dispatch.rs`

### Lane C1

- Topic: `mdt-world` save writer and post-load applicative helper work
- Files:
  - `rust/mdt-world/src/lib.rs`

### Serial Lane S

- Topic: `worldDataBegin` / `client_loaded` / defer-replay lifecycle owner
- Files:
  - `rust/mdt-client-min/src/client_session.rs`
  - `rust/mdt-client-min/src/bootstrap_flow.rs`

## Changed File Paths

- `D:\MDT\mindustry\audit\protocol-snapshot-gap-refresh-20260326.md`
