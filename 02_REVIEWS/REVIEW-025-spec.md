# REVIEW-025: SPEC-025 对抗审查记录

- 审查对象: `04_SPECS/SPEC-025-WindowsUpdate.md`（TASK-025）
- 审查方式: **降级独立审查 pass（环境无真实子智能体工具，双视角隔离审查 + 安全专项视角）**
- 日期: 2026-08-08

## Pass 1（安全专项视角）
1. **[P0] windows crate 缺 Service feature**：`crates/pony_core/Cargo.toml` 当前 features 无 `Win32_System_Services`，SCM API 不可用。修订：加 feature（或用 `net stop/start wuauserv` fallback）。倾向 SCM API（`net` 解析脆弱）。
2. **[P1] 权限不足路径**：停止 wuauserv 需管理员；PonyClean 默认非提权。权限不足时必须**不删、明确报错**（"需要管理员权限"），不得静默失败。`SoftwareDistribution` 目录普通用户可能只读 → wu_download 删除失败按现有 failed 路径处理即可，但 DataStore 服务操作必须前置权限检测。
3. **[P1] 服务状态恢复**：停止后若删除失败/进程崩溃，服务可能停着。实现需用 guard（Drop 恢复）保证服务恢复，SPEC 已提"尽力恢复"，需升为**硬要求**（Drop guard）。

## Pass 2（代码/边界视角）
1. **[P1] 计数断言同步**：55→57 需更新 `test_new_targets_resolve`（cleaner.rs）与 `tests/integration_cleaner.rs::test_get_clean_targets_count`，SPEC 已提，保持。
2. **[P2] DataStore 删除粒度**：明确只删文件（*.db / *.jrs / *.blb / *.log），保留目录与子目录结构。修订补全。
3. **[P2] 与 TASK-027 文件冲突**：两者都改 cleaner.rs，编排串行，SPEC 已提。

## 结论
**有条件通过**。需修订：① Cargo 加 Service feature 或明确 net fallback；② 权限不足前置检测；③ 服务恢复升级为 Drop guard 硬要求；④ DataStore 文件粒度。

## 采纳记录
| 意见 | 采纳 | 处理 |
|---|---|---|
| P0 Service feature 缺失 | 采纳 | SPEC 修订：加 `Win32_System_Services` feature |
| P1 权限前置检测 | 采纳 | SPEC 修订：权限不足不删并明确报错 |
| P1 服务恢复 Drop guard | 采纳 | SPEC 修订：升级为硬要求 |
| P2 DataStore 粒度 | 采纳 | SPEC 修订：限定文件类型 |
