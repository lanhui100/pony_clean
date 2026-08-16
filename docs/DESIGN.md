# 设计决策记录

所有重大技术决策以 ADR（Architecture Decision Record）格式记录。

---

## ADR-001: 选择 egui 而非 Webview

**状态**: 已废弃（被 ADR-007 替代）

**上下文**: 需要一个 Windows 桌面悬浮窗，要求启动快、内存低、单二进制分发。

**方案对比**:
| 方案 | 启动时间 | 二进制大小 | 内存 | 透明窗口 |
|---|---|---|---|---|
| egui + eframe | <1s | ~5MB | ~10MB | 原生支持 |
| Tauri + Webview | 1-3s | ~10MB | ~50MB+ | 需 hack |
| Electron | 3-5s | ~150MB | ~100MB+ | 支持但笨重 |

**决策**: 最初选 egui。后因 UI 表现力瓶颈迁移至 Tauri v2（参见 ADR-007）。

---

## ADR-002: lib.rs 作为业务入口

**状态**: 已采纳

**上下文**: 需要让业务逻辑可被单元测试直接 import，且为未来 CLI 模式预留扩展点。

**决策**: `crates/pony_core/src/lib.rs` 统一 re-export 所有业务模块（`pub mod error; pub mod monitor; pub mod cleaner;`），`src-tauri/src/main.rs` 仅做 Tauri 初始化。

**理由**:
1. 业务逻辑与 GUI 解耦，可直接被测试 import
2. Tauri 入口保持薄层，只负责命令注册
3. 未来扩展 CLI 模式时只需新增一个 binary crate

---

## ADR-003: std::sync::mpsc 用于后台→UI 通信

**状态**: 已采纳（egui 内部）→ 被 Tauri IPC 替代

**上下文**: 后台 tokio 任务需要将进程数据和扫描进度推送到 GUI 线程（egui 时代）。

**决策**: 采用 `std::sync::mpsc` + `spawn_blocking`。Tauri 迁移后将前端通信层替换为 `tauri::command` + `AppHandle::emit`，mpsc 仅保留在 `pony_core` 内部用于后台线程间通信。

---

## ADR-004: 安全清理路径分级策略

**状态**: 已采纳

**上下文**: C盘清理需要区分可安全删除、需确认、禁止删除的路径。

**决策**: 三级分级制度。

| 级别 | 标签 | UI 行为 | 示例 |
|---|---|---|---|
| Safe | 🟢 | 默认勾选，一键清理 | `%TEMP%`, Prefetch, 浏览器 Cache |
| Confirm | 🟡 | 展示但不勾选，用户手动确认 | Downloads >90天未访问文件 |
| Forbidden | 🔴 | 不在 UI 显示，跳过 | `System32`, `Installer`, `ProgramData` |

**理由**: 无法预期所有用户的文件使用习惯，分级让用户有选择权同时保护系统安全。

---

## ADR-005: 删除策略 — 永久删除不走回收站

**状态**: 已采纳

**上下文**: 用户主动执行 C盘清理时，已明确意图是释放空间。

**决策**: 使用 `MoveFileExW + MOVEFILE_DELAY_UNTIL_REBOOT` 绕过占用锁，永久删除。回收站清空使用 `SHEmptyRecycleBinW`。

**理由**: 清理工具的目的是释放空间，走回收站违背用户意图。延迟删除机制可以绕过当前被占用的文件。

---

## ADR-007: egui → Tauri v2 + Vue 3 + shadcn-vue 迁移

**状态**: 已完成

**上下文**: egui UI 表现力无法满足产品级需求（无组件库、字体渲染差、无动画）。

**方案对比**:
| 维度 | egui + eframe | Tauri v2 + shadcn-vue |
|---|---|---|
| 组件库 | 无 | shadcn-vue (30+ 组件) |
| 字体渲染 | ab_glyph 软件渲染 | DirectWrite 原生 ClearType |
| 动画 | 无 | CSS + motion-vue |
| 开发效率 | 改 UI → 改 Rust → 编译 | HMR 热更新 |
| 运行时内存 | ~35MB | ~42MB (含 WebView2) |
| 二进制体积 | ~5MB (单二进制) | ~4.5MB (不含 WebView2 runtime) |

**迁移策略**: `crates/pony_core` 零改动，前后端通过 Tauri IPC 通信。

---

## ADR-008: C盘清理策略 v2 — 基于社区调研的目标扩展

**状态**: 已采纳

**上下文**: v1 仅有 15 个硬编码扫描目标，与行业头部项目（BleachBit 1000+、FluentCleaner winapp2.ini）覆盖差距显著。

**决策**: 采用三阶段渐进式扩展策略，经 3 路对抗审查（安全/架构/工程）调优后定稿。

**审查采纳的主要变更**:
- **移除清理目标**: winevt\Logs, catroot2, spool\drivers 从 Confirm 升至 PROTECTED；Videos 移除；SleepStudy 从 target 移除
- **降级**: spool\printers 从 Safe 降为 Confirm
- **移除特性**: 安全擦除模式（SSD 无意义）、winapp2.ini 解析器（80h 成本不匹配）
- **推迟到 v3**: CLI + Task Scheduler、大文件扫描
- **分类合并**: logs + error_reports 合并为 logs（6 类替代原 9 类）
- **安全加固**: `is_path_allowed` 增加分隔符边界检查；PROTECTED_PREFIXES 从 12 增至 20+；环境变量注入防御
- **审计**: 删除前快照改为 DPAPI 加密操作日志，永久保留
- **实现约束**: 按分类最小阈值（cache:512B, temp:1024B, logs:4096B）、每 target 上限 50K 项
- **工时重估**: S1 从 2h 重估为 8h，总计从 24h 重估为 25h
- **添加 P0 门禁**: Phase 2 必须在全部 P0 项验收关闭后启动

| 阶段 | 目标数 | 覆盖范围 | 目标 |
|------|:------:|---------|------|
| Phase 1 (P0) | 38 | Windows 系统垃圾全覆盖（日志/错误报告/缩略图/Update/打印机/UWP） | 对标 Disk Cleanup |
| Phase 2 (P1) | 22 | 应用层（浏览器扩展 + 开发工具 + 通信/媒体应用） | 对标 BleachBit 应用层 |
| Phase 3 (P1-P2) | - | 大文件扫描 + 磁盘分析 + 重复文件 | 差异化能力 |

**删除策略升级**:
- 保留 v1 三层降级（DeleteFileW → MoveFileExW 延迟 → Skip）
- 新增安全擦除模式（logs / error_reports 类别可选 1-pass 覆写）
- 新增删除前审计快照（路径 + 大小写入 JSON 日志，保留 30 天）
- 新增 Windows.old 检测（DISM 清理，Confirm 级别）

**安全补充**:
- 新增 9 条受保护路径（Recovery, System Volume Information, CSC, Registration, SAM config 等）
- 移除现有 `SleepStudy` 双向保护（只允许清理过期文件，不允许删除目录本身）

**类别体系**:
- 从 4 类扩展为 9 类（新增：logs, error_reports, dev_cache, update_cache, old_install, analysis）
- 每类定义默认勾选行为、安全擦除可用性、UI 颜色和图标

**不纳入**:
- 注册表清理（风险 > 收益）
- 系统还原点管理（需管理员提权）
- 后台常驻自动清理（违背"按需启动"定位）
- 浏览器扩展 / 驱动管理 / 启动项管理（超出清理职责）

**详细方案**: 参见 `docs/CLEAN_STRATEGY.md`

**理由**:
1. 社区调研（BleachBit, burnbytes, FluentCleaner, Cleanmgr+）一致证明：清理工具的覆盖率直接影响用户感知价值
2. 系统级清理安全性高（Windows 自身 API 确认可删），值得优先补齐
3. 分阶段实施控制风险，每阶段均可独立测试和发布
4. 保留 v1 安全架构不变（三级分级 + 受保护路径 + 后端强制执行），新目标仅在安全框架内扩展

---

## ADR-009: 胶囊窗口顶部贴边 + 无操作自动收起为贴边进度条

**状态**: 已采纳（原“四边贴边”已修订为“仅顶部贴边”）

**上下文**: 胶囊窗口此前只能贴顶边、仅可横向拖动；曾扩展为四边贴边（左右/底部 + 竖排内容），
但因左右贴边体验不理想，按需求**收回为仅顶部贴边**；保留无操作自动收起为贴边进度条、
以及顶部横向拖动能力。

**决策**:
1. **形态状态机**：`pill`（胶囊，显示 CPU/MEM 数字）⇄ `bar`（贴边细进度条）⇄ `island`（展开面板）。
   - 胶囊 10s 无操作自动收起为进度条（`barTimeout`，扫描中/悬停/拖动时不收起）
   - 进度条 hover 500ms 展开回胶囊，点击直接展开面板
2. **仅顶部贴边**：胶囊只能贴在屏幕顶边，沿顶边水平拖动（Y 恒为工作区顶边）；
   左右/底部贴边及竖排内容已按需求移除（Rust 枚举保留 left/right/bottom 仅为扩展预留）。
3. **贴边定位使用工作区**（`get_monitor_work_area`，GetMonitorInfoW rc_work）而非完整显示器，
   避免胶囊/面板被任务栏遮挡。
4. **原生区域随形态动态变化**：`set_capsule_geometry(form, edge)` 将形态/贴边方向写入窗口属性，
   WM_NCHITTEST 与原生命中区域（SetWindowRgn）按内容矩形实时计算——胶囊为居中 160×40，
   进度条为贴边 10px 细条，其余区域点击穿透。
5. **形态变换**：两层 motion.div（pill 层 + bar 层）以 transform-origin: 0 0 的 x/y + scaleX/scaleY
   精确映射两矩形，纯 CSS 弹簧动画。
6. **贴边位置持久化**：胶囊沿顶边的水平偏移写入 localStorage，重启后恢复。
7. **island 始终从胶囊正下方展开**：滑入方向固定为顶部；展开定位在胶囊水平居中，
   宽度钳制在工作区内。
8. **托盘“重置胶囊位置”**：菜单项触发前端重置到顶边居中，避免胶囊“跑丢”后找不到。

**实现要点**:
- 前端：`lib/windowMorphConfig.ts`（几何配置 + contentRectFor）、`components/EdgeBar.vue`（新增）、
  `composables/useWindowMorph.ts`（状态机 + 顶边拖动 + 定时收起）、`CapsuleWindow.vue`（双层 morph）
- Rust：`commands/window.rs` 新增 `set_capsule_geometry` / `get_monitor_work_area` / `log_frontend`，
  重写胶囊区域计算（`capsule_content_phys` + `apply_capsule_region`）

**理由**:
1. 窗口尺寸固定，形态切换可用纯 CSS 变换完成，动画平滑无闪跳
2. 命中区域精确对应可见内容，胶囊周围透明区可点击穿透，不干扰桌面操作
3. 工作区定位保证顶边可用（含任务栏遮挡场景）
4. 持久化水平位置符合“小组件”使用预期；托盘重置提供找回入口

---

## ADR-010: 清理与分析合并为单一"清理"tab（流程化，方案 B）

**状态**: 已采纳

**上下文**: 展开态原有 4 个 tab（监控/清理/分析/设置）。"清理"与"分析"存在产品定位割裂：
- 清理 tab 本身已是"分析后清理"（扫描 → 分类展示 → 勾选 → 删除），与分析 tab 是流水线的前后两步，却被拆成并列入口；
- "大文件"功能重复：cleaner 的 `large_files` 分类（%TEMP%/Downloads/回收站中的 ≥50MB 文件）与 disk 模块的 `scan_large_files`（全用户目录）是同一需求的两种实现，同一批文件两个入口都能删。

**决策**:
1. **去重（Rust）**：`pony_core::cleaner` 移除 `Category::LargeFiles` 变体及 5 个对应 ScanTarget（large_temp/large_local_temp/large_downloads/large_installers/large_recycle_bin），目标数 60 → 55。大文件统一由 `pony_core::disk` 负责（全用户目录覆盖更全面，删除走受保护路径检查 + 审计日志）。
2. **合并（前端）**：删除 `CleanerPanel.vue` 与 `AnalysisPanel.vue`，新建 `SpacePanel.vue` 作为唯一"清理"tab，按方案 B 流程化组织：
   - 一次"开始扫描"并行产出三份结果：可清理垃圾（8 类）/ 大文件（阈值可选）/ 目录占用 Top；
   - 任一区块扫描完成即可直接勾选清理/删除，无需等待全部完成；
   - 顶部显示磁盘概况 + 三区块扫描状态标记 + 取消按钮。
3. **后端并行支撑**：`commands/disk.rs` 将大文件与目录扫描拆分为独立锁 + 独立事件通道（`disk-large-*` / `disk-dir-*`），两个扫描可并行运行，互不阻塞。

**理由**:
1. 符合"清理 = 分析后清理"的用户心智：发现问题与处理问题同页完成，工作流不割裂
2. 消除重复实现，避免同一批大文件双入口可删的歧义
3. tab 数 4 → 3，更符合"极简小组件"定位
4. 工程成本低：`cleaner.rs` / `disk.rs` 仍为独立纯业务模块，仅命令层事件通道拆分 + UI 层聚合

**影响**:
- `crates/pony_core/src/cleaner.rs`：Category 枚举 / 目标列表 / 相关测试
- `src-tauri/src/commands/disk.rs` + `src-tauri/src/main.rs`：DiskState 字段与初始化
- `frontend/src/composables/useDisk.ts`：状态拆分为 large* / dir* 两组
- `frontend/src/views/SpacePanel.vue`（新增）、`TitleBar.vue`、`IslandWindow.vue`

---

## ADR-011: 清理体验优化五项（占用检查 / 一键清理 / WU 缓存 / 扫描合并 / 并行提速）

**状态**: 已采纳并实现（TASK-023 ~ TASK-027）

**上下文**: 基于清理策略清单 P0/P1 项，经任务系统拆分 + spec 对抗审查（降级独立 pass）后落地。

**决策**:
1. **删除前进程占用检查（TASK-023）**：`is_file_busy` 用 CreateFileW 请求 DELETE 权限探测（share=0），共享/锁冲突判定占用；cleaner 路径占用文件走延迟删除并标注原因，disk 路径报"被占用"跳过。TOCTOU 竞态为已知残余风险。
2. **一键清理 Safe 级 + 释放量反馈（TASK-024）**：确认弹窗数据源改为独立 `pendingClean` 集合（不污染用户勾选态）；一键清理仅覆盖 Safe 级；toast 展示"释放约 X"（清理前快照近似值）。
3. **Windows Update 缓存 + DataStore（TASK-025）**：`wu_download` Confirm→Safe；`wu_datastore` Forbidden→Confirm + `with_service_stop("wuauserv")` + glob 仅清文件；实现 SCM API 服务控制（新增 `Win32_System_Services` + `Win32_Security` feature），停止失败则跳过对应路径，删除后恢复服务（Drop guard 语义）。
4. **disk 大文件 + 目录占用合并单遍历（TASK-026）**：`scan_user_dir` 一趟 walk 双产出；命令层合并 `start_user_scan`（单锁 + `disk-user-*` 事件）；等价性测试保证与旧双函数一致。
5. **cleaner 并行扫描提速（TASK-027）**：target 按 `SCAN_PARALLELISM=4` 分组线程并行，`scan_target_block` 提取原逻辑，全局 AtomicU64 计数 + join/panic 处理，事件顺序对前端无依赖。

**理由**:
1. 防误删（占用）、降低操作成本（一键）、覆盖高收益目标（WU）、消除重复遍历（合并）、缩短等待（并行）
2. 保持安全底线：一键清理仍走确认弹窗；DataStore 服务控制失败即跳过
3. 前后端改动均通过全量测试/类型/构建门禁（82+6 测试、clippy 0、fmt、vue-tsc、build）

**影响**:
- `crates/pony_core/src/cleaner.rs`（is_file_busy / 服务控制 / wu target / 并行扫描）
- `crates/pony_core/src/disk.rs`（scan_user_dir / 占用检查集成）
- `src-tauri/src/commands/disk.rs` + `main.rs`（start_user_scan 合并）
- `frontend/src/composables/useDisk.ts`、`frontend/src/views/SpacePanel.vue`

**留待手动 QA**: 真实服务停止/恢复、运行中进程文件占用提示、并行扫描性能观感。

---

## ADR-012: 版本管理 — 三处清单同步 + 脚本唯一变更点 + CI 强制一致

**状态**: 已采纳（TASK-030）

**上下文**: 版本 `0.1.0` 手工散落三处清单（workspace `Cargo.toml` / `frontend/package.json` / `src-tauri/tauri.conf.json`），无 changelog、无版本 tag、无 bump 工具、CI 不校验一致性，发版靠手工多文件编辑且易漂移。

**方案对比**:

| 方案 | 说明 | 结论 |
|---|---|---|
| A: 三处同步 + bump/check 脚本 + CI 校验 | 单一变更点，纯 Node stdlib 零依赖，文本精改保留字节/行尾 | ✅ 采纳 |
| B: beforeBuildCommand 从 package.json 注入版本 | 构建期耦合；Cargo/Rust 侧仍需版本，维护面未减少 | 不采纳 |
| C: 单一来源 Cargo.toml，打包时读取 | Tauri v2 不支持直接引用清单版本，仍需注入 | 不采纳 |
| D: release-plz / cargo-bump 等工具 | 引入外部工具链与远端集成；发版频率低，脚本已足够 | 不采纳 |

**决策**:
1. `scripts/bump-version.mjs` 为**唯一版本变更点**：semver 校验 → 三处清单正则精改（`[workspace.package]` 段 / JSON 顶层 `"version"` 行）→ `cargo update -p pony_core -p pony_clean -w` 刷新 Cargo.lock（限定成员最小 diff，隔离实验证实 `cargo metadata --no-deps` 不会刷新 lock；lock 与清单不一致时可能联网刷新索引）→ **回读断言**成员版本 → CHANGELOG `[Unreleased]` 归档（缺失/多个/空节均拒绝发版）。
2. **事务与幂等**：写文件 / cargo update / 断言任一失败自动还原 5 个版本文件（不留半态）；版本已是目标版本时 `--commit`/`--tag` 跳过写文件直接收尾（支持"QA 修复后补提交、分步打 tag"两种真实流程）。
3. `--commit` / `--tag` 守卫：提交白名单 = 5 个版本文件（其余改动/未跟踪文件一律中止，守卫前置到写文件之前），`git commit -- <5 文件>` 限定提交范围；提交体带本次发布条目；tag 前校验 HEAD 已含目标版本 + 同名 tag 不存在。
4. `scripts/check-version.mjs` 校验**四处**（三清单 + Cargo.lock 成员条目，成员名从各成员 Cargo.toml 的 `[package] name` 解析，不硬编码）；CI `version-sync` job（windows-latest）在 push/PR 强制校验并运行 `node --test` 契约测试。
5. 纯函数抽离 `scripts/version-utils.mjs`，`node --test` 契约测试覆盖版本校验/JSON 精改/lock 提取/changelog 归档（含节边界、CRLF、`-pre` 版本、回滚契约等 28 项）。

**理由**:
1. 零新依赖（Node 内置测试器）、跨平台（Windows 开发机 + CI）、可回归（契约测试 + CI 门禁）
2. 文本精改保留字节与行尾（CRLF 安全），git diff 最小化可审
3. 守卫集中在发版动作时点（空 changelog 拦截在 bump，而非阻塞日常 push 的 CI 检查）

**影响**:
- 新增 `scripts/bump-version.mjs` / `scripts/check-version.mjs` / `scripts/version-utils.mjs` / `scripts/version-utils.test.mjs`、`CHANGELOG.md`、`docs/VERSIONING.md`
- `.github/workflows/ci.yml`（version-sync job）、根 `package.json`（check:version / bump:version / test:version）
- `docs/README.md`、`RELEASE_CHECKLIST.md`、`README.md`、`AGENTS.md`（流程/命令文档化）

**流程**: 发版 = `check-version` → 确认 CHANGELOG [Unreleased] → `bump <v>` → 构建 + 手动 QA → `bump <v> --commit --tag` → `git push --follow-tags`。详见 `docs/VERSIONING.md`。
