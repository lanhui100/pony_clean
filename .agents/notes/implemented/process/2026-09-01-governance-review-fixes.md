# Agent Note: 治理审核问题修复——门被门入 CI + pre-push 违规反证

Status: implemented

## Problem

governance-review（2026-09-01）审计出 4 项：
1. 决策格式门负样本 spec 未入 CI——本地 pre-commit 守提交时，CI 不守"门禁本身没失效"；
2. 双轨治理资产未归一（docs/DESIGN.md 等与 .agents/notes 并存）；
3. hooksPath 协作者失效风险；
4. pre-push 钩子未通过违规反证（只测了 pre-commit 格式门）。

## Decision

- 问题 1（修）：ci.yml 新增独立 governance-gates job——正样本（现有 notes 全过）+
  负样本 spec（非法样本必须被拒），不依赖 Rust 工具链，门被门检验（P2）。
- 问题 2（评估结论）：维持双轨过渡——docs/DESIGN.md + 04_SPECS/03_TASKS 按
  "已认可替代物"条款合法，不强行归一（破坏性迁移留给 review 决策）。
- 问题 3（已处理）：AGENTS.md 已于 2026-09-01 补本地钩子挂载指引（git config
  core.hooksPath .githooks）。
- 问题 4（修 + 反证）：真实 dead_code 违规冒烟 pre-push → exit 101 拦截；
  冒烟还暴露一个教训——死代码探针名不能以 _ 开头（rustc 对下划线前缀项豁免
  dead_code），否则不是真违规（首次冒烟 exit 0 是正确放行，非漏拦）。

## Alternatives considered

- 门被门检验放本地 pre-commit：会拖慢秒级预算且协作者本地未必有 bash 环境
  （Windows 原生），否决——放 CI 独立 job。
- 双轨强行归一：破坏既有流程与 git 历史锚点，过渡态在可审计前提下合法，否决。
- 把 pre-push 违规冒烟结论写"exit 0 即未拦截"：错误归因（__ 前缀豁免），
  以真实探针为准，否决。

## Consequences

- 门禁有效性被 CI 持续证明（负样本必须被拒，否则 CI 红）。
- pre-push 中继层拒绝路径实证（exit 101）；探针经验写入本 ADR 防复发。
- 双轨过渡保持，由后续 governance-review 决定是否归一。
