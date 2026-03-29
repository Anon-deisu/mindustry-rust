# Release Unfinished Current (2026-03-24f)

Purpose: give later subagents a short, current, conflict-aware list of what is still unfinished after the latest M6-M9/U5 parity slices landed.

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
- `mdt-world` low-risk legacy save building-tail coverage is already much wider than the older unfinished text said.
  - existing `parse_legacy_building_tail_snapshot(...)` coverage now also includes:
    - `core-shard` / `core-foundation` / `core-nucleus`
    - `wave` / `tsunami` / `lancer` / `arc` / `meltdown` / `afflict` / `malign`
    - item-turret family plus `build-tower` / `lustre` / `sublimate`
    - drill / generator / reactor / generic-crafter / `cultivator`
    - payload building families (`payload-loader` / `payload-unloader` / `payload-source` / `payload-router` / `payload-mass-driver` / `constructor` / `landing-pad` / `block-producer` / `unit-assembler`)
    - unit-factory / reconstructor families
  - malformed legacy building tail regression tests are now also landed for this slice
  - remaining `U3` work is broader live runtime ownership/building semantics, not re-opening these parser branches as if still missing
- typed runtime building shell coverage is already wider than the older unfinished text said.
  - the low-risk runtime shell batch now also covers:
    - `liquid-router` / `liquid-junction` / `liquid-container` / `liquid-tank`
    - `micro-processor` / `logic-processor` / `hyper-processor`
    - `message` family empty-string fallback shell
    - reconstructor family explicit `None` command shell when runtime/config is absent
  - remaining work is broader family depth and deeper Java-like runtime semantics, not re-adding these shell baselines
- `hiddenSnapshot` lifecycle delete is already narrowed to known runtime-owned non-local semantics instead of deleting every hidden entity row.
  - current cleanup now covers non-local `Unit` / `Fire` / `Puddle` / `WeatherState`
  - `WorldLabel` is still intentionally preserved as a conservative boundary
- `hiddenSnapshot` cleanup regression coverage now also includes hidden `building-tether payload` rows.
  - `class_id=36` hidden rows are now pinned in the current cleanup matrix instead of being left implicit
  - remaining hidden work is deeper runtime semantics, not re-proving this cleanup path
- `hiddenSnapshot` latest-set sync now also clears stale entity hidden flags when ids leave the hidden set.
  - `EntityTableProjection::apply_hidden_ids(...)` mirrors the newest hidden-id set instead of only setting `hidden=true`
  - remaining hidden work is deeper runtime semantics, not re-fixing stale hidden flags on surviving rows
- `effect(..., data)` runtime overlay already consumes the `float_length` contract for ray-endpoint projection.
- `tileConfig` authority reconcile is no longer a single-value last-write-only pending model.
  - per-building local intents now keep FIFO request order
  - authoritative `tileConfig` / `constructFinish` / parse-fail fallback only resolve the oldest pending request
  - later local config intents on the same building are preserved instead of being cleared by the first authoritative response
  - power-node `constructFinish` now also has family-specific canonical-authority regression coverage, including seeding canonical full-replace authority and later parse-failed `tileConfig` fallback without double-toggle drift
- inbound custom/logic packet typed registry glue is already landed.
  - `mdt-remote` now exposes `payload_kind()` for inbound custom-channel families
  - `mdt-client-min` now has typed inbound dispatch specs and `typed_remote_dispatch.rs` helper coverage
  - remaining work is live session/business adoption, not re-adding the typed metadata layer
- typed custom-channel remote dispatch fixed-table glue is now also landed.
  - `mdt-remote::typed_custom_channel_remote_dispatch_specs(...)` now exposes a typed custom-channel dispatch table, and `mdt-client-min` consumes it through `custom_channel_registry_glue.rs` plus a fixed-table `CustomChannelPacketRegistry` instead of rebuilding that surface only from repeated manifest scans
  - remaining `M6-1` work is broader typed registry/session adoption beyond this custom-channel fixed-table slice, not re-adding this lookup layer
- `mdt-remote` typed generation is no longer limited to the high-frequency subset.
  - generated typed registry/dispatch coverage now also includes custom-channel and inbound dispatch families
  - remaining M6 work is deeper live session/business adoption, not re-adding the first broader typed generation layer
- remote manifest validation already fail-closes the wire-level invariants older audit notes used to call out.
  - `mdt-remote` now rejects `packetIdByte`, `lengthField`, compression-flag, and compression-threshold drift during manifest parse, with direct regression coverage
  - remaining remote/codegen work is broader semantic adoption, not re-adding these wire-spec guards
- remote manifest validation now also rejects duplicate remote `packetId` values before typed/runtime lookup can silently first-match-shadow the later entry.
  - `validate_remote_manifest(...)` fail-closes duplicate remote ids with regression coverage instead of leaving that ambiguity to `RemotePacketRegistry::get_by_packet_id(...)`
- `connect` payload preflight now also rejects empty `versionType` / `name` / `locale` fields before encoding.
  - `ConnectPacketSpec::encode_payload(...)` now fail-closes on those empty fields instead of letting them encode through as blank strings
- `connect` payload size-limit boundary regressions are now also landed.
  - direct tests now pin malformed base64 identity decode plus `TooManyMods` / `StringTooLong` encode failure boundaries instead of leaving those size guards implicit
- `mdt-remote` fixed-table packet lookup now fail-closes on duplicate `packet_id` values instead of silently overwriting earlier entries.
  - `RemotePacketIdFixedTable::from_iter(...)` now asserts on duplicate packet ids during fixed-table construction
- `mdt-remote` shared-only bidirectional wire fallback is now explicitly pinned by regression test.
  - `targets=both` packets whose params are all shared still fall back to `ClientToServer` wire flow without changing `wire_params`, and this should not be reopened as an ambiguity bug
- `mdt-remote` required-family lookup APIs now also have explicit missing-family regressions.
  - `high_frequency_remote_packets`, `custom_channel_remote_packets`, and `inbound_remote_packets` now pin their error paths when an expected remote family is absent from the manifest
- host runtime custom-packet action handling now dedupes repeated actions instead of replaying them.
  - `host` action dedupe is landed and should not be reopened as if still missing
- relay redirect handling now clears the runtime state before applying the redirect target.
  - `relay` redirect reset is landed and should not be reopened as if still missing
- runtime custom-packet reset/read-side hardening is now wider than the older unfinished text said.
  - `relay` now also clears runtime state on `WorldStreamStarted`, and reset regressions now cover text/binary/logic routes instead of only text
  - `bridge` reset counter regressions now pin `surface_reset_count` vs `reconnect_reset_count` isolation
  - `bridge` debug-string regressions now pin malformed escape behavior and trailing-backslash rejection
  - `relay` spec parser regressions now pin empty-field / extra-`@` / invalid-transport rejection
  - `surface` reset regressions now also pin summary/latest-view clearing for redirect/world-stream resets
  - `surface` decode-error regressions now also cover invalid UTF-8 binary payloads and logic `no_string_payload` failures
- `net_loop` inbound packet invariants are now covered on all three low-risk paths.
  - regression coverage now pins decode-failure no-op, unknown-packet no-op, and classified snapshot success-path counter/state updates
- minimal command-mode state container is already landed.
  - `mdt-input` now carries `CommandModeState` / `CommandModeProjection` with selected-units, command-buildings, command-rect, control-groups, and last target/command/stance selections
  - `mdt-client-min-online` runtime outbound action sync now updates that container instead of keeping command-mode as packet-observability-only state
- builder-queue local activity now treats incomplete observation sets as non-authoritative for reordering.
  - `update_local_activity(...)` only rotates/falls back when the active queue has a complete observation set
  - missing later observations no longer spuriously move the queue head
  - CLI/runtime seed controls for bind/recall/clear-group and rect are also landed, including replay after world reload/reconnect clears
  - remaining work is richer live input binding and command/build UI flow, not re-adding the state container baseline
- `mdt-world` post-load activation preflight is already landed.
  - `SavePostLoadActivationSurface` exposes loadable/skipped entity candidates, unresolved remap names, building-center reference validity, and `can_seed_runtime_apply()`
  - remaining work is consuming that surface for Java-like live world/entity activation
- `mdt-world` passive save parser fail-close coverage is now wider than the older unfinished text said.
  - content/patch parser regressions now pin trailing-byte rejection for `content` header and `patches` regions
  - `post_load_world()` now has explicit required-region regressions for missing `content` / `map`
  - `team plan` regressions now pin undersized group bytes for both modern and legacy formats instead of only huge-count rejection
  - save-map regressions now also pin oversized block-run rejection instead of only floor-run / chunk-len failures
  - `static fog` regressions now pin overrun and trailing-byte rejection
  - entity sync regressions now pin invalid `player unit_kind`, dead-player non-zero unit value, and non-finite `weather/world-label` positions
  - save `entities` region regressions now pin trailing-byte rejection for modern save versions
  - entity remap parsing now fail-closes duplicate `custom_id` / duplicate `name` instead of silently accepting ambiguous remaps
- `mdt-world` now also has a deterministic `SavePostLoadRuntimeSeedPlan` layer above that preflight.
  - `.msav -> post_load_world() -> projection_contract() -> activation_surface()` is now folded into a passive seed plan for later runtime/apply consumers
  - remaining M7-3 work is consuming that seed plan in deeper runtime/world ownership, not re-adding the first passive plan layer
- `mdt-world` consumer-side post-load apply plan helper is now also landed.
  - `SavePostLoadConsumerApplyPlan::consumer_apply_plan()` now turns the stricter contract/activation/seed surfaces into a deterministic consumer-stage plan with explicit blocker reasons (`contract issue`, duplicate `entity_id`, invalid building-center refs, skipped entity)
  - remaining `M7-3` work is still wiring that passive plan into real runtime/world ownership, not re-adding this consumer-side helper layer
- `mdt-world` consumer-runtime stage helper is now also landed.
  - `consumer_runtime_helper()` now classifies each post-load consumer stage as `ApplyNow` / `AwaitingWorldShell` / `Blocked` / `Deferred`, preserves blocker reasons per stage, and exposes apply/await/block/defer step counts for later runtime owners
  - remaining `M7-3` work is still real runtime/world ownership and stage execution, not re-adding this passive runtime-readiness helper
- `mdt-world` runtime-apply batch view helper is now also landed.
  - `runtime_apply_batch_view()` now folds non-empty consumer runtime stages into deterministic contiguous apply batches, preserving batch order, per-batch disposition, stage detail, aggregated `step_count`, and deduplicated blockers for later runtime owners
  - remaining `M7-3` work is still executing those batches inside real runtime/world ownership, not re-adding this passive batch-view helper
- `mdt-typeio` raw `WeaponMount[]` codec is already landed.
  - remaining non-object codec gap is now more about `abilities/status` and wider unit-sync families than mounts specifically
- `mdt-typeio` read-side guard coverage is now wider for several low-risk scalar/container paths.
  - payload-header presence bytes now reject non-binary values
  - `item` / `liquid` ids now reject negative values other than the `-1` null sentinel
  - `traceInfo` read-side now fail-closes `ips` / `names` counts above the writer's `12` entry cap
  - dynamic status-entry flags now reject reserved high bits instead of silently ignoring them
- `mdt-render-ui` runtime dialog summary is already landed.
  - prompt priority: `text input > follow-up menu > menu`
  - notice priority: `warning toast > info toast > reliable hud > hud`
  - remaining gap is richer chat/dialog UI interaction, not re-adding a first dialog summary layer
- `effect(..., data)` runtime content projection is already widened for the landed clean-file slices.
  - `content_icon` / `payload_target_content` now accept `TechNode`-backed content refs in the runtime effect overlay path, and `drop_item` now emits a content projection instead of only carrying the drop origin
  - remaining work is broader `effect` executor depth, not re-adding these landed content-ref / content-projection slices
- online build planning no longer trusts stale loaded-world plan markers for nearby place/conflict candidate selection.
  - `select_place_near_player_tile_with_visibility(...)` and `select_conflict_place_near_player_tile(...)` now ignore `graph.team_plans_at(...)` and rely on live building projection plus visibility filtering instead
  - regression coverage now pins the `removeTile -> stale team-plan marker` case so it does not block valid live candidates
  - remaining work is deeper builder/runtime execution semantics, not re-adding this stale-plan read-side fix
- runtime entity ownership conflict resolution is already hardened beyond the earlier controller-vs-heuristic baseline.
  - same-player authoritative conflicts now resolve by newest `last_seen_entity_snapshot_count`
  - heuristic ownership no longer overrides an already established authoritative owner for the same player
  - remaining work is broader entity semantic depth and Java-like live ownership/group behavior, not re-fixing this narrower ownership arbitration bug
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
- `stateSnapshot` core inventory fallback now also has a regression test for the `no new coreData` path.
  - `derive_state_snapshot_core_inventory_transition(Some(previous), None)` now reuses the previous inventory snapshot with `synced=false` and zero duplicate counts instead of being left implicit
  - remaining `M7` work is broader core/state ownership semantics, not re-proving this fallback branch
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
- runtime batch intent sampling now also respects override semantics.
  - `RuntimeIntentTracker` batch sampling now honors persistent and one-shot overrides the same way the single-snapshot path does, and `mdt-input` exposes `map_snapshot_batch_or_override(...)` so batch mapping no longer silently bypasses override state
  - remaining M8 work is still richer live input source parity and deeper command/build flow, not re-adding this batch/override consistency slice
- empty runtime intent batches now also clear transient action edges correctly.
  - `RuntimeIntentTracker::sample_runtime_snapshot_batch()` now clears stale `pressed_actions` / `released_actions` even when the incoming runtime batch is empty and no override is active, while preserving persistent active/axis/build/mining state
  - remaining M8 work is still richer live input source parity and deeper command/build flow, not re-adding this empty-batch edge cleanup slice
- builder queue tile-state validation is already landed in `mdt-input`.
  - local queued place/break entries can now be pruned against observed tile states when the tile is already air or already matches the requested block/rotation
  - remaining work is broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding the validation primitive
- builder queue tile-state validation now also supports explicit rotation-irrelevant observations.
  - `BuilderQueueTileStateObservation.requires_rotation_match` can now preserve or clear local place plans based on whether the observed tile family actually requires rotation equality
  - remaining work is still broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding this validation refinement
- builder queue local activity/reconcile state-machine semantics are now richer.
  - `update_local_activity()` now reports explicit head-selection outcomes (`HeadInRange`, reorder/fallback/skip/out-of-range/missing cases), and `validate_against_tile_states()` now reports whether reconcile left the queue unchanged, removed a non-head entry, advanced the head, or cleared the queue
  - remaining work is still broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding this pure state-machine selection/reconcile slice
- online builder-queue / auto-build read-side now also consumes merged live building view instead of trusting loaded-world center truth.
  - `ClientSession::building_live_state_at(...)` / `building_live_state_projection(...)` now expose the merged per-tile live view
  - `mdt-client-min-online` builder queue reconcile/activity and auto-build selection now use that merged view, so `removeTile` stale centers and live `setTile` rotation updates no longer mislead conflict/break target selection or queued place suppression
  - remaining work is still broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding this merged-view read-side bridge
- builder queue now also preserves bounded known progress on same-mode local replacement/progression paths.
  - `BuilderQueueEntry` now carries `progress_permyriad`, `observe_progress(...)` records exact tile+breaking progress with clamp-to-`10_000`, and same-tile replacement / begin / sync paths preserve progress only when the breaking mode still matches
  - remaining work is still broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding this pure queue-progress state slice
- builder queue duplicate-tile batch sync ordering is now also corrected.
  - `sync_local_entries(...)` no longer pushes a duplicate tile to the queue tail unconditionally; unique tiles keep prior relative order, and duplicate tiles are reinserted by their last incoming occurrence
  - remaining work is still broader runtime integration and Java-equivalent `BuilderComp` depth, not re-adding this duplicate-tile ordering correction
- building-table block identity carry-through is already landed.
  - `BuildingProjection` / `BuildingTableProjection` now include `block_name` and `last_block_name`
  - world baseline, entity building rows, loaded-world extra entry, `constructFinish`, and `deconstructFinish` already wire `block_name` into the building table
  - `render_runtime` build inspector now prefers the typed runtime view sourced from building table + `configured_block_projection`
  - online `render_runtime` now also has a `ClientSession` path that consumes merged building live view for runtime-building scene objects, build inspector rows, and `runtime_buildings` HUD summary instead of raw building-table rows alone
  - remaining work is deeper live building ownership/runtime parity, not re-adding this field plumbing or inspector bridge
- `mdt-world` post-load contract validation now cross-checks actual entity chunks against the summary.
  - `SavePostLoadWorldObservation::projection_contract()` no longer accepts only `loadable + skipped == total`; it now re-derives the effective post-load entity summary from `world_entity_chunks` and rejects summary drift
  - remaining `M7-3` work is deeper consumer-side runtime/world ownership, not re-adding this stricter passive contract check
- `mdt-world` post-load contract now also fail-closes the specific `single damaged static-fog-data chunk` case instead of silently treating it as if no static fog existed.
  - `projection_contract()` now surfaces `StaticFogCoverageMismatch` when the lone `static-fog-data` chunk parses as `Unknown`, with a direct regression test
- typed high-frequency snapshot registry glue is now landed.
  - `mdt-remote` now exposes `HighFrequencyRemoteRegistry`, `mdt-client-min` snapshot packet registry now consumes typed glue via `snapshot_registry_glue.rs`, and inbound-family registry construction no longer depends on unrelated outbound custom-channel families
  - remaining `M6-1` work is broader typed registry consumption outside the first snapshot/inbound glue path, not re-adding this typed snapshot registry layer
- typed inbound remote dispatch fixed-table glue is now also landed.
  - `mdt-remote::typed_inbound_remote_dispatch_specs(...)` now exposes a typed non-snapshot inbound dispatch table, and `mdt-client-min` packet registry consumes it through `inbound_remote_registry_glue.rs` instead of rebuilding that lookup only from string/manifest scans
  - remaining `M6-1` work is broader typed registry/session adoption beyond this fixed-table inbound dispatch slice, not re-adding the first fixed-table glue layer
- typed runtime `packet_id -> family/spec` fixed-table consumption is now also landed for the non-snapshot inbound/custom-channel registries.
  - `mdt-remote` now exposes `RemotePacketIdFixedTable`, and `mdt-client-min` `InboundRemotePacketRegistry` / `CustomChannelPacketRegistry` use that typed fixed-table for runtime `packet_id` lookup instead of falling back to manifest/string scans on the hot path
  - remaining `M6-1` work is broader typed registry/session/business adoption, not re-adding this fixed-table hot-path lookup slice
- `mdt-render-ui` minimap/overlay semantic detail breakdown is now landed.
  - render/model panel presenters now expose deterministic family+detail counts for minimap and overlay summaries instead of only coarse kind buckets
  - remaining `M9` work is still deeper renderer pipeline and interactive UI flow, not re-adding this detail-breakdown presentation slice
- `mdt-render-ui` presenter-local HUD/chat/menu/dialog/minimap detail rows are now landed.
  - panel/window/ascii presenters now expose `HUD-DETAIL`, `MINIMAP-*DETAIL`, `RUNTIME-MENU-DETAIL`, `RUNTIME-DIALOG-DETAIL`, and `RUNTIME-CHAT-DETAIL` rows derived from existing runtime observability instead of only coarse summary rows
  - remaining `M9` work is still interactive UI/user-flow depth, not re-adding this presenter-local detail slice
- `mdt-render-ui` runtime notice detail rows are now also landed.
  - panel/window/ascii presenters now expose deterministic `RUNTIME-NOTICE-DETAIL` rows derived from existing HUD/toast/text-input observability instead of leaving `RUNTIME-NOTICE` as a summary-only line
  - remaining `M9` work is still interactive UI/user-flow depth, not re-adding this presenter-local notice-detail slice
- `mdt-render-ui` rich runtime UI observability bridge is now also landed.
  - `RuntimeUiObservability` now carries structured `announce` / `infoMessage` / `infoPopup` / `copyToClipboard` / `openURI` notice data, `menu` / `followUpMenu` metadata, and `menuChoose` / `textInputResult` result fields instead of leaving them only in compact runtime text or raw `SessionState`
  - `render_runtime` now projects those fields directly, and panel/window/ascii presenters expose them through richer `RUNTIME-NOTICE`, `RUNTIME-MENU`, and `RUNTIME-CHOICE` rows plus deterministic detail output
  - remaining `M9` work is still deeper dialog/chat interaction and Java-equivalent UI lifecycle, not re-adding this observability bridge
- `mdt-render-ui` build/minimap assist presenter slice is now landed.
  - panel/window presenters now expose `BuildMinimapAssistPanelModel` and `BUILD-MINIMAP-AUX` rows that combine build head/reconcile/config/auth/runtime hints into a single deterministic presenter-local summary
  - remaining `M9` work is still broader interactive UI and renderer/runtime parity, not re-adding this presenter-local assist summary slice
- `mdt-render-ui` runtime-session presenter summary is now landed.
  - panel/window/ascii presenters now expose deterministic `RUNTIME-SESSION` rows that aggregate existing kick/loading/reconnect observability without changing the existing `RUNTIME-KICK` / `RUNTIME-LOADING` / `RUNTIME-RECONNECT` detail rows
  - remaining `M9` work is still broader interactive UI and renderer/runtime parity, not re-adding this presenter-local session summary slice
- `mdt-render-ui` runtime UI stack presenter summary is now also landed.
  - panel/window/ascii presenters now expose deterministic `RUNTIME-STACK` and `RUNTIME-STACK-DETAIL` rows that surface current `text input / follow-up menu / menu / chat / notice` stack composition and depth from existing `runtime_ui` observability
  - remaining `M9` work is still broader interactive UI and renderer/runtime parity, not re-adding this presenter-local stack summary slice
- `mdt-render-ui` build/minimap user-flow presenter summary is now also landed.
  - panel/window/ascii presenters now expose deterministic `BUILD-FLOW` rows that compress current build interaction plus minimap assist state into a stable next-action label such as `arm / realign / seed / resolve / refocus / survey / commit / break / idle`
  - remaining `M9` work is still broader interactive UI and renderer/runtime parity, not re-adding this presenter-local build-flow summary slice
- `mdt-render-ui` window build-config detail rows are now also landed.
  - window presentation now emits deterministic `BUILD-CONFIG-ENTRY` and `BUILD-CONFIG-MORE` rows on top of the existing capped build-config panel data instead of only the compact summary text
  - remaining `M9` work is still broader interactive UI and renderer/runtime parity, not re-adding this presenter-local detail slice
- typed building runtime apply state is now landed as a separate persistent layer.
  - `SessionState` now keeps `runtime_typed_building_apply_projection` with fallback to the computed typed join when tests/setup mutate only raw tables
  - typed building models now carry already parsed base/head/turret fields (`rotation/team/io_version/module/time-scale/health/enabled/efficiency/visible_flags/build-turret summary`) in addition to the configured domain value
  - `client_session` now refreshes that layer from loaded-world tail/business folds, authoritative `constructFinish` / `tileConfig` / `buildHealthUpdate`, `deconstructFinish` / `removeTile`, and `worldDataBegin` clear, and `render_runtime` build inspector now consumes that runtime-owned projection instead of rebuilding only from the raw table join at the callsite
  - remaining `U3` work is still broader family depth and true Java-like `tile.build.readSync(..., version)` runtime ownership, not re-adding this first persistent typed building apply layer
- low-risk `tileConfig` link-family coverage now also includes `bridge-conduit` / `phase-conduit`.
  - authoritative `constructFinish` + `tileConfig` now drive the existing item-bridge link projection and typed runtime building view for these `LiquidBridge` families instead of limiting that low-risk link slice to `bridge-conveyor` / `phase-conveyor`
  - remaining `U4` / `U3` work is still broader configured business and live building semantics, not re-adding these two low-risk link families
- narrow `effect_id=142` `drop_item` executor wiring is now landed.
  - `effect_contract(Some(142))` now resolves to `drop_item`, and the runtime effect executor projects the overlay origin forward along rotation with fixed-length `dropItem` behavior instead of leaving it as a generic item-content packet summary
  - remaining `U5` work is still landing additional narrow `effect_id -> contract/executor` families, not re-adding this first `drop_item` slice
- narrow `effect_id=10` `point_beam` contract/executor wiring is now also landed.
  - `effect_contract(Some(10))` now resolves to `point_beam`, contract-aware business projection still reuses the existing `PositionTarget { source, target }` payload semantics, and runtime rendering now keeps the dedicated beam line behavior keyed to `effect_id=10`
  - Rust now also carries the same narrow unit-parent source-follow binding for `effect_id=10`, so the spawned beam start no longer freezes at the original packet origin when the parent unit later moves
  - remaining `U5` work is still landing additional narrow `effect_id -> contract/executor` families plus deeper parent/source parity, not re-adding this `point_beam` slice
- narrow `effect_id=11` `pointHit` contract/executor wiring is now also landed.
  - `effect_contract(Some(11))` now resolves to `point_hit`, session/runtime surfaces keep the dedicated contract name, and runtime rendering now emits an expanding hit-ring fallback from the effect position instead of stopping at a generic marker
  - remaining `U5` work is still landing additional narrow `effect_id -> contract/executor` families, not re-adding this `pointHit` slice
- narrow `effect_id=8` `unitSpirit` executor wiring is now also landed.
  - Rust keeps `effect_id=8` on the existing `position_target` contract, and runtime rendering now emits the effect-specific double-diamond fallback from the captured source/target bits instead of stopping at the target marker alone
  - Rust now also carries a narrow source-follow binding for `effect_id=8` when `data` is a parent `Unit`, so the spawned source point and rendered diamonds move with the parent instead of freezing at the original world source
  - remaining `U5` work is now parity depth rather than first-pass absence: wider `position_target` source-follow, stable effect-instance seed parity, and broader parent semantics are still open, but `unitSpirit` no longer stops at static-source fallback behavior
- narrow `effect_id=9` `itemTransfer` executor wiring is now also landed.
  - Rust keeps `effect_id=9` on the existing `position_target` contract, and runtime rendering now emits a conservative pseudo-seeded double-ring fallback plus marker-position override instead of leaving only a target marker
  - Rust now also carries a narrow source-follow binding for `effect_id=9` when `data` is a parent `Unit`, so the spawned source point and curve/marker geometry move with the parent instead of freezing at the original world source
  - remaining `U5` work for this family is now exact-parity depth rather than total absence: Java-like per-instance lateral offset still needs a stable effect-instance seed equivalent to `e.id`, and wider parent-follow/rotation semantics are still incomplete outside this narrow slice
- narrow `effect_id=263` `legDestroy` contract/executor wiring is now also landed.
  - Rust now maps `effect_id=263` to `leg_destroy`, keeps the contract name on the session surface, projects the line target from the second explicit position with fallback to the first explicit position when needed, and renders a dedicated runtime line fallback instead of collapsing to a generic marker
  - remaining `U5` work for this family is now deeper segment/region geometry and higher-fidelity effect-instance semantics, not re-adding the first `legDestroy` contract/executor slice
- runtime effect overlay lifetime behavior is no longer fixed to one global `3 tick` decay.
  - `RuntimeEffectOverlay` now carries both `lifetime_ticks` and `remaining_ticks`, and `render_runtime` seeds effect-shaped TTLs for the currently landed runtime families instead of forcing every effect through the same fixed short-lived decay
  - remaining `E3` work is now narrower: Rust still lacks full `position_target` source-follow parity, general building-parent offset follow, clearer binding/fallback observability, and deeper effect-instance parity, but `rotWithParent`, `startDelay`, `clip`, and the first lifetime-aware overlay path are already landed
- narrow `effect_id=261/262` `chainLightning` / `chainEmp` executor wiring is now also landed.
  - Rust now keeps deterministic segmented chain line overlays for `261/262` on top of the existing `position_target` payload semantics instead of stopping at a single marker/target projection
- narrow `effect_id=13` `lightning` contract/executor wiring is now also landed.
  - Rust now maps `effect_id=13` to `lightning`, preserves `Vec2Array` polyline payloads in business/runtime projection, and renders per-overlay lightning segments instead of collapsing to a single first-point marker
  - remaining `U5` work is still landing additional narrow `effect_id -> contract/executor` families, not re-adding these first chain-effect slices
- narrow `effect_id=256` `shieldBreak` executor wiring is now also landed.
  - Rust now maps `effect_id=256` to `shield_break`, keeps the effect-specific runtime executor name on the session surface, and renders the Java fallback-style expanding hexagon as runtime line segments keyed by the effect origin plus `rotation`
  - remaining `U5` work is still landing additional narrow `effect_id -> contract/executor` families, not re-adding this first shield-break fallback slice
- narrow `effect_id=257/260` `arcShieldBreak` / `unitShieldBreak` unit-parent fallback geometry is now also landed.
  - Rust keeps the existing `unit_parent` parent-follow binding path, runtime rendering no longer stops at a bare marker, and authoritative entity-table hits now lazily freeze the spawned effect offset instead of snapping straight to the parent origin on every frame: `257` emits a parent-rotation-aware double-arc fallback band and `260` emits a circle-plus-burst fallback
  - remaining `U5` work for this family is now metadata and deeper parent semantics rather than first-pass geometry: exact `257` parity still needs `ShieldArcAbility` radius/width/offset data, exact `260` parity still needs `unit_type -> hitSize`, and non-authoritative fallbacks still do not preserve Java-equivalent relative offsets
- `mdt-client-min-online` custom/logic runtime surface wiring is now landed as a narrow `M6-3` harness slice.
  - the online harness now reuses `custom_packet_runtime_surface` across `--consume-client-*` custom/logic flows, emits runtime/business overlay summaries on updates and resets, and re-installs that surface after reconnect/redirect rebuilds
  - remaining `M6-3` work is still deeper Java-equivalent business integration, not re-adding this harness/runtime summary bridge
- `connectConfirm` queued-vs-flushed observability is now landed as the first narrow `U6` transport split.
  - `SessionState` now tracks both `connect_confirm_sent` and `connect_confirm_flushed`, ArcNet only flips the flushed bit after a real TCP frame fully drains (not merely after enqueue / partial nonblocking write progress), and UDP-only driver paths preserve the intended `queued-but-not-flushed` boundary
  - `finish_connecting()` replay-failure rollback now also has regression coverage for the prequeued-`connectConfirm` case, so preexisting TCP backlog survives late deferred replay failure byte-for-byte instead of being silently mutated by the aborted load-complete path
  - ArcNet ready-tick coverage now also pins `connectConfirm` ahead of later queued chat/gameplay TCP traffic on the first post-load flush
  - remaining `U6` work is still deeper Java-equivalent lifecycle atomicity, not re-adding this first queued/flushed split
- `mdt-remote` manifest validation now also rejects duplicate `packetClass` values and duplicate generated Rust packet const names.
  - remaining work is broader typed-registry/session adoption, not re-adding this manifest/codegen guardrail
- `rules_objectives_semantics` objective flag counts now track only successfully parsed string flags.
  - malformed mixed-type `flagsAdded` / `flagsRemoved` arrays no longer overstate count metadata
- `custom_packet_runtime_host` world-position fallback now accepts both `x,y` and `x:y`.
  - the narrow `RequestDropPayload` host-action path already has regression coverage for the colon form
- `mdt-world` post-load runtime execution now has explicit regression coverage for the current shell-vs-full-apply split.
  - duplicate non-world custom chunks still fail full runtime apply but do not block `execute_runtime_world_semantics()`
  - duplicate marker ids still fail world-semantic activation after `world_shell_ready`
- handoff target-checkout resolution now normalizes relative paths against the source workspace before validation.
  - this applies consistently across `-TargetCheckout`, source-workspace git config, and `MDT_TARGET_CHECKOUT`

## Latest Closures

- custom-packet text/surface parsers now reject fake JSON key lookalikes, and text/surface/logic number paths now fail-close on non-finite payloads instead of stringifying `NaN`/`inf`.
- `mdt-remote` now fail-closes blank `generator.source` and `generator.call_class`.
- `arcnet` now clears the TCP read buffer on `TcpClosed` so partial frames do not linger.
- `static fog` runtime seeding now blocks damaged single-chunk payloads instead of treating them as seedable.
- `mdt-world` activation now requires `world_shell_ready` and unresolved effective names to be clear; `projection_contract` now flags line-marker end overflow, legacy save-map / legacy entity-group / malformed UBJSON length regressions are pinned, and repeated readiness source regions merge counts/blockers deterministically.
- `mdt-typeio` string-writer limits and `mdt-typeio::unit_sync` count ceilings now have explicit panic-bound regressions.
- `rules_objectives_semantics` now normalizes blank/whitespace objective flags, rejects malformed JSON string escapes/raw control chars, and treats decoded objective string literals as real JSON strings instead of preserving wire escapes verbatim.
- custom-packet text/surface/logic helpers now ignore nested text JSON fields for world-position extraction, reject unknown debug-string escapes, and keep non-finite world-position payloads fail-closed.
- runtime effect helpers now reject non-finite position hints and malformed lightning geometry instead of partially projecting corrupted effect bindings.
- snapshot/custom-channel typed registry glue now also rejects `clientSnapshot` packet-id collisions with inbound snapshot ids before fixed-table lookup can shadow the later family.
- `mdt-world` consumer/runtime ownership helpers now keep zero-step `ApplyNow` stages ready, preserve blocked zero-step ownership surfaces, and require `world_shell_ready` before world semantics can apply.
- `mdt-world` projection-contract tile validation now also fail-closes stale tile-side `building_center_index` references instead of only checking center-to-tile backreferences.
- `mdt-typeio` plan/trace readers now reject non-binary bool markers on the remaining plan/trace paths, and non-dynamic status-entry batches no longer serialize stray dynamic-field payloads.

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
- narrow `BuildTurret`/`build-tower` anchor-order hardening is now also landed.
  - loaded-world building anchors now preserve `build_turret rotation/plans/count` from parsed tails, and `client_session` now refreshes the runtime typed building apply projection from merged live state across loaded-world tail business, authoritative tile/team updates, and authoritative config paths instead of dropping back to raw table-only refresh in those slices
  - regression coverage now proves the merge helper keeps `build-tower` tail summary when the live head lacks it, and the runtime typed building apply layer stays coherent when `build-tower` tail-derived anchor state is refreshed before a later authoritative `constructFinish`

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
- Rust has bounded runtime overlays, several contract-aware projections, and narrow landed slices for `effect_id=8 -> unitSpirit`, `effect_id=9 -> itemTransfer`, `effect_id=10 -> point_beam`, `effect_id=11 -> point_hit`, `effect_id=13 -> lightning`, `effect_id=142 -> drop_item`, `effect_id=256/257/260 -> shield/parent families`, `effect_id=261/262 -> chainLightning/chainEmp`, and `effect_id=263 -> legDestroy`, but still not Java `Effect`-executor semantics.

Best bounded next slice:
- prefer one narrow semantic deepening slice at a time: highest-signal candidates are `binding/fallback` observability, `9` exact-parity seed support, or wider `position_target` source-follow beyond `8/9`
- do not re-open `263` `legDestroy` as if it were still a missing first-pass family
- stay above raw packet decode and below full renderer parity

Write scope:
- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/render_runtime.rs`

### U6 `finishConnecting` / `clientLoaded` lifecycle parity

Remaining gap:
- `mark_client_loaded()` now fail-closes deferred replay and auto-queues `connectConfirm` once the world becomes ready, and the resulting ready-state action ordering has been regression-revalidated across the full current `mdt-client-min` suite.
- the remaining gap is now narrower still: queued-vs-flushed `connectConfirm` observability is explicit and ArcNet marks flush only after a real TCP write, but Java-equivalent transport/lifecycle atomicity across `finishConnecting`, replay side effects, reconnect edges, split-driver coordination, and higher-layer UI/runtime assumptions is still incomplete.

Best bounded next slice:
- keep this serial-only and do not mix with snapshot/entity/world ownership work

Write scope:
- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/arcnet_loop.rs`
- `rust/mdt-client-min/src/udp_loop.rs`

## Conflict Notes

- Do not assign more than one worker at a time to `client_session.rs` unless their write sets are clearly disjoint and pre-reviewed.
- Treat `worldDataBegin`, reconnect, deferred replay, and `clientLoaded` as serial-owner areas.
- If a worker proposes work on a slice listed under `Do Not Re-Do`, redirect it to one of `U1`..`U6` instead.
