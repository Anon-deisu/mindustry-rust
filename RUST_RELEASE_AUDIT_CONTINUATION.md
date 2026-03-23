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
   - M6 remaining high-complexity slice: Java-equivalent `effect(..., data)` business semantics on top of the now-broader `TypeIO.readObject` coverage, not packet detection itself.
   - Recommended next focus: M7 deeper snapshot semantics (`entity` / `block` / `hidden` breadth and `stateSnapshot.coreData` semantic apply), while preserving minimal-compatibility release claims.
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
5. `resolved` Minimal remote-control coverage widened:
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
6. `resolved` Transitional fixture fallback cleanup is closed at R+2 canonical-only policy:
   - Plan/report: `audit/transitional-fixture-cleanup.md`
   - Canonical policy marker: `release_prereq_check: ... fixture_policy=canonical_only`
   - Release verify marker: `verified_windows_release_set: ... fixture_policy=canonical_only ...`
   - Transitional switches/waiver flow removed from release-facing scripts.
   - Explicit transitional fixture path usage now hard-fails in release scripts.
   - Evidence: `tools/check-mdt-release-prereqs.ps1:21-28`, `:39-47`
   - Evidence: `tools/verify-mdt-client-min-release-set.ps1:53-60`, `:128-137`, `:306`
   - Evidence: `tools/package-mdt-client-min-online.ps1:95-99`, `:115-120`
   - Evidence: `tools/WINDOWS-RELEASE.md:60-69`
7. `resolved` Remote manifest source vs fixture mirror relationship is explicitly documented for release operations:
   - Generated build artifact source: `build/mdt-remote/remote-manifest-v1.json`
   - Canonical release fixture mirror: `fixtures/remote/remote-manifest-v1.json`
   - Release packaging uses canonical-first candidate order and rejects transitional `rust/fixtures/...` paths at R+2.
   - Evidence: `tools/package-mdt-client-min-online.ps1:97-99`, `:115`, `:128`
   - Evidence: `tools/check-mdt-release-prereqs.ps1:39-41`
8. `resolved` Build/codegen minimal viable chain is connected:
   - Gradle tasks can generate `build/mdt-remote/remote-manifest-v1.json`
   - Gradle codegen refreshes `rust/mdt-client-min/src/generated/remote_high_frequency_gen.rs`

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
  - `audit/input-build-plan-parity.md` (P6/M8)
  - `audit/render-ui-parity.md` (P7/M9)
  - `audit/transitional-fixture-cleanup.md` (P8)
- Manifest path relation (release policy):
  - generated source artifact: `build/mdt-remote/remote-manifest-v1.json`
  - canonical release fixture mirror: `fixtures/remote/remote-manifest-v1.json`
  - transitional `rust/fixtures/...` paths are not accepted by release-facing scripts at R+2
- Current state:
  - M6: small control packets improved (`connect(String,int)` now also has a minimal online-runtime redirect execution chain, `kick(KickReason.serverRestarting)` now also has a minimal online-runtime reconnect execution chain against the current server, `playerDisconnect`, `setCameraPosition`, `sound`, `soundAt`, broader `effect(..., data)` object parsing/storage plus event-level `data_object` carriage, lightweight business projections for the highest-signal effect-data tags (now including entity-table `UnitId`, first-element `Point2[]` / `Vec2[]` position discovery, bounded nested `object[]` discovery, structured `object[]` kind summaries, explicit `ParentRef` handling for `BuildingPos` / `UnitId`, and typed `ContentRef` distinction between raw `Content` and raw `TechNode`), fixed-shape `effect` / `effectReliable`, effect-data parse-fail/trailing observability, `traceInfo`, outbound `adminRequest`, outbound `requestDebugStatus`, inbound `debugStatusClient` / `debugStatusClientUnreliable`, `setRules`, `setObjectives`, `setRule`, `clearObjectives`, `completeObjective`, strict inbound `beginPlace` appended `TypeIO` config tracking, `constructFinish` full appended `TypeIO` config tracking, inbound `tileConfig` minimal event/state tracking with parse-fail observability plus event-level authoritative-apply/rollback flags, and a lightweight authoritative building table keyed by `build_pos` that now updates from `blockSnapshot` first-entry fixed-prefix data plus `constructFinish` / `tileConfig` / `deconstructFinish`, while `constructFinish` also seeds the tracked authoritative config map for that building and `deconstructFinish` clears stale config state for removed buildings, `clientPacket*`, `clientBinaryPacket*`, inbound `serverPacket*`, inbound `serverBinaryPacket*`, session-level custom packet handler registration/dispatch plus minimal outbound queue coverage, inbound `clientLogicData*` event/state observability plus session-level channel handler registration/dispatch and minimal outbound queue coverage, and online runtime watch/handler/print wiring via `--watch-client-packet` / `--watch-client-binary-packet` / `--print-client-packets`). `blockSnapshot` fixed-prefix/base coverage now includes `health/rotation/team/ioVersion/enabled/moduleBitmask/timeScale/timeScaleDuration/lastDisabler/legacyConsumeConnected/efficiency/optionalEfficiency/visibleFlags`, and the building table now preserves minimal `block_id/rotation/team/io_version/module_bitmask/time_scale_bits/time_scale_duration_bits/last_disabler_pos/legacy_consume_connected/enabled/config/health/efficiency/optional_efficiency/visible_flags/last_update` facts rather than only `block_id/config/health`. `mdt-typeio` now also covers a much broader object subset across raw `Content`, `IntSeq`, `Point2[]`, raw `TechNode`, `Team`, `LAccess`, `double`, raw `Building`, legacy unit-command-null marker, `boolean[]`, raw `Unit`, `Vec2[]`, `UnitCommand`, and Java-correct `int[]` short-length semantics. Inbound command/control families now also have a first pure-observability pass for `buildingControlSelect` / `unitBuildingControlSelect` / `unitClear` / `unitControl` / `commandBuilding` / `commandUnits` / `setUnitCommand` / `setUnitStance` across `ClientSessionEvent`, session-state counters/last-values, runtime HUD count labels, and online `--print-client-packets` summaries; this `unitBuildingControlSelect` landing completes the first command/control observability batch. Remaining outliers are Java-level `effect(..., data)` business semantics, fuller `tileConfig` business semantics/rollback parity, and broader custom-handler business parity.
  - M6 additive packet-slice update: inbound `takeItems` / `transferItemTo` / `transferItemToUnit` / `payloadDropped` / `pickedBuildPayload` / `pickedUnitPayload` / `unitDespawn` now have explicit event/state handling, and outbound coverage now also includes `setUnitCommand` / `setUnitStance`.
  - M6 additive observability update: HUD/UI notice (`setHudText` / `setHudTextReliable` / `hideHudText` / `announce` / `infoMessage` / `infoToast` / `warningToast` / `copyToClipboard` / `openURI`), menu lifecycle (`menu` / `followUpMenu` / `hideFollowUpMenu` / `textInput` 6/7-arg), and resource mirror (`setItem` / `setItems` / `setLiquid` / `setLiquids` / `setTileItems` / `setTileLiquids`) now have explicit decode/event/state observability with runtime and `--print-client-packets` visibility; `text_input.message` now uses structured `runtime_ui` observability instead of legacy `runtime_ui_menu` message concatenation.
  - M6 additive typeio-summary update: `mdt-typeio` now exposes bounded `effect_summary` output with stable kind + semantic-parent/position hints, and `mdt-client-min` effect-data kind labels now consume that shared summary surface.
  - M7: snapshot/session chain is working for minimal release; default cadence now tracks Java more closely and timeout defaults mirror the 30min connect / 20s snapshot-stall baseline. Rust now minimally applies/tracks `stateSnapshot` header, decodes `coreData` into a lightweight `team -> items` projection in session state, derives a business/runtime projection for `wave/enemies/paused/gameOver/timeData/tps/rand/core inventory` plus lightweight apply semantics (`Playing/Paused/GameOver` state transitions, wave-advance from/to, `timeData` delta/rollback/apply counters, and compact core-inventory total/nonzero/changed-team summaries), and now also keeps a separate session-authoritative mirror that always applies state headers, preserves last-good core inventory on malformed `coreData`, tracks `gameOver > paused > playing` precedence plus wave-advance-only-on-increase semantics, exposes core-parse-fail counters, and is reset on `worldDataBegin`; runtime HUD now prefers that authority mirror for snapshot labels. Rust records both packet-parse and `coreData`-parse failures, adds `entitySnapshot` envelope/apply observability plus a minimal local-player entity table row (`entity_id -> class/local/unit/position/hidden`), batch-upserts any additional parseable `classId=12` player rows into that entity table, and now also parses known-prefix non-player sync rows so alpha-shape `classId=0` (alpha) rows are real-decoded and upserted into the same entity table while the same revision family `classId=29/30/31/33` shares that shape parser path, mech-shape `classId=4` plus same-family `classId=17/19/32` now follow a second real parse+upsert path, missile-shape `classId=39` now follows a third real parse+upsert path with explicit `lifetime/time` field parsing, and payload-shape `classId=5/23/26/36` now follows a fourth real parse+upsert path for `payloadCount=0` plus loaded-world-context recursive `BuildPayload` consumption for `payloadCount > 0` while `UnitPayload` recursion still fail-closes (`classId=36` additionally parses the tethered `building` reference); local-player extraction is also hardened so only a unique parseable `player_id + classId(12)` match is accepted while ambiguous/multi-hit payloads are rejected with explicit counters/last-match observability. Rust also adds minimal `blockSnapshot` / `hiddenSnapshot` envelope parse observability (`amount`/`data_len`, `count`/`first_id`) with dedicated parse-failure telemetry, exposes a lightweight `blockSnapshot` head runtime projection plus first-entry fixed-prefix diagnostics (`health/rotation/team/ioVersion/enabled/moduleBitmask/timeScale/timeScaleDuration/lastDisabler/legacyConsumeConnected/efficiency/optionalEfficiency/visibleFlags`), and now prevents conflicting `blockSnapshot` head data from overwriting an already tracked authoritative building row at the same `build_pos` when `block_id/team/io_version` disagree, surfacing dedicated conflict-skip telemetry instead. When a loaded world bundle is available, Rust can now additionally parse and apply later `blockSnapshot` entries beyond the first by using the loaded-world block name + revision context to recover safe entry boundaries, but that incremental path still only updates the lightweight building table's base fields rather than claiming full Java `tile.build.readSync(...)` behavior. `hiddenSnapshot` is treated as a one-shot trigger for the current payload's IDs instead of a persistent hidden set, now also applies hidden flags into that entity table and performs a minimal lifecycle removal for non-local hidden rows, and the lightweight authoritative building table keyed by `build_pos` (`block_id/rotation/team/io_version/module_bitmask/time_scale_bits/time_scale_duration_bits/last_disabler_pos/legacy_consume_connected/enabled/config/health/efficiency/optional_efficiency/visible_flags/last_update`) survives across `blockSnapshot` / `constructFinish` / `tileConfig` / `deconstructFinish` / `buildHealthUpdate`. Runtime HUD/refresh surfaces snapshot summaries plus state/effect business-apply labels, the building-table summary label, block fixed-prefix diagnostics, and explicit config / config-rollback markers for authoritative `tileConfig`, world-stream begin packets enforce a hard size limit before allocation, the ready-state snapshot timeout anchor is armed on connect-confirm and refreshed by `EntitySnapshot` only, and Java's `clientLoaded` load gate is now approximated by deferring/replaying normal-priority inbound packets during active world-data load while dropping low-priority inbound packets and clearing the deferred queue on `worldDataBegin`. `worldDataBegin` now also clears lightweight rules/objectives projections, the entity table, the builder queue projection, the building table, `tile_config_projection`, and both snapshot business + authority state so a new world does not inherit prior-world semantics. The main remaining gap is broader snapshot application depth and full live-world/system parity.
  - M7 additive semantics update: `hiddenSnapshot` now explicitly tracks both latest hidden-id set and real delta (`added` / `removed` versus previous payload), with runtime HUD labels for both current and delta state (`runtime_hidden`, `runtime_hidden_delta`).
  - M7 additive observability update: runtime HUD now also exposes compact audio/admin counters (`runtime_audio`, `runtime_admin`) together with loading/snapshot gate counters (`runtime_loading`) so `sound` / `soundAt`, `traceInfo`, `debugStatusClient*`, deferred/replayed inbound packets, dropped loading-time low-priority packets, and state/entity snapshot parse failures are visible without deep logs. Truncated `sound` / `soundAt` / `traceInfo` / `debugStatusClient*` payloads now also increment dedicated session-state parse-fail counters, and the audio/admin HUD labels surface those counts directly.
  - M7 additive hardening update: the loaded-world `blockSnapshot` extra-entry path is now fail-closed and only batch-applies later entries when the entire loaded-world parse succeeds and the reconstructed first entry still matches the already parsed head projection; `entitySnapshot` now also keeps a short-lived tombstone guard for recently removed entity IDs so stale immediate-following player-row snapshots do not instantly recreate entities just removed by `unitDespawn` / `unitEnteredPayload` / `playerDisconnect` / hidden lifecycle.
  - M7 additive hardening update: `stateSnapshot` now explicitly retains `last_good_state_snapshot_core_data` and runtime summary surfaces use this as a `last_good` fallback when current `coreData` parse fails; `entitySnapshot` parseable-player-row extraction now fail-closes when parseable rows exceed declared envelope `amount`; and hidden non-local entity IDs now continue blocking `entitySnapshot` re-upsert while they remain hidden.
  - M7 additive entitySnapshot slice update: known-prefix non-player entity-row parsing is now wired into the same entity-table apply path, with landed real parse+upsert support for alpha-shape `classId=0` (`alpha`) plus same-shape revision family coverage for `classId=29/30/31/33`, mech-shape `classId=4` plus same-shape revision family coverage for `classId=17/19/32`, missile-shape `classId=39`, and payload-shape `classId=5/23/26/36` for `payloadCount=0` plus loaded-world-context recursive `BuildPayload` consumption for `payloadCount > 0`.
  - M7 additive entitySnapshot slice update: fixed-shape environment entities `Fire` (`classId=10`), `Puddle` (`classId=13`), `WeatherState` (`classId=14`), and bounded string-shape `WorldLabel` (`classId=35`) are now parsed from the same known-prefix chain and upserted into the minimal entity table.
  - M7 additive entitySnapshot safety update: payload-family parsing is now split by recursive payload kind; `BuildPayload` entries inside `payloadCount > 0` are boundary-consumed when loaded-world `content_header` context can resolve the block name, while `UnitPayload` recursion and unknown build-payload block mappings remain explicit fail-closed (including tether-payload shape).
  - M7 current primary entitySnapshot risk: recursive `UnitPayload` bodies when `payloadCount > 0` still lack a proven boundary-safe `unit.read(...)` consume parser; this is now the main outstanding gap in entity-row coverage.
  - M7 additive load-gate parity update: normal-priority inbound packets are no longer hard-capped to 256 entries during active world-data load; Rust now keeps Java-closer queue+replay semantics for this path while still dropping `priorityLow` packets during loading and clearing stale deferred packets on `worldDataBegin`.
  - M7 additive block/state update: `blockSnapshot` world-apply responsibility is now unified on the `client_session + loaded_world` path instead of split between `snapshot_ingest` head-apply and `client_session` extra-entry apply. `snapshot_ingest` now keeps envelope/head observability only, while the session-side loaded-world path can apply all parsed entries, preserves the already-applied prefix on later-entry parse failure, and still fail-closes on outer trailing-byte drift. In parallel, `stateSnapshot` now also seeds an explicit `authoritative_state_mirror` runtime-facing field (currently mirroring the authority projection) that is cleared on `worldDataBegin` and preferred by runtime HUD surfaces when present.
  - M8: basic input/build compatibility is working; snapshot cadence and `getMaxPlans` guard baseline are now closer to Java, Rust build-plan config encoding now covers a much broader `TypeIO` subset (`Int` / `Long` / `Float` / `Bool` / `IntSeq` / `Point2` / `Point2[]` / `TechNodeRaw` / `Double` / `BuildingPos` / `LAccess` / `String` / `byte[]` / `LegacyUnitCommandNull` / `boolean[]` / `UnitId` / `Vec2` / `Vec2[]` / `Team` / `int[]` / `object[]` / raw `Content` / `UnitCommand`) in addition to `None`, `mdt-input` `rotatePlans/flipPlans` utilities are now wired into the online CLI path, `rotate_plans` now also normalizes multi-step direction turns, `mdt-input` now also exposes a minimal stateful action-edge mapper (`ActionPressed` / `ActionHeld` / `ActionReleased`) with duplicate-action dedupe plus stable edge ordering across input permutations / multi-release drops, and the online runtime now exposes queued `requestItem`, `requestUnitPayload`, `unitClear`, `unitControl`, `unitBuildingControlSelect`, `buildingControlSelect`, `clearItems`, `clearLiquids`, `transferInventory`, `requestBuildPayload`, `requestDropPayload`, `dropItem`, `rotateBlock`, `tileConfig`, `tileTap`, `deletePlans`, `commandBuilding`, and `commandUnits`. Rust now also keeps a minimal builder queue projection driven by `BeginBreak` / `BeginPlace` / `RemoveQueueBlock` / `ConstructFinish` / `DeconstructFinish`, surfacing `Queued` / `InFlight` / `Finished` / `Removed` plus orphan-authoritative counts and a queue-head view in runtime HUD text; same-tile place/break replacement follows Java `BuilderComp.addBuild(...)` more closely by using `(x,y)` dedupe semantics in that queue view; `snapshot_input.building` in `mdt-client-min` now follows this authoritative queue projection instead of staying CLI/override-only when inbound queue packets advance or clear the local build state; and the online harness now has a stable outbound-action script regression so scheduled client events can be replay-checked as a deterministic signature. Full builder/input parity and remaining online-harness integration are still open, but the main remaining delta is Java behavior semantics rather than the old narrow config-type baseline.
  - M8 additive compatibility update: online/runtime outbound actions now also include `setUnitCommand` and `setUnitStance`, and runtime HUD additionally reports inbound counters for `takeItems` / `transferItemTo` / `transferItemToUnit` / `payloadDropped` / `pickedBuildPayload` / `pickedUnitPayload` / `unitDespawn`.
  - M8 additive mapper update: `mdt-input` now also exposes `IntentSamplingMode::LiveSampling` plus `LiveIntentState` semantics (pressed/released edges without repeated held-edge spam while active-action state persists), with runtime wiring remaining follow-up work.
  - M9: render/UI remains release-scope excluded or parity backlog, not release-complete.
  - M9 additive presentation update: `mdt-render-ui` now classifies render objects by semantic kind (`player/marker/plan/block/terrain/unknown`) and applies stable floor/clamp/zoom normalization in player-centered window crop sizing/focus math; this is low-risk presentation hardening, not Java desktop UI parity closure.
- Done when:
  - tracks are non-overlapping and subagent-ready
  - each track has concrete evidence targets

## 2026-03-23 Additional Parity Audit Backlog

### Session/Network Follow-Ups
- `medium` Java client still has broader server-discovery / host-probe behavior than Rust, but Rust now has a minimal usable discovery chain instead of direct-`--server` only.
  - Java evidence: `core/src/mindustry/net/ArcNetProvider.java:236`, `:273`; `core/src/mindustry/net/NetworkIO.java:100`
  - Rust evidence: `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`; `rust/mdt-client-min/src/arcnet_loop.rs`
- `high` disconnect / quiet-reset lifecycle is still materially simplified in Rust.
  - Java evidence: `core/src/mindustry/core/NetClient.java:120`, `:345`, `:657`
  - Rust evidence: `rust/mdt-client-min/src/client_session.rs:5118`; `rust/mdt-client-min/src/arcnet_loop.rs:147`
- `high` `clientLoaded` load-phase backlog dropping under the old 256-entry cap is now closed; the remaining gap is the semantic apply depth of replayed packets after load rather than whether normal-priority backlog survives loading.
  - Java evidence: `core/src/mindustry/net/Net.java:137`, `:292`
  - Rust evidence: `rust/mdt-client-min/src/client_session.rs:3949`
- `high` world/snapshot semantic apply depth remains the main post-release parity gap.
  - Java evidence: `core/src/mindustry/net/NetworkIO.java:64`; `core/src/mindustry/core/NetClient.java:485`, `:513`, `:539`
  - Rust evidence: `rust/mdt-client-min/src/bootstrap_flow.rs:218`; `rust/mdt-client-min/src/client_session.rs:3811`, `:4540`; `rust/mdt-client-min/src/snapshot_ingest.rs:48`

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
