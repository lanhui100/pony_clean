# REVIEW-027: SPEC-027 对抗审查记录

- 审查对象: `04_SPECS/SPEC-027-ParallelScan.md`（TASK-027）
- 审查方式: **降级独立审查 pass（环境无真实子智能体工具，双视角隔离审查兜底）**
- 日期: 2026-08-08

## Pass 1（并发/性能视角）
1. **[P1] 全局上限原子性**：`MAX_SCAN_ITEMS`（300k）全局上限须在汇总线程统一判断，避免并行组各自计数超限。SPEC 已提，保持。
2. **[P2] 内存峰值**：4 线程 × batch(500) 峰值内存上升可接受；但大 target（如 user_temp）批次推送频率应保持，避免单组积压。
3. **[P1] 顺序变化对前端**：`scan-items` 前端 concat 追加、`scan-done` 收尾，无顺序依赖 → 已确认（useCleaner.ts:99）。保留。

## Pass 2（代码/边界视角）
1. **[P1] cancel 竞态**：取消后各线程需在**自己的**循环检查点退出；汇总线程 join 全部子线程后才发 Done，避免提前 Done。SPEC 已提 join，保持。
2. **[P2] 子线程 panic**：某 target 遍历 panic 不应拖垮整个扫描 → scoped thread join 返回 Result，panic 时记录 warning 并继续。修订补 panic 处理。
3. **[P2] 与 TASK-025 文件冲突**：都改 cleaner.rs，编排串行。SPEC 已提。

## 结论
**有条件通过**。需修订：① 子线程 panic 处理（join 检查 + warning）；② 批次推送频率不变量说明。

## 采纳记录
| 意见 | 采纳 | 处理 |
|---|---|---|
| P1 全局上限原子性 | 采纳 | SPEC 保持（实现时汇总线程统一计数） |
| P2 panic 处理 | 采纳 | SPEC 修订：join 检查 panic → warning + 继续 |
| P2 批次频率 | 采纳 | SPEC 修订：保留每 target 批次推送逻辑 |
