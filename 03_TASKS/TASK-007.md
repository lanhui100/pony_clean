# TASK-007: Vue 设计系统 + 窗口布局

## Basic Info
- Status: Backlog
- Priority: P0
- Owner: @self
- Created: 2026-06-27
- Estimated: 4h
- Depends: TASK-005

## Goal
实现 Tauri 窗口的外壳：无边框透明窗口、拖拽区域、标题栏、Tab 导航。配置 shadcn-vue 主题（CSS variables）与现有深色玻璃设计语言一致。

## Output
- `frontend/src/styles/globals.css` — Tailwind + shadcn-vue CSS variables（深色主题）
- `frontend/src/App.vue` — 窗口布局（TitleBar + TabBar + 内容区）
- `frontend/src/components/TitleBar.vue` — 标题 + 拖拽 + 关闭
- `frontend/src/components/TabBar.vue` — 标签页导航
- `tauri.conf.json` — 无边框、透明、置顶窗口配置
- `src-tauri/src/main.rs` — Tauri 窗口创建 + 拖拽处理

## Acceptance
1. 窗口无边框、透明背景、始终置顶
2. TitleBar 可拖拽窗口、关闭按钮可用
3. TabBar 正确切换 "进程监控" / "C盘清理"
4. shadcn-vue 组件（Button / Card / Separator）正确渲染
5. 深色主题与现有设计一致（碳黑背景、蓝/紫色强调色）
6. 窗口初始大小 420×680，响应式

## Spec
详见 `04_SPECS/SPEC-007-Design-System-Window.md`
