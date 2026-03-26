# Runtime Semantic Gap Backlog

## Scope

This file tracks the highest-confidence remaining semantic parity gaps after the remote-family coverage pass moved from `missing packets` into `partial runtime semantics`.

## Snapshot Runtime Apply

### S1 `resolved` `stateSnapshot` now has a dedicated runtime apply chain
- Rust location: `rust/mdt-client-min/src/snapshot_ingest.rs`
- Rust location: `rust/mdt-client-min/src/session_state.rs`
- Rust location: `rust/mdt-client-min/src/client_session.rs`
- Rust location: `rust/mdt-client-min/src/render_runtime.rs`
- Java reference: `core/src/mindustry/core/NetClient.java`
- Current state:
  - Rust now applies `StateSnapshot` into a dedicated runtime-facing `authoritative_state_mirror` instead of aliasing authority projection data directly.
  - That runtime apply path now covers wave state, pause/game-over precedence, net-seconds, random seeds, and core inventory mutation/retention across malformed `coreData`.
  - `snapshot_ingest` still preserves the separate `authority_projection` and `business_projection`, but runtime consumers can now read a true applied-state container instead of inferring runtime from projection-only data.
  - Parse-success `StateSnapshot` packets now also emit a dedicated `StateSnapshotApplied` event, `--print-client-packets` exports that projection directly, and runtime HUD keeps a compact `runtime_snap_apply=...` label for explicit apply semantics.
  - Rust now also records a dedicated wave-advance live signal only on strict `wave` increases, clears it on `worldDataBegin`, and surfaces it in `runtime_gameplay_signal=...` so the Java-side `WaveEvent` edge is no longer hidden inside aggregate snapshot counters.

### S2 `entitySnapshot` is still a parseable-row projection, not `readSyncEntity` parity
- Rust location: `rust/mdt-client-min/src/client_session.rs`
- Rust location: `rust/mdt-client-min/src/session_state.rs`
- Java reference: `core/src/mindustry/core/NetClient.java`
- Current state:
  - Rust writes parseable rows into `EntityTableProjection`.
  - Rust does not yet create typed runtime entities, execute `readSync`, call `snapSync`, and attach entities into live groups.
- Next landing direction:
  - Add an entity runtime apply layer, starting with `Player` and `Unit`.
  - Drive real runtime entity state from decoded row/protocol data instead of only mirroring into a projection table.

### S3 `blockSnapshot` only lands head/base facts, not full building-tail runtime semantics
- Rust location: `rust/mdt-client-min/src/client_session.rs`
- Rust location: `rust/mdt-client-min/src/session_state.rs`
- Java reference: `core/src/mindustry/core/NetClient.java`
- Current state:
  - Rust now carries one minimal tail semantic slice for `BuildTurret`: `rotation_bits + plans_present + plan_count` is parsed from loaded-world `blockSnapshot` entries and mirrored into `BuildingTableProjection`.
  - Loaded-world parsing now also recognizes `message` / `reinforced-message` / `world-message` building tails and can recover structured message text for that family instead of leaving those tails fully opaque bytes in `mdt-world`.
  - Loaded-world parsing now also recognizes `payload-router` / `reinforced-payload-router` building tails and can recover bounded structured tail data for that family (`sorted` mixed content ref, `recDir`, and carried-payload kind/length/hash summary) instead of leaving those tails fully opaque bytes in `mdt-world`.
  - Runtime HUD observability now exposes the mirrored `BuildTurret` tail rotation bits via `runtime_buildings ... :trb0x????????`.
  - Rust still does not do Java-style `tile.build.readSync(..., version)` semantic application for the broader building/module/tail surface; the newly landed message-family and payload-router-family tail parsing should still be read as parser coverage, not full runtime building behavior parity.
- Next landing direction:
  - Connect parsed tail/module data into a typed building runtime model.
  - Merge head and tail application into one atomic update keyed by block/revision.

## Effect Runtime Semantics

### E1 `effect` is still decoded/observed, not executed as a runtime effect instance
- Rust location: `rust/mdt-client-min/src/client_session.rs`
- Rust location: `rust/mdt-client-min/src/render_runtime.rs`
- Java reference: `core/src/mindustry/core/NetClient.java`
- Java reference: `core/src/mindustry/entities/Effect.java`
- Current state:
  - Rust updates `last_effect_*`, emits `EffectRequested`, and now pushes bounded runtime effect instances with a minimal TTL/expiry loop instead of one-shot static markers.
  - Rust still does not execute Java-style `effect.at(..., data)` renderer/entity behavior keyed by the real `Effect` implementation.
- Next landing direction:
  - Add an `effect executor` keyed by `effect_id`.
  - Route both `effect` and `effectReliable` into runtime effect instances instead of overlay-only observability.

### E2 `effect(..., data)` still uses lightweight cross-effect projection instead of effect-specific contracts
- Rust location: `rust/mdt-client-min/src/client_session.rs`
- Java reference: `core/src/mindustry/io/TypeIO.java`
- Current state:
  - Rust extracts generic `ContentRef`, `ParentRef`, `WorldPosition`, and `FloatValue`.
  - Rust uses bounded DFS heuristics instead of effect-specific data interpretation.
- Next landing direction:
  - Define `effect_id -> data contract` mappings for the highest-signal effects first.
  - Upgrade from generic projection to effect-specific `TypeIoObject` consumption.

### E3 Parent-follow and effect-instance parity semantics remain partial
- Rust location: `rust/mdt-client-min/src/render_runtime.rs`
- Rust location: `rust/mdt-client-min/src/effect_runtime.rs`
- Rust location: `rust/mdt-client-min/src/client_session.rs`
- Java reference: `core/src/mindustry/entities/Effect.java`
- Current state:
  - Rust now stores short-lived runtime effect overlays with minimal position rebinding for `ParentRef` / `BuildingPos` / `Point2` / `Point2[]` / `Vec2`-style payloads.
  - Rust now also carries per-overlay `lifetime_ticks` and effect-shaped TTL seeding for the currently landed runtime families.
  - Rust `unit_parent` overlays now lazily freeze a relative offset on the first authoritative entity-table hit instead of snapping every frame to the parent origin, and unresolved parent-unit payloads with a position hint now fall back to that hint instead of discarding it.
  - Rust `effect_id=8/9` now also carry a narrow `source_binding` path, so `unitSpirit` / `itemTransfer` can move the spawned source point with a parent `Unit` instead of freezing it at the original world source.
  - Rust now also has first-pass `rotWithParent`, `startDelay`, `clip`, and `effect_id=263 -> legDestroy` coverage, but still does not model Java-equivalent wider `position_target` source-follow beyond `8/9`, general building-parent offset follow, binding/fallback observability, or stable effect-instance seed parity.
- Next landing direction:
  - Extend runtime effect instances so the remaining `position_target` families that should follow a parent can move the spawned source point with the parent instead of only rebinding the target marker.
  - Add clearer binding/fallback outcome observability plus stable effect-instance seed support, then deepen source-follow and parent-offset behavior beyond the already landed `rotWithParent` / `startDelay` / `clip` baseline.

## Tile Config Runtime Semantics

### T1 Rejected `tileConfig` does not yet drive a full forced rollback loop
- Rust location: `rust/mdt-client-min/src/client_session.rs`
- Java reference: `core/src/mindustry/input/InputHandler.java`
- Current state:
  - Rust queues outbound config, tracks pending local intent, and now routes authoritative `tileConfig` plus `constructFinish` config application through one shared authority/reconcile path.
  - That path now carries `source`, authoritative value, replaced local value, rollback flag, and deterministic pending-intent clearing.
  - Rust still does not model the full Java server-side rejection lifecycle as an explicit request/response business loop.
- Next landing direction:
  - Add request/response reconciliation for `tileConfig`.
  - On rejection, force authoritative overwrite and clear pending local intent deterministically.

### T2 Successful `tileConfig` is still a projection update, not a configured business-execution chain
- Rust location: `rust/mdt-client-min/src/session_state.rs`
- Java reference: `core/src/mindustry/entities/comp/BuildingComp.java`
- Java reference: `core/src/mindustry/input/InputHandler.java`
- Current state:
  - Rust updates `building_table_projection` and `tile_config_projection`, and `constructFinish` now also feeds the same authoritative config apply entrance instead of bypassing it.
  - Rust now has a first minimal `configured(...)` business layer for a low-risk block batch:
    - `unit-cargo-unload-point`
    - `landing-pad`
    - `item-source`
    - `liquid-source`
    - `sorter`
    - `inverted-sorter`
    - `bridge-conveyor`
    - `phase-conveyor`
    - `switch`
    - `world-switch`
    - `door`
    - `door-large`
    - `message`
    - `reinforced-message`
    - `world-message`
    - `canvas`
    - `large-canvas`
    - `constructor`
    - `large-constructor`
    - `illuminator`
    - `payload-source`
    - `payload-router`
    - `reinforced-payload-router`
    - `unloader`
    - `duct-unloader`
    - `duct-router`
    - `mass-driver`
    - `payload-mass-driver`
    - `large-payload-mass-driver`
    - `power-node`
    - `power-node-large`
    - `surge-tower`
    - `beam-link`
    - `additive-reconstructor`
    - `multiplicative-reconstructor`
    - `exponential-reconstructor`
    - `tetrative-reconstructor`
  - The typed runtime building view above that configured mirror is also no longer limited to the oldest shell subset: low-risk runtime shells now cover the liquid family (`liquid-source`, `liquid-router`, `liquid-junction`, `liquid-container`, `liquid-tank`), the processor family (`micro-processor`, `logic-processor`, `hyper-processor`), `message` family empty-string fallback shells, and reconstructor fallback shells that preserve explicit `None` command/runtime state.
  - Those paths currently cover `ContentRaw(Item/Liquid)`, `Bool` (for `switch`/`world-switch` and `door` family), normalized `String` text for `message` / `reinforced-message` / `world-message`, strict fixed-length `byte[]` canvas payloads for `canvas` / `large-canvas`, `ContentRaw(Block)` for `constructor` / `large-constructor` recipe selection, `Int` color for `illuminator`, mixed `ContentRaw(Block/UnitType)` for `payload-source` / `payload-router` family, bounded `UnitCommand | clear` for the reconstructor family, a limited single-link slice for `bridge-conveyor` / `phase-conveyor` / `mass-driver` / `payload-mass-driver` / `large-payload-mass-driver`, and bounded `PowerNode`-family link-set projection where authoritative `Point2[]` becomes full-replace while absolute `Int` / `BuildingPos` remains single-edge toggle.
  - The currently landed canvas family accepts only exact-length `Bytes` payloads (`54` for `canvas`, `288` for `large-canvas`); `Null`, non-`Bytes`, and wrong-length `Bytes` all reject without clearing previous state, matching Java's handler/no-op shape rather than the broader `Null(clear)` path used by some other configured blocks.
  - The currently landed link handling accepts `Null(clear)`, relative `Point2`, packed absolute `Int`, and `BuildingPos`, then applies the resolved link into `ConfiguredBlockProjection`; broader typed config dispatch and Java-equivalent configured side effects are still missing.
  - The currently landed `PowerNode` family accepts only authoritative `PackedPoint2Array` full-replace and absolute `Int` / `BuildingPos` toggle updates; `Null` and relative `Point2` still reject intentionally because Java does not use them as the family clear path.
  - `ConfiguredBlockProjection` now keeps bounded domain projections for these newer families as well (`message_text_by_build_pos`, `canvas_bytes_by_build_pos`, `constructor_recipe_block_by_build_pos`, `light_color_by_build_pos`, payload mixed-content refs, power-node link sets, and reconstructor command refs), but that remains a lightweight authoritative/configured mirror rather than full Java block-execution parity.
  - Rust now also records domain-shaped configured outcomes on that authority/apply chain instead of leaving configured execution implicit inside counters only. The current landed outcome slice is:
    - `Applied`
    - `RejectedMissingBuilding`
    - `RejectedMissingBlockMetadata`
    - `RejectedUnsupportedBlock`
    - `RejectedUnsupportedConfigType`
  - Those outcomes now flow through `TileConfigProjection`, `ClientSessionEvent::TileConfig`, `--print-client-packets`, and runtime HUD `runtime_tilecfg_apply=...`, but they still describe only the current minimal configured-block batch, including the newly landed link-based subset above, rather than full Java `configured(...)` behavior.
- Next landing direction:
  - Add a configuration business layer keyed by block and `TypeIO` config type.
  - Extend the current outcome slice from low-risk configured blocks into broader block/type coverage and richer domain-specific results.

### T3 `resolved` Parse-fail `tileConfig` now clears pending local intent
- Rust location: `rust/mdt-client-min/src/client_session.rs`
- Rust location: `rust/mdt-client-min/src/session_state.rs`
- Current state:
  - On parse failure Rust now clears the matching pending local intent instead of leaving it hanging.
  - Remaining gap has moved back to rejection semantics and block-specific configured execution, not parse-fail cleanup.
