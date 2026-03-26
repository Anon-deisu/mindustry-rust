# Agent Render/UI Gap (2026-03-24)

Date: 2026-03-24
Scope: `mdt-render-ui` presenter-local low-risk parity backlog
Focus: presenter/model slices that do not require session/world/runtime behavior rewrites

## Already Landed

- `window` presenter already emits broad deterministic HUD/minimap/runtime status rows.
- presenter-local runtime surfaces already cover:
  - notice
  - menu
  - dialog
  - chat
  - command
  - admin
  - rules
  - world-label
  - kick/loading/reconnect
  - live-entity
  - live-effect
- build-side presentation is not blank.
  - `BuildConfigPanelModel`
  - `BuildInteractionPanelModel`
  - `BuildMinimapAssistPanelModel`
- `ascii` presenter already exposes more build detail rows than `window` in some areas.
- the older presenter-local backlog slices below are no longer open:
  - `BUILD-CONFIG-ENTRY`
  - `BUILD-CONFIG-MORE`
  - `BUILD`
  - `BUILD-INSPECTOR`
  - `BUILD-MINIMAP-AUX`

## Best Low-Risk Remaining Presenter Slices

1. Add explicit `RUNTIME-WORLD-RELOAD` detail output.
   - keep it read-only and derived from existing session observability

## Recommended Write Scope

- `rust/mdt-render-ui/src/window_presenter.rs`
- `rust/mdt-render-ui/src/ascii_presenter.rs`
- optionally `rust/mdt-render-ui/src/panel_model.rs` only if a tiny presenter-facing helper is missing

## Avoid Conflict With

- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/session_state.rs`
- `rust/mdt-client-min/src/snapshot_ingest.rs`
- `rust/mdt-world/src/lib.rs`

## Not This Lane

- Java interactive UI flow parity:
  - scene/dialog stack behavior
  - chat/console input handling
  - placement/editor fragments
  - fullscreen minimap pan/zoom/click
- renderer-pipeline parity:
  - Java `Renderer` multi-layer draw pipeline
  - deeper effect/minimap render ownership

These stay high-risk and should not be mixed into presenter-local formatting work.
