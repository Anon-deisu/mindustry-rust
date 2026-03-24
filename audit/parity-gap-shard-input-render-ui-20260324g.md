# Parity Gap Shard Input/Render/UI (2026-03-24g)

Purpose: freeze the highest-signal remaining input/render/UI gaps so later workers can dispatch from a current shard instead of repeating a broad audit.

## High

- runtime input capture is still far thinner than Java desktop/mobile bindings.
  - Java anchor: `core/src/mindustry/input/InputHandler.java`
  - Java anchor: `core/src/mindustry/input/DesktopInput.java`
  - Java anchor: `core/src/mindustry/input/MobileInput.java`
  - Rust anchor: `rust/mdt-input/src/live_intent.rs`
  - Rust anchor: `rust/mdt-input/src/mapper.rs`
  - Rust anchor: `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
  - Current state: Rust now has one-shot live-intent schedule override instead of sticky schedule-only behavior, but still lacks real keyboard/mouse/touch capture.

- command-mode state container is still missing.
  - Java anchor: command selection/control state in desktop/mobile input paths
  - Rust anchor: `rust/mdt-client-min/src/client_session.rs`
  - Rust anchor: `rust/mdt-input/`
  - Current state: Rust can emit command packets and keep last-observed command facts, but not Java-like `selectedUnits` / `commandBuildings` / `commandRect` / control-group state.

- build placement/config interaction chain is still mostly read-only.
  - Java anchor: `core/src/mindustry/ui/fragments/PlacementFragment.java`
  - Java anchor: `core/src/mindustry/ui/fragments/PlanConfigFragment.java`
  - Rust anchor: `rust/mdt-render-ui/`
  - Rust anchor: `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
  - Current state: Rust has build/config observability and panel summaries, but not interactive placement/config UI flow.

## Medium

- render pipeline is still presenter-centric, not Java layered renderer/effects/minimap depth.
  - Java anchor: `core/src/mindustry/core/Renderer.java`
  - Rust anchor: `rust/mdt-render-ui/src/projection.rs`
  - Rust anchor: `rust/mdt-render-ui/src/render_model.rs`
  - Current state: Rust now preserves projected `view_window` through projection/presenter/minimap paths, but the renderer stack is still intentionally shallow.

- effect runtime semantics remain partial even with more contract-aware overlays.
  - Java anchor: `core/src/mindustry/entities/Effect.java`
  - Rust anchor: `rust/mdt-client-min/src/render_runtime.rs`
  - Rust anchor: `rust/mdt-client-min/src/client_session.rs`

## Low

- do not re-audit as missing:
  - typed render `view_window` preservation across projection/presenter/minimap is landed
  - live-intent schedule override now yields back to runtime sampling after the due tick
  - build-inspector/panel/minimap read-only summaries are landed

## Immediate Next 3

- add a real runtime input adapter/binding profile on top of `LiveIntentState`
- add a minimal command-mode state container instead of last-packet observability only
- add interactive build/config UI flow rather than only panel/readout summaries
