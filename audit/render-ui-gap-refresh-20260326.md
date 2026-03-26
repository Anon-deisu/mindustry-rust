# Render/UI Gap Refresh (2026-03-26)

## Scope

输入基线：
- `RUST_RELEASE_AUDIT_FINDINGS.md`
- `RUST_RELEASE_AUDIT_CONTINUATION.md`
- `audit/render-ui-parity.md`
- `audit/agent-render-ui-gap-20260324.md`

本次结论基于当前 Rust 代码复核，不直接复述旧 backlog。若旧审计项已被代码关闭，则不再列为缺口。

## 已确认关闭、不要重复开工的旧项

以下 presenter-local 缺口已经落地，不应再作为“仍未实现”统计：
- `window` 和 `ascii` 都已经输出 `BUILD-CONFIG`、`BUILD-CONFIG-ENTRY`、`BUILD-INSPECTOR`
- `window` 和 `ascii` 都已经输出 `RUNTIME-DIALOG`、`RUNTIME-CHAT`、`RUNTIME-WORLD-LABEL`、`RUNTIME-RECONNECT`
- `world_reload` 细节输出已经存在，不再是缺口
- `RenderPrimitive::Text` 已落地，`RuntimeWorldLabel` / `MarkerText` / `MarkerShapeText` 已能走 typed text primitive，被 `ascii` / `window` presenter 直接消费

## 仍然成立的高价值缺口

### Batch 1: 可并发 lane

#### Lane M9-LR1: Render primitive/model 扩展

- 分类：`低风险补丁`
- 价值：高
- 结论：当前 render model 仍然过窄，世界标签、文本标注、图标、矩形/区域、高级效果只能退化为“点对象”或由 `marker:line:*` 约定推导出的线段，明显低于 Java 侧 UI/render 表达力。
- 代码证据：
  - `RenderModel` 只有 `viewport`、`view_window`、`objects`；没有独立 text/icon/rect primitive 存储
  - `RenderPrimitive` 当前已有 `Line + Text`
  - `RenderObject` 只有 `id/layer/x/y`，没有文本内容、尺寸、颜色、朝向、图标等 render payload
  - `Text` 已覆盖 world-label / marker-text 这一小批高信号文本面，但更丰富的 icon/rect/area payload 仍缺
- 影响：
  - `world label`、`marker text`、runtime notice/icon 类覆盖层无法真正“画出来”，只能靠面板/状态行侧显
  - richer minimap、标签、效果表现继续被 primitive 模型上限卡住
- 证据：
  - `rust/mdt-render-ui/src/render_model.rs:7-23,50-56`
  - `rust/mdt-client-min/src/render_runtime.rs:4506-4517`
- 建议切法：
  - additive 地继续引入 `Icon/Rect` 或等价 richer typed primitive
  - 先不拆旧 `RenderObject` 路径，保留兼容层，避免 blast radius
- 建议改动文件：
  - `rust/mdt-render-ui/src/render_model.rs`
  - `rust/mdt-client-min/src/render_runtime.rs`

#### Lane M9-LR2: 非交互 minimap/visibility 视觉补强

- 分类：`低风险补丁`
- 价值：高
- 结论：当前 minimap 已经有 summary 和 inset，但仍是“窗口/计数/焦点”摘要，不是有内容密度的地图视图。
- 代码证据：
  - minimap panel 主要由 `HudSummary` 的 map/window/fog/counts 和 scene semantic counts 组装
  - window inset 只画背景、viewport 框、player 点、focus 点
  - ascii minimap 仍是文本摘要行
- 影响：
  - 即使不做交互，当前 minimap 也还明显弱于 Java 的可读性
  - 操作员对 fog、可见区、对象分布的理解仍然依赖文字而不是图形
- 证据：
  - `rust/mdt-render-ui/src/panel_model.rs:1395-1487`
  - `rust/mdt-render-ui/src/window_presenter.rs:3152-3259`
  - `rust/mdt-render-ui/src/ascii_presenter.rs:1473-1565`
- 建议切法：
  - 保持非交互范围，只补“显示密度”
  - 用现有 `HudSummary`、`semantic_summary`、window bounds 增加 fog/visible/hidden/object-density 的 minimap 可视层
- 建议改动文件：
  - `rust/mdt-render-ui/src/panel_model.rs`
  - `rust/mdt-render-ui/src/minimap_user_flow.rs`
  - `rust/mdt-render-ui/src/projection.rs`
  - `rust/mdt-render-ui/src/window_presenter.rs`
  - `rust/mdt-render-ui/src/ascii_presenter.rs`

#### Lane M9-ARCH1: 输入/事件回传通道

- 分类：`必须依赖更大架构改动`
- 价值：最高
- 结论：Rust 当前 render/UI 仍然是单向 presenter。没有 keyboard/mouse/touch 事件回流，任何“桌面 UI parity”都无从谈起。
- 代码证据：
  - `ScenePresenter` trait 只有 `present(&RenderModel, &HudModel)`，没有事件返回面
  - `MinifbWindowBackend` 只做 `is_open()` 和 `update_with_buffer()`，没有按键、鼠标、滚轮、点击、文本输入采集
  - online runtime 仍通过 CLI schedule 驱动聊天和动作，而不是窗口内交互
- 影响：
  - chat、menu、text input、placement、minimap pan/zoom/click 都不可能在现结构下做成真正 UI
  - 这不是 presenter 文案问题，而是缺少 input/event architecture
- 证据：
  - `rust/mdt-render-ui/src/scene_present.rs:6-7`
  - `rust/mdt-render-ui/src/window_presenter.rs:282-305`
  - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs:129-132`
  - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs:1931-2148`
  - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs:6768-6800`
- 建议改动文件：
  - `rust/mdt-render-ui/src/scene_present.rs`
  - `rust/mdt-render-ui/src/window_presenter.rs`
  - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
  - `rust/mdt-input/*`

### Batch 2: 依赖 Batch 1 或与其冲突，不建议并行落同文件

#### Lane M9-ARCH2: 交互式 chat/dialog/menu/placement UI

- 分类：`必须依赖更大架构改动`
- 价值：高
- 结论：当前 Rust 已有很多 `runtime_ui` observability，但本质仍是“观察面板”，不是 Java 那种交互式 UI 状态机和桌面控件层。
- 代码证据：
  - `HudModel` 里的 `runtime_ui` 是 observability projection，不是 widget tree
  - menu / text input 的“响应”仍通过 `--action-menu-choose` / `--action-text-input-result` 注入
- 影响：
  - 这解释了为什么当前 chat/dialog/menu 已“可观测”但仍明显不一致
  - 不能把当前状态误判成只差 presenter polish
- 证据：
  - `rust/mdt-render-ui/src/hud_model.rs:1-60`
  - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs:2357-2374`
- 依赖：
  - 先有 `M9-ARCH1` 的 input/event channel
- 建议改动文件：
  - `rust/mdt-render-ui/src/hud_model.rs`
  - `rust/mdt-render-ui/src/panel_model.rs`
  - `rust/mdt-render-ui/src/window_presenter.rs`
  - `rust/mdt-render-ui/src/ascii_presenter.rs`
  - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`

#### Lane M9-ARCH3: renderer fidelity / layered pipeline

- 分类：`必须依赖更大架构改动`
- 价值：高
- 结论：当前 window renderer 仍是 tile-grid 栅格化，再按 semantic family 着色；这和 Java 的 layered renderer 仍是类别级差距，不是简单补几行 panel text 能解决的。
- 代码证据：
  - `compose_frame()` 先生成 tile buffer，再把对象转成 point/line command 写像素
  - object color 只按 semantic family 选色
- 影响：
  - 没有 sprite/effect batching/light/fog/shader/pass 层
  - 即使上层状态更丰富，最终呈现仍会被这个单层 raster model 限死
- 证据：
  - `rust/mdt-render-ui/src/window_presenter.rs:321-369`
  - `rust/mdt-render-ui/src/window_presenter.rs:587-600`
- 依赖：
  - 至少部分依赖 `M9-LR1` 的 richer primitive/model
  - 若要逼近 Java，还会继续依赖更深的 content/effect/runtime 语义层
- 建议改动文件：
  - `rust/mdt-render-ui/src/render_model.rs`
  - `rust/mdt-render-ui/src/window_presenter.rs`
  - `rust/mdt-render-ui/src/projection.rs`
  - `rust/mdt-client-min/src/render_runtime.rs`

## 优先级建议

1. 先做 `M9-LR1`
- 这是当前最值的低风险切口
- 不需要先重做 session/world 架构
- 能直接提高 world-label / marker / effect / overlay 的表现上限

2. 并行做 `M9-LR2`
- 主要是 render-ui 本地计算和呈现补强
- 不碰协议和 session 行为，风险低

3. 再决定是否开 `M9-ARCH1`
- 一旦想做真正桌面交互 UI，这个是硬前置
- 没有它，`chat/dialog/minimap/placement` 只能继续停留在 CLI + observability

4. `M9-ARCH2`、`M9-ARCH3` 不应伪装成“小补丁”
- 它们都已经越过 presenter-local 范围
- 应按架构任务立项，不应混入低风险 lane

## 当前审计判断

- Rust 的 render/UI 已经明显超出“只有空壳输出”的阶段
- 但当前高价值剩余缺口已经集中到两类：
  - `低风险但高收益`：primitive/model 扩展、非交互 minimap 视觉补强
  - `必须架构化`：输入事件回传、交互式桌面 UI、真正 layered renderer
- 发布口径仍应继续停留在：
  - `minimal compatibility client`
  - `render/debug surfaces usable`
  - 不能宣称 Java desktop UI/render parity 已完成

## 涉及文件路径总表

### 低风险补丁

- `rust/mdt-render-ui/src/render_model.rs`
- `rust/mdt-client-min/src/render_runtime.rs`
- `rust/mdt-render-ui/src/panel_model.rs`
- `rust/mdt-render-ui/src/minimap_user_flow.rs`
- `rust/mdt-render-ui/src/projection.rs`
- `rust/mdt-render-ui/src/window_presenter.rs`
- `rust/mdt-render-ui/src/ascii_presenter.rs`

### 更大架构改动

- `rust/mdt-render-ui/src/scene_present.rs`
- `rust/mdt-render-ui/src/window_presenter.rs`
- `rust/mdt-render-ui/src/hud_model.rs`
- `rust/mdt-render-ui/src/panel_model.rs`
- `rust/mdt-render-ui/src/render_model.rs`
- `rust/mdt-render-ui/src/projection.rs`
- `rust/mdt-render-ui/src/ascii_presenter.rs`
- `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
- `rust/mdt-client-min/src/render_runtime.rs`
- `rust/mdt-input/*`
