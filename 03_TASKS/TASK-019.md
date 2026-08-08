# TASK-019: 端到端测试与修复（暗色适配 + 删除按钮响应）

## Basic Info
- Status: Done
- Priority: P1
- Owner: @self
- Created: 2026-08-08
- Estimated: 4h
- Depends: TASK-018

## Goal
对运行中的应用做端到端验证，修复发现的暗色适配与删除按钮响应问题。

## Output
- Playwright E2E 脚本（`frontend/e2e/ui-interact.mjs`，19 项断言）+ 对比度审计（`ui-check.mjs`）
- `globals.css` 声明 `color-scheme: dark`（修复原生 select 下拉/滚动条浅色渲染）
- kill/删除按钮默认可见（opacity-50，原 opacity-0 仅 hover 显示）
- 分析面板底部批量删除确认态（"确认删除？" + ring）

## Acceptance
1. E2E 覆盖：kill 按钮、清理删除流程、大文件删除二次确认、目录占用、select 暗色
2. 删除安全：mock 数据验证 UI 流程；真实删除由 Rust 侧保护（扫描根前缀 + 受保护路径 + 审计日志，单元测试已覆盖）
3. `npx vue-tsc --noEmit`、`npm run build` 通过

## Validation
- E2E 19/19 通过（`cd frontend && npm run e2e`）
- 对比度审计 0 低对比度（修正背景回退逻辑后）
- `npx vue-tsc --noEmit` — Pass
- `npm run build` — Pass
- 手动 QA — 待用户确认实际窗口观感

## Next Action
用户手动确认窗口实际效果（select 下拉、按钮可见性）。