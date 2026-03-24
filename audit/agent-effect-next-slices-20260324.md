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
| `257` / `260` `Fx.arcShieldBreak` / `Fx.unitShieldBreak` | Declared at `core/src/mindustry/content/Fx.java:2818` and `:2852`. Java uses parent `Unit` plus ability/unit-derived geometry, not just origin markers. | Rust already maps these ids to `unit_parent` and follows parent position, but still stops at marker-level positioning rather than effect-shaped arcs/circles. | Next narrow slice can deepen `unit_parent` into effect-specific geometry while keeping the current parent-follow binding path. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `263` `Fx.legDestroy` | Declared at `core/src/mindustry/content/Fx.java:2945`, called from `core/src/mindustry/entities/comp/LegsComp.java:79-80`. Java depends on `LegDestroyData` plus region/segment geometry. | Rust has no dedicated contract or executor for this family. | Defer until parent/segment executor depth is stronger; this is wider than the current shield slices. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |

## Suggested Order

Recommended implementation order:

1. `257` / `260` shield-break unit-parent geometry depth
2. `263` `legDestroy`

Why this order:

- `252`, `26`, and now `256` are already landed as narrow executor-focused slices, so the clearest remaining gap shifts to the parent-shaped shield families and then `legDestroy`.
- `257/260` can build directly on the already-landed `unit_parent` binding path.
- `263` is still real value, but it depends on wider segment/region semantics.

## Defer For Now

These are real gaps, but they are less suitable for the next narrow slice because they depend more directly on E3 parent-follow or custom payload/runtime state:

| candidate effect_id | reason to defer |
| --- | --- |
| `252` `Fx.healBlockFull` | Already landed as `block_content_icon`; do not re-open it as missing. |
| `26` `Fx.payloadDeposit` | Already landed as `payload_target_content`; do not re-open it as missing. |
| `263` `Fx.legDestroy` | Declared at `core/src/mindustry/content/Fx.java:2945`, called from `core/src/mindustry/entities/comp/LegsComp.java:79-80`. Java depends on `LegDestroyData` plus `TextureRegion`, so it is not a cheap next slice. |

## Backlog Alignment

- E1 at `audit/runtime-semantic-gap-backlog.md:49-58` says Rust still needs effect executors keyed by `effect_id`.
- E2 at `audit/runtime-semantic-gap-backlog.md:61-68` says Rust should add `effect_id -> data contract` mappings for the highest-signal effects first.
- E3 at `audit/runtime-semantic-gap-backlog.md:71-79` says parent-follow, rot-with-parent, start-delay, clip, and lifetime are still their own gap, so the next slice should avoid depending on those semantics unless necessary.

## Smallest Reasonable PR Shapes

If the next PR must stay very narrow, the best two options are:

- Executor-only PR
  - `effect_id=257`
- New-contract PR
  - `effect_id=257`
  - or `effect_id=263`

These options hit E1/E2 directly without expanding into the broader E3 runtime semantics.
