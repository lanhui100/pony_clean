# TASK-003 C盘扫描与安全清理模块

## Basic Info
- ID: TASK-003
- 状态: Backlog
- 优先级: P0
- 负责人: @self
- 创建日期: 2026-06-27
- 更新日期: 2026-06-27
- 预估工时: 7h
- 依赖: TASK-001

## Goal
实现 C盘安全扫描与清理模块：异步并行遍历安全路径，按安全级别分级，预估可释放空间，支持执行清理操作。Windows API 调用需正确处理 COM 初始化和权限降级。

## Output
- `src/cleaner.rs` — 完整实现（含扫描 + 删除 + 回收站清空）
- `src/bin/cleaner_probe.rs` — 独立验证入口

## 验收标准
1. 扫描以下路径：`%TEMP%`、`%LOCALAPPDATA%\Temp`、`%WINDIR%\Temp`（🟡 默认不勾选）、Prefetch（🟡 默认不勾选）、Chrome/Edge/Firefox 缓存、回收站
2. 遍历使用 jwalk 并行，`follow_links(false)` 防止 junction 逃逸，`skip_permission_errors(true)` 跳过无法访问的条目
3. 按文件维度输出聚合结果（批次发送，非逐条），包含路径 + 大小 + 安全级别
4. 支持 dry-run 模式（只统计不删除）
5. 支持防重入：扫描进行中再次调用 start_scan 返回 Err
6. 支持取消扫描：通过 CancellationToken
7. 支持执行清理：DeleteFileW 主路径 → MoveFileExW 延迟删除降级 → 跳过
8. 回收站清空在已初始化的 COM 公寓上调用
9. 路径验证后端强制执行：禁止删除非扫描来源的路径
10. 进度通过 std::sync::mpsc 聚合推送，不阻塞 UI

## 接口设计

```rust
// cleaner.rs

use std::path::PathBuf;
use std::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct CleanItem {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub level: SafetyLevel,
    pub category: String,        // "temp", "cache", "prefetch", "recycle_bin"
}

#[derive(Clone, Debug, PartialEq)]
pub enum SafetyLevel {
    Safe,       // 🟢 默认勾选
    Confirm,    // 🟡 展示但不勾选
    Forbidden,  // 🔴 不在 UI 显示
}

#[derive(Clone, Debug)]
pub struct ScanProgress {
    pub scanned: u64,
    pub current: String,
}

#[derive(Clone, Debug)]
pub enum ScanEvent {
    /// 批次聚合推送（每 N 条或每扫描完一个目录发射一次），避免单文件粒度
    ItemsFound { items: Vec<CleanItem>, batch_complete: bool },
    /// 扫描完成
    Done { total_items: u64, total_bytes: u64 },
    /// 扫描取消
    Cancelled,
    /// 扫描错误（非终止性）
    Warning(String),
}

#[derive(Debug)]
pub struct DeleteResult {
    pub success: u64,
    pub failed: u64,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub enum CleanCommand {
    /// 执行删除（后端验证路径在白名单内，禁止删除系统保护路径）
    Execute(Vec<PathBuf>),
    EmptyRecycleBin,
    CancelScan,
    Shutdown,
}

/// 启动 C盘扫描任务
///
/// 返回 (cmd_tx, cancel_token)
/// - 扫描运行时再次调用返回 Err
/// - 通过 cancel_token.cancel() 取消扫描
pub fn start_scan(
    tx: mpsc::Sender<ScanEvent>,
) -> Result<(mpsc::Sender<CleanCommand>, CancellationToken), String>;

/// 删除文件（阻塞调用，需封装在 spawn_blocking 中执行）
///
/// 降级策略：
/// 1. DeleteFileW — 立即删除，无权限要求
/// 2. MoveFileExW + MOVEFILE_DELAY_UNTIL_REBOOT — 被占用时降级
/// 3. 跳过 — 标记在 errors 中
///
/// 后端强制执行路径验证：
/// - 非扫描来源路径拒绝删除
/// - 系统保护路径（System32, Installer 等）拒绝删除
pub fn delete_files(paths: &[PathBuf]) -> DeleteResult;

/// 清空回收站（需在 COM 初始化的线程上调用）
pub fn empty_recycle_bin() -> Result<(), String>;
```

## 安全路径规则

| 路径 | 级别 | 获取方式 | 说明 |
|---|---|---|---|
| `%TEMP%` | 🟢 | `std::env::var("TEMP")` | 用户临时文件，安全可删 |
| `%LOCALAPPDATA%\Temp` | 🟢 | `std::env::var("LOCALAPPDATA")` | 当前用户临时文件 |
| `%WINDIR%\Temp` | 🟡 | `ExpandEnvironmentStringsW` | 系统临时文件，Windows Update 可能在使用，默认不勾选 |
| Prefetch | 🟡 | `{SystemRoot}\Prefetch` | 清空后首次冷启动变慢，默认不勾选 |
| Chrome Cache | 🟢 | `%LOCALAPPDATA%\Google\Chrome\User Data\Default\Cache` + `Code Cache` + `CacheStorage` | 多级缓存目录 |
| Edge Cache | 🟢 | 同上，替换 `Google\Chrome` → `Microsoft\Edge` | |
| Firefox Cache | 🟢 | `%LOCALAPPDATA%\Mozilla\Firefox\Profiles\*.default*\cache2\entries` | 需遍历 profile 目录 |
| 回收站 | 🟢 | `SHGetKnownFolderPath(&FOLDERID_RecycleBinFolder)` | 调用 `SHEmptyRecycleBinW` |
| `C:\Windows\Installer` | 🔴 | 硬编码 | MSI 卸载所需，禁止 |
| `C:\Windows\System32` | 🔴 | 硬编码 | 系统文件，禁止 |
| `C:\ProgramData\Package Cache` | 🔴 | 硬编码 | 安装缓存，禁止 |
| `%APPDATA%` 非 Cache | 🔴 | 展开后过滤 | 用户配置数据，禁止 |

**注意**: 所有 `C:\` 硬编码通过 `std::env::var("SystemRoot")` 或 `ExpandEnvironmentStringsW("%SystemRoot%")` 动态计算，支持非 C: 系统盘。

## 实现要点

### 扫描流程
```
start_scan()
  │ 检查是否已在运行 → 是则返回 Err
  │
  ├─ 展开所有目标路径（ExpandEnvironmentStringsW）
  ├─ jwalk 并行遍历（follow_links=false, skip_permission_errors=true）
  ├─ 每遍历完一个目录，聚合该目录下的 CleanItem 批次发送
  ├─ 检查 CancellationToken → 发送 Cancelled 事件
  └─ 发送 Done 事件
```

### 删除流程
```
delete_files()
  ├─ 路径验证：不在白名单路径内 → 拒绝
  ├─ 路径验证：命中系统保护路径 → 拒绝
  │
  ├─ DeleteFileW → 成功 → 计数
  ├─ DeleteFileW → 被占用 → MoveFileExW(DELAY_UNTIL_REBOOT) → 成功 → 计数
  └─ 全部失败 → 标记 errors
```

### 回收站清空
```rust
pub fn empty_recycle_bin() -> Result<(), String> {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        if hr != S_OK && hr != S_FALSE {
            return Err("COM init failed".into());
        }
        let result = SHEmptyRecycleBinW(
            None,
            None,
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI,
        );
        if hr == S_OK {
            CoUninitialize();
        }
        if result.is_err() {
            return Err(format!("EmptyRecycleBin failed: {:?}", result));
        }
        Ok(())
    }
}
```

### 系统盘动态获取
```rust
fn system_drive() -> String {
    let sys_root = std::env::var("SystemRoot")
        .unwrap_or_else(|_| r"C:\Windows".to_string());
    sys_root[..2].to_string() // "C:"
}
```

## 测试策略
- **纯函数测试**: 路径安全校验逻辑、分类映射、DeleteResult 聚合
- **集成测试**: 在 `%TEMP%` 创建测试文件 → 扫描确认发现 → 删除确认释放
- **不测**: Windows API 本身的回收站行为、COM 初始化

## Current Progress
- 尚未开始

## Next Action
等待 TASK-001 完成后，在 `src/cleaner.rs` 中按上述接口实现扫描和清理逻辑。注意 `CoInitializeEx` 在 spawn_blocking 中调用，而非直接 tokio::spawn。

## Resume Hint
打开 `src/cleaner.rs`。先实现路径常量表和辅助函数（system_drive、路径展开），然后实现 `start_scan()` 的 jwalk 遍历 + 聚合推送，再实现 `delete_files()` 的三层降级策略和路径安全校验，最后实现 `empty_recycle_bin()` + COM 初始化。
