use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
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
/// 扫描并行度（target 分组数，TASK-027）
const SCAN_PARALLELISM: usize = 4;

/// 安全级别
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafetyLevel {
    Safe,
    Confirm,
    Forbidden,
}

/// 清理目标分类，序列化为小写 JSON
/// 前端类型: type Category = 'temp' | 'cache' | 'logs' | 'prefetch' | 'old_install' | 'app_cache' | 'dev_cache'
/// （recycle_bin 枚举保留用于配置兼容，但 TASK-028 起不再作为扫描目标）
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Temp,
    Cache,
    Logs,
    Prefetch,
    RecycleBin,
    OldInstall,
    /// 应用缓存（Discord/Steam/微信/QQ/Electron 等）
    AppCache,
    /// 开发工具缓存（npm/pip/cargo/gradle 等）
    DevCache,
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
            Category::AppCache => "app_cache",
            Category::DevCache => "dev_cache",
        };
        write!(f, "{s}")
    }
}

/// 浏览器 profile 匹配配置
#[derive(Clone, Debug)]
pub struct BrowserProfileConfig {
    pub profile_patterns: Vec<String>,
    pub cache_subdirs: Vec<String>,
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
    MaxItemsReached {
        target_id: String,
        items: u64,
    },
    PermissionDenied {
        target_id: String,
        path: String,
    },
    GlobNoMatch {
        target_id: String,
        pattern: String,
    },
    ServiceStopFailed {
        target_id: String,
        service: String,
        reason: String,
    },
    EnvInjectionDetected {
        target_id: String,
        path: String,
    },
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
    pub id: String,
    pub path: String,
    pub level: SafetyLevel,
    pub category: Category,
    pub description: String,
    pub min_size: u64,
    pub max_items_per_target: u64,
    pub max_depth: usize,
    pub glob_include: Option<Vec<String>>,
    pub glob_exclude: Option<Vec<String>>,
    pub requires_service_stop: Option<String>,
    pub browser_profiles: Option<BrowserProfileConfig>,
}

impl ScanTarget {
    pub fn new(id: String, path: &str, level: SafetyLevel, cat: Category, desc: String) -> Self {
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
    pub fn with_min_size(mut self, v: u64) -> Self {
        self.min_size = v;
        self
    }
    /// 快捷设置最小文件大小（MB 单位）
    pub fn with_min_size_mb(mut self, mb: u64) -> Self {
        self.min_size = mb * 1_048_576;
        self
    }
    pub fn with_glob(mut self, inc: Vec<String>) -> Self {
        self.glob_include = Some(inc);
        self
    }
    pub fn with_glob_exclude(mut self, exc: Vec<String>) -> Self {
        self.glob_exclude = Some(exc);
        self
    }
    pub fn with_max_depth(mut self, v: usize) -> Self {
        self.max_depth = v;
        self
    }
    pub fn with_service_stop(mut self, s: String) -> Self {
        self.requires_service_stop = Some(s);
        self
    }
    pub fn with_browser(mut self, b: BrowserProfileConfig) -> Self {
        self.browser_profiles = Some(b);
        self
    }
}

/// 用户自定义清理目标
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomTarget {
    /// 唯一标识（与内置目标 id 冲突时忽略）
    pub id: String,
    /// 目录路径，支持 %ENV% 展开
    pub path: String,
    /// 安全级别（Forbidden 级别不会被扫描）
    pub level: SafetyLevel,
    /// 分类（决定默认勾选与最小文件大小）
    pub category: Category,
    pub description: String,
    pub enabled: bool,
}

/// 用户配置
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PonyConfig {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub disabled_target_ids: Vec<String>,
    #[serde(default)]
    pub disabled_targets: Vec<String>,
    #[serde(default)]
    pub custom_exclude_paths: Vec<String>,
    #[serde(default)]
    pub per_target_config: HashMap<String, TargetConfig>,
    #[serde(default)]
    pub custom_targets: Vec<CustomTarget>,
    /// 磁盘分析参数（TASK-028）：大文件阈值与目录占用分解层数，缺失时使用默认值
    #[serde(default)]
    pub disk_scan: Option<DiskScanConfig>,
}

/// 磁盘分析扫描参数（TASK-028），全部字段可选、缺失时取默认值
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiskScanConfig {
    /// 大文件最小体积（MB），None = 100，合法范围 50..=10000
    pub min_bytes_mb: Option<u64>,
    /// 目录占用分解层数，None = 3，合法范围 1..=5
    pub dir_depth: Option<usize>,
}

impl PonyConfig {
    /// 解析磁盘分析扫描参数为 (min_bytes_mb, dir_depth)，带 clamp 防手改配置文件恶意值
    pub fn disk_scan_params(&self) -> (u64, usize) {
        let mb = self
            .disk_scan
            .as_ref()
            .and_then(|d| d.min_bytes_mb)
            .unwrap_or(100);
        let depth = self
            .disk_scan
            .as_ref()
            .and_then(|d| d.dir_depth)
            .unwrap_or(3);
        (mb.clamp(50, 10_000), depth.clamp(1, 5))
    }
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
    let json =
        serde_json::to_string_pretty(config).map_err(|e| format!("Serialize config: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Write config: {e}"))
}

/// 获取过滤后的扫描目标（内置目标 + 用户自定义目标）
pub fn get_filtered_targets(config: &PonyConfig) -> Vec<ScanTarget> {
    let mut targets: Vec<ScanTarget> = get_clean_targets()
        .into_iter()
        .filter(|t| !config.disabled_target_ids.contains(&t.id))
        .filter(|t| {
            config
                .per_target_config
                .get(t.id.as_str())
                .and_then(|c| c.enabled)
                .unwrap_or(true)
        })
        .collect();

    // 合并用户自定义目标（跳过禁用、id 冲突、Forbidden 级别）
    for ct in &config.custom_targets {
        if !ct.enabled || ct.level == SafetyLevel::Forbidden {
            continue;
        }
        if targets.iter().any(|t| t.id == ct.id) {
            continue;
        }
        targets.push(ScanTarget {
            id: ct.id.clone(),
            path: ct.path.clone(),
            level: ct.level.clone(),
            category: ct.category.clone(),
            description: ct.description.clone(),
            min_size: ct.category.default_min_size(),
            max_items_per_target: MAX_ITEMS_PER_TARGET,
            max_depth: DEFAULT_MAX_DEPTH,
            glob_include: None,
            glob_exclude: None,
            requires_service_stop: None,
            browser_profiles: None,
        });
    }
    targets
}

pub fn config_dir() -> PathBuf {
    data_dir()
}
fn data_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\Default".into());
        format!("{home}\\AppData\\Local")
    });
    PathBuf::from(local).join("PonyClean")
}

fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

/// 获取系统盘符
fn system_drive() -> String {
    std::env::var("SYSTEMDRIVE")
        .ok()
        .or_else(|| {
            std::env::var("SystemRoot")
                .ok()
                .map(|r| r.get(..2).unwrap_or("C:").to_string())
        })
        .unwrap_or_else(|| "C:".to_string())
}

/// 展开环境变量
fn expand_env(raw: &str) -> String {
    // 缓存环境变量值，避免同一变量多次查询
    let mut cache: HashMap<String, String> = HashMap::new();
    let get_env = |name: &str, c: &mut HashMap<String, String>| -> Option<String> {
        if let Some(v) = c.get(name) {
            return Some(v.clone());
        }
        let val = std::env::var(name)
            .ok()
            .or_else(|| std::env::var(name.to_uppercase()).ok())
            .or_else(|| std::env::var(name.to_lowercase()).ok());
        if let Some(v) = val {
            c.insert(name.to_string(), v.clone());
            Some(v)
        } else {
            None
        }
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
                None => {
                    s.push('%');
                    s.push_str(var_name);
                    s.push('%');
                }
            }
            rest = &after_pct[end + 1..];
        } else {
            s.push('%');
            rest = after_pct;
        }
    }
    s.push_str(rest);
    if s.contains("%SYSTEMDRIVE%") {
        s = s.replace("%SYSTEMDRIVE%", &system_drive());
    }
    if s.starts_with('\\') {
        format!("{}{}", system_drive(), s)
    } else {
        s
    }
}

pub fn default_targets() -> Vec<ScanTarget> {
    get_clean_targets()
}

/// 安全扫描路径列表（54 个 target；TASK-028 起回收站不再作为扫描目标，
/// 唯一入口为 `empty_recycle_bin` 命令 + 前端确认弹窗）
pub fn get_clean_targets() -> Vec<ScanTarget> {
    let d = system_drive();
    vec![
        // === 已有 15 目标 ===
        ScanTarget::new(
            "user_temp".into(),
            "%TEMP%",
            SafetyLevel::Safe,
            Category::Temp,
            "用户临时文件".into(),
        ),
        ScanTarget::new(
            "local_temp".into(),
            "%LOCALAPPDATA%\\Temp",
            SafetyLevel::Safe,
            Category::Temp,
            "当前用户临时文件".into(),
        ),
        ScanTarget::new(
            "sys_temp".into(),
            "%WINDIR%\\Temp",
            SafetyLevel::Confirm,
            Category::Temp,
            "系统临时文件".into(),
        ),
        ScanTarget::new(
            "prefetch".into(),
            &format!("{d}\\Windows\\Prefetch"),
            SafetyLevel::Confirm,
            Category::Prefetch,
            "应用启动缓存".into(),
        ),
        ScanTarget::new(
            "chrome_code_cache".into(),
            "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Code Cache",
            SafetyLevel::Safe,
            Category::Cache,
            "Chrome JS Code Cache".into(),
        ),
        ScanTarget::new(
            "chrome_cache".into(),
            "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Cache",
            SafetyLevel::Safe,
            Category::Cache,
            "Chrome 磁盘缓存".into(),
        ),
        ScanTarget::new(
            "chrome_cache_storage".into(),
            "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\CacheStorage",
            SafetyLevel::Safe,
            Category::Cache,
            "Chrome CacheStorage".into(),
        ),
        ScanTarget::new(
            "edge_code_cache".into(),
            "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Code Cache",
            SafetyLevel::Safe,
            Category::Cache,
            "Edge JS Code Cache".into(),
        ),
        ScanTarget::new(
            "edge_cache".into(),
            "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Cache",
            SafetyLevel::Safe,
            Category::Cache,
            "Edge 磁盘缓存".into(),
        ),
        ScanTarget::new(
            "edge_cache_storage".into(),
            "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\CacheStorage",
            SafetyLevel::Safe,
            Category::Cache,
            "Edge CacheStorage".into(),
        ),
        ScanTarget::new(
            "firefox_cache".into(),
            "%APPDATA%\\Mozilla\\Firefox\\Profiles",
            SafetyLevel::Safe,
            Category::Cache,
            "Firefox 缓存".into(),
        )
        .with_browser(BrowserProfileConfig {
            profile_patterns: vec![
                "default".into(),
                ".default-release".into(),
                ".default-esr".into(),
                ".default-nightly".into(),
                ".dev-edition-default".into(),
            ],
            cache_subdirs: vec![
                "cache2/entries".into(),
                "startupCache".into(),
                "thumbnails".into(),
                "offlineCache".into(),
            ],
        }),
        ScanTarget::new(
            "wu_download".into(),
            "%WINDIR%\\SoftwareDistribution\\Download",
            SafetyLevel::Safe,
            Category::Cache,
            "Windows Update 下载缓存".into(),
        ),
        ScanTarget::new(
            "driver_store".into(),
            &format!("{d}\\Windows\\System32\\DriverStore\\FileRepository"),
            SafetyLevel::Confirm,
            Category::Cache,
            "旧驱动备份".into(),
        ),
        ScanTarget::new(
            "inet_cache".into(),
            "%LOCALAPPDATA%\\Microsoft\\Windows\\INetCache",
            SafetyLevel::Safe,
            Category::Cache,
            "Internet 临时文件".into(),
        ),
        // 注意（TASK-028）：回收站不再作为扫描目标（$Recycle.Bin 目录扫描基本权限失败），
        // 唯一入口为 empty_recycle_bin 命令 + 前端确认弹窗。
        // === 新增 28 目标 ===
        ScanTarget::new(
            "sys_logfiles".into(),
            "%WINDIR%\\System32\\LogFiles",
            SafetyLevel::Confirm,
            Category::Logs,
            "系统日志文件".into(),
        ),
        ScanTarget::new(
            "sys_logs".into(),
            "%WINDIR%\\Logs",
            SafetyLevel::Confirm,
            Category::Logs,
            "Windows 组件日志".into(),
        ),
        ScanTarget::new(
            "wer_user".into(),
            "%LOCALAPPDATA%\\Microsoft\\Windows\\WER",
            SafetyLevel::Safe,
            Category::Logs,
            "用户错误报告".into(),
        ),
        ScanTarget::new(
            "wer_system".into(),
            "%ALLUSERSPROFILE%\\Microsoft\\Windows\\WER",
            SafetyLevel::Safe,
            Category::Logs,
            "系统错误报告".into(),
        ),
        ScanTarget::new(
            "wer_temp_user".into(),
            "%LOCALAPPDATA%\\Temp",
            SafetyLevel::Safe,
            Category::Logs,
            "Temp 中 WER".into(),
        )
        .with_glob(vec!["*WER*".into()]),
        ScanTarget::new(
            "wer_temp_sys".into(),
            "%WINDIR%\\Temp",
            SafetyLevel::Safe,
            Category::Logs,
            "系统 Temp 中 WER".into(),
        )
        .with_glob(vec!["*WER*".into()]),
        ScanTarget::new(
            "sru".into(),
            "%WINDIR%\\System32\\sru",
            SafetyLevel::Confirm,
            Category::Logs,
            "系统资源使用统计（仅 SRUDB.dat）".into(),
        )
        .with_glob(vec!["SRUDB.dat".into()]),
        ScanTarget::new(
            "inet_cache_ie".into(),
            "%LOCALAPPDATA%\\Microsoft\\Windows\\INetCache\\IE",
            SafetyLevel::Safe,
            Category::Cache,
            "IE/Edge 传统 Internet 缓存".into(),
        ),
        ScanTarget::new(
            "oobe_info".into(),
            "%WINDIR%\\System32\\oobe\\info",
            SafetyLevel::Safe,
            Category::Temp,
            "OOBE 安装信息残留".into(),
        ),
        ScanTarget::new(
            "ntms_data".into(),
            "%WINDIR%\\System32\\NtmsData",
            SafetyLevel::Safe,
            Category::Temp,
            "可移动存储管理数据".into(),
        ),
        ScanTarget::new(
            "downloaded_progs".into(),
            "%WINDIR%\\Downloaded Program Files",
            SafetyLevel::Confirm,
            Category::Temp,
            "已下载程序文件".into(),
        ),
        ScanTarget::new(
            "flash_cache".into(),
            "%WINDIR%\\System32\\Macromed\\Flash",
            SafetyLevel::Safe,
            Category::Cache,
            "Flash 共享对象".into(),
        ),
        ScanTarget::new(
            "wu_datastore".into(),
            "%WINDIR%\\SoftwareDistribution\\DataStore",
            SafetyLevel::Confirm,
            Category::Cache,
            "Windows 更新数据库（停用更新服务后清理）".into(),
        )
        .with_service_stop("wuauserv".into())
        .with_glob(vec![
            "*.db".into(),
            "*.edb".into(),
            "*.jrs".into(),
            "*.blb".into(),
            "*.log".into(),
        ]),
        ScanTarget::new(
            "spool_servers".into(),
            "%WINDIR%\\System32\\spool\\SERVERS",
            SafetyLevel::Safe,
            Category::Temp,
            "打印服务器临时文件".into(),
        ),
        ScanTarget::new(
            "msdtc_trace".into(),
            "%WINDIR%\\System32\\MsDtc\\Trace",
            SafetyLevel::Safe,
            Category::Logs,
            "分布式事务协调器日志".into(),
        ),
        ScanTarget::new(
            "uwp_temp".into(),
            "%LOCALAPPDATA%\\Packages",
            SafetyLevel::Safe,
            Category::Temp,
            "UWP 临时文件".into(),
        ),
        ScanTarget::new(
            "uwp_inet_cache".into(),
            "%LOCALAPPDATA%\\Packages",
            SafetyLevel::Safe,
            Category::Cache,
            "UWP Internet 缓存".into(),
        ),
        ScanTarget::new(
            "uwp_local_cache".into(),
            "%LOCALAPPDATA%\\Packages",
            SafetyLevel::Safe,
            Category::Cache,
            "UWP 本地缓存".into(),
        ),
        ScanTarget::new(
            "windows_app_cache".into(),
            "%LOCALAPPDATA%\\Microsoft\\Windows\\AppCache",
            SafetyLevel::Safe,
            Category::Cache,
            "Windows App 缓存".into(),
        ),
        ScanTarget::new(
            "ts_client_cache".into(),
            "%LOCALAPPDATA%\\Microsoft\\TerminalServer Client\\Cache",
            SafetyLevel::Safe,
            Category::Cache,
            "远程桌面图标缓存".into(),
        ),
        ScanTarget::new(
            "downloads_old".into(),
            "%USERPROFILE%\\Downloads",
            SafetyLevel::Confirm,
            Category::Temp,
            "下载文件夹过时文件".into(),
        )
        .with_min_size(102_400),
        ScanTarget::new(
            "crashdumps".into(),
            "%USERPROFILE%\\AppData\\Local\\CrashDumps",
            SafetyLevel::Safe,
            Category::Logs,
            "应用崩溃转储".into(),
        ),
        ScanTarget::new(
            "etl_logs".into(),
            "%LOCALAPPDATA%\\Temp",
            SafetyLevel::Safe,
            Category::Logs,
            "事件跟踪日志".into(),
        )
        .with_glob(vec!["*.etl".into()]),
        ScanTarget::new(
            "app_logs".into(),
            "%LOCALAPPDATA%\\Temp",
            SafetyLevel::Safe,
            Category::Logs,
            "应用日志".into(),
        )
        .with_glob(vec!["*.log".into()]),
        ScanTarget::new(
            "wmp_cache".into(),
            "%LOCALAPPDATA%\\Microsoft\\Media Player",
            SafetyLevel::Safe,
            Category::Cache,
            "WMP 媒体库缓存".into(),
        ),
        ScanTarget::new(
            "explorer_cache".into(),
            "%LOCALAPPDATA%\\Microsoft\\Windows\\Caches",
            SafetyLevel::Safe,
            Category::Cache,
            "资源管理器缓存".into(),
        ),
        ScanTarget::new(
            "sys_reset".into(),
            &format!("{d}\\$SysReset"),
            SafetyLevel::Confirm,
            Category::Temp,
            "系统重置备份".into(),
        ),
        ScanTarget::new(
            "win_upgrade_tmp".into(),
            &format!("{d}\\$Windows.~BT"),
            SafetyLevel::Confirm,
            Category::Temp,
            "Windows 升级临时文件".into(),
        ),
        // === 应用缓存分类 ===
        ScanTarget::new(
            "discord_cache".into(),
            "%APPDATA%\\discord\\Cache",
            SafetyLevel::Safe,
            Category::AppCache,
            "Discord 缓存文件".into(),
        ),
        ScanTarget::new(
            "discord_code_cache".into(),
            "%APPDATA%\\discord\\Code Cache",
            SafetyLevel::Safe,
            Category::AppCache,
            "Discord JS Code Cache".into(),
        ),
        ScanTarget::new(
            "steam_cache".into(),
            "%LOCALAPPDATA%\\Steam\\htmlcache",
            SafetyLevel::Safe,
            Category::AppCache,
            "Steam 内置浏览器缓存".into(),
        ),
        ScanTarget::new(
            "wechat_cache".into(),
            "%LOCALAPPDATA%\\WeChat\\XPlugin\\Plugins",
            SafetyLevel::Confirm,
            Category::AppCache,
            "微信插件缓存（清理后自动重建）".into(),
        ),
        ScanTarget::new(
            "wechat_files".into(),
            "%LOCALAPPDATA%\\WeChat\\WeChatApp\\Cache",
            SafetyLevel::Safe,
            Category::AppCache,
            "微信应用缓存".into(),
        ),
        ScanTarget::new(
            "qq_cache".into(),
            "%LOCALAPPDATA%\\Tencent\\QQ\\Temp",
            SafetyLevel::Safe,
            Category::AppCache,
            "QQ 临时缓存".into(),
        ),
        ScanTarget::new(
            "electron_cache".into(),
            "%APPDATA%\\electron\\Cache",
            SafetyLevel::Safe,
            Category::AppCache,
            "Electron 框架缓存（Electron 基础框架缓存）".into(),
        ),
        // === 开发工具缓存分类 ===
        ScanTarget::new(
            "npm_cache".into(),
            "%APPDATA%\\npm-cache",
            SafetyLevel::Safe,
            Category::DevCache,
            "npm 包缓存".into(),
        ),
        ScanTarget::new(
            "pip_cache".into(),
            "%LOCALAPPDATA%\\pip\\cache",
            SafetyLevel::Safe,
            Category::DevCache,
            "pip 包缓存".into(),
        ),
        ScanTarget::new(
            "cargo_cache".into(),
            "%USERPROFILE%\\.cargo\\registry",
            SafetyLevel::Safe,
            Category::DevCache,
            "Cargo 注册表缓存（清理后需重新下载 crate）".into(),
        ),
        ScanTarget::new(
            "cargo_git".into(),
            "%USERPROFILE%\\.cargo\\git",
            SafetyLevel::Safe,
            Category::DevCache,
            "Cargo git 依赖缓存".into(),
        ),
        ScanTarget::new(
            "gradle_cache".into(),
            "%USERPROFILE%\\.gradle\\caches",
            SafetyLevel::Confirm,
            Category::DevCache,
            "Gradle 构建缓存（清理后构建速度下降）".into(),
        ),
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
    if cleaned.is_empty() {
        return true;
    }
    let on_c = cleaned.replacen(&d, "c:", 1);

    // PROTECTED_PREFIXES 匹配 + 分隔符边界
    let prog_data_lower = std::env::var("PROGRAMDATA")
        .unwrap_or_default()
        .to_lowercase();
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
        || cleaned == "c:\\windows\\system32\\sleepstudy"
        || cleaned.trim_end_matches('\\') == "c:\\windows\\system32\\sleepstudy"
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
    let matched: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let fname = e.file_name();
            let name = fname.to_string_lossy();
            cfg.profile_patterns
                .iter()
                .any(|p| name.contains(p.as_str()) || name.ends_with(p.as_str()))
                && !is_reparse_point(e)
        })
        .map(|e| e.path())
        .collect();
    matched
        .iter()
        .flat_map(|dir| cfg.cache_subdirs.iter().map(move |sub| dir.join(sub)))
        .filter(|p| p.exists())
        .collect()
}

/// 解析 UWP 包目录（限 MAX_UWP_PACKAGES，跳过 junction）
fn is_reparse_point(entry: &std::fs::DirEntry) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        entry
            .metadata()
            .map(|m| {
                // FILE_ATTRIBUTE_REPARSE_POINT = 0x400
                (m.file_attributes() & 0x400) != 0
            })
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn resolve_uwp_packages() -> Vec<PathBuf> {
    let packages_dir = expand_env("%LOCALAPPDATA%\\Packages");
    let dir = match std::fs::read_dir(&packages_dir) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    dir.filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false) && !is_reparse_point(e))
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
        if t.level == SafetyLevel::Forbidden {
            continue;
        }
        let expanded = expand_env(&t.path);
        let p = PathBuf::from(&expanded);

        let safe_path = match std::fs::canonicalize(&p) {
            Ok(p) => p,
            Err(_) => p,
        };
        if is_path_protected(&safe_path) {
            continue;
        }
        // 验证展开路径在预期前缀内
        let path_ok = match std::fs::canonicalize(&safe_path) {
            Ok(canon) => verify_env_path_inner(&canon, &t.path),
            Err(_) => verify_env_path_inner(&safe_path, &t.path), // fallback: 用非 canonical 路径
        };
        if !path_ok {
            continue;
        }

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
                let sub = match t.id.as_str() {
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
        (
            "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Code Cache",
            "chrome_code_cache",
        ),
        (
            "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Cache",
            "chrome_cache",
        ),
        ("%APPDATA%\\Mozilla\\Firefox\\Profiles", "firefox_cache"),
        ("%WINDIR%\\SoftwareDistribution\\Download", "wu_download"),
        (
            "%LOCALAPPDATA%\\Microsoft\\Windows\\INetCache",
            "inet_cache",
        ),
    ]
    .into();
    for old_path in &config.disabled_targets {
        if let Some(id) = path_to_id.get(old_path.as_str())
            && !config.disabled_target_ids.contains(&id.to_string())
        {
            config.disabled_target_ids.push(id.to_string());
        }
    }
    config.disabled_targets.clear();
    config.version = Some(2);
    config
}

/// 加载用户配置（自动迁移 v1→v2→v3）
pub fn load_config() -> PonyConfig {
    let path = config_path();
    let mut config: PonyConfig = fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let ver = config.version.unwrap_or(1);
    if ver < 2 {
        config = migrate_v1_to_v2(config);
    }
    if ver < 3 {
        // v3: rename `app_cache` target id → `windows_app_cache`
        if let Some(pos) = config
            .disabled_target_ids
            .iter()
            .position(|id| id == "app_cache")
        {
            config.disabled_target_ids[pos] = "windows_app_cache".to_string();
        }
        config.version = Some(3);
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
            fn drop(&mut self) {
                SCAN_IN_PROGRESS.store(false, Ordering::SeqCst);
            }
        }
        let _guard = ScanGuard;

        let _ = tx.send(ScanEvent::Progress {
            scanned: 0,
            current: "Starting scan...".into(),
        });
        // ─── TASK-027: target 级并行扫描 ───
        // 按 resolved 顺序轮询分为 SCAN_PARALLELISM 组，每组独立线程；汇总线程转发事件并统一计数
        let mut groups: Vec<Vec<(PathBuf, usize)>> = vec![Vec::new(); SCAN_PARALLELISM];
        for (i, item) in resolved.iter().enumerate() {
            groups[i % SCAN_PARALLELISM].push(item.clone());
        }
        let groups: Vec<Vec<(PathBuf, usize)>> =
            groups.into_iter().filter(|g| !g.is_empty()).collect();

        let (agg_tx, agg_rx) = mpsc::channel::<ScanEvent>();
        let global_count = Arc::new(AtomicU64::new(0));
        let global_hit = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();

        for group in groups {
            let agg_tx = agg_tx.clone();
            let cancel = cancel_token_clone.clone();
            let targets = targets.clone();
            let global_count = global_count.clone();
            let global_hit = global_hit.clone();
            handles.push(std::thread::spawn(move || {
                let mut batch: Vec<CleanItem> = Vec::with_capacity(BATCH_SIZE);
                let mut total_bytes = 0u64;
                let mut skipped = 0u64;
                let mut cancelled = false;
                for (target_path, target_idx) in group {
                    if cancel.is_cancelled() || global_hit.load(Ordering::Relaxed) {
                        cancelled = cancel.is_cancelled();
                        break;
                    }
                    let (b, s) = scan_target_block(
                        &targets[target_idx],
                        &target_path,
                        &cancel,
                        &global_count,
                        &global_hit,
                        &mut batch,
                        &agg_tx,
                    );
                    total_bytes += b;
                    skipped += s;
                }
                if !batch.is_empty() {
                    let _ = agg_tx.send(ScanEvent::ItemsFound {
                        items: batch,
                        batch_complete: true,
                    });
                }
                (total_bytes, skipped, cancelled)
            }));
        }
        drop(agg_tx);

        // 汇总：转发事件，join 全部工作线程后统一收尾
        let mut total_bytes = 0u64;
        let mut skipped_small = 0u64;
        let mut cancelled_any = false;
        let mut panicked = false;
        for ev in agg_rx {
            match ev {
                ScanEvent::ItemsFound {
                    items,
                    batch_complete,
                } => {
                    let _ = tx.send(ScanEvent::ItemsFound {
                        items,
                        batch_complete,
                    });
                }
                ScanEvent::Progress { scanned, current } => {
                    let _ = tx.send(ScanEvent::Progress { scanned, current });
                }
                ScanEvent::Warning(w) => {
                    let _ = tx.send(ScanEvent::Warning(w));
                }
                _ => {}
            }
        }
        for h in handles {
            match h.join() {
                Ok((b, s, cancelled)) => {
                    total_bytes += b;
                    skipped_small += s;
                    cancelled_any |= cancelled;
                }
                Err(_) => panicked = true,
            }
        }
        if panicked {
            let _ = tx.send(ScanEvent::Warning(ScanWarning::PermissionDenied {
                target_id: "scan".into(),
                path: "worker thread panicked".into(),
            }));
        }
        let total_items = global_count.load(Ordering::SeqCst);
        if cancelled_any {
            let _ = tx.send(ScanEvent::Cancelled);
        } else {
            if global_hit.load(Ordering::SeqCst) {
                let _ = tx.send(ScanEvent::Warning(ScanWarning::MaxItemsReached {
                    target_id: "global".into(),
                    items: MAX_SCAN_ITEMS,
                }));
            }
            let _ = tx.send(ScanEvent::Done {
                total_items,
                total_bytes,
                skipped_small,
            });
        }
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

/// 扫描单个 target（供并行扫描 worker 线程调用，TASK-027）
///
/// 保持原串行语义：glob/mtime/单 target 上限过滤、批次推送、进度推送。
/// 全局计数经 `global_count`（AtomicU64）原子累加，超 `MAX_SCAN_ITEMS` 时置 `global_hit`。
/// 返回 (累计字节, 跳过的微效文件数)。
#[allow(clippy::too_many_arguments)]
fn scan_target_block(
    target_def: &ScanTarget,
    target_path: &Path,
    cancel_token: &CancellationToken,
    global_count: &AtomicU64,
    global_hit: &AtomicBool,
    batch: &mut Vec<CleanItem>,
    tx: &mpsc::Sender<ScanEvent>,
) -> (u64, u64) {
    let mut total_bytes = 0u64;
    let mut skipped_small = 0u64;
    let cat_min_size = target_def.min_size;
    let glob_inc = target_def.glob_include.as_deref();
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
                        !(name == "node_modules"
                            || name == ".git"
                            || name == "__pycache__"
                            || name == ".svn")
                    })
                });
            }
            children.retain(|e| e.is_ok());
        });

    for entry in walk_dir.into_iter().filter_map(|e| e.ok()) {
        if cancel_token.is_cancelled() || global_hit.load(Ordering::Relaxed) {
            return (total_bytes, skipped_small);
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let size = meta.len();
        if size < cat_min_size {
            skipped_small += 1;
            continue;
        }

        // mtime 过滤（logs 类别）
        if let Some(cutoff) = mtime_cutoff
            && let Ok(mtime) = meta.modified()
            && let Ok(secs) = mtime.duration_since(std::time::UNIX_EPOCH)
            && secs.as_secs() as i64 > cutoff
        {
            continue;
        }

        // glob_include 过滤：支持 `*.ext`（后缀）, `*WER*`（包含）, `prefix*`（前缀）
        if let Some(inc) = glob_inc {
            let fname = entry.file_name().to_string_lossy();
            if !inc.iter().any(|p| {
                let p = p.as_str();
                let has_prefix_wild = p.starts_with('*');
                let has_suffix_wild = p.ends_with('*');
                let inner = p.trim_start_matches('*').trim_end_matches('*');
                match (has_prefix_wild, has_suffix_wild) {
                    (true, true) => fname.contains(inner),
                    (true, false) => fname.ends_with(inner),
                    (false, true) => fname.starts_with(inner),
                    (false, false) => fname == *p,
                }
            }) {
                continue;
            }
        }

        if target_count >= target_def.max_items_per_target {
            let _ = tx.send(ScanEvent::Warning(ScanWarning::MaxItemsReached {
                target_id: target_def.id.clone(),
                items: target_count,
            }));
            break;
        }

        let n = global_count.fetch_add(1, Ordering::Relaxed) + 1;
        if n > MAX_SCAN_ITEMS {
            global_hit.store(true, Ordering::Relaxed);
            return (total_bytes, skipped_small);
        }

        total_bytes += size;
        target_count += 1;
        batch.push(CleanItem {
            path: entry.path(),
            size_bytes: size,
            level: target_def.level.clone(),
            category: target_def.category.to_string(),
        });

        if batch.len() >= BATCH_SIZE {
            let _ = tx.send(ScanEvent::ItemsFound {
                items: std::mem::take(batch),
                batch_complete: false,
            });
        }
        if n.is_multiple_of(100) {
            let _ = tx.send(ScanEvent::Progress {
                scanned: n,
                current: target_path.to_string_lossy().to_string(),
            });
        }
    }
    (total_bytes, skipped_small)
}

fn chrono_placeholder_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 检测文件是否被进程占用（请求 DELETE 访问权限探测，TASK-023）
///
/// 被其他进程以不允许删除共享的方式打开 → true。
/// 仅在 Windows 生效；非 Windows 恒 false。
#[cfg(windows)]
pub fn is_file_busy(path: &Path) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION, GetLastError,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, DELETE, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_MODE, OPEN_EXISTING,
    };

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // 请求 DELETE 访问：任何已打开句柄未共享删除 → 打开失败（共享/锁冲突 = 占用）
    let handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            DELETE.0,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };
    match handle {
        Ok(h) => {
            unsafe {
                let _ = CloseHandle(h);
            }
            false
        }
        Err(_) => {
            let err = unsafe { GetLastError() };
            err == ERROR_SHARING_VIOLATION || err == ERROR_LOCK_VIOLATION
        }
    }
}

#[cfg(not(windows))]
pub fn is_file_busy(_path: &Path) -> bool {
    false
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
    // TASK-028：校验目标集使用与扫描一致的 filtered targets（内置 + 用户自定义，
    // 受 disabled_target_ids / per_target_config 过滤），否则自定义 target 扫出的
    // 文件会在删除时因不在内置目标集而被拒绝。
    let targets = get_filtered_targets(&load_config());
    delete_files_with_targets(paths, &targets, progress_tx)
}

/// 按给定目标集删除文件（内部实现：公开入口传入 filtered targets，测试传入 fixture）
fn delete_files_with_targets(
    paths: &[PathBuf],
    targets: &[ScanTarget],
    progress_tx: Option<mpsc::Sender<DeleteProgress>>,
) -> DeleteResult {
    let mut result = DeleteResult::default();
    let total = paths.len() as u64;
    let mut done = 0u64;

    // TASK-025: 目标需停服务的（如 wu_datastore → wuauserv），先停止，删除后恢复
    let service_targets: Vec<(String, String)> = targets
        .iter()
        .filter_map(|t| {
            t.requires_service_stop
                .as_ref()
                .map(|s| (s.clone(), t.path.clone()))
        })
        .collect();
    let mut stopped: Vec<String> = Vec::new();
    let mut stop_failed: Vec<String> = Vec::new();
    for (service, _) in &service_targets {
        if stopped.contains(service) || stop_failed.contains(service) {
            continue;
        }
        match stop_service(service) {
            Ok(_) => stopped.push(service.clone()),
            Err(e) => {
                stop_failed.push(service.clone());
                result.errors.push(format!("无法停止服务 {service}: {e}"));
            }
        }
    }

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
        if !is_path_allowed(&safe_path, targets) {
            result.failed += 1;
            done += 1;
            result
                .errors
                .push(format!("Path not in scan scope: {}", safe_path.display()));
            send_progress(&progress_tx, done, total, path);
            continue;
        }

        // 服务停止失败时，属于该服务的 target 路径跳过删除（避免损坏数据）
        let safe_str = safe_path.to_string_lossy().to_lowercase();
        let blocked = service_targets.iter().any(|(s, p)| {
            stop_failed.contains(s) && safe_str.starts_with(&expand_env(p).to_lowercase())
        });
        if blocked {
            result.failed += 1;
            done += 1;
            result
                .errors
                .push(format!("服务无法停止，跳过删除: {}", safe_path.display()));
            send_progress(&progress_tx, done, total, path);
            continue;
        }

        match std::fs::remove_file(&safe_path) {
            Ok(()) => result.success += 1,
            Err(e) => {
                // 占用检测：被占用则注明原因（仍走延迟删除通道）
                let busy = cfg!(windows) && is_file_busy(&safe_path);
                let delayed = if cfg!(windows) {
                    delete_file_delayed_windows(&safe_path)
                } else {
                    Err(format!("{e}"))
                };
                match delayed {
                    Ok(()) => result.success += 1,
                    Err(msg) => {
                        result.failed += 1;
                        if busy {
                            result
                                .errors
                                .push(format!("文件被进程占用，延迟删除失败: {msg}"));
                        } else {
                            result.errors.push(msg);
                        }
                    }
                }
            }
        }
        done += 1;
        if done.is_multiple_of(10) || done == total {
            send_progress(&progress_tx, done, total, path);
        }
    }

    // 恢复服务（后进先出）
    for service in stopped.iter().rev() {
        if let Err(e) = start_service(service) {
            result
                .errors
                .push(format!("删除后无法恢复服务 {service}: {e}"));
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

/// 停止 Windows 服务，返回服务原本是否在运行（TASK-025）
#[cfg(windows)]
fn stop_service(name: &str) -> Result<bool, String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::time::Duration;
    use windows::Win32::Foundation::{ERROR_SERVICE_NOT_ACTIVE, GetLastError};
    use windows::Win32::Security::SC_HANDLE;
    use windows::Win32::System::Services::*;
    use windows::core::PCWSTR;

    let wide: Vec<u16> = OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // 句柄守卫：作用域结束时关闭
    struct Handle(SC_HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseServiceHandle(self.0);
            }
        }
    }

    let scm = unsafe { OpenSCManagerW(None, None, SC_MANAGER_CONNECT) }
        .map_err(|e| format!("OpenSCManagerW failed: {e}"))?;
    let _scm = Handle(scm);

    let svc = unsafe {
        OpenServiceW(
            scm,
            PCWSTR(wide.as_ptr()),
            SERVICE_STOP | SERVICE_QUERY_STATUS | SERVICE_START,
        )
    }
    .map_err(|e| format!("OpenServiceW({name}) failed（可能需要管理员权限）: {e}"))?;
    let _svc = Handle(svc);

    let mut status = SERVICE_STATUS::default();
    unsafe { QueryServiceStatus(svc, &mut status) }
        .map_err(|e| format!("QueryServiceStatus failed: {e}"))?;
    if status.dwCurrentState == SERVICE_STOPPED {
        return Ok(false);
    }

    if let Err(e) = unsafe { ControlService(svc, SERVICE_CONTROL_STOP, &mut status) } {
        let err = unsafe { GetLastError() };
        if err == ERROR_SERVICE_NOT_ACTIVE {
            return Ok(false);
        }
        return Err(format!("ControlService({name}) 停止失败: {e}"));
    }

    // 等待服务停止（最多 30s）
    for _ in 0..300 {
        let mut st = SERVICE_STATUS::default();
        if unsafe { QueryServiceStatus(svc, &mut st) }.is_ok()
            && st.dwCurrentState == SERVICE_STOPPED
        {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!("等待服务 {name} 停止超时"))
}

/// 启动 Windows 服务（已运行视为成功）
#[cfg(windows)]
fn start_service(name: &str) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{ERROR_SERVICE_ALREADY_RUNNING, GetLastError};
    use windows::Win32::Security::SC_HANDLE;
    use windows::Win32::System::Services::*;
    use windows::core::PCWSTR;

    let wide: Vec<u16> = OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    struct Handle(SC_HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseServiceHandle(self.0);
            }
        }
    }

    let scm = unsafe { OpenSCManagerW(None, None, SC_MANAGER_CONNECT) }
        .map_err(|e| format!("OpenSCManagerW failed: {e}"))?;
    let _scm = Handle(scm);

    let svc = unsafe { OpenServiceW(scm, PCWSTR(wide.as_ptr()), SERVICE_START) }
        .map_err(|e| format!("OpenServiceW({name}) 失败: {e}"))?;
    let _svc = Handle(svc);

    if let Err(e) = unsafe { StartServiceW(svc, None) } {
        let err = unsafe { GetLastError() };
        if err == ERROR_SERVICE_ALREADY_RUNNING {
            return Ok(());
        }
        return Err(format!("StartServiceW({name}) 失败: {e}"));
    }
    Ok(())
}

#[cfg(not(windows))]
fn stop_service(_name: &str) -> Result<bool, String> {
    Ok(false)
}

#[cfg(not(windows))]
fn start_service(_name: &str) -> Result<(), String> {
    Ok(())
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
                // 解码 HRESULT 为可读错误，便于定位（E_ACCESSDENIED=文件被占用/权限不足等）
                use windows::Win32::Foundation::E_ACCESSDENIED;
                use windows::Win32::Foundation::E_FAIL;
                let err = result.expect_err("result.is_ok() checked above");
                let hr = err.code(); // HRESULT
                let hr_hex = format!("0x{:08X}", hr.0);
                let message = err.message();
                let hint = if hr == E_ACCESSDENIED {
                    "回收站中有文件正被其他程序占用（如云盘、杀毒软件或正在运行的应用），或当前权限不足，请关闭占用程序后重试"
                } else if hr == E_FAIL {
                    "回收站中部分文件无法删除，可能是系统文件被保护或目录结构异常"
                } else {
                    "清空回收站失败，请稍后重试或检查回收站目录"
                };
                Err(format!("清空回收站失败 ({hr_hex}): {message}。{hint}"))
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

#[derive(Clone, Debug, Default, Serialize)]
pub struct CleanLogSummary {
    pub entries: Vec<CleanLogEntry>,
}

const CLEAN_LOG_FILE: &str = "clean_log.jsonl";
const MAX_LOG_BYTES: u64 = 1_048_576;
const MAX_LOG_BACKUPS: u32 = 5;
static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn timestamp_now() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
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
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) {
        &LEAP_MONTH_DAYS[..]
    } else {
        &NORM_MONTH_DAYS[..]
    };
    let mut mo = 1u32;
    for md in month_days {
        if remaining < *md as i64 {
            break;
        }
        remaining -= *md as i64;
        mo += 1;
    }
    format!(
        "{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z",
        d = remaining as u32 + 1
    )
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
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open log: {e}"))?;
    use std::io::Write;
    writeln!(file, "{json}").map_err(|e| format!("write log: {e}"))?;
    Ok(())
}

fn rotate_if_needed(path: &Path) {
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size <= MAX_LOG_BYTES {
        return;
    }
    for i in (1..MAX_LOG_BACKUPS).rev() {
        let old = path.with_extension(format!("{i}.jsonl"));
        let new = path.with_extension(format!("{}.jsonl", i + 1));
        if old.exists() {
            let _ = fs::rename(&old, &new);
        }
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
        content
            .lines()
            .rev()
            .take(limit)
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    } else {
        vec![]
    };
    Ok(CleanLogSummary { entries })
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
        assert!(
            !is_path_protected(Path::new(r"C:\Windows\System32\LogFiles\some.log")),
            "LogFiles 应允许扫描"
        );
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
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\Tasks\SomeTask"
        )));
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

    #[serial_test::serial]
    #[test]
    fn test_is_path_not_protected() {
        let temp = std::env::var("TEMP").unwrap_or_else(|_| r"C:\Temp".to_string());
        assert!(!is_path_protected(Path::new(&temp)));
    }

    #[serial_test::serial]
    #[test]
    fn test_is_path_allowed_valid() {
        let targets = vec![ScanTarget::new(
            "t".into(),
            "%TEMP%",
            SafetyLevel::Safe,
            Category::Temp,
            "".into(),
        )];
        let temp = std::env::var("TEMP").unwrap();
        let test_path = PathBuf::from(&temp).join("test.txt");
        assert!(is_path_allowed(&test_path, &targets));
    }

    #[test]
    fn test_is_path_allowed_rejected() {
        let targets = vec![ScanTarget::new(
            "t".into(),
            "%TEMP%",
            SafetyLevel::Safe,
            Category::Temp,
            "".into(),
        )];
        assert!(!is_path_allowed(
            Path::new(r"C:\Windows\System32\test.dll".into()),
            &targets,
        ));
    }

    #[test]
    fn test_is_path_allowed_cache_category() {
        let targets = vec![ScanTarget::new(
            "t".into(),
            "%LOCALAPPDATA%\\Google\\Chrome",
            SafetyLevel::Safe,
            Category::Cache,
            "".into(),
        )];
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let test_path = PathBuf::from(&local).join("Google\\Chrome\\Cache\\f_000001");
        assert!(is_path_allowed(&test_path, &targets));
    }

    #[serial_test::serial]
    #[test]
    fn test_resolve_targets_excludes_forbidden() {
        // 固定 TEMP 为完整路径：GitHub runner 的 TEMP 是 8.3 短名（RUNNER~1），
        // canonicalize 展开后与原始值不匹配会导致 verify_env_path_inner 失败
        temp_env::with_var("TEMP", Some("C:\\TestTemp"), || {
            let targets = vec![
                ScanTarget::new(
                    "temp".into(),
                    "%TEMP%",
                    SafetyLevel::Safe,
                    Category::Temp,
                    "".into(),
                ),
                ScanTarget::new(
                    "sys32".into(),
                    "C:\\Windows\\System32",
                    SafetyLevel::Forbidden,
                    Category::Temp,
                    "".into(),
                ),
            ];
            let resolved = resolve_targets(&targets);
            // 应包含指向 TEMP 的路径（Safe）
            assert!(!resolved.is_empty(), "should include TEMP");
            // 应排除 System32（Forbidden）
            assert!(
                !resolved
                    .iter()
                    .any(|(p, _)| p.to_string_lossy().to_lowercase().contains("system32")),
                "should NOT include System32"
            );
        });
    }

    #[test]
    fn test_resolve_targets_skips_protected() {
        let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        // 使用 config（在 PROTECTED_PREFIXES 中）作为受保护路径
        let protected = format!("{}\\System32\\config", sys_root);
        let targets = vec![ScanTarget::new(
            "p".into(),
            &protected,
            SafetyLevel::Safe,
            Category::Temp,
            "".into(),
        )];
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
    #[cfg(windows)]
    fn test_is_file_busy() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("busy.bin");
        std::fs::write(&f, vec![0u8; 16]).unwrap();
        // 未占用
        assert!(!is_file_busy(&f), "unlocked file should not be busy");
        // Rust File::open 不共享删除 → 占用
        let handle = std::fs::File::open(&f).unwrap();
        assert!(is_file_busy(&f), "opened file should be busy");
        drop(handle);
        assert!(!is_file_busy(&f), "after close should be free");
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
            ScanEvent::Done {
                total_items,
                skipped_small,
                ..
            } => {
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
        assert_eq!(
            targets.len(),
            54,
            "should have 54 targets (55 minus recycle_bin, TASK-028 回收站改走 empty_recycle_bin)"
        );
        // 检查一些特定目标存在
        assert!(targets.iter().any(|t| t.id == "sys_logfiles"));
        assert!(targets.iter().any(|t| t.id == "uwp_temp"));
        assert!(targets.iter().any(|t| t.id == "downloads_old"));
        // 回收站不再作为扫描目标（TASK-028）
        assert!(!targets.iter().any(|t| t.id == "recycle_bin"));
    }

    #[test]
    fn test_target_ids_unique() {
        let ids: Vec<String> = get_clean_targets().iter().map(|t| t.id.clone()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len(), "target ids must be unique");
    }

    #[test]
    fn test_category_serde_roundtrip() {
        assert_eq!(
            serde_json::to_value(&Category::Temp).unwrap(),
            serde_json::json!("temp")
        );
        assert_eq!(
            serde_json::to_value(&Category::Cache).unwrap(),
            serde_json::json!("cache")
        );
        assert_eq!(
            serde_json::to_value(&Category::Logs).unwrap(),
            serde_json::json!("logs")
        );
        assert_eq!(
            serde_json::to_value(&Category::Prefetch).unwrap(),
            serde_json::json!("prefetch")
        );
        assert_eq!(
            serde_json::to_value(&Category::RecycleBin).unwrap(),
            serde_json::json!("recycle_bin")
        );
        assert_eq!(
            serde_json::to_value(&Category::OldInstall).unwrap(),
            serde_json::json!("old_install")
        );
        assert_eq!(
            serde_json::to_value(&Category::AppCache).unwrap(),
            serde_json::json!("app_cache")
        );
        assert_eq!(
            serde_json::to_value(&Category::DevCache).unwrap(),
            serde_json::json!("dev_cache")
        );
    }

    #[test]
    fn test_category_display() {
        assert_eq!(Category::Temp.to_string(), "temp");
        assert_eq!(Category::OldInstall.to_string(), "old_install");
        assert_eq!(Category::AppCache.to_string(), "app_cache");
        assert_eq!(Category::DevCache.to_string(), "dev_cache");
    }

    #[test]
    fn test_min_size_per_category() {
        assert_eq!(Category::Cache.default_min_size(), 512);
        assert_eq!(Category::Logs.default_min_size(), 4096);
        assert_eq!(Category::Temp.default_min_size(), 1024);
        assert_eq!(Category::AppCache.default_min_size(), 1024);
        assert_eq!(Category::DevCache.default_min_size(), 1024);
    }

    #[test]
    fn test_scan_warning_enum_variants() {
        let w1 = ScanWarning::MaxItemsReached {
            target_id: "t".into(),
            items: 100,
        };
        let w2 = ScanWarning::PermissionDenied {
            target_id: "t".into(),
            path: "p".into(),
        };
        match w1 {
            ScanWarning::MaxItemsReached { .. } => {}
            _ => panic!(),
        }
        match w2 {
            ScanWarning::PermissionDenied { .. } => {}
            _ => panic!(),
        }
    }

    #[test]
    fn test_is_path_protected_separator() {
        // 关键子路径受保护
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\config\SAM"
        )));
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\Tasks\test"
        )));
    }

    #[test]
    fn test_protected_winevt_logs() {
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\winevt\Logs\Security.evtx"
        )));
    }

    #[test]
    fn test_protected_sleepstudy_dir() {
        // SleepStudy 目录本身由 is_path_protected 中的特殊代码保护
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\sleepstudy"
        )));
    }

    #[test]
    fn test_sleepstudy_subfile_not_protected() {
        // 子文件不再受 System32 总前缀保护
        assert!(!is_path_protected(Path::new(
            r"C:\Windows\System32\sleepstudy\sub.etl"
        )));
    }

    #[serial_test::serial]
    #[test]
    fn test_is_path_allowed_trailing_slash() {
        let targets = vec![ScanTarget::new(
            "t".into(),
            "%TEMP%\\",
            SafetyLevel::Safe,
            Category::Temp,
            "".into(),
        )];
        let temp = std::env::var("TEMP").unwrap_or_default();
        if !temp.is_empty() {
            let sub = PathBuf::from(temp.trim_end_matches('\\')).join("test.tmp");
            assert!(
                is_path_allowed(&sub, &targets),
                "trailing \\ in expanded should work"
            );
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
    fn test_custom_targets_merged() {
        let mut config = PonyConfig::default();
        config.custom_targets.push(CustomTarget {
            id: "my_app_cache".into(),
            path: "%LOCALAPPDATA%\\MyApp\\Cache".into(),
            level: SafetyLevel::Safe,
            category: Category::AppCache,
            description: "我的应用缓存".into(),
            enabled: true,
        });
        let filtered = get_filtered_targets(&config);
        let custom = filtered.iter().find(|t| t.id == "my_app_cache");
        assert!(custom.is_some(), "custom target should be merged");
        assert_eq!(custom.unwrap().category, Category::AppCache);
        assert_eq!(custom.unwrap().level, SafetyLevel::Safe);
    }

    #[test]
    fn test_custom_targets_disabled_or_forbidden_skipped() {
        let mut config = PonyConfig::default();
        config.custom_targets.push(CustomTarget {
            id: "disabled_one".into(),
            path: "%TEMP%\\X".into(),
            level: SafetyLevel::Safe,
            category: Category::Temp,
            description: "".into(),
            enabled: false,
        });
        config.custom_targets.push(CustomTarget {
            id: "forbidden_one".into(),
            path: "%WINDIR%\\System32".into(),
            level: SafetyLevel::Forbidden,
            category: Category::Temp,
            description: "".into(),
            enabled: true,
        });
        let filtered = get_filtered_targets(&config);
        assert!(!filtered.iter().any(|t| t.id == "disabled_one"));
        assert!(!filtered.iter().any(|t| t.id == "forbidden_one"));
    }

    #[test]
    fn test_custom_targets_id_conflict_skipped() {
        let mut config = PonyConfig::default();
        config.custom_targets.push(CustomTarget {
            id: "user_temp".into(), // 与内置目标冲突
            path: "%TEMP%\\Custom".into(),
            level: SafetyLevel::Safe,
            category: Category::Temp,
            description: "".into(),
            enabled: true,
        });
        let filtered = get_filtered_targets(&config);
        let user_temp = filtered.iter().find(|t| t.id == "user_temp").unwrap();
        assert_eq!(
            user_temp.path, "%TEMP%",
            "builtin target wins on id conflict"
        );
    }

    #[test]
    fn test_custom_target_serde_roundtrip() {
        let ct = CustomTarget {
            id: "roundtrip".into(),
            path: "%TEMP%\\R".into(),
            level: SafetyLevel::Confirm,
            category: Category::Logs,
            description: "往返测试".into(),
            enabled: true,
        };
        let json = serde_json::to_string(&ct).unwrap();
        let back: CustomTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "roundtrip");
        assert_eq!(back.level, SafetyLevel::Confirm);
        assert_eq!(back.category, Category::Logs);
        assert!(back.enabled);
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

    // ===== TASK-028: DiskScanConfig 兼容性 + clamp =====
    #[test]
    fn test_disk_scan_config_missing_field_defaults() {
        // 旧 config.json 无 disk_scan 字段 → 默认 (100, 3)
        let cfg: PonyConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.disk_scan_params(), (100, 3));
        // disk_scan 空对象 → 默认 (100, 3)
        let cfg: PonyConfig = serde_json::from_str(r#"{"disk_scan": {}}"#).unwrap();
        assert_eq!(cfg.disk_scan_params(), (100, 3));
    }

    #[test]
    fn test_disk_scan_config_partial_field() {
        // 只设 min_bytes_mb，dir_depth 缺失 → 深度回退默认
        let cfg: PonyConfig =
            serde_json::from_str(r#"{"disk_scan": {"min_bytes_mb": 500}}"#).unwrap();
        assert_eq!(cfg.disk_scan_params(), (500, 3));
        // 只设 dir_depth
        let cfg: PonyConfig = serde_json::from_str(r#"{"disk_scan": {"dir_depth": 5}}"#).unwrap();
        assert_eq!(cfg.disk_scan_params(), (100, 5));
    }

    #[test]
    fn test_disk_scan_config_clamp_out_of_range() {
        // 手改配置文件恶意/越界值 → clamp 到合法范围
        let cfg: PonyConfig =
            serde_json::from_str(r#"{"disk_scan": {"min_bytes_mb": 0, "dir_depth": 0}}"#).unwrap();
        assert_eq!(cfg.disk_scan_params(), (50, 1));
        let cfg: PonyConfig =
            serde_json::from_str(r#"{"disk_scan": {"min_bytes_mb": 99999999, "dir_depth": 999}}"#)
                .unwrap();
        assert_eq!(cfg.disk_scan_params(), (10_000, 5));
        // 边界值：合法下限/上限两侧
        let cfg: PonyConfig =
            serde_json::from_str(r#"{"disk_scan": {"min_bytes_mb": 49, "dir_depth": 0}}"#).unwrap();
        assert_eq!(cfg.disk_scan_params(), (50, 1));
        let cfg: PonyConfig =
            serde_json::from_str(r#"{"disk_scan": {"min_bytes_mb": 50, "dir_depth": 1}}"#).unwrap();
        assert_eq!(cfg.disk_scan_params(), (50, 1));
        let cfg: PonyConfig =
            serde_json::from_str(r#"{"disk_scan": {"min_bytes_mb": 10000, "dir_depth": 5}}"#)
                .unwrap();
        assert_eq!(cfg.disk_scan_params(), (10_000, 5));
        let cfg: PonyConfig =
            serde_json::from_str(r#"{"disk_scan": {"min_bytes_mb": 10001, "dir_depth": 6}}"#)
                .unwrap();
        assert_eq!(cfg.disk_scan_params(), (10_000, 5));
    }

    #[test]
    fn test_disk_scan_config_roundtrip() {
        let cfg = PonyConfig {
            disk_scan: Some(DiskScanConfig {
                min_bytes_mb: Some(500),
                dir_depth: Some(4),
            }),
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: PonyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.disk_scan_params(), (500, 4));
    }

    // ===== TASK-028: 自定义 target 文件可删除 =====
    #[test]
    fn test_delete_custom_target_file_succeeds() {
        // 临时目录作为自定义 target，其下文件应能被删除
        // （修复：删除校验目标集此前只用内置 get_clean_targets，自定义路径被拒）
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("custom_file.bin");
        std::fs::write(&f, vec![0u8; 2048]).unwrap();

        let mut config = PonyConfig::default();
        config.custom_targets.push(CustomTarget {
            id: "my_custom".into(),
            path: dir.path().to_string_lossy().to_string(),
            level: SafetyLevel::Safe,
            category: Category::Temp,
            description: "测试自定义目标".into(),
            enabled: true,
        });
        let filtered = get_filtered_targets(&config);
        assert!(
            filtered.iter().any(|t| t.id == "my_custom"),
            "custom target should be in filtered targets"
        );
        assert!(
            is_path_allowed(&f, &filtered),
            "custom target file should pass scope check"
        );

        let result = delete_files_with_targets(&[f.clone()], &filtered, None);
        assert_eq!(
            result.success, 1,
            "custom target file should be deleted: {:?}",
            result.errors
        );
        assert!(!f.exists());
    }

    #[test]
    fn test_delete_custom_target_file_rejects_outside_path() {
        // 自定义 target 之外的文件仍被拒绝（安全边界不回退）。
        // 注意：tempfile 目录位于 %LOCALAPPDATA%\Temp，属于内置 local_temp 目标，
        // 因此用"仅含自定义目标"的目标集验证自定义目标的边界拒绝逻辑。
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let f = outside.path().join("outside.bin");
        std::fs::write(&f, vec![0u8; 2048]).unwrap();

        let custom_only = vec![ScanTarget::new(
            "my_custom".into(),
            &dir.path().to_string_lossy(),
            SafetyLevel::Safe,
            Category::Temp,
            "".into(),
        )];
        assert!(!is_path_allowed(&f, &custom_only));
        let result = delete_files_with_targets(&[f.clone()], &custom_only, None);
        assert_eq!(result.failed, 1);
        assert!(f.exists(), "file outside custom target must not be deleted");
    }

    #[test]
    fn test_resolve_targets_dedup_same_path() {
        let targets = vec![
            ScanTarget::new(
                "a".into(),
                "%TEMP%",
                SafetyLevel::Safe,
                Category::Temp,
                "".into(),
            ),
            ScanTarget::new(
                "b".into(),
                "%TEMP%",
                SafetyLevel::Safe,
                Category::Temp,
                "".into(),
            ), // 同路径
        ];
        let resolved = resolve_targets(&targets);
        let paths: std::collections::HashSet<_> = resolved.iter().map(|(p, _)| p).collect();
        assert_eq!(paths.len(), resolved.len(), "no duplicate paths");
    }

    #[test]
    fn test_clean_log_append_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let entry = CleanLogEntry {
            timestamp: "2026-01-01T00:00:00Z".into(),
            total_files: 5,
            total_bytes: 10240,
            success: 5,
            failed: 0,
            errors: vec![],
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

    #[serial_test::serial]
    #[test]
    fn test_expand_case_insensitive() {
        temp_env::with_var("TEMP", Some("C:\\TestTemp"), || {
            assert_eq!(expand_env("%temp%\\foo"), r"C:\TestTemp\foo");
            assert_eq!(expand_env("%Temp%\\foo"), r"C:\TestTemp\foo");
        });
    }

    #[serial_test::serial]
    #[test]
    fn test_expand_trailing_backslash_fixed() {
        temp_env::with_var("TEMP", Some("C:\\TestTemp"), || {
            let r = expand_env("%TEMP%\\");
            assert!(!r.starts_with('\\'), "should not start with \\");
        });
    }

    #[test]
    fn test_protected_trailing_space() {
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\config\SAM "
        )));
    }

    #[test]
    fn test_protected_trailing_dot() {
        assert!(is_path_protected(Path::new(r"C:\Windows\System32\config.")));
    }

    #[test]
    fn test_protected_forward_slash() {
        assert!(is_path_protected(Path::new(
            r"C:/Windows/System32/config/SAM"
        )));
    }

    #[test]
    fn test_protected_win32_namespace() {
        assert!(is_path_protected(Path::new(
            r"\\?\C:\Windows\System32\config\SAM"
        )));
    }

    #[test]
    fn test_protected_system_volume_information() {
        assert!(is_path_protected(Path::new(
            r"C:\System Volume Information\some"
        )));
    }

    #[serial_test::serial]
    #[test]
    fn test_allowed_separator_reject_adjacent() {
        temp_env::with_var("TEMP", Some("C:\\Temp"), || {
            let targets = vec![ScanTarget::new(
                "t".into(),
                "%TEMP%",
                SafetyLevel::Safe,
                Category::Temp,
                "".into(),
            )];
            assert!(!is_path_allowed(
                Path::new(r"C:\Temp_malicious\evil.exe".into()),
                &targets
            ));
        });
    }

    #[test]
    fn test_scan_target_block_filters() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.log"), vec![0u8; 5000]).unwrap();
        std::fs::write(dir.path().join("small.log"), vec![0u8; 100]).unwrap();
        std::fs::write(dir.path().join("big.txt"), vec![0u8; 5000]).unwrap();

        let target = ScanTarget::new(
            "test".into(),
            "%TEMP%",
            SafetyLevel::Safe,
            Category::Temp,
            "test".into(),
        )
        .with_min_size(4096)
        .with_glob(vec!["*.log".into()]);

        let (tx, _rx) = mpsc::channel::<ScanEvent>();
        let cancel = CancellationToken::new();
        let count = AtomicU64::new(0);
        let hit = AtomicBool::new(false);
        let mut batch = Vec::new();
        let (bytes, skipped) =
            scan_target_block(&target, dir.path(), &cancel, &count, &hit, &mut batch, &tx);

        assert_eq!(batch.len(), 1, "only big.log should match");
        assert_eq!(batch[0].path, dir.path().join("big.log"));
        assert_eq!(bytes, 5000);
        assert_eq!(skipped, 1, "small.log skipped by min_size");
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "global count = matched files"
        );
    }

    #[test]
    fn test_scan_target_block_parallel_count() {
        // 两个线程共享全局计数并发扫描，验证无丢失/重复
        let dir = tempfile::tempdir().unwrap();
        let sub1 = dir.path().join("a");
        let sub2 = dir.path().join("b");
        std::fs::create_dir_all(&sub1).unwrap();
        std::fs::create_dir_all(&sub2).unwrap();
        for i in 0..10 {
            std::fs::write(sub1.join(format!("f{i}.tmp")), vec![0u8; 2048]).unwrap();
            std::fs::write(sub2.join(format!("g{i}.tmp")), vec![0u8; 2048]).unwrap();
        }

        let target = ScanTarget::new(
            "t".into(),
            "%TEMP%",
            SafetyLevel::Safe,
            Category::Temp,
            "t".into(),
        )
        .with_min_size(1024);

        let count = Arc::new(AtomicU64::new(0));
        let hit = Arc::new(AtomicBool::new(false));
        let cancel = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<ScanEvent>();
        let mut handles = Vec::new();
        for sub in [sub1, sub2] {
            let count = count.clone();
            let hit = hit.clone();
            let cancel = cancel.clone();
            let target = target.clone();
            let tx = tx.clone();
            handles.push(std::thread::spawn(move || {
                let mut batch = Vec::new();
                scan_target_block(&target, &sub, &cancel, &count, &hit, &mut batch, &tx)
            }));
        }
        let mut total_bytes = 0u64;
        for h in handles {
            total_bytes += h.join().unwrap().0;
        }
        assert_eq!(
            count.load(Ordering::SeqCst),
            20,
            "10 + 10 files counted exactly"
        );
        assert_eq!(total_bytes, 20 * 2048);
    }

    #[test]
    fn test_wu_targets_config() {
        let targets = get_clean_targets();
        let dl = targets.iter().find(|t| t.id == "wu_download").unwrap();
        assert_eq!(dl.level, SafetyLevel::Safe, "下载缓存应可安全删除");
        let ds = targets.iter().find(|t| t.id == "wu_datastore").unwrap();
        assert_eq!(ds.level, SafetyLevel::Confirm, "更新数据库需确认");
        assert_eq!(
            ds.requires_service_stop.as_deref(),
            Some("wuauserv"),
            "DataStore 需停 wuauserv"
        );
        assert!(ds.glob_include.is_some(), "DataStore 仅清文件不删目录");
    }

    #[test]
    fn test_downloads_mtime_filters_recent() {
        let targets = get_clean_targets();
        let dl = targets.iter().find(|t| t.id == "downloads_old").unwrap();
        assert_eq!(dl.min_size, 102_400);
    }

    #[test]
    fn test_with_min_size_mb_converts_correctly() {
        let t = ScanTarget::new(
            "test".into(),
            "%TEMP%",
            SafetyLevel::Safe,
            Category::Temp,
            "test".into(),
        )
        .with_min_size_mb(50);
        assert_eq!(t.min_size, 52_428_800, "50MB should be 52_428_800 bytes");
        let t2 = ScanTarget::new(
            "test2".into(),
            "%TEMP%",
            SafetyLevel::Safe,
            Category::Temp,
            "test2".into(),
        )
        .with_min_size_mb(100);
        assert_eq!(
            t2.min_size, 104_857_600,
            "100MB should be 104_857_600 bytes"
        );
    }

    #[test]
    fn test_category_default_min_size() {
        assert_eq!(Category::Cache.default_min_size(), 512);
        assert_eq!(Category::Logs.default_min_size(), 4096);
        assert_eq!(Category::Temp.default_min_size(), 1024);
        assert_eq!(Category::Prefetch.default_min_size(), 1024);
        assert_eq!(Category::RecycleBin.default_min_size(), 1024);
        assert_eq!(Category::OldInstall.default_min_size(), 1024);
        assert_eq!(Category::AppCache.default_min_size(), 1024);
        assert_eq!(Category::DevCache.default_min_size(), 1024);
    }

    #[test]
    fn test_data_dir() {
        let dir = data_dir();
        assert!(
            dir.to_string_lossy().contains("PonyClean"),
            "data_dir should end with PonyClean"
        );
    }

    #[test]
    fn test_protected_globalroot_device() {
        assert!(is_path_protected(Path::new(
            r"\\.\GLOBALROOT\Device\HarddiskVolume1\Windows\System32\config\SAM"
        )));
        assert!(is_path_protected(Path::new(
            r"\\?\GLOBALROOT\Device\Harddisk0\Partition1\Windows\System32"
        )));
    }

    #[test]
    fn test_protected_null_byte_after_path() {
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\config\SAM\0..\..\Temp"
        )));
    }

    #[test]
    fn test_protected_mixed_separators() {
        assert!(is_path_protected(Path::new(
            r"C:\Windows/System32\config/SAM"
        )));
    }

    #[cfg_attr(not(windows), ignore)]
    #[serial_test::serial]
    #[test]
    fn test_env_injection_temp_is_protected() {
        temp_env::with_var("TEMP", Some(r"C:\Windows\System32\config"), || {
            let targets = get_clean_targets();
            let filtered: Vec<_> = targets
                .into_iter()
                .filter(|t| t.id == "user_temp")
                .collect();
            let resolved = resolve_targets(&filtered);
            assert!(
                resolved.is_empty(),
                "TEMP pointing to protected path must be rejected"
            );
        });
    }
}
