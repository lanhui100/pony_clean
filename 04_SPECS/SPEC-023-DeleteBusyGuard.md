# SPEC-023: 删除前进程占用检查

- 状态: Draft（待对抗审查）
- 关联: TASK-023
- 日期: 2026-08-08

## 1. 背景与目标
删除失败的两个主因是"权限"与"被占用"。当前 `delete_files_with_progress` / `delete_large_files` 对占用文件报 generic 错误（`{e}`），用户无法区分原因，也无法预期重启后是否可删。目标：删除前检测占用，占用文件标记明确原因并走延迟删除通道。

## 2. 范围与非目标
- 范围：`crates/pony_core/src/cleaner.rs` 的 `delete_files_with_progress`、`crates/pony_core/src/disk.rs` 的 `delete_large_files`
- 非目标：不枚举占用进程名（P2 候选）；不改确认流程；不动 `empty_recycle_bin`

## 3. 用户/系统行为
- 用户删除含被占用文件的集合 → 未被占用文件正常删除；被占用文件计入 failed，errors 文案为"文件被进程占用，已尝试重启后删除/删除失败"
- 被占用文件优先尝试 `MOVEFILE_DELAY_UNTIL_REBOOT`（延迟删除），失败才计 failed

## 4. 技术方案与替代
- **占用检测**：`CreateFileW(path, DELETE|GENERIC_READ, share=0, OPEN_EXISTING)` —— 打开成功 = 未被独占占用（**立即关闭句柄**后继续删除）；打开失败且错误为共享冲突/占用 = 被占用。
  - 区分 ERROR_SHARING_VIOLATION (32) / ERROR_LOCK_VIOLATION (33) → 占用；ERROR_ACCESS_DENIED (5) → 权限（不误报占用）。
- **cleaner 路径**：占用文件 → 尝试 `MOVEFILE_DELAY_UNTIL_REBOOT` 延迟删除，失败计 failed。
- **disk 路径**（`delete_large_files`）：占用文件 → 报"文件被进程占用"错误 + 跳过（不做延迟删除，保持简单）。
- **已知残余风险**：检测与删除存在 TOCTOU 时间窗，检测后仍可能被占用 → 删除失败仍计入 failed（可接受）。
- 非 Windows 平台：跳过检测，维持现状（`remove_file`）。
- **替代方案**：NtQuerySystemInformation 枚举句柄 → 成本高、版本不稳定，否决。

## 5. 影响面与依赖
- 无新增依赖（复用 `windows` crate 现有 import）。
- 与 TASK-026 删除入口收敛无冲突（两者改不同文件或可顺序合并）。

## 6. 任务拆解与并行边界
- TASK-023 独立实现，可与 TASK-024/026/027 并行；安全审查随代码审查进行。

## 7. 风险、回滚与迁移
- 风险：误判占用（权限错误被当成占用）→ 仅影响删除文案与通道，不产生数据损失；延迟删除失败仍计 failed。
- 回滚：仅删新增检测函数，恢复原删除路径。

## 8. 测试计划
- 单测：正常文件删除成功；以独占句柄打开模拟占用 → 断言错误文案含"占用"且尝试延迟删除（Windows 集成测试需真实句柄，用 `#[cfg(windows)]` + 打开自身测试文件句柄）。

## 9. 验收标准
见 TASK-023 Acceptance（占用检测、延迟删除通道、单测、门禁全过）。

## 10. 审核记录
（审查后填写：审查者 pass、意见、采纳情况）
