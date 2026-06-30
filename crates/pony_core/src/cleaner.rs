use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

const BATCH_SIZE: usize = 500;
/// 全局扫描结果上限
const MAX_SCAN_ITEMS: u64 = 300_000;
/// 每 target 扫描上限
const MAX_ITEMS_PER_TARGET: u64 = 50_000;
/// UWP 包枚举上限
const MAX_UWP_PACKAGES: usize = 50;
/// 日志过期天数
const LOG_EXPIRY_DAYS: i64 = 90;
/// 默认扫描深度
const DEFAULT_MAX_DEPTH: usize = 10;

/// 安全级别
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum SafetyLevel {
    Safe,
    Confirm,
    Forbidden,
}

/// 清理目标分类，序列化为小写 JSON
/// 前端类型: type Category = 'temp' | 'cache' | 'logs' | 'prefetch' | 'recycle_bin' | 'old_install'
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Temp,
    Cache,
    Logs,
    Prefetch,
    RecycleBin,
    OldInstall,
}

impl Category {
    pub fn default_min_size(&self) -> u64 {
        match self {
            Category::Cache => 512,
            Category::Logs => 4096,
            _ => 1024,
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Category::Temp => "temp",
            Category::Cache => "cache",
            Category::Logs => "logs",
            Category::Prefetch => "prefetch",
            Category::RecycleBin => "recycle_bin",
            Category::OldInstall => "old_install",
        };
        write!(f, "{s}")
    }
}

/// 浏览器 profile 匹配配置
#[derive(Clone, Debug)]
pub struct BrowserProfileConfig {
    pub profile_patterns: &'static [&'static str],
    pub cache_subdirs: &'static [&'static str],
}

/// 可清理项
#[derive(Clone, Debug, Serialize)]
pub struct CleanItem {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub level: SafetyLevel,
    pub category: String,
}

/// 类型化的扫描警告
#[derive(Clone, Debug)]
pub enum ScanWarning {
    MaxItemsReached { target_id: String, items: u64 },
    PermissionDenied { target_id: String, path: String },
    GlobNoMatch { target_id: String, pattern: String },
    ServiceStopFailed { target_id: String, service: String, reason: String },
    EnvInjectionDetected { target_id: String, path: String },
}

/// 扫描进度事件
#[derive(Clone, Debug)]
pub enum ScanEvent {
    Progress {
        scanned: u64,
        current: String,
    },
    ItemsFound {
        items: Vec<CleanItem>,
        batch_complete: bool,
    },
    Done {
        total_items: u64,
        total_bytes: u64,
        skipped_small: u64,
    },
    Cancelled,
    Warning(ScanWarning),
}

/// 清理命令
#[derive(Debug)]
pub enum CleanCommand {
    Execute(Vec<PathBuf>),
    EmptyRecycleBin,
    CancelScan,
    Shutdown,
}

/// 删除进度事件
#[derive(Clone, Debug, Serialize)]
pub struct DeleteProgress {
    pub done: u64,
    pub total: u64,
    pub current: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct DeleteResult {
    pub success: u64,
    pub failed: u64,
    pub errors: Vec<String>,
}

/// 扫描目标
#[derive(Clone, Debug)]
pub struct ScanTarget {
    pub id: &'static str,
    pub path: String,
    pub level: SafetyLevel,
    pub category: Category,
    pub description: &'static str,
    pub min_size: u64,
    pub max_items_per_target: u64,
    pub max_depth: usize,
    pub glob_include: Option<&'static [&'static str]>,
    pub glob_exclude: Option<&'static [&'static str]>,
    pub requires_service_stop: Option<&'static str>,
    pub browser_profiles: Option<BrowserProfileConfig>,
}

impl ScanTarget {
    pub fn new(id: &'static str, path: &str, level: SafetyLevel, cat: Category, desc: &'static str) -> Self {
        Self {
            id,
            path: path.into(),
            level,
            category: cat.clone(),
            description: desc,
            min_size: cat.default_min_size(),
            max_items_per_target: MAX_ITEMS_PER_TARGET,
            max_depth: DEFAULT_MAX_DEPTH,
            glob_include: None,
            glob_exclude: None,
            requires_service_stop: None,
            browser_profiles: None,
        }
    }
    pub fn with_min_size(mut self, v: u64) -> Self { self.min_size = v; self }
    pub fn with_glob(mut self, inc: &'static [&'static str]) -> Self { self.glob_include = Some(inc); self }
    pub fn with_glob_exclude(mut self, exc: &'static [&'static str]) -> Self { self.glob_exclude = Some(exc); self }
    pub fn with_max_depth(mut self, v: usize) -> Self { self.max_depth = v; self }
    pub fn with_service_stop(mut self, s: &'static str) -> Self { self.requires_service_stop = Some(s); self }
    pub fn with_browser(mut self, b: BrowserProfileConfig) -> Self { self.browser_profiles = Some(b); self }
}

/// 用户配置
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PonyConfig {
    #[serde(default)] pub version: Option<u32>,
    #[serde(default)] pub disabled_target_ids: Vec<String>,
    #[serde(default)] pub disabled_targets: Vec<String>,
    #[serde(default)] pub custom_exclude_paths: Vec<String>,
    #[serde(default)] pub per_target_config: HashMap<String, TargetConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetConfig {
    pub enabled: Option<bool>,
    pub max_items: Option<u64>,
    pub exclude_subdirs: Option<Vec<String>>,
}

/// 保存用户配置
pub fn save_config(config: &PonyConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| format!("Serialize config: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Write config: {e}"))
}

/// 获取过滤后的扫描目标
pub fn get_filtered_targets(config: &PonyConfig) -> Vec<ScanTarget> {
    let all = get_clean_targets();
    all.into_iter()
        .filter(|t| !config.disabled_target_ids.contains(&t.id.to_string()))
        .filter(|t| {
            config.per_target_config.get(t.id)
                .and_then(|c| c.enabled)
                .unwrap_or(true)
        })
        .collect()
}

pub fn config_dir() -> PathBuf { data_dir() }
fn data_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\Default".into());
        format!("{home}\\AppData\\Local")
    });
    PathBuf::from(local).join("PonyClean")
}

fn config_path() -> PathBuf { data_dir().join("config.json") }

/// 获取系统盘符
fn system_drive() -> String {
    std::env::var("SYSTEMDRIVE").ok()
        .or_else(|| {
            std::env::var("SystemRoot").ok().map(|r| r.get(..2).unwrap_or("C:").to_string())
        })
        .unwrap_or_else(|| "C:".to_string())
}

/// 展开环境变量
fn expand_env(raw: &str) -> String {
    // 缓存环境变量值，避免同一变量多次查询
    let mut cache: HashMap<String, String> = HashMap::new();
    let get_env = |name: &str, c: &mut HashMap<String, String>| -> Option<String> {
        if let Some(v) = c.get(name) { return Some(v.clone()); }
        let val = std::env::var(name).ok()
            .or_else(|| std::env::var(name.to_uppercase()).ok())
            .or_else(|| std::env::var(name.to_lowercase()).ok());
        if let Some(v) = val { c.insert(name.to_string(), v.clone()); Some(v) } else { None }
    };

    let mut s = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find('%') {
        s.push_str(&rest[..start]);
        let after_pct = &rest[start + 1..];
        if let Some(end) = after_pct.find('%') {
            let var_name = &after_pct[..end];
            match get_env(var_name, &mut cache) {
                Some(v) => s.push_str(&v),
                None => { s.push('%'); s.push_str(var_name); s.push('%'); }
            }
            rest = &after_pct[end + 1..];
        } else {
            s.push('%');
            rest = after_pct;
        }
    }
    s.push_str(rest);
    if s.contains("%SYSTEMDRIVE%") { s = s.replace("%SYSTEMDRIVE%", &system_drive()); }
    if s.starts_with('\\') { format!("{}{}", system_drive(), s) } else { s }
}

pub fn default_targets() -> Vec<ScanTarget> { get_clean_targets() }

/// 安全扫描路径列表（15 已有 + 28 新增 = 43 target）
pub fn get_clean_targets() -> Vec<ScanTarget> {
    let d = system_drive();
    vec![
        // === 已有 15 目标 ===
        ScanTarget::new("user_temp", "%TEMP%", SafetyLevel::Safe, Category::Temp, "用户临时文件"),
        ScanTarget::new("local_temp", "%LOCALAPPDATA%\\Temp", SafetyLevel::Safe, Category::Temp, "当前用户临时文件"),
        ScanTarget::new("sys_temp", "%WINDIR%\\Temp", SafetyLevel::Confirm, Category::Temp, "系统临时文件"),
        ScanTarget::new("prefetch", &format!("{d}\\Windows\\Prefetch"), SafetyLevel::Confirm, Category::Prefetch, "应用启动缓存"),
        ScanTarget::new("chrome_code_cache", "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Code Cache", SafetyLevel::Safe, Category::Cache, "Chrome JS Code Cache"),
        ScanTarget::new("chrome_cache", "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Cache", SafetyLevel::Safe, Category::Cache, "Chrome 磁盘缓存"),
        ScanTarget::new("chrome_cache_storage", "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\CacheStorage", SafetyLevel::Safe, Category::Cache, "Chrome CacheStorage"),
        ScanTarget::new("edge_code_cache", "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Code Cache", SafetyLevel::Safe, Category::Cache, "Edge JS Code Cache"),
        ScanTarget::new("edge_cache", "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Cache", SafetyLevel::Safe, Category::Cache, "Edge 磁盘缓存"),
        ScanTarget::new("edge_cache_storage", "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\CacheStorage", SafetyLevel::Safe, Category::Cache, "Edge CacheStorage"),
        ScanTarget::new("firefox_cache", "%APPDATA%\\Mozilla\\Firefox\\Profiles", SafetyLevel::Safe, Category::Cache, "Firefox 缓存")
            .with_browser(BrowserProfileConfig {
                profile_patterns: &["default", ".default-release", ".default-esr", ".default-nightly", ".dev-edition-default"],
                cache_subdirs: &["cache2/entries", "startupCache", "thumbnails", "offlineCache"],
            }),
        ScanTarget::new("wu_download", "%WINDIR%\\SoftwareDistribution\\Download", SafetyLevel::Confirm, Category::Cache, "Windows Update 下载缓存"),
        ScanTarget::new("driver_store", &format!("{d}\\Windows\\System32\\DriverStore\\FileRepository"), SafetyLevel::Confirm, Category::Cache, "旧驱动备份"),
        ScanTarget::new("inet_cache", "%LOCALAPPDATA%\\Microsoft\\Windows\\INetCache", SafetyLevel::Safe, Category::Cache, "Internet 临时文件"),
        ScanTarget::new("recycle_bin", &format!("{d}\\$Recycle.Bin"), SafetyLevel::Safe, Category::RecycleBin, "回收站"),
        // === 新增 28 目标 ===
        ScanTarget::new("sys_logfiles", "%WINDIR%\\System32\\LogFiles", SafetyLevel::Confirm, Category::Logs, "系统日志文件"),
        ScanTarget::new("sys_logs", "%WINDIR%\\Logs", SafetyLevel::Confirm, Category::Logs, "Windows 组件日志"),
        ScanTarget::new("wer_user", "%LOCALAPPDATA%\\Microsoft\\Windows\\WER", SafetyLevel::Safe, Category::Logs, "用户错误报告"),
        ScanTarget::new("wer_system", "%ALLUSERSPROFILE%\\Microsoft\\Windows\\WER", SafetyLevel::Safe, Category::Logs, "系统错误报告"),
        ScanTarget::new("wer_temp_user", "%LOCALAPPDATA%\\Temp", SafetyLevel::Safe, Category::Logs, "Temp 中 WER").with_glob(&["*WER*"]),
        ScanTarget::new("wer_temp_sys", "%WINDIR%\\Temp", SafetyLevel::Safe, Category::Logs, "系统 Temp 中 WER").with_glob(&["*WER*"]),
        ScanTarget::new("sru", "%WINDIR%\\System32\\sru", SafetyLevel::Confirm, Category::Logs, "系统资源使用统计（仅 SRUDB.dat）").with_glob(&["SRUDB.dat"]),
        ScanTarget::new("inet_cache_ie", "%LOCALAPPDATA%\\Microsoft\\Windows\\INetCache\\IE", SafetyLevel::Safe, Category::Cache, "IE/Edge 传统 Internet 缓存"),
        ScanTarget::new("oobe_info", "%WINDIR%\\System32\\oobe\\info", SafetyLevel::Safe, Category::Temp, "OOBE 安装信息残留"),
        ScanTarget::new("ntms_data", "%WINDIR%\\System32\\NtmsData", SafetyLevel::Safe, Category::Temp, "可移动存储管理数据"),
        ScanTarget::new("downloaded_progs", "%WINDIR%\\Downloaded Program Files", SafetyLevel::Confirm, Category::Temp, "已下载程序文件"),
        ScanTarget::new("flash_cache", "%WINDIR%\\System32\\Macromed\\Flash", SafetyLevel::Safe, Category::Cache, "Flash 共享对象"),
        ScanTarget::new("wu_datastore", "%WINDIR%\\SoftwareDistribution\\DataStore", SafetyLevel::Forbidden, Category::Cache, "WU 数据库（需管理员）"),
        ScanTarget::new("spool_servers", "%WINDIR%\\System32\\spool\\SERVERS", SafetyLevel::Safe, Category::Temp, "打印服务器临时文件"),
        ScanTarget::new("msdtc_trace", "%WINDIR%\\System32\\MsDtc\\Trace", SafetyLevel::Safe, Category::Logs, "分布式事务协调器日志"),
        ScanTarget::new("uwp_temp", "%LOCALAPPDATA%\\Packages", SafetyLevel::Safe, Category::Temp, "UWP 临时文件"),
        ScanTarget::new("uwp_inet_cache", "%LOCALAPPDATA%\\Packages", SafetyLevel::Safe, Category::Cache, "UWP Internet 缓存"),
        ScanTarget::new("uwp_local_cache", "%LOCALAPPDATA%\\Packages", SafetyLevel::Safe, Category::Cache, "UWP 本地缓存"),
        ScanTarget::new("app_cache", "%LOCALAPPDATA%\\Microsoft\\Windows\\AppCache", SafetyLevel::Safe, Category::Cache, "Windows App 缓存"),
        ScanTarget::new("ts_client_cache", "%LOCALAPPDATA%\\Microsoft\\TerminalServer Client\\Cache", SafetyLevel::Safe, Category::Cache, "远程桌面图标缓存"),
        ScanTarget::new("downloads_old", "%USERPROFILE%\\Downloads", SafetyLevel::Confirm, Category::Temp, "下载文件夹过时文件").with_min_size(102_400),
        ScanTarget::new("crashdumps", "%USERPROFILE%\\AppData\\Local\\CrashDumps", SafetyLevel::Safe, Category::Logs, "应用崩溃转储"),
        ScanTarget::new("etl_logs", "%LOCALAPPDATA%\\Temp", SafetyLevel::Safe, Category::Logs, "事件跟踪日志").with_glob(&["*.etl"]),
        ScanTarget::new("app_logs", "%LOCALAPPDATA%\\Temp", SafetyLevel::Safe, Category::Logs, "应用日志").with_glob(&["*.log"]),
        ScanTarget::new("wmp_cache", "%LOCALAPPDATA%\\Microsoft\\Media Player", SafetyLevel::Safe, Category::Cache, "WMP 媒体库缓存"),
        ScanTarget::new("explorer_cache", "%LOCALAPPDATA%\\Microsoft\\Windows\\Caches", SafetyLevel::Safe, Category::Cache, "资源管理器缓存"),
        ScanTarget::new("sys_reset", &format!("{d}\\$SysReset"), SafetyLevel::Confirm, Category::Temp, "系统重置备份"),
        ScanTarget::new("win_upgrade_tmp", &format!("{d}\\$Windows.~BT"), SafetyLevel::Confirm, Category::Temp, "Windows 升级临时文件"),
    ]
}

/// 受保护路径前缀（禁止删除）
const PROTECTED_PREFIXES: &[&str] = &[
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
    "%SYSTEMDRIVE%\\Windows\\System32\\winevt\\Logs",
    "%SYSTEMDRIVE%\\Windows\\System32\\catroot2",
    "%SYSTEMDRIVE%\\Windows\\System32\\catroot",
    "%SYSTEMDRIVE%\\Windows\\System32\\spool\\drivers",
    "%SYSTEMDRIVE%\\Windows\\System32\\Tasks",
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
    "%SYSTEMDRIVE%\\Boot",
    "%PROGRAMDATA%\\Microsoft\\Windows\\Containers",
    // System32 下允许清理的子目录不在此列表（LogFiles, sru, oobe, NtmsData, spool\SERVERS, MsDtc\Trace, Macromed\Flash, sleepstudy 等）
    // 这由 is_path_protected 和 resolve_targets 中 separate 逻辑控制
];

/// 检查路径是否受保护
pub fn is_path_protected(path: &Path) -> bool {
    let d = system_drive().to_lowercase();
    let raw_str = path.to_string_lossy();

    // GLOBALROOT 直接返回受保护（大小写不敏感）
    let lower = raw_str.to_lowercase();
    if lower.starts_with("\\\\.\\globalroot\\") || lower.starts_with("\\\\?\\globalroot\\") {
        return true;
    }

    let cleaned_str = raw_str.trim_start_matches("\\\\?\\");
    let cleaned_str = cleaned_str.trim_start_matches("\\\\.\\");
    let cleaned_str = cleaned_str.trim_start_matches("//?/");
    let cleaned_str = cleaned_str.trim_start_matches("\\??\\");
    let normalized = cleaned_str.replace('/', "\\").to_lowercase();
    let normalized = normalized.trim_end_matches(&[' ', '.'][..]);
    let cleaned = match normalized.split('\0').next() {
        Some(s) => s,
        None => return true,
    };
    if cleaned.is_empty() { return true; }
    let on_c = cleaned.replacen(&d, "c:", 1);

    // PROTECTED_PREFIXES 匹配 + 分隔符边界
    let prog_data_lower = std::env::var("PROGRAMDATA").unwrap_or_default().to_lowercase();
    PROTECTED_PREFIXES.iter().any(|p| {
        let expanded = p.replace("%SYSTEMDRIVE%", &d)
            .replace("%PROGRAMDATA%", &prog_data_lower);
        let p_trimmed = expanded.trim_end_matches('\\').to_lowercase();
        let pt_len = p_trimmed.len();
        on_c.starts_with(&p_trimmed)
            && (on_c.len() == pt_len
                || on_c.as_bytes().get(pt_len) == Some(&b'\\'))
    }) || cleaned == format!("{d}\\")
        || path.parent().is_none()
        // SleepStudy: 目录本身受保护（代码级，不在 PROTECTED_PREFIXES）
        || cleaned == format!("c:\\windows\\system32\\sleepstudy")
        || cleaned.trim_end_matches('\\') == format!("c:\\windows\\system32\\sleepstudy")
}

/// 验证路径是否在允许的扫描目标内（含分隔符边界 + Win32 ns 剥离）
pub fn is_path_allowed(path: &Path, targets: &[ScanTarget]) -> bool {
    let raw = path.to_string_lossy().to_lowercase();
    // 剥离 Win32 命名空间前缀，与 is_path_protected / canonicalize 行为一致
    let raw = raw.trim_start_matches("\\\\?\\");
    let raw = raw.trim_start_matches("\\\\.\\");
    let raw = raw.trim_start_matches("//?/");
    let path_str = raw.trim_end_matches(&[' ', '.'][..]);
    targets.iter().any(|t| {
        let expanded = expand_env(&t.path).to_lowercase();
        let trimmed = expanded.trim_end_matches('\\');
        path_str.starts_with(trimmed)
            && (path_str.len() == trimmed.len()
                || path_str.as_bytes().get(trimmed.len()) == Some(&b'\\'))
    })
}

/// 验证环境变量展开的路径仍在预期前缀内
pub fn verify_env_path(expanded: &Path, raw_pattern: &str) -> bool {
    match std::fs::canonicalize(expanded) {
        Ok(canon) => verify_env_path_inner(&canon, raw_pattern),
        Err(_) => {
            tracing::warn!("env path does not exist: {}", expanded.display());
            false
        }
    }
}

fn verify_env_path_inner(canon: &Path, raw_pattern: &str) -> bool {
    let expected = expand_env(raw_pattern).to_lowercase();
    let raw = canon.to_string_lossy();
    let raw = raw.trim_start_matches("\\\\?\\");
    let raw = raw.trim_start_matches("\\\\.\\");
    let raw = raw.trim_start_matches("//?/");
    let can_str = raw.to_lowercase();
    let exp_trimmed = expected.trim_end_matches('\\');
    can_str.starts_with(exp_trimmed)
}

/// 解析浏览器 profile 目录
fn resolve_browser_profiles(base: &Path, cfg: &BrowserProfileConfig) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(base) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let matched: Vec<PathBuf> = entries.filter_map(|e| e.ok())
        .filter(|e| {
            let fname = e.file_name();
            let name = fname.to_string_lossy();
            cfg.profile_patterns.iter().any(|p| name.contains(p) || name.ends_with(p))
                && !is_reparse_point(e)
        })
        .map(|e| e.path())
        .collect();
    matched.iter().flat_map(|dir| {
        cfg.cache_subdirs.iter().map(move |sub| dir.join(sub))
    }).filter(|p| p.exists()).collect()
}

/// 解析 UWP 包目录（限 MAX_UWP_PACKAGES，跳过 junction）
fn is_reparse_point(entry: &std::fs::DirEntry) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        entry.metadata().map(|m| {
            // FILE_ATTRIBUTE_REPARSE_POINT = 0x400
            (m.file_attributes() & 0x400) != 0
        }).unwrap_or(false)
    }
    #[cfg(not(windows))]
    { false }
}

fn resolve_uwp_packages() -> Vec<PathBuf> {
    let packages_dir = expand_env("%LOCALAPPDATA%\\Packages");
    let dir = match std::fs::read_dir(&packages_dir) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    dir.filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && !is_reparse_point(e)
        })
        .take(MAX_UWP_PACKAGES)
        .map(|e| e.path())
        .filter(|p| !is_path_protected(p))
        .collect()
}

/// 展开扫描目标为实际路径列表（同路径去重，返回索引）
pub fn resolve_targets(targets: &[ScanTarget]) -> Vec<(PathBuf, usize)> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for (idx, t) in targets.iter().enumerate() {
        if t.level == SafetyLevel::Forbidden { continue; }
        let expanded = expand_env(&t.path);
        let p = PathBuf::from(&expanded);

        let safe_path = match std::fs::canonicalize(&p) {
            Ok(p) => p,
            Err(_) => p,
        };
        if is_path_protected(&safe_path) { continue; }
        // 验证展开路径在预期前缀内
        let path_ok = match std::fs::canonicalize(&safe_path) {
            Ok(canon) => verify_env_path_inner(&canon, &t.path),
            Err(_) => verify_env_path_inner(&safe_path, &t.path), // fallback: 用非 canonical 路径
        };
        if !path_ok { continue; }

        // Firefox 浏览器 profile 展开
        if let Some(browser_cfg) = &t.browser_profiles {
            for profile_path in resolve_browser_profiles(&safe_path, browser_cfg) {
                if seen.insert(profile_path.clone()) {
                    result.push((profile_path, idx));
                }
            }
            continue;
        }

        // UWP 通配路径
        if t.id.starts_with("uwp_") {
            for uwp_path in resolve_uwp_packages() {
                let sub = match t.id {
                    "uwp_temp" => "AC\\Temp",
                    "uwp_inet_cache" => "AC\\INetCache",
                    "uwp_local_cache" => "LocalCache",
                    _ => continue,
                };
                let full = uwp_path.join(sub);
                if full.exists() && !is_path_protected(&full) && seen.insert(full.clone()) {
                    result.push((full, idx));
                }
            }
            continue;
        }

        // 普通路径
        if seen.insert(safe_path.clone()) {
            result.push((safe_path, idx));
        }
    }
    result
}

/// 配置迁移 v1→v2
pub fn migrate_v1_to_v2(mut config: PonyConfig) -> PonyConfig {
    let path_to_id: HashMap<&str, &str> = [
        ("%TEMP%", "user_temp"),
        ("%LOCALAPPDATA%\\Temp", "local_temp"),
        ("%WINDIR%\\Temp", "sys_temp"),
        ("%WINDIR%\\Prefetch", "prefetch"),
        ("%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Code Cache", "chrome_code_cache"),
        ("%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Cache", "chrome_cache"),
        ("%APPDATA%\\Mozilla\\Firefox\\Profiles", "firefox_cache"),
        ("%WINDIR%\\SoftwareDistribution\\Download", "wu_download"),
        ("%LOCALAPPDATA%\\Microsoft\\Windows\\INetCache", "inet_cache"),
    ].into();
    for old_path in &config.disabled_targets {
        if let Some(id) = path_to_id.get(old_path.as_str()) {
            if !config.disabled_target_ids.contains(&id.to_string()) {
                config.disabled_target_ids.push(id.to_string());
            }
        }
    }
    config.disabled_targets.clear();
    config.version = Some(2);
    config
}

/// 加载用户配置（自动迁移 v1→v2）
pub fn load_config() -> PonyConfig {
    let path = config_path();
    let mut config: PonyConfig = fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if config.version.unwrap_or(1) < 2 {
        config = migrate_v1_to_v2(config);
    }
    config
}

/// 全局扫描防重入锁
static SCAN_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// 启动 C盘扫描任务
///
/// 扫描已 resolve 的安全路径，通过 mpsc 推送 ScanEvent。
/// 支持防重入（使用 AtomicBool 守卫）。
/// 返回 (cmd_tx, cancel_token)。
/// 启动 C盘扫描任务
pub fn start_scan(
    tx: mpsc::Sender<ScanEvent>,
) -> Result<(mpsc::Sender<CleanCommand>, CancellationToken), String> {
    if SCAN_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err("Scan already in progress".into());
    }

    let (cmd_tx, cmd_rx) = mpsc::channel::<CleanCommand>();
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    let config = load_config();
    let targets = get_filtered_targets(&config);
    let resolved = resolve_targets(&targets);
    if resolved.is_empty() {
        SCAN_IN_PROGRESS.store(false, Ordering::SeqCst);
        return Err("No scan targets available".into());
    }

    tokio::task::spawn_blocking(move || {
        struct ScanGuard;
        impl Drop for ScanGuard {
            fn drop(&mut self) { SCAN_IN_PROGRESS.store(false, Ordering::SeqCst); }
        }
        let _guard = ScanGuard;

        let _ = tx.send(ScanEvent::Progress { scanned: 0, current: "Starting scan...".into() });

        let mut total_items = 0u64;
        let mut total_bytes = 0u64;
        let mut skipped_small = 0u64;
        let mut hit_max = false;
        let mut batch = Vec::with_capacity(BATCH_SIZE);

        'outer: for (target_path, target_idx) in &resolved {
            let target_def = &targets[*target_idx];
            if cancel_token_clone.is_cancelled() { flush_and_cancel(&tx, &mut batch); return; }

            let cat_min_size = target_def.min_size;
            let glob_inc_static: Option<&'static [&'static str]> = target_def.glob_include;
            let needs_mtime = target_def.category == Category::Logs
                || target_def.id == "downloads_old"
                || target_def.id == "etl_logs"
                || target_def.id == "app_logs";
            let mtime_cutoff = if needs_mtime {
                Some(chrono_placeholder_now() - LOG_EXPIRY_DAYS * 86400)
            } else {
                None
            };
            let mut target_count = 0u64;

            let walk_dir = jwalk::WalkDir::new(target_path)
                .follow_links(false)
                .max_depth(target_def.max_depth)
                .process_read_dir(|depth, _path, _state, children| {
                    if depth.unwrap_or(0) > 5 {
                        children.retain(|e| {
                            e.as_ref().ok().is_none_or(|entry| {
                                let name = entry.file_name.to_string_lossy();
                                !(name == "node_modules" || name == ".git" || name == "__pycache__" || name == ".svn")
                            })
                        });
                    }
                    children.retain(|e| e.is_ok());
                });

            for entry in walk_dir.into_iter().filter_map(|e| e.ok()) {
                if cancel_token_clone.is_cancelled() { flush_and_cancel(&tx, &mut batch); return; }
                if !entry.file_type().is_file() { continue; }

                let Ok(meta) = entry.metadata() else { continue; };
                let size = meta.len();
                if size < cat_min_size { skipped_small += 1; continue; }

                // mtime 过滤（logs 类别）
                if let Some(cutoff) = mtime_cutoff {
                    if let Ok(mtime) = meta.modified() {
                        if let Ok(secs) = mtime.duration_since(std::time::UNIX_EPOCH) {
                            if secs.as_secs() as i64 > cutoff { continue; }
                        }
                    }
                }

                // glob_include 过滤
                if let Some(inc) = glob_inc_static {
                    let fname = entry.file_name().to_string_lossy();
                    if !inc.iter().any(|p| {
                        let p_trimmed = p.trim_start_matches('*');
                        fname.ends_with(p_trimmed)
                    }) { continue; }
                }

                if target_count >= target_def.max_items_per_target {
                    let _ = tx.send(ScanEvent::Warning(ScanWarning::MaxItemsReached {
                        target_id: target_def.id.into(), items: target_count,
                    }));
                    break;
                }
                if total_items >= MAX_SCAN_ITEMS {
                    if !batch.is_empty() {
                        let _ = tx.send(ScanEvent::ItemsFound { items: std::mem::take(&mut batch), batch_complete: false });
                    }
                    hit_max = true;
                    break 'outer;
                }

                total_items += 1; total_bytes += size; target_count += 1;
                batch.push(CleanItem {
                    path: entry.path(), size_bytes: size,
                    level: target_def.level.clone(),
                    category: target_def.category.to_string(),
                });

                if batch.len() >= BATCH_SIZE {
                    let _ = tx.send(ScanEvent::ItemsFound { items: std::mem::take(&mut batch), batch_complete: false });
                }
                if total_items % 100 == 0 {
                    let _ = tx.send(ScanEvent::Progress { scanned: total_items, current: target_path.to_string_lossy().to_string() });
                }
            }
        }

        if !batch.is_empty() {
            let _ = tx.send(ScanEvent::ItemsFound { items: batch, batch_complete: true });
        }
        if hit_max {
            let _ = tx.send(ScanEvent::Warning(ScanWarning::MaxItemsReached {
                target_id: "global".into(), items: MAX_SCAN_ITEMS,
            }));
        }
        let _ = tx.send(ScanEvent::Done { total_items, total_bytes, skipped_small });
    });

    let cancel_token_cmd = cancel_token.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CleanCommand::CancelScan => cancel_token_cmd.cancel(),
                CleanCommand::Shutdown => break,
                _ => {}
            }
        }
    });

    Ok((cmd_tx, cancel_token))
}

fn flush_and_cancel(tx: &mpsc::Sender<ScanEvent>, batch: &mut Vec<CleanItem>) {
    if !batch.is_empty() {
        let _ = tx.send(ScanEvent::ItemsFound { items: std::mem::take(batch), batch_complete: false });
    }
    let _ = tx.send(ScanEvent::Cancelled);
}

fn chrono_placeholder_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// 删除文件（阻塞调用，使用 spawn_blocking 执行）
///
/// 降级策略：
/// 1. DeleteFileW — 立即删除
/// 2. MoveFileExW + MOVEFILE_DELAY_UNTIL_REBOOT — 被占用时降级
/// 3. 跳过
pub fn delete_files(paths: &[PathBuf]) -> DeleteResult {
    delete_files_with_progress(paths, None)
}

/// 删除文件并推送进度（供 Tauri 命令层使用）
pub fn delete_files_with_progress(
    paths: &[PathBuf],
    progress_tx: Option<mpsc::Sender<DeleteProgress>>,
) -> DeleteResult {
    let targets = get_clean_targets();
    let mut result = DeleteResult::default();
    let total = paths.len() as u64;
    let mut done = 0u64;

    for path in paths {
        // 规范化路径防止 .. 遍历和正斜杠绕过
        let safe_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(e) => {
                result.failed += 1;
                done += 1;
                result
                    .errors
                    .push(format!("Cannot resolve path {}: {e}", path.display()));
                send_progress(&progress_tx, done, total, path);
                continue;
            }
        };

        // 后端强制执行安全验证
        if is_path_protected(&safe_path) {
            result.failed += 1;
            done += 1;
            result
                .errors
                .push(format!("Protected path: {}", safe_path.display()));
            send_progress(&progress_tx, done, total, path);
            continue;
        }
        if !is_path_allowed(&safe_path, &targets) {
            result.failed += 1;
            done += 1;
            result
                .errors
                .push(format!("Path not in scan scope: {}", safe_path.display()));
            send_progress(&progress_tx, done, total, path);
            continue;
        }

        match std::fs::remove_file(&safe_path) {
            Ok(()) => result.success += 1,
            Err(e) => {
                // 尝试延迟删除
                if cfg!(windows) {
                    match delete_file_delayed_windows(&safe_path) {
                        Ok(()) => result.success += 1,
                        Err(msg) => {
                            result.failed += 1;
                            result.errors.push(msg);
                        }
                    }
                } else {
                    result.failed += 1;
                    result.errors.push(format!("{e}"));
                }
            }
        }
        done += 1;
        if done % 10 == 0 || done == total {
            send_progress(&progress_tx, done, total, path);
        }
    }

    result
}

fn send_progress(tx: &Option<mpsc::Sender<DeleteProgress>>, done: u64, total: u64, current: &Path) {
    if let Some(tx) = tx {
        let _ = tx.send(DeleteProgress {
            done,
            total,
            current: current.to_string_lossy().to_string(),
        });
    }
}

#[cfg(windows)]
fn delete_file_delayed_windows(path: &Path) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::GetLastError;
    use windows::Win32::Storage::FileSystem::MOVEFILE_DELAY_UNTIL_REBOOT;
    use windows::Win32::Storage::FileSystem::MoveFileExW;

    // 拒绝含空字节的路径（防止 API 截断）
    let path_str = path.to_string_lossy();
    if path_str.contains('\0') {
        return Err(format!("Path contains null byte: {}", path.display()));
    }

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let result = unsafe {
        MoveFileExW(
            windows::core::PCWSTR(wide.as_ptr()),
            None,
            MOVEFILE_DELAY_UNTIL_REBOOT,
        )
    };
    if result.is_ok() {
        Ok(())
    } else {
        let err_code = unsafe { GetLastError() };
        Err(format!(
            "MoveFileExW failed for {} (error: {err_code:?})",
            path.display()
        ))
    }
}

/// 清空回收站（需在 COM 初始化的线程上调用）
pub fn empty_recycle_bin() -> Result<(), String> {
    #[cfg(not(windows))]
    return Err("Recycle bin is only supported on Windows".into());

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{S_FALSE, S_OK};
        use windows::Win32::System::Com::{
            COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
        };
        use windows::Win32::UI::Shell::{
            SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHEmptyRecycleBinW,
        };

        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);

            // 检查 COM 初始化是否成功（S_OK=成功, S_FALSE=已初始化）
            if hr != S_OK && hr != S_FALSE {
                return Err(format!("COM init failed with unexpected HRESULT: {hr:?}"));
            }

            let result = SHEmptyRecycleBinW(None, None, SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI);

            if hr == S_OK {
                CoUninitialize();
            }

            if result.is_ok() {
                Ok(())
            } else {
                Err(format!("EmptyRecycleBin failed: {result:?}"))
            }
        }
    }
}

// ===================== 操作日志 =====================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanLogEntry {
    pub timestamp: String,
    pub total_files: u64,
    pub total_bytes: u64,
    pub success: u64,
    pub failed: u64,
    pub errors: Vec<String>,
    pub by_category: HashMap<String, CategorySummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategorySummary {
    pub files: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CleanStats {
    pub total_files: u64,
    pub total_bytes: u64,
    pub last_cleaned_at: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CleanLogSummary {
    pub entries: Vec<CleanLogEntry>,
    pub total_cleaned_files: u64,
    pub total_cleaned_bytes: u64,
}

const CLEAN_LOG_FILE: &str = "clean_log.jsonl";
const STATS_FILE: &str = "clean_stats.json";
const MAX_LOG_BYTES: u64 = 1_048_576;
const MAX_LOG_BACKUPS: u32 = 5;
static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn timestamp_now() -> String {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();

    // 正确 UTC 日期计算（不含 chrono 依赖）
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let h = time_secs / 3600;
    let m = (time_secs % 3600) / 60;
    let s = time_secs % 60;

    // 从 1970-01-01 起算天数 → 年月日
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) { &LEAP_MONTH_DAYS[..] } else { &NORM_MONTH_DAYS[..] };
    let mut mo = 1u32;
    for md in month_days {
        if remaining < *md as i64 { break; }
        remaining -= *md as i64;
        mo += 1;
    }
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z", d = remaining as u32 + 1)
}

const NORM_MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const LEAP_MONTH_DAYS: [u64; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// 追加清理日志到 JSONL 文件
///
/// TODO: add DPAPI encryption - currently plaintext.
pub fn append_clean_log(entry: &CleanLogEntry) -> Result<(), String> {
    append_clean_log_at(entry, &data_dir())
}

fn append_clean_log_at(entry: &CleanLogEntry, dir: &Path) -> Result<(), String> {
    let _lock = LOG_LOCK.lock().map_err(|e| format!("log lock: {e}"))?;
    fs::create_dir_all(dir).map_err(|e| format!("log dir: {e}"))?;
    let path = dir.join(CLEAN_LOG_FILE);
    rotate_if_needed(&path);
    let json = serde_json::to_string(entry).map_err(|e| format!("serialize log: {e}"))?;
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path)
        .map_err(|e| format!("open log: {e}"))?;
    use std::io::Write;
    writeln!(file, "{json}").map_err(|e| format!("write log: {e}"))?;
    update_stats_at(dir, entry.total_files, entry.total_bytes);
    Ok(())
}

fn rotate_if_needed(path: &Path) {
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size <= MAX_LOG_BYTES { return; }
    for i in (1..MAX_LOG_BACKUPS).rev() {
        let old = path.with_extension(format!("{i}.jsonl"));
        let new = path.with_extension(format!("{}.jsonl", i + 1));
        if old.exists() { let _ = fs::rename(&old, &new); }
    }
    let first = path.with_extension("1.jsonl");
    let _ = fs::rename(path, &first);
}

pub fn get_clean_logs(limit: usize) -> Result<CleanLogSummary, String> {
    get_clean_logs_at(limit, &data_dir())
}

fn get_clean_logs_at(limit: usize, dir: &Path) -> Result<CleanLogSummary, String> {
    let path = dir.join(CLEAN_LOG_FILE);
    let entries = if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        content.lines().rev().take(limit)
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    } else {
        vec![]
    };
    let stats = fs::read_to_string(dir.join(STATS_FILE)).ok()
        .and_then(|s| serde_json::from_str::<CleanStats>(&s).ok())
        .unwrap_or_default();
    Ok(CleanLogSummary { entries, total_cleaned_files: stats.total_files, total_cleaned_bytes: stats.total_bytes })
}

fn update_stats_at(dir: &Path, files: u64, bytes: u64) {
    let path = dir.join(STATS_FILE);
    let mut stats = fs::read_to_string(&path).ok()
        .and_then(|s| serde_json::from_str::<CleanStats>(&s).ok())
        .unwrap_or_default();
    stats.total_files += files;
    stats.total_bytes += bytes;
    stats.last_cleaned_at = timestamp_now();
    if let Ok(json) = serde_json::to_string(&stats) {
        let _ = fs::write(&path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_drive_returns_valid() {
        let d = system_drive();
        assert_eq!(d.len(), 2, "system drive should be like 'C:'");
        assert!(d.ends_with(':'), "should end with colon");
    }

    #[test]
    fn test_expand_temp() {
        let result = expand_env("%TEMP%\\test");
        assert!(!result.contains("%TEMP%"), "TEMP should be expanded");
        assert!(result.ends_with("\\test"), "suffix should be preserved");
    }

    #[test]
    fn test_expand_windir() {
        let result = expand_env("%WINDIR%\\Temp");
        assert!(!result.contains("%WINDIR%"), "WINDIR should be expanded");
        assert!(result.ends_with("\\Temp"), "suffix should be preserved");
    }

    #[test]
    fn test_expand_localappdata() {
        let result = expand_env("%LOCALAPPDATA%\\test");
        assert!(!result.contains("%LOCALAPPDATA%"));
    }

    #[test]
    fn test_expand_systemdrive() {
        let result = expand_env("%SYSTEMDRIVE%\\Test");
        assert!(!result.contains("%SYSTEMDRIVE%"));
        assert!(result.ends_with("\\Test"));
    }

    #[test]
    fn test_expand_no_vars() {
        assert_eq!(expand_env("plain path"), "plain path");
    }

    #[test]
    fn test_is_path_protected_system32_config() {
        // config 子路径应受保护
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\config\SAM"
        )));
        // System32 根不再有宽保护，但关键子路径受保护
        assert!(!is_path_protected(Path::new(r"C:\Windows\System32\LogFiles\some.log")),
            "LogFiles 应允许扫描");
    }

    #[test]
    fn test_is_path_protected_installer() {
        assert!(is_path_protected(Path::new(
            r"C:\Windows\Installer\some.msi"
        )));
    }

    #[test]
    fn test_is_path_protected_winsxs() {
        assert!(is_path_protected(Path::new(r"C:\Windows\WinSxS\amd64_foo")));
    }

    #[test]
    fn test_is_path_protected_tasks() {
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\Tasks\SomeTask")));
    }

    #[test]
    fn test_is_path_protected_program_files() {
        assert!(is_path_protected(Path::new(r"C:\Program Files\SomeApp")));
        assert!(is_path_protected(Path::new(
            r"C:\Program Files (x86)\SomeApp"
        )));
    }

    #[test]
    fn test_is_path_protected_windows_apps() {
        assert!(is_path_protected(Path::new(
            r"C:\Program Files\WindowsApps\SomePkg"
        )));
    }

    #[test]
    fn test_is_path_not_protected() {
        let temp = std::env::var("TEMP").unwrap_or_else(|_| r"C:\Temp".to_string());
        assert!(!is_path_protected(Path::new(&temp)));
    }

    #[test]
    fn test_is_path_allowed_valid() {
        let targets = vec![ScanTarget::new("t", "%TEMP%", SafetyLevel::Safe, Category::Temp, "")];
        let temp = std::env::var("TEMP").unwrap();
        let test_path = PathBuf::from(&temp).join("test.txt");
        assert!(is_path_allowed(&test_path, &targets));
    }

    #[test]
    fn test_is_path_allowed_rejected() {
        let targets = vec![ScanTarget::new("t", "%TEMP%", SafetyLevel::Safe, Category::Temp, "")];
        assert!(!is_path_allowed(
            Path::new(r"C:\Windows\System32\test.dll"),
            &targets,
        ));
    }

    #[test]
    fn test_is_path_allowed_cache_category() {
        let targets = vec![ScanTarget::new("t", "%LOCALAPPDATA%\\Google\\Chrome", SafetyLevel::Safe, Category::Cache, "")];
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let test_path = PathBuf::from(&local).join("Google\\Chrome\\Cache\\f_000001");
        assert!(is_path_allowed(&test_path, &targets));
    }

    #[test]
    fn test_resolve_targets_excludes_forbidden() {
        let targets = vec![
            ScanTarget::new("temp", "%TEMP%", SafetyLevel::Safe, Category::Temp, ""),
            ScanTarget::new("sys32", "C:\\Windows\\System32", SafetyLevel::Forbidden, Category::Temp, ""),
        ];
        let resolved = resolve_targets(&targets);
        // 应包含指向 TEMP 的路径（Safe）
        assert!(!resolved.is_empty(), "should include TEMP");
        // 应排除 System32（Forbidden）
        assert!(
            !resolved.iter().any(|(p, _)| p.to_string_lossy().to_lowercase().contains("system32")),
            "should NOT include System32"
        );
    }

    #[test]
    fn test_resolve_targets_skips_protected() {
        let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        // 使用 config（在 PROTECTED_PREFIXES 中）作为受保护路径
        let protected = format!("{}\\System32\\config", sys_root);
        let targets = vec![ScanTarget::new("p", &protected, SafetyLevel::Safe, Category::Temp, "")];
        let resolved = resolve_targets(&targets);
        assert!(resolved.is_empty(), "protected path should be skipped");
    }

    #[test]
    fn test_safety_level_ordering() {
        assert_ne!(SafetyLevel::Safe, SafetyLevel::Forbidden);
        assert_eq!(SafetyLevel::Safe, SafetyLevel::Safe);
    }

    #[test]
    fn test_get_clean_targets_has_entries() {
        let targets = get_clean_targets();
        assert!(!targets.is_empty(), "should have at least one target");
        assert!(targets.iter().any(|t| t.level != SafetyLevel::Forbidden));
    }

    #[test]
    fn test_delete_result_default() {
        let r = DeleteResult::default();
        assert_eq!(r.success, 0);
        assert_eq!(r.failed, 0);
        assert!(r.errors.is_empty());
    }

    #[test]
    fn test_delete_result_construction() {
        let r = DeleteResult {
            success: 5,
            failed: 2,
            errors: vec!["access denied".into()],
        };
        assert_eq!(r.success, 5);
        assert_eq!(r.failed, 2);
        assert_eq!(r.errors.len(), 1);
    }

    #[test]
    fn test_clean_item_construction() {
        let item = CleanItem {
            path: PathBuf::from(r"C:\Temp\test.tmp"),
            size_bytes: 1024,
            level: SafetyLevel::Safe,
            category: "temp".into(),
        };
        assert_eq!(item.size_bytes, 1024);
        assert_eq!(item.category, "temp");
    }

    #[test]
    fn test_scan_event_variants() {
        let p = ScanEvent::Progress {
            scanned: 100,
            current: "scanning...".into(),
        };
        match p {
            ScanEvent::Progress { scanned, .. } => assert_eq!(scanned, 100),
            _ => panic!("wrong variant"),
        }

        let d = ScanEvent::Done {
            total_items: 1000,
            total_bytes: 50_000_000,
            skipped_small: 42,
        };
        match d {
            ScanEvent::Done { total_items, skipped_small, .. } => {
                assert_eq!(total_items, 1000);
                assert_eq!(skipped_small, 42);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_clean_command_variants() {
        match CleanCommand::EmptyRecycleBin {
            CleanCommand::EmptyRecycleBin => {}
            _ => panic!("wrong variant"),
        }
        match CleanCommand::CancelScan {
            CleanCommand::CancelScan => {}
            _ => panic!("wrong variant"),
        }
    }

    // ===== S1 新增测试 =====
    #[test]
    fn test_new_targets_resolve() {
        let targets = get_clean_targets();
        assert_eq!(targets.len(), 43, "should have 43 targets");
        // 检查一些特定目标存在
        assert!(targets.iter().any(|t| t.id == "sys_logfiles"));
        assert!(targets.iter().any(|t| t.id == "uwp_temp"));
        assert!(targets.iter().any(|t| t.id == "downloads_old"));
    }

    #[test]
    fn test_target_ids_unique() {
        let ids: Vec<&str> = get_clean_targets().iter().map(|t| t.id).collect();
        let mut sorted = ids.clone(); sorted.sort(); sorted.dedup();
        assert_eq!(ids.len(), sorted.len(), "target ids must be unique");
    }

    #[test]
    fn test_category_serde_roundtrip() {
        assert_eq!(serde_json::to_value(&Category::Temp).unwrap(), serde_json::json!("temp"));
        assert_eq!(serde_json::to_value(&Category::Cache).unwrap(), serde_json::json!("cache"));
        assert_eq!(serde_json::to_value(&Category::Logs).unwrap(), serde_json::json!("logs"));
        assert_eq!(serde_json::to_value(&Category::Prefetch).unwrap(), serde_json::json!("prefetch"));
        assert_eq!(serde_json::to_value(&Category::RecycleBin).unwrap(), serde_json::json!("recycle_bin"));
        assert_eq!(serde_json::to_value(&Category::OldInstall).unwrap(), serde_json::json!("old_install"));
    }

    #[test]
    fn test_category_display() {
        assert_eq!(Category::Temp.to_string(), "temp");
        assert_eq!(Category::OldInstall.to_string(), "old_install");
    }

    #[test]
    fn test_min_size_per_category() {
        assert_eq!(Category::Cache.default_min_size(), 512);
        assert_eq!(Category::Logs.default_min_size(), 4096);
        assert_eq!(Category::Temp.default_min_size(), 1024);
    }

    #[test]
    fn test_scan_warning_enum_variants() {
        let w1 = ScanWarning::MaxItemsReached { target_id: "t".into(), items: 100 };
        let w2 = ScanWarning::PermissionDenied { target_id: "t".into(), path: "p".into() };
        match w1 { ScanWarning::MaxItemsReached { .. } => {} _ => panic!() }
        match w2 { ScanWarning::PermissionDenied { .. } => {} _ => panic!() }
    }

    #[test]
    fn test_is_path_protected_separator() {
        // 关键子路径受保护
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\config\SAM")));
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\Tasks\test")));
    }

    #[test]
    fn test_protected_winevt_logs() {
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\winevt\Logs\Security.evtx")));
    }

    #[test]
    fn test_protected_sleepstudy_dir() {
        // SleepStudy 目录本身由 is_path_protected 中的特殊代码保护
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\sleepstudy")));
    }

    #[test]
    fn test_sleepstudy_subfile_not_protected() {
        // 子文件不再受 System32 总前缀保护
        assert!(!is_path_protected(Path::new(r"C:\Windows\System32\sleepstudy\sub.etl")));
    }

    #[test]
    #[test]
    fn test_is_path_allowed_trailing_slash() {
        let targets = vec![ScanTarget::new("t", "%TEMP%\\", SafetyLevel::Safe, Category::Temp, "")];
        let temp = std::env::var("TEMP").unwrap_or_default();
        if !temp.is_empty() {
            let sub = PathBuf::from(temp.trim_end_matches('\\')).join("test.tmp");
            assert!(is_path_allowed(&sub, &targets), "trailing \\ in expanded should work");
        }
    }

    #[test]
    fn test_get_filtered_targets_uses_id() {
        let mut config = PonyConfig::default();
        config.disabled_target_ids.push("firefox_cache".into());
        config.disabled_target_ids.push("wu_download".into());
        let filtered = get_filtered_targets(&config);
        assert!(!filtered.iter().any(|t| t.id == "firefox_cache"));
        assert!(!filtered.iter().any(|t| t.id == "wu_download"));
        assert!(filtered.iter().any(|t| t.id == "user_temp"));
    }

    #[test]
    fn test_config_migration_v1_to_v2() {
        let v1 = PonyConfig {
            disabled_targets: vec!["%TEMP%".into(), "%WINDIR%\\Temp".into()],
            custom_exclude_paths: vec!["C:\\MyData".into()],
            ..Default::default()
        };
        let v2 = migrate_v1_to_v2(v1);
        assert_eq!(v2.version, Some(2));
        assert!(v2.disabled_target_ids.contains(&"user_temp".to_string()));
        assert!(v2.disabled_target_ids.contains(&"sys_temp".to_string()));
        assert!(v2.custom_exclude_paths.contains(&"C:\\MyData".to_string()));
        assert!(v2.disabled_targets.is_empty());
    }

    #[test]
    fn test_resolve_targets_dedup_same_path() {
        let targets = vec![
            ScanTarget::new("a", "%TEMP%", SafetyLevel::Safe, Category::Temp, ""),
            ScanTarget::new("b", "%TEMP%", SafetyLevel::Safe, Category::Temp, ""), // 同路径
        ];
        let resolved = resolve_targets(&targets);
        let paths: std::collections::HashSet<_> = resolved.iter().map(|(p, _)| p).collect();
        assert_eq!(paths.len(), resolved.len(), "no duplicate paths");
    }

    #[test]
    fn test_clean_log_append_and_read() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let entry = CleanLogEntry {
            timestamp: "2026-01-01T00:00:00Z".into(),
            total_files: 5, total_bytes: 10240,
            success: 5, failed: 0, errors: vec![],
            by_category: HashMap::new(),
        };
        append_clean_log_at(&entry, dir.path()).unwrap();
        let summary = get_clean_logs_at(10, dir.path()).unwrap();
        assert_eq!(summary.entries.len(), 1);
        assert_eq!(summary.entries[0].success, 5);
    }

    #[test]
    fn test_expand_alluserprofile() {
        let val = std::env::var("ALLUSERSPROFILE").ok();
        if val.is_some() {
            let result = expand_env("%ALLUSERSPROFILE%\\test");
            assert!(!result.contains("%ALLUSERSPROFILE%"));
            assert!(result.ends_with("\\test"));
        }
    }

    #[test]
    fn test_expand_case_insensitive() {
        temp_env::with_var("TEMP", Some("C:\\TestTemp"), || {
            assert_eq!(expand_env("%temp%\\foo"), r"C:\TestTemp\foo");
            assert_eq!(expand_env("%Temp%\\foo"), r"C:\TestTemp\foo");
        });
    }

    #[test]
    fn test_expand_trailing_backslash_fixed() {
        temp_env::with_var("TEMP", Some("C:\\TestTemp"), || {
            let r = expand_env("%TEMP%\\");
            assert!(!r.starts_with('\\'), "should not start with \\");
        });
    }

    #[test]
    fn test_protected_trailing_space() {
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\config\SAM ")));
    }

    #[test]
    fn test_protected_trailing_dot() {
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\config.")));
    }

    #[test]
    fn test_protected_forward_slash() {
        assert!(is_path_protected(Path::new(r"C:/Windows/System32/config/SAM")));
    }

    #[test]
    fn test_protected_win32_namespace() {
        assert!(is_path_protected(Path::new(r"\\?\C:\Windows\System32\config\SAM")));
    }

    #[test]
    fn test_protected_system_volume_information() {
        assert!(is_path_protected(Path::new(r"C:\System Volume Information\some")));
    }

    #[test]
    fn test_allowed_separator_reject_adjacent() {
        temp_env::with_var("TEMP", Some("C:\\Temp"), || {
            let targets = vec![ScanTarget::new("t", "%TEMP%", SafetyLevel::Safe, Category::Temp, "")];
            assert!(!is_path_allowed(Path::new(r"C:\Temp_malicious\evil.exe"), &targets));
        });
    }

    #[test]
    fn test_downloads_mtime_filters_recent() {
        let targets = get_clean_targets();
        let dl = targets.iter().find(|t| t.id == "downloads_old").unwrap();
        assert_eq!(dl.min_size, 102_400);
    }

    #[test]
    fn test_category_default_min_size() {
        assert_eq!(Category::Cache.default_min_size(), 512);
        assert_eq!(Category::Logs.default_min_size(), 4096);
        assert_eq!(Category::Temp.default_min_size(), 1024);
        assert_eq!(Category::Prefetch.default_min_size(), 1024);
        assert_eq!(Category::RecycleBin.default_min_size(), 1024);
        assert_eq!(Category::OldInstall.default_min_size(), 1024);
    }

    #[test]
    fn test_data_dir() {
        let dir = data_dir();
        assert!(dir.to_string_lossy().contains("PonyClean"), "data_dir should end with PonyClean");
    }

    #[test]
    fn test_protected_globalroot_device() {
        assert!(is_path_protected(Path::new(r"\\.\GLOBALROOT\Device\HarddiskVolume1\Windows\System32\config\SAM")));
        assert!(is_path_protected(Path::new(r"\\?\GLOBALROOT\Device\Harddisk0\Partition1\Windows\System32")));
    }

    #[test]
    fn test_protected_null_byte_after_path() {
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\config\SAM\0..\..\Temp")));
    }

    #[test]
    fn test_protected_mixed_separators() {
        assert!(is_path_protected(Path::new(r"C:\Windows/System32\config/SAM")));
    }

    #[cfg_attr(not(windows), ignore)]
    #[serial_test::serial]
    #[test]
    fn test_env_injection_temp_is_protected() {
        temp_env::with_var("TEMP", Some(r"C:\Windows\System32\config"), || {
            let targets = get_clean_targets();
            let filtered: Vec<_> = targets.into_iter().filter(|t| t.id == "user_temp").collect();
            let resolved = resolve_targets(&filtered);
            assert!(resolved.is_empty(), "TEMP pointing to protected path must be rejected");
        });
    }
}
