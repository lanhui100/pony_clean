# PonyClean 项目总控面板

## 项目信息
- 名称: PonyClean
- 描述: Windows 极简桌面小组件 — 进程监控报警 + C盘安全分析清理 + 内存整理
- 技术栈: Rust (Tauri v2) + Vue 3 + shadcn-vue + Tailwind + sysinfo + jwalk
- 负责人: @self
- 当前阶段: Wave 4 — 清理体验优化（5 任务全部完成：占用检查/一键清理/WU缓存/扫描合并/并行提速，待手动 QA）
- 更新日期: 2026-08-08

## 整体目标
1. 实时监控 Windows 进程，对 CPU/内存异常超高的进程报警，支持一键 kill
2. 安全扫描 C盘，分级展示可清理文件，支持勾选后批量清理
3. 一键整理内存（EmptyWorkingSet），恢复系统流畅度
4. 极简半透明悬浮窗 UI（胶囊灵动岛），美观、流畅、低资源占用

## 当前状态
- 最关键任务: **TASK-013/015/016 手动 QA** — 灵动岛面板接入 + Wave 3 + 设置面板验证
- 已完成: ✅ Wave 1 核心模块（monitor + cleaner + memory）开发完成，71 项测试通过
- 已完成: ✅ UI 重构（Tauri v2 + Vue 3 + shadcn-vue）+ 双窗口灵动岛
- 已完成: ✅ 主链路打通（面板接入 + 动态高度）+ 内存整理 + 托盘/通知/自启/配置 + 设置面板
- 最大阻塞: 无（待真实窗口手动 QA）
- 下一步最小动作: 启动 `npm run dev:tauri` 手动 QA 灵动岛交互与托盘/通知/设置

## 任务概览

| ID | 标题 | 状态 | 优先级 | 负责人 | 下一步 |
|---|---|---|---|---|---|
| TASK-001 | 项目脚手架搭建 | Done | P0 | @self | ✅ 已完成 |
| TASK-002 | 进程监控模块 | Done | P0 | @self | ✅ 已完成 |
| TASK-003 | C盘扫描与安全清理模块 | Done | P0 | @self | ✅ 已完成 |
| TASK-004 | UI 集成与数据流 | Done | P1 | @self | ✅ 已完成 |
| TASK-012 | 双窗口灵动岛毛玻璃实现 | Done | P1 | @self | ✅ 已完成 |
| TASK-014 | 内存整理（EmptyWorkingSet） | Done | P1 | @self | ✅ 已完成 |
| TASK-016 | 设置面板 — 告警阈值 + 开机自启 | Done | P1 | @self | ✅ 已完成 |
| **TASK-013** | **灵动岛面板接入 + 动态高度** | **Validation** | **P0** | **@self** | **手动 QA** |
| **TASK-015** | **Wave 3 托盘/通知/自启/配置** | **Validation** | **P1** | **@self** | **手动 QA** |
| TASK-023 | 删除前进程占用检查 | Done | P0 | @self | ✅ 82+6 测试通过 |
| TASK-024 | 一键清理 Safe + 释放量反馈 | Done | P0 | @self | ✅ 前端构建通过 |
| TASK-025 | Windows Update 缓存 + DataStore | Done | P0 | @self | ✅ SCM 服务控制 |
| TASK-026 | disk 大文件+目录占用合并 | Done | P1 | @self | ✅ 单遍历双产出 |
| TASK-027 | cleaner target 并行扫描 | Done | P1 | @self | ✅ 4 线程并行 |
| TASK-011 | UI 设计规范制定与对抗式审核 | In Progress | P1 | @self | 补产物（SPEC-011 缺失） |

## 里程碑
- **M1 (Wave 1)**: 核心功能完成（monitor + cleaner 模块） ✅
- **M2 (Wave 2)**: Tauri v2 版本 UI 重构完成 ✅
- **M3 (Wave 3)**: 托盘图标、系统通知、开机自启、配置持久化 — 代码完成，待 QA
- **M4 (Wave 4)**: 自定义阈值 UI ✅（设置面板）、清理路径规则可配（待）、动效打磨（部分）
