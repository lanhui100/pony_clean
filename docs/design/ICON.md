# PonyClean App Icon

## 目标

为 `PonyClean`（Windows 极简桌面小组件：进程监控报警 + C盘安全分析清理）提供桌面图标与托盘图标，与同系列项目 `pony-agent` 保持视觉一致，同时体现本项目的"清理/守护"定位。

## 设计方向（多轮迭代收敛）

| 轮次 | 方向 | 结论 |
|---|---|---|
| 1 | 马头 + 盾牌轮廓（深色玻璃背景） | 马头写实感过强，弃 |
| 2 | 盾牌右下角徽章 + pony 抽象图形 | 构图偏离主体，弃 |
| 3 | 盾牌居中 + 回形马纹 | 回形纹填充不准确，弃 |
| 4 | 米白底 + 蓝色渐变盾牌 + 白色镂空 pony 图形 | 风格偏冷，弃 |
| 5 | 贴纸风（圆角边框 + 蓝色小马） | 颜色限定过死，弃 |
| 6 | 贴纸风（暗金棕边框 + 自然色雄壮小马） | 过于写实，弃 |
| 7 | 扁平手绘插画版 | 接近，仍偏写实，弃 |
| 8 | **简笔画（黑色线条 + 红色马鬃 + 米白底 + 无边框）** | 方向确认 |
| 9 | 简笔画抽象化（几何线条 + 红色马鬃） | 确认 |
| 10 | 抽象简笔 + 点状眼睛 | 采用（v11） |
| 11 | **粗笔触抽象化（线宽 4 倍）+ 透明底** | ✅ 最终采用（v13） |

### v13 相对 v11 的变化

- **线条加粗**：主线宽从约 14px 提至 56px（1024 画布），追求神似而非形似
- **背景透明**：去掉米色圆角方块与黑色外底，线条直接立于任意底色上
- 配色不变：黑线 `#181818` + 红鬃 `#D0342C`（经对比全红/金棕头方案后确认保留）

## 最终方案

- **主体**：抽象简笔马头侧脸——黑色粗轮廓线条（双耳、额头弧线、口鼻、颈线），无填充、无阴影
- **马鬃**：红色流动粗笔触（三缕渐细，沿颈部扫过）
- **眼睛**：单个黑色点状（极简）
- **背景**：完全透明（PNG alpha），无边框无底色
- **风格**：极简、扁平、干净负空间、小尺寸（16px-512px）清晰可辨
- **系列一致性**：与 pony-agent 同属 pony 品牌（马形主体 + 暖色系）

## 文件

- 设计候选：`docs/design/icon-candidates/`（v1-v15 迭代过程；`candidate-*-bold-transparent.svg` 为矢量源稿）
- 母版：`src-tauri/icons/icon-master.png`（1024x1024，透明底）
- 正式导出：`src-tauri/icons/`（`tauri icon` 生成全套规格）
  - `icon.ico`（16/24/32/48/64/256 多尺寸，桌面 + 托盘共用）
  - `icon.png`（1024）、`32x32.png`、`64x64.png`、`128x128.png`、`128x128@2x.png`
  - `Square*.png` / `StoreLogo.png`（Windows Store 规格）
- 托盘图标：复用 `icon.ico`（`app.default_window_icon()`，Windows 托盘自动选用 16/32 尺寸）

## 生成方式

```bash
# 矢量源稿 -> 透明母版（系统 Edge 渲染，需 Node）
node frontend/render-master.cjs docs/design/icon-candidates/candidate-13-bold-transparent.svg src-tauri/icons/icon-master.png

# 母版 -> 全套规格（会额外生成 iOS/Android/icns 产物，本项目为 Windows 桌面应用，需清理 ios/ android/ 目录）
npx --prefix frontend tauri icon src-tauri/icons/icon-master.png
```

辅助工具：

```bash
# 候选稿批量预览：全尺寸透明 PNG + 16-256px 明暗底可辨性网格
node frontend/icon-preview.cjs [svg ...]
```
