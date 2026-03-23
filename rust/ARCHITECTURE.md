# Rust Workspace Architecture

## 目标

这份文档定义 `rust/` 下各 crate 的唯一边界契约，避免后续把协议、世界解析、运行时编排和 UI 继续混写。

当前目标仍是：

- 交付可持续推进的 Rust 版 Mindustry
- 当前对外发布口径仍然是 `minimal compatibility client`
- 不能把“已有协议/观测覆盖”误写成“Java 全量语义已完成”

## 当前 crate 划分

### 基础层

- `mdt-protocol`
  - 负责 packet/framework 编解码、压缩、传输字节层
  - 不负责 session、世界语义、UI、CLI
- `mdt-remote`
  - 负责 remote manifest / codegen 输入输出
  - 不负责在线 session 行为
- `mdt-typeio`
  - 负责 TypeIO 对象编解码与对象级语义归类
  - 不负责会话状态机、运行时 HUD、网络循环

### 模型与解析层

- `mdt-world`
  - 负责 world/world-stream/snapshot 解析与世界模型
  - 可以依赖 `mdt-protocol`、`mdt-typeio`
  - 不应承担 UI 展示逻辑
- `mdt-input`
  - 负责输入意图、动作边沿、计划旋转/翻转等纯输入逻辑
  - 不应直接依赖网络会话或渲染层

### 表现与编排层

- `mdt-render-ui`
  - 负责投影、窗口呈现、最小 HUD / 视图输出
  - 当前可读取世界模型，但新增逻辑优先经稳定投影/adapter 输入，避免继续放大对 `mdt-world` 内部类型的直接耦合
- `mdt-client-min`
  - 集成入口 crate
  - 负责网络循环、client session、runtime orchestration、CLI、adapter 拼装
  - 可以依赖其他 Rust crates
  - 新增“兼容客户端行为”默认先落在这里，再决定是否下沉到更窄的 crate

## 允许的依赖方向

建议把依赖方向收敛为：

- `mdt-protocol` <- `mdt-world` <- `mdt-render-ui` <- `mdt-client-min`
- `mdt-typeio` <- `mdt-world` <- `mdt-client-min`
- `mdt-remote` <- `mdt-client-min`
- `mdt-input` <- `mdt-client-min`

约束：

- `mdt-protocol`、`mdt-remote`、`mdt-typeio` 不能反向依赖 `mdt-client-min`
- `mdt-world` 不能依赖 `mdt-render-ui` 或 `mdt-client-min`
- `mdt-input` 保持纯逻辑，不反向依赖运行时 crate
- `mdt-render-ui` 新增能力时，优先吃稳定投影/DTO，不继续直接穿透 `mdt-world` 内部实现

## 当前已知混乱点

- `rust/mdt-world/src/lib.rs` 体量过大，是后续拆分热点
- `rust/mdt-client-min/src/client_session.rs` 同时承担 remote 绑定、解包、部分业务投影、加载期策略，是当前集成热点
- `rust/mdt-client-min/src/bin/mdt-client-min-online.rs` 承担过多 CLI/runtime 拼装职责；后续新增逻辑优先放入库侧 adapter，再由 bin 调用
- `mdt-render-ui` 当前直接依赖部分 `mdt-world` 类型；继续扩张前先补 adapter 边界

## 新增代码落点规则

- 纯字节协议问题：放 `mdt-protocol`
- remote manifest / codegen / registry 问题：放 `mdt-remote`
- TypeIO 对象读写或对象级解析问题：放 `mdt-typeio`
- world-stream、snapshot、世界模型问题：放 `mdt-world`
- 输入映射、动作边沿、计划编辑问题：放 `mdt-input`
- 展示投影、窗口输出、HUD 排版问题：放 `mdt-render-ui`
- 联机 session、加载期 gating、runtime 观测、CLI action wiring：放 `mdt-client-min`

如果一个改动同时碰到多个层：

- 先把窄职责逻辑下沉到对应 crate
- 再在 `mdt-client-min` 做 orchestration
- 不要把临时 glue 直接塞进基础层

## Fixture 与工作区规则

- `fixtures/...` 是 canonical fixture 路径
- `rust/fixtures/...` 只是过渡镜像，不作为发布脚本主路径
- 当前 source tree 仍是 split workspace：
  - `rust/Cargo.toml` 只覆盖 `mdt-protocol`、`mdt-remote`、`mdt-typeio`、`mdt-world`
  - `mdt-input`、`mdt-client-min`、`mdt-render-ui` 各自带独立 `[workspace]`
- 不能假设一次 `cargo test --workspace --manifest-path rust/Cargo.toml` 覆盖发布链
- 统一验证入口是 `tools/verify-rust-workspaces.ps1`

## 当前执行约定

- 新增 remote 覆盖时，优先做“小而闭合”的 packet slice
- 若 Java 语义体积过大，先落 header-only observability 或最小业务投影
- 每次补丁尽量同时补：
  - `client_session` 显式绑定/事件
  - `session_state` 计数/last 字段
  - runtime HUD 或 CLI summary
  - 至少一个回归测试

## 文档关系

- 这份文档负责 crate 边界与架构规则
- `tools/MINDUSTRY-RUST-HANDOFF.md` 负责同步/上传规则
- 发布口径和发布检查仍看 `tools/WINDOWS-RELEASE.md`、`RUST_RELEASE_AUDIT_FINDINGS.md`
