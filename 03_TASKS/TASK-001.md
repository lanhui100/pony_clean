# TASK-001 完整项目脚手架搭建

## Basic Info
- ID: TASK-001
- 状态: Done
- 优先级: P0
- 负责人: @self
- 创建日期: 2026-06-27
- 更新日期: 2026-06-27
- 预估工时: 6h
- 实际工时: ~4h

## Goal
搭建完整的工程环境，不仅是「能跑」，还要「能测、能 lint、能 CI、能快速迭代」。核心依赖配置、模块结构、测试框架、代码规范、开发工作流一步到位。

## Output

| 文件 | 说明 |
|---|---|
| `Cargo.toml` | 完整依赖 + dev-depends + profile 优化 |
| `src/lib.rs` | 库入口（不依赖 GUI），业务模块统一 re-export |
| `src/main.rs` | 薄 binary 层，仅初始化 tokio + eframe |
| `src/app.rs` | egui App 骨架 |
| `src/monitor.rs` | 模块占位 + 单元测试 |
| `src/cleaner.rs` | 模块占位 + 单元测试 |
| `src/error.rs` | 统一错误类型 |
| `tests/common/mod.rs` | 测试辅助函数 |
| `tests/integration_monitor.rs` | 集成测试示例 |
| `.rustfmt.toml` | 代码格式规范 |
| `.vscode/settings.json` | VS Code 推荐设置 + Rust Analyzer |
| `.vscode/extensions.json` | 推荐插件列表 |
| `.github/workflows/ci.yml` | CI：fmt + clippy + build + test + deny |
| `scripts/dev.ps1` | Windows 开发启动脚本 |
| `justfile` | 常用命令入口（just build/test/lint/run） |
| `.gitignore` | 完整忽略规则（Rust + IDE + OS + 产物） |
| `.gitattributes` | Git 属性（LF 归一化、语言标注） |
| `docs/ARCHITECTURE.md` | 架构文档（占位，TASK-002/003 完成后更新） |
| `docs/DESIGN.md` | 设计决策记录（占位，TASK-002/003 完成后更新） |
| `docs/README.md` | 文档入口索引 |

## 验收标准
1. `cargo build` 通过
2. `cargo test` 通过（含单元测试 + 集成测试）
3. `cargo clippy` 无 warning
4. `cargo fmt --check` 通过
5. `cargo run` 弹出半透明 egui 窗口 ⚠️ manual（CI 无桌面环境，需人肉验证）
6. `just build` / `just test` / `just lint` / `just run` 都可用
7. CI 配置完整，能在 GitHub Actions 中跑通 build + test + clippy + fmt
8. `.gitignore` 覆盖 Rust 产物、IDE 目录、OS 文件
9. `.gitattributes` 配置正确，LF 归一化
10. 本地分支名为 `main`，`git status` 干净，可 `git push` 到 GitHub
11. `docs/` 目录包含架构图（模块关系 + 数据流）和设计决策记录
12. 源代码关键模块（monitor、cleaner、error）有 `///` doc 注释

## 具体方案

### Cargo.toml

```toml
[package]
name = "pony_clean"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
description = "Windows 极简桌面小组件 — 进程监控报警 + C盘安全清理"
authors = ["pony"]

[profile.release]
opt-level = 2
lto = "thin"
codegen-units = 1
strip = "symbols"
panic = "abort"     # 权衡：catch_unwind 失效，见 DESIGN.md ADR-006

[profile.dev]
panic = "unwind"
opt-level = 1       # 比默认 0 快，保留调试信息

[dependencies]
egui = "0.27"
eframe = { version = "0.27", default-features = false, features = ["wgpu", "native"] }
tokio = { version = "1", features = ["full"] }
sysinfo = "0.30"
jwalk = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
thiserror = "2"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.54", features = [
    "Win32_Foundation",
    "Win32_Storage_FileSystem",
    "Win32_UI_Shell",
    "Win32_System_Threading",
    "Win32_System_Com",
] }

# dev-dependencies 按需引入，勿预装
```

### 目录结构

```
src/
├── lib.rs           # pub mod 统一导出，不含 GUI
├── main.rs          # 薄层：tokio runtime + eframe
├── app.rs           # PonyCleanApp (egui::App)
├── monitor.rs       # 进程监控模块
├── cleaner.rs       # C盘清理模块
└── error.rs         # 统一错误类型
tests/
├── common/
│   └── mod.rs       # 测试辅助函数
├── integration_monitor.rs
└── integration_cleaner.rs
```

### lib.rs — 以 lib 为中心

```rust
pub mod error;
pub mod monitor;
pub mod cleaner;
```

这样 `src/main.rs` 只做一件事：

```rust
use pony_clean::app::PonyCleanApp;

fn main() -> eframe::Result<()> {
    // ... 初始化逻辑
}
```

而所有业务逻辑在 lib 中，可以被单元测试直接 import，不依赖 GUI。

### main.rs — 薄 binary 层

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pony_clean::app::PonyCleanApp;
use tracing_subscriber::EnvFilter;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let rt = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime");
    let _enter = rt.enter();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 600.0])
            .with_always_on_top()
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "PonyClean",
        options,
        Box::new(|_cc| Ok(Box::new(PonyCleanApp::new(rt)))),
    )
}
```

`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` — release 模式下不显示控制台窗口。

### app.rs — 骨架

```rust
use eframe::egui;

pub struct PonyCleanApp {
    pub rt: tokio::runtime::Runtime,
}

impl PonyCleanApp {
    pub fn new(rt: tokio::runtime::Runtime) -> Self {
        Self { rt }
    }
}

impl eframe::App for PonyCleanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                ui.heading("PonyClean");
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}
```

### error.rs

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PonyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Cleanup error: {0}")]
    Cleanup(String),
}

pub type Result<T> = std::result::Result<T, PonyError>;
```

### monitor.rs — 含单元测试

```rust
/// 进程监控模块 — 占位
pub fn placeholder() -> &'static str {
    "monitor module ready"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_returns_non_empty() {
        let result = placeholder();
        assert!(!result.is_empty(), "placeholder should return a non-empty string");
        assert!(
            result.contains("monitor"),
            "placeholder should mention 'monitor', got: {result}"
        );
    }
}
```

### cleaner.rs — 含单元测试

```rust
/// C盘清理模块 — 占位
pub fn placeholder() -> &'static str {
    "cleaner module ready"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_returns_non_empty() {
        let result = placeholder();
        assert!(!result.is_empty(), "placeholder should return a non-empty string");
        assert!(
            result.contains("cleaner"),
            "placeholder should mention 'cleaner', got: {result}"
        );
    }
}
```

### .rustfmt.toml

```toml
max_width = 100
tab_spaces = 4
edition = "2024"
use_small_heuristics = "Default"
```

### .gitignore

```gitignore
# Rust
/target
**/*.rs.bk
Cargo.lock

# IDE
.idea/
*.iml
/.vscode/*
!/.vscode/settings.json
!/.vscode/extensions.json
*.swp
*.swo

# OS
Thumbs.db
.DS_Store
Desktop.ini

# Build artifacts
*.exe
*.pdb
*.ilk
*.exp
*.lib
*.obj
```

### .gitattributes

```gitattributes
* text=auto eol=lf

*.rs text diff=rust
*.toml text
*.md text
*.json text
*.yaml text
*.yml text
*.ps1 text
*.just text

*.png binary
*.ico binary
*.jpg binary
```

### 本地文档系统

#### docs/README.md — 文档索引

```markdown
# PonyClean 文档

## 项目文档
- [架构文档](ARCHITECTURE.md) — 模块依赖、数据流图、项目地图
- [设计决策](DESIGN.md) — 技术选型理由、约束、权衡

## 任务系统
- [总控面板](../00_DASHBOARD.md) — 当前阶段、里程碑、任务概览
- [任务板](../01_TASK_BOARD.md) — 状态流转
- [任务卡](../03_TASKS/) — 单个任务详情
```

#### docs/ARCHITECTURE.md — 架构文档

```markdown
# PonyClean 架构

## 模块依赖关系

```
main.rs  (薄入口层)
   │
app.rs   (egui::App, UI 状态, update 渲染)
   │
   ├── monitor.rs  (进程监控：sysinfo 轮询 + mpsc 推送)
   │
   ├── cleaner.rs  (C盘清理：jwalk 遍历 + 安全分级 + 删除)
   │
   └── error.rs    (统一错误类型)
```

## 数据流

```
┌──────────────┐    mpsc::channel    ┌──────────────────┐
│  Tokio Task  │ ──────────────────► │  PonyCleanApp    │
│  (monitor)   │   Vec<ProcessInfo>  │  (egui::App)     │
└──────────────┘                     │                  │
                                     │  rx.try_recv()   │
┌──────────────┐    mpsc::channel    │  每帧轮询         │
│  Tokio Task  │ ──────────────────► │  更新 self.state  │
│  (cleaner)   │   ScanProgress     │                  │
└──────────────┘                     │  ui() 渲染        │
                                     └──────────────────┘
```

## 项目地图

| 目录/文件 | 职责 |
|---|---|
| `src/main.rs` | eframe::run_native + tokio runtime |
| `src/app.rs` | egui 状态 + update 循环 |
| `src/monitor.rs` | 进程快照、阈值检测、kill |
| `src/cleaner.rs` | 路径扫描、安全分级、删除执行 |
| `src/error.rs` | PonyError 枚举 + Result 别名 |
| `tests/` | 集成测试 |
| `docs/` | 项目文档 |
```

#### docs/DESIGN.md — 设计决策记录

```markdown
# 设计决策记录

## ADR-001: 选择 egui 而非 Webview

**状态**: 已采纳

**理由**:
1. 单二进制分发，不需嵌入浏览器引擎
2. 启动快（<1s），内存低（~10MB）
3. 透明无边框悬浮窗原生支持
4. 即时模式 GUI，状态管理简单

**权衡**: 样式能力有限，不适合复杂布局

---

## ADR-002: lib.rs 作为业务入口

**状态**: 已采纳

**理由**:
1. 业务逻辑与 GUI 解耦，可直接被测试 import
2. main.rs 保持薄层，只负责初始化
3. 未来扩展 CLI 模式时只需换一个 binary 入口

---

## ADR-003: 用 std::sync::mpsc 而非 tokio::sync::mpsc 用于 UI 通信

**状态**: 待定 (TASK-004 确定)

**理由**:
- `std::sync::mpsc` 更简单，Receiver 是 `Send + !Sync`，适合单消费者
- `tokio::sync::mpsc` 需要 tokio 上下文，GUI 线程没有 tokio runtime
- 但在某些流式场景下 `tokio::sync::watch` 更方便

**决定**: 待 TASK-004 时根据实际数据流模式选择

---

## ADR-006: Release profile 使用 panic=abort

**状态**: 已采纳

**上下文**: 桌面工具二进制应尽量小，且异常应快速失败而非传播。

**决策**: Release profile 启用 `panic = "abort"`，dev profile 保留 `panic = "unwind"`。

**权衡**:
- `std::panic::catch_unwind` 在 release 下无效，任务 panic 会直接终止进程
- tokio 任务 panic 将导致整个进程退出（而非单个任务重启）
- abort 模式下 backtrace 信息不完整

**备注**: 如果后续发现 abort 导致关键数据丢失，可考虑回退到 unwind + 全局 panic hook。
```

#### 源码 doc 注释要求

每个公开函数和类型必须包含 `///` doc 注释，例如：

```rust
/// 进程监控模块
///
/// 每 2s 轮询系统进程，检测 CPU/内存异常超高的进程，
/// 通过 mpsc channel 向 UI 推送快照。
pub fn start(
    system: System,
    tx: mpsc::Sender<Vec<ProcessInfo>>,
) -> mpsc::Sender<MonitorCommand> { ... }
```

```rust
/// PonyClean 统一错误类型
#[derive(Error, Debug)]
pub enum PonyError { ... }
```

`cargo doc --no-deps` 应能正常生成文档，无 broken link。

### .vscode/settings.json

```json
{
    "rust-analyzer.check.command": "clippy",
    "rust-analyzer.cargo.allFeatures": true,
    "editor.formatOnSave": true,
    "files.exclude": {
        "target/": true
    }
}
```

### .vscode/extensions.json

```json
{
    "recommendations": [
        "rust-lang.rust-analyzer",
        "tamasfe.even-better-toml",
        "vadimcn.vscode-lldb",
        "serayuzgur.crates"
    ]
}
```

### .github/workflows/ci.yml

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.85.0
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2

      - run: cargo fmt --check
      - run: cargo build
      - run: cargo clippy -- -D clippy::all -D clippy::pedantic -A clippy::allow_attributes_without_reason
      - run: cargo test
      - run: cargo deny check
```

### Git 仓库初始化

1. **分支重命名**：`master` → `main`
   ```powershell
   git branch -m master main
   ```

2. **首次提交**
   ```powershell
   git add . && git commit -m "chore: initial project scaffold"
   ```

3. **GitHub 仓库创建**（使用 `gh` CLI）
   ```powershell
   gh repo create pony_clean --public --push --source=.
   ```
   验证：`gh repo view pony_clean` 能正常返回

4. **CI 验证**
   推送后确认 GitHub Actions 页面上的 CI workflow 全部绿色通过

### justfile

```just
set shell := ["pwsh", "-c"]
set windows-shell := ["pwsh", "-c"]

# 开发常用命令
default:
    @just --list

build:
    cargo build

run:
    cargo run

test:
    cargo test

lint:
    cargo clippy -- -D clippy::all -D clippy::pedantic -A clippy::allow_attributes_without_reason

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

clean:
    cargo clean

deny:
    cargo deny check

ci: fmt-check build lint test

release:
    cargo build --release
```

### scripts/dev.ps1

```powershell
# 开发启动脚本 — 自动 cargo run
Write-Host "🚀 PonyClean dev mode" -ForegroundColor Cyan
cargo run
```

### tests/common/mod.rs

```rust
/// 测试辅助函数
#[allow(dead_code)]
pub fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .try_init();
}
```

### tests/integration_monitor.rs

```rust
mod common;

#[test]
fn test_monitor_module_is_accessible() {
    common::init_logging();
    let msg = pony_clean::monitor::placeholder();
    assert!(!msg.is_empty());
    assert!(msg.contains("monitor"));
}
```

### tests/integration_cleaner.rs

```rust
mod common;

#[test]
fn test_cleaner_module_is_accessible() {
    common::init_logging();
    let msg = pony_clean::cleaner::placeholder();
    assert!(!msg.is_empty());
    assert!(msg.contains("cleaner"));
}
```

## 实现顺序（并行优化）

```
Step 0: git init + .gitignore + .gitattributes + 首次空提交
         （尽早设好 git，后续每步可独立提交）

Step 1 (并行):
  ├── Cargo.toml (依赖+profile)
  ├── .rustfmt.toml
  └── .vscode/ (settings + extensions)

Step 2 (并行): Rust 源码
  ├── src/error.rs
  ├── src/lib.rs
  ├── src/monitor.rs + cleaner.rs (含单元测试)
  ├── src/app.rs
  └── src/main.rs

Step 3 (并行):
  ├── tests/ (common + 集成测试)
  ├── justfile
  ├── scripts/dev.ps1
  └── .github/workflows/ci.yml

Step 4 (并行):
  ├── docs/ (README / ARCHITECTURE 占位 / DESIGN 占位)
  └── AGENTS.md

Step 5: 验证链
  cargo build && cargo test && cargo clippy && cargo fmt --check && cargo doc --no-deps

Step 6: Git
  git branch -m master main
  git add . && git commit -m "chore: initial project scaffold"
  gh repo create pony_clean --public --push --source=.
  确认 CI 绿
```

## Current Progress
- Cargo 项目已初始化（cargo new）
- 尚无任何依赖和代码

## Next Action
1. 按并行优化后的实现顺序写文件，Step 0 先配 git
2. 每步完成后 `cargo build` 及早发现问题
3. 全部写完后运行完整验证链：`cargo build && cargo test && cargo clippy && cargo fmt --check && cargo doc --no-deps`
4. Git 初始化：主分支重命名 → 提交 → `gh repo create` → 推送 → 验证 CI 绿

## Resume Hint
按并行优化顺序执行。从 Step 0（git init + .gitignore）开始，然后 Step 1 并行写 Cargo.toml、.rustfmt.toml、.vscode/。Step 2 写 src/ 源码（含含测试）。Step 3 写 tests/、justfile、dev.ps1、CI。Step 4 写 docs/ 和 AGENTS.md。最后验证链一次通过后 git 推送。

## Related Files
- `README.md`
- `00_DASHBOARD.md`
