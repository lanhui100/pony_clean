# TASK-011: UI 设计规范制定与对抗式审核

## Basic Info
- Status: In Progress
- Priority: P1
- Owner: @self
- Created: 2026-06-28
- Estimated: 4h
- Depends: TASK-007, UI_DESIGN.md

## Goal
将 UI 设计稿（`docs/UI_DESIGN.md`）转化为可执行的 SPEC 规范文档，通过多智能体对抗式审核识别盲点，调优后锁定设计方向，指导 TASK-008/009 的实现。

## Output
- `04_SPECS/SPEC-011-UI-Design.md` — UI 设计规范文档
- `04_SPECS/SPEC-011-REVIEW.md` — 对抗式审核报告（含采纳/拒绝决策）
- `docs/UI_DESIGN.md` — 根据审核意见调优后的设计稿（v2）

## Acceptance
1. SPEC-011 覆盖设计系统、Monitor Panel、Cleaner Panel 三大域的完整规范
2. 至少 3 路智能体完成对抗式审核，产出书面审核报告
3. 审核意见逐条记录采纳/拒绝决策及理由
4. 设计稿根据采纳意见完成调优（delta 可追溯）
5. TASK-007/008/009 的 Acceptance 与 SPEC-011 保持一致

## Spec
详见 `04_SPECS/SPEC-011-UI-Design.md`
