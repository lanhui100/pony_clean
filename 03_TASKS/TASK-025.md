# TASK-025: Windows Update 缓存 + DataStore 清理

## Basic Info
- Status: Done
- Validated: 2026-08-08
- Priority: P0
- Owner: @self（agent team 编排）
- Created: 2026-08-08
- Estimated: 5h
- Depends: 无
- Complexity: B（服务控制 + 系统目录，安全敏感）
- Spec: `04_SPECS/SPEC-025-WindowsUpdate.md`

## Goal
新增 2 个清理目标：
1. `SoftwareDistribution\Download` — Windows Update 下载缓存（Safe 级，直接删文件）
2. `SoftwareDistribution\DataStore` — 更新数据库（Confirm 级，需停 wuauserv 服务 → 删除 → 重启服务）

## 背景
策略清单 P0 覆盖面项；Windows Update 缓存常积攒数 GB，属最高收益目标之一。

## Acceptance
1. `wu_download`（SoftwareDistribution\Download）：Confirm → **Safe**（下载缓存删除无风险，重下即可）
2. `wu_datastore`（SoftwareDistribution\DataStore）：Forbidden → **Confirm** + `with_service_stop("wuauserv")` + glob 限定文件类型（*.db/*.edb/*.jrs/*.blb/*.log），仅清文件保留目录
3. 删除流程集成服务控制：需停服务的 target 路径删除前先停服务，删除后恢复（后进先出）
4. 服务停止失败：属于该服务的路径**跳过删除**并报错；恢复失败记录错误不阻断
5. 权限不足（非管理员）：服务控制报"需要管理员权限"；下载缓存删除走现有 failed 路径
6. 单测：`test_wu_targets_config`（级别/服务/glob）新增；全量测试通过
7. `cargo test -p pony_core`、`clippy`、`fmt` 全过

## Non-Goal
- 不做 Windows.old / DISM 组件清理（需提权，独立后续任务）
- 服务控制不做通用框架，仅 DataStore 专用

## Validation Evidence
- `cargo test -p pony_core`：80 + 6 全过 ✅
- `cargo clippy`：0 警告 ✅
- `cargo fmt --check`：通过 ✅
- Windows 集成（真实服务停止/恢复）：留手动 QA

## Next Action
spec 审查（含安全审查 pass）通过后实现：cleaner.rs 加 target + 服务控制模块。

## Resume Hint
读 `04_SPECS/SPEC-025-WindowsUpdate.md` → `crates/pony_core/src/cleaner.rs`（ScanTarget + DataStore 服务控制逻辑）→ 测试 → 门禁。
