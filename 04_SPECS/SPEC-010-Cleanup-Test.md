# SPEC-010: 集成测试 + 旧代码清理 + ADR 更新

## 1. 目标

完成 Tauri 版本端到端验证，清理 egui 旧代码，更新架构文档和 ADR。

## 2. 旧代码清理顺序（前置条件: TASK-008/009 全部通过）

### Step 1: 根 Cargo.toml 转为纯 workspace
```toml
[workspace]
resolver = "2"
members = [
    "crates/pony_core",
    "src-tauri",
]
```
移除 `[package]`、`[profile]`、旧 `[dependencies]`（egui/eframe/wgpu）。
构建配置移至 `crates/pony_core/Cargo.toml` 和 `src-tauri/Cargo.toml`。

### Step 2: 删除旧文件
- `src/app.rs`（egui App, 1086 行）
- `src/theme.rs`（设计系统, 被 CSS Variables 替代）
- `src/monitor.rs`、`src/cleaner.rs`、`src/error.rs`（已迁至 pony_core）

### Step 3: 清理 src/lib.rs 和 src/main.rs
- `src/lib.rs` → 删除（旧 lib 入口，不再需要）
- `src/main.rs` → 删除（旧 eframe 入口，不再需要）

### Step 4: 删除旧目录 src/
整个 `src/` 目录已无保留价值。

### Step 5: 重新生成 Cargo.lock
```bash
cargo generate-lockfile
```

## 3. E2E 验证清单

### 3.1 监控功能

| # | 测试项 | 预期 |
|---|---|---|
| M1 | 应用启动 | Tauri 窗口弹出，无边框透明，置顶 |
| M2 | 首次加载 | 显示 `<Skeleton>` 加载态 → 2s 内显示数据 |
| M3 | 摘要行颜色 | CPU≥80% 红色, 50-80% 琥珀, <50% 正常色 |
| M4 | 内存颜色 | mem≥85% 红色, 65-85% 琥珀, <65% 正常色 |
| M5 | 进程列表 | 默认仅显示 CPU>10% 或 MEM>200MB |
| M6 | 列排序 | 点击 Name/CPU%/Mem 列头排序，▲▼ 指示器正确 |
| M7 | 搜索过滤 | 输入搜索词实时过滤，搜索时取消阈值限制 |
| M8 | 搜索无结果 | 显示 "没有进程匹配" |
| M9 | Kill 确认弹窗 | 点击 × → AlertDialog 确认 |
| M10 | Kill 成功 | 反馈 "进程已终止" |
| M11 | Kill 失败(权限) | 反馈错误消息 |
| M12 | 数据自动刷新 | 每 2s 更新，列表不闪烁 |

### 3.2 清理功能

| # | 测试项 | 预期 |
|---|---|---|
| C1 | 空闲态 | 居中描述 + 蓝色 "开始扫描" 按钮 |
| C2 | 开始扫描 | 点击 → 进度条 + 文件数递增 |
| C3 | 扫描进度事件 | Event 实时更新，scanned 递增 |
| C4 | 取消扫描 | 进度停止 → 显示已取消 + 重新扫描按钮 |
| C5 | 扫描错误 | 状态变为 error → 红色消息 + 重试 |
| C6 | 扫描完成(有数据) | 总计 + 分类图例 + 折叠列表 |
| C7 | 扫描完成(空结果) | ✓ + "没有发现可清理文件" |
| C8 | 分类折叠展开 | 默认展开，点击折叠/展开 |
| C9 | 分类全选 checkbox | 勾选分类 checkbox → 该分类所有项选中 |
| C10 | 分类半选态 | 分类部分选中时 checkbox 显示 indeterminate |
| C11 | 全局全选 | 底部 "全选" 切换 |
| C12 | 删除确认弹窗 | 显示文件数和大小 |
| C13 | 删除执行 | 显示 "删除中..." → 结果反馈 |
| C14 | 删除结果(全成功) | Alert success "成功删除 N 个文件" |
| C15 | 删除结果(有失败) | Alert destructive + 失败列表 |
| C16 | 删除中状态 | 显示不确定进度条 |

### 3.3 跨面板测试

| # | 测试项 | 预期 |
|---|---|---|
| X1 | Tab 切换状态保持 | 切到清理再切回监控，进程列表仍正常刷新 |
| X2 | 窗口最小尺寸 | 380×500 布局不变形 |
| X3 | 深色主题一致性 | 所有面板使用同一色板 |

## 4. 文档更新

### 4.1 docs/ARCHITECTURE.md
更新模块依赖图和数据流（见 MIGRATION_PLAN.md 架构对比图）。

### 4.2 docs/DESIGN.md
新增 ADR-007：
```markdown
## ADR-007: egui → Tauri v2 + Vue 3 + shadcn-vue 迁移

**状态**: 已采纳

**上下文**: egui UI 表现力无法满足产品级需求。

**方案对比**:
| 维度 | egui + eframe | Tauri v2 + shadcn-vue |
|---|---|---|
| 组件库 | 无 | shadcn-vue (30+ 组件) |
| 字体渲染 | ab_glyph 软件渲染 | DirectWrite 原生 ClearType |
| 动画 | 无 | CSS + motion-vue |
| 开发效率 | 改 UI → 改 Rust → 编译 | HMR 热更新 |
| 运行时内存 | ~35MB | ~42MB (含 WebView2) |
| 二进制体积 | ~5MB (单二进制) | ~4.5MB (不含 WebView2 runtime, 系统自带) |

**迁移策略**: pony_core 零改动，前后端通过 Tauri IPC 通信。
```

### 4.3 AGENTS.md 命令更新
```markdown
| `cargo tauri dev` | 开发运行（前端 HMR + Rust） |
| `cargo tauri build` | 打包 release 二进制 |
| `cargo build -p pony_core` | 编译业务核心 |
| `cargo test -p pony_core` | 单元测试 29 项 |
| `cargo clippy -p pony_core` | lint |
| `cd frontend && npm run dev` | 仅前端开发 |
```

## 5. 测试通过条件

```bash
cargo build -p pony_core        # 编译通过
cargo test -p pony_core         # 29 项全通过
cargo clippy -p pony_core       # 0 warnings
cargo fmt --check               # 格式正确
cargo tauri build               # Release 打包成功

# E2E: M1-M12 + C1-C16 + X1-X3 全部手动验证通过
```
