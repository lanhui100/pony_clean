# SPEC-027: cleaner 55 target 并行扫描提速

- 状态: Draft（待对抗审查）
- 关联: TASK-027
- 日期: 2026-08-08

## 1. 背景与目标
cleaner 扫描对 55 个 target 串行遍历（jwalk 单目录内部已并行读，但 target 之间串行），机械盘/大目录下可达分钟级。目标：target 级并行，保持结果等价与事件语义。

## 2. 范围与非目标
- 范围：`crates/pony_core/src/cleaner.rs` 的 `start_scan`
- 非目标：不做增量/缓存；不改 target 定义、安全分级、保护路径体系

## 3. 用户/系统行为
- 扫描总时长下降；进度事件仍持续推送（前端无感知变化）；取消立即生效

## 4. 技术方案与替代
- **实现**：`start_scan` 内将 resolved targets 按固定并发度（`SCAN_PARALLELISM = 4`）分组，`std::thread::scope` + 每线程一个 mpsc 子通道汇总到主通道：
  - 每个 target 内部逻辑（glob/mtime/上限/批次）**原样保留**，仅外层并行；批次推送频率不变量保持
  - 事件顺序：不要求跨 target 严格有序（前端按批次追加，无顺序依赖，已确认 useCleaner.ts:99 concat）；`Done` 在所有组完成后发送
  - 取消：共享 `CancellationToken`（现有），各线程循环检查；**汇总线程 join 全部子线程后才发 Done**；join 检查 panic（`JoinHandle::join` 返回 Result）→ 记录 warning 并继续，不拖垮整体
  - 全局上限 `MAX_SCAN_ITEMS`、单 target 上限 `MAX_ITEMS_PER_TARGET` 不变量：**汇总线程统一计数**
- **替代**：rayon 并行迭代 → 引入新依赖且取消语义需重写，否决；手写线程池更可控

## 5. 影响面与依赖
- 仅 cleaner.rs `start_scan`；`SCAN_IN_PROGRESS` 守卫、warning 事件、batch 汇总逻辑随动
- 与 TASK-025 同文件（cleaner.rs）→ 编排串行或合并提交

## 6. 任务拆解与并行边界
- 与 TASK-024/026 前端无冲突；与 TASK-025 同文件需协调

## 7. 风险、回滚与迁移
- 风险：并发下批次顺序变化 → 前端已按批次 concat，无顺序假设（需确认 `scan-items` 处理）；取消竞态 → join 保证
- 回滚：恢复串行循环（小 diff）

## 8. 测试计划
- 等价性单测：同 fixture 并行 vs 串行结果集一致（集合相等）
- 取消测试：扫描中触发 cancel → 无线程泄漏（join 后计数）
- 全量门禁

## 9. 验收标准
见 TASK-027 Acceptance。

## 10. 审核记录
（审查后填写）
