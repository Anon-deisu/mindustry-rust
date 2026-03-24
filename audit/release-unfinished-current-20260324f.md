# Release Unfinished Current (2026-03-24f)

Purpose: give later subagents a short, current, conflict-aware list of what is still unfinished after the latest M6-M9 parity slices landed.

## Do Not Re-Do

These are already landed and should not be re-opened as if missing:

- `stateSnapshot` strict wave-increase live signal is landed.
  - session state records `received_wave_advance_signal_count`, `last_wave_advance_signal_from/to`, and `last_wave_advance_signal_apply_count`
  - equal/regressed waves do not re-trigger
  - `worldDataBegin` clears the signal
  - runtime HUD surfaces it in `runtime_gameplay_signal=...`
- current-vanilla `entitySnapshot` `Syncc` family coverage is already guarded against generated Java `Syncc` ids.
  - old `7/8/9/11/15/28/42` “missing entitySnapshot families” are false positives for current vanilla `Groups.sync`
- loaded-world `blockSnapshot` parser-to-business fold already covers more than the older audit text said.
  - existing configured/resource projection folds already include:
    - `message` / `reinforced-message` / `world-message`
    - `payload-router` / `reinforced-payload-router`
    - `payload-source`
    - `duct-unloader`
    - `reconstructor`
    - `canvas` / `large-canvas`
    - `payload-mass-driver` / `large-payload-mass-driver`
    - `sorter` / `inverted-sorter` / `unloader` / `duct-router`
    - `bridge-conveyor` / `phase-conveyor`
    - `illuminator`
    - `switch` / `world-switch`
- `hiddenSnapshot` lifecycle delete is already narrowed to known runtime-owned non-local semantics instead of deleting every hidden entity row.
  - current cleanup now covers non-local `Unit` / `Fire` / `Puddle` / `WeatherState`
  - `WorldLabel` is still intentionally preserved as a conservative boundary
- `hiddenSnapshot` latest-set sync now also clears stale entity hidden flags when ids leave the hidden set.
  - `EntityTableProjection::apply_hidden_ids(...)` mirrors the newest hidden-id set instead of only setting `hidden=true`
  - remaining hidden work is deeper runtime semantics, not re-fixing stale hidden flags on surviving rows
- `effect(..., data)` runtime overlay already consumes the `float_length` contract for ray-endpoint projection.
- `tileConfig` authority reconcile is no longer a single-value last-write-only pending model.
  - per-building local intents now keep FIFO request order
  - authoritative `tileConfig` / `constructFinish` / parse-fail fallback only resolve the oldest pending request
  - later local config intents on the same building are preserved instead of being cleared by the first authoritative response
- inbound custom/logic packet typed registry glue is already landed.
  - `mdt-remote` now exposes `payload_kind()` for inbound custom-channel families
  - `mdt-client-min` now has typed inbound dispatch specs and `typed_remote_dispatch.rs` helper coverage
  - remaining work is live session/business adoption, not re-adding the typed metadata layer
- `mdt-remote` typed generation is no longer limited to the high-frequency subset.
  - generated typed registry/dispatch coverage now also includes custom-channel and inbound dispatch families
  - remaining M6 work is deeper live session/business adoption, not re-adding the first broader typed generation layer
- minimal command-mode state container is already landed.
  - `mdt-input` now carries `CommandModeState` / `CommandModeProjection` with selected-units, command-buildings, command-rect, control-groups, and last target/command/stance selections
  - `mdt-client-min-online` runtime outbound action sync now updates that container instead of keeping command-mode as packet-observability-only state
  - CLI/runtime seed controls for bind/recall/clear-group and rect are also landed, including replay after world reload/reconnect clears
  - remaining work is richer live input binding and command/build UI flow, not re-adding the state container baseline
- `mdt-world` post-load activation preflight is already landed.
  - `SavePostLoadActivationSurface` exposes loadable/skipped entity candidates, unresolved remap names, building-center reference validity, and `can_seed_runtime_apply()`
  - remaining work is consuming that surface for Java-like live world/entity activation
- `mdt-world` now also has a deterministic `SavePostLoadRuntimeSeedPlan` layer above that preflight.
  - `.msav -> post_load_world() -> projection_contract() -> activation_surface()` is now folded into a passive seed plan for later runtime/apply consumers
  - remaining M7-3 work is consuming that seed plan in deeper runtime/world ownership, not re-adding the first passive plan layer
- `mdt-world` consumer-side post-load apply plan helper is now also landed.
  - `SavePostLoadConsumerApplyPlan::consumer_apply_plan()` now turns the stricter contract/activation/seed surfaces into a deterministic consumer-stage plan with explicit blocker reasons (`contract issue`, duplicate `entity_id`, invalid building-center refs, skipped entity)
  - remaining `M7-3` work is still wiring that passive plan into real runtime/world ownership, not re-adding this consumer-side helper layer
- `mdt-typeio` raw `WeaponMount[]` codec is already landed.
  - remaining non-object codec gap is now more about `abilities/status` and wider unit-sync families than mounts specifically
- `mdt-render-ui` runtime dialog summary is already landed.
  - prompt priority: `text input > follow-up menu > menu`
  - notice priority: `warning toast > info toast > reliable hud > hud`
  - remaining gap is richer chat/dialog UI interaction, not re-adding a first dialog summary layer
- `entitySnapshot` typed `WorldLabel` rows are no longer packet-counter-only.
  - runtime/HUD now consumes active label count plus latest `entity_id/text/flags/font_size/z/position`
  - remaining work is broader render/UI depth, not re-adding the first runtime-apply bridge
- `world-label` presentation depth is already wider than the first runtime apply bridge.
  - panel/presenter output now also includes inactive count, text length, line count, and `font` / `z` bits plus decoded `f32`
  - remaining work is broader render/UI parity, not re-adding those derived world-label fields
- `typed_runtime_entities()` baseline join helper is already landed for existing parseable entity rows.
  - `SessionState` now exposes read-only typed runtime joins for `Player` and `Unit`
  - remaining `U1` work is consumer-side runtime apply depth, not re-adding a first typed join surface
- `typed_runtime_entity_projection()` is also landed as the first aggregate runtime model over those typed joins.
  - `SessionState` now exposes typed player/unit counts, hidden count, local-player id, and latest player/unit/entity ids
  - remaining `U1` work is deeper runtime ownership/apply, not re-adding the first typed summary/projection layer
- `runtime-owned` typed entity apply state is now landed as a separate persistent layer.
  - `SessionState` now keeps `runtime_typed_entity_apply_projection`, and `client_session` drives it from bootstrap local-player seed, `entitySnapshot` player/unit applies, hidden-snapshot rebuilds, despawn/disconnect removals, and `worldDataBegin` clear
  - runtime HUD live-entity observability now prefers that persistent apply layer instead of only rebuilding typed player/unit joins on demand from the raw projection tables
  - remaining `U1` work is deeper live ownership/group semantics, not re-adding the first persistent typed runtime apply layer
- runtime live-entity HUD/presenter output now also consumes that typed projection layer.
  - live entity observability/panels now surface typed player/unit counts plus latest typed entity/player/unit ids
  - remaining M9/U1 work is deeper runtime/apply behavior and richer UI depth, not re-adding the first typed live-entity aggregate view
- `mdt-input` batch runtime intent sampling is already landed.
  - same-tick multi-snapshot batches now preserve transient press/release edges instead of only keeping the final frame
  - remaining work is richer live input source parity, not re-adding batch edge retention
- online/runtime live intent sampling is now the default path.
  - `mdt-client-min-online` now defaults to `RuntimeIntentTracker + IntentSamplingMode::LiveSampling`
  - `--intent-snapshot` now also carries the `building` bit explicitly
  - remaining M8 work is richer live input source parity and deeper command/build flow, not re-landing default live sampling
- builder queue tile-state validation is already landed in `mdt-input`.
  - local queued place/break entries can now be pruned against observed tile states when the tile is already air or already matches the requested block/rotation
  - remaining work is broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding the validation primitive
- builder queue tile-state validation now also supports explicit rotation-irrelevant observations.
  - `BuilderQueueTileStateObservation.requires_rotation_match` can now preserve or clear local place plans based on whether the observed tile family actually requires rotation equality
  - remaining work is still broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding this validation refinement
- builder queue local activity/reconcile state-machine semantics are now richer.
  - `update_local_activity()` now reports explicit head-selection outcomes (`HeadInRange`, reorder/fallback/skip/out-of-range/missing cases), and `validate_against_tile_states()` now reports whether reconcile left the queue unchanged, removed a non-head entry, advanced the head, or cleared the queue
  - remaining work is still broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding this pure state-machine selection/reconcile slice
- building-table block identity carry-through is already landed.
  - `BuildingProjection` / `BuildingTableProjection` now include `block_name` and `last_block_name`
  - world baseline, entity building rows, loaded-world extra entry, `constructFinish`, and `deconstructFinish` already wire `block_name` into the building table
  - `render_runtime` build inspector now prefers the typed runtime view sourced from building table + `configured_block_projection`
  - remaining work is deeper live building ownership/runtime parity, not re-adding this field plumbing or inspector bridge
- `mdt-world` post-load contract validation now cross-checks actual entity chunks against the summary.
  - `SavePostLoadWorldObservation::projection_contract()` no longer accepts only `loadable + skipped == total`; it now re-derives the effective post-load entity summary from `world_entity_chunks` and rejects summary drift
  - remaining `M7-3` work is deeper consumer-side runtime/world ownership, not re-adding this stricter passive contract check
- typed high-frequency snapshot registry glue is now landed.
  - `mdt-remote` now exposes `HighFrequencyRemoteRegistry`, `mdt-client-min` snapshot packet registry now consumes typed glue via `snapshot_registry_glue.rs`, and inbound-family registry construction no longer depends on unrelated outbound custom-channel families
  - remaining `M6-1` work is broader typed registry consumption outside the first snapshot/inbound glue path, not re-adding this typed snapshot registry layer
- typed inbound remote dispatch fixed-table glue is now also landed.
  - `mdt-remote::typed_inbound_remote_dispatch_specs(...)` now exposes a typed non-snapshot inbound dispatch table, and `mdt-client-min` packet registry consumes it through `inbound_remote_registry_glue.rs` instead of rebuilding that lookup only from string/manifest scans
  - remaining `M6-1` work is broader typed registry/session adoption beyond this fixed-table inbound dispatch slice, not re-adding the first fixed-table glue layer
- `mdt-render-ui` minimap/overlay semantic detail breakdown is now landed.
  - render/model panel presenters now expose deterministic family+detail counts for minimap and overlay summaries instead of only coarse kind buckets
  - remaining `M9` work is still deeper renderer pipeline and interactive UI flow, not re-adding this detail-breakdown presentation slice
- `mdt-render-ui` presenter-local HUD/chat/menu/dialog/minimap detail rows are now landed.
  - panel/window/ascii presenters now expose `HUD-DETAIL`, `MINIMAP-*DETAIL`, `RUNTIME-MENU-DETAIL`, `RUNTIME-DIALOG-DETAIL`, and `RUNTIME-CHAT-DETAIL` rows derived from existing runtime observability instead of only coarse summary rows
  - remaining `M9` work is still interactive UI/user-flow depth, not re-adding this presenter-local detail slice
- typed building runtime apply state is now landed as a separate persistent layer.
  - `SessionState` now keeps `runtime_typed_building_apply_projection` with fallback to the computed typed join when tests/setup mutate only raw tables
  - typed building models now carry already parsed base/head/turret fields (`rotation/team/io_version/module/time-scale/health/enabled/efficiency/visible_flags/build-turret summary`) in addition to the configured domain value
  - `client_session` now refreshes that layer from loaded-world tail/business folds, authoritative `constructFinish` / `tileConfig` / `buildHealthUpdate`, `deconstructFinish` / `removeTile`, and `worldDataBegin` clear, and `render_runtime` build inspector now consumes that runtime-owned projection instead of rebuilding only from the raw table join at the callsite
  - remaining `U3` work is still broader family depth and true Java-like `tile.build.readSync(..., version)` runtime ownership, not re-adding this first persistent typed building apply layer

## Highest-Confidence Remaining Lanes

### U1 `entitySnapshot` typed runtime apply

Remaining gap:
- Rust still writes parsed rows into lightweight projection tables instead of doing Java-like `readSyncEntity -> readSync -> snapSync -> add`.
- a read-only typed runtime join helper plus a first aggregate typed runtime projection for existing `Player` / `Unit` rows are already present; the remaining gap is applying those rows into a stronger runtime ownership model.

Best bounded next slice:
- start with a typed runtime apply layer for the already parseable `Player` / `Unit` families
- keep it below full Java group ownership; do not combine with lifecycle/load-gate rewrites in the same lane

Write scope:
- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/session_state.rs`
- optionally a new helper module under `rust/mdt-client-min/src/`

### U2 `hiddenSnapshot` deeper hidden/runtime semantics

Remaining gap:
- Rust has latest-trigger/delta tracking, hidden blocking, and conservative cleanup for known runtime-owned non-local `Unit` / `Fire` / `Puddle` / `WeatherState`, but still not Java-equivalent `handleSyncHidden()` depth.

Best bounded next slice:
- improve hidden apply semantics without touching `worldDataBegin`, reconnect, or packet defer/replay
- prefer `snapshot_ingest.rs` + `session_state.rs` helper-layer work over broad `ClientSession` changes

Write scope:
- `rust/mdt-client-min/src/snapshot_ingest.rs`
- `rust/mdt-client-min/src/session_state.rs`

### U3 `blockSnapshot` typed building runtime model

Remaining gap:
- low-risk loaded-world tail/base folds, `block_name` carry-through, a persistent typed building apply layer, and the first runtime build-inspector consumer are landed, but Rust still does not have Java-like `tile.build.readSync(..., version)` runtime ownership or broad per-family live building semantics.

Best bounded next slice:
- extend the typed building runtime model one low-risk family at a time above the already landed persistent apply layer
- do not spend time re-landing already wired configured/resource folds, `block_name` / `last_block_name`, the persistent apply layer itself, or the current build-inspector runtime consumer

Write scope:
- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/session_state.rs`
- `rust/mdt-world/src/lib.rs` only if a new parsed field is strictly required

### U4 `tileConfig` request/response rejection loop

Remaining gap:
- Rust now has per-building FIFO request reconciliation, but still stops short of full Java configure lifecycle depth.
- remaining gap is broader server-authoritative configure semantics, canonicalization/business execution depth, and UI/runtime follow-through beyond the narrowed request queue loop.

Best bounded next slice:
- extend the now-explicit request queue loop into richer authoritative configure semantics
- avoid mixing with snapshot apply work in the same edit

Write scope:
- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/session_state.rs`

### U5 `effect` executor / contract table depth

Remaining gap:
- Rust has bounded runtime overlays and several contract-aware projections, but still not Java `Effect`-executor semantics.

Best bounded next slice:
- add one narrow `effect_id -> contract/executor` family at a time
- stay above raw packet decode and below full renderer parity

Write scope:
- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/render_runtime.rs`

### U6 `finishConnecting` / `clientLoaded` lifecycle parity

Remaining gap:
- `mark_client_loaded()` now fail-closes deferred replay and auto-queues `connectConfirm` once the world becomes ready, and the resulting ready-state action ordering has been regression-revalidated across the full current `mdt-client-min` suite.
- the remaining gap is narrower: deeper Java-equivalent transport/lifecycle atomicity across `finishConnecting`, replay side effects, reconnect edges, split-driver transport coordination, and higher-layer UI/runtime assumptions about when the queued `connectConfirm` is actually flushed.

Best bounded next slice:
- keep this serial-only and do not mix with snapshot/entity/world ownership work

Write scope:
- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/bootstrap_flow.rs`

## Conflict Notes

- Do not assign more than one worker at a time to `client_session.rs` unless their write sets are clearly disjoint and pre-reviewed.
- Treat `worldDataBegin`, reconnect, deferred replay, and `clientLoaded` as serial-owner areas.
- If a worker proposes work on a slice listed under `Do Not Re-Do`, redirect it to one of `U1`..`U6` instead.
