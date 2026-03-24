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
| `10` `Fx.pointBeam` | Declared at `core/src/mindustry/content/Fx.java:152`. Requires `e.data instanceof Position`; draws a straight beam from `e.x/e.y` to target and adds light. Used by `core/src/mindustry/type/weapons/PointDefenseWeapon.java:27` and `core/src/mindustry/world/blocks/defense/turrets/PointDefenseTurret.java:26`. | Already mapped to `position_target` in `rust/mdt-client-min/src/effect_runtime.rs:47`. Current executor only projects origin/target for overlay placement via `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs:19-24`, `:95-108`, `:233-257`; it does not execute a beam. | Do not add a new contract. Add an effect-specific beam executor for `effect_id=10` on top of existing `PositionTarget { source, target }` business projection. This is the cleanest executor-only slice. | `rust/mdt-client-min/src/render_runtime.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs` |
| `261` `Fx.chainLightning` and `262` `Fx.chainEmp` | Declared at `core/src/mindustry/content/Fx.java:2871` and `:2908`. Both require `e.data instanceof Position`; both render segmented jittered chains from source to target. Both explicitly use `.followParent(false).rotWithParent(false)`. Used by `core/src/mindustry/entities/abilities/EnergyFieldAbility.java:25` and `core/src/mindustry/entities/bullet/EmpBulletType.java:12`. | Both IDs are already in `position_target` at `rust/mdt-client-min/src/effect_runtime.rs:47`, so Rust can already recover source and target. Current runtime still stops at marker/overlay semantics and has no chain executor. | Reuse the existing `position_target` contract. Add a deterministic segmented chain executor for `effect_id=261/262`. First slice does not need Java-perfect jitter; it only needs effect-specific chain rendering. | `rust/mdt-client-min/src/render_runtime.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs` |
| `13` `Fx.lightning` | Declared at `core/src/mindustry/content/Fx.java:188`. Requires `e.data instanceof Seq<Vec2>`; Java renders the entire polyline and endpoint circles. Call site: `core/src/mindustry/entities/Lightning.java:94`. | No `effect_id=13` mapping exists in `rust/mdt-client-min/src/effect_runtime.rs:45-50`. Generic business projection in `rust/mdt-client-min/src/client_session.rs:10536-10589` can only keep a first hit or first hint, so full path semantics are lost. | Add a new strict `vec2_polyline` or `lightning_path` contract that only consumes `Vec2[]/Seq<Vec2>`, then add a narrow executor that draws connected segments and nodes. This is the clearest contract gap left in `effect(data)`. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `252` `Fx.healBlockFull` | Declared at `core/src/mindustry/content/Fx.java:2781`. Requires `e.data instanceof Block`; Java mixes `e.color` and draws `block.fullIcon` at the effect origin. Used by `core/src/mindustry/world/blocks/defense/MendProjector.java:114`, `core/src/mindustry/entities/abilities/EnergyFieldAbility.java:168`, and `core/src/mindustry/entities/bullet/EmpBulletType.java:31`. | Rust generic parsing can already recover content references, but there is no `effect_id=252` contract in `rust/mdt-client-min/src/effect_runtime.rs:45-50`. Existing `drop_item` only covers item content semantics, not block icon semantics. | Add a `block_content_icon` contract limited to `Content(Block)` and a narrow executor that shows a block icon at the effect origin. This is a low-risk content-ref slice. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |
| `26` `Fx.payloadDeposit` | Declared at `core/src/mindustry/content/Fx.java:295`. Requires `YeetData(target, item)`; Java lerps from source to target and draws either block or unit payload content. Call site: `core/src/mindustry/world/blocks/units/UnitAssembler.java:675`. | No dedicated contract for `26`. Generic DFS may recover one `Vec2` or one `ContentRef`, but not the paired `target + payload content` semantics needed for motion and payload choice. | Add a `payload_target_content` contract that extracts one target position plus one payload content ref. First executor slice can just lerp an icon along the path; it does not need Java shadow fidelity. | `rust/mdt-client-min/src/effect_runtime.rs`; `rust/mdt-client-min/src/client_session.rs`; `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`; `rust/mdt-client-min/src/render_runtime.rs` |

## Suggested Order

Recommended implementation order:

1. `10` `pointBeam`
2. `261/262` `chainLightning` / `chainEmp`
3. `13` `lightning`
4. `252` `healBlockFull`
5. `26` `payloadDeposit`

Why this order:

- `10` and `261/262` are executor-only slices. They prove that existing contracts can graduate into effect-specific runtime behavior without expanding contract surface.
- `13` is the clearest remaining `effect(data)` contract gap because generic first-hit projection cannot preserve the whole polyline.
- `252` is a very clean content-ref slice.
- `26` is still a good slice, but it needs paired target+content extraction, so it is slightly wider.

## Defer For Now

These are real gaps, but they are less suitable for the next narrow slice because they depend more directly on E3 parent-follow or custom payload/runtime state:

| candidate effect_id | reason to defer |
| --- | --- |
| `256` `Fx.shieldBreak` | Declared at `core/src/mindustry/content/Fx.java:2807`. Java uses `ForceFieldAbility` data, and `Effect.add(...)` parent binding in `core/src/mindustry/entities/Effect.java:162-170` matters. This is closer to parent-follow and ability-shape runtime work than to a narrow contract slice. |
| `257` `Fx.arcShieldBreak` and `260` `Fx.unitShieldBreak` | Rust already maps these IDs to `unit_parent` in `rust/mdt-client-min/src/effect_runtime.rs:50`, but Java behavior still needs unit/ability-specific shape logic. Better after parent-follow executor infrastructure is stronger. |
| `263` `Fx.legDestroy` | Declared at `core/src/mindustry/content/Fx.java:2945`, called from `core/src/mindustry/entities/comp/LegsComp.java:79-80`. Java depends on `LegDestroyData` plus `TextureRegion`, so it is not a cheap next slice. |

## Backlog Alignment

- E1 at `audit/runtime-semantic-gap-backlog.md:49-58` says Rust still needs effect executors keyed by `effect_id`.
- E2 at `audit/runtime-semantic-gap-backlog.md:61-68` says Rust should add `effect_id -> data contract` mappings for the highest-signal effects first.
- E3 at `audit/runtime-semantic-gap-backlog.md:71-79` says parent-follow, rot-with-parent, start-delay, clip, and lifetime are still their own gap, so the next slice should avoid depending on those semantics unless necessary.

## Smallest Reasonable PR Shapes

If the next PR must stay very narrow, the best two options are:

- Executor-only PR
  - `effect_id=10`
  - `effect_id=261`
  - `effect_id=262`
- New-contract PR
  - `effect_id=13`
  - or `effect_id=252`

These options hit E1/E2 directly without expanding into the broader E3 runtime semantics.
