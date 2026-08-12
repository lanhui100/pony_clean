# TASK-023: 删除前进程占用检查 + 占用处理

## Basic Info
- Status: Done
- Validated: 2026-08-08
- Priority: P0
- Owner: @self（agent team 编排）
- Created: 2026-08-08
- Estimated: 4h
- Depends: 无（与 TASK-026 删除入口收敛后可联动）
- Complexity: B（安全敏感，跨 cleaner/disk 删除路径）
- Spec: `04_SPECS/SPEC-023-DeleteBusyGuard.md`

## Goal
清理/删除文件前检测目标是否被进程占用：被占用的文件不再盲目报"删除失败"，而是标记占用 → 走延迟删除（重启后删除）或跳过，并在结果中提示。

## 背景
现状 `delete_files` / `delete_large_files` 直接 `remove_file`，占用文件报 generic 错误。用户此前要求"删除前进程占用检查，防删运行中文件"。

## Acceptance
1. 删除前尝试独占打开文件（CreateFileW，无共享标志）；失败即判定占用
2. 被占用文件：结果 errors 含明确"文件被进程占用"提示；不重复尝试普通删除
3. 占用文件走 `MOVEFILE_DELAY_UNTIL_REBOOT` 延迟删除（如失败则计入 failed）
4. 单测覆盖：占用文件场景（独占打开模拟）、正常文件不受影响
5. `cargo test -p pony_core` 全过；`clippy` 0 警告；`fmt --check` 通过

## Non-Goal
- 不枚举占用进程名（句柄级枚举成本高，降级为 P2 候选）
- 不改变删除确认流程

## Validation Evidence
（实现后填写：测试命令与结果）

## Next Action
等待 spec 审查通过后进入实现（先实现后端 `delete_file_delayed_windows` 前的占用检测）。

## Resume Hint
读 `04_SPECS/SPEC-023-DeleteBusyGuard.md` → 改 `crates/pony_core/src/cleaner.rs` 的 `delete_files_with_progress` 与 `crates/pony_core/src/disk.rs` 的 `delete_large_files` → 补单测 → 跑门禁。
