# SPEC-029: 窗口视觉统一与过渡打磨（三层合一 / 阴影 / 极简按钮）

- 关联任务：TASK-029
- 状态：Draft（待双路对抗审核）
- 复杂度：B

## 1. 背景与目标

### 用户反馈（原文归纳）
1. 打开面板貌似有 3 层：1 层直角暗色、2 层圆角（最底层亮色、最上层 Vue 层），三层需要更统一；窗口壳层可增加 Windows 阴影，胶囊和贴边进度条都需要阴影
2. 面板缩小为胶囊时不够平滑，过渡不自然，中间态有框体残影
3. 清理面板中「一键清理」的高级菜单应在一键清理按钮的上方
4. 清理面板所有按钮极简化，当前过于明亮突出；如「重新扫描」按钮应仅显示图标，hover 时才显示背景色

### 现状（代码证据）
| 现象 | 根因 |
|---|---|
| 三层分层不统一 | `IslandWindow.vue`：`.island-shell` 直角铺满 + 1px 白边框；`.island-card` 直角 inset 0 深色渐变；原生 Region 16px 圆角（`window.rs apply_full_round_region`）。Vue 层与原生层形状不一致，圆角仅靠 Region 硬裁 |
| 无任何阴影 | `tauri.conf.json shadow:false`；`IslandWindow/CapsuleWindow` onMounted `setShadow(false)`；`window.rs` `DWMWA_NCRENDERING_POLICY=DISABLED` |
| pill⇄bar 中间态残影 | `form` 变化瞬间 `set_capsule_geometry` 即把原生 Region 切到目标形态（硬裁剪动画中的内容）；CSS 双图层（pill/bar）同时反向 scale+opacity 交叉淡入 → 中间态双重叠影 |
| island→capsule 框体残影 | island 上滑 y:-120 + 胶囊淡入两窗口并行；`.island-shell` 直角暗色边框在滑动中形成残帧 |
| 按钮过于突出 | `SpacePanel.vue`：顶部扫描按钮 `variant="outline"`（亮边框）；一键清理 `default`（bg-primary 亮黄）；清理所选/删除所选 `destructive`（实心红） |
| 高级菜单位置 | `SpacePanel.vue` 「高级」Collapsible 在一键清理 Button 之后（L620-685 vs L602-610） |

## 2. 范围与非目标

### 范围
- `frontend/src/components/IslandWindow.vue`、`CapsuleWindow.vue`、`CapsuleBar.vue`、`EdgeBar.vue`、`TitleBar.vue`（如需）
- `frontend/src/views/SpacePanel.vue`、`frontend/src/components/ui/button/*`
- `frontend/src/composables/useWindowMorph.ts`、`frontend/src/lib/windowMorphConfig.ts`
- `frontend/src/styles/globals.css`
- `src-tauri/src/commands/window.rs`、`src-tauri/tauri.conf.json`

### 非目标
- 扫描/清理业务逻辑、数据流零改动
- 窗口形态状态机语义（pill/bar/island）不变
- 不新增装饰性动效；不改弹窗按钮语义色

## 3. 方案

### 3.1 三层统一（island）
- `.island-shell`、`.island-card` 统一 `border-radius: 16px`（与 `ISLAND_RADIUS` 一致），`overflow: hidden`
- 合并视觉面：shell 保留细边框 + 玻璃渐变 + 氛围光晕；card 深色渐变与 shell 同形同角，去除非必要的第二层边框感（内阴影只保留顶部高光）
- `window.rs` 中过时注释（“直角 Region/radius=0”）同步更正

### 3.2 阴影
- **首选（原生 Windows 阴影）**：`tauri.conf.json` 两窗口 `shadow: true`；Vue 两窗口 `setShadow(true)`；Rust 侧显式设置 `CS_DROPSHADOW`（0x00020000）类样式（`GetClassLongPtrW/SetClassLongPtrW`，幂等 OR 操作），DWM 按 Region 形状投影（island 圆角 / pill / bar 各自形状）
- **回退（若截图验证原生阴影对透明窗口无效）**：扩大两窗口逻辑尺寸留出阴影边距（约 16-20px），Region 扩至阴影范围、命中测试保持内容矩形不变；CSS box-shadow 绘制阴影
- 胶囊 pill / EdgeBar 的阴影随 Region 形状（pill ⇄ bar 形态切换后 Region 重算，阴影跟随）

### 3.3 过渡平滑
- **pill⇄bar**：原生几何同步（`set_capsule_geometry`）延迟到 CSS morph 动画完成（motion `on-animation-complete` → `syncGeometryToBackend`，`lastSyncedForm` 幂等防抖）；出层只淡出（不缩放），入层单独 morph（scale+opacity），消除双向重叠残影
- **island→capsule**：island 收起改为向顶边胶囊收缩（`scaleY→0.15` + `opacity→0`，`transform-origin: top center`），胶囊以 `scale 0.9→1` 接续淡入，两窗口时序对齐（~250-300ms）；此方案要求 island 壳层先圆角（3.1），否则收缩时仍见直角残影
- island 展开进入动画保持现有 spring，仅调整收起方向

### 3.4 高级菜单上移
- `SpacePanel.vue`：把「高级」Collapsible 块移到「一键清理」Button 之前

### 3.5 按钮极简化
- 顶部扫描/取消按钮：`variant="ghost"` icon-only（hover `bg-white/5`）
- 「一键清理」：主 CTA 降为柔和样式（透明底 + 细描边 + primary 文字图标，hover 才加深背景），不再 `bg-primary` 实心高亮
- 「清理所选」「删除所选」：ghost + destructive 文字（hover 才 `bg-destructive/15`），取消实心红底
- 「清空回收站」保持文字链接样式（本就 muted）
- 弹窗确认/取消按钮保持现状（临时浮层语义色，不参与极简）
- 图标按钮保留 `title`（无障碍）

## 4. 影响面与依赖
- 窗口尺寸常量若走回退方案：`windowMorphConfig.ts` 与 `window.rs` 常量须同步（capsuleW/H、fullW/H、ISLAND_RADIUS）
- `positionIslandWindow` 定位依赖 `WINDOW_MORPH.fullW`，回退方案需按新窗口宽重新钳位
- 原生阴影若不可用：回退方案涉及 region 形状与命中区域边界，需回归「点击穿透」行为

## 5. 风险与回滚
- **CS_DROPSHADOW 对 DWM 合成透明窗口可能无效**：以截图验证，无效则走 CSS 阴影回退（回退不引入新依赖）
- **DWMWA_NCRENDERING_POLICY 保持 DISABLED**：不改此设置（恢复会重新引入 Win11 标题栏问题，TASK-022 教训）；若 CS_DROPSHADOW 受其压制，再评估 margin 方案
- **Region 扩大影响点击穿透**：回退方案中命中测试仍用内容矩形（`capsule_content_phys`），阴影区不响应点击，但 Region 外点击穿透行为需实测
- 回滚 = 逐文件 revert（全部变更集中于上述文件列表）

## 6. 测试计划
1. `npx vue-tsc --noEmit`、`npm run build`
2. `cargo check -p pony_core -p pony_clean`、`cargo fmt --check`、`cargo test -p pony_core`
3. 截图验证：面板展开态四角 / pill / bar 阴影 / 过渡中间态 / 高级菜单位置 / 按钮 hover 前后
4. 手动 QA 清单：点击穿透（胶囊周边）、拖动、bar hover 展开、扫描联动

## 7. 验收标准
1. 面板展开态四角同一 16px 圆角，无直角暗色层（截图）
2. island / pill / bar 三形态均有可见阴影（截图）
3. pill⇄bar、island⇄capsule 无框体残影，过渡 200–350ms（截图/录屏）
4. 「高级」折叠区位于「一键清理」按钮上方
5. 清理面板常驻按钮默认无背景（仅图标/文字），hover 显示背景；重新扫描 icon-only
6. 门禁全绿

## 8. 审核记录

### Reviewer A（架构/窗口层）与 Reviewer B（前端动效/UX）双路对抗
采纳（两 reviewer 一致）：
1. 弃 CS_DROPSHADOW 原生阴影，改为「扩大窗口边距 + CSS box-shadow」唯一方案
2. island 收起先播 scaleY 收缩、动画完成后再切物理高度（现实现是立即 set_size 硬裁，P0）
3. 拆分 on-animation-complete 回调（pill/bar morph 与 island 淡出层不得共享，P0）
4. 几何同步串行化（form 切换后延迟 300ms 同步原生 Region，防硬裁剪中间态）
5. 阴影环带点击穿透代价：接受（consultant Q4 裁决），命中测试仍用内容矩形

采纳（reviewer B）：
6. 新增 `soft` / `destructive-ghost` 变体与显式 disabled 态；图标按钮补 `aria-label`（P1/P2）
7. reduced-motion 覆盖 motion-v spring（matchMedia → duration:0）（P2）
8. pill⇄bar 改用 CSS Transition（v-if + enter/leave class）替代双 motion.div 交叉淡入——出层只淡出、入层 morph，规避 complete 不可靠与双向叠影

部分采纳：
9. reviewer A「Acrylic 不受 Region 裁剪是直角暗色层来源」——采纳为根因假设之一，实施「双保险」：区域化毛玻璃（DwmEnableBlurBehindWindow + hRgnBlur 圆角，consultant Q3 裁决）+ CSS 层统一 16px 圆角
10. reviewer A「回退方案≈几何层重写」——接受其成本评估，本 spec 明确为窗口几何常量全量联动（§4 扩充），非"常量同步"

不采纳：
11. reviewer A「保留 SWCA 只改 CSS」与「立即同步为兜底主路径」——consultant Q3 已裁决区域化 blur 为主路径（SWCA 为回退）；延迟同步为主路径（300ms 定时 + 末次生效，等价于 pending 队列，无 complete 依赖）

### Consultant 终审
- Q1 主 CTA：**soft 折中**（`bg-primary/15 text-primary border-primary/30 hover:bg-primary/25 hover:border-primary/45`），保留唯一主 CTA 层级与 disabled 可辨性
- Q2 高级区：**仅重排**（Safe 分类行 → 高级 Collapsible → 一键清理），不做吸底/锚定
- Q3 毛玻璃：**区域化**（DwmEnableBlurBehindWindow + 圆角 hRgnBlur，染色归 CSS；SWCA 回退）
- Q4 阴影环带：**接受 ~8-12px 环带吃点击**（HTTRANSPARENT 有 Win11 标题栏预览风险，单侧阴影不对称）

### 实现偏差记录
- 目标几何：island 窗口 347×116/496（内容 315 + 左右 2×16 边距 + 下 16 边距）、胶囊 192×56（160 胶囊顶边贴边 + 左右 2×16 + 下 16）。Region 外扩 12px 覆盖阴影，命中测试仍用内容矩形
- pill⇄bar 采用 CSS Transition（v-if + enter/leave class + `--from-*` 变量），出层仅 160ms 淡出、入层 300ms morph；原生几何同步延迟 300ms+50 由单定时器串行化
- island 收起 = scaleY→0.12 折叠向顶边 + 淡出（200ms easeIn），**动画完成后**才切物理高度；胶囊立即 show（z 高于下叠于其下方，折叠时逐段露出，视觉即「缩回胶囊」）
- 按钮：新增 `soft`（一键清理）、`destructive-ghost`（清理所选/删除所选）、`ghost` hover 改 `bg-white/5`、`icon-sm` 尺寸；顶部扫描/取消改 ghost icon-only + aria-label

### 关键修订（用户实测反馈后，2026-08-08 二次修订）
用户实测面板透明、无毛玻璃、阴影不自然 → 修正两处：
1. **毛玻璃回退 SWCA Acrylic**：此前按「区域化 DwmEnableBlurBehindWindow」实现，实测在透明 WebView2 窗口返回成功但不出毛玻璃。已恢复 `apply_acrylic_swca` 为首选；移除 `apply_island_blur`/`DWM_BLURBEHIND` 等死代码
2. **阴影自然化**：CSS 阴影统一「key 光+环境光」两层且 blur≤12px；`apply_island_region` 左右对称；卡片 alpha 调回

### 终版修订（用户裁决，2026-08-08）
用户实测「面板即窗口」后：无外圈了，但**面板上层圆角、底层 SWCA 直角，四角两层不重叠**。确认机制：SWCA Acrylic 是窗口级、铺满方形窗口且不被 SetWindowRgn 裁剪，任何圆角 Region/CSS 都会在四角露出底层 SWCA 直角。
**用户裁决：面板采用「方角毛玻璃」**（最快最稳）：
- `apply_island_region` 改为方角 Region（椭圆直径 0）
- `.island-shell`/`.island-card` 移除 `border-radius`（方角），SWCA = CSS = Region 三者方角彻底一致，四角无分层
- 胶囊（pill/bar）保持圆头胶囊形状（无毛玻璃、不受影响）
- 阴影仍为原生 DWM（CS_DROPSHADOW），按方角 Region 投影
- 残余风险：原生 CS_DROPSHADOW 在透明+`DWMWA_NCRENDERING_POLICY=DISABLED` 窗口可能不投影（需用户实测确认）；若阴影缺失再补方案

### 代码审核（Reviewer A 窗口层 / Reviewer B 前端 UX）与回改
- Reviewer A：**有条件通过**。无问题项：DWM_BLURBEHIND 布局/客户区坐标/DeleteObject、Region 椭圆直径语义、常量三方同步、非 windows 分支。
- Reviewer B：**有条件通过**。正确落地：三层统一、CSS 阴影边距、Transition morph 矩形映射验算、几何同步串行化、complete 回调拆分、高级区空/非空两态渲染、reduced-motion 全覆盖。
- 采纳回改：
  1. `hideIsland` 去掉硬编码 140ms 延迟 show → 立即 `win.show()`（island z 序更高、折叠时逐段露出胶囊，天然「缩回」；消除两窗口动画无同步锚点的竞态——A P2-4 / B P1-1）
  2. `apply_capsule_region` 补充注释说明 bar 满宽时左右阴影 pad 有意钳回 0（bar 阴影仅向下）
  3. island 收起兜底：`scheduleShrinkFallback` 600ms 强制清 `expanded`（防 on-animation-complete 不派发致 expanded 残留——A P1-1 / B 建议）
  4. island 壳层阴影 blur 收敛（0 10px 20px）以适配 16px 边距（B P1-2）
  5. `TitleBar.vue` side 图标按钮补 `aria-label`/`aria-current`（B 建议）
  6. `apply_capsule_region` pad 改用双轴 DPR（`phys_w/lw`、`phys_h/lh`），消除竖边扩展雷（A P1-2）
  7. `onCapsuleAnimComplete` 重命名 `onIslandFadeComplete` + 注释「pill/bar 为纯 CSS transition 不派发 motion complete」，落成裁决 3 显式约束（A P2-3）
  8. 删除死命令 `set_hit_test_mode` 与 `apply_window_region`（无前端调用；杜绝把 capsule 切到 island Region 的几何错乱入口）（A P3-5）
  9. `apply_island_region` 圆角椭圆按 `min(rw, rh)` 钳到区域短边，防止概要态窄高区椭圆弧切入阴影（B P1-2；CreateRoundRectRgn 虽内置钳制，显式化更稳）
- 接受残余风险：阴影环带吃点击（已裁决）；A「P0-1 bar 侧向 pad」经复核为有意行为（bar 满宽、阴影仅向下、纵向 pad 已保留），已用注释与双轴 DPR 澄清；bar 形态 Region 圆头半径由 CreateRoundRectRgn 自动钳到 rect 短边，属可接受外观；B P2-3「soft hover 对比度偏弱」与 P3-1「icon-sm 28px」为打磨项，spec 范围外保留（hover/disabled 语义 consultnt 已批准整体 opacity-50）；B P2-2 reduced-motion 三路径均被覆盖（island/capsule spring→0.001、morph→CSS 0.01ms）
- 环境限制：`cargo check` 与 `npm run build` 因沙箱禁联网（crates.io 缺 anyhow 1.0.103）与子进程管道（spawn EPERM）无法在本会话执行（升级权限被用户取消）；`npx vue-tsc --noEmit` ✅、`cargo fmt --check` ✅。构建/编译级门禁留待用户环境验收。
