# Agent Note: 补齐门禁分层——秒级本地层 + 10秒级中继层（P7 时间经济性）

Status: implemented

## Problem

pony_clean 此前只有 CI（远端层，分钟级），无本地层与中继层——fmt/clippy 的快速子集
只能在 push 后等 CI 跑分钟级才暴露，高频问题无法在提交时被秒级拦住。ponygo audit
（gate_layers_report，2026-09-01 框架修复）精确诊断为"缺本地层/中继层、有远端层"。

## Decision

按 P7 分层强制原理（本地窄、CI 全，两层跑同样检查 = 白付延迟）补齐两层：

- .githooks/pre-commit（秒级本地层）：cargo fmt --check + git diff --cached --check；
- .githooks/pre-push（10秒级中继层）：cargo clippy -- -D warnings + cargo test -p pony_core --lib；
- git config core.hooksPath .githooks（本地配置，不入版本库）。

CI（远端层）保留原样，不重复跑本地已过的检查（cargo fmt/clippy 在 CI 仍全量，属全仓矩阵）。

## Alternatives considered

- 只靠 CI 不补本地层：高频问题反馈延迟到分钟级，否决（P7）。
- 本地层跑全量测试：秒级预算被击穿，否决——本地只抓 fmt/空白等快速子集，
  全量测试归 CI。
- 用 lefthook 管理：当前仓库无 Node/pnpm 工具链义务，原生 .githooks 零依赖，
  与 ponygo 栈无关原则一致，否决引入。

## Consequences

- 三道时间经济防线就位：秒级（fmt/空白）→ 10秒级（clippy/核心单测）→ 分钟级（CI 全仓矩阵）。
- ponygo audit 门禁分层参考现报三层全"有"；级自洽 L0-L1 全 PASS。
- hooksPath 是本地 git 配置，协作者需各自 git config core.hooksPath .githooks（已在 AGENTS.md 投影命约可补充说明）。
