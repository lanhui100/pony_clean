# TASK-033: 清理 Tab 空间分析扫描数目准确性验证与修复

## Basic Info
- Status: Done（复现→修复→门禁全绿→真机交叉验证通过→双审闭环；手动 QA 转日常使用观察）
- Priority: P0
- Owner: @self（agent team 编排）
- Created: 2026-08-24
- Estimated: 3h
- Depends: TASK-026（合并单遍历引入缺陷）、TASK-028（前端消费方）
- Complexity: A
- Spec: 无独立 spec（验证型任务，验收标准即本卡 Acceptance；根因/方案记录于本卡）

## Goal
1. 验证清理 Tab「空间分析」扫描产出的大文件集合、目录占用、进度计数与真实磁盘一致。
2. 修复验证中发现的所有数目失真缺陷，保证前端显示的文件数目准确。

## 背景（验证发现的缺陷，全部已复现）
`crates/pony_core/src/disk.rs` 的 `scan_user_dir`/`scan_large_files` 在循环内每凑满
50 个大文件就 `split_off` 分批推送事件，引发四个失真：

| # | 缺陷 | 用户可见症状 |
|---|---|---|
| B0 | jwalk 默认 `skip_hidden=true`（名字以「.」开头即跳过，跨平台名字判定非 Windows 属性），disk 模块三处遍历全部中招：`.rustup`/`.cargo`/`.bun`/`.gradle` 等开发缓存整棵静默漏扫 | 大文件数、目录占用、「已扫描 N 个文件」系统性偏少（真机 ≥100MB 大文件 78 vs 真值 97） |
| B1 | `files.len() >= max_files` 截断判断作用在分批后的余量上：上限 ≥50 时永不触发；<50 时保留的是**先遍历到的 N 个**（与体积无关） | 结果集可能远超设计的 1000 上限，或漏掉真正最大的文件 |
| B2 | 结束时只对尾批排序，全局顺序 = 并行遍历遭遇序 | 「仅展示最大的 20 个」名不副实 |
| B3 | 进度事件每 200 个节流、终值不补发 | 前端「已扫描 N 个文件」欠账最多 199；不足 200 的目录树计数全程停在 0 |
| B4 | 函数返回值只剩分批余量，与事件流数据不一致 | API 层数据自相矛盾（当前调用方恰好忽略返回值才未出事） |

## 方案
1. 三处 WalkDir 显式 `.skip_hidden(false)`（SKIP_DIRS 名单剪枝与 hive 名单仍生效）。
2. 新增 `TopLargest` 最小堆收集器（容量 `max_files`，(size, path) 全序保证截断集合确定性）：
   遍历全程不中断（保进度与 DirUsage 完整）；结束时一次性发送全量降序 `LargeFiles`
   事件（与返回值一致）；`Done` 前强制补发精确 `Progress` 终值。前端零改动
   （useDisk.ts 对单事件的 concat 语义天然兼容）。

## Acceptance
1. 夹具集成测试（120 大文件 + 小文件/hive/Temp/node_modules/隐藏目录干扰项）证明：
   大文件集合 == 地面真值、无重复、全局降序、终值进度 == 遍历数、DirUsage 文件数/字节逐目录吻合。✅
   （`tests/integration_disk.rs::test_scan_user_dir_event_stream_matches_disk_truth`）
2. cap 语义：60 候选 + max_files=10 → 恰好保留最大 10 个且降序；命中数恰等于上限（=120）全保留。✅
3. 返回值 == 事件负载；隐藏目录纳入扫描且 `.git` 剪枝仍生效；零命中/取消契约锁定。✅
4. 真机冒烟（#[ignore] 手动）：生产参数（≥100MB/深度 3/上限 1000）扫描 %USERPROFILE%，
   与独立单线程 std::fs 地面真值交叉比对通过。
5. 门禁全绿：`cargo fmt --check` ✅、`cargo clippy -p pony_core -p pony_clean`（0 warning）✅、
   `cargo test -p pony_core` ✅；`npm run build`
   环境受限失败（沙箱 spawn EPERM + tailwindcss oxide 原生模块加载失败，与本次零前端改动无关，
   同 TASK-029 先例）。

## Non-Goal
- 不改前端展示逻辑（本次缺陷根因全在后端事件契约）。
- **已知遗留（不在本卡修）**：cleaner.rs 的 jwalk 遍历同样默认 skip_hidden=true——
  因目标根路径本身不受过滤、现役 target 极少含嵌套点开条目录，实际影响小；
  且改动会扩大**删除候选**范围，属行为变更，需独立评估后再动。
- 已知遗留：cleaner `Done.total_items` 在触顶时多计 1，UI 不展示该字段，暂不处理。

## Review Records
- @code-reviewer-a（正确性/回归）：有条件通过。条件已落实——fmt 修复、cancel/LargeFiles
  陈旧文档修正；P3（堆准入平局确定性）采纳为 (size, path) 全序比较。
- @code-reviewer-b（边界/失败路径/更简方案）：有条件通过。条件已落实——①静止树三重门禁
  终验留档；②cleaner skip_hidden 遗留建 TASK-034 跟踪。
  采纳：终值 Progress 去重守卫、max_files=0/1 单测、TASK-034 建卡。
  不采纳：Vec+sort+truncate 替代 TopLargest（内存上界是 pub API 真实论据，reviewer 自身亦裁定维持）；
  SKIP_DIRS 大小写不敏感（并入 TASK-034 后续评估）；
  扫描中异步取消契约测试（预置取消已锁事件序列形状，异步时序在 jwalk 并行下天然不可靠）。

## 测试证据
- 复现（修复前）：60 候选 + cap=10 实际返回 `[1K..10K]`（最小 10 个）；122 文件夹具 0 条 Progress 事件；
  真机 ≥100MB 大文件 78 vs 独立真值 97（缺失样本全部位于点开头目录）。
- 修复后（静止树终验，2026-08-24）：
  `cargo fmt --check` exit 0；`cargo clippy -p pony_core -p pony_clean` 0 警告；
  `cargo test -p pony_core`：136 单测 + 6 cleaner 集成 + 8 disk 集成全绿（1 ignored）；
  真机冒烟 `-- --ignored` ok（66 万+ 文件两遍遍历互证，163s）。
