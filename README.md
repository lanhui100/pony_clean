# PonyClean

Windows 极简桌面小组件。三大核心功能：

- **进程监控** — 实时检测 CPU/内存异常超高的进程，报警并支持一键 kill
- **C盘安全清理** — 分级扫描可清理文件，安全删除临时文件、缓存、回收站等
- **卡机启动管理** — 枚举所有开机自启动的第三方应用（注册表 Run 键 / 启动文件夹），支持一键关闭或重新打开，并自动过滤 Windows 系统自带启动项

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + shadcn-vue |
| 样式 | TailwindCSS 4 |
| 后端 | Rust (pony_core) |
| 异步 | tokio |
| 进程 | sysinfo |
| 磁盘 | jwalk + windows-rs |

## 开发

```bash
npm run dev:tauri      # Tauri dev（前端 HMR + Rust 热重载）
cargo test -p pony_core # 运行单元测试
```

## 版本管理

- 版本号唯一权威：`Cargo.toml` / `frontend/package.json` / `src-tauri/tauri.conf.json` / `Cargo.lock` 四处一致（CI 强制校验）
- 发版：`node scripts/bump-version.mjs 0.2.0 [--commit] [--tag]`（同步版本 + Cargo.lock + 归档 CHANGELOG）
- 自查：`node scripts/check-version.mjs`
- 流程详见 [docs/VERSIONING.md](docs/VERSIONING.md)

## 项目结构

```
pony_clean/
├── crates/pony_core/  # 业务核心库（零框架依赖）
├── src-tauri/         # Tauri 壳层
├── frontend/          # Vue 3 前端
├── scripts/           # 版本管理脚本（bump / check + 契约测试）
├── docs/              # 项目文档
├── CHANGELOG.md       # 版本变更记录
├── 00_DASHBOARD.md    # 任务总控面板
├── 01_TASK_BOARD.md   # 任务板
└── 03_TASKS/          # 任务卡
```
