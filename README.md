# PonyClean

Windows 极简桌面小组件。三大核心功能：

- **进程监控** — 实时检测 CPU / 内存异常偏高的进程，报警并支持一键 kill；支持对非关键进程调用 `EmptyWorkingSet` 做内存整理（仅释放工作集，不结束进程）
- **C盘安全清理** — 分级扫描可清理文件（临时文件、缓存、日志、prefetch、旧安装残留、应用缓存、开发工具缓存、Windows Update 缓存等），安全删除并支持占用文件延迟删除
- **卡机启动管理** — 枚举第三方自启动项（注册表 Run 键 + 启动文件夹），支持一键关闭或重新打开，并自动过滤 Windows 系统自带启动项

## 功能特性

- **磁盘分析** — 大文件扫描（按类型分类）与目录空间占用分析，定位空间黑洞
- **灵动岛悬浮 UI** — 胶囊 / 贴边进度条双形态，SWCA Acrylic 毛玻璃、托盘图标、系统通知
- **设置面板** — 告警阈值、自启开关、清理目标启停、自定义清理目标、磁盘分析参数配置

## 下载与安装

从 [Releases](https://cnb.cool/lanhui100/pony_clean/-/releases) 页面下载 `PonyClean_<版本>_<架构>-setup.exe`（x64 为 64 位，arm64 为 Windows on ARM），双击即可安装。

> 当前为内测分发，安装包为**未权威签名**（自签名），浏览器/SmartScreen 可能拦截，请按下面步骤放行：
>
> 1. 浏览器下载 `.exe` 弹"保留/仍要保留"时，点**保留**（或右键属性 → 勾选"解除锁定"）
> 2. SmartScreen 弹"Windows 已保护你的电脑"时，点**更多信息 → 仍要运行**
> 3. 内测用户可双击 `ponyclean-selfsigned.cer` → 安装证书 → 选择"受信任的根证书颁发机构"，红色拦截即消失
>
> 这条黄色警告的根源是"安装包未签名"，正式发版将接入权威代码签名证书彻底消除。
> 内测签名脚本见 `scripts/sign/`，详见 [docs/RELEASE_BUILD.md](docs/RELEASE_BUILD.md)。

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + shadcn-vue |
| 样式 | TailwindCSS 4 + Vite |
| 后端 | Rust (pony_core 业务库，零 Tauri 依赖) |
| 异步 | tokio + tokio-util |
| 进程 | sysinfo |
| 磁盘 | jwalk + windows-rs |

## 开发

```bash
npm install             # 安装前端依赖
npm run dev:tauri       # Tauri dev（前端 HMR + Rust 热重载）
cd frontend && npm run dev   # 仅启动前端 Vite dev server
cargo test -p pony_core       # 运行单元测试
cargo check -p pony_core -p pony_clean   # 类型检查两 crate
cargo clippy -p pony_core -p pony_clean  # clippy 检查
```

## 版本管理

- 版本号唯一权威：`Cargo.toml` / `frontend/package.json` / `src-tauri/tauri.conf.json` / `Cargo.lock` 四处一致（CI 强制校验）
- 发版：`node scripts/bump-version.mjs 0.2.0 [--commit] [--tag]`（同步版本 + 刷新 Cargo.lock + 归档 CHANGELOG）
- 自查：`node scripts/check-version.mjs`
- 流程详见 [docs/VERSIONING.md](docs/VERSIONING.md)

## 项目结构

```
pony_clean/
├── crates/pony_core/   # 业务核心库（纯 Rust，零框架依赖，含单元测试）
│   └── src/
│       ├── monitor.rs  # 进程监控：快照轮询 + kill + 内存整理
│       ├── cleaner.rs  # C盘清理：jwalk 遍历 + 安全分级 + 删除执行
│       ├── disk.rs     # 磁盘分析：大文件扫描 + 目录空间占用
│       ├── memory.rs   # 内存整理（EmptyWorkingSet）
│       ├── startup.rs  # 开机自启动管理
│       ├── icon.rs     # 进程图标提取（Windows）
│       └── error.rs    # 统一错误类型
├── src-tauri/          # Tauri 壳层（命令层 + 窗口/托盘/毛玻璃）
│   └── src/commands/   # Tauri 命令层（monitor / cleaner / disk / config / startup / window）
├── frontend/           # Vue 3 前端（Views：Monitor / Space / Settings / Startup）
├── scripts/            # 版本管理脚本（bump / check + 契约测试）
├── docs/               # 项目文档
├── CHANGELOG.md        # 版本变更记录
├── 00_DASHBOARD.md     # 任务总控面板
├── 01_TASK_BOARD.md    # 任务板
└── 03_TASKS/           # 任务卡
```

## 文档

- [文档索引](docs/README.md)
- [架构文档](docs/ARCHITECTURE.md) — 模块依赖、数据流图
- [设计决策](docs/DESIGN.md) — ADR 技术选型记录
- [C盘清理策略](docs/CLEAN_STRATEGY.md) — 60+ 清理目标 + 安全策略
