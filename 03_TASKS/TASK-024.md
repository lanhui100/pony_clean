# TASK-024: 一键清理 Safe 级 + 清理前后释放量反馈

## Basic Info
- Status: Done
- Validated: 2026-08-08
- Priority: P0
- Owner: @self（agent team 编排）
- Created: 2026-08-08
- Estimated: 2h
- Depends: 无
- Complexity: A
- Spec: `04_SPECS/SPEC-024-OneClickClean.md`

## Goal
清理页（SpacePanel）垃圾区块增加"一键清理（仅 Safe 级）"：跳过逐项勾选直接执行；清理完成 toast 展示"本次释放 X GB"。

## 背景
小组件定位要求低操作成本；当前必须逐项勾选才能清理。策略清单 P0 体验项。

## Acceptance
1. 垃圾区块 done 态出现"一键清理"按钮（仅当存在 Safe 级可清理项时）
2. 一键清理 = 自动选中全部 Safe 项 → 走确认弹窗 → executeClean；Confirm 项不入一键
3. 清理完成 toast 显示释放量（复用选中项字节预估值），失败项数保持展示
4. `vue-tsc --noEmit`、`npm run build` 通过；不引入后端改动

## Non-Goal
- 不做"免确认直接删"（安全底线保留确认弹窗）
- 不做真实删除前后磁盘差值统计（近似值足够，P2 再升级）

## Validation Evidence
- `vue-tsc --noEmit`、`npm run build` ✅
- 实现：一键清理按钮（Zap 图标，仅 Safe 级）+ pendingClean 独立确认数据源 + toast 释放量
- 手动 QA：一键清理流程与释放量显示（留待用户）

## Next Action
手动 QA 一键清理与释放量 toast。

## Next Action
spec 审查通过后进入实现（改 `frontend/src/views/SpacePanel.vue`）。

## Resume Hint
读 `04_SPECS/SPEC-024-OneClickClean.md` → 改 `SpacePanel.vue`（新增一键清理按钮 + 复用 handleClean/confirmClean 路径 + toast 释放量）→ 前端门禁。
