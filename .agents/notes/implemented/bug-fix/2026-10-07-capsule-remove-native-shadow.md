# Agent Note: 移除桌面胶囊态与贴边进度条态的原生阴影（纯净贴边方案 A）

Status: implemented

## Problem

桌面胶囊态（Pill）与磁吸屏幕边缘的进度条态（Bar）此前依赖 Windows 原生类样式 `CS_DROPSHADOW` 投射阴影。由于 DWM 投影强依赖 GDI Region 的二值化整像素阶梯轮廓，与前端 WebView2 亚像素平滑边缘产生错位，表现为：
1. 胶囊态边缘发脏、有粗糙不对称黑边/毛刺；
2. 进度条态无法与屏幕边缘齐平融入，反而投射出污迹状的突兀投影；
3. Morph 缩放过渡与收起过程中容易出现阶段性残影与跳变。

## Decision

彻底去除胶囊窗口的原生 `CS_DROPSHADOW` 阴影（方案 A）：
1. 在 `src-tauri/src/commands/window.rs` 中：
   - 将 `CapsuleForm::wants_shadow()` 统一返回 `false`（Pill 与 Bar 均不开启原生阴影）。
   - 初始化创建胶囊窗口 subclass 时显式设置 `set_native_shadow(hwnd, false)`。
   - 更新并强化 `wants_shadow_follows_form` 单元测试为双 false 机器硬断言。
2. 保留组件自身的内部发丝线高光（`inset 0 1px 0 rgba(255, 255, 255, 0.08)`）提供视觉精致度，使贴边进度条与屏幕边缘完全平齐贴合。

## Alternatives considered

- 保留原生阴影微调 Region：GDI Region 本质是阶梯光栅，无法解决亚像素边缘错位与 DWM 全向投射导致的顶沿溢出。否决。
- 方案 B（胶囊态 CSS 阴影 + 进度条无阴影）：需要在外层容器预留 padding 并适配 hit-test 与 morph 动画插值，较复杂且胶囊本体已有精致高光与半透明磨砂质感。用户选择方案 A。

## Consequences

- 胶囊态与进度条态完全去除粗糙的原生黑影与贴边污迹，UI 边缘锐利清爽，无缝贴合屏幕。
- 彻底根除因 DWM Region 投影残留导致的重绘 bug。
- 机器门禁：`cargo test -p pony_clean`、`cargo clippy -p pony_clean`、`npx vue-tsc --noEmit` 均全绿通过。
