# Agent Note: 载入 ponygo 治理架构（增量，双轨归一）

Status: implemented

## Problem

pony_clean 是成熟的 Rust/Tauri 桌面项目，已有自有治理资产（SPEC→TASK→REVIEW→LOG
流水线、docs/DESIGN.md 的 ADR、.github/workflows CI、手写 AGENTS.md），但无统一可判定的
治理根，机械判据与升级阶梯缺失——"有没有治理"靠人眼，成熟度靠感觉。

## Decision

按"已认可替代物"条款**增量载入 ponygo**，不推倒既有资产：

1. 注入 .agents/（notes + skills）与 .meta/（constitution/gates/docs-tier/meta.yaml）骨架。
2. 宪法槽位填事实初稿（待确认），sync 投影——既有手写 AGENTS.md 原文保留，投影块追加。
3. 成熟度目标 L2（机械判据层是主要缺口）。

**既有治理资产替代物映射（人工自评命中，机器不裁决价值）：**

| ponygo 形态 | 既有资产 | 处置 |
|---|---|---|
| .agents/notes/ 决策记录 | docs/DESIGN.md（ADR 技术选型）+ 04_SPECS/ 02_REVIEWS/ | 保留为既有载体，替代物命中问① |
| 任务/流程记录 | 03_TASKS/ 99_LOGS/ 00_DASHBOARD/ 01_TASK_BOARD | 保留，替代物命中 |
| .meta/gates/ 门禁 | .github/workflows/{ci,test,build-installers}.yml | 保留为 CI 强制层，替代物命中问② |
| .meta/constitution/ 投影 | AGENTS.md（手写约定） | 保留原文 + 追加 ponygo 投影块 |
| .meta/docs-tier/ | docs/ 分层（ARCHITECTURE/DESIGN/README 索引） | 保留，替代物 |

**不迁移**：既有资产不搬进 .agents/notes/ 或 .meta/——双载体并行是过渡态，
由后续 governance-review 决定是否归一；本次只新增 ponygo 骨架，零删除。

## Alternatives considered

- 全套迁移（把 docs/DESIGN.md、04_SPECS 搬进 .agents/notes/）：破坏既有流程与
  git 历史锚点，双轨过渡期强行归一风险高，否决（本次）。
- 只 audit 不注入骨架：ponygo 的机械判据/升级阶梯/技能路由无载体可挂，否决。
- 盲目 init 且覆盖 AGENTS.md：已由 sync 三分支避免——原文保留。

## Consequences

- 机械判据层建立：ponygo status / audit / verify-note.sh 可跑；卫生 WARN 生效。
- 双轨过渡：自有资产 + ponygo 骨架并存，由 governance-review 技能定期审查是否归一。
- 新决策从今日起先 ADR 后代码（时序条款），历史决策不回溯入册。
