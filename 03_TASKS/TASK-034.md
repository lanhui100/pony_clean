# TASK-034: cleaner 扫描链路 skip_hidden 漏扫评估与修复

## Basic Info
- Status: Backlog
- Priority: P2
- Owner: @self
- Created: 2026-08-24
- Estimated: 1h
- Depends: TASK-033（同根因，disk 模块已修复）
- Complexity: S

## 背景
TASK-033 真机验证发现 jwalk 默认 `skip_hidden=true`（名字以「.」开头即跳过）。
`crates/pony_core/src/cleaner.rs:1434、1595` 两处 WalkDir 仍用默认值：
目标**根路径本身**不受过滤（如 `.cargo\registry` 作为 root 可正常扫描），
但 target 内部嵌套的点开条目录会被静默跳过（漏扫 → 统计偏少）。

## 为什么不在 TASK-033 顺手修
改动会扩大**删除候选**范围（隐藏目录内文件进入清理列表），属删除行为变更，
与 TASK-033 的展示准确性修复风险等级不同，需独立评估 + 安全审查。

## Acceptance
1. 盘点现役全部 scan target：确认哪些存在嵌套点开条目录、量化影响面。
2. 决策并实现：统一 `.skip_hidden(false)` 或按 target 配置化；受影响单测补齐。
3. 若扩大删除候选：Confirm 级默认不勾选契约不受影响；CHANGELOG 行为变更条目。
4. 顺带评估 SKIP_DIRS 名单大小写不敏感匹配（`.Git`/`.GIT` 变体在 skip_hidden(false) 后
   失去双重跳过保护；TASK-033 双审 P3-2 提出，git 工具实际只创建小写 `.git`，风险低）。

## Review Records
- 来源：TASK-033 双审 reviewer-B P3-3 / TASK-033.md 已知遗留项升级为独立任务。
