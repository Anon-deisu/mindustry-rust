# Client Snapshot Parity (M7 Session/Network)

Date: 2026-03-23  
Scope: current repository state (`rust/mdt-client-min` vs Java `NetClient`)

## Aligned Items (Implemented)

1. `clientSnapshot` core payload path is implemented in Rust and sent on loop cadence.
   - Rust encoder: `encode_client_snapshot_payload(...)` in `rust/mdt-client-min/src/client_session.rs:1194`
   - Rust send path: `advance_time(...)` in `rust/mdt-client-min/src/client_session.rs:366`, `:384`
   - Java reference send path: `sync()` -> `Call.clientSnapshot(...)` in `core/src/mindustry/core/NetClient.java:686`, `:692`

2. Mining tile is no longer placeholder-only and is encoded with `pack_point2`.
   - Rust: `rust/mdt-client-min/src/client_session.rs:1222`, `:1239`
   - Rust test evidence: `rust/mdt-client-min/src/client_session.rs:2022`, `:2068`
   - Java golden decode assertion: `tests/src/test/java/ApplicationTests.java:6569`, `:6571`

3. Build-plan queue is hard-capped to Java network limit `20`.
   - Rust cap: `rust/mdt-client-min/src/client_session.rs:1272`
   - Rust test evidence: `rust/mdt-client-min/src/client_session.rs:2177`

4. Ping/pingResponse flow is implemented in both directions (queue response + RTT record).
   - Rust send ping in runtime: `rust/mdt-client-min/src/client_session.rs:366`, `:368`
   - Rust receive ping/pingResponse: `rust/mdt-client-min/src/client_session.rs:677`, `:684`
   - Rust helpers: `rust/mdt-client-min/src/client_session.rs:766`, `:782`
   - Java reference: `Call.ping(Time.millis())` in `core/src/mindustry/core/NetClient.java:710`; ping handlers at `:328`, `:333`

5. Java-style load-complete gating is now present in the Rust session path.
   - During active world-data load, Rust now mirrors Java's broad `clientLoaded` boundary more closely:
     - `StreamBegin` / `StreamChunk` keep flowing
     - normal-priority inbound remotes defer and replay after bootstrap/world-ready marks the session `client_loaded`
     - low-priority inbound packets (notably snapshot families) are dropped instead of being replayed
     - `worldDataBegin` clears the deferred queue
   - Rust evidence: `rust/mdt-client-min/src/client_session.rs`, `rust/mdt-client-min/src/arcnet_loop.rs`, `rust/mdt-client-min/src/session_state.rs`
   - Java reference: `core/src/mindustry/net/Net.java:270`, `:292`, `:298`; `core/src/mindustry/core/NetClient.java:631`, `:632`

6. Session gating for live interaction remains present (`ready_to_enter_world` + `connectConfirm`).
   - Rust gate check: `rust/mdt-client-min/src/client_session.rs`
   - Rust connectConfirm send: `rust/mdt-client-min/src/client_session.rs`
   - Java reference post-connect confirm: `core/src/mindustry/core/NetClient.java:632`

7. Default snapshot cadence now closely tracks Java baseline.
   - Java: `playerSyncTime = 4` ticks (`core/src/mindustry/core/NetClient.java:41`, `:687`)
   - Rust default: `client_snapshot_interval_ms = 67` (`rust/mdt-client-min/src/client_session.rs`)

8. Timeout floor now matches Java's high-level baseline more closely.
   - Java: `dataTimeout = 60 * 30` while connecting, `entitySnapshotTimeout = 20s` in-game (`core/src/mindustry/core/NetClient.java:38`, `:39`, `:591`, `:603`)
   - Rust default: `connect_timeout_ms = 1_800_000`, `timeout_ms = 20_000` with ready-state snapshot-stall tracking (`rust/mdt-client-min/src/client_session.rs`)

9. `stateSnapshot` now has minimal payload application plus failure telemetry in Rust.
   - Rust now parses/applies tracked `stateSnapshot` header fields, keeps the raw `coreData` bytes, projects `coreData` into a lightweight `team -> items` structure in session state, derives a business/runtime projection for `wave/enemies/paused/gameOver/timeData/tps/rand/core-inventory`, and also keeps a separate session-authoritative mirror that always applies header fields, preserves last-good core inventory across malformed `coreData`, tracks `gameOver > paused > playing` precedence, wave-advance-only-on-increase semantics, and core-parse-fail counts, and is preferred by runtime HUD snapshot labels. Packet-level and `coreData`-level parse failures are recorded separately (`rust/mdt-client-min/src/snapshot_ingest.rs`, `rust/mdt-client-min/src/session_state.rs`, `rust/mdt-client-min/src/render_runtime.rs`)
   - Rust now explicitly retains `last_good_state_snapshot_core_data` across malformed `coreData`, and runtime snapshot counters/labels can fall back to that last-good surface when current `coreData` parsing fails.
   - Rust test evidence: state snapshot apply + malformed-payload non-regression coverage (`rust/mdt-client-min/src/client_session.rs`, `rust/mdt-client-min/src/lib.rs`)

10. Ready-state timeout anchor now refreshes only on `EntitySnapshot`.
   - Java watchdog semantics key off entity snapshots (`core/src/mindustry/core/NetClient.java:487`, `:591`)
   - Rust now refreshes `last_snapshot_at_ms` on `EntitySnapshot` only, so `StateSnapshot` no longer extends ready-state snapshot timeout (`rust/mdt-client-min/src/client_session.rs`)

11. `blockSnapshot` / `hiddenSnapshot` now have minimal structured envelope parsing with parse-failure telemetry.
   - Rust now decodes/stores:
     - `blockSnapshot`: `amount:i16`, `data_len:u16`, plus first-entry fixed-prefix/base observability (`first_build_pos`, `first_block_id`, `first_health_bits`, `first_rotation`, `first_team_id`, `first_io_version`, `first_enabled`, `first_module_bitmask`, `first_time_scale_bits`, `first_time_scale_duration_bits`, `first_last_disabler_pos`, `first_legacy_consume_connected`, `first_efficiency`, `first_optional_efficiency`, `first_visible_flags`) when present, and a lightweight runtime head projection
     - `hiddenSnapshot`: `count:i32`, `first_id`, a bounded `sample_ids` summary, one-shot trigger-count observability for the current payload, plus latest-set replacement and real added/removed delta projection against the previous trigger set
   - Rust now tracks parse failures separately from simple seen/count telemetry.
   - Rust now also writes the parsed `blockSnapshot` fixed prefix/base and the authoritative `constructFinish` / `tileConfig` / `deconstructFinish` / `buildHealthUpdate` outcomes into a lightweight building table keyed by `build_pos` (`block_id/rotation/team/io_version/module_bitmask/time_scale_bits/time_scale_duration_bits/last_disabler_pos/legacy_consume_connected/enabled/config/health/efficiency/optional_efficiency/visible_flags/last_update`) so minimal building-state facts survive beyond one packet event.
   - When a loaded world bundle is available, Rust can now also parse/apply `blockSnapshot amount > 1` additional entries (beyond the first) by reusing loaded-world tile/block revision context, and upserts those entries into the same building-table projection.
   - That loaded-world extra-entry path is now fail-closed: if the full loaded-world parse does not complete cleanly, or if the reconstructed first entry no longer matches the already parsed head projection, Rust refuses to apply any of the additional entries instead of partially committing a prefix.
   - When a first-entry `blockSnapshot` head conflicts with an already tracked authoritative building row at the same `build_pos`, Rust now keeps the prior row intact and surfaces explicit conflict-skip telemetry instead of silently overwriting that authoritative state.
   - That additional-entry path is still a minimal compatibility approximation: it currently applies parsed base fields into the building table, not full Java `tile.build.readSync(...)` live-world semantics.
   - Current hard blocker for generic safe multi-entry `blockSnapshot` parity: the packet only gives `amount + byte[] data` and each entry is `pos + blockId + build.writeSync(...)` with no per-entry length marker, while Java-side `readSync(read, tile.build.version())` depends on a live local tile/build instance to know the correct revision/tail semantics. With the current Rust parser surface, generic full-stream entry walking without context is still unsafe.
   - `hiddenSnapshot` now not only toggles hidden flags on the minimal entity table, but also removes non-local hidden rows to better approximate Java's hidden-entity lifecycle without claiming full `readSyncEntity` parity.
   - Hidden IDs now also keep entity-snapshot tombstone blocking active, so non-local entities hidden by `hiddenSnapshot` are not re-upserted by near-following `entitySnapshot` player rows while the ID remains hidden.
   - Rust now also stores the current hidden trigger set (`hidden_snapshot_ids`) and a dedicated delta projection (`hidden_snapshot_delta_projection`) so downstream runtime/HUD can distinguish "latest set" from "change since previous payload".
   - Rust evidence: `rust/mdt-client-min/src/snapshot_ingest.rs`, `rust/mdt-client-min/src/session_state.rs`, `rust/mdt-client-min/src/lib.rs`

12. `entitySnapshot` is no longer only a side-effecting local-player sync path.
   - Rust now tracks `entitySnapshot` envelope observability (`amount`, `body_len`), parse-failure telemetry, and whether local-player sync was actually applied from the packet.
   - Rust now fail-closes parseable-player-row extraction when detected rows exceed the declared envelope `amount`, so overflow rows are not partially applied.
   - Rust now also writes that local player into a minimal entity table row (`entity_id -> class/local/unit/position/hidden`) when sync lands.
   - Rust now also batch-upserts any additional parseable `classId=12` player rows from the same snapshot into the entity table, while broader non-player class coverage is still partial rather than Java-complete.
   - Rust now also parses known-prefix alpha-shape rows from the same snapshot and upserts them into `entity_table`: landed support includes real parse/apply for `classId=0` (`alpha`), and the same revision family `classId=29/30/31/33` follows that same shape parser path.
    - Rust now also parses known-prefix mech-shape rows from that same snapshot prefix and upserts them into `entity_table`: landed support includes real parse/apply for `classId=4`, and the same revision family `classId=17/19/32` follows that same shape parser path.
    - Rust now also parses known-prefix missile-shape rows from that same snapshot prefix and upserts them into `entity_table`: landed support currently covers `classId=39` with explicit `lifetime/time` field parsing.
    - Rust now also parses known-prefix payload-shape rows from that same snapshot prefix and upserts them into `entity_table`: landed support covers `classId=5/23/26/36` for `payloadCount=0`, and when loaded-world `content_header` context exists it can now also boundary-consume recursive `BuildPayload` entries inside `payloadCount > 0`; `UnitPayload` recursion still remains fail-closed, and build payloads without block-name mapping still fail-closed.
    - Rust now also parses known-prefix environment rows from that same snapshot prefix and upserts them into `entity_table`: landed support currently covers `Fire classId=10`, `Puddle classId=13`, `WeatherState classId=14`, and `WorldLabel classId=35`.
    - Rust now also keeps a short-lived tombstone guard for recently removed entity IDs so stale immediate-following `entitySnapshot` player rows do not instantly recreate entities just removed by `unitDespawn` / `unitEnteredPayload` / `playerDisconnect` / hidden-lifecycle removal.
    - Rust evidence: `rust/mdt-client-min/src/client_session.rs`, `rust/mdt-client-min/src/session_state.rs`

13. Runtime observability now reacts to more than just local-player movement.
- The online runtime now treats `StateSnapshot` / `BlockSnapshot` / `HiddenSnapshot` as refresh signals and surfaces snapshot summaries plus parse-fail counters in HUD/status text.
- `blockSnapshot` HUD text now also includes the first observed build position/block id together with fixed-prefix rotation/team/version/enabled/efficiency facts when present, the runtime scene includes a lightweight block-head marker, the HUD now also reports a compact building-table summary (`runtime_buildings=...`), inbound authoritative `tileConfig` now renders explicit runtime config / config-rollback markers, and `hiddenSnapshot` HUD text now includes both a bounded ID sample and current-trigger summaries for faster diagnosis. `hiddenSnapshot` now also minimally applies hidden flags into that local entity table instead of remaining statistics-only, and the HUD now surfaces explicit hidden delta labels (`runtime_hidden_delta=...`) in addition to the current hidden trigger sample (`runtime_hidden=...`).
- Runtime HUD now also surfaces compact loading/snapshot gate counters (`runtime_loading`) together with compact audio/admin counters (`runtime_audio`, `runtime_admin`) so deferred/replayed inbound packets, dropped loading-time low-priority packets, state/entity snapshot parse failures, `sound` / `soundAt`, `traceInfo`, and `debugStatusClient*` are visible without deep logs.
- Runtime HUD now also surfaces minimal inbound interaction counters for `takeItems` / `transferItemTo` / `transferItemToUnit` / `payloadDropped` / `pickedBuildPayload` / `pickedUnitPayload` / `unitDespawn` (`runtime_take_items`, `runtime_transfer_item`, `runtime_transfer_item_unit`, `runtime_payload_drop`, `runtime_payload_pick_build`, `runtime_payload_pick_unit`, `runtime_unit_despawn`).
  - Rust evidence: `rust/mdt-client-min/src/render_runtime.rs`, `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`

14. World-bootstrap observability now surfaces a compact runtime projection.
   - Rust now stores bootstrap-side `rules/tags/locales` hashes plus `team/marker/custom-chunk/content-patch` and plan/fog-team counts in session state, and the runtime HUD reports that projection without claiming Java `NetworkIO.loadWorld` parity.
   - Rust evidence: `rust/mdt-client-min/src/session_state.rs`, `rust/mdt-client-min/src/bootstrap_flow.rs`, `rust/mdt-client-min/src/render_runtime.rs`

## Remaining Differences

### Release-Critical

1. Timeout semantics are closer now, but not fully Java-equivalent.
   - Rust now mirrors the 30min connect/load floor and 20s in-game snapshot-stall floor, and ready-state snapshot timeout refresh is tied to `EntitySnapshot`.
   - Remaining difference: Rust still uses a simplified local timing/watchdog model vs Java's exact `lastSnapshotTimestamp` / `timeoutTime` split and broader game-state coupling.
   - Risk: edge-case disconnect timing can still differ in unusual lifecycle transitions.

2. Server snapshot families are mostly tracked, not fully applied.
   - Java applies state/entity/block/hidden snapshots in dedicated handlers (`core/src/mindustry/core/NetClient.java:485`, `:502`, `:513`, `:539`)
   - Rust now minimally applies `stateSnapshot` header fields, projects `coreData` into a lightweight `team -> items` structure, derives a business/runtime state projection, keeps a separate session-authoritative mirror for header + last-good core inventory state, applies local player sync from `entitySnapshot`, batch-upserts additional parseable `classId=12` player rows into the same minimal entity table, additionally upserts parseable alpha-shape rows in the same `entitySnapshot` prefix path (`classId=0` landed plus same-shape revision family `classId=29/30/31/33`), additionally upserts parseable mech-shape rows in that same prefix path (`classId=4` landed plus same-shape revision family `classId=17/19/32`), additionally upserts parseable missile-shape rows in that same prefix path (`classId=39` landed), additionally upserts parseable payload-shape rows in that same prefix path (`classId=5/23/26/36` landed for `payloadCount=0`, plus loaded-world-context `BuildPayload` recursion for `payloadCount > 0`), additionally upserts parseable environment rows in that same prefix path (`classId=10/13/14/35` landed), minimally applies `hiddenSnapshot` hidden flags into that table as a one-shot trigger while also tracking latest hidden-id set + real added/removed delta projection, records parse/error telemetry for `block` / `hidden` snapshot envelopes, stores first-entry `blockSnapshot` fixed-prefix/base building facts, and (when loaded-world context exists) also applies additional parseable `blockSnapshot` entries into the authoritative building table (`rust/mdt-client-min/src/snapshot_ingest.rs`, `rust/mdt-client-min/src/client_session.rs`, `rust/mdt-client-min/src/session_state.rs`, `rust/mdt-client-min/src/render_runtime.rs`)
   - `worldDataBegin` now also clears deferred loading packets, snapshot business + authority projections, the entity table, the builder queue projection, the building table, `tile_config_projection`, and lightweight rules/objectives projections so a new world load does not inherit the previous world's authoritative state surface.
   - Remaining difference: `entity` / `block` / `hidden` full-world application depth and full Java-equivalent live-world/system application remain incomplete; the new loaded-world `blockSnapshot` fail-closed path and short-lived `entitySnapshot` tombstone guard only reduce obvious stale-revival / partial-apply hazards, they do not close the Java parity gap.
   - Risk: long-session world-state convergence and behavior fidelity remain incomplete.

### Parity Backlog

1. Build-plan config encoding is broader now, but full Java behavior parity is still incomplete.
   - Rust now emits a much wider `TypeIO` subset for `BuildPlan.config` (`Int` / `Long` / `Float` / `Bool` / `IntSeq` / `Point2` / `Point2[]` / `TechNodeRaw` / `Double` / `BuildingPos` / `LAccess` / `String` / `byte[]` / `LegacyUnitCommandNull` / `boolean[]` / `UnitId` / `Vec2` / `Vec2[]` / `Team` / `int[]` / `object[]` / raw `Content` / `UnitCommand`, plus `None`).
   - Impact: the remaining gap is now richer Java-equivalent business semantics and end-to-end runtime behavior, not the old minimal-type baseline.

2. Many server remote packets still land as `IgnoredPacket`.
   - Evidence: ignored metadata/event plumbing in `rust/mdt-client-min/src/client_session.rs:1011`, `:1022`, `:2942`
   - Impact: acceptable for minimal release chain; not full Java parity. `ping` / `pingResponse` are no longer part of this ignored subset because they now emit dedicated session events.

## Classification Summary

- `release-critical` (M7): deeper snapshot-application parity gap (`entity` / `block` / `hidden` breadth + detailed `coreData` semantics).
- `parity backlog`: exact timeout semantics edge cases, extended build config coverage, broader remote packet behavior coverage.

## Next Minimal Slices

1. Bootstrap/runtime projection visibility beyond bare stream-ready fields.
- Keep the current observational boundary.
- Surface already-parsed bootstrap facts (`rules/tags/locales` hashes and team/marker/custom-chunk/content-patch counts) in runtime/session visibility without mutating live world systems.

2. Keep the current timeout semantics stable while landing more observability slices.
- `EntitySnapshot` remains the only ready-state timeout refresh source, and connect-confirm now arms the ready-state snapshot timeout anchor.
- Do not couple these observational/runtime-visibility slices with lifecycle/redirect rewrites or full snapshot world apply.

3. After observability slices, move to one serial owner for deeper snapshot/lifecycle application.
- Candidate follow-up: `entitySnapshot` payload-family recursive `UnitPayload` bodies when `payloadCount > 0` (full `unit.read(...)` consume parser, not just `readSync`), plus stronger fail-closed diagnostics for unknown build-payload block mappings, then fuller snapshot apply depth and deeper Java-equivalent world-state application on top of the now-landed load gate.

## Release Scope Statement

Current evidence supports: **minimal compatibility client release chain**.  
Current evidence does not support: **full Java desktop networking/session parity**.
