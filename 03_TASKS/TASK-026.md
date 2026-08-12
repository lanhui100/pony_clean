# TASK-026: disk 大文件 + 目录占用合并为单遍历

## Basic Info
- Status: Done
- Validated: 2026-08-08
- Priority: P1
- Owner: @self（agent team 编排）
- Created: 2026-08-08
- Estimated: 5h
- Depends: 无
- Complexity: B（后端重构，接口/事件变化）
- Spec: `04_SPECS/SPEC-026-DiskScanMerge.md`

## Goal
消除用户目录被遍历两遍的浪费：`scan_user_dir` 一趟 walk 同时产出大文件（≥阈值，含风险分级）与目录占用聚合，两个独立事件流合并为一条。

## 背景
`scan_large_files` 与 `scan_dir_usage` 均从 USERPROFILE 全递归，同一棵目录树走两遍。策略清单"扫描优化"核心项；上一轮分析已确认大文件+目录合并才是有收益的合并（垃圾扫描保持独立）。

## Acceptance
1. 新增 `scan_user_dir`：单次 jwalk 遍历，同时产出 LargeFiles 批次 + 目录聚合（结束一次性发 DirUsage）
2. 命令层合并为 `start_user_scan`（单锁单事件通道 `disk-user-*`）；删除大文件命令保留
3. `useDisk.ts` 状态合并为单组（large/dir 进度联动为同一扫描）
4. SpacePanel 大文件/目录区块共享同一扫描状态与进度
5. 行为等价：阈值/风险分级/跳过逻辑（Temp、hive、node_modules）全部保留；测试覆盖等价性
6. `cargo test -p pony_core`、`clippy`、`fmt`、`vue-tsc`、`build` 全过

## Non-Goal
- 不动 cleaner 扫描（垃圾扫描独立）
- 不做增量/缓存扫描（P2）

## Validation Evidence
- `cargo test -p pony_core`：82 + 6 全过（含 `test_scan_user_dir_matches_old_functions` 等价性测试）✅
- `cargo clippy`：0 警告 ✅  `cargo fmt --check`：通过 ✅
- `cargo build -p pony_clean`、`vue-tsc`、`npm run build` ✅
- 实现：`scan_user_dir` 单遍历双产出；命令层合并为 `start_user_scan`（disk-user-* 事件）；useDisk 单状态；SpacePanel 共享进度
- 手动 QA：大文件/目录区块进度联动（留待用户）

## Next Action
spec 审查通过后实现：disk.rs 合并函数 → commands/disk.rs → useDisk.ts → SpacePanel 进度联动。

## Resume Hint
读 `04_SPECS/SPEC-026-DiskScanMerge.md` → 后端 `crates/pony_core/src/disk.rs` 合并 → `src-tauri/src/commands/disk.rs` → 前端 `useDisk.ts` / `SpacePanel.vue` → 全量门禁。
