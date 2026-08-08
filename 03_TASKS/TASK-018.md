# TASK-018: Phase 3 — 大文件扫描 + 磁盘分析

## Basic Info
- Status: Done
- Priority: P1
- Owner: @self
- Created: 2026-08-08
- Estimated: 6h
- Depends: TASK-017

## Goal
Phase 3 差异化能力：定位用户数据中的空间黑洞（大文件 + 目录占用），提供安全删除通道。

## Output
- `pony_core::disk` 模块：`scan_large_files`（类型推断/分批推送/取消）+ `scan_dir_usage`（目录聚合）
- `delete_large_files`：扫描根前缀验证 + 受保护路径检查 + 审计日志
- 命令层：`start_large_scan` / `start_dir_scan` / `cancel_disk_scan` / `delete_large_files`
- 前端「分析」tab：大文件列表（≥100/500/1000MB 可选、勾选删除、二次确认）+ 目录占用 Top 榜

## Acceptance
1. 大文件扫描支持阈值选择、进度推送、取消
2. 目录占用按目录聚合展示 Top N
3. 删除仅限扫描根内 + 非受保护路径，记录审计日志
4. fmt / clippy / test / vue-tsc / build 全通过

## Validation
- `cargo fmt --check` — Pass
- `cargo clippy -p pony_core -p pony_clean` — Pass（0 警告）
- `cargo test -p pony_core` — Pass（75 单元 + 6 集成，含 6 项 disk 新增）
- `npx vue-tsc --noEmit` — Pass
- `npm run build` — Pass
- 手动 QA — 待执行（扫描速度、列表渲染、删除流程）

## Next Action
随其余任务一并手动 QA。