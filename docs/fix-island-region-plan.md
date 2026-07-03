# 灵动岛 Win32 Region / DPI 修复计划

## 背景

灵动岛窗口在 idle 态显示白色方框、胶囊位置偏移、缩放残影等问题。根因有三层坐标系统未对齐：
1. `SetWindowRgn` 用物理像素坐标
2. `setEffects({ blur })` 在整个窗口区域渲染 DWM 亚克力
3. CSS 用逻辑像素坐标

## 修复方案

### 核心变更：保留 `setEffects`（降级），DPI 校正所有坐标，修复拖拽单位

| 变更 | 原因 |
|---|---|
| 保留 `win.setEffects({ blur })`，设置 `color` 参数匹配背景色 | 顾问审查指出 `backdrop-filter` 在透明 WebView2 中未经验证，保留 DWM 亚克力作为稳定降级。设背景色为深色避免白色方框 |
| `SetWindowRgn` 坐标乘以 `devicePixelRatio` — DPI 校正 | 匹配物理/逻辑像素差异 |
| 拖拽代码 `e.screenX`（逻辑像素）到 `PhysicalPosition`（物理像素）的转换 | 两者单位不一致导致高 DPI 下拖拽偏移 |
| `onLeaveDone` 先 `await setRegionCapsule()` 再切 `idle` 状态 | 避免 region 切换滞后于状态切换的竞态 |
| `showIsland` 守卫条件增加 `leaving` | 避免动画中途被暴力打断 |
| CSS `backdrop-filter` 保留 | 岛面板毛玻璃效果由 Vue 层独立控制 |

### 视觉层设计

- 保留 `setEffects({ blur })` 提供 DWM 亚克力效果
- 设置 `color` 参数匹配背景 `hsl(30 12% 9% / 0.92)`，减少白色伪影
- 岛面板 CSS `backdrop-filter` 叠加在 DWM 亚克力之上
- 胶囊通过 `background: rgba(30, 28, 26, 0.95)` + `border-radius: 9999px` 实现 pill 形状
- `SetWindowRgn` 在 idle 态裁剪到 160×40 → 亚克力只出现在胶囊区域，不再有白色方框

### 状态流转

```
idle:     region = 160×40 (居中), 只有胶囊区域可点击，透明区域鼠标穿透
entering: region → 315×100 (全), 岛面板滑入动画
visible:  region = 315×100 (全), 正常交互
leaving:  region = 315×100 (全), 岛面板滑出动画结束 → idle
```

## 修改文件清单

| 文件 | 修改类型 | 说明 |
|---|---|---|
| `frontend/src/composables/useWindowMorph.ts` | 修改 | 移除 `win.setEffects()`；`setRegionCapsule/Full` 保留 DPI 校正 |
| `frontend/src/App.vue` | 排查 | 检查胶囊 `left: calc(50% - 80px)` 是否正确 |
| `frontend/src/styles/globals.css` | 排查 | 检查 `body`/`#app` 透明背景设置 |
| `src-tauri/src/commands/window.rs` | 无需改动 | `set_window_rounded_corners` 已有 x/y 参数 |

## 风险与验证

- **视觉**：无 `setEffects` 时 `backdrop-filter` 是否能模糊桌面 → 手动测试验证
- **交互**：idle 态胶囊区域外鼠标是否穿透 → 手动测试验证
- **DPI**：`setRegionCapsule/Full` 中的 `* dpr` 是否正确 → `window.devicePixelRatio` 在 WebView 中可靠
- **regression**：现有 68 个单元测试 + clippy + 构建全量通过

## 验收标准

1. Idle 态：仅显示胶囊（160×40），无白色方框背景
2. Idle 态：胶囊区域外鼠标事件穿透到下层窗口
3. Entring/Visible：岛面板全屏（315×100）显示，毛玻璃效果正常
4. 胶囊拖拽后再次展开，位置保持正确
5. 高 DPI 屏幕（125%/150%/200%）下无偏移
6. 收起动画后回到 idle 态，仅显示胶囊
