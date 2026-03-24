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
- `effect(..., data)` runtime overlay already consumes the `float_length` contract for ray-endpoint projection.
- `tileConfig` authority reconcile is no longer a single-value last-write-only pending model.
  - per-building local intents now keep FIFO request order
  - authoritative `tileConfig` / `constructFinish` / parse-fail fallback only resolve the oldest pending request
  - later local config intents on the same building are preserved instead of being cleared by the first authoritative response
- inbound custom/logic packet typed registry glue is already landed.
  - `mdt-remote` now exposes `payload_kind()` for inbound custom-channel families
  - `mdt-client-min` now has typed inbound dispatch specs and `typed_remote_dispatch.rs` helper coverage
  - remaining work is live session/business adoption, not re-adding the typed metadata layer
- minimal command-mode state container is already landed.
  - `mdt-input` now carries `CommandModeState` / `CommandModeProjection` with selected-units, command-buildings, command-rect, control-groups, and last target/command/stance selections
  - `mdt-client-min-online` runtime outbound action sync now updates that container instead of keeping command-mode as packet-observability-only state
  - CLI/runtime seed controls for bind/recall/clear-group and rect are also landed, including replay after world reload/reconnect clears
  - remaining work is richer live input binding and command/build UI flow, not re-adding the state container baseline
- `mdt-world` post-load activation preflight is already landed.
  - `SavePostLoadActivationSurface` exposes loadable/skipped entity candidates, unresolved remap names, building-center reference validity, and `can_seed_runtime_apply()`
  - remaining work is consuming that surface for Java-like live world/entity activation
- `mdt-typeio` raw `WeaponMount[]` codec is already landed.
  - remaining non-object codec gap is now more about `abilities/status` and wider unit-sync families than mounts specifically
- `mdt-render-ui` runtime dialog summary is already landed.
  - prompt priority: `text input > follow-up menu > menu`
  - notice priority: `warning toast > info toast > reliable hud > hud`
  - remaining gap is richer chat/dialog UI interaction, not re-adding a first dialog summary layer
- `entitySnapshot` typed `WorldLabel` rows are no longer packet-counter-only.
  - runtime/HUD now consumes active label count plus latest `entity_id/text/flags/font_size/z/position`
  - remaining work is broader render/UI depth, not re-adding the first runtime-apply bridge
- `mdt-input` batch runtime intent sampling is already landed.
  - same-tick multi-snapshot batches now preserve transient press/release edges instead of only keeping the final frame
  - remaining work is richer live input source parity, not re-adding batch edge retention
- builder queue tile-state validation is already landed in `mdt-input`.
  - local queued place/break entries can now be pruned against observed tile states when the tile is already air or already matches the requested block/rotation
  - remaining work is broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding the validation primitive
- building-table block identity carry-through is already landed.
  - `BuildingProjection` / `BuildingTableProjection` now include `block_name` and `last_block_name`
  - world baseline, entity building rows, loaded-world extra entry, `constructFinish`, and `deconstructFinish` already wire `block_name` into the building table
  - `render_runtime` build inspector now prefers the typed runtime view sourced from building table + `configured_block_projection`
  - remaining work is deeper live building ownership/runtime parity, not re-adding this field plumbing or inspector bridge

## Highest-Confidence Remaining Lanes

### U1 `entitySnapshot` typed runtime apply

Remaining gap:
- Rust still writes parsed rows into lightweight projection tables instead of doing Java-like `readSyncEntity -> readSync -> snapSync -> add`.

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
- low-risk loaded-world tail/base folds, `block_name` carry-through, and the first typed build-inspector runtime view are landed, but Rust still does not have Java-like `tile.build.readSync(..., version)` runtime ownership.

Best bounded next slice:
- connect already parsed base/tail data into a stronger typed building runtime model
- do not spend time re-landing already wired configured/resource folds, `block_name` / `last_block_name`, or the current build-inspector typed runtime consumer

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
