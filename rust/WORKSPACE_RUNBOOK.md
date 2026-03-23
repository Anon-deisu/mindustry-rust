# Rust Workspace Runbook

## 目标

这个仓库当前仍是 split workspace。

不要假设一次 `cargo test --workspace --manifest-path rust/Cargo.toml` 能覆盖全部 Rust 交付面。

统一原则：

- 日常全量验证入口：`tools/verify-rust-workspaces.ps1`
- 发布前验证入口：`tools/package-mdt-client-min-release-set.ps1 -Verify`
- 只在明确知道自己要测哪个 crate 时，才直接跑单 crate `cargo test`

## 当前工作区结构

- 根 workspace：`rust/Cargo.toml`
  - 包含：`mdt-protocol`、`mdt-remote`、`mdt-typeio`、`mdt-world`
- 独立 workspace：
  - `rust/mdt-input`
  - `rust/mdt-client-min`
  - `rust/mdt-render-ui`

## 标准命令

### 1. 日常全量 Rust 验证

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1
```

### 2. 根 workspace 验证

适用范围：

- `mdt-protocol`
- `mdt-remote`
- `mdt-typeio`
- `mdt-world`

命令：

```powershell
cargo test --workspace --manifest-path .\rust\Cargo.toml
```

### 3. `mdt-client-min` 验证

```powershell
cargo test --manifest-path .\rust\mdt-client-min\Cargo.toml
```

### 4. `mdt-render-ui` 验证

```powershell
cargo test --manifest-path .\rust\mdt-render-ui\Cargo.toml
```

### 5. `mdt-input` 验证

```powershell
cargo test --manifest-path .\rust\mdt-input\Cargo.toml
```

### 6. 发布链验证

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\package-mdt-client-min-release-set.ps1 -Verify
```

## 什么时候跑什么

- 改 `mdt-protocol` / `mdt-remote` / `mdt-typeio` / `mdt-world`
  - 至少跑根 workspace 测试
- 改 `mdt-client-min`
  - 至少跑 `rust/mdt-client-min` 测试
- 改 `mdt-render-ui`
  - 至少跑 `rust/mdt-render-ui` 测试
- 改 `mdt-input`
  - 至少跑 `rust/mdt-input` 测试
- 改发布脚本、fixture 路径、workspace 入口
  - 直接跑 `tools/verify-rust-workspaces.ps1`
  - 如影响交付链，再跑 release-set verify

## 常见误区

- 误区：根 workspace 绿了，说明 `mdt-client-min` 也绿了
  - 错。`mdt-client-min` 是独立 workspace。

- 误区：只跑 `mdt-client-min` 就等于验证了全部 Rust crates
  - 错。`mdt-world` / `mdt-protocol` 等也有独立测试面。

- 误区：发布前只需要 `cargo test`
  - 错。发布链还依赖 PowerShell 脚本、fixture 路径和打包验证。

## 相关文档

- `rust/ARCHITECTURE.md`
- `rust/FIXTURE_PATHS.md`
- `tools/MINDUSTRY-RUST-HANDOFF.md`
- `tools/WINDOWS-RELEASE.md`
