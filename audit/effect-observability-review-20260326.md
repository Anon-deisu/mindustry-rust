# Effect Binding Observability Review
Date: 2026-03-26

## 范围
审查以下文件中 effect binding observability 相关实现，只做读审查：
- `D:\MDT\mindustry\rust\mdt-client-min\src\effect_runtime.rs`
- `D:\MDT\mindustry\rust\mdt-client-min\src\render_runtime.rs`
- `D:\MDT\mindustry\rust\mdt-client-min\src\client_session.rs`
- `D:\MDT\mindustry\rust\mdt-client-min\src\session_state.rs`

## 结论
现有实现已经覆盖了部分 target binding/source binding 的解析与 overlay 跟随，但 observability 仍有几处明显缺口：一部分分支只实现未做端到端断言，一部分 HUD 展示存在旧 overlay 覆盖最新 packet 状态的风险，另有 building follow 的可观测语义偏乐观。

## Findings

### High
1. source binding observability 的端到端测试基本缺失，回归后很可能静默失真
- `client_session` 在 effect-with-data 路径会写入 `last_effect_runtime_source_binding_state`，见 `client_session.rs:5908-5914`。
- 但仓库内对 `last_effect_runtime_source_binding_state` 的检索只有赋值，没有测试断言；当前没有看到任何 session 级测试验证 source state 的 `follow/reject/fallback`。
- `effect_runtime` 已实现 source binding 分类逻辑，见 `effect_runtime.rs:172-195`，且 source binding 仅对 effect id `8|9|178|261|262` 启用，见 `effect_runtime.rs:922-924`。
- 已有测试主要覆盖 source overlay position 跟随，不覆盖 session observability 管道本身，见 `effect_runtime.rs:1150-1455`。

建议补测：
- item transfer / regen suppress seek / chain lightning / chain emp 的 source state `ParentFollow`
- source parent 缺失时的 `UnresolvedFallback`
- source data 为 building/content/tech node 时的 `BindingRejected`
- render HUD 中 `target/source` 双槽位同时显示的端到端断言

### Medium
2. HUD binding label 优先使用 active overlay，可能掩盖“最新 packet”的 observability
- `runtime_effect_binding_label` 先读 `world_overlay.effect_overlays.last()`，只有 overlay 没有状态时才回退到 `session_state.last_effect_runtime_*_binding_state`，见 `render_runtime.rs:4121-4135`。
- 这意味着只要队列末尾仍残留旧 overlay，HUD 就可能显示旧 effect 的 binding 状态，而不是最近一条 packet 的状态。
- 现有测试只验证了 `reject/none` 和 `fallback/none` 两种单点场景，未验证 overlay-state 与 session-state 冲突时的优先级，见 `render_runtime.rs:9314-9359`。

建议补测：
- active overlay 为旧 effect，session_state 为新 effect 时，HUD 应显示哪一类状态
- target/source 其中一侧来自 overlay、另一侧来自 session_state 的混合场景

### Medium
3. building binding 的 overlay observability 永远报告 `ParentFollow`，语义可能过于乐观
- overlay 级状态判断里，`ParentBuilding` 不检查 building 当前是否仍存在，直接返回 `ParentFollow`，见 `effect_runtime.rs:399-406`。
- 实际位置解析也只把 `build_pos` 还原成 tile 世界坐标，不读取 building table，见 `effect_runtime.rs:665-682`。
- 这会把“仍跟随真实父 building”与“仅锚定原 tile 坐标”混成同一个 observability 状态。
- 现有测试只覆盖 building 绑定生成与 offset 冻结，未覆盖 building 消失/替换后的 observability 语义，见 `effect_runtime.rs:992-1080`。

建议补测：
- building 已不存在/已换块时 overlay binding state 的预期语义
- 若设计上本来只锚定 tile，建议 observability 文案不要继续使用 `follow`

### Medium
4. observability 状态只保留“最后一次”，缺少趋势计数，排查间歇性 fallback/reject 不够用
- `SessionState` 只保存 `last_effect_runtime_binding_state` 和 `last_effect_runtime_source_binding_state`，见 `session_state.rs:4968-4971`。
- 与 parse failure 已有累计计数 `failed_effect_data_parse_count` 不同，binding reject/fallback 没有累计或样本。
- 对高频 effect 来说，最后一条状态很容易被覆盖，难以判断问题是偶发还是持续。

建议补充状态覆盖：
- `effect_binding_follow_count`
- `effect_binding_reject_count`
- `effect_binding_fallback_count`
- source binding 同类计数
- 最近一次出现 reject/fallback 的 effect id / contract

### Low
5. `observe_runtime_effect_binding_state` / `observe_runtime_effect_source_binding_state` 的若干分支未见直接测试
- target binding 的 building enabled / disabled、content / tech node `None`、leg_destroy `BindingRejected` 分支位于 `effect_runtime.rs:142-169`。
- source binding 的 disabled / building|content|tech node reject 分支位于 `effect_runtime.rs:172-195`。
- `client_session` 目前只看到 target state 的少量断言：building reject、unit unresolved fallback，见 `client_session.rs:39545-39567`、`client_session.rs:39571-39593`。
- 已有 resolved unit parent 用例只断言 business projection，没有断言 runtime binding state，见 `client_session.rs:39643-39680`。

建议补测：
- resolved unit parent 应写出 `ParentFollow`
- building-enabled contract 对 building parent 应写出 `ParentFollow`
- content / tech node parent 应返回 `None`
- source binding disabled effect 不应污染 source state

## 建议优先级
1. 先补 source binding 的 session/render 端到端测试。
2. 再补 HUD overlay-vs-session 优先级测试。
3. 明确 building binding 的 observability 语义：是“跟随 building”还是“锚定 tile”。
4. 若该面板要用于线上排障，再加 binding 状态计数。
