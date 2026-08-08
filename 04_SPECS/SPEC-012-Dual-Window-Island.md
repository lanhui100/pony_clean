# SPEC-012: 双窗口灵动岛毛玻璃实现

## 背景与目标
当前单窗口方案在 `315x100` 透明 WebView 中同时承载胶囊与灵动岛。CSS `backdrop-filter` 无法可靠模糊桌面背景；窗口级 Acrylic/Blur 又会污染整个窗口并可能出现黑色背景框。本 spec 将胶囊与灵动岛拆成两个 Tauri 窗口，以便灵动岛独立承载 native window effect。

## 范围
- 新增/配置 `capsule` 与 `island` 两个窗口。
- Vue 根据当前窗口 label 渲染不同根组件。
- 胶囊 hover 仅提供视觉反馈；单击胶囊显示灵动岛，idle/blur 隐藏灵动岛。
- 胶囊拖拽时同步灵动岛位置。
- 原生层提供窗口定位、显示/隐藏、可选毛玻璃 effect 命令。

## 非目标
- 不重构 monitor/cleaner 业务逻辑。
- 不实现托盘、通知或开机自启。
- 不保证所有 Windows 版本都有真实 Acrylic；不支持时回退拟态玻璃。

## 技术方案
1. `capsule` 窗口尺寸为 `160x40`，常驻顶层，负责胶囊 UI 和拖拽。
2. `island` 窗口尺寸为 `315x100`，默认隐藏，负责灵动岛 UI。
3. 胶囊窗口通过 Tauri invoke 请求显示/隐藏灵动岛；Rust 根据胶囊窗口位置计算 island 的中心对齐位置。
4. 灵动岛 native effect 只作用于 `island` 窗口。若 effect 失败，只记录日志并保留 Vue 半透明玻璃样式。
5. 前端保留 idle timeout 和拖拽阈值；胶囊 hover 不触发跨窗口 show，只管理轻量视觉反馈。

## 替代方案
- 单窗口继续模拟玻璃：风险低但无法真实 blur 桌面背景。
- 单窗口动态启用 Acrylic/Blur：已验证会出现不协调的黑色背景框，不采用。

## 影响面
- `src-tauri/tauri.conf.json`
- `src-tauri/src/commands/window.rs`
- `src-tauri/src/main.rs`
- `frontend/src/App.vue`
- `frontend/src/composables/useWindowMorph.ts`
- `frontend/src/components/*`

## 风险与回滚
- 风险：多窗口同步动画出现时序差。缓解：先使用 show/hide + CSS fade，不做复杂跨窗口物理 morph。
- 风险：native effect 在部分系统上无效或黑底。缓解：effect best-effort，失败回退 CSS。
- 风险：monitor 数据双窗口重复轮询。缓解：先接受，后续可用 backend event 广播优化。
- 回滚：移除 `island` 窗口配置，恢复单窗口 `main`/`capsule` 渲染路径。

## 测试计划
- `npx vue-tsc --noEmit`
- `npm run build`
- `cargo check -p pony_clean`
- 手动 QA：hover 不展开、click 显示、拖拽不触发展开、idle 隐藏、拖拽跟随、副屏位置、native effect 失败时无黑框。

## 验收标准
同 `03_TASKS/TASK-012.md` Acceptance。

## 审核记录
当前环境没有可调用子智能体工具，无法完成真正多智能体对抗审核。实施后由主会话按架构、交互、失败路径、测试覆盖四个维度做独立审查 pass，并在最终回复标明残余风险。

## 实施偏离与验证记录
- 已采用 `capsule` / `island` 双窗口，`App.vue` 按窗口 label 分发根组件。
- 胶囊窗口在 island 进入完成后隐藏，避免透明顶层窗口拦截 island 点击；island 离开前重新显示胶囊。
- native effect 仍使用 Tauri JS `Window.setEffects()` best-effort 接入，仅作用于 `island` 窗口；若实测仍黑框，下一步应切换到 `window-vibrancy` 原生 crate 或默认关闭 native effect。
- 自动验证：`vue-tsc`、前端 build、`cargo check`、`cargo build` 通过。
- 未完成：真实 Tauri 窗口手动 QA。
