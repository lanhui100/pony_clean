# SPEC-S4: Phase 1 操作日志 + 确认弹窗增强

> 对应 CLEAN_STRATEGY.md §3.6-§3.7。实现删除操作日志持久化 + 清理确认对话框升级。
>
> **审查调优**: 经 3 路对抗审查发现 19 项问题，采纳 18 项。关键变更: `hasDelayedDelete` 移至后端计算; `total_cleaned_bytes` 使用独立统计文件而非动态计算; `categorize_by_target` 在删除前执行; 日志轮转保留 5 个文件; 追加操作加 Mutex 保护; `_at` 测试注入函数; 工时重估 3h→7h。
>
> **工时**: 7h（Rust ~130 行 + 前端 ~150 行 + 测试 ~70 行）
>
> **完成状态**: 后端全部完成（CleanLogEntry、append_clean_log、get_clean_logs、轮转、stats、错误脱敏、execute_clean 集成）。前端任务未完成：确认弹窗增强（类别明细表、延迟删除提示）、操作记录面板（底部按钮 + Sheet 日志展示）、DPAPI 加密。

---

## 1. 功能概述

### 1.1 操作日志

- 每次 `execute_clean` 自动记录到 `%LOCALAPPDATA%\PonyClean\clean_log.jsonl`
- **保留 5 个轮转文件**（clean_log.0..4.jsonl），每个 1MB，覆盖 ~2500 条记录
- **清理统计独立持久化**: `clean_stats.json` 维护 `{total_bytes, total_files}` 原子更新
- DPAPI 加密（默认开启，明文 fallback 仅在加密失败时）
- 日志内容: timestamp, total_files, total_bytes, success, failed, errors(sanitized), by_category

### 1.2 确认弹窗增强

- 类别明细表（颜色 + 文件数 + 大小）
- 延迟删除重启警告（从后端计算）
- 操作记录提示

---

## 2. 数据结构

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanLogEntry {
    pub timestamp: String,
    pub total_files: u64,
    pub total_bytes: u64,
    pub success: u64,
    pub failed: u64,
    pub errors: Vec<String>,       // sanitized: 仅文件名, 不含完整路径
    pub by_category: HashMap<String, CategorySummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategorySummary {
    pub files: u64,
    pub bytes: u64,
}

// 独立统计文件
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CleanStats {
    pub total_bytes: u64,
    pub total_files: u64,
    pub last_cleaned_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CleanConfirmData {
    pub selected_count: u64,
    pub selected_bytes: u64,
    pub by_category: HashMap<String, CategorySummary>,
    pub has_delayed_delete: bool,
}
```

---

## 3. 后端实现

### 3.1 数据目录抽象

```rust
fn data_dir() -> PathBuf {
    let local = env!("LOCALAPPDATA").unwrap_or_else(|| format!("{}\\AppData\\Local", env!("USERPROFILE").unwrap_or_default()));
    PathBuf::from(local).join("PonyClean")
}
// 替代 config_path + clean_log_path 各自拼接
fn config_path() -> PathBuf { data_dir().join("config.json") }
fn clean_log_path() -> PathBuf { data_dir().join("clean_log.jsonl") }
fn stats_path() -> PathBuf { data_dir().join("clean_stats.json") }
```

### 3.2 日志追加（含 Mutex 保护）

```rust
use std::sync::Mutex;
static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn append_clean_log(entry: &CleanLogEntry) -> Result<(), String> {
    append_clean_log_at(entry, &data_dir())
}

fn append_clean_log_at(entry: &CleanLogEntry, dir: &Path) -> Result<(), String> {
    let _lock = LOG_LOCK.lock().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).ok();
    let path = dir.join("clean_log.jsonl");
    rotate_if_needed(&path, 5);  // 保留 5 个备份
    let json = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)
        .map_err(|e| e.to_string())?;
    use std::io::Write;
    writeln!(file, "{json}").map_err(|e| e.to_string())?;

    // 更新统计
    update_stats(dir, entry.total_files, entry.total_bytes);
    Ok(())
}

fn rotate_if_needed(path: &Path, max_backups: u32) {
    if path.exists() && path.metadata().map(|m| m.len()).unwrap_or(0) > 1_048_576 {
        for i in (1..max_backups).rev() {
            let old = path.with_extension(format!("{i}.jsonl"));
            let new = path.with_extension(format!("{}.jsonl", i + 1));
            if old.exists() { let _ = std::fs::rename(&old, &new); }
        }
        let first = path.with_extension("1.jsonl");
        let _ = std::fs::rename(path, &first);
    }
}
```

### 3.3 统计持久化

```rust
fn update_stats(dir: &Path, files: u64, bytes: u64) {
    let path = dir.join("clean_stats.json");
    let mut stats = std::fs::read_to_string(&path).ok()
        .and_then(|s| serde_json::from_str::<CleanStats>(&s).ok())
        .unwrap_or_default();
    stats.total_files += files;
    stats.total_bytes += bytes;
    stats.last_cleaned_at = chrono::Utc::now().to_rfc3339();
    if let Ok(json) = serde_json::to_string(&stats) {
        let _ = std::fs::write(&path, json);
    }
}
```

### 3.4 日志查询

```rust
pub fn get_clean_logs(limit: usize) -> Result<CleanLogSummary, String> {
    let dir = data_dir();
    let path = dir.join("clean_log.jsonl");
    let mut entries = Vec::new();
    if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        entries = content.lines()
            .rev().take(limit)
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
    }
    let stats = std::fs::read_to_string(dir.join("clean_stats.json")).ok()
        .and_then(|s| serde_json::from_str::<CleanStats>(&s).ok())
        .unwrap_or_default();
    Ok(CleanLogSummary { entries, total_cleaned_bytes: stats.total_bytes, total_cleaned_files: stats.total_files })
}
```

### 3.5 execute_clean 集成

```rust
// 删除前计算 category 和 total_bytes（避免删除后 stat 为 0）
fn prepare_clean_log(pathbufs: &[PathBuf], targets: &[ScanTarget]) -> (HashMap<String, CategorySummary>, u64) {
    let mut by_category = HashMap::new();
    let mut total_bytes = 0u64;
    for p in pathbufs {
        let cat = classify_path(p, targets);
        let size = p.metadata().map(|m| m.len()).unwrap_or(0);
        *by_category.entry(cat).or_insert(CategorySummary { files: 0, bytes: 0 }) = CategorySummary {
            files: by_category[&cat].files + 1,
            bytes: by_category[&cat].bytes + size,
        };
        total_bytes += size;
    }
    (by_category, total_bytes)
}
```

### 3.6 hasDelayedDelete 后端计算

```rust
fn check_delayed_deletes(paths: &[PathBuf]) -> bool {
    // 在 execute_clean 中, 对每个尝试 DeleteFileW 失败
    // 且 fallback 到 MoveFileExW DELAY_UNTIL_REBOOT 的文件计数
    // 如果有任何文件使用了延迟删除 → has_delayed_delete = true
    // 返回值通过 execute_clean 返回给前端
}
```

---

## 4. 前端实现

（同 v1 版 SPEC-S4。有以下关键变更点:）

- 确认弹窗的 `hasDelayedDelete` 从 `execute_clean` 返回值获取，非前端计算
- `totalCleanedBytes` 从 `get_clean_logs` 返回的 `total_cleaned_bytes` 读取，非本地累计
- 日志面板中 errors 显示已 sanitize（仅文件名）

---

## 5. 测试

```rust
#[test]
fn test_append_and_read_log() {
    let dir = tempfile::tempdir().unwrap();
    let entry = CleanLogEntry {
        timestamp: "2026-01-01T00:00:00Z".into(),
        total_files: 10, total_bytes: 10240,
        success: 10, failed: 0, errors: vec![],
        by_category: HashMap::new(),
    };
    append_clean_log_at(&entry, dir.path()).unwrap();
    let result = get_clean_logs_at(10, dir.path()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].success, 10);
}

#[test]
fn test_log_rotation_keeps_multiple_backups() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..2000 {  // >5MB 触发多次轮转
        let entry = CleanLogEntry { timestamp: format!("2026-01-01T00:00:{i:04}Z"), .. };
        append_clean_log_at(&entry, dir.path()).unwrap();
    }
    // 应保留 5 个备份 + 当前文件
    let count = std::fs::read_dir(dir.path()).unwrap()
        .filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().contains("clean_log")).count();
    assert!(count >= 4, "should keep multiple rotated files: {count}");
}

#[test]
fn test_clean_stats_persistent() {
    let dir = tempfile::tempdir().unwrap();
    update_stats_at(dir.path(), 100, 50000);
    let stats = get_stats_at(dir.path()).unwrap();
    assert_eq!(stats.total_files, 100);
    // 再次追加
    update_stats_at(dir.path(), 50, 25000);
    let stats = get_stats_at(dir.path()).unwrap();
    assert_eq!(stats.total_files, 150);
    assert_eq!(stats.total_bytes, 75000);
}

#[test]
fn test_prepare_clean_log_before_delete() {
    let dir = tempfile::tempdir().unwrap();
    let f1 = dir.path().join("test.tmp"); std::fs::write(&f1, vec![0u8; 1000]).unwrap();
    let (by_cat, total) = prepare_clean_log_inner(&[f1], &get_clean_targets());
    assert_eq!(total, 1000);
}
```

---

## 6. 文件变更

| 文件 | 变更 | 行数 |
|------|------|:----:|
| `crates/pony_core/src/cleaner.rs` | data_dir / 日志 / 统计 / confirm_data | +100 |
| `src-tauri/src/commands/cleaner.rs` | get_clean_logs / execute_clean 日志集成 | +30 |
| `frontend/src/composables/useCleaner.ts` | 日志类型 + loadCleanLogs + confirmData | +40 |
| `frontend/src/views/CleanerPanel.vue` | 确认弹窗增强 + 操作记录面板 | +80 |
| `frontend/src/components/CleanConfirmDialog.vue` | 新增子组件 | +50 |
| `frontend/src/components/CleanLogSheet.vue` | 新增子组件 | +40 |
| **合计** | | **~340** |

---

## 7. 验收标准

1. [ ] 清理后自动写入 `clean_log.jsonl`，errors 路径已 sanitize
2. [ ] `CleanConfirmData.has_delayed_delete` 从后端计算
3. [ ] 日志保留 5 个轮转文件，不丢失历史
4. [ ] `clean_stats.json` 维护累计统计，轮转不影响
5. [ ] `get_clean_logs` 使用 `spawn_blocking`（async）
6. [ ] 确认弹窗显示类别明细表 + 颜色 + 重启警告
7. [ ] 前端「操作记录」面板展示日志
8. [ ] 日志追加使用 Mutex 保护并发写入
9. [ ] 全部测试通过
