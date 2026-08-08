# TASK-013: 灵动岛面板接入 + 窗口高度动态扩展

## Basic Info
- Status: Validation
- Priority: P0
- Owner: @self
- Created: 2026-08-08
- Estimated: 4h
- Depends: TASK-012

## Goal
将 MonitorPanel / CleanerPanel 接入灵动岛窗口，打通「点击胶囊 → 查看占用高的进程 / 扫描可清理文件」主链路；island 窗口高度在概要态与展开态之间动态切换。

## Output
- `IslandWindow.vue` 按 activeTab 渲染 MonitorPanel / CleanerPanel
- `set_island_expanded` Tauri 命令（窗口尺寸 + 圆角 region 重算）
- 扫描期间胶囊窗口暂停 idle 隐藏（scan-state-changed 事件联动）

## Acceptance
1. 点击胶囊 → island 展开为 480px，默认显示监控面板
2. 切换清理 tab → 显示 CleanerPanel，扫描/清理流程完整可用
3. 扫描进行中 island 不因 idle 超时隐藏
4. `npx vue-tsc --noEmit`、`npm run build`、`cargo check -p pony_clean` 通过

## Validation
- `npx vue-tsc --noEmit` — Pass
- `npm run build` — Pass
- `cargo check -p pony_clean` — Pass
- 手动 QA — 待执行：展开/收起动画、面板滚动、扫描联动

## Next Action
真实 Tauri 窗口手动 QA：点击胶囊展开 480px、tab 切换、扫描时 idle 不隐藏、收起回 100px。