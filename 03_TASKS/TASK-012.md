# TASK-012: 双窗口灵动岛毛玻璃实现

## Basic Info
- Status: Validation
- Priority: P1
- Owner: @self
- Created: 2026-07-03
- Estimated: 6h
- Depends: TASK-007

## Goal
将当前单窗口胶囊/灵动岛 morph 改为胶囊窗口与灵动岛窗口分离，使灵动岛可以独立承载原生毛玻璃效果，同时避免胶囊态透明大窗口阻塞下方操作。

## Output
- `04_SPECS/SPEC-012-Dual-Window-Island.md` — 双窗口实现 spec
- Tauri 双窗口配置与窗口命令
- Vue 胶囊/灵动岛根组件分离
- 双窗口 click、drag、idle、show/hide 协调

## Acceptance
1. 胶囊态只保留真实 `160x40` 胶囊窗口，不再用 `315x100` 透明区域承载胶囊。
2. 灵动岛使用独立 `315x100` 窗口，默认隐藏；胶囊 hover 仅提供视觉反馈，单击后显示。
3. 灵动岛窗口可单独尝试 native blur/acrylic；失败时不能出现不透明黑框，必须回退到 Vue 拟态玻璃。
4. 胶囊拖拽时，灵动岛可见则保持中心对齐跟随。
5. idle/blur 后灵动岛隐藏，胶囊恢复可见。
6. `npx vue-tsc --noEmit`、`npm run build`、`cargo check -p pony_clean` 通过。

## Spec
详见 `04_SPECS/SPEC-012-Dual-Window-Island.md`

## Validation
- `npx vue-tsc --noEmit` — Pass
- `npm run build` — Pass（依赖包 pure annotation 警告，不影响构建）
- `cargo check -p pony_clean` — Pass
- `cargo build -p pony_clean` — Pass
- `tauri build --debug --no-bundle` — 未完成，10 分钟超时后停止残留构建进程

## Next Action
启动真实 Tauri 窗口做手动 QA：hover 不展开、click 显示、拖拽不触发展开、idle 隐藏、胶囊隐藏后不拦截 island 点击、副屏定位、native effect 是否仍出现黑框。

## Resume Hint
重点从 `frontend/src/composables/useWindowMorph.ts` 的 `showIsland()` / `hideIsland()` / `onEnterDone()` 开始排查跨窗口时序。
