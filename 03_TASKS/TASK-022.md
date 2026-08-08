# TASK-022: 手写 SWCA Acrylic — 消除原生标题栏残留

## Basic Info
- Status: Done
- Priority: P1
- Owner: @self
- Created: 2026-08-08
- Estimated: 3h
- Depends: TASK-021

## Goal
解决展开面板"两层视觉不一致"：直角浅色层（原生 Win 标题栏）+ 圆角深色层（Vue 卡片）。

## 根因分析
- window-vibrancy 0.5.3 的 `apply_acrylic` 在 Win11 走 `DWMWA_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW` 路径
- **TRANSIENTWINDOW（瞬态窗口）背景会强制 DWM 绘制系统标题栏**（最小化/关闭/最大化按钮）
- 此前圆角 Region 恰好裁剪掉标题栏按钮区域；改直角 Region 后标题栏完整暴露
- 标题栏透过半透明 Vue 卡片可见 → "直角浅色层 + 圆角深色层"两层

## 方案
- 手写 SWCA：`SetWindowCompositionAttribute` + `ACCENT_ENABLE_ACRYLICBLURBEHIND`
- 该 API 不在 MSVC user32.lib 导入表（动态 API），用 GetProcAddress 动态加载
- Win10/11 通用、**不触发 DWM 标题栏**
- 失败回退 Blur（ACCENT_ENABLE_BLURBEHIND）→ CSS 渐变
- 移除 window-vibrancy 依赖

## Acceptance
1. island 窗口无原生标题栏（无最小化/关闭/最大化按钮）
2. 无"直角浅层 + 圆角深层"两层视觉
3. 运行时日志 `Acrylic applied to island window`
4. E2E 21/21 通过

## Validation
- `cargo fmt --check` — Pass
- `cargo clippy` — Pass（仅增量编译目录锁 warning，非代码问题）
- E2E 21/21 — Pass
- 运行时日志 — `[PonyClean] Acrylic applied to island window` ✅
- 手动 QA — 待用户确认真实窗口观感

## Next Action
用户确认真实窗口：无标题栏、无两层视觉、毛玻璃自然。