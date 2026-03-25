# Agent Effect Next Slices 2026-03-24

## Scope

This note compares Java `effect / effect(data)` behavior against the current Rust runtime and
selects the best narrow next slices for executor/contract work.

Primary references:

- Java
  - `core/src/mindustry/core/NetClient.java:213-227`
  - `core/src/mindustry/entities/Effect.java:33-41`
  - `core/src/mindustry/entities/Effect.java:133-170`
  - `core/src/mindustry/content/Fx.java`
- Rust
  - `rust/mdt-client-min/src/effect_runtime.rs:13-17`
  - `rust/mdt-client-min/src/effect_runtime.rs:45-50`
  - `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs:19-38`
  - `rust/mdt-client-min/src/client_session.rs:10357-10435`
  - `rust/mdt-client-min/src/client_session.rs:10514-10589`
- Backlog
  - `audit/runtime-semantic-gap-backlog.md:49-79`

Current Rust baseline:

- Contracts currently implemented:
  - `position_target`
  - `lightning`
  - `point_beam`
  - `point_hit`
  - `shield_break`
  - `block_content_icon`
  - `content_icon`
  - `payload_target_content`
  - `drop_item`
  - `float_length`
  - `unit_parent`
- Backlog still says:
  - E1: effect runtime still lacks effect-specific executors.
  - E2: `effect(..., data)` still relies too much on generic projection.
  - E3: parent-follow, rot-with-parent, start delay, clip, and lifetime semantics are still partial.

## Best Next Narrow Slices

| candidate effect_id | Java behavior summary | Rust status | recommended next narrow slice | involved Rust files |
| --- | --- | --- | --- | --- |
| `256` `Fx.shieldBreak` | Declared at `core/src/mindustry/content/Fx.java:2807`. Java fallback path draws a hexagon at `e.x/e.y` using `e.rotation + e.fin()` even when no typed ability payload is available. | Rust now maps `effect_id=256` to `shield_break` and renders the fallback-style expanding hexagon as runtime line segments keyed by effect origin + `rotation`. | Landed. Keep as the current narrow fallback executor reference point for future parent/ability-aware shield work. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `257` / `260` `Fx.arcShieldBreak` / `Fx.unitShieldBreak` | Declared at `core/src/mindustry/content/Fx.java:2818` and `:2852`. Java uses parent `Unit` plus ability/unit-derived geometry, not just origin markers. | Rust still maps these ids to `unit_parent`, and now also renders effect-specific fallback geometry on top of the existing parent-follow binding path: `257` gets a parent-rotation-aware double-arc band, `260` gets a parent-centered circle-plus-burst fallback. | Landed as a narrow fallback executor deepening. Remaining gap is metadata depth, not total absence: `257` still lacks `ShieldArcAbility` radius/width/offset parameters, and `260` still lacks `unit_type -> hitSize` data for exact radius parity. | `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `11` `Fx.pointHit` | Declared at `core/src/mindustry/content/Fx.java:161`. Java draws an expanding hit ring centered at the effect position and does not require a deeper typed payload than the effect origin itself. | Rust now maps `effect_id=11` to `point_hit`, keeps the dedicated contract name on the session surface, and renders an expanding circle fallback as runtime line segments keyed by effect position. | Landed as a narrow contract/executor slice. Keep it closed; remaining U5 work is other `effect_id -> contract/executor` families and deeper lifetime parity, not re-opening `pointHit` as missing. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `8` `Fx.unitSpirit` | Declared at `core/src/mindustry/content/Fx.java:120`, called from `core/src/mindustry/input/InputHandler.java:811`. Java moves two 45-degree squares from source to target with different eased interpolation curves. | Rust still keeps `effect_id=8` on the existing `position_target` contract, and now renders a narrow double-diamond fallback from the captured source/target bits instead of stopping at the target marker alone. | Landed as an executor-only slice. Keep it closed; remaining work is `itemTransfer` and deeper lifetime/parent-follow parity, not re-opening `unitSpirit` as a missing first-pass executor. | `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `263` `Fx.legDestroy` | Declared at `core/src/mindustry/content/Fx.java:2945`, called from `core/src/mindustry/entities/comp/LegsComp.java:79-80`. Java depends on `LegDestroyData` plus region/segment geometry. | Rust has no dedicated contract or executor for this family. | Defer until parent/segment executor depth is stronger; this is wider than the current shield slices. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |

## Suggested Order

Recommended implementation order:

1. `9` `itemTransfer`
2. `263` `legDestroy`

Why this order:

- `11` `pointHit` is now landed as a dedicated contract/executor slice, so it should not stay in the next-slice pool.
- `8` `unitSpirit` is now also landed as an executor-only slice, so the cheapest remaining source-target mover shifts to `9` `itemTransfer`.
- `9` `itemTransfer` is still narrow, but its Java look depends on a stable effect-instance seeded lateral offset; the next slice should either add that deterministic seed surface or land a clearly documented conservative fallback.
- `263` is still real value, but it depends on wider segment/region semantics.

## Defer For Now

These are real gaps, but they are less suitable for the next narrow slice because they depend more directly on E3 parent-follow or custom payload/runtime state:

| candidate effect_id | reason to defer |
| --- | --- |
| `252` `Fx.healBlockFull` | Already landed as `block_content_icon`; do not re-open it as missing. |
| `26` `Fx.payloadDeposit` | Already landed as `payload_target_content`; do not re-open it as missing. |
| `11` `Fx.pointHit` | Already landed as `point_hit`; Rust now keeps the dedicated contract name and renders an expanding hit-ring fallback from the effect position, so do not re-open it as the next missing contract slice. |
| `8` `Fx.unitSpirit` | Already landed as a `position_target`-backed executor slice; Rust now renders a double-diamond fallback from the captured source/target bits, so do not re-open it as the next missing executor slice. |
| `263` `Fx.legDestroy` | Declared at `core/src/mindustry/content/Fx.java:2945`, called from `core/src/mindustry/entities/comp/LegsComp.java:79-80`. Java depends on `LegDestroyData` plus `TextureRegion`, so it is not a cheap next slice. |
| `257` / `260` exact parity revisit | First fallback geometry slice is landed; defer any revisit until Rust can source `ShieldArcAbility` metadata and `unit_type -> hitSize` instead of widening this PR. |

## Backlog Alignment

- E1 at `audit/runtime-semantic-gap-backlog.md:49-58` says Rust still needs effect executors keyed by `effect_id`.
- E2 at `audit/runtime-semantic-gap-backlog.md:61-68` says Rust should add `effect_id -> data contract` mappings for the highest-signal effects first.
- E3 at `audit/runtime-semantic-gap-backlog.md:71-79` says parent-follow, rot-with-parent, start-delay, clip, and lifetime are still their own gap, so the next slice should avoid depending on those semantics unless necessary.

## Smallest Reasonable PR Shapes

If the next PR must stay very narrow, the best two options are:

- Executor-only PR
  - `effect_id=9`
- New-contract PR
  - `effect_id=263`

These options hit E1/E2 directly without expanding into the broader E3 runtime semantics.
