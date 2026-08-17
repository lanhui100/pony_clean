# TASK-029: 窗口视觉统一与过渡打磨 — 三层合一 / 阴影 / 极简按钮

## Basic Info
- Status: Validation（代码 + 门禁部分 + 双代码审核完成；构建/编译门禁与手动 QA 留待用户环境）
- Priority: P0
- Owner: @self（agent team 编排）
- Created: 2026-08-08
- Estimated: 5h
- Depends: 无（承接 TASK-012/022 毛玻璃、TASK-028 空间面板）
- Complexity: B
- Spec: `04_SPECS/SPEC-029-VisualPolish.md`

## Goal
消除打开面板时可见的三层视觉分层（1 直角暗色层 + 2 圆角层），让原生 Acrylic、玻璃壳层、内容卡片统一为同一圆角形状；为窗口壳层、胶囊、贴边进度条补充阴影；面板⇄胶囊切换平滑无框体残影；清理面板「高级」菜单移到一键清理按钮上方；清理面板全部按钮极简化（默认无背景、hover 显背景）。

**追加（用户实测迭代）**：面板采用**方角毛玻璃**（SWCA 为窗口级、无法区域化到圆角，用户裁决取方角彻底一致）；支持**面板态水平拖动**（island 展开时按住面板空白处拖动，胶囊+面板同步移动、松手吸附）。

## 背景
用户反馈（4 点）：
1. 打开面板有 3 层：1 层直角暗色 + 2 层圆角（最底层亮色、最上层 Vue 层），需要统一；窗口壳层可加 Windows 阴影，胶囊和贴边进度条都需要阴影
2. 面板缩小为胶囊时不够平滑，中间态有框体残影
3. 清理面板中一键清理的高级菜单应在一键清理按钮上方
4. 清理面板按钮过于明亮突出，应极简化（如重新扫描按钮仅显示图标，hover 才显示背景色）

现状（代码证据）：
- `IslandWindow.vue`：`.island-shell` 直角铺满（无 border-radius）+ 1px 白边；`.island-card` 直角 inset 0 深色渐变；原生 Region 为 16px 圆角（`window.rs apply_full_round_region`）→ Vue 层与原生层形状不一致
- 阴影全关：`tauri.conf.json shadow:false`、两窗口 `setShadow(false)`、`DWMWA_NCRENDERING_POLICY=DISABLED`
- pill⇄bar：`form` 一变，`set_capsule_geometry` 立即把原生 Region 切到目标形态，CSS 双图层反向 scale+opacity 交叉淡入 → 中间态双影
- island→capsule：island 上滑 + 胶囊淡入两窗口并行，直角壳层边框滑动残留帧影

## Acceptance
1. 面板展开态四角同一 16px 圆角，无直角暗色层；原生/壳层/卡片三层形状一致（截图佐证）
2. island、pill、bar 三形态均有可见阴影（截图佐证）
3. pill⇄bar、island⇄capsule 过渡平滑、无框体残影，时长 200–350ms（截图/录屏佐证）
4. 「高级」折叠区位于「一键清理」按钮上方
5. 清理面板所有常驻按钮默认无背景（仅图标/文字），hover 显示背景；重新扫描按钮 icon-only
6. 门禁全绿：`npx vue-tsc --noEmit`、`npm run build`、`cargo check -p pony_core -p pony_clean`、`cargo fmt --check`、`cargo test -p pony_core`

## Non-Goal
- 不改动扫描/清理业务逻辑与数据流
- 不重构窗口形态状态机（pill/bar/island 状态机语义不变）
- 不新增视觉装饰（光晕、动效数量不增）
- 弹窗确认按钮保持语义色（临时浮层不参与极简化）

## Validation Evidence
- 门禁：`npx vue-tsc --noEmit` ✅、`cargo fmt --check` ✅
- 环境受限未跑（沙箱禁联网缺 anyhow、禁子进程管道）留待用户：`npm run build`、`cargo check -p pony_core -p pony_clean`、`cargo test -p pony_core`
- 审查闭环：spec 双路对抗（架构/UX）+ consultant 四问裁决 + 双代码审核（A/B）+ 全部采纳项回改
- 变更文件：`src-tauri/src/commands/window.rs`、`src-tauri/src/main.rs`、`src-tauri/tauri.conf.json`、`frontend/src/lib/windowMorphConfig.ts`、`composables/useWindowMorph.ts`、`components/{IslandWindow,CapsuleWindow,CapsuleBar,EdgeBar,TitleBar}.vue`、`views/SpacePanel.vue`、`components/ui/button/index.ts`
- 手动 QA（留给用户）：面板展开四角圆角一致无直角暗色层、island/pill/bar 三形态阴影、pill⇄bar 与面板⇄胶囊过渡无残影、高级菜单在按钮上方、按钮 hover 前无背景

## Next Action
用户在真实 Windows 会话运行 `npm run dev:tauri` 验证（spec §8 手动 QA 清单），并补跑 `npm run build` / `cargo check` 门禁。
