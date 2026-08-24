# Session Log 006 — 清理 Tab 扫描数目准确性验证与修复（TASK-033）

- 日期: 2026-08-24
- 项目: PonyClean
- 目标: 验证清理 Tab「空间分析」扫描数据与真实磁盘一致，确保前端显示文件数目准确

## 本次完成

### 1. 验证发现的五个根因（全部复现后修复）
| # | 根因 | 真机/夹具证据 |
|---|------|--------------|
| B0 | jwalk 默认 `skip_hidden=true`（名字以 `.` 开头即跳过），`.rustup`/`.cargo`/`.bun` 等整棵漏扫 | 真机 ≥100MB 大文件 78 vs 独立真值 97，缺失样本全部位于点开头目录 |
| B1 | 分批 split_off 后 `max_files` 截断判断失效 | 60 候选 + cap=10 返回的是最小的 10 个 |
| B2 | 结束仅对尾批排序，全局顺序 = 遭遇序 | 夹具事件流乱序 |
| B3 | 进度节流无终值补发 | 122 文件夹具 0 条 Progress；真机 66 万文件欠账 1~199 |
| B4 | 函数返回值只剩尾批 ≠ 事件流 | 集合比对直接暴露 |

### 2. 修复（`crates/pony_core/src/disk.rs`）
- 三处 WalkDir 显式 `.skip_hidden(false)`（SKIP_DIRS/hive 名单仍生效）
- 新增 `TopLargest` 最小堆收集器：(size, path) 全序、O(cap) 内存、遍历不中断
- LargeFiles 改为结束时一次性全量降序发送（与返回值一致）；Done 前强制补发精确进度终值
- 前端零改动

### 3. 测试与验证
- 新增 `tests/integration_disk.rs`：8 个夹具测试 + 1 个 #[ignore] 真机冒烟（独立 std::fs 地面真值交叉验证，活盘漂移容差 ±8 大文件 / ±256 计数）
- 门禁终验全绿：fmt / clippy 双 crate 0 警告 / 136 单测 + 6 + 8 集成 / 真机冒烟 ok（163s）

### 4. 双审闭环
- @code-reviewer-a（正确性/回归）：有条件通过 → 条件已落实（fmt、cancel/LargeFiles 陈旧文档、堆准入改 (size,path) 全序）
- @code-reviewer-b（边界/失败路径/更简方案）：有条件通过 → 条件已落实（静止树门禁留档、cleaner 遗留建 TASK-034 跟踪）；采纳终值进度去重守卫与 max_files=0/1 单测；不采纳 Vec+truncate 替代方案（内存上界论据）

## 改动文件
- `crates/pony_core/src/disk.rs`（核心修复）
- `crates/pony_core/tests/integration_disk.rs`（新增）
- `03_TASKS/TASK-033.md`（新建）、`03_TASKS/TASK-034.md`（新建）、`03_TASKS/TASK-035.md`（新建）、`01_TASK_BOARD.md`
- `CHANGELOG.md`（Unreleased ×2 Fixed）
- `docs/DESIGN.md`（ADR-014）

## 当前结果
- TASK-033 收口为 Done；TASK-034/035 入 Backlog 跟踪

## 下一步动作
- **待用户**：应用内手动 QA——扫描一次核对「N 个大文件 · X」摘要与目录占用 Top 榜
- TASK-034：cleaner.rs 两处 WalkDir 的 skip_hidden 影响面评估（涉删除候选扩大，需独立评估）
- TASK-035：cleaner `Done.total_items` 触顶多计 1（UI 当前不展示该字段）

## Resume Hint
根因与决策详见 docs/DESIGN.md ADR-014 与 03_TASKS/TASK-033.md；真机冒烟运行命令：
`cargo test -p pony_core --test integration_disk -- --ignored`
