# Changelog

本项目的所有显著变更都会记录在此文件。
格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本遵循[语义化版本](https://semver.org/lang/zh-CN/)。
条目类型：`Added` / `Changed` / `Fixed` / `Removed` / `Security`（中文描述）。

## [Unreleased]

- Added: 版本管理体系建设 — bump/check 脚本（`scripts/bump-version.mjs` / `scripts/check-version.mjs`）、CHANGELOG、CI 版本一致性校验、docs/VERSIONING.md

## [0.1.0] - 2026-08-15

- Added: 初始版本 — 进程监控报警、C盘安全扫描与分级清理、内存整理（EmptyWorkingSet）
- Added: 灵动岛悬浮 UI（胶囊/贴边进度条双形态、SWCA Acrylic 毛玻璃）
- Added: 托盘图标、系统通知、开机自启、设置面板（告警阈值配置）
- Added: 一键清理 Safe 级、删除前进程占用检查、Windows Update 缓存清理、大文件/目录占用合并扫描、target 并行扫描
- Added: 70+ 单元测试与集成测试、CI（fmt/clippy/doc/test）
