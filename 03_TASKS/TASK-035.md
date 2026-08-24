# TASK-035: cleaner Done.total_items 触顶多计 1 修正

## Basic Info
- Status: Backlog
- Priority: P3
- Owner: @self
- Created: 2026-08-24
- Estimated: 0.5h
- Depends: 无（TASK-033 验证过程中的读码发现）
- Complexity: S

## 背景
`crates/pony_core/src/cleaner.rs` `scan_target_block`：`global_count.fetch_add(1)` 先自增、
再判断 `n > MAX_SCAN_ITEMS` 提前返回——该最后一个未产出的文件也计入了计数。
`start_scan` 收尾 `Done { total_items: global_count }` 因此比实际推送的 CleanItem 数**多 1**
（仅在触发 MAX_SCAN_ITEMS 全局上限时发生）。

当前 UI 不展示 `Done.total_items`（items 列表自洽派生计数），无用户可见影响；
但事件契约层数据不一致，未来任何消费方都会拿到偏大值。

## Acceptance
1. 触顶场景单测：total_items == 实际 ItemsFound 推送条数（不多不少）。
2. `cargo test -p pony_core` 全绿，无行为回归。

## Review Records
- 来源：TASK-033 双审期间读码发现，TASK-033.md Non-Goal 声明遗留后升级为独立任务。
