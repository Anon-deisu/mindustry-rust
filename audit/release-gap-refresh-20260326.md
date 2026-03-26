# Release Gap Refresh (2026-03-26)

目的：给后续继续分发用，只保留当前代码里仍然高价值、且未完成的 gap。按可并行写入 lane 划分；`client_session.rs` / `session_state.rs` 视为热点区，不要并行派两个人。

## 不要重开

- `unitCapDeath` 已闭合，不要再按未完成派发。
  - 现在会清理 resource delta 并移除 entity projection：`rust/mdt-client-min/src/client_session.rs:5549`
  - 回归已断言 `704` 被删、墓碑已写、资源镜像已清：`rust/mdt-client-min/src/client_session.rs:37240`
- 玩家语义镜像已落地，不再是优先缺口。
  - `EntityPlayerSemanticProjection` 已包含 `admin/name/color/team/mouse/selectedBlock/selectedRotation/typing/shooting/boosting`：`rust/mdt-client-min/src/session_state.rs:4062`
- building merged live view 第一刀已落地，不要再按“完全没有 merged view”派发。
  - `rust/mdt-client-min/src/client_session.rs:1540`
  - `rust/mdt-client-min/src/client_session.rs:1613`
- UI 线原始 `Line` primitive 已落地，不要再按“没有 primitive 通道”派发。
  - `rust/mdt-render-ui/src/render_model.rs:13`
- batch sampling / override consistency 已落地，不要再把空 batch stale edge cleanup 或 override 尊重问题当成独立 lane。

## Lane A: Snapshot Entity Semantic Widening

- 优先级：`P0`
- 并行性：`串行热点`，独占 `client_session.rs` + `session_state.rs`
- 归属：`session_state.rs` / `client_session.rs` / `protocol-snapshot`
- 当前缺口：
  - `EntityUnitSemanticProjection` 仍只保留薄语义：`team/unit_type/health/rotation/shield/mine/status/payload/building/lifetime/time/controller`
  - 证据：`rust/mdt-client-min/src/session_state.rs:4106`
  - bounded runtime-sync widen 已部分落地：typed runtime unit mirror 当前已保留 `ammo_bits/elevation_bits/flag_bits`，机械族额外保留 `base_rotation_bits`，并接上当前覆盖 family 的 carried-item stack mirror
  - 证据：`rust/mdt-client-min/src/client_session.rs:7436`、`rust/mdt-client-min/src/client_session.rs:7572`
  - 当前剩余 gap 已转向更广的 family breadth 和更深的 Java live apply semantics，而不再是这批字段完全缺失
- 最小切法：
  - 继续沿 runtime semantic mirror / typed runtime entity 路径扩更广 family breadth
  - 在已 landed 的 bounded runtime-sync widen 之上补 status/payload/controller 等更深语义
  - 保持在“runtime semantic mirror”层，不碰 Java 级 group attach / live world ownership
- 完成判定：
  - 新字段从 parseable row 落到 runtime typed entity
  - HUD / debug / ownership 侧可消费这些字段
  - 不改 `worldDataBegin` / defer replay / reconnect 逻辑

## Lane B: Building Runtime Family Widening

- 优先级：`P1`
- 并行性：`串行热点`，排在 Lane A 后
- 归属：`client_session.rs` / `session_state.rs` / `blockSnapshot`
- 当前缺口：
  - merged building live view 已有，但 typed runtime building 仍是白名单模型，未覆盖的 block family 直接 `return None`
  - 证据：`rust/mdt-client-min/src/client_session.rs:1540`、`rust/mdt-client-min/src/session_state.rs:3126`、`rust/mdt-client-min/src/session_state.rs:3440`
  - loaded-world tail -> business fold 也是按 family 白名单推进，仍不是广义 `tile.build.readSync(..., version)` 级语义
  - 证据：`rust/mdt-client-min/src/client_session.rs:8231`
- 最小切法：
  - 一次只扩一个低风险 family batch
  - 优先选当前已解析 tail 但 runtime model 未承接充分的家族
  - 保持 head+tail keyed by `block/revision` 的原子更新
- 不要混入：
  - reconnect
  - `clientLoaded`
  - entity ownership

## Lane C: Reconnect Command Surface

- 优先级：`P1`
- 并行性：`可并行`
- 归属：`mdt-client-min-online.rs` / `arcnet_loop.rs` / `udp_loop.rs`
- 当前缺口：
  - reconnect executor 仍只存在于 online bin，本质还是 loop-local policy，不是 durable session command
  - 证据：`rust/mdt-client-min/src/bin/mdt-client-min-online.rs:552`
  - online 主循环仍靠 `events + report.timed_out` 驱动调度
  - 证据：`rust/mdt-client-min/src/bin/mdt-client-min-online.rs:173`
  - TCP/ArcNet 会回报 `timed_out_kind`，UDP 侧还没有同等信息
  - 证据：`rust/mdt-client-min/src/arcnet_loop.rs:186`、`rust/mdt-client-min/src/udp_loop.rs:89`
- 最小切法：
  - 先补齐 UDP `timed_out_kind`
  - 把 redirect / restart / timeout 的 intent surface 统一
  - 先不碰 `clientLoaded` / deferred replay / finishConnecting` 串行区

## Lane D: Effect Contract / Executor Deepening

- 优先级：`P1`
- 并行性：`可并行`
- 归属：`client_session.rs/effect_runtime.rs`，但优先避开 `client_session.rs`，先做 `effect_runtime.rs` + `render_runtime`
- 当前缺口：
  - contract table 仍是窄覆盖，只有少数 `effect_id` 被映射
  - 证据：`rust/mdt-client-min/src/effect_runtime.rs:120`
  - source-follow 只对 `8/9/178/261/262` 开启
  - 证据：`rust/mdt-client-min/src/effect_runtime.rs:922`
  - instance seed 仍是对 overlay 字段做 hash，不是稳定 Java effect instance id 语义
  - 证据：`rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs:675`
- 最小切法：
  - 三选一，不要混做大杂烩
  - 扩一批高信号 `effect_id -> contract`
  - 扩 `position_target` 更广的 source-follow
  - 补 binding/fallback outcome observability 和 seed parity
- 完成判定：
  - 新增 family 有明确 executor
  - runtime overlay 不再只靠 generic marker/line fallback

## Lane E: UI / Render Primitive Model

- 优先级：`P1`
- 并行性：`可并行`
- 归属：`UI/渲染`
- 写入范围：`mdt-render-ui/src/render_model.rs` + presenter 层；尽量不碰 `client_session.rs`
- 当前缺口：
  - `RenderPrimitive` 已不再只有 `Line`；`Text` primitive 已 landed，并由 `RuntimeWorldLabel` / `MarkerText` / `MarkerShapeText` 派生
  - 证据：`rust/mdt-render-ui/src/render_model.rs:13`
  - 当前剩余 gap 是 richer typed primitive breadth，尤其 `Icon/Rect` 或等价 payload，而不是 text primitive 缺席
  - presenter 侧 notice/chat/dialog/build-config 文字摘要已经很多，当前瓶颈在 primitive model，不在 summary text
- 最小切法：
  - 先补 `Icon` / `Rect` 或等价 richer typed primitive
  - presenter 优先消费 primitive，而不是继续从 object id 解析
  - 不做完整交互 minimap/dialog 生命周期

## Lane F: finishConnecting / clientLoaded 原子性

- 优先级：`P1`
- 并行性：`串行专属`
- 归属：`协议/快照/生命周期`
- 写入范围：`client_session.rs` + `arcnet_loop.rs` + `udp_loop.rs`
- 当前缺口：
  - `connect_confirm_flushed` 已显式跟踪，但 transport/lifecycle atomicity 还没有完全收口
  - 证据：`rust/mdt-client-min/src/session_state.rs:4769`
  - `connectConfirm` flushed 位目前主要靠 loop 写入，仍属于 transport-report 驱动
  - 证据：`rust/mdt-client-min/src/client_session.rs:40713`、`rust/mdt-client-min/src/client_session.rs:40883`
- 最小切法：
  - 保持这个 lane 单独串行
  - 不与 Lane A/B/C 混做
  - 目标是 finishConnecting / replay / reconnect 边界的一致命令面，不是再补 observability

## 建议派发顺序

1. Lane A
2. Lane C + Lane D + Lane E 并行
3. Lane B
4. Lane F 单独串行

## 涉及文件路径

- `rust/mdt-client-min/src/session_state.rs`
- `rust/mdt-client-min/src/client_session.rs`
- `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
- `rust/mdt-client-min/src/effect_runtime.rs`
- `rust/mdt-client-min/src/render_runtime.rs`
- `rust/mdt-client-min/src/render_runtime/effect_contract_executor.rs`
- `rust/mdt-client-min/src/arcnet_loop.rs`
- `rust/mdt-client-min/src/udp_loop.rs`
- `rust/mdt-render-ui/src/render_model.rs`
- `rust/mdt-render-ui/src/ascii_presenter.rs`
- `rust/mdt-render-ui/src/window_presenter.rs`
- `rust/mdt-render-ui/src/panel_model.rs`
- `rust/mdt-world/src/lib.rs`
