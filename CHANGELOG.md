# Changelog

本项目的所有显著变更都会记录在此文件。
格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本遵循[语义化版本](https://semver.org/lang/zh-CN/)。
条目类型：`Added` / `Changed` / `Fixed` / `Removed` / `Security`（中文描述）。

## [Unreleased]

<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->

- Added: 应用图标设计 — 抽象简笔马头（黑色线条 + 红色马鬃 + 点状眼睛）+ 米白底色，与 pony-agent 同系列；全套 Tauri 图标规格（icon.ico 多尺寸 + 各尺寸 PNG）已导出并集成（桌面图标 + 托盘图标）
- Added: GitHub Actions 构建流水线（`.github/workflows/build-installers.yml`）— windows-latest 原生构建 x64 + MSVC 交叉编译 arm64 NSIS 安装包，推 tag 自动创建 GitHub Release（与 CNB 方案 A 互为备份）
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
