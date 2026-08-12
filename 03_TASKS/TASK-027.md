# TASK-027: cleaner 55 target 并行扫描提速

## Basic Info
- Status: Done
- Validated: 2026-08-08
- Priority: P1
- Owner: @self（agent team 编排）
- Created: 2026-08-08
- Estimated: 4h
- Depends: 无
- Complexity: B（并发改造，需保持事件顺序与取消语义）
- Spec: `04_SPECS/SPEC-027-ParallelScan.md`

## Goal
cleaner 扫描当前串行遍历 55 个 target，改为按 target 分组并行（线程池），保持事件流顺序稳定与取消语义不变。

## 背景
策略清单"扫描优化"P0 项；串行扫描在机械盘/大目录下可达分钟级。

## Acceptance
1. 扫描并发度：按 target 分组并行（默认 4 组，可配置常量），每个 target 内部串行
2. 事件顺序：Progress/ItemsFound/Done 与串行等价可观测（前端不感知顺序变化）；批次上限/总量上限不变量保持
3. 取消：任一扫描取消信号立即传播到全部并行组，无泄漏线程
4. 结果与串行扫描一致（同目录同结果集）；单测覆盖并行 vs 串行等价
5. `cargo test -p pony_core`、`clippy`、`fmt` 全过

## Non-Goal
- 不做增量/缓存扫描
- 不改 target 定义与安全分级体系

## Validation Evidence
- `cargo test -p pony_core`：82 + 6 全过（含 `test_scan_target_block_filters` / `test_scan_target_block_parallel_count`）✅
- `cargo clippy`：0 警告 ✅  `cargo fmt --check`：通过 ✅
- 实现：`scan_target_block` 提取 + SCAN_PARALLELISM=4 线程分组 + 全局 AtomicU64 计数 + join/panic 处理
- 手动 QA：真实磁盘扫描提速与进度观感（留待用户）

## Next Action
spec 审查通过后实现：cleaner.rs `start_scan` 内并行改造（std::thread + mpsc 汇总，注意 cancel 传播）。

## Resume Hint
读 `04_SPECS/SPEC-027-ParallelScan.md` → `crates/pony_core/src/cleaner.rs` 的 `start_scan` 并发改造 → 等价性测试 → 门禁。
