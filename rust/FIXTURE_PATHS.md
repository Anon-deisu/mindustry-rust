# Fixture Paths

## 目标

当前仓库同时存在发布 fixture、测试 fixture 和过渡镜像 fixture。

这份文档只回答一件事：不同场景下，哪条路径才是“应该读的那条”。

## 三类路径

### 1. Canonical release fixtures

这些路径是发布和 handoff 的主路径：

- `fixtures/remote/remote-manifest-v1.json`
- `fixtures/world-streams/archipelago-6567-world-stream.hex`

适用场景：

- 发布脚本
- handoff 同步
- 面向用户/交付文档的默认路径

### 2. Rust parity test fixtures

这些路径主要给测试、golden、部分工具入口使用：

- `tests/src/test/resources/connect-packet.hex`
- `tests/src/test/resources/control-packet-goldens.txt`
- `tests/src/test/resources/framework-message-goldens.txt`
- `tests/src/test/resources/payload-campaign-compound-goldens.txt`
- `tests/src/test/resources/snapshot-goldens.txt`
- `tests/src/test/resources/typeio-goldens.txt`
- `tests/src/test/resources/unit-payload-goldens.txt`
- `tests/src/test/resources/world-stream.hex`

适用场景：

- Rust 单元测试
- parity/golden 生成与回归
- 仍保留硬编码 `tests/src/test/resources/...` 的工具入口

### 3. Transitional mirror fixtures

这些路径是过渡镜像，不是 canonical：

- `rust/fixtures/remote/remote-manifest-v1.json`
- `rust/fixtures/world-streams/archipelago-6567-world-stream.hex`

适用场景：

- 兼容旧路径
- source 仓内镜像保留

不适用场景：

- 新的发布脚本默认路径
- 对外文档默认路径

## 当前规则

- 发布默认读 `fixtures/...`
- handoff 默认同步 `fixtures/...`
- `rust/fixtures/...` 只保留为镜像，不作为主路径
- `tests/src/test/resources/...` 继续保留给测试与尚未抽象掉的工具入口

## 改路径前先问

- 这是发布路径，还是测试路径？
- 这是 canonical，还是过渡镜像？
- 这次改动会不会影响 `tools/package-mdt-client-min-online.ps1`
- 这次改动会不会影响 `tools/package-mdt-client-min-release-set.ps1`
- 这次改动会不会影响 `tools/sync-mindustry-rust-handoff.ps1`
- 这次改动会不会打断 `tests/src/test/resources/...` 的现有硬编码引用？

## 相关文档

- `rust/ARCHITECTURE.md`
- `rust/WORKSPACE_RUNBOOK.md`
- `tools/MINDUSTRY-RUST-HANDOFF.md`
