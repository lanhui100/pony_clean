# PonyClean C 盘扫描与清理 — 综合优化方案

> 基于 3 路智能体并行分析（扫描策略 / 清理策略 / 架构集成）汇总采纳意见，形成一版可执行的优化方案。
> 优先级：**P0（必须）→ P1（建议）→ P2（锦上添花）**

---

## P0 — 高优先级（稳定性 / 安全 / 必须修复）

### P0-1：添加文件大小阈值，过滤微效文件

**现状**：当前只过滤 `size == 0` 的空文件。数万个 512B 的 `.tmp` 日志文件每个生成一个 `CleanItem`，浪费内存和 UI 渲染性能。

**方案**：在 `start_scan()` 遍历循环中新增 `MIN_CLEAN_SIZE` 常量（默认 1024 字节），小于此值的文件跳过。可扩展为按分类设置不同阈值（cache: 1KB, temp: 512B）。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — `start_scan()` 中 `size == 0` 判断后追加阈值检查

**预期收益**：减少 40-60% 的 `CleanItem` 数量，UI 加载和选择操作显著提升。

---

### P0-2：实现删除进度条反馈

**现状**：`delete_files()` 是同步阻塞的黑箱操作，UI 仅显示不确定进度条「清理中...」，用户无法感知进度。

**方案**：在 `delete_files()` 签名中增加 `Option<mpsc::Sender<DeleteProgress>>`，每删除 10 个文件推送一次进度事件。Tauri 命令层转发为 `delete-progress` 事件，前端展示 `已清理 N/M 个文件` + 确定进度条。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — `delete_files()` 新增 `tx` 参数 + `DeleteProgress` 结构体
- `src-tauri/src/commands/cleaner.rs` — `execute_clean()` 转发进度事件
- `frontend/src/composables/useCleaner.ts` — 监听 `delete-progress` 事件
- `frontend/src/views/CleanerPanel.vue` — 确定进度条替代不确定动画

**预期收益**：大量文件清理时用户可感知进度，减少焦虑和误操作（用户以为卡死而重启）。

---

### P0-3：修复前端 CleanItem 缺失 level 字段

**现状**：后端 `CleanItem` 包含 `level: SafetyLevel`，但前端 `CleanItem` 接口 (`useCleaner.ts:5-9`) 缺失此字段，导致 `Confirm` 级别的文件（系统 Temp、Prefetch）无法被前端区分，默认全部勾选。

**方案**：前端 `CleanItem` 补充 `level: string` 字段；`CleanerPanel.vue` 中 `toggleAll` 逻辑修改为不自动选中 `Confirm` 级别文件；UI 上对 `Confirm` 文件显示黄色警告标签。

**涉及修改**：
- `frontend/src/composables/useCleaner.ts` — `CleanItem` 接口加 `level: string`
- `frontend/src/views/CleanerPanel.vue` — 分类勾选逻辑、UI 级别标识

**预期收益**：消除前后端数据不一致 bug，避免用户误删系统谨慎建议文件。

---

### P0-4：扫描结果内存上限守卫

**现状**：所有扫描结果累积在 `accumulated: Vec<CleanItem>` 中，无限制。极端情况（大型缓存目录 200 万+ 文件）可导致 OOM。

**方案**：在 `start_scan()` 中设 `MAX_ITEMS`（建议 300,000），超过后发送 `ScanEvent::Warning` 并提前终止遍历。Tauri 命令层和前端转发此警告。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — 遍历循环中加截断判断
- `frontend/src/views/CleanerPanel.vue` — 展示截断警告提示

**预期收益**：防止应用在极端场景下崩溃，提升长期运行稳定性。

---

### P0-5：修复 useCleaner 事件注册竞态

**现状**：`onMounted` 中 `guardScanning` 绑定的 `invokeSeq` 始终为初始值 `0`。若组件挂载后首次 `startScan()` 异步完成但 `onMounted` 的 `Promise.all` 未 resolve，事件监听器未注册，导致事件静默丢失。

**方案**：将事件注册逻辑从 `onMounted` 改为惰性注册——`startScan()` 中先确保监听器就绪再 `invoke`，或使用 `once` + 注册完成的 Promise 栅栏。

**涉及修改**：
- `frontend/src/composables/useCleaner.ts` — 事件注册改为惰性模式

**预期收益**：消除偶发性的首次扫描事件丢失 bug。

---

### P0-6：清理操作二次确认弹窗

**现状**：点击「清理选中」直接执行永久删除，无任何确认步骤。ADR-005 已决定永久删除不走回收站，因此一旦误操作不可逆。

**方案**：当 `selectedCount > 0` 时弹出 `AlertDialog`，显示文件数、总大小、分类明细。特别地，当 `selectedBytes > 1GB` 或包含 `Confirm` 级别文件时，确认按钮使用 destructive 红色。

**涉及修改**：
- `frontend/src/views/CleanerPanel.vue` — 新增确认对话框

**预期收益**：防止误操作导致不可逆数据丢失，提升用户安全感。

---

## P1 — 中等优先级（体验提升）

### P1-1：扩展清理路径覆盖范围

**现状**：仅 8 个硬编码路径，缺少大量常见的可安全清理空间。

**方案**：新增以下扫描目标：

| 路径 | 级别 | 分类 | 说明 |
|---|---|---|---|
| `%WINDIR%\SoftwareDistribution\Download` | Confirm | temp | Windows Update 缓存 |
| `%LOCALAPPDATA%\Microsoft\Windows\INetCache` | Safe | cache | IE/Edge 旧缓存 |
| `%WINDIR%\System32\DriverStore\FileRepository` | Confirm | cache | 旧驱动备份 |
| `%LOCALAPPDATA%\Packages\*\AC\INetCache` | Safe | cache | UWP 应用缓存 |
| `%APPDATA%\..\LocalLow` | Safe | temp | 低级完整性缓存 |

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — `get_clean_targets()` 追加路径

**预期收益**：清理能力覆盖范围翻倍，用户可释放更多 C 盘空间。

---

### P1-2：添加最大扫描深度限制

**现状**：`jwalk::WalkDir::new(target)` 无 `max_depth` 设置，极深目录树（如 `node_modules`）导致遍历时间不可控。

**方案**：设置 `.max_depth(20)`，并在 `process_read_dir` 回调中对深度 > 5 的目录跳过已知打包目录（`node_modules`、`.git`、`__pycache__`、`.svn`）。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — `start_scan()` 中 walk_dir 配置追加

**预期收益**：避免因深层目录遍历导致的扫描时间异常，提升扫描稳定性。

---

### P1-3：配置持久化（tauri-plugin-store）

**现状**：用户偏好（扫描哪些类、自定义排除项）每次重启丢失。`selectedPaths` 在 Tab 切换时丢失。

**方案**：引入 `tauri-plugin-store` 持久化以下配置：
- 各 `ScanTarget` 的 `enabled` 开关状态
- 用户自定义排除路径列表
- 上次扫描时间（用于增量提示）

**涉及修改**：
- `Cargo.toml` — 添加 `tauri-plugin-store`
- `frontend/package.json` — 添加 `@tauri-apps/plugin-store`
- `crates/pony_core/src/cleaner.rs` — 新增 `load_config()` / `save_config()`
- `frontend/src/views/CleanerPanel.vue` — 配置面板 UI

**预期收益**：用户偏好持久化，体验更成熟。

---

### P1-4：Firefox 缓存路径匹配增强

**现状**：仅匹配包含 `default` 的目录名和 `cache2/entries`，忽略 `release`、`dev-edition-default` 等变体，且未覆盖 `startupCache`、`thumbnails` 等缓存目录。

**方案**：扩展匹配规则，涵盖所有 profile 命名模式（`*.default-release`、`*.default-esr`、`*.dev-edition-default`），并扫描多个缓存子目录（`cache2/entries`、`startupCache`、`thumbnails`）。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — `resolve_targets()` 中 Firefox 分支

**预期收益**：Firefox 缓存清理量提升 2-3 倍。

---

### P1-5：浏览器缓存全面覆盖（Chrome/Edge Cache）

**现状**：Chrome/Edge 仅扫描 `Code Cache`（~200MB），忽略真正的磁盘缓存 `Cache` 和 `Media Cache`（通常 1-5GB）。

**方案**：新增 Chrome/Edge 的 `Cache` 和 `CacheStorage` 目录扫描目标。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — `get_clean_targets()` 追加路径

**预期收益**：浏览器缓存清理量提升 3-5 倍，是用户最直接的释放空间感知点。

---

### P1-6：扫描时校验路径权限（提前过滤受保护路径）

**现状**：`is_path_protected` 只在删除时检查，扫描时不做。可能遍历受保护路径下的文件，浪费 I/O 并引发权限错误日志。

**方案**：在 `resolve_targets()` 或 `start_scan()` 中，对每个扫描目标做 `is_path_protected` 验证，受保护路径直接跳过。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — 扫描循环中增加保护路径过滤（当前已在 resolve_targets 中校验，但扫描时个别递归子路径可能不受限）

**预期收益**：减少不必要的 I/O 开销约 10-20%。

---

## P2 — 低优先级（锦上添花）

### P2-1：错误恢复与重试失败项

**现状**：清理失败后只能完整重扫，没有「重试失败项」功能。

**方案**：清理结果的 Toast 中加入「重试失败项」按钮，将 `DeleteResult.errors` 中的路径重新发起 `execute_clean()`。

**涉及修改**：
- `frontend/src/views/CleanerPanel.vue` — Toast 中追加操作按钮

### P2-2：键盘快捷键

**现状**：完全依赖鼠标操作。

**方案**：
- `Ctrl+Shift+S` — 开始扫描
- `Ctrl+Shift+C` — 取消扫描
- `Ctrl+Shift+X` — 清理选中
- `Escape` — 取消/关闭弹窗

**涉及修改**：
- `frontend/src/views/CleanerPanel.vue` — `onMounted` 注册 keydown 监听

### P2-3：事件协议规范化（编译时类型安全）

**现状**：Rust 侧手写 `serde_json::json!({...})` 构造事件 payload，前端监听时全靠隐式约定，无编译时检查。

**方案**：在 `crates/pony_core` 中定义类型化 payload 结构体（如 `ScanProgressPayload`、`ScanItemsPayload`），Tauri 命令层用这些结构体 `emit()`。远期可用 `typescript-type-def` crate 生成 `.d.ts`。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — 新增 payload 结构体
- `src-tauri/src/commands/cleaner.rs` — 使用结构体替换 `json!()`

### P2-4：I/O 节流机制

**现状**：扫描无速率限制，可能影响用户的其他磁盘操作。

**方案**：在扫描循环中每处理 100 个文件检测耗时，若 < 1 秒则 `std::thread::sleep(Duration::from_millis(1))` 让步。仅对 HDD 用户有明显的系统响应改善。

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — 扫描循环中追加节流

### P2-5：按大小排序和分类快捷操作

**现状**：分类列表顺序未指定，无排序/过滤功能。

**方案**：按 `totalBytes` 降序排列分类（最大类置顶）；分类标题栏加排序按钮（按大小升/降）和「全选此类」「仅选 >10MB 项」快捷操作。

**涉及修改**：
- `frontend/src/views/CleanerPanel.vue` — 分组排序 + 快捷操作 UI

### P2-6：测试覆盖增强

**现状**：`delete_files` 无单元测试，Tauri 命令层无测试，`useCleaner` 无测试。

**方案**：
- `delete_files` 单元测试：在临时目录中创建文件，验证正常删除、受保护路径拒绝、越权拒绝
- 取消路径测试：启动扫描→发送 `CancelScan`→确认 `Cancelled` 事件
- `useCleaner` 测试：mock invoke，测试全部状态转换路径

**涉及修改**：
- `crates/pony_core/src/cleaner.rs` — 追加 `mod tests` 用例
- `tests/` — 新增 Tauri 命令层集成测试
- `frontend/src/__tests__/` — 新增 composable 测试

---

## 实施路线图

| 阶段 | 内容 | 工时估算 |
|---|---|---|
| **Phase 1（本迭代）** | P0-1 文件大小阈值 + P0-3 level 字段修复 + P0-6 确认弹窗 | 1-2 天 |
| **Phase 2（下迭代）** | P0-2 删除进度 + P0-4 内存上限 + P0-5 事件竞态修复 | 2-3 天 |
| **Phase 3** | P1-1 扩展路径 + P1-2 深度限制 + P1-4/5 浏览器增强 | 2-3 天 |
| **Phase 4** | P1-3 配置持久化 + P1-6 扫描权限过滤 | 2 天 |
| **Phase 5** | P2 各项锦上添花 + 测试覆盖 | 3-4 天 |

---

## 已采纳决策汇总

| ID | 决策 | 来源 | 理由 |
|---|---|---|---|
| D1 | 不做 dry-run 预览模式，用二次确认弹窗替代 | Agent 2 提议 | 扫描本身已是预览，二次确认更轻量、更符合用户直觉 |
| D2 | 不做增量扫描缓存 | Agent 1 + 2 提议 | 复杂度高收益有限，用户通常单次扫描后即清理，不会频繁重扫 |
| D3 | 不做系统还原点创建 | Agent 2 提议 | 需要管理员提权，且与非管理员用户场景冲突；安全验证已足够 |
| D4 | 不做回收站/软删除选项 | Agent 2 提议 | 与 ADR-005 永久删除决策冲突，增加复杂度和用户困惑 |
| D5 | 不做 I/O 节流（P2-4 延后） | Agent 1 提议 | 扫描短暂（通常 < 30s），节流没有必要，且 SSD 不受影响 |
| D6 | 分批加载 / 流式传输 | Agent 1 提议 | 当前架构已是批次推送前端，不解耦的原因是前端需要全量做勾选汇总；用 P0-4 截断代替流式 |