# SPEC-026: disk 大文件 + 目录占用合并为单遍历

- 状态: Draft（待对抗审查）
- 关联: TASK-026
- 日期: 2026-08-08

## 1. 背景与目标
`scan_large_files` 与 `scan_dir_usage` 均从 USERPROFILE 全递归 jwalk，同一目录树遍历两遍。目标：单趟遍历同时产出大文件批次与目录占用聚合，命令/事件/前端状态收敛为单扫描。

## 2. 范围与非目标
- 范围：`crates/pony_core/src/disk.rs`、`src-tauri/src/commands/disk.rs`、`frontend/src/composables/useDisk.ts`、`frontend/src/views/SpacePanel.vue`（进度联动）
- 非目标：不动 cleaner；不做增量扫描

## 3. 用户/系统行为
- "开始扫描"触发一个 disk 后端进程；大文件与目录区块共享同一扫描状态（同进度、同 done/error）
- 大文件阈值单选、风险分级、跳过逻辑（Temp/hive/node_modules）行为不变

## 4. 技术方案与替代
- **新函数** `scan_user_dir(tx, root, min_bytes, cancel, max_files, dir_depth)`：**单次全深遍历**（不设 max_depth，保持大文件覆盖不变），遍历中同时：
  - 收集 ≥min_bytes 文件 → LargeFile（含 level）→ 分批推 `LargeFiles` 事件
  - 聚合父目录 size/file_count → **仅对深度 ≤ dir_depth(3) 的父目录计入**（保持原"3 层"目录占用语义）→ 结束排序推 `DirUsage`
  - 复用现有 `send_progress` / `SKIP_DIRS` / `SKIP_SYSTEM_FILES` / `SKIP_TEMP_DIRS` / `risk_level`
- **事件收敛**：数据事件名**保留** `disk-large-files` / `disk-dir-usage`（前端分区块渲染不变）；仅进度/done/error 收敛为 `disk-user-progress` / `disk-user-done` / `disk-user-error`
- **命令层**：`start_large_scan` + `start_dir_scan` 合并为 `start_user_scan(min_bytes_mb, max_depth)`，单锁；`cancel_disk_scan` / `delete_large_files` 保留
- **前端**：useDisk 合并 large*/dir* 状态为单组（files + dirs + 单一 state/progress）
- **替代**：保留双命令但内部共享遍历（状态耦合复杂）→ 否决；直接合并命令更干净

## 5. 影响面与依赖
- 命令注册（main.rs）、事件名变化 → 前后端同步；Tauri 命令名 `start_large_scan`/`start_dir_scan` 移除（无外部调用方）
- SpacePanel 大文件/目录区块改为共享状态；与 TASK-024 同文件冲突 → 编排串行

## 6. 任务拆解与并行边界
- 后端（disk.rs + commands）与前端（useDisk + SpacePanel）可并行；同文件注意合并

## 7. 风险、回滚与迁移
- 行为等价风险：单遍历必须保留原有跳过/阈值/分批逻辑 → 用等价性测试兜底
- 回滚：恢复双函数双命令（git revert 级）

## 8. 测试计划
- disk 单测：合并函数产出 LargeFiles + DirUsage 与旧双函数结果一致（同 fixture 对比）
- 前端：vue-tsc / build；手动 QA 进度联动

## 9. 验收标准
见 TASK-026 Acceptance。

## 10. 审核记录
（审查后填写）
