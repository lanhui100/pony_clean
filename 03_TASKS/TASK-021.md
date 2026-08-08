# TASK-021: 借鉴 blur_win 重构 island 毛玻璃（消除圆角/直角分层）

## Basic Info
- Status: Done
- Priority: P1
- Owner: @self
- Created: 2026-08-08
- Estimated: 3h
- Depends: TASK-020

## Goal
解决毛玻璃"圆角面板 + 直角底部"分层问题。借鉴同级 blur_win 项目（Tauri 2 毛玻璃测试项目）的三层架构方案。

## 根因分析
- Acrylic 是 **HWND 直角区域**的 DWM 合成背景
- 原实现用 SetWindowRgn 圆角裁剪 + CSS 圆角面板，与 Acrylic 直角范围不一致（Win11 上 Acrylic 不完全尊重 Region）
- 结果：圆角面板下方露出 Acrylic 直角边缘 → 视觉分层

## 方案（blur_win 三层架构）
1. **原生窗口层**：island 窗口改直角 Region（radius=0），Acrylic 整块直铺
2. **CSS 玻璃壳层**（`.island-shell` 直角铺满窗口）：多层渐变基底 + 暖色径向光晕 + 内阴影 + backdrop-filter 辅助
3. **内容卡片层**（`.island-card` 圆角深色半透明）：玻璃上的卡片，四周留 6px 玻璃边距
4. Acrylic 失败回退 Blur，再失败回退 CSS 渐变（三级降级）

## Acceptance
1. island 窗口无圆角/直角分层（整块玻璃观感）
2. 内容区保持圆角卡片美学
3. 运行时日志 `Acrylic applied to island window`
4. E2E 21/21 通过、对比度审计 0 低对比

## Validation
- `cargo fmt --check` — Pass
- `cargo clippy` — Pass（仅增量编译目录锁 warning，非代码问题）
- E2E 21/21 — Pass
- 对比度审计 — 0 低对比
- 运行时日志 — `[PonyClean] Acrylic applied to island window` ✅
- 手动 QA — 待用户确认真实窗口观感

## Next Action
用户确认真实窗口毛玻璃观感（无分层、光晕层次）。