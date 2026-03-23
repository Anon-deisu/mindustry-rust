# Rust Workspace Architecture

## 目标

这份文档定义 `rust/` 下各 crate 的职责边界，避免协议、世界解析、运行时编排、渲染和 CLI 继续混写。

当前最终目标仍然是：
- 交付可持续推进的 Rust 版 Mindustry
- 对外发布口径仍然是 `minimal compatibility client`
- 不能把“协议已覆盖 / 运行时可观测”误写成“Java 全量语义已完成”

## Crate 边界

### 基础层

- `mdt-protocol`
  - 负责 packet/framework 编解码、压缩、传输字节层。
  - 不负责 session、世界语义、UI、CLI。

- `mdt-remote`
  - 负责 remote manifest、registry、codegen 输入输出。
  - 不负责在线 session 行为。

- `mdt-typeio`
  - 负责 `TypeIO` 对象编解码，以及对象级语义归类。
  - 不负责会话状态机、runtime HUD、网络循环。

### 模型与解析层

- `mdt-world`
  - 负责 world/world-stream/snapshot 解析与世界模型。
  - 可以依赖 `mdt-protocol` 和 `mdt-typeio`。
  - 不承担 UI 呈现逻辑。

- `mdt-input`
  - 负责输入意图、动作边沿、建造计划编辑等纯输入逻辑。
  - 不直接依赖网络会话或渲染层。

### 表现与编排层

- `mdt-render-ui`
  - 负责投影、ASCII/窗口 presenter、HUD 视图输出。
  - 新增逻辑优先吃稳定 adapter/DTO 输入，不继续扩大对 `mdt-world` 内部实现的直接耦合。

- `mdt-client-min`
  - 集成入口 crate。
  - 负责网络循环、client session、runtime orchestration、CLI、adapter 拼装。
  - 可以依赖其它 Rust crates。
  - 新增“兼容客户端行为”默认先落在这里，再判断是否值得下沉到更窄的 crate。

## 依赖方向

建议长期保持：

- `mdt-protocol <- mdt-world <- mdt-render-ui <- mdt-client-min`
- `mdt-typeio <- mdt-world <- mdt-client-min`
- `mdt-remote <- mdt-client-min`
- `mdt-input <- mdt-client-min`

硬约束：

- `mdt-protocol`、`mdt-remote`、`mdt-typeio` 不能反向依赖 `mdt-client-min`
- `mdt-world` 不能依赖 `mdt-render-ui` 或 `mdt-client-min`
- `mdt-input` 保持纯输入逻辑，不反向依赖运行时 crate
- `mdt-render-ui` 后续新增能力时，优先引入稳定投影 DTO，不直接穿透 `mdt-world` 内部实现

## 当前热区

- `rust/mdt-world/src/lib.rs`
  - 体量过大，是后续拆分热点。
  - 新增解析能力时优先先按“family / packet / object kind”收口 helper，再谈大拆分。

- `rust/mdt-client-min/src/client_session.rs`
  - 同时承担 remote 绑定、解包、部分业务 apply、加载期策略和 runtime glue。
  - 这里是当前最高压集成热点，新增逻辑必须尽量先落到 helper 或 state 投影结构，再由 session 编排。

- `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
  - 目前承载过多 CLI/runtime 拼装职责。
  - 后续新逻辑优先放入库侧 adapter 或 queue API，bin 只负责参数解析和调用。

- `rust/mdt-render-ui`
  - 当前仍直接吃一部分 `mdt-world` 类型。
  - 再继续扩展前，应优先补 adapter 边界，而不是再加新直连。

## 新增代码落点规则

- 纯字节协议问题：放 `mdt-protocol`
- remote manifest / codegen / registry 问题：放 `mdt-remote`
- `TypeIO` 对象读写或对象级解析问题：放 `mdt-typeio`
- world-stream / snapshot / 世界模型问题：放 `mdt-world`
- 输入映射、动作边沿、计划编辑问题：放 `mdt-input`
- 视图投影、窗口输出、HUD 排版问题：放 `mdt-render-ui`
- 联机会话、加载期 gating、runtime observability、CLI action wiring：放 `mdt-client-min`

如果一个改动同时跨多层：

1. 先把窄职责逻辑下沉到对应 crate。
2. 再由 `mdt-client-min` 做 orchestration。
3. 不要把临时 glue 直接塞进基础层。

## `mdt-client-min` 内部约束

当前虽然还没完全拆模块，但后续新增代码应尽量遵守：

- packet decode / queue / transport glue：留在 `client_session.rs`
- 持久化的 session projection / runtime mirror：留在 `session_state.rs`
- snapshot envelope ingest 和共享 apply 入口：留在 `snapshot_ingest.rs`
- runtime HUD / overlay 文本和 presenter 适配：留在 `render_runtime.rs`
- CLI flag 解析、脚本 action wiring、stdout 输出：留在 `src/bin/mdt-client-min-online.rs`

下一轮如果继续触碰以下热点，优先考虑抽 helper，而不是继续横向堆大文件：

- configured block business apply
- snapshot business/runtime apply helpers
- runtime HUD label formatting
- loading gate / deferred packet policy

## Fixture 与工作区规则

- `fixtures/...` 是 canonical fixture 路径
- `rust/fixtures/...` 只是过渡镜像，不作为发布脚本主路径
- 当前 source tree 仍是 split workspace
  - `rust/Cargo.toml` 只覆盖 `mdt-protocol`、`mdt-remote`、`mdt-typeio`、`mdt-world`
  - `mdt-input`、`mdt-client-min`、`mdt-render-ui` 各自带独立 `[workspace]`
- 不能假设一次 `cargo test --workspace --manifest-path rust/Cargo.toml` 覆盖发布链
- 统一验证入口是 `tools/verify-rust-workspaces.ps1`

## 现在不要做的整理

- 不要在赶发布阶段对 `mdt-world/src/lib.rs` 做纯结构性大拆分
- 不要为了“好看”重排 remote/codegen/fixture 路径
- 不要把 `mdt-client-min-online` 的 CLI 行为直接搬进 `mdt-render-ui`
- 不要让 `mdt-render-ui` 反向拥有网络或 session 状态

## 当前执行约定

每次补丁尽量同步补齐四件事：

- `client_session` 显式绑定 / 事件入口
- `session_state` 计数或 projection
- runtime HUD 或 CLI summary
- 至少一个回归测试

## 相关文档

- 本文档负责 crate 边界和架构规则
- `tools/MINDUSTRY-RUST-HANDOFF.md` 负责同步 / 上传规则
- 发布口径与发布检查看 `tools/WINDOWS-RELEASE.md` 和 `RUST_RELEASE_AUDIT_FINDINGS.md`
