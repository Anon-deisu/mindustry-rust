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
  - `leg_destroy`
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
  - E3: wider source-follow beyond `8/9`, general building-parent offset follow, binding/fallback observability, and stable effect-instance parity are still partial; `rotWithParent`, `startDelay`, `clip`, and the first lifetime-aware overlay path are already landed.

## Best Next Narrow Slices

| candidate effect_id | Java behavior summary | Rust status | recommended next narrow slice | involved Rust files |
| --- | --- | --- | --- | --- |
| `256` `Fx.shieldBreak` | Declared at `core/src/mindustry/content/Fx.java:2807`. Java fallback path draws a hexagon at `e.x/e.y` using `e.rotation + e.fin()` even when no typed ability payload is available. | Rust now maps `effect_id=256` to `shield_break` and renders the fallback-style expanding hexagon as runtime line segments keyed by effect origin + `rotation`. | Landed. Keep as the current narrow fallback executor reference point for future parent/ability-aware shield work. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `257` / `260` `Fx.arcShieldBreak` / `Fx.unitShieldBreak` | Declared at `core/src/mindustry/content/Fx.java:2818` and `:2852`. Java uses parent `Unit` plus ability/unit-derived geometry, not just origin markers. | Rust still maps these ids to `unit_parent`, now renders effect-specific fallback geometry, and now also freezes a relative spawned offset on the first authoritative entity-table hit instead of rebinding every frame to the parent origin. | Landed as a narrow fallback-executor plus parent-offset slice. Remaining gap is metadata depth and fallback depth, not total absence: `257` still lacks `ShieldArcAbility` radius/width/offset parameters, `260` still lacks `unit_type -> hitSize`, and snapshot/world-player fallback paths still do not preserve Java-equivalent relative offsets. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `11` `Fx.pointHit` | Declared at `core/src/mindustry/content/Fx.java:161`. Java draws an expanding hit ring centered at the effect position and does not require a deeper typed payload than the effect origin itself. | Rust now maps `effect_id=11` to `point_hit`, keeps the dedicated contract name on the session surface, and renders an expanding circle fallback as runtime line segments keyed by effect position. | Landed as a narrow contract/executor slice. Keep it closed; remaining U5 work is other `effect_id -> contract/executor` families and deeper lifetime parity, not re-opening `pointHit` as missing. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `8` `Fx.unitSpirit` | Declared at `core/src/mindustry/content/Fx.java:120`, called from `core/src/mindustry/input/InputHandler.java:811`. Java moves two 45-degree squares from source to target with different eased interpolation curves. | Rust still keeps `effect_id=8` on the existing `position_target` contract, renders a narrow double-diamond fallback from the captured source/target bits, and now also carries a source-follow binding so the spawned source point moves with a parent `Unit`. | Landed as an executor-plus-source-follow slice. Keep it closed; remaining work is wider `rotWithParent` / parent-follow parity, not re-opening `unitSpirit` as a missing first-pass executor. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `9` `Fx.itemTransfer` | Declared at `core/src/mindustry/content/Fx.java:138`, called from `core/src/mindustry/input/InputHandler.java:312`. Java moves a mid-life-tapered circle pair along a `pow3` source-target curve with an `e.id`-seeded lateral offset. | Rust still keeps `effect_id=9` on the existing `position_target` contract, renders a conservative pseudo-seeded double-ring fallback plus a marker-position override, and now also carries a source-follow binding so the spawned source point moves with a parent `Unit`. | Landed as an executor-plus-source-follow slice. Keep it closed as a first-pass implementation; exact Java parity still needs a stable effect-instance seed equivalent to `e.id`, but the family is no longer absent. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `263` `Fx.legDestroy` | Declared at `core/src/mindustry/content/Fx.java:2945`, called from `core/src/mindustry/entities/comp/LegsComp.java:79-80`. Java depends on `LegDestroyData` plus region/segment geometry. | Rust now maps this id to `leg_destroy`, projects the line target from the second explicit position with first-position fallback, and renders a dedicated runtime line fallback instead of a generic marker. | Landed as a first-pass contract/executor slice. Keep it closed; remaining work is deeper segment/region geometry and effect-instance parity, not re-opening `legDestroy` as a missing family. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |

## Suggested Order

Recommended implementation order:

1. binding / fallback observability
2. `9` exact-parity seed support
3. wider `position_target` source-follow beyond `8` / `9`

Why this order:

- `257` / `260` no longer belong at the front of the queue for first-pass parent-follow work; their narrow lazy-offset slice is landed.
- `8` `unitSpirit` and `9` `itemTransfer` now also have their first source-follow slice, so they should leave the front of the queue.
- exact `9` parity still also needs a stable effect-instance seeded lateral offset; that remains a later parity revisit.
- `rotWithParent`, `startDelay`, and `clip` are already landed, so they should leave the front of the queue.
- `263` is now also landed as a first-pass slice, so the next value is semantic deepening rather than opening a new family.

## Defer For Now

These are real gaps, but they are less suitable for the next narrow slice because they depend more directly on E3 parent-follow or custom payload/runtime state:

| candidate effect_id | reason to defer |
| --- | --- |
| `252` `Fx.healBlockFull` | Already landed as `block_content_icon`; do not re-open it as missing. |
| `26` `Fx.payloadDeposit` | Already landed as `payload_target_content`; do not re-open it as missing. |
| `11` `Fx.pointHit` | Already landed as `point_hit`; Rust now keeps the dedicated contract name and renders an expanding hit-ring fallback from the effect position, so do not re-open it as the next missing contract slice. |
| `8` `Fx.unitSpirit` | Already landed as a `position_target`-backed executor slice; Rust now renders a double-diamond fallback from the captured source/target bits, so do not re-open it as the next missing executor slice. |
| `9` `Fx.itemTransfer` | Already landed as a conservative `position_target`-backed executor slice; Rust now renders pseudo-seeded double rings and moves the marker along the fallback curve, so do not re-open it as a first-pass missing executor. |
| `263` `Fx.legDestroy` | Already landed as a first-pass contract/executor slice; defer any revisit until Rust is ready to deepen segment/region geometry and effect-instance parity. |
| `9` exact parity revisit | First fallback executor slice is landed; defer any revisit until Rust can carry a stable effect-instance seed equivalent to Java `e.id` instead of widening the runtime state ad hoc. |
| `257` / `260` exact parity revisit | First fallback geometry slice is landed; defer any revisit until Rust can source `ShieldArcAbility` metadata and `unit_type -> hitSize` instead of widening this PR. |

## Backlog Alignment

- E1 at `audit/runtime-semantic-gap-backlog.md:49-58` says Rust still needs effect executors keyed by `effect_id`.
- E2 at `audit/runtime-semantic-gap-backlog.md:61-68` says Rust should add `effect_id -> data contract` mappings for the highest-signal effects first.
- E3 now mainly points at wider source-follow, building-parent offset behavior, binding/fallback observability, and stable effect-instance parity rather than re-opening already landed `rotWithParent` / `startDelay` / `clip`.

## Smallest Reasonable PR Shapes

If the next PR must stay very narrow, the best remaining option is:

- Effect observability PR
  - explicit binding/fallback outcome counters or state for rejected/unresolved parent-follow cases

The cheapest U5 gaps are no longer total absences; the next narrow work is semantic deepening such as binding observability, `9` seed parity, or wider source-follow beyond `8/9`.
