# Agent Render Snapshot Gap Refresh (2026-03-24)

Purpose: keep a short, current list of Java-vs-Rust M7/M9 gaps that remain high-signal after the latest landed observability and presenter slices.

## Critical

- M7 snapshot apply still does not reach Java live-world semantics.
  - Java reference: `core/src/mindustry/core/NetClient.java`
    - `entitySnapshot`: `readSyncEntity -> entity.readSync -> snapSync -> add`
    - `hiddenSnapshot`: `handleSyncHidden()`
    - `blockSnapshot`: `tile.build.readSync(...)`
  - Rust boundary:
    - `rust/mdt-client-min/src/snapshot_ingest.rs`
    - `rust/mdt-client-min/src/client_session.rs`
    - `rust/mdt-client-min/src/session_state.rs`
  - Current Rust behavior is still centered on parse/projection, authority mirrors, loaded-world-assisted head/table updates, and conservative hidden-id cleanup rather than Java-style live entity/building ownership.

- M7 world-stream completion still lacks the final “enter the synchronized world” closure.
  - Java reference: `core/src/mindustry/core/NetClient.java` `finishConnecting()`
  - Rust boundary:
    - `rust/mdt-client-min/src/bootstrap_flow.rs`
    - `rust/mdt-world/src/lib.rs`
  - Rust bootstrap writes world/session state, but the repo still explicitly treats full `snapshotApply + readSyncEntity + handleSyncHidden + tile.build.readSync + stateSnapshot` closure as unfinished.

## High

- M7 still has many server->client remote families that fall through to `IgnoredPacket`.
  - Rust boundary: `rust/mdt-client-min/src/client_session.rs`
  - This is not just a HUD gap; it is remaining behavior surface that still lacks decode/apply/runtime semantics.

- M9 render remains a lightweight projection/presenter stack, not the Java renderer pipeline.
  - Java reference:
    - `core/src/mindustry/core/Renderer.java`
    - `core/src/mindustry/graphics/MinimapRenderer.java`
    - `core/src/mindustry/graphics/OverlayRenderer.java`
  - Rust boundary:
    - `rust/mdt-render-ui/src/render_model.rs`
    - `rust/mdt-render-ui/src/projection.rs`
    - `rust/mdt-render-ui/src/*presenter.rs`
  - Recent presenter depth should not be mistaken for parity with Java layered draw/fog/light/minimap/effect rendering.

- M9 UI still lacks the Java fragment/dialog interaction stack.
  - Java reference:
    - `core/src/mindustry/ui/UI.java`
    - `core/src/mindustry/ui/fragments/HudFragment.java`
    - `core/src/mindustry/ui/fragments/ChatFragment.java`
  - Rust boundary:
    - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
    - `rust/mdt-render-ui/src/hud_model.rs`
    - `rust/mdt-render-ui/src/*presenter.rs`
  - Rust still depends on CLI/runtime flags plus presenter summaries rather than a native in-process UI stack.

## Medium

- M9 HUD/chat/build surfaces are still observability summaries, not interactive UI implementations.
  - Rust boundary:
    - `rust/mdt-render-ui/src/hud_model.rs`
    - `rust/mdt-render-ui/src/window_presenter.rs`
    - `rust/mdt-render-ui/src/ascii_presenter.rs`
  - Java reference:
    - `core/src/mindustry/ui/fragments/HudFragment.java`
    - `core/src/mindustry/ui/fragments/ChatFragment.java`

- M9 desktop/mobile placement UI remains absent.
  - Java reference:
    - `core/src/mindustry/input/DesktopInput.java`
    - `core/src/mindustry/input/MobileInput.java`
  - Rust currently exposes build/config/runtime actions without a Java-like placement UI layer.

## Recommended Next Order

1. M7 world-sync closure: move `entity/block/hidden/state` from projection-only handling to controlled runtime apply.
2. M7 `IgnoredPacket` reduction: prioritize inbound families that change world/session behavior.
3. M9 low-risk depth: keep extending presenter consumption of existing structured state without overstating Java UI parity.
