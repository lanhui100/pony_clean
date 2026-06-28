# TASK-008: Vue 监控面板

## Basic Info
- Status: Backlog
- Priority: P1
- Owner: @self
- Created: 2026-06-27
- Estimated: 6h
- Depends: TASK-006, TASK-007

## Goal
实现进程监控面板全部 UI，功能等价于当前 egui 版本，UI 质量显著提升。

## Output
- `frontend/src/views/MonitorPanel.vue` — 监控面板主视图
- `frontend/src/components/ProcessTable.vue` — 进程表格（排序/搜索/高亮）
- `frontend/src/composables/useMonitor.ts` — Tauri IPC 封装 + 轮询逻辑
- 搜索、排序、kill、高亮阈值全部实现

## Acceptance
1. 顶部紧凑摘要行：CPU 总和 / 内存 / 进程数（着色阈值与 egui 版本一致）
2. 进程列表每 2 秒自动刷新（不闪烁）
3. 搜索框实时过滤进程名（URL case-insensitive substring match）
4. 列头点击排序（Name / CPU% / Mem / Mem%），▲▼ 指示器
5. 仅显示 CPU>10% 或 MEM>200MB 的进程（搜索时不过滤）
6. Kill 按钮 → 确认 → 反馈（成功/失败）
7. 进程名过长时 text-ellipsis
8. 使用原生 `<table>` 实现（非 shadcn-vue Table——需要自定义进度条/着色，原生 table 控制力更强），支持 striped rows（`even:bg-muted/5`）
9. 加载态（等待首次数据）和空搜索态

## Spec
详见 `04_SPECS/SPEC-008-Monitor-Panel.md` 和 `04_SPECS/SPEC-011-UI-Design.md`（SPEC-011 覆盖设计规范和对抗式审核调优结果）
