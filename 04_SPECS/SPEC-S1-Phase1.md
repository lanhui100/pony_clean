# SPEC-S1: Phase 1 系统清理全覆盖

> 对应 CLEAN_STRATEGY.md §3.3–3.9，实现 28 个新增扫描目标 + 安全加固 + 基础设施升级。
>
> **审查记录**: 经 3 路对抗审查（安全/架构/工程）共发现 35 项问题，采纳 33 项后形成本版。
>
> **工时重估**: 原估 8h，实际约 **40-50h**（cleaner.rs ~550 行 + 测试 ~300 行）。
>
> **完成状态**: 核心代码已完成（Category 枚举、ScanTarget Builder、43 target、PROTECTED_PREFIXES 32 条、所有安全加固、审计日志后端、全部 58 单元测试 + 6 集成测试通过）。"确认弹窗增强"的前端部分未完成（见 SPEC-S4）。

---

## 1. 数据结构变更

### 1.1 ScanTarget — Builder 模式

```rust
#[derive(Clone, Debug)]
pub struct ScanTarget {
    pub id: &'static str,            // 稳定标识符，配置持久化基于 id
    pub path: String,                // 含 %VAR% 的路径模板
    pub level: SafetyLevel,
    pub category: Category,
    pub description: &'static str,
    pub min_size: u64,               // 默认 1024
    pub max_items_per_target: u64,   // 默认 50_000
    pub max_depth: usize,            // 默认 10
    pub glob_include: Option<&'static [&'static str]>,
    pub glob_exclude: Option<&'static [&'static str]>,
    pub requires_service_stop: Option<&'static str>,
    pub browser_profiles: Option<BrowserProfileConfig>, // 新增
}

#[derive(Clone, Debug)]
pub struct BrowserProfileConfig {
    pub base_dirs: &'static [&'static str],
    pub profile_patterns: &'static [&'static str],
    pub cache_subdirs: &'static [&'static str],
}
```

**Builder 模式** — 避免 43×12 字段样板代码:

```rust
impl ScanTarget {
    pub fn new(id: &'static str, path: &str, level: SafetyLevel, cat: Category, desc: &'static str) -> Self {
        Self {
            id, path: path.into(), level, category: cat, description: desc,
            min_size: cat.default_min_size(),
            max_items_per_target: 50_000, max_depth: 10,
            glob_include: None, glob_exclude: None,
            requires_service_stop: None, browser_profiles: None,
        }
    }
    pub fn with_min_size(mut self, v: u64) -> Self { self.min_size = v; self }
    pub fn with_glob(mut self, inc: &'static [&'static str]) -> Self { self.glob_include = Some(inc); self }
    pub fn with_max_depth(mut self, v: usize) -> Self { self.max_depth = v; self }
    pub fn with_service_stop(mut self, s: &'static str) -> Self { self.requires_service_stop = Some(s); self }
    pub fn with_browser(mut self, b: BrowserProfileConfig) -> Self { self.browser_profiles = Some(b); self }
}

impl Category {
    fn default_min_size(&self) -> u64 {
        match self { Category::Cache => 512, Category::Logs => 4096, _ => 1024 }
    }
}
```

### 1.2 Category 枚举

```rust
/// Serialized as lowercase JSON via serde(rename_all = "lowercase").
/// Frontend type: type Category = 'temp' | 'cache' | 'logs' | 'prefetch' | 'recycle_bin' | 'old_install'
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Temp, Cache, Logs, Prefetch, RecycleBin, OldInstall,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_value(self).unwrap().as_str().unwrap())
    }
}
```

### 1.3 配置结构体

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PonyConfig {
    #[serde(default)] pub version: Option<u32>,           // 新增
    #[serde(default)] pub disabled_target_ids: Vec<String>, // 新增: 基于 id
    #[serde(default)] pub disabled_targets: Vec<String>,    // 保留: 旧版基于 path，迁移用
    #[serde(default)] pub custom_exclude_paths: Vec<String>,
    #[serde(default)] pub per_target_config: HashMap<String, TargetConfig>, // 基于 id
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetConfig {
    pub enabled: Option<bool>,
    pub max_items: Option<u64>,
    pub exclude_subdirs: Option<Vec<String>>,
}

/// 迁移 v1 config → v2。路径→id 映射表。
pub fn migrate_v1_to_v2(mut config: PonyConfig) -> PonyConfig {
    let path_to_id: HashMap<&str, &str> = [
        ("%TEMP%", "user_temp"), ("%LOCALAPPDATA%\\Temp", "local_temp"), ...
    ].into();
    for old_path in &config.disabled_targets {
        if let Some(id) = path_to_id.get(old_path.as_str()) {
            config.disabled_target_ids.push(id.to_string());
        }
    }
    config.disabled_targets.clear();
    config.version = Some(2);
    config
}

pub fn load_config() -> PonyConfig {
    let mut cfg: PonyConfig = /* deserialize from JSON */;
    if cfg.version.unwrap_or(1) < 2 { cfg = migrate_v1_to_v2(cfg); }
    cfg
}
```

### 1.4 ScanWarning 枚举化

```rust
#[derive(Clone, Debug)]
pub enum ScanWarning {
    MaxItemsReached { target_id: String, items: u64 },
    PermissionDenied { target_id: String, path: String },
    GlobNoMatch { target_id: String, pattern: String },
    ServiceStopFailed { target_id: String, service: String, reason: String },
    EnvInjectionDetected { target_id: String, path: String },
}

#[derive(Clone, Debug)]
pub enum ScanEvent {
    Progress { scanned: u64, current: String },
    ItemsFound { items: Vec<CleanItem>, batch_complete: bool },
    Done { total_items: u64, total_bytes: u64, skipped_small: u64 },
    Cancelled,
    Warning(ScanWarning),   // String → ScanWarning
}
```

### 1.5 CleanItem — 移除 mtime

```rust
#[derive(Clone, Debug, Serialize)]
pub struct CleanItem {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub level: SafetyLevel,
    pub category: String,       // 来自 Category::to_string()
    // mtime 已被移除。仅在扫描循环内部使用局部变量做时间过滤。
}
```

---

## 2. 常量表

| 常量 | 值 | 说明 |
|------|:---:|------|
| `BATCH_SIZE` | 500 | 不变 |
| `MAX_SCAN_ITEMS` | **300,000** | 保持 v1 值（1M 扫描 = 8min，用户不可接受） |
| `MAX_ITEMS_PER_TARGET` | **50,000** | 新增，替代全局截断 |
| `MAX_UWP_PACKAGES` | **50** | 新增 |
| `LOG_EXPIRY_DAYS` | **90** | 新增，覆盖 logs + 系统日志目标 |
| `DEFAULT_MAX_DEPTH` | **10** | 从 20 降为 10，UWP 目标覆写为 15 |

---

## 3. 函数变更

### 3.1 `get_clean_targets()` — Builder 定义 28+15 目标

**已有 15 目标**（用 Builder 重写，省略 description）:

```rust
pub fn get_clean_targets() -> Vec<ScanTarget> {
    let d = system_drive();
    vec![
        ScanTarget::new("user_temp", "%TEMP%", Safe, Category::Temp, "用户临时文件"),
        ScanTarget::new("local_temp", "%LOCALAPPDATA%\\Temp", Safe, Category::Temp, "当前用户临时文件"),
        ScanTarget::new("sys_temp", "%WINDIR%\\Temp", Confirm, Category::Temp, "系统临时文件"),
        // ... prefetch, chrome/edge x3, firefox, wu_download, driver_store, inet_cache, recycle_bin
    ]
}
```

**Firefox 使用 BrowserProfileConfig**:

```rust
ScanTarget::new("firefox_cache", "%APPDATA%\\Mozilla\\Firefox\\Profiles", Safe, Category::Cache, "Firefox 缓存")
    .with_browser(BrowserProfileConfig {
        base_dirs: &["%APPDATA%\\Mozilla\\Firefox\\Profiles"],
        profile_patterns: &["default", ".default-release", ".default-esr", ".default-nightly", ".dev-edition-default"],
        cache_subdirs: &["cache2/entries", "startupCache", "thumbnails", "offlineCache"],
    }),
```

**新增 28 目标（完整）**:

| id | path | level | cat | min_size | 特殊 |
|----|------|-------|:---:|:--------:|------|
| `sys_logfiles` | `%WINDIR%\System32\LogFiles` | Confirm | Logs | 4096 | max_depth=8 |
| `sys_logs` | `%WINDIR%\Logs` | Confirm | Logs | 4096 | max_depth=8 |
| `wer_user` | `%LOCALAPPDATA%\Microsoft\Windows\WER` | Safe | Logs | 4096 | |
| `wer_system` | `%ALLUSERSPROFILE%\Microsoft\Windows\WER` | Safe | Logs | 4096 | |
| `wer_temp_user` | `%LOCALAPPDATA%\Temp` | Safe | Logs | 4096 | glob_include=["*WER*"] |
| `wer_temp_sys` | `%WINDIR%\Temp` | Safe | Logs | 4096 | glob_include=["*WER*"] |
| `sru` | `%WINDIR%\System32\sru` | Confirm | Logs | 4096 | glob_include=["SRUDB.dat"] |
| `inet_cache_ie` | `%LOCALAPPDATA%\Microsoft\Windows\INetCache\IE` | Safe | Cache | 512 | |
| `oobe_info` | `%WINDIR%\System32\oobe\info` | Safe | Temp | 1024 | |
| `ntms_data` | `%WINDIR%\System32\NtmsData` | Safe | Temp | 1024 | |
| `downloaded_progs` | `%WINDIR%\Downloaded Program Files` | Confirm | Temp | 1024 | |
| `flash_cache` | `%WINDIR%\System32\Macromed\Flash` | Safe | Cache | 512 | |
| `sys_reset` | `%SYSTEMDRIVE%\$SysReset` | Confirm | Temp | 1024 | |
| `win_upgrade_tmp` | `%SYSTEMDRIVE%\$Windows.~BT` | Confirm | Temp | 1024 | |
| `wu_datastore` | `%WINDIR%\SoftwareDistribution\DataStore` | **Forbidden** | Cache | 4096 | 需管理员，运行时降级 |
| `spool_servers` | `%WINDIR%\System32\spool\SERVERS` | Safe | Temp | 1024 | |
| `msdtc_trace` | `%WINDIR%\System32\MsDtc\Trace` | Safe | Logs | 4096 | |
| `uwp_temp` | `%LOCALAPPDATA%\Packages\*\AC\Temp` | Safe | Temp | 1024 | max_items=10K |
| `uwp_inet_cache` | `%LOCALAPPDATA%\Packages\*\AC\INetCache` | Safe | Cache | 512 | max_items=10K |
| `uwp_local_cache` | `%LOCALAPPDATA%\Packages\*\LocalCache` | Safe | Cache | 512 | max_items=10K |
| `app_cache` | `%LOCALAPPDATA%\Microsoft\Windows\AppCache` | Safe | Cache | 512 | |
| `ts_client_cache` | `%LOCALAPPDATA%\Microsoft\TerminalServer Client\Cache` | Safe | Cache | 512 | |
| `downloads_old` | `%USERPROFILE%\Downloads` | Confirm | Temp | **100KB** | mtime>90d |
| `crashdumps` | `%USERPROFILE%\AppData\Local\CrashDumps` | Safe | Logs | 4096 | |
| `etl_logs` | `%LOCALAPPDATA%\Temp` | Safe | Logs | 4096 | glob_include=["*.etl"], mtime>90d |
| `app_logs` | `%LOCALAPPDATA%\Temp` | Safe | Logs | 4096 | glob_include=["*.log"], mtime>90d |
| `wmp_cache` | `%LOCALAPPDATA%\Microsoft\Media Player` | Safe | Cache | 512 | |
| `explorer_cache` | `%LOCALAPPDATA%\Microsoft\Windows\Caches` | Safe | Cache | 512 | |

### 3.2 `resolve_targets()` — 同路径去重

**关键修复（审查 H-04）**: 多个 target 共享相同展开路径时，合并而非重复扫描:

```rust
pub fn resolve_targets(targets: &[ScanTarget]) -> Vec<(PathBuf, &ScanTarget)> {
    let expanded: Vec<(PathBuf, &ScanTarget)> = targets.iter()
        .filter(|t| t.level != SafetyLevel::Forbidden)
        .flat_map(|t| expand_single_target(t))
        .collect();

    // 同路径去重：保留第一个
    let mut seen = HashSet::new();
    expanded.into_iter().filter(|(p, _)| seen.insert(p.clone())).collect()
}
```

Firefox + UWP 展开改为扫描时实时处理（不预处理展开）:

```rust
fn resolve_browser_profiles(cfg: &BrowserProfileConfig) -> Vec<PathBuf> {
    let base = expand_env(cfg.base_dirs[0]);
    read_dir_filtered(&base, |name| {
        cfg.profile_patterns.iter().any(|p| name.contains(p) || name.ends_with(p))
    }).into_iter().flat_map(|profile| {
        cfg.cache_subdirs.iter().map(move |sub| profile.join(sub))
    }).filter(|p| p.exists()).collect()
}
```

**UWP junction 防御**（审查 H-03）:

```rust
fn resolve_uwp_packages() -> Vec<PathBuf> {
    let packages_dir = expand_env("%LOCALAPPDATA%\\Packages");
    std::fs::read_dir(&packages_dir).into_iter().flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && !e.metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) // 跳过 junction
        })
        .take(MAX_UWP_PACKAGES)
        .map(|e| e.path())
        .filter(|p| !is_path_protected(p))  // 二次验证
        .collect()
}
```

### 3.3 `expand_env()` — 补全变量 + 防御

```rust
fn expand_env(raw: &str) -> String {
    let vars = [
        ("%TEMP%", env!("TEMP")), ("%LOCALAPPDATA%", env!("LOCALAPPDATA")),
        ("%APPDATA%", env!("APPDATA")), ("%WINDIR%", env!("SystemRoot")),
        ("%SYSTEMROOT%", env!("SystemRoot")), ("%USERPROFILE%", env!("USERPROFILE")),
        ("%ALLUSERSPROFILE%", env!("ALLUSERSPROFILE")),   // 新增
        ("%PROGRAMDATA%", env!("PROGRAMDATA")),            // 新增
        ("%PUBLIC%", env!("PUBLIC")),                      // 新增
    ];
    // ... 同 v1 replace 逻辑

    // 防御：结果以 \ 开头时补驱动器前缀
    let s = s.replace("%SYSTEMDRIVE%", &system_drive());
    if s.starts_with('\\') { format!("{}{}", system_drive(), s) } else { s }
}
```

### 3.4 `start_scan()` — 扫描循环

**性能优化（审查 C1/H2）**: glob 在 `process_read_dir` 回调中做目录级剪枝；每文件过滤顺序优化为一次 metadata 调用完成所有检查；预计算 target 查找表避免内层循环线性查找。

```rust
fn start_scan(tx: Sender<ScanEvent>) -> Result<...> {
    let config = load_config();
    let targets = get_filtered_targets(&config);
    let resolved = resolve_targets(&targets); // Vec<(PathBuf, &ScanTarget)>

    // 预计算 target 查找表
    let target_map: HashMap<&str, &ScanTarget> = targets.iter()
        .map(|t| (t.id, t)).collect();

    for (target_path, target_def) in &resolved {
        let glob_inc = target_def.glob_include;
        let glob_exc = target_def.glob_exclude;
        let mtime_cutoff = match target_def.category {
            Category::Logs => Some(/* now - 90 days */),
            _ => None,
        };

        let walk_dir = jwalk::WalkDir::new(target_path)
            .follow_links(false)
            .max_depth(target_def.max_depth)
            .process_read_dir(|depth, _path, _state, children| {
                // 目录级剪枝：depth>0 时检查 glob_include 后缀匹配
                if let Some(inc) = glob_inc {
                    if depth.unwrap_or(0) > 0 {
                        children.retain(|e| {
                            e.as_ref().ok().is_none_or(|entry| {
                                let name = entry.file_name.to_string_lossy();
                                inc.iter().any(|p| name.ends_with(&p[1..])) // "*.etl" → ".etl"
                            })
                        });
                    }
                }
                children.retain(|e| e.is_ok());
            });

        for entry in walk_dir.filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() { continue; }
            let Ok(meta) = entry.metadata() else { continue; };
            let size = meta.len();

            // 单次 metadata() 调用完成所有过滤
            if size < target_def.min_size { skipped_small += 1; continue; }
            if let Some(cutoff) = mtime_cutoff {
                if let Ok(mtime) = meta.modified() {
                    if mtime > cutoff { continue; } // 跳过不过期文件
                }
            }
            if let Some(inc) = glob_inc {
                let name = entry.file_name().to_string_lossy();
                if !inc.iter().any(|p| glob_match(p, &name)) { continue; }
            }
            if let Some(exc) = glob_exc {
                let name = entry.file_name().to_string_lossy();
                if exc.iter().any(|p| glob_match(p, &name)) { continue; }
            }
            // ... push to batch
        }
    }
}
```

### 3.5 `is_path_protected()` — 20+ 条 + 分隔符边界

```rust
const PROTECTED_PREFIXES: &[&str] = &[
    // 原 12 条
    "%SYSTEMDRIVE%\\Windows\\System32",            // 需分隔符边界
    "%SYSTEMDRIVE%\\Windows\\Installer",
    "%SYSTEMDRIVE%\\Windows\\WinSxS",
    "%SYSTEMDRIVE%\\Windows\\SystemResources",
    "%SYSTEMDRIVE%\\Windows\\Fonts",
    "%SYSTEMDRIVE%\\Windows\\assembly",
    "%SYSTEMDRIVE%\\Windows\\Servicing",
    "%SYSTEMDRIVE%\\Program Files",
    "%SYSTEMDRIVE%\\Program Files (x86)",
    "%SYSTEMDRIVE%\\ProgramData\\Package Cache",
    "%SYSTEMDRIVE%\\Program Files\\WindowsApps",
    "%SYSTEMDRIVE%\\Users\\Default",
    // 新增 18 条
    "%SYSTEMDRIVE%\\Windows\\System32\\winevt\\Logs",
    "%SYSTEMDRIVE%\\Windows\\System32\\catroot2",
    "%SYSTEMDRIVE%\\Windows\\System32\\catroot",
    "%SYSTEMDRIVE%\\Windows\\System32\\spool\\drivers",
    "%SYSTEMDRIVE%\\Windows\\System32\\Tasks",
    "%SYSTEMDRIVE%\\Windows\\System32\\Tasks\\MICROSOFT",
    "%SYSTEMDRIVE%\\Windows\\System32\\drivers\\etc",
    "%SYSTEMDRIVE%\\Windows\\System32\\CodeIntegrity",
    "%SYSTEMDRIVE%\\Windows\\System32\\Licensing",
    "%SYSTEMDRIVE%\\Windows\\System32\\config",
    "%SYSTEMDRIVE%\\Windows\\System32\\config\\RegBack",
    "%SYSTEMDRIVE%\\Windows\\System32\\GroupPolicy",
    "%SYSTEMDRIVE%\\Windows\\System32\\SMI\\Store\\Machine",
    "%SYSTEMDRIVE%\\System Volume Information",
    "%SYSTEMDRIVE%\\Windows\\CSC",
    "%SYSTEMDRIVE%\\Windows\\Registration",
    "%SYSTEMDRIVE%\\Config.Msi",
    "%SYSTEMDRIVE%\\ProgramData\\USOShared",
    "%SYSTEMDRIVE%\\Recovery",
];

pub fn is_path_protected(path: &Path) -> bool {
    let d = system_drive().to_lowercase();
    let raw = path.to_string_lossy();
    let raw = raw.trim_start_matches("\\\\?\\");
    let normalized = raw.replace('/', "\\").to_lowercase();
    let cleaned = normalized.split('\0').next().unwrap_or("");
    let on_c = cleaned.replacen(&d, "c:", 1);

    // SleepStudy: 目录本身受保护，允许删除过期子文件（已移除 PROTECTED_PREFIXES）
    let sleepstudy = format!("c:\\windows\\system32\\sleepstudy");
    let is_ss_dir = cleaned == sleepstudy
        || cleaned.trim_end_matches('\\') == sleepstudy;
    if is_ss_dir { return true; }

    // PROTECTED_PREFIXES 匹配 + 分隔符边界（防止 System32_Extra 误判）
    PROTECTED_PREFIXES.iter().any(|p| {
        let p = p.replace("%SYSTEMDRIVE%", "c:").to_lowercase();
        let p_trimmed = p.trim_end_matches('\\');
        on_c.starts_with(p_trimmed)
            && (on_c.len() == p_trimmed.len()
                || on_c.as_bytes().get(p_trimmed.len()) == Some(&b'\\'))
    }) || cleaned == format!("{d}\\") || path.parent().is_none()
}
```

### 3.6 `is_path_allowed()` — 分隔符边界 + 尾部反斜杠保护

```rust
pub fn is_path_allowed(path: &Path, targets: &[ScanTarget]) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    targets.iter().any(|t| {
        let expanded = expand_env(&t.path).to_lowercase();
        let trimmed = expanded.trim_end_matches('\\');  // 修复尾部 \ 盲区
        path_str.starts_with(trimmed)
            && (path_str.len() == trimmed.len()
                || path_str.as_bytes().get(trimmed.len()) == Some(&b'\\'))
    })
}
```

### 3.7 环境变量注入防御（审查 H-02）

```rust
fn verify_env_path(expanded: &Path, raw_pattern: &str) -> bool {
    match std::fs::canonicalize(expanded) {
        Ok(canon) => {
            let expected = expand_env(raw_pattern).to_lowercase();
            let can_str = canon.to_string_lossy().to_lowercase();
            can_str.starts_with(expected.trim_end_matches('\\'))
        }
        Err(_) => {
            tracing::warn!("Path from env expansion does not exist: {}", expanded.display());
            false       // 路径不存在 = 不放过（防御 TOCTOU）
        }
    }
}
```

### 3.8 删除操作日志 + 轮转

```rust
#[derive(Serialize, Deserialize)]
pub struct CleanLogEntry {
    pub timestamp: String, paths: Vec<String>,
    total_bytes: u64, result: DeleteResult,
}

const MAX_LOG_BYTES: u64 = 1_048_576; // 1MB rotation

pub fn append_clean_log(entry: &CleanLogEntry) -> Result<(), String> {
    let dir = config_dir();
    let log_path = dir.join("clean_log.jsonl");

    // 轮转：超过 1MB 时重命名
    if log_path.exists() && log_path.metadata().map(|m| m.len()).unwrap_or(0) > MAX_LOG_BYTES {
        let rotated = dir.join("clean_log.0.jsonl");
        let _ = std::fs::rename(&log_path, &rotated);
    }

    let json = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true).append(true).open(&log_path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{json}").map_err(|e| e.to_string())?;
    Ok(())
}
```

### 3.9 Tauri 命令层适配 ScanWarning

```rust
// commands/cleaner.rs
Ok(ScanEvent::Warning(w)) => {
    let payload = match w {
        ScanWarning::MaxItemsReached { target_id, items } => json!({
            "type": "max_items_reached", "target_id": target_id, "items": items
        }),
        ScanWarning::PermissionDenied { target_id, path } => json!({
            "type": "permission_denied", "target_id": target_id, "path": path
        }),
        ScanWarning::GlobNoMatch { target_id, pattern } => json!({
            "type": "glob_no_match", "target_id": target_id, "pattern": pattern
        }),
        ScanWarning::ServiceStopFailed { target_id, service, reason } => json!({
            "type": "service_stop_failed", "target_id": target_id, "service": service, "reason": reason
        }),
        ScanWarning::EnvInjectionDetected { target_id, path } => json!({
            "type": "env_injection_detected", "target_id": target_id, "path": path
        }),
    };
    let _ = app_handle.emit("scan-warning", payload);
}
```

---

## 4. 测试策略

### 4.1 单元测试

| 测试 | 验证点 |
|------|--------|
| `test_new_targets_resolve` | 43 target resolve 不 panic，长度正确 |
| `test_target_ids_unique` | 所有 id 唯一 |
| `test_uwp_package_limit` | 枚举不超过 50 |
| `test_uwp_junction_skip` | junction 目录被跳过 |
| `test_glob_include_filter` | glob_include 正确保留/排除 |
| `test_glob_exclude_filter` | glob_exclude 正确排除 |
| `test_min_size_per_category` | 各 category 默认 min_size |
| `test_per_target_max_items` | 单 target 不超过 max_items |
| `test_is_path_allowed_separator` | 拒绝 Temp_malicious，接受正常子路径 |
| `test_is_path_allowed_trailing_slash` | expanded 尾部 \ 不影响 |
| `test_is_path_protected_separator` | 拒绝 System32_Extra，保护 System32 |
| `test_env_injection_defense` | TEMP=C: 展开路径被 refuse |
| `test_env_expand_path_non_exist` | verify_env_path 对不存在路径返回 false |
| `test_alluserprofile_expand` | 新变量正确展开 |
| `test_protected_winevt_logs` | winevt 被保护 |
| `test_protected_sleepstudy_dir` | SleepStudy 目录本身受保护 |
| `test_sleepstudy_subfile_allowed` | 子文件不受保护（靠 mtime 过滤）|
| `test_logs_mtime_filter` | >90d 跳过，<90d 保留 |
| `test_downloads_size_and_mtime` | >100KB + >90d 才列出 |
| `test_category_serde_roundtrip` | `Temp→"temp"`, `OldInstall→"old_install"` |
| `test_clean_log_append_and_rotate` | 日志追加 + 1MB 轮转 |
| `test_config_migration_v1_to_v2` | 旧 config 自动迁移到 id-based |
| `test_get_filtered_targets_uses_id` | disabled_target_ids 按 id 过滤 |
| `test_resolve_targets_dedup_same_path` | 同路径 target 去重 |
| `test_scan_warning_enum_variants` | 所有 ScanWarning 变体可构造/匹配 |

### 4.2 集成测试

| 测试 | 验证点 |
|------|--------|
| `test_temp_scan_and_delete` | %TEMP% 创建文件 → 扫描发现 → 清理成功 |
| `test_protected_paths_skipped` | 尝试扫描被保护路径 → resolve_targets 跳过 |
| `test_log_expiry_integration` | 创建过期/非过期日志 → 扫描确认过滤 |
| `test_uwp_packages_limit` | 创建 60 个包 → 只枚举 50 |
| `test_multi_target_same_path` | 同路径不同 glob → 扫描不重复不遗漏 |

### 4.3 手动验证（发布前）

```
[ ] 逐个检查 28 个 target 路径在 Windows 11/10 上存在
[ ] 中文/日文/德文 Windows: %ALLUSERSPROFILE% 展开正确
[ ] UWP Packages 目录 >500 包场景下枚举时间 <2s
[ ] Firefox 多 profile 共存（default + release + dev-edition）
[ ] DISM 在 zh-CN/en-US 下的退出码一致性
[ ] 非管理员用户下 `wu_datastore` 正确降级跳过
[ ] EFS 不可用环境下操作日志正常写入（明文 fallback）
```

---

## 5. 文件变更

| 文件 | 变更 | 估算 |
|------|------|:----:|
| `crates/pony_core/src/cleaner.rs` | Builder / Category / 28 target / 扫描改造 / 安全加固 / 日志 | +550 |
| `crates/pony_core/src/error.rs` | 可能新增日志相关错误变体 | +5 |
| `crates/pony_core/src/lib.rs` | pub use Category | +2 |
| `src-tauri/src/commands/cleaner.rs` | ScanWarning 结构化发射 | +20 |
| `frontend/src/composables/useCleaner.ts` | ScanWarning 监听, Category 类型 | +25 |
| `frontend/src/views/CleanerPanel.vue` | Warning 横幅展示 | +40 |
| **合计** | | **~640** |
| **测试** | 24 单元 + 5 集成 | **~300** |
| **总计** | | **~940** |

**工时**: ~40-50h。建议分两轮：
- **P1 (20h)**: 数据结构 + 安全加固 + 日志 + 测试框架
- **P2 (20-30h)**: 28 target 追加 + 扫描循环改造 + 验收测试

---

## 6. 验收标准

1. [ ] Builder 构建 43 个 target，id 全部唯一
2. [ ] Category 枚举序列化为小写，roundtrip 测试通过
3. [ ] 每 target min_size/max_items/max_depth 独立生效
4. [ ] 同路径 target 去重，不会重复扫描
5. [ ] `is_path_allowed` 分隔符边界 + 尾部 `\` 修复
6. [ ] `is_path_protected` 分隔符边界 + SleepStudy 目录保护
7. [ ] `expand_env` 新增 3 变量 + `\`开头补驱动器前缀
8. [ ] `verify_env_path` 路径不存在时 false（不放过）
9. [ ] UWP junction 检测 + 50 包上限
10. [ ] Firefox 通过 BrowserProfileConfig 可配置
11. [ ] Logs 类别 mtime >90d 过滤
12. [ ] 日志 1MB 自动轮转写入
13. [ ] ScanWarning 5 变体枚举化，前端结构化展示
14. [ ] config 迁移: v1→v2 路径→id 映射
15. [ ] `wu_datastore` 非管理员下自动降级跳过
16. [ ] CI: `cargo fmt --check && cargo clippy && cargo build -p pony_core && cargo test -p pony_core`
