# TASK-016: 设置面板 — 告警阈值 + 开机自启

## Basic Info
- Status: Done
- Priority: P1
- Owner: @self
- Created: 2026-08-08
- Estimated: 3h
- Depends: TASK-015

## Goal
M4 里程碑：将告警阈值与开机自启从后端能力升级为可视化设置 UI。

## Output
- `SettingsPanel.vue` — CPU/内存告警阈值滑块（50-100%）、开机自启开关、保存反馈
- TitleBar 新增设置入口（齿轮图标）
- `useMonitor.setAlertThresholds()` — 保存后即时生效，无需重启

## Acceptance
1. 设置面板可调节 CPU/内存告警阈值并保存（持久化到 config.json）
2. 保存后前端告警检测立即使用新阈值
3. 开机自启开关保存后同步 HKCU Run 键
4. `npx vue-tsc --noEmit`、`npm run build` 通过

## Validation
- `npx vue-tsc --noEmit` — Pass
- `npm run build` — Pass
- 手动 QA — 待执行（滑块交互、保存反馈、自启开关）

## Next Action
随 TASK-013/015 一并手动 QA。