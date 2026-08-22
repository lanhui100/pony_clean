# Changelog

本项目的所有显著变更都会记录在此文件。
格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本遵循[语义化版本](https://semver.org/lang/zh-CN/)。
条目类型：`Added` / `Changed` / `Fixed` / `Removed` / `Security`（中文描述）。

## [Unreleased]

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Fixed: 更新安装重试期回落到滞后更新清单时被误判为「已是最新版本」——已确认存在新版本时改为按次失败继续重试，耗尽后如实提示失败原因
- Changed: 更新请求增加显式超时（检查 15 秒 / 下载 10 分钟），网络黑洞时不再无限停留在「准备中…」；下载进度行精简为仅进度条 + 百分比，去除重复 spinner
- Removed: CNB 发版平台与备用更新端点 —— 发版与更新源收敛为 GitHub 单平台（ADR-013），删除 `.cnb.yml` 流水线与遗留清单文件；存量 v0.2.0 及更早客户端需手动到 GitHub Releases 升级一次

## [0.3.4] - 2026-08-22

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Changed: 高级清理区交互优化 —— 点击组名（如「系统重置备份」）改为折叠/展开该组文件明细，默认全部收起；全选只由组行勾选框承担；组行增加折叠箭头与条目数指示
- Changed: 清理主操作按钮紧凑化 —— 「一键清理」「清空回收站」降为 h-7 紧凑尺寸（11px 字号），「清空回收站」由描边实底改为 ghost 悬浮显底，降低结果卡片中的视觉压迫
- Fixed: 贴边条屏幕边缘透明缺口 —— 贴边侧两角改方角、远端两角半圆（CSS + 原生 Region 同步），修复全圆角贴边留下的漏缝
- Fixed: 胶囊⇄贴边条 morph 动画中间帧被旧轮廓啃角 —— 过渡期立即应用两形态 Region 并集，动画结束后再切目标形态精确 Region

## [0.3.3] - 2026-08-22

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Changed: 设置页更新操作改图标按钮 —— 「立即安装」文字按钮改为与「检查更新」刷新按钮同款式的图标按钮（Download 图标，下载中切换旋转指示），视觉与交互统一
- Fixed: 更新包下载失败自动重试 —— 最多 3 次尝试、线性退避，每次尝试重新选择更新源端点，缓解直连 GitHub 下载地址的瞬时网络阻断（`error sending request`）
- Fixed: 更新失败提示中文化 —— 网络类错误映射为可读中文说明，原始错误保留一键复制

## [0.3.2] - 2026-08-22

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Fixed: 开发工具缓存统计失真 — npm/pip/cargo/gradle 五个缓存目标启用聚合模式，整目录统计体积并生成单个清理项、删除时整体移除，不再受单目标 5 万文件上限截断（修复「npm 缓存恒定 800+MB 且数值不变」）
- Changed: 清理分类维度重构 — 旧混合「缓存」分类拆分为「浏览器缓存」（Chrome/Edge/Firefox/IE 磁盘缓存）与「系统缓存」（Windows 更新下载、旧驱动备份、UWP/资源管理器缓存等），消除一级与高级区同名分类数值不一致
- Changed: 高级清理区改为按具体清理目标分组展示（如「旧驱动备份」「系统临时文件」），不再复用一级分类名；`CleanItem` 新增 `label` 字段贯通前后端，自定义规则支持选择新分类
- Changed: 应用图标资源全套重新生成（icon.ico 与各尺寸 PNG、MSIX 资源、托盘图同步更新）

## [0.3.1] - 2026-08-22

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Added: 设置页「软件更新」区块显示当前版本号（`getVersion()` 读取，随发版自动更新）
- Fixed: 托盘图标模糊 — 新增 DPI 对齐的专用托盘图（16/20/24/32px 预渲染，编译期嵌入），按主屏缩放比例选择最优尺寸，替代大图 GDI 缩小

## [0.3.0] - 2026-08-21

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Changed: 应用图标全面改版 — 米白色圆角背景 + 马头 logo 居中构图，全套 Tauri 图标重新生成（icon.ico 七尺寸 + 各尺寸 PNG + MSIX 资源），托盘与桌面图标同步更新
- Added: 应用自动重启能力 — 集成 tauri-plugin-process（Rust 插件注册 + `process:allow-restart` 权限 + 前端 `@tauri-apps/plugin-process`），更新安装后自动重启生效
- Fixed: dev 启动失败两处根因 — 残留 vite 进程占用 5183 端口；`relaunch` 误从 `@tauri-apps/api/process` 导入（Tauri 2 中已迁移至独立插件）

## [0.2.1] - 2026-08-21

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Added: 自动更新源增加 GitHub（主）+ CNB（备）双源冗余 — `tauri.conf.json` endpoints 新增 `https://github.com/lanhui100/pony_clean/releases/latest/download/latest.json`
- Changed: 构建流水线生成 updater 清单 `latest.json`（双架构签名 + GitHub Release 下载 URL）并随 Release 上传，使 GitHub 自动更新链路闭环

## [0.2.0] - 2026-08-21

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Added: 应用图标设计 — 抽象简笔马头（黑色线条 + 红色马鬃 + 点状眼睛）+ 米白底色，与 pony-agent 同系列；全套 Tauri 图标规格（icon.ico 多尺寸 + 各尺寸 PNG）已导出并集成（桌面图标 + 托盘图标）
- Added: GitHub Actions 构建流水线（`.github/workflows/build-installers.yml`）— windows-latest 原生构建 x64 + MSVC 交叉编译 arm64 NSIS 安装包，推 tag 自动创建 GitHub Release（与 CNB 方案 A 互为备份）
- Changed: 全部错误提示统一为 Toast 显示（带一键复制原始错误）— 覆盖启动项加载失败、进程加载失败、垃圾扫描失败、空间分析失败、删除大文件失败、清空回收站失败、设置保存失败、软件更新失败；内联错误文本移除（保留轻量占位），成功/信息状态维持内联
- Changed: 清空回收站结果改用统一 Toast 显示（错误提示带一键复制原始错误按钮），替代原内联文本
- Changed: 发版文档更新 — VERSIONING.md 新增「双平台发版」（GitHub + CNB 并行构建、tag 指向/版本守卫/签名密钥/tag 冲突注意事项），RELEASE_CHECKLIST.md 增加双平台构建验证步骤，RELEASE_BUILD.md 新增 §5.7 GitHub 发版流程
- Changed: 监控面板进程列表支持固定（暂停刷新，便于稳定终止进程，kill 成功后自动恢复）
- Changed: 统一操作反馈 toast — 公共组件（fixed 定位不随滚动、毛玻璃背景、错误可一键复制），监控/空间/启动三处面板统一
- Fixed: 监控面板 CPU 列排序三角换行（补 whitespace-nowrap）

## [0.1.2] - 2026-08-19

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Added: Windows NSIS 安装程序支持简体中文（安装界面语言选择器，默认随系统语言显示）
- Added: 代码签名初期方案 — 自签名证书生成 + exe 签名脚本（`scripts/sign/`），解决 Issue #7 浏览器/SmartScreen 警告；README 增加下载安装指引（解除文件锁定）
- Added: 应用内自动更新 — tauri-plugin-updater（设置面板检查更新，被动静默安装，x64/ARM64 双架构支持），更新源基于 CNB Release + git raw 固定清单

## [0.1.1] - 2026-08-19

- Added: 版本管理体系建设 — bump/check 脚本（`scripts/bump-version.mjs` / `scripts/check-version.mjs`）、CHANGELOG、CI 版本一致性校验、docs/VERSIONING.md
- Added: CNB 流水线交叉编译 Windows 安装包（NSIS `.exe`，推 tag 自动构建发布）+ docs/RELEASE_BUILD.md 方案文档
- Added: 双架构构建 — x64 + ARM64（Windows on ARM）安装包一并产出并上传 Release 附件
- Fixed: 关闭开机启动项报 invalid args — StartupItem 可选字段加 serde(default) 容错反序列化；错误信息中文化 + 一键复制原始错误

## [0.1.0] - 2026-08-15

- Added: 初始版本 — 进程监控报警、C盘安全扫描与分级清理、内存整理（EmptyWorkingSet）
- Added: 灵动岛悬浮 UI（胶囊/贴边进度条双形态、SWCA Acrylic 毛玻璃）
- Added: 托盘图标、系统通知、开机自启、设置面板（告警阈值配置）
- Added: 一键清理 Safe 级、删除前进程占用检查、Windows Update 缓存清理、大文件/目录占用合并扫描、target 并行扫描
- Added: 70+ 单元测试与集成测试、CI（fmt/clippy/doc/test）
