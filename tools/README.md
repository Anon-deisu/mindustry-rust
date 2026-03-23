# 工具入口

这里集中放 `mindustry-rust` 最常用的脚本和说明文档。

如果你的目标是运行当前版本、做一次本地校验、打一个可交付包，或者把最新 Rust 成果同步到目标仓库，先从这里开始看。

## 最常用文档

- [`WINDOWS-RELEASE.md`](WINDOWS-RELEASE.md)：Windows 环境下最常用的打包、校验、发布说明
- [`MINDUSTRY-RUST-HANDOFF.md`](MINDUSTRY-RUST-HANDOFF.md)：当前源码仓到目标仓的同步/交接说明

## 最常用脚本

- [`check-mdt-release-prereqs.ps1`](check-mdt-release-prereqs.ps1)：发布前的非破坏性环境检查
- `.\gradlew -PnoLocalArc verifyMdtRemoteFreshness`：校验 Java remote 生成物、fixture 镜像和 Rust 高频生成文件没有漂移
- [`verify-rust-workspaces.ps1`](verify-rust-workspaces.ps1)：检查多个 Rust workspace 是否都能通过基础验证
- [`package-mdt-client-min-release-set.ps1`](package-mdt-client-min-release-set.ps1)：生成发布用产物；加 `-Verify` 可直接带上默认校验流程
- [`verify-mdt-client-min-release-set.ps1`](verify-mdt-client-min-release-set.ps1)：校验已生成的发布目录、清单和基础可运行性
- [`package-mdt-client-min-online.ps1`](package-mdt-client-min-online.ps1)：单独打一个 `core` 或 `devtools` 包
- [`clean-legacy-mdt-package-dirs.ps1`](clean-legacy-mdt-package-dirs.ps1)：清理旧版 staging 目录，避免混入历史产物

## 同步到目标仓库

- [`mindustry-rust-target.json`](mindustry-rust-target.json)：固定同步目标记录
- [`get-mindustry-rust-target.ps1`](get-mindustry-rust-target.ps1)：输出当前生效的目标仓路径和同步策略
- [`sync-mindustry-rust-handoff.ps1`](sync-mindustry-rust-handoff.ps1)：把当前 handoff 范围内的文件同步到 `mindustry-rust` 仓库
- [`mindustry-rust-repo-README.md`](mindustry-rust-repo-README.md)：同步到目标仓根目录的 README 模板

## 适用边界

这些脚本和文档服务的是 Rust 最小兼容客户端的交付、校验和追踪流程，不代表项目已经完成与原版 Java 桌面客户端的全部功能等价。

## 仓库维护补充

如果你在做发布治理、CI 审查或同步规则维护，还可以继续看：

- [`../audit/ci-gate-plan.md`](../audit/ci-gate-plan.md)
- `verifyMdtRemoteFreshness` 支持通过 `-PremoteFreshnessOnCheck=true` 挂到 Gradle `check`，适合在 CI 或正式发布前开启
