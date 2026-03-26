# Rust Release Audit Continuation

## Scope
This document tracks release-readiness audit continuation for the Rust deliverable.

- Goal: release an accurate `minimal compatibility client` package
- Non-goal: claiming full parity with the Java desktop client
- Current plan context: `AI_HIGH_RISK_REFACTOR_PLAN.md` (`M10-T2`)

## Total Status (Severity Sorted)

### Critical
1. `resolved` Release docs now state the correct scope (`minimal compatibility client`) and no longer imply full Java parity.
   - Evidence: `tools/WINDOWS-RELEASE.md:3`
   - Evidence: `tools/README.md:13-14`
   - Evidence: `README.md:31`
2. `resolved` Windows release docs now use repo-relative commands instead of machine-specific absolute paths.
   - Evidence: `tools/WINDOWS-RELEASE.md`
3. `resolved` Release gate prerequisites now explicitly mention localhost server dependency.
   - Evidence: `tools/WINDOWS-RELEASE.md:21`, `:54-55`
   - Evidence: `tools/check-mdt-release-prereqs.ps1:47`
4. `resolved` Fixture fallback bug fixed in release scripts (`Select-FirstExistingPath` now returns empty string instead of non-existent first candidate).
   - Evidence: `tools/package-mdt-client-min-online.ps1:67-77`
   - Evidence: `tools/verify-mdt-client-min-release-set.ps1:102-113`

### High
1. `resolved` Rust parity task semantics tightened (`opt-in`, explicit `-PrustGoldenDir` requirement, clearer skip/fail behavior).
   - Evidence: `build.gradle:451-470`
   - Evidence: `tests/src/test/java/ApplicationTests.java:6885-6896`
2. `resolved` `clientSnapshot` build-plan queue now follows the current Java `TypeIO.getMaxPlans()` baseline more closely (`20` plan cap plus `String` / `byte[]` config payload budget guard).
   - Evidence: `rust/mdt-client-min/src/client_session.rs:2491-2503`
   - Evidence (test): `rust/mdt-client-min/src/client_session.rs:4153-4173`, `:4212-4225`
3. `resolved` `clientSnapshot` mining tile path is implemented and covered by test (no longer fixed placeholder-only behavior).
   - Evidence: `rust/mdt-client-min/src/client_session.rs:2442-2445`
   - Evidence (test): `rust/mdt-client-min/src/client_session.rs:4059`
4. `resolved` `mdt-world` CLI is now CWD-independent via manifest-dir based repo-root resolution.
   - Evidence: `rust/mdt-world/src/main.rs:13-14`
   - Evidence: `rust/mdt-world/src/main.rs:614-624`
5. `resolved` Multi-workspace verification script is landed and validated.
   - Evidence: `tools/verify-rust-workspaces.ps1`
   - Verified locally: `powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1`
6. `resolved` Release preflight script is landed and passes with canonical fixture paths.
   - Evidence: `tools/check-mdt-release-prereqs.ps1`
   - Verified locally: `powershell -ExecutionPolicy Bypass -File .\tools\check-mdt-release-prereqs.ps1`
7. `resolved` Release-set verification flow now supports optional split-workspace checks.
   - Evidence: `tools/package-mdt-client-min-release-set.ps1:23-28`, `:49-51`, `:128-131`
   - Evidence: `tools/verify-mdt-client-min-release-set.ps1:19-24`, `:46-49`, `:157-167`
8. `resolved` Canonical fixtures exist under `fixtures/...` and satisfy preflight without transitional fallback.
   - Evidence: `tools/check-mdt-release-prereqs.ps1` local run output `status=ok issue_count=0`

### Medium
1. `resolved` F-012 is closed with a frozen go/no-go checklist (owner/command/marker/failure-handling) and final decision block.
   - Evidence: `RUST_RELEASE_AUDIT_FINDINGS.md` sections:
     - `Frozen Go/No-Go Checklist (Track P1)`
     - `Final Decision Block (Frozen)`
2. `in-progress` Remaining post-release parity backlog (M6/M7/M8/M9) needs structured parallel shards for continued review.
   - Target doc: `RUST_RELEASE_AUDIT_FINDINGS.md`
   - Primary subagent dispatch doc: `audit/m6-m9-subagent-write-lanes-20260324.md`
   - M6 remaining high-complexity slice: Java-equivalent `effect(..., data)` business semantics on top of the now-broader `TypeIO.readObject` coverage, not packet detection itself.
   - `stateSnapshot` now also has an explicit wave-advance live signal edge (`strict increase only`, cleared on `worldDataBegin`, surfaced in runtime HUD), so the next M7 focus is deeper `entity` / `block` / `hidden` apply breadth plus broader `stateSnapshot.coreData` semantic depth, while preserving minimal-compatibility release claims.
3. `resolved` Preflight default enforcement is landed (verify runs preflight unless explicitly bypassed).
   - Evidence: `tools/verify-mdt-client-min-release-set.ps1:26-29`, `:140-147`, `:306`
   - Evidence: `tools/package-mdt-client-min-release-set.ps1:30-32`, `:134-136`
   - Evidence: `tools/WINDOWS-RELEASE.md:47`, `:57-58`, `:68-69`
4. `resolved` CI release gate workflow governance is now frozen with required-check stability, owner mapping, and waiver/rerun policy:
   - Evidence: `.github/workflows/rust-release-gate.yml`
   - Evidence: `CODEOWNERS`
   - Plan/report: `audit/ci-gate-plan.md`
   - Current gate includes:
   - `tools/check-mdt-release-prereqs.ps1`
   - `tools/verify-rust-workspaces.ps1`
   - release-set verify smoke in a controlled environment
5. `resolved` Minimal configured-block projection widened again without changing the release claim boundary.
   - Newly landed bounded coverage includes:
   - `message` / `reinforced-message` / `world-message` configured text projection
   - `canvas` / `large-canvas` strict fixed-length `byte[]` configured projection
   - `constructor` / `large-constructor` recipe-block projection
   - `illuminator` int-color projection
   - `payload-source` mixed `Block | UnitType | clear` configured projection
   - `payload-router` / `reinforced-payload-router` mixed `Block | UnitType | clear` configured projection
   - `power-node` / `power-node-large` / `surge-tower` / `beam-link` bounded link-set projection, with authoritative `Point2[]` full-replace and absolute `Int` / `BuildingPos` toggle
   - reconstructor family (`additive` / `multiplicative` / `exponential` / `tetrative`) bounded `UnitCommand | clear` configured projection
   - Evidence: `rust/mdt-client-min/src/client_session.rs`
   - Evidence: `rust/mdt-client-min/src/session_state.rs`
   - Evidence (tests): `rust/mdt-client-min/src/client_session.rs`, `rust/mdt-client-min/src/render_runtime.rs`
6. `resolved` `mdt-world` now parses the `message` building-tail family (`message` / `reinforced-message` / `world-message`) as structured tail data instead of leaving it opaque.
   - Evidence: `rust/mdt-world/src/lib.rs:23533-23534`, `:24466-24475`
   - Evidence (tests): `rust/mdt-world/src/lib.rs:37483-37515`
7. `resolved` `mdt-world` now parses the `payload-router` building-tail family (`payload-router` / `reinforced-payload-router`) as structured tail data instead of leaving it opaque.
   - Landed bounded coverage includes:
   - sorted mixed content ref (`Block | UnitType | clear`)
   - `recDir`
   - carried payload presence/type and serialized body summary, with safe parse-through when full nested payload recovery is unavailable
   - Evidence: `rust/mdt-world/src/lib.rs`
   - Evidence (tests): `rust/mdt-world/src/lib.rs`
8. `resolved` Minimal remote-control coverage widened:
   - `connect(String,int)` now decodes as an explicit redirect/connect event with tracked target host/port state, without over-claiming Java reconnect parity.
   - `kick(KickReason.serverRestarting)` now also has a minimal online-runtime reconnect path that schedules reconnect attempts against the current server by reusing the existing redirect reconnect infrastructure, without over-claiming full Java lifecycle parity.
   - `playerDisconnect` now clears local-player sync state when applicable.
   - `setCameraPosition(float,float)` now updates tracked camera/view-center state without moving player position.
   - `sound(Sound,float,float,float)` and `soundAt(Sound,float,float,float,float)` now decode as fixed-shape audio packets with explicit event/state tracking.
   - inbound `ping` / `pingResponse` no longer surface only as side-effecting ignored packets; they now emit explicit session events alongside the existing reply/RTT behavior.
   - minimal `effect(Effect,float,float,float,Color,Object data)` handling now parses and stores the leading `TypeIO` object while fixed-shape `effect(Effect,float,float,float,Color)` / `effectReliable(Effect,float,float,float,Color)` continue to decode with explicit event/state tracking. Lightweight business projection now also recognizes the first position carried by `Point2[]` / `Vec2[]` payloads instead of reducing those cases to length-only observability.
   - `traceInfo(Player,TraceInfo)` now decodes as structured raw admin payload with explicit event/state tracking.
   - outbound `adminRequest(Player,AdminAction,Object)` and outbound `requestDebugStatus(Player)` now have explicit queue APIs, while inbound `debugStatusClient(int,int,int)` / `debugStatusClientUnreliable(int,int,int)` now decode as structured debug-status events/state.
   - `setRules(Rules)` and `setObjectives(MapObjectives)` now decode as raw length-prefixed JSON replacement packets with explicit event/state tracking.
   - `setRule(String,String)`, `clearObjectives()`, and `completeObjective(int)` now emit explicit minimal events and update tracked session state.
   - runtime HUD now also exposes compact rules/objectives counters (`runtime_rules`), and online `--print-client-packets` summaries now explicitly surface `setRules` / `setObjectives` / `setRule` / `clearObjectives` / `completeObjective`.
   - lightweight rules/objectives semantics are widened again without over-claiming Java parity: objective `parents` are now preserved from `setObjectives`, `qualified` is recomputed from dependency completion, `completeObjective` applies bounded `flagsAdded/flagsRemoved` mutation into a tracked `objective_flags` set, `setFlag` updates the same flag set, and `runtime_rules` now carries compact `qualified/parent-edge/flag-count` state.
   - `RulesProjection` now also keeps a broader high-signal rules subset across `pvp`, `canGameOver`, `coreCapture`, `reactorExplosions`, `schematicsAllowed`, `fire`, `unitAmmo`, `ghostBlocks`, `logicUnitControl`, `logicUnitBuild`, `logicUnitDeconstruct`, `blockWhitelist`, `unitWhitelist`, `winWave`, `unitCap`, `disableUnitCap`, `defaultTeam`, `waveTeam`, `initialWaveSpacing`, and the previously landed economy/wave multipliers.
   - `ObjectivesProjection` now also keeps lightweight objective metadata such as `hidden`, `details`, `completionLogicCode`, `team_id`, and `flagsAdded/flagsRemoved` counts in addition to the earlier target/count/text/position subset.
   - malformed `setRules` / `setObjectives` / `setRule` payloads now leave explicit parse-failure observability in session state instead of remaining silent ignore-only cases.
   - `constructFinish` now minimally parses and tracks the full appended `TypeIO` config object instead of only its leading kind byte.
   - inbound `tileConfig` now decodes as an explicit minimal event/state path with `TypeIO` object tracking plus parse-fail observability for unsupported/trailing object cases.
   - `clientPacket* / clientBinaryPacket*` now support session-level handler registration+dispatch plus minimal outbound queue APIs in `client_session`; inbound `serverPacket* / serverBinaryPacket*` now also feed the same minimal event/state/handler chain; and inbound `clientLogicData*` now has explicit event/state tracking plus session-level channel handler registration/dispatch, minimal outbound queue coverage, `--watch-client-logic-data`, and `--print-client-packets` summaries. The online harness now also schedules outbound `clientPacket*` / `clientBinaryPacket*` / `clientLogicData*` via dedicated `--action-*` flags, though Java-equivalent broader business-handler parity remains follow-up work.
   - `clientSnapshot` build plan config encoding now includes `String` / `byte[]`, and the queue cap now respects the Java `String` / `byte[]` payload budget guard as well as the `20` plan hard cap.
   - `BuilderQueueProjection` now also follows Java `BuilderComp.addBuild(...)` more closely on same-tile dedupe by treating `(x,y)` as the effective replacement key instead of allowing separate place/break rows for one tile in the Rust queue view, and it now preserves a minimal ordered queue-head projection for runtime observability.
    - outbound queue/runtime wiring now explicitly includes `commandUnits` together with `requestItem`, `requestUnitPayload`, `unitClear`, `unitControl`, `unitBuildingControlSelect`, `buildingControlSelect`, `clearItems`, `clearLiquids`, `transferInventory`, `requestBuildPayload`, `requestDropPayload`, `dropItem`, `rotateBlock`, `tileConfig`, `tileTap`, `deletePlans`, and `commandBuilding`.
    - inbound command/control package #8 `unitBuildingControlSelect` now has pure observability across `ClientSessionEvent`, `SessionState`, runtime HUD counters, and online `--print-client-packets` summaries. This is the completion of the first command/control observability batch.
    - inbound `setPlayerTeamEditor`, `menuChoose`, `textInputResult`, and `requestItem` now also have explicit observability decode paths; this decode batch is compatible with server->client `player`-prefixed variants, and observability decode for `buildingControlSelect`, `unitControl`, `requestBuildPayload`, `requestUnitPayload`, `transferInventory`, `rotateBlock`, and `deletePlans` has been regression-revalidated.
    - inbound `copyToClipboard`, `openURI`, and `textInput` (6/7-arg) now also have explicit observability decode paths with runtime HUD counters and online `--print-client-packets` summaries.
    - HUD/UI notice observability (`setHudText`, `setHudTextReliable`, `hideHudText`, `announce`, `infoMessage`, `infoToast`, `warningToast`, `infoPopup` with/without `id`, `infoPopupReliable` with/without `id`, `copyToClipboard`, `openURI`), menu lifecycle observability (`menu`, `followUpMenu`, `hideFollowUpMenu`, `textInput` 6/7-arg), and resource mirror observability (`setItem`, `setItems`, `setLiquid`, `setLiquids`, `setTileItems`, `setTileLiquids`) are now explicitly covered across decode/event/state, runtime HUD counters, and online `--print-client-packets` summaries.
    - `textInput.message` HUD slice is now carried by structured runtime UI (`hud.runtime_ui.text_input.last_message`) instead of being packed into legacy `runtime_ui_menu` compact text; `runtime_ui_menu` intentionally keeps compact menu/text-input counters and omits free-form `message` payload text.
    - world-label observability (`label` 4/5-arg, `labelReliable` 4/5-arg, `removeWorldLabel`) is now explicitly covered across decode/event/state, runtime HUD counters (`runtime_world_label`), and online `--print-client-packets` summaries.
    - Gameplay-signal observability (`setFlag`, `gameOver`, `updateGameOver`, `sectorCapture`, `researched`) is now explicitly covered across decode/event/state and online `--print-client-packets` summaries.
    - `mdt-typeio` now covers raw `Content`, `IntSeq`, `Team`, `LAccess`, `UnitCommand`, Java-correct `int[]` short-length semantics, and `Point2[]`, reducing `TypeIO.readObject` drift for the currently landed subset.
    - `mdt-typeio` now also exposes `TypeIoObject::effect_summary` / `effect_summary_bounded` for stable effect-data kind strings plus first semantic-parent/position hints, and `mdt-client-min` now uses that summary output for effect-data kind labeling.
    - Evidence: `rust/mdt-client-min/src/render_runtime.rs:974-999`, `:1091-1104`, `:2800-2823`; `rust/mdt-render-ui/src/window_presenter.rs:446-465`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/session_state.rs`; `tests/src/test/java/ApplicationTests.java`; `tests/src/test/resources/control-packet-goldens.txt`; local run `cargo test --manifest-path .\\rust\\mdt-client-min\\Cargo.toml client_session::tests`; local run `./gradlew -PnoLocalArc :tests:test --tests ApplicationTests.controlPacketGoldenSamples --tests ApplicationTests.controlPacketKickAndPingReadbackAssertions`
8. `resolved` Transitional fixture fallback cleanup is closed at R+2 canonical-only policy:
   - Plan/report: `audit/transitional-fixture-cleanup.md`
   - Canonical policy marker: `release_prereq_check: ... fixture_policy=canonical_only`
   - Release verify marker: `verified_windows_release_set: ... fixture_policy=canonical_only ...`
   - Transitional switches/waiver flow removed from release-facing scripts.
   - Explicit transitional fixture path usage now hard-fails in release scripts.
   - Evidence: `tools/check-mdt-release-prereqs.ps1:21-28`, `:39-47`
   - Evidence: `tools/verify-mdt-client-min-release-set.ps1:53-60`, `:128-137`, `:306`
   - Evidence: `tools/package-mdt-client-min-online.ps1:95-99`, `:115-120`
   - Evidence: `tools/WINDOWS-RELEASE.md:60-69`
9. `resolved` Remote manifest source vs fixture mirror relationship is explicitly documented for release operations:
   - Generated build artifact source: `build/mdt-remote/remote-manifest-v1.json`
   - Canonical release fixture mirror: `fixtures/remote/remote-manifest-v1.json`
   - Release packaging uses canonical-first candidate order and rejects transitional `rust/fixtures/...` paths at R+2.
   - Evidence: `tools/package-mdt-client-min-online.ps1:97-99`, `:115`, `:128`
   - Evidence: `tools/check-mdt-release-prereqs.ps1:39-41`
10. `resolved` Build/codegen minimal viable chain is connected:
   - Gradle tasks can generate `build/mdt-remote/remote-manifest-v1.json`
   - Gradle codegen refreshes both `rust/mdt-client-min/src/generated/remote_registry_gen.rs` and `rust/mdt-client-min/src/generated/remote_high_frequency_gen.rs`

## Release Claim Boundary

- Can publish now with this claim:
  - `Rust minimal compatibility client release chain is working`
  - `Release scripts, preflight, and split-workspace verification are available`
- Cannot publish now with this claim:
  - `Rust client already implements all original Mindustry parts`
  - `Rust client is fully feature-parity with Java desktop client`

## Parallel Work Queue (Subagent Ready)

### Track A: Final Publish/No-Publish Checklist
- Objective: close audit with severity-sorted decision list.
- Status: `resolved`
- Closure evidence:
  - `RUST_RELEASE_AUDIT_FINDINGS.md` contains frozen checklist with owners/commands/markers/failure handling.
  - `RUST_RELEASE_AUDIT_FINDINGS.md` contains explicit `Final Decision Block (Frozen)` (`YES (conditional)` + `NO` conditions).

### Track B: Release Gate Automation
- Objective: reduce manual misses in release flow.
- Deliverables:
  - default preflight integration decision and implementation task list
  - CI execution plan for workspace+preflight+verify
  - release-set verify output markers that keep canonical-only fixture policy visible
- Done when:
  - required gate commands are explicit and reproducible
  - ownership is assigned for CI landing
  - canonical-only fixture policy markers are stable in prereq + verify output

### Track C: M6/M7/M8/M9 Parity Backlog Split
- Objective: keep parity audit parallelizable without blocking minimal release claim.
- Deliverables:
  - backlog split by `M6 protocol/remote`, `M7 session/network`, `M8 input/gameplay`, `M9 render/UI`
  - per-track acceptance criteria
- Reports:
  - `audit/remote-packet-coverage.md` (P4/M6)
  - `audit/client-snapshot-parity.md` (P5/M7)
  - `audit/runtime-semantic-gap-backlog.md` (cross-cutting semantic backlog for `snapshot` / `effect(data)` / `tileConfig`)
  - `audit/release-unfinished-current-20260324f.md` (latest short-form unfinished lanes after recent landed slices; use this first when assigning new subagents)
  - `audit/input-build-plan-parity.md` (P6/M8)
  - `audit/render-ui-parity.md` (P7/M9)
  - `audit/agent-m6-m9-high-value-gap-audit-20260324.md` (subagent-ready high-value gap split with write-scope/conflict guidance)
  - `audit/m6-m9-subagent-write-lanes-20260324.md` (dispatch-first worker lanes with recommended write scope and explicit conflict boundaries)
  - `audit/transitional-fixture-cleanup.md` (P8)
  - `audit/agent-curie-connect-audit.md` (connect/handshake shard)
  - `audit/agent-galileo-world-audit.md` (world/snapshot shard)
  - `audit/agent-ampere-serialization-audit.md` (serialization/payload shard)
  - `audit/agent-hooke-remote-audit.md` (remote/codegen shard)
  - `audit/agent-kierkegaard-client-audit.md` (client/input/UI shard)
  - `audit/agent-poincare-content-audit.md` (content/gameplay shard)
  - `audit/agent-serialization-gap-refresh-20260324b.md` (serialization/payload refresh shard)
- Manifest path relation (release policy):
  - generated source artifact: `build/mdt-remote/remote-manifest-v1.json`
  - canonical release fixture mirror: `fixtures/remote/remote-manifest-v1.json`
  - transitional `rust/fixtures/...` paths are not accepted by release-facing scripts at R+2
- Current state:
  - M6: small control packets improved (`connect(String,int)` now also has a minimal online-runtime redirect execution chain, `kick(KickReason.serverRestarting)` now also has a minimal online-runtime reconnect execution chain against the current server, `playerDisconnect`, `setCameraPosition`, `sound`, `soundAt`, broader `effect(..., data)` object parsing/storage plus event-level `data_object` carriage, lightweight business projections for the highest-signal effect-data tags (now including entity-table `UnitId`, first-element `Point2[]` / `Vec2[]` position discovery, bounded nested `object[]` discovery, structured `object[]` kind summaries, explicit `ParentRef` handling for `BuildingPos` / `UnitId`, and typed `ContentRef` distinction between raw `Content` and raw `TechNode`), fixed-shape `effect` / `effectReliable`, effect-data parse-fail/trailing observability, `traceInfo`, outbound `adminRequest`, outbound `requestDebugStatus`, inbound `debugStatusClient` / `debugStatusClientUnreliable`, `setRules`, `setObjectives`, `setRule`, `clearObjectives`, `completeObjective`, strict inbound `beginPlace` appended `TypeIO` config tracking, `constructFinish` full appended `TypeIO` config tracking, inbound `tileConfig` minimal event/state tracking with parse-fail observability plus event-level authoritative-apply/rollback flags, explicit configured outcome classification (`Applied` / `RejectedMissingBuilding` / `RejectedMissingBlockMetadata` / `RejectedUnsupportedBlock` / `RejectedUnsupportedConfigType`) for the current low-risk configured-block batch, and a lightweight authoritative building table keyed by `build_pos` that now updates from `blockSnapshot` first-entry fixed-prefix data plus `constructFinish` / `tileConfig` / `deconstructFinish`, while `constructFinish` also seeds the tracked authoritative config map for that building and `deconstructFinish` clears stale config state for removed buildings, `clientPacket*`, `clientBinaryPacket*`, inbound `serverPacket*`, inbound `serverBinaryPacket*`, session-level custom packet handler registration/dispatch plus minimal outbound queue coverage, inbound `clientLogicData*` event/state observability plus session-level channel handler registration/dispatch and minimal outbound queue coverage, and online runtime watch/handler/print wiring via `--watch-client-packet` / `--watch-client-binary-packet` / `--print-client-packets`). Within that still-limited `tileConfig` business slice, landed link-based coverage now includes `bridge-conveyor`, `phase-conveyor`, `mass-driver`, `payload-mass-driver`, and `large-payload-mass-driver`, using the shared `Null` / relative `Point2` / packed absolute `Int` / `BuildingPos` link reconciliation path rather than claiming full Java configured parity for those blocks. `blockSnapshot` fixed-prefix/base coverage now includes `health/rotation/team/ioVersion/enabled/moduleBitmask/timeScale/timeScaleDuration/lastDisabler/legacyConsumeConnected/efficiency/optionalEfficiency/visibleFlags`, and the building table now preserves minimal `block_id/rotation/team/io_version/module_bitmask/time_scale_bits/time_scale_duration_bits/last_disabler_pos/legacy_consume_connected/enabled/config/health/efficiency/optional_efficiency/visible_flags/last_update` facts rather than only `block_id/config/health`. `mdt-typeio` now also covers a much broader object subset across raw `Content`, `IntSeq`, `Point2[]`, raw `TechNode`, `Team`, `LAccess`, `double`, raw `Building`, legacy unit-command-null marker, `boolean[]`, raw `Unit`, `Vec2[]`, `UnitCommand`, and Java-correct `int[]` short-length semantics. Inbound command/control families now also have a first pure-observability pass for `buildingControlSelect` / `unitBuildingControlSelect` / `unitClear` / `unitControl` / `commandBuilding` / `commandUnits` / `setUnitCommand` / `setUnitStance` across `ClientSessionEvent`, session-state counters/last-values, runtime HUD count labels, and online `--print-client-packets` summaries; this `unitBuildingControlSelect` landing completes the first command/control observability batch. Remaining outliers are Java-level `effect(..., data)` business semantics, fuller `tileConfig` business semantics/rollback parity, and broader custom-handler business parity.
  - M6 additive packet-slice update: inbound `takeItems` / `transferItemTo` / `transferItemToUnit` / `payloadDropped` / `pickedBuildPayload` / `pickedUnitPayload` / `unitDespawn` now have explicit event/state handling, and outbound coverage now also includes `setUnitCommand` / `setUnitStance`. `takeItems` / `transferItemTo` / `transferItemToUnit` now also feed a bounded unified `resource_delta_projection` (`last kind/item/amount/build/unit/entity target` plus per-family counts) exposed through `runtime_resource_delta`, so the slice is no longer limited to isolated last-packet mirrors.
  - M6 additive authority-cleanup update: inbound `removeTile` now clears the same authoritative per-building surfaces that `deconstructFinish` clears (`building_table_projection`, `tile_config_projection`, `configured_block_projection`) instead of remaining event-only; loading-time low-priority drops now also emit a dedicated `DroppedLowPriorityPacketWhileLoading` event rather than collapsing into generic `IgnoredPacket`.
  - M6 additive observability update: HUD/UI notice (`setHudText` / `setHudTextReliable` / `hideHudText` / `announce` / `infoMessage` / `infoToast` / `warningToast` / `copyToClipboard` / `openURI`), menu lifecycle (`menu` / `followUpMenu` / `hideFollowUpMenu` / `textInput` 6/7-arg), and resource mirror (`setItem` / `setItems` / `setLiquid` / `setLiquids` / `setTileItems` / `setTileLiquids`) now have explicit decode/event/state observability with runtime and `--print-client-packets` visibility; `text_input.message` now uses structured `runtime_ui` observability instead of legacy `runtime_ui_menu` message concatenation.
  - M6 additive typeio-summary update: `mdt-typeio` now exposes bounded `effect_summary` output with stable kind + semantic-parent/position hints, and `mdt-client-min` effect-data kind labels now consume that shared summary surface.
  - M6 additive effect-runtime update: runtime effect overlay binding/position logic is now split into a dedicated `effect_runtime` module, and recent effect markers no longer freeze only the first sampled point; render-time rebinding now resolves `UnitId` / `BuildingPos` / `Point2` / first-element `Point2[]` / `Vec2` / first-element `Vec2[]` hints against the current entity table, local snapshot position, and world-player fallback before drawing the marker.
  - M6 additive configured-block update: the low-risk configured business batch now also covers `landing-pad` item routing, `door` / `door-large` boolean open-state toggles, `message` / `reinforced-message` / `world-message` trimmed-text projection, `constructor` / `large-constructor` recipe-block projection, `illuminator` int-color projection, and a small link-based block subset: `bridge-conveyor`, `phase-conveyor`, `mass-driver`, `payload-mass-driver`, and `large-payload-mass-driver`. Those paths currently share the same authoritative `constructFinish` / `tileConfig` business entrance, with the link family accepting `Null`, relative `Point2`, packed absolute `Int`, and `BuildingPos`, but this is still a bounded configured-projection slice rather than full Java block-behavior parity.
  - M7: snapshot/session chain is working for minimal release; default cadence now tracks Java more closely and timeout defaults mirror the 30min connect / 20s snapshot-stall baseline. Rust now minimally applies/tracks `stateSnapshot` header, decodes `coreData` into a lightweight `team -> items` projection in session state, derives a business/runtime projection for `wave/enemies/paused/gameOver/timeData/tps/rand/core inventory` plus lightweight apply semantics (`Playing/Paused/GameOver` state transitions, wave-advance from/to, `timeData` delta/rollback/apply counters, and compact core-inventory total/nonzero/changed-team summaries), and now also keeps a separate session-authoritative mirror that always applies state headers, preserves last-good core inventory on malformed `coreData`, tracks `gameOver > paused > playing` precedence plus wave-advance-only-on-increase semantics, exposes core-parse-fail counters, and is reset on `worldDataBegin`; runtime HUD now prefers that authority mirror for snapshot labels. Rust records both packet-parse and `coreData`-parse failures, adds `entitySnapshot` envelope/apply observability plus a minimal local-player entity table row (`entity_id -> class/local/unit/position/hidden`), batch-upserts any additional parseable `classId=12` player rows into that entity table, and now also parses known-prefix non-player sync rows so alpha-shape `classId=0` (alpha) rows are real-decoded and upserted into the same entity table while the same revision family `classId=29/30/31/33` shares that shape parser path, mech-shape `classId=4` plus same-family `classId=17/19/32` now follow a second real parse+upsert path, missile-shape `classId=39` now follows a third real parse+upsert path with explicit `lifetime/time` field parsing, and payload-shape `classId=5/23/26/36` now follows a fourth real parse+upsert path for `payloadCount=0` plus loaded-world-context recursive `BuildPayload` consumption and boundary-safe recursive `UnitPayload` body consumption for `payloadCount > 0` across the currently covered vanilla unit revision families (`classId=36` additionally parses the tethered `building` reference); local-player extraction is also hardened so only a unique parseable `player_id + classId(12)` match is accepted while ambiguous/multi-hit payloads are rejected with explicit counters/last-match observability. Rust also adds minimal `blockSnapshot` / `hiddenSnapshot` envelope parse observability (`amount`/`data_len`, `count`/`first_id`) with dedicated parse-failure telemetry, exposes a lightweight `blockSnapshot` head runtime projection plus first-entry fixed-prefix diagnostics (`health/rotation/team/ioVersion/enabled/moduleBitmask/timeScale/timeScaleDuration/lastDisabler/legacyConsumeConnected/efficiency/optionalEfficiency/visibleFlags`), and now prevents conflicting `blockSnapshot` head data from overwriting an already tracked authoritative building row at the same `build_pos` when `block_id/team/io_version` disagree, surfacing dedicated conflict-skip telemetry instead. When a loaded world bundle is available, Rust can now additionally parse and apply later `blockSnapshot` entries beyond the first by using the loaded-world block name + revision context to recover safe entry boundaries, but that incremental path still only updates the lightweight building table's base fields rather than claiming full Java `tile.build.readSync(...)` behavior. `hiddenSnapshot` is treated as a one-shot trigger for the current payload's IDs instead of a persistent hidden set, now also applies hidden flags into that entity table and performs a minimal lifecycle removal for non-local hidden rows, and the lightweight authoritative building table keyed by `build_pos` (`block_id/rotation/team/io_version/module_bitmask/time_scale_bits/time_scale_duration_bits/last_disabler_pos/legacy_consume_connected/enabled/config/health/efficiency/optional_efficiency/visible_flags/last_update`) survives across `blockSnapshot` / `constructFinish` / `tileConfig` / `deconstructFinish` / `buildHealthUpdate`. Runtime HUD/refresh surfaces snapshot summaries plus state/effect business-apply labels, the building-table summary label, block fixed-prefix diagnostics, and explicit config / config-rollback markers for authoritative `tileConfig`, world-stream begin packets enforce a hard size limit before allocation, the ready-state snapshot timeout anchor is armed on connect-confirm and refreshed by `EntitySnapshot` only, and Java's `clientLoaded` load gate is now approximated by deferring/replaying normal-priority inbound packets during active world-data load while dropping low-priority inbound packets and clearing the deferred queue on `worldDataBegin`. `worldDataBegin` now also clears lightweight rules/objectives projections, the entity table, the builder queue projection, the building table, `tile_config_projection`, and both snapshot business + authority state so a new world does not inherit prior-world semantics. The main remaining gap is broader snapshot application depth and full live-world/system parity.
  - M7 additive semantics update: `hiddenSnapshot` now explicitly tracks both latest hidden-id set and real delta (`added` / `removed` versus previous payload), with runtime HUD labels for both current and delta state (`runtime_hidden`, `runtime_hidden_delta`).
  - M7 additive hidden-lifecycle update: hidden apply is now centralized on `SessionState::apply_hidden_snapshot(...)`, and non-local hidden ids now also prune orphan rows from the lightweight semantic/resource/payload lifecycle tables instead of only removing the primary entity-table row. This remains bounded table cleanup, not a claim of Java live-world ownership parity.
  - M7 additive observability update: runtime HUD now also exposes compact audio/admin counters (`runtime_audio`, `runtime_admin`) together with loading/snapshot gate counters (`runtime_loading`) so `sound` / `soundAt`, `traceInfo`, `debugStatusClient*`, deferred/replayed inbound packets, dropped loading-time low-priority packets, and state/entity snapshot parse failures are visible without deep logs. Truncated `sound` / `soundAt` / `traceInfo` / `debugStatusClient*` payloads now also increment dedicated session-state parse-fail counters, and the audio/admin HUD labels surface those counts directly.
  - M7 additive hardening update: the loaded-world `blockSnapshot` extra-entry path is now fail-closed and only batch-applies later entries when the entire loaded-world parse succeeds and the reconstructed first entry still matches the already parsed head projection; `entitySnapshot` now also keeps a short-lived tombstone guard for recently removed entity IDs so stale immediate-following player-row snapshots do not instantly recreate entities just removed by `unitDespawn` / `unitEnteredPayload` / `playerDisconnect` / hidden lifecycle.
  - M7 additive hardening update: `stateSnapshot` now explicitly retains `last_good_state_snapshot_core_data` and runtime summary surfaces use this as a `last_good` fallback when current `coreData` parse fails; `entitySnapshot` parseable-player-row extraction now fail-closes when parseable rows exceed declared envelope `amount`; and hidden non-local entity IDs now continue blocking `entitySnapshot` re-upsert while they remain hidden.
  - M7 additive entitySnapshot slice update: known-prefix non-player entity-row parsing is now wired into the same entity-table apply path, with landed real parse+upsert support for alpha-shape `classId=0` (`alpha`) plus same-shape revision family coverage for `classId=29/30/31/33`, mech-shape `classId=4` plus same-shape revision family coverage for `classId=17/19/32`, missile-shape `classId=39`, and payload-shape `classId=5/23/26/36` for `payloadCount=0` plus loaded-world-context recursive `BuildPayload` consumption for `payloadCount > 0`.
  - M7 additive entitySnapshot slice update: fixed-shape environment entities `Fire` (`classId=10`), `Puddle` (`classId=13`), `WeatherState` (`classId=14`), and bounded string-shape `WorldLabel` (`classId=35`) are now parsed from the same known-prefix chain and upserted into the minimal entity table.
  - M7 additive entitySnapshot slice update: `BuildingComp` (`classId=6`) now has a bounded parse/apply closure under a fail-closed loaded-world gate. Rust only accepts rows whose `entity_id` can be matched back to a known loaded-world building center as `build_pos`; on success it upserts a minimal building entity row into the lightweight entity table, refreshes the existing authoritative building-table head through `apply_block_snapshot_head(...)`, and reuses the loaded-world parsed-tail business helpers for already-supported high-signal configured/resource projections such as `message`, `payload-router`, `payload-source`, `duct-unloader`, `reconstructor`, `canvas`, `payload-mass-driver`, `sorter` / `inverted-sorter` / `unloader` / `duct-router`, `bridge-conveyor` / `phase-conveyor`, `illuminator`, and `switch` / `world-switch` / `door` / `door-large`. This remains a conservative compatibility path rather than a Java `tile.build.readSync(...)` parity claim, and should be treated as extra compatibility breadth rather than required current-vanilla `Groups.sync` coverage.
  - M7 additive entitySnapshot safety update: payload-family parsing is now split by recursive payload kind; `BuildPayload` entries inside `payloadCount > 0` are boundary-consumed when loaded-world `content_header` context can resolve the block name, while recursive `UnitPayload` bodies now use a bounded `unit.read(...)` consume parser for the covered vanilla class/revision families; unknown unit class-id families or unresolved build-payload block mappings remain explicit fail-closed (including tether-payload shape).
  - M7 additive payload-family precision update: `classId=5/23/26` no longer share one merged payload-legacy body parser. Rust now has explicit `mega`, `oct`, and `quad` consume layouts, which closes the hidden early-revision drift where `oct` revision 1 carries `flag` without `mineTile`, `quad` revisions 0..2 postpone `mineTile`, and `quad` revision 6 adds `abilities`.
  - M7 additive regression-fixture update: Java tests now emit a dedicated `tests/src/test/resources/unit-payload-goldens.txt` resource with real serialized `UnitPayload` bodies for `alpha`, `flare`, `mono`, `poly`, `mace`, `mega`, `quad`, `oct`, `manifold`, `quell-missile`, `spiroct`, `stell`, `vanquish`, `elude`, and `latum`; Rust `mdt-world` now parses that resource directly in tests so parser coverage is anchored to current Java serialization rather than synthetic bodies alone.
  - M7 additive integration-fixture update: `mdt-client-min` now also consumes `unit-payload-goldens.txt` in `entitySnapshot` prefix-parser regressions and high-level session/entity-table apply regressions for both payload rows and building-tether payload rows, so mixed payload-row recovery is no longer validated only with Rust-side synthetic nested bodies.
  - M7 additive payload-gate regression update: Rust tests now also pin explicit current-vs-legacy compatibility IDs for payload-family candidate mapping (`4/25`, `40/43`, `46/47`), so compatibility-only IDs remain intentional rather than accidental drift.
  - M7 audit correction update: current Java vanilla `entitySnapshot` emission is bounded by generated `Syncc` classes in `Groups.sync`, not the full `EntityMapping` table. A generated-Java guard test now confirms that the current Rust family set already covers every active vanilla `Syncc` `classId`, while the only extra accepted ids are compatibility-only `6/40/44`.
    - evidence shard: `audit/entity-snapshot-syncc-refresh-20260324e.md`
  - M7 current primary entitySnapshot risk: recursive payload rows are no longer blocked on `UnitPayload` body boundary recovery; the remaining entity-row risk is now lightweight-apply depth, legacy-id hygiene, unknown/modded unit class ids, and the fact that parsed rows are still applied only into the lightweight entity/building projections rather than full Java live-world systems.
  - M7 additive entity-status hardening update: five covered entity sync paths (`alpha` / `mech` / `missile` / `payload` / `building-tether payload`) no longer hard-fail on `status_count > 0`; Rust now structurally consumes `status_id + duration` entries and, when a `content_header` is available, also consumes dynamic-status extra `f32` payloads according to the bitflag width. This closes the old immediate fail path for many real packets while still remaining conservative rather than claiming Java-equivalent status semantics.
  - M7 additive load-gate parity update: normal-priority inbound packets are no longer hard-capped to 256 entries during active world-data load; Rust now keeps Java-closer queue+replay semantics for this path while still dropping `priorityLow` packets during loading and clearing stale deferred packets on `worldDataBegin`.
  - M7 additive block/state update: `blockSnapshot` world-apply responsibility is now unified on the `client_session + loaded_world` path instead of split between `snapshot_ingest` head-apply and `client_session` extra-entry apply. `snapshot_ingest` now keeps envelope/head observability only, while the session-side loaded-world path can apply all parsed entries on clean parse, rejects partial prefixes on later-entry parse failure, and still fail-closes on outer trailing-byte drift. In parallel, `stateSnapshot` now also seeds an explicit `authoritative_state_mirror` runtime-facing field (currently mirroring the authority projection) that is cleared on `worldDataBegin` and preferred by runtime HUD surfaces when present.
  - M7 additive world-tail update: `mdt-world` now parses `message` / `reinforced-message` / `world-message` building tails into structured `MessageTailSnapshot { message }` values from direct tail bytes and world-chunk snapshots, and now also parses `payload-router` / `reinforced-payload-router` building tails into structured typed tail snapshots carrying `sorted` mixed content refs plus `recDir` and bounded carried-payload summaries. The loaded-world consumer side in `mdt-client-min` now also applies the already-parsed tail/base fields that map onto existing configured/resource projections for `constructor.recipe_block_id`, `landing_pad.config_item_id`, `message`, `payload-source` content refs, `payload-router` sorted content refs, `duct-unloader.item_id`, `reconstructor.command`, `canvas.data_bytes`, `payload-mass-driver.link`, `nullableItemRef` item families (`sorter` / `inverted-sorter` / `unloader` / `duct-router`), `bridge-conveyor` / `phase-conveyor` links, `illuminator` light color, and `switch` / `world-switch` / `door` / `door-large` boolean state. This narrows a broader loaded-world parser-to-runtime gap without claiming Java-equivalent live building semantics.
    - remaining low-risk loaded-world consumer targets are the still-parsed fields that do not yet feed configured/resource projections: broader `item/mass-driver` links outside the already landed payload-mass-driver + item-bridge subsets, and `base.item_module.entries`
  - M7 additive applied-event update: parse-success `stateSnapshot` packets now emit a dedicated `StateSnapshotApplied` event carrying wave/gameplay/rollback/core-fallback semantics, `--print-client-packets` now exports that summary directly, and runtime HUD additionally surfaces a compact `runtime_snap_apply=...` label so snapshot semantic change is distinguishable from plain refresh counting.
  - M7 additive gameplay-signal update: `stateSnapshot` wave increases now also record an independent live wave-advance signal (`count/from/to/apply_count`) on top of the existing applied projection, matching the Java-side distinction between header apply and wave-advance gameplay signaling more closely; equal/regressed waves do not increment it, `worldDataBegin` clears it, and runtime HUD now exposes the latest signal state inside `runtime_gameplay_signal`.
  - M8: basic input/build compatibility is working; snapshot cadence and `getMaxPlans` guard baseline are now closer to Java, Rust build-plan config encoding now covers a much broader `TypeIO` subset (`Int` / `Long` / `Float` / `Bool` / `IntSeq` / `Point2` / `Point2[]` / `TechNodeRaw` / `Double` / `BuildingPos` / `LAccess` / `String` / `byte[]` / `LegacyUnitCommandNull` / `boolean[]` / `UnitId` / `Vec2` / `Vec2[]` / `Team` / `int[]` / `object[]` / raw `Content` / `UnitCommand`) in addition to `None`, `mdt-input` `rotatePlans/flipPlans` utilities are now wired into the online CLI path, `rotate_plans` now also normalizes multi-step direction turns, `mdt-input` now also exposes a minimal stateful action-edge mapper (`ActionPressed` / `ActionHeld` / `ActionReleased`) with duplicate-action dedupe plus stable edge ordering across input permutations / multi-release drops, and the online runtime now exposes queued `requestItem`, `requestUnitPayload`, `unitClear`, `unitControl`, `unitBuildingControlSelect`, `buildingControlSelect`, `clearItems`, `clearLiquids`, `transferInventory`, `requestBuildPayload`, `requestDropPayload`, `dropItem`, `rotateBlock`, `tileConfig`, `tileTap`, `deletePlans`, `commandBuilding`, and `commandUnits`. The current `tileConfig` business slice now also handles bounded configured projection for `message` / `reinforced-message` / `world-message`, `canvas` / `large-canvas` strict fixed-length `byte[]` payloads, `constructor` / `large-constructor`, `illuminator`, `payload-source`, `payload-router` / `reinforced-payload-router`, the `PowerNode` family (`power-node` / `power-node-large` / `surge-tower` / `beam-link`) with authoritative link-set full-replace plus absolute toggle semantics, and the reconstructor family, plus the previously landed small single-link subset (`bridge-conveyor`, `phase-conveyor`, `mass-driver`, `payload-mass-driver`, `large-payload-mass-driver`) through shared link decoding, but that should still be read as bounded compatibility work rather than broad configured-behavior closure. Rust now also keeps a minimal builder queue projection driven by `BeginBreak` / `BeginPlace` / `RemoveQueueBlock` / `ConstructFinish` / `DeconstructFinish`, surfacing `Queued` / `InFlight` / `Finished` / `Removed` plus orphan-authoritative counts and a queue-head view in runtime HUD text; same-tile place/break replacement follows Java `BuilderComp.addBuild(...)` more closely by using `(x,y)` dedupe semantics in that queue view; `snapshot_input.building` in `mdt-client-min` now follows this authoritative queue projection instead of staying CLI/override-only when inbound queue packets advance or clear the local build state; and the online harness now has a stable outbound-action script regression so scheduled client events can be replay-checked as a deterministic signature. Full builder/input parity and remaining online-harness integration are still open, but the main remaining delta is Java behavior semantics rather than the old narrow config-type baseline.
  - M8 additive command-units update: Rust now also has Java-like chunk helpers for `commandUnits`, with a default chunk size of `200`, correct `finalBatch` handling on the last chunk only, and empty-input no-op behavior; the online CLI additionally accepts a shorter `--action-command-units ...@queueCommand` form that uses this automatic chunking path instead of forcing manual `finalBatch` management.
  - M8 additive compatibility update: online/runtime outbound actions now also include `setUnitCommand` and `setUnitStance`, and runtime HUD additionally reports inbound counters for `takeItems` / `transferItemTo` / `transferItemToUnit` / `payloadDropped` / `pickedBuildPayload` / `pickedUnitPayload` / `unitDespawn`.
  - M8 additive mapper update: `mdt-input` now also exposes `IntentSamplingMode::LiveSampling` plus `LiveIntentState` semantics (pressed/released edges without repeated held-edge spam while active-action state persists), and the online runtime now carries the `building` bit end-to-end through `PlayerIntent::SetBuilding`, live snapshot sampling, live-intent state tracking, and snapshot writeback so runtime-sampled build-mode state is no longer lost on the `mdt-input -> mdt-client-min-online -> snapshot_input` path.
  - M8 additive builder-activity update: `mdt-input` `BuilderQueueStateMachine` now also exposes bounded local activity/head-selection semantics (`update_local_activity(...)`) modeled on the lowest-risk part of Java `BuilderComp.updateBuildLogic()`: prefer the first `in_range && !should_skip` plan, otherwise fall back to the closest in-range plan only when the current head is out of range, and keep ordering stable when activity observations are incomplete. This is still library-level queue logic rather than end-to-end runtime build execution parity.
  - M9: render/UI remains release-scope excluded or parity backlog, not release-complete.
  - M9 additive presentation update: `mdt-render-ui` now classifies render objects by semantic kind (`player/marker/plan/block/terrain/unknown`) and applies stable floor/clamp/zoom normalization in player-centered window crop sizing/focus math; this is low-risk presentation hardening, not Java desktop UI parity closure.
  - M9 additive HUD-structure update: `mdt-render-ui` `HudSummary` now also carries `overlay_visible`, `fog_enabled`, `visible_tile_count`, and `hidden_tile_count`, so render/presenter code can consume visibility state structurally instead of reparsing status text.
  - M9 additive session-panel update: lifecycle observability for kick/loading/reconnect is now also exposed structurally through `RuntimeSessionObservability`, and both presenters emit dedicated session lines (`RUNTIME-SESSION`, `session:k=...;l=...;r=...`) so reconnect phase/reason, timeout/reset taxonomy, and last world-reload cleanup facts are accessible without reparsing the legacy compact runtime labels.
  - M9 additive build-config update: runtime/UI shaping now also exposes a dedicated read-only rollback strip for build/config authority flow. `render_runtime` lifts `TileConfigProjection` rollback facts into structured `BuildUiObservability.rollback_strip`, and both presenters emit explicit rollback/apply/source/outcome/build-tile state (`BUILD-ROLLBACK`, `cfgstrip`) instead of burying that information only in compact summary text.
  - M9 additive runtime-rich-UI update: `RuntimeUiObservability` now also preserves structured `announce` / `infoMessage` / `infoPopup` / `copyToClipboard` / `openURI` notice data, `menu` / `followUpMenu` metadata, and `menuChoose` / `textInputResult` result fields. `render_runtime` now projects those directly out of `SessionState`, and `mdt-render-ui` panel/window/ascii presenters now emit richer `RUNTIME-NOTICE`, `RUNTIME-MENU`, and `RUNTIME-CHOICE` rows plus detail lines instead of leaving those signals only inside compact runtime status text.
  - Audit precision note: `classId=1/25/40/47` remain legacy compatibility IDs in `classids.properties`, but current vanilla `EntityMapping` uses `24/4/43/46`. Future parity review should treat `1/25/40/47` as compatibility-only paths unless Java runtime mapping changes.
  - Audit precision follow-up: Rust `mdt-world` unit-payload routing now also pins current vanilla `43/45/46` as the active current-shape ids validated against generated `EntityMapping.java` and real `unit-payload-goldens.txt` samples (`stell`, `vanquish`, `elude`, `latum`), while `40/47` remain explicitly treated as legacy compatibility aliases instead of quietly coexisting in the same current-id comment path.
- Done when:
  - tracks are non-overlapping and subagent-ready
  - each track has concrete evidence targets
  - dispatch-first worker lanes remain available in `audit/m6-m9-subagent-write-lanes-20260324.md`

## 2026-03-23 Additional Parity Audit Backlog

## 2026-03-24 Additive Progress Update

- lifecycle/input stabilization update:
  - `mdt-input` command-mode state is now explicit in `rust/mdt-input/src/command_mode.rs`, including selected units/buildings, rect selection, control groups, and last target/command/stance selections; `mdt-client-min-online` runtime outbound action sync now writes into that state instead of keeping only last-packet facts.
  - `mdt-client-min` `mark_client_loaded()` now auto-queues `connectConfirm` through the normal pending-packet path once the world becomes ready, while `prepare_connect_confirm_packet()` reuses queued bytes if the confirm is already pending.
  - lifecycle regression expectations were updated for the new ready-state action ordering, including queued gameplay/chat actions and the standalone UDP driver test surface.
  - verification: `cargo test --manifest-path rust\\mdt-input\\Cargo.toml` and `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml` are green after the command-mode module recovery plus the queued-`connectConfirm` lifecycle/test realignment.
  - verified locally:
    - `cargo test --manifest-path rust\mdt-input\Cargo.toml`
    - `cargo test --manifest-path rust\mdt-client-min\Cargo.toml`
  - current status: `mdt-input` `52` tests green; `mdt-client-min` `412 + 141` tests green after the lifecycle/command-mode/hiddenSnapshot stabilization pass.

- `mdt-world` additive world-tail closure:
  - `duct-unloader` now parses as structured `DuctUnloaderTailSnapshot { item_id, offset }` instead of falling back to `Unknown`.
  - `memory-cell` / `memory-bank` now parse as structured `MemoryTailSnapshot { len, values_bits }`, preserving raw `double` bit patterns for fail-closed parity-safe inspection rather than speculating on higher-level memory semantics.
  - `canvas` / `large-canvas` now parse as structured `CanvasTailSnapshot { data_len, data_sha256, data_bytes }`, preserving raw bytes and bounded hash evidence instead of staying opaque.
  - The dispatcher, `building_tail_kind`, and world summaries now recognize those three low-risk families, and `mdt-world` tests cover positive + truncated-tail rejection paths.

- `mdt-client-min` additive runtime observability closure:
  - `runtime_command_ctrl` is no longer a counters-only HUD slice. It now carries bounded last-target / tile / pos-bits / batch metadata for the already-covered command/control family (`setPlayerTeamEditor`, `menuChoose`, `textInputResult`, `requestItem`, `buildingControlSelect`, `unitControl`, `unitBuildingControlSelect`, `commandBuilding`, `commandUnits`, `setUnitCommand`, `setUnitStance`, `rotateBlock`, `transferInventory`, `requestBuildPayload`, `requestDropPayload`, `requestUnitPayload`, `dropItem`, `deletePlans`, `tileTap`).
  - This keeps the current `minimal compatibility client` boundary while making command/control debugging materially more useful without deeper Java business semantics.

- `mdt-client-min` / `mdt-render-ui` additive live-summary closure:
  - `RuntimeUiObservability` now carries a structured `live` DTO split into `entity` and `effect` summaries instead of forcing presenters to infer this only from compact status text.
  - `entity` summary now exposes bounded current entity totals, hidden totals, local entity/unit identity, recent entity-snapshot count, and latest local position bits.
  - `effect` summary now exposes bounded recent effect totals, spawn-effect totals, last effect id/kind/contract, and a best-effort recent position hint with explicit source precedence (`business projection` -> `effect packet` -> `spawn effect packet`).
  - ASCII/window presenters now surface this live summary in `RUNTIME-UI` / frame-status output without widening pixel-level rendering claims.
  - ASCII/window presenters now also expose dedicated live-summary slices instead of only the packed runtime UI line:
    - ASCII: `RUNTIME-LIVE-ENTITY:` and `RUNTIME-LIVE-EFFECT:`
    - window status text: `liveent:` and `livefx:`
  - This remains presenter-only UI deepening on top of existing structured observability, not a new runtime/state claim.

- `mdt-client-min` additive `effect(..., data)` contract closure:
  - typed `effect_id -> contract` selection is now wired into business projection instead of leaving all decoded `TypeIoObject` payloads on the same generic DFS heuristic.
  - `position_target` effects now ignore unrelated nested content refs and project explicit `PositionTarget { source, target }` semantics from the effect packet origin to the resolved position-like payload.
  - `float_length` effects now project explicit `LengthRay { source, target, rotation, length }` semantics from the effect packet origin plus decoded float payload instead of reducing those packets to generic `FloatValue`.
  - `item_content` and `unit_parent` contracts now hard-reject mismatched decoded payload families instead of accepting unrelated generic projections.
  - runtime effect HUD/live-summary target-position extraction now covers `PositionTarget` / `LengthRay`, so contract-specific effect semantics remain visible without over-claiming full Java effect executor parity.

- `mdt-client-min-online` additive custom/logic semantic-consumer closure:
  - online runtime now supports bounded semantic consumer flags for already-landed custom packet families:
    - `--consume-client-packet <type@semantic>`
    - `--consume-client-binary-packet <type@semantic>`
    - `--consume-client-logic-data <channel@semantic>`
  - the new isolated `custom_packet_runtime` helper turns watched custom/binary/logic payloads into stable semantic lines and summaries instead of raw preview-only watch output.
  - the current low-risk semantic set includes `server-message`, `chat-message`, `hud-text`, `announce`, `clipboard`, `open-uri`, `world-pos`, `build-pos`, `unit-id`, `team`, `bool`, and `number`.
  - redirect/restart reconnect paths now re-install these semantic consumers so runtime behavior remains stable across reconnect without widening `ClientSession` state-machine scope.
  - this stays within the current `minimal compatibility client` boundary: semantic consumers are runtime/debug surfaces layered on top of existing handler registration and do not claim full Java custom-packet business parity.
  - online runtime now also supports bounded relay flags on top of the same custom/logic packet surface:
    - `--relay-client-packet <inbound@outbound@reliable|unreliable>`
    - `--relay-client-binary-packet <inbound@outbound@reliable|unreliable>`
    - `--relay-client-logic-data <inbound@outbound@reliable|unreliable>`
  - the new `custom_packet_runtime_relay` helper can translate matching inbound custom/logic packets into existing outbound queue actions, and reconnect/redirect paths re-install the relay hooks after session rebuild.
  - this remains harness/runtime behavior, not a claim of full Java mod/plugin packet business parity.

- `mdt-render-ui` additive presenter closure:
  - ASCII/window presenters now consume more of the existing structured HUD/runtime projection instead of relying only on the giant compact `status_text` string.
  - `AsciiScenePresenter` now emits explicit `SUMMARY:` and `RUNTIME-UI:` lines from `HudSummary` / `RuntimeUiObservability`.
  - `WindowPresenter` now appends compact structured HUD-summary state alongside runtime UI slices in frame status text.
  - This is a low-risk M9 slice: presentation-side consumption only, no protocol or session-apply widening.

- `mdt-input` additive local queue primitive:
  - `BuilderQueueStateMachine` now exposes bounded local-only helpers `move_to_front(x, y, breaking)` and `remove_local_entry(x, y, breaking)`.
  - These helpers intentionally do not mutate authoritative counters (`finished/rejected/orphan`) so UI/local-plan edits stay distinct from server authoritative transitions.
  - They are groundwork for later M8 queue-selection / exact local removal behavior, not full Java desktop input parity.
  - Same-tile `block_id` inheritance is now fail-closed across `place <-> break` mode switches in both sync and `mark_begin(...)` paths, so the Rust builder queue no longer leaks stale block identity when Java would treat the opposite-mode row as a replacement.

- `mdt-input` additive command-mode groundwork:
  - `mdt-input` now exposes a bounded local `CommandModeState` / `CommandModeProjection` abstraction with explicit `active` state plus recent target / command / stance selection tracking.
  - Target projection is intentionally minimal (`build`, `unit`, `position bits`) and local-only; it does not claim Java `InputHandler` parity yet.
  - This is groundwork for later command-mode runtime wiring and UI/presenter consumption, not a complete command-mode business implementation.

- `mdt-world` additive save-envelope groundwork:
  - `mdt-world` now exposes passive `.msav` envelope observability via `read_msav_envelope(...)`.
  - The new observation path recognizes the outer zlib envelope, validates the `MSAV` header, records `save_version`, tracks compressed/uncompressed lengths, and reports the first region-length field without claiming full save-version dispatch or live save parity.
  - Tests cover success, wrong-header rejection, and truncated-envelope rejection.

- `mdt-world` additive save-entities groundwork:
  - `mdt-world` now also exposes passive save-region observability via `parse_msav_save(...)` and `parse_save_entity_region(...)`.
  - The current slice dispatches save regions by `save_version`, records `entities` region structure, parses remap table entries plus team/world entity chunks, and distinguishes legacy `Save6-9` `u16` chunk lengths from `Save10-11` `u32` chunk lengths.
  - This remains parser-only groundwork for later save-region/post-load semantics; it does not claim live-world apply parity.
  - On top of that parser layer, Rust now also exposes `SaveEntityPostLoadSummary` helpers so later post-load checks can directly inspect builtin/custom/unknown class counts, per-`class_id` summaries, resolved builtin/custom names, and duplicate `entity_id` collisions without hand-walking raw chunk bytes.
  - `SavePostLoadWorldObservation` now also carries the parsed remap/world-entity observation from the `entities` region, and `SaveEntityRemapSummary` explicitly reports duplicate `custom_id` / `name` collisions so later post-load diagnostics can consume this without re-walking raw region bytes.

- `mdt-remote` additive typed-registry closure:
  - `mdt-remote` now exposes a full read-only typed metadata surface via `RemotePriority`, `TypedRemoteParamMetadata`, `TypedRemotePacketMetadata`, `RemotePacketRegistry`, and `typed_remote_packets(...)`.
  - Manifest validation is stricter: unknown `targets` / `priority`, empty packet metadata fields, and empty param name/type now fail closed instead of silently drifting.
  - High-frequency snapshot helpers keep their old API shape but now reuse the full registry internally, and `mdt-client-min` snapshot packet registry now consumes that typed registry instead of directly scanning raw manifest rows.
  - `mdt-remote` now also exposes typed inbound family metadata for `serverPacket*` / `serverBinaryPacket*` / `clientLogicData*`, and `mdt-client-min` `packet_registry` now consumes that helper directly instead of its old local stringly `remote_lookup` path. Decoy overload regressions pin signature-first matching, and the dead local `remote_lookup.rs` glue has been removed.
  - `mdt-remote` now also exposes a broader typed `CustomChannelRemoteFamily` surface for all `10` custom/logic channel families (`clientPacket*`, `clientBinaryPacket*`, `serverPacket*`, `serverBinaryPacket*`, `clientLogicData*`), with one selector source for method/flow/unreliable/param-shape matching.
  - `mdt-client-min` `packet_registry` now builds a full `CustomChannelPacketRegistry`, derives the older inbound subset from it, and `client_session` now uses that typed registry for custom-channel packet-id classification plus runtime decode dispatch instead of the old hand-written packet-id chain.

- `mdt-client-min` additive hidden-lifecycle hardening:
  - non-local entities hidden by the latest `hiddenSnapshot` set no longer revive on a later `entitySnapshot` while their IDs remain hidden.
  - Rust now keeps this as an explicit entity-snapshot gate beside the short-lived despawn tombstone gate, and targeted regressions cover both `hidden while active => blocked` and `hidden cleared => allowed again`.
  - This is still bounded lifecycle hardening, not full Java `Groups.sync` parity.

- `mdt-client-min` additive `stateSnapshot.coreData` semantics hardening:
  - repeated `coreData` fold logic is now centralized in `state_snapshot_semantics`.
  - duplicate `team/item` keys now consistently follow last-write-wins not only for the stored per-team inventory map, but also for derived metrics such as total amount, nonzero item count, and item-entry count; duplicate-key telemetry remains preserved separately.
  - this is still bounded semantic cleanup on already parsed bytes, not a new lifecycle or live-world claim.

- `mdt-client-min` additive tile-config rollback hardening:
  - parse-failed `tileConfig` packets now perform a bounded fallback rollback to the last known authoritative config when the same `build_pos` still has pending local intent plus a previously known authoritative value.
  - This closes one concrete authority-loss branch where Rust previously dropped pending local intent without re-aligning runtime/business state to the last known server authority.
  - This remains conservative fallback behavior; it does not claim full Java `InputHandler.configTap(...)` or `Building.config()` parity.

- `mdt-world` additive post-load world observation:
  - `MsavSaveObservation` now exposes `post_load_world()` and the new `SavePostLoadWorldObservation` / `SaveMapRegionObservation` surface so passive `.msav` parsing can summarize world-init facts beyond the earlier entity-only post-load summary.
  - The current slice passively joins `content`, `patches`, `map`, `markers`, `custom`, and `entities` regions into one post-load observation model, reusing the existing building/marker/custom parsing helpers and a shared save-chunk-length reader for both legacy `save6-9` and newer `save10+` region formats.
  - `post_load_world()` now also carries the parsed world-entity chunk list and remap-duplicate summary (`custom_id` / `name` collisions), so later diagnostics or deeper restore work can consume those observations without re-walking raw `entities` region bytes.
  - This is still parser/observation groundwork for later `loadWorld` parity, not a claim of live-world apply closure.
  - Entity remap/post-load observation is widened again:
    - save entities now classify post-load effective kind as `Builtin`, `RemappedBuiltin`, `UnresolvedCustom`, or `Unknown`
    - chunk summaries now report post-load target class/name plus whether Java-style `readWorldEntities` would load or skip that row
    - remap summaries now expose last-wins effective custom-id resolution, including which remaps end on builtin targets versus unresolved custom targets
    - entity post-load summaries now include explicit `loadable/skipped` counts plus aggregation by post-load effective kind
  - This remains post-load observability only; it does not claim Java entity instantiation/add-to-groups parity.

- additive audit-doc refresh:
  - `audit/agent-m6-m7-high-value-gap-refresh-20260324.md` freezes the current `M6/M7` priority order after the latest landing batch.
  - `audit/m8-m9-ui-gap-audit-20260324.md` freezes the current `M8/M9` high-value UI/input gaps so later workers can dispatch without re-auditing the same surface.
  - `audit/agent-parallel-refresh-20260324c.md` freezes the latest low-conflict worker lanes across `mdt-typeio`, `mdt-world`, `M7 snapshot semantics`, `M6 typed remote`, and presenter-only `M9` follow-ups so later parallel dispatch can start from current boundaries.

- `mdt-client-min` additive remote observability closure:
  - `createBullet`, `destroyPayload`, and `transferItemEffect` no longer fall through to generic ignored-packet handling.
  - Rust now binds those packet ids from the manifest, performs decode-only event/state dispatch, stores bounded last-packet projections in `SessionState`, and surfaces them through `event_summary` output without claiming deeper gameplay semantics.

- `mdt-input` additive capability-gate groundwork:
  - `mdt-input` now exposes a transport-agnostic `CapabilityGate` with structured `CapabilityContext`, `CapabilityDecision`, `CapabilityDenyReason`, `CapabilityBuildRequest`, and `CapabilityCommandRequest`.
  - The current slice fail-closes obvious illegal local actions (`missing/dead controlled unit`, disabled mining/building/command, missing build block, inactive command mode, empty command target) while preserving explicit `None` command/stance selection as a valid follow-up path.

- `mdt-typeio` additive basic-codec centralization:
  - `mdt-typeio` now also exposes basic read-side helpers for non-object codecs (`bool/byte/short/int/float/string/block/content/team/tile/unit-null/vec2/rules-json`) together with `unpack_point2(...)`.
  - This is low-risk codec centralization groundwork for later reuse across crates; it narrows one architecture gap without claiming Java-wide `TypeIO` parity.
  - `mdt-typeio` now also centralizes the raw length-prefixed JSON codec surface for `rules`, `objectives`, and `objective-marker`, with shared read/write helpers, round-trip/negative-length regressions, and typeio golden coverage for `objectives.basic` plus `objectiveMarker.basic`. This is still raw-JSON codec work, not Java `JsonIO.read(...)` object restoration parity.
  - `mdt-typeio` now also exposes a reusable payload wire-prefix boundary:
    - `TypedPayload`
    - `PayloadType`
    - `BuildPayloadHeader`
    - `UnitPayloadHeader`
    - `PayloadSummary`
    - payload prefix read/write helpers
  - `mdt-world` now reuses those helpers for payload-header and payload-router prefix probing instead of keeping that boundary fully scattered in crate-local parsing code. Payload body semantics still remain in `mdt-world`.

- `mdt-client-min` / `mdt-render-ui` additive build-inspector closure:
  - `BuildUiObservability` now carries structured read-only `inspector_entries`.
  - Runtime folds the existing configured-block projection into inspector rows, and ASCII/window presenters now render bounded `BUILD-INSPECTOR` / compact `cfg` summaries instead of only aggregate counters.
  - This widens build/config inspection value without changing packet or runtime business semantics.

- `mdt-render-ui` additive panel-model closure:
  - `mdt-render-ui` now also exposes presenter-local `MinimapPanelModel` and `BuildConfigPanelModel` helpers.
  - ASCII presenter now emits explicit `MINIMAP:` / `BUILD-CONFIG:` / `BUILD-CONFIG-ENTRY:` lines, while window presentation folds the same low-risk panel summaries into compact `mini:` / `cfgpanel:` status text.
  - This is still UI-flow shaping only: it consumes existing structured HUD/runtime/build-inspector data and does not widen protocol or gameplay semantics.
  - render taxonomy is now also split into coarse `family` plus finer detail kinds, adding explicit distinctions such as `marker-line`, `marker-line-end`, `runtime-building`, `runtime-config`, `runtime-config-rollback`, `runtime-deconstruct`, and `runtime-place`.
  - marker projection now emits detail-aware ids (`marker:{kind}:{id}` and `marker:{kind}:{id}:line-end`), while presenters keep family-level color/sprite/minimap behavior but now also surface overlay detail counts in text output.

- toolchain additive remote-freshness wiring:
  - `tools/verify-rust-workspaces.ps1` can now run `verifyMdtRemoteFreshness` inside the Rust workspace verification flow and reports explicit `remote_freshness_check: ...` markers plus a `remote_freshness_checked=...` summary field.
  - Release verify/package entrypoints now expose the same explicit remote-freshness switch so the release path can surface codegen/manifest drift without requiring a separate manual Gradle invocation.
  - The currently exposed drift in `rust/mdt-client-min/src/generated/remote_high_frequency_gen.rs` was corrected to match the generated remote artifact, and metadata-only workspace verification with remote freshness now passes.

- `mdt-world` / `mdt-render-ui` additive marker closure:
  - `LineMarker` now has a structured Rust model with Java-aligned aliases/defaults (`Line` / `line` / `LineMarker` / `lineMarker`; default `stroke=1f`, `outline=true`, default colors `ffd37f`, missing `endPos -> (0,0)`).
  - `mdt-render-ui` projection now emits both the start anchor (`marker:{id}`) and a low-risk end anchor (`marker:{id}:line-end`) for line markers instead of silently dropping the second endpoint.
  - Marker projection now also filters non-finite world coordinates before they become render objects, so malformed marker positions no longer leak `NaN`/`inf` into the presenter surface.

- `mdt-input` additive live-intent closure:
  - Placeholder action semantics are now split into explicit `Boost` / `Chat` / `Interact`, and `PlayerIntent::SetMiningTile { tile }` is now part of the mapper/live-intent path.
  - `mdt-client-min-online` `--intent-snapshot` now accepts an optional mining target tail (`:mineX,mineY` or `:none`) and keeps low-risk compatibility aliases (`pause -> Chat`, `use -> Interact`).
  - `mdt-client-min-online` now also instantiates runtime live sampling when `--intent-live-sampling` is set even without any `--intent-snapshot` schedule, so the live-input capture path is no longer silently disabled in schedule-free runs.
  - This remains intentionally thin compatibility work: `Interact` is explicit and observable, but it still does not claim Java-equivalent deeper gameplay semantics.

- `mdt-input` additive builder-queue hardening:
  - `BuilderQueueStateMachine::sync_local_entries(...)` no longer leaks `InFlight` stage or carried `block_id` across same-tile `place <-> break` replacement. Only same-mode replacement preserves those fields now, which is closer to Java `BuilderComp.addBuild(...)` semantics where a mode switch on one tile becomes a fresh local plan instead of inheriting old execution state.
  - The same sync path now also preserves existing relative order for already-known unique tiles instead of rebuilding strictly from the latest iteration order, so re-submitting the same local batch no longer causes gratuitous head/order churn. Duplicate tiles in one sync still follow the existing `tail-wins` rule, and genuinely new tiles still append.
  - This is still bounded local queue hardening, not full Java `shouldSkip` / range / stuck / resource-check parity.

- `mdt-client-min` additive session-lifecycle closure:
  - Session state now keeps explicit timeout/reset/world-reload taxonomy (`SessionTimeoutKind`, `SessionResetKind`, `WorldReloadProjection`) so `connect/loading timeout`, `ready snapshot stall`, `reconnect`, `kick`, and `worldDataBegin`-driven reloads are distinguished instead of collapsing into one generic reset bucket.
  - `ArcNetTickReport` now carries the timeout kind alongside the existing timeout marker, while external `ClientSessionEvent::WorldDataBegin` and `ClientSessionAction::TimedOut { idle_ms }` shapes remain backward-compatible to avoid unnecessary blast radius into presenters and CLI code.
  - This narrows a real lifecycle-observability gap without yet claiming the deeper `clientLoaded -> deferred replay -> connectConfirm` atomic parity that Java still has.
  - Additive loading-gate closure: `mark_client_loaded()` no longer commits `client_loaded=true` before deferred replay finishes. Deferred packet replay now batches success-side effects, leaves `client_loaded` unchanged on replay failure, and restores the failed packet plus remaining backlog to the deferred queue instead of partially advancing replay-visible state.
  - Additive ready-watchdog closure: the ready-state snapshot timeout anchor is now established when `mark_client_loaded()` finishes the `finishConnecting`-equivalent path rather than waiting for `connectConfirm`, `prepare_connect_confirm_packet()` no longer resets an existing snapshot anchor, and ready-state inbound liveness counting no longer extends the snapshot watchdog window. This moves Rust closer to Java's `world load complete => ready-state snapshot timeout starts` boundary without yet claiming full transport/lifecycle parity.
  - Additive `connectConfirm` queue closure: `mark_client_loaded()` now also auto-queues `connectConfirm` onto the normal pending-packet path once deferred replay succeeds; `prepare_connect_confirm_packet()` reuses already-queued bytes instead of regenerating/sending a second logical confirm, `worldDataBegin` still reopens the next world-load cycle for a later confirm, and the transport-facing tests were updated to treat the queued TCP confirm as the first post-load outbound action rather than an out-of-band manual step.
  - Additive runtime-HUD closure: `runtime_loading` now exposes the new timeout/reset/world-reload taxonomy in compact form (counts, last timeout kind + `idle_ms`, last reset kind, and a compressed world-reload summary) so this lifecycle state is visible without deep logs.
  - Additive reconnect-surface closure: `SessionState` now also carries a bounded `ReconnectProjection` (`phase`, `reason_kind`, `reason_text`, `reason_hint`, `target_host`, `target_port`, transition count), and `mdt-client-min` updates it on connect redirect, timeout, kick, reconnect-attempt prepare, and connect-confirm success. This remains observability-only and does not claim Java-equivalent reconnect lifecycle/UI parity.
  - Additive entity authoritative-mirror closure: `entitySnapshot` rows from the currently supported parser families now also feed a deeper authoritative semantic mirror in session state instead of stopping only at the lighter projection/table layer, and `hiddenSnapshot` authoritative cleanup keeps the local player row while still pruning non-local hidden rows. This remains bounded to already supported families and does not yet claim Java `readSyncEntity` / live-group parity.

- Local verification evidence for this batch:
  - `cargo test --manifest-path rust\\mdt-world\\Cargo.toml`
  - `cargo test --manifest-path rust\\mdt-render-ui\\Cargo.toml`
  - `cargo test --manifest-path rust\\mdt-input\\Cargo.toml`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml connect_confirm`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml world_data_begin_resets_ready_state_and_allows_second_connect_confirm`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml render_runtime::tests::`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File .\\tools\\verify-rust-workspaces.ps1`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File D:\\MDT\\mindustry-rust\\tools\\verify-rust-workspaces.ps1`
  - handoff repo synced/pushed: `D:\\MDT\\mindustry-rust` commit `2f4fb3a` (`feat: add remote effect and payload observability`)

### Session/Network Follow-Ups
- `medium` Java client still has broader server-discovery / host-probe behavior than Rust, but Rust now has a minimal usable discovery chain instead of direct-`--server` only.
  - Java evidence: `core/src/mindustry/net/ArcNetProvider.java:236`, `:273`; `core/src/mindustry/net/NetworkIO.java:100`
  - Rust evidence: `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`; `rust/mdt-client-min/src/arcnet_loop.rs`
- `high` disconnect / quiet-reset lifecycle is still materially simplified in Rust.
  - Java evidence: `core/src/mindustry/core/NetClient.java:120`, `:345`, `:657`
  - Rust evidence: `rust/mdt-client-min/src/client_session.rs:5118`; `rust/mdt-client-min/src/arcnet_loop.rs:147`
  - 2026-03-23 additive note: transport-layer reconnect now clears stale TCP/UDP/connect gate state before replacement connect attempts, so the remaining gap has narrowed to deeper session/business reset semantics rather than leftover socket-driver state.
- `medium` connect/kick failure taxonomy is still incomplete in Rust; kick-reason ordinals are now mapped to Java reason names (including `typeMismatch/customClient/clientOutdated/serverOutdated`), but the broader handshake-failure lifecycle/reporting path is still thinner than Java.
  - Java evidence: `core/src/mindustry/core/NetServer.java:218`, `:261`
  - Rust evidence: `rust/mdt-client-min/src/connect_packet.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
  - 2026-03-23 additive note: Rust now also stores high-signal remediation hints for `typeMismatch/customClient/clientOutdated/serverOutdated/serverRestarting`, so the remaining gap has narrowed from “raw ordinals only” to “named reasons plus fixed hint text”, with broader UX/policy handling still remaining follow-up work.
- `high` `clientLoaded` load-phase backlog dropping under the old 256-entry cap is now closed; the remaining gap is the semantic apply depth of replayed packets after load rather than whether normal-priority backlog survives loading.
  - Java evidence: `core/src/mindustry/net/Net.java:137`, `:292`
  - Rust evidence: `rust/mdt-client-min/src/client_session.rs:3949`
- `high` world/snapshot semantic apply depth remains the main post-release parity gap.
  - Java evidence: `core/src/mindustry/net/NetworkIO.java:64`; `core/src/mindustry/core/NetClient.java:485`, `:513`, `:539`
  - Rust evidence: `rust/mdt-client-min/src/bootstrap_flow.rs:218`; `rust/mdt-client-min/src/client_session.rs:3811`, `:4540`; `rust/mdt-client-min/src/snapshot_ingest.rs:48`

### Remote / Codegen Follow-Ups
- `high` Rust still has a sizeable `server -> client` remote-method tail that is registered but only lands in `IgnoredPacket`, so packet-id parity is better than semantic parity.
  - Evidence shard: `audit/agent-hooke-remote-audit.md`
- `medium` remote manifest validation still under-checks wire-level invariants (`packetIdByte`, `lengthField`, compression markers/threshold), which leaves room for manifest/codec drift.
  - Evidence shard: `audit/agent-hooke-remote-audit.md`
- `medium` generated remote artifacts are still vulnerable to refresh drift unless freshness/consistency checks are enforced by build/CI.
  - Evidence shard: `audit/agent-hooke-remote-audit.md`

### Client / Input / UI Follow-Ups
- `resolved` outbound menu/text-input reply closure is now landed: `ClientSession` now exposes `queue_menu_choose` / `queue_text_input_result`, online CLI now exposes `--action-menu-choose` / `--action-text-input-result`, and both payload shapes are covered by regression tests.
  - Evidence: `rust/mdt-client-min/src/client_session.rs`
  - Evidence: `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
  - Evidence shard: `audit/agent-kierkegaard-client-audit.md`
- `high` realtime input semantics remain far narrower than Java bindings, especially around build/cargo/command/stance flows.
  - Evidence shard: `audit/agent-kierkegaard-client-audit.md`
- `medium` `entitySnapshot` class-family breadth is no longer the main vanilla risk; the sharper residual risk is that already-covered rows still land only in lightweight projections and legacy-id gating remains wider than the current Java runtime map.
  - Evidence shard: `audit/entity-snapshot-syncc-refresh-20260324e.md`
  - Evidence shard: `audit/agent-kierkegaard-client-audit.md`

### Serialization / Payload Follow-Ups
- `high` class-id gating for some Rust unit-family parsers still carries legacy compatibility IDs beyond current vanilla `EntityMapping`, so the accepted parser surface is broader than the true runtime mapping.
  - Evidence shard: `audit/agent-ampere-serialization-audit.md`
- `medium` revision-boundary real-Java fixtures are still uneven for some families (`manifold`, `missile`, `spiroct`) and controller-type coverage beyond `0` remains thin.
  - Evidence shard: `audit/agent-ampere-serialization-audit.md`

### Content / Gameplay Follow-Ups
- `high` Rust still lacks a true content/gameplay registry and runtime execution layer for `unit/block/bullet/ability/status/effect`, so current parity remains protocol/projection-oriented rather than gameplay-semantic.
  - Evidence shard: `audit/agent-poincare-content-audit.md`

### World/TypeIO Follow-Ups
- `high` Rust still lacks Java-style `.msav` container + save-version dispatch (`Save1..11`) and regionized save pipeline.
  - Java evidence: `core/src/mindustry/io/SaveIO.java:21`, `:23`, `:166`; `core/src/mindustry/io/SaveVersion.java:66`, `:81`
  - Rust evidence: `rust/mdt-world/src/lib.rs:23073`, `:23077`
- `high` entity-region read/write and entity remap parity is still missing.
  - Java evidence: `core/src/mindustry/io/SaveVersion.java:417`, `:463`, `:499`
  - Rust evidence: `rust/mdt-world/src/lib.rs:12447`, `:23233`
- `high` snapshot application in `mdt-world` is still contract/projection oriented, not Java-equivalent live entity/build/state execution.
  - Java evidence: `core/src/mindustry/io/TypeIO.java:291`, `:309`; `core/src/mindustry/entities/comp/BuildingComp.java:289`
  - Rust evidence: `rust/mdt-world/src/lib.rs:7339`, `:7498`
  - 2026-03-24 additive note: `message` / `reinforced-message` / `world-message` building tails are now structurally parsed in `mdt-world`, so the remaining gap for that family is semantic application depth rather than raw tail opacity.
- `medium` `mdt-typeio` still lacks much of Java `TypeIO`'s non-object codec surface (`payload/mounts/abilities/rules/objectives/status/unit sync` families).
  - Java evidence: `core/src/mindustry/io/TypeIO.java:215`, `:284`, `:699`, `:771`
  - Rust evidence: `rust/mdt-typeio/src/lib.rs:5`, `:88`; `rust/mdt-typeio/src/object.rs:5`

### Render/UI Follow-Ups
- `high` Rust still lacks real-time keyboard/mouse/touch capture and stays largely CLI-driven.
  - Java evidence: `core/src/mindustry/input/DesktopInput.java:233`; `core/src/mindustry/input/MobileInput.java:512`; `core/src/mindustry/input/InputHandler.java:51`
  - Rust evidence: `rust/mdt-client-min/src/bin/mdt-client-min-online.rs:56`; `rust/mdt-render-ui/src/window_presenter.rs:231`
- `high` Java's layered renderer/HUD/build-config UI remains mostly absent in Rust.
  - Java evidence: `core/src/mindustry/core/Renderer.java:31`, `:332`; `core/src/mindustry/ui/fragments/HudFragment.java:235`, `:542`; `core/src/mindustry/ui/fragments/PlacementFragment.java:37`
  - Rust evidence: `rust/mdt-render-ui/src/window_presenter.rs:409`; `rust/mdt-render-ui/src/render_model.rs:36`; `rust/mdt-render-ui/src/hud_model.rs:3`; `rust/mdt-client-min/src/render_runtime.rs:1241`
- `medium` chat/dialog/minimap/battle-entity rendering remain user-visible backlog items even after minimal runtime observability work.
  - Java evidence: `core/src/mindustry/ui/fragments/ChatFragment.java:26`; `core/src/mindustry/core/UI.java:200`, `:243`; `core/src/mindustry/ui/Minimap.java:59`; `core/src/mindustry/graphics/ParticleRenderer.java:14`
  - Rust evidence: `rust/mdt-client-min/src/client_session.rs:2005`; `rust/mdt-client-min/src/bin/mdt-client-min-online.rs:4172`; `rust/mdt-render-ui/src/projection.rs:156`, `:301`; `rust/mdt-client-min/src/render_runtime.rs:966`, `:1404`

## Non-Negotiable Messaging
Do not state:
- "Rust version already implements all original Mindustry parts"
- "workspace green means release binaries are fully covered"

Use:
- "Rust minimal compatibility client release chain is working"
- "Full Java client parity is not complete"

## 2026-03-24 Additional Landing Notes

- `tileConfig` request/response reconcile no longer uses one last-value pending slot per building.
  - `TileConfigProjection` now keeps FIFO local request queues per building.
  - authoritative `tileConfig`, `constructFinish`, and parse-fail fallback only resolve the oldest pending request, preserving later local config intents on the same building.
  - targeted `mdt-client-min` regressions now cover oldest-only consume, rollback-preserves-later-intent, and parse-fail fallback preserving later intent.

- `mdt-client-min` typed inbound custom/logic registry glue is landed.
  - `mdt-remote` now exposes inbound `payload_kind()`.
  - `mdt-client-min` now has typed inbound dispatch specs plus `typed_remote_dispatch.rs` helper coverage for `serverPacket*` / `clientLogicData*`.
  - current remaining gap is adoption in live session/business paths, not typed manifest metadata itself.

- `mdt-world` now has stronger `.msav -> post_load_world()` query helpers.
  - `save_post_load.rs` exposes graph/team-plan/marker/custom/static-fog accessors so post-load world semantics are no longer summary-only.
  - this still stops below Java `NetworkIO.loadWorld(...)` live world/entity application.

- `mdt-input` live-intent schedule override is now one-shot instead of sticky.
  - due schedule samples apply only for that tick and then yield back to runtime sampling.
  - this narrows one real runtime-input gap without claiming desktop/mobile capture parity.

- `mdt-render-ui` now preserves authoritative `view_window` through projection/presenter/minimap paths.
  - already-windowed render models are no longer expanded back out by presenter-local fallback logic.

- `hiddenSnapshot` cleanup now goes beyond non-local `Unit` rows.
  - `snapshot_ingest.rs` / `session_state.rs` now also prune non-local known runtime-owned `Fire` / `Puddle` / `WeatherState` rows while still keeping `WorldLabel` conservative.
  - this narrows `handleSyncHidden()` parity risk without claiming Java-equivalent live group ownership.

- `mdt-client-min-online` now has explicit command-mode CLI/runtime seed controls.
  - new flags: `--command-mode-bind-group`, `--command-mode-recall-group`, `--command-mode-clear-group`, `--command-mode-rect`.
  - runtime command-mode seed ops are replayed after `WorldDataBegin`, connect redirect, and server-restart reconnect clears so local command-mode setup is not silently lost on map/reconnect boundaries.

- `mdt-render-ui` now has a user-visible runtime dialog summary layer.
  - `panel_model` folds `menu` / `follow-up menu` / `text input` / `hud text` / `toast` into one `RuntimeDialogPanelModel`.
  - `ascii_presenter` emits `RUNTIME-DIALOG:` and `window_presenter` adds a compact `dialog:` status slice.

- `mdt-world` now has a post-load activation preflight helper.
  - `save_post_load_activation.rs` exposes `SavePostLoadActivationSurface` with loadable/skipped entity candidates, unresolved remap names, building-center reference validity, and `can_seed_runtime_apply()`.
  - this still stops below Java `NetworkIO.loadWorld(...) -> finishConnecting()` live world/entity apply.

- `mdt-typeio` now has raw `WeaponMount[]` codec coverage.
  - `unit_sync.rs` exposes `WeaponMountRaw`, `write_weapon_mounts`, `read_weapon_mounts`, and `read_weapon_mounts_prefix`.
  - this narrows the non-object `TypeIO` gap for unit sync without claiming broader `abilities/status` parity.

- Local verification evidence for this landing batch:
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml typed_dispatch_ --lib`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml tile_config_`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml construct_finish_packet_reconciles_pending_tile_config_intent`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml hidden_snapshot_`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml parse_args_accepts_command_mode`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml runtime_command_mode_cli_updates_projection`
  - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml`
  - `cargo test --manifest-path rust\\mdt-render-ui\\Cargo.toml`
  - `cargo test --manifest-path rust\\mdt-world\\Cargo.toml activation_surface_`
  - `cargo test --manifest-path rust\\mdt-typeio\\Cargo.toml weapon_mounts`

## 2026-03-26 Parallel Dispatch Refresh

### Newly Landed Today
- `mdt-client-min` runtime scene now surfaces command-mode overlay objects for:
  - `command_buildings`
  - `command_rect`
  - `last_target.build_target`
  - `last_target.position_target`
  - `last_target.unit_target`
  - `last_target.rect_target`
- `mdt-render-ui` window HUD now surfaces:
  - top-line session banner preference over `wave_text`
  - third bottom-line build strip summary with selected block, rotation, queue stage, and authority state

### Immediate High-Value Independent Shards
- `P0 active` `unitCapDeath` lifecycle parity in `rust/mdt-client-min/src/client_session.rs`
  - Current gap: packet updates counters only, unlike `unitDeath` / `unitDestroy` / `unitEnvDeath` / `unitSafeDeath` it does not remove entity projection or clear resource delta state.
  - Why it matters: this is a concrete behavior bug, not just missing observability.
  - Minimum target: make `unitCapDeath` remove the affected entity and clean resource mirror state, with regression coverage.

- `P0` player semantic mirror in `rust/mdt-client-min/src/session_state.rs` + `rust/mdt-client-min/src/bootstrap_flow.rs` + `rust/mdt-client-min/src/client_session.rs`
  - Current gap: Rust parses `admin/name/color/team/mouse/selectedBlock/selectedRotation/typing/shooting/boosting`, but runtime player state still stores almost only `player_id/unit_kind/unit_value/x/y`.
  - Why it matters: inbound player sync currently feeds outbound snapshot fields, but not a reusable runtime player model for render/debug/gameplay semantics.
  - Minimum target: add `EntityPlayerSemanticProjection`, persist both local and remote player semantic fields, and update local-player mirror on bootstrap plus entity snapshot apply.

- `P0` unit semantic retention in `rust/mdt-client-min/src/session_state.rs` + `rust/mdt-client-min/src/client_session.rs`
  - Current gap: `EntityUnitSemanticProjection` keeps only a thin subset while parsed sync rows already contain `ammo/elevation/velocity/base_rotation/flag/controller/stack/status payload` data.
  - Why it matters: this is the main blocker between current lightweight unit presence tracking and Java `UnitComp`-level behavior fidelity.
  - Minimum target: first widen stored unit fields, then split follow-up shards for `status`, `payload content`, and `controller/command` semantics.

- `P1` loaded-world building live-state unification in `rust/mdt-client-min/src/client_session.rs` + `rust/mdt-world/src/lib.rs`
  - Current gap: `setTile/removeTile` update table/projection state and may create placeholder building centers, but loaded-world building centers still drift from live authority and lack multiblock lifecycle parity.
  - Why it matters: world state is still dual-sourced between patched world data and runtime projections.
  - Minimum target: give runtime-created centers stable revision/base state, define center lifecycle policy, and make later block/build updates patch loaded-world building truth instead of only auxiliary tables.

- `P1` render primitive/model expansion in `rust/mdt-render-ui/src/render_model.rs` + `rust/mdt-render-ui/src/projection.rs` + `rust/mdt-client-min/src/render_runtime.rs`
  - Current gap: `RenderObject` still only carries `id/layer/x/y`, which blocks richer minimap, marker, label, health, and runtime effect rendering.
  - Why it matters: current runtime render breadth is increasingly observability-rich, but fidelity stays capped by the primitive model.
  - Minimum target: introduce explicit line/rect/text/marker-style payloads or equivalent primitive metadata without breaking existing presenters.

## 2026-03-26 Dispatch Follow-Up

### Newly Landed After Refresh
- `mdt-render-ui` ASCII presenter now rasterizes paired `marker:line:*` + `:line-end` overlays into visible line segments instead of showing only endpoint dots.
  - Local verification: `cargo test --manifest-path rust\\mdt-render-ui\\Cargo.toml --quiet`
  - Target handoff: `D:\\MDT\\mindustry-rust` commit `1f456ae` (`feat: rasterize ascii marker line segments`)

- `mdt-client-min` runtime player model now carries authoritative player semantic mirrors for both bootstrap and `entitySnapshot` paths.
  - Landed fields: `admin/name/color_rgba/team_id/mouse_x_bits/mouse_y_bits/selected_block_id/selected_rotation/typing/shooting/boosting`
  - Local verification: `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml --quiet`
  - Target handoff: `D:\\MDT\\mindustry-rust` commit `55cfdae` (`feat: project player semantic state into runtime mirrors`)

- `mdt-client-min` now seeds unit carried-item stacks directly from `entitySnapshot` authoritative sync rows across the currently covered unit families.
  - Landed behavior:
    - `stack_amount > 0` and valid `stack_item_id` overwrite `entity_item_stack_by_entity_id`
    - `stack_amount <= 0` clears the unit-carried stack
  - Local verification: `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml --quiet`
  - Target handoff: `D:\\MDT\\mindustry-rust` commit `eafe3a4` (`feat: seed unit carried item stacks from snapshots`)

- `mdt-client-min-online` now uses one reconnect executor for redirect, `serverRestarting`, and timeout reconnect attempts, with once-only scheduling plus failure backoff.
  - Local verification: `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml --quiet`
  - Target handoff: `D:\\MDT\\mindustry-rust` commit `ad6c9de` (`feat: unify reconnect executor paths`)

- `mdt-render-ui` now exposes a low-blast-radius line primitive channel derived from legacy runtime marker line pairs.
  - Landed shape:
    - `RenderPrimitive::Line`
    - `RenderModel::primitives()` derived from paired `marker:line:*` / `:line-end`
  - Local verification:
    - `cargo test --manifest-path rust\\mdt-render-ui\\Cargo.toml --quiet`
    - `cargo test --manifest-path rust\\mdt-client-min\\Cargo.toml --quiet`
  - Target handoff: `D:\\MDT\\mindustry-rust` commit `7c5e783` (`feat: derive line primitives from runtime markers`)

### Refreshed Release-Critical Order
- `P0 active` loaded-world building live-state is still dual-sourced between `loaded_world_bundle` and projection/runtime authority.
  - Primary source paths:
    - `rust/mdt-client-min/src/client_session.rs`
    - `rust/mdt-client-min/src/session_state.rs`
  - Landed first cut:
    - live authority packets no longer mutate `loaded_world_bundle.world.building_centers[*].building.base`
    - `setTile` / `setTileBlocks` / `constructFinish` no longer fabricate new loaded-world building centers for live-only tiles
    - `entitySnapshot` / loaded-world `blockSnapshot` building parsers now fall back to current `building_table_projection` metadata when the loaded-world center is missing or stale, as long as the needed revision metadata is still available
    - regression tests now pin "projection/live view moves, loaded-world baseline centers stay untouched" for `setTeam` / `setTeams` / `setTile` / `setTileBlocks` / `constructFinish` / `buildHealthUpdate`
  - Immediate cut:
    - stop live building packets from mutating `loaded_world_bundle.world.building_centers[*].building.base`
    - stop fabricating live-only building centers into loaded-world baseline
    - add a merged building live view by `build_pos`
  - Concrete first-cut boundaries:
    - remove or bypass live writeback helpers centered on `ensure_loaded_world_building_center`, `apply_loaded_world_building_center_authority`, and `apply_loaded_world_building_health`
    - keep `apply_world_baseline_from_bundle` plus `apply_loaded_world_parsed_tail_business` as baseline seeding only
    - add a merged accessor that combines loaded-world anchor data with `building_table_projection`, `configured_block_projection`, `resource_delta_projection`, and `runtime_typed_building_projection`
  - Minimum regression matrix:
    - baseline seed still exposes loaded-world center/tail-derived runtime state
    - `setTeam` / `setTeams` update merged live view without mutating loaded-world baseline team
    - live-only `setTile` / `constructFinish` stop fabricating new loaded-world centers
    - `removeTile` / `deconstructFinish` clear live view without deleting baseline bundle data
    - `buildHealthUpdate` / `tileConfig` remain visible through the merged view

- `P0 landed` controller/ownership semantics now consume authoritative unit controller fields before heuristic player-to-unit linkage.
  - Primary source paths:
    - `rust/mdt-client-min/src/runtime_entity_ownership.rs`
    - `rust/mdt-client-min/src/session_state.rs`
    - `rust/mdt-client-min/src/client_session.rs`
  - Landed cut:
    - snapshot `controller_type/controller_value` now projects into unit semantic state across the currently covered unit families
    - `controller_type == 0` / `controller_value == player_id` now wins over `player.unit_value -> unit` heuristic ownership
    - heuristic fallback is now gated to `controller_type == 0 && controller_value.is_none()`
    - focused ownership regression tests now cover controller-preferred ownership, heuristic fallback, and non-player controller rejection

- `P1 active` reconnect executor is landed, but reconnect command state is still split across projection, events, and ad-hoc online-loop policy.
  - Primary source paths:
    - `rust/mdt-client-min/src/client_session.rs`
    - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
  - Remaining gap:
    - timeout still executes mainly from `report.timed_out/timed_out_kind`, not from a durable reconnect command
    - redirect target and restart delay still live in separate event/state slots
    - reconnect policy is still minimal and online-loop-local rather than session-level
  - Next cut:
    - move toward one consumable reconnect command surface instead of event-special-casing

- `P1 active` render primitive line channel is landed, but text/icon/richer primitive storage is still missing.
  - Primary source paths:
    - `rust/mdt-render-ui/src/render_model.rs`
    - `rust/mdt-client-min/src/render_runtime.rs`
  - Remaining gap:
    - current line primitive is derived from legacy objects, not an independent stored channel
    - presenters/summary still consume objects only
    - world-label text and icon-like runtime overlays are still encoded as point objects
  - Next cut:
    - add text/icon primitive families or an equivalent typed overlay payload path

### Current Parallel Lanes
- `ready` loaded-world building live-state merged-view cut
- `ready` reconnect command unification beyond online-loop-local policy
- `ready` render primitive follow-up for text/icon and presenter consumption
