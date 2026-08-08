# TASK-020: OptionPicker 暗色下拉 + island 原生 Acrylic 毛玻璃

## Basic Info
- Status: Done
- Priority: P1
- Owner: @self
- Created: 2026-08-08
- Estimated: 4h
- Depends: TASK-019

## Goal
解决两个体验问题：① 原生 select 下拉在暗色主题下仍渲染浅色；② island 面板缺少真实毛玻璃效果。

## Output
- `OptionPicker.vue` 自定义下拉组件（暗色完全可控），替换全部 4 处原生 select
- island 毛玻璃：`window-vibrancy::apply_acrylic`（HWND 层级，DWM 合成）
- island 面板背景透明度 0.90→0.58 让 Acrylic 透出，失败时 CSS 渐变兜底
- 移除 IslandWindow 的 clearEffects 调用

## 技术要点（层级）
- Windows 毛玻璃是**窗口级（HWND）**效果：CSS backdrop-filter 只能模糊 WebView 内部，无法模糊窗口背后桌面
- 必须在 Rust 侧对 HWND 调用系统 API（SetWindowCompositionAttribute），且需在 hit-test subclass 之后应用
- 透明窗口 + Acrylic 是 Tauri 透明窗口的标准组合

## Acceptance
1. 页面无任何原生 select（全部 OptionPicker）
2. 应用运行时日志出现 `Acrylic applied to island window`
3. island 面板呈现真实桌面模糊（可调透明度）
4. E2E 21 项全通过

## Validation
- `cargo fmt --check` — Pass
- `cargo clippy -p pony_core -p pony_clean` — Pass（0 警告）
- `cargo test -p pony_core` — Pass（75 + 6）
- `npx vue-tsc --noEmit` — Pass
- `npm run build` — Pass
- E2E 21/21 — Pass（OptionPicker 暗色、原生 select 移除）
- 运行时日志 — `[PonyClean] Acrylic applied to island window` ✅
- 手动 QA — 待用户确认真实窗口观感（毛玻璃效果 + 下拉样式）

## Next Action
用户确认真实窗口毛玻璃观感与下拉交互。