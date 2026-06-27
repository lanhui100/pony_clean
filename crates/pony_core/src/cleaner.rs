use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

const BATCH_SIZE: usize = 500;

/// 安全级别
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum SafetyLevel {
    Safe,
    Confirm,
    Forbidden,
}

/// 可清理项
#[derive(Clone, Debug, Serialize)]
pub struct CleanItem {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub level: SafetyLevel,
    pub category: String,
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
    },
    Cancelled,
    Warning(String),
}

/// 清理命令
#[derive(Debug)]
pub enum CleanCommand {
    Execute(Vec<PathBuf>),
    EmptyRecycleBin,
    CancelScan,
    Shutdown,
}

/// 删除结果
#[derive(Clone, Debug, Default, Serialize)]
pub struct DeleteResult {
    pub success: u64,
    pub failed: u64,
    pub errors: Vec<String>,
}

/// 扫描目标分类
#[derive(Clone, Debug)]
pub struct ScanTarget {
    pub path: String,
    pub level: SafetyLevel,
    pub category: String,
    pub description: &'static str,
}

/// 获取系统盘符
fn system_drive() -> String {
    let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    sys_root.get(..2).unwrap_or("C:").to_string()
}

/// 展开环境变量（模拟 ExpandEnvironmentStringsW，仅支持基本变量）
fn expand_env(raw: &str) -> String {
    let mut s = raw.to_string();
    let vars = [
        ("%TEMP%", std::env::var("TEMP").ok()),
        ("%LOCALAPPDATA%", std::env::var("LOCALAPPDATA").ok()),
        ("%APPDATA%", std::env::var("APPDATA").ok()),
        ("%WINDIR%", std::env::var("SystemRoot").ok()),
        ("%SYSTEMROOT%", std::env::var("SystemRoot").ok()),
        ("%USERPROFILE%", std::env::var("USERPROFILE").ok()),
    ];
    for (pattern, value) in &vars {
        if let Some(v) = value {
            s = s.replace(pattern, v);
        }
    }
    s.replace("%SYSTEMDRIVE%", &system_drive())
}

/// 安全扫描路径列表
pub fn get_clean_targets() -> Vec<ScanTarget> {
    let d = system_drive();
    vec![
        ScanTarget {
            path: "%TEMP%".into(),
            level: SafetyLevel::Safe,
            category: "temp".into(),
            description: "用户临时文件",
        },
        ScanTarget {
            path: "%LOCALAPPDATA%\\Temp".into(),
            level: SafetyLevel::Safe,
            category: "temp".into(),
            description: "当前用户临时文件",
        },
        ScanTarget {
            path: "%WINDIR%\\Temp".into(),
            level: SafetyLevel::Confirm,
            category: "temp".into(),
            description: "系统临时文件（Windows Update 可能在使用）",
        },
        ScanTarget {
            path: format!("{d}\\Windows\\Prefetch"),
            level: SafetyLevel::Confirm,
            category: "prefetch".into(),
            description: "应用启动缓存（清空后首次引导变慢）",
        },
        ScanTarget {
            path: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Code Cache".into(),
            level: SafetyLevel::Safe,
            category: "cache".into(),
            description: "Chrome 缓存",
        },
        ScanTarget {
            path: "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Code Cache".into(),
            level: SafetyLevel::Safe,
            category: "cache".into(),
            description: "Edge 缓存",
        },
        ScanTarget {
            path: "%LOCALAPPDATA%\\Mozilla\\Firefox\\Profiles".into(),
            level: SafetyLevel::Safe,
            category: "cache".into(),
            description: "Firefox 缓存（自动匹配 profile 目录）",
        },
        ScanTarget {
            path: format!("{d}\\$Recycle.Bin"),
            level: SafetyLevel::Safe,
            category: "recycle_bin".into(),
            description: "回收站",
        },
    ]
}

/// 受保护路径前缀（禁止删除）
/// 使用 %SYSTEMDRIVE% 占位，运行时动态匹配
const PROTECTED_PREFIXES: &[&str] = &[
    "%SYSTEMDRIVE%\\Windows\\System32",
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
];

/// 检查路径是否受保护
pub fn is_path_protected(path: &Path) -> bool {
    let d = system_drive().to_lowercase();

    // 去除 Win32 命名空间前缀 (\\?\ 和 \\.\) 和空字节
    let raw = path.to_string_lossy();
    let raw = raw.trim_start_matches("\\\\?\\");
    let raw = raw.trim_start_matches("\\\\.\\");
    let raw = raw.trim_start_matches("//?/");
    let normalized = raw.replace('/', "\\").to_lowercase();

    // 去除嵌入空字节后的截断部分
    let cleaned = match normalized.split('\0').next() {
        Some(s) => s,
        None => return true,
    };

    // 将系统盘符统一到 C:
    let on_c = cleaned.replacen(&d, "c:", 1);
    PROTECTED_PREFIXES.iter().any(|p| {
        let p = p.replace("%SYSTEMDRIVE%", "c:");
        on_c.starts_with(&p.to_lowercase())
    }) || cleaned == format!("{d}\\")
        || path.parent().is_none()
}

/// 验证路径是否在允许的扫描目标内
pub fn is_path_allowed(path: &Path, targets: &[ScanTarget]) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    targets.iter().any(|t| {
        let expanded = expand_env(&t.path).to_lowercase();
        path_str.starts_with(&expanded)
    })
}

/// 展开扫描目标为实际路径列表
/// 返回的路径均已通过 is_path_protected 验证
pub fn resolve_targets(targets: &[ScanTarget]) -> Vec<PathBuf> {
    targets
        .iter()
        .filter(|t| t.level != SafetyLevel::Forbidden)
        .flat_map(|t| {
            let expanded = expand_env(&t.path);
            let p = PathBuf::from(&expanded);

            // 跳过受保护路径（防御环境变量注入）
            if is_path_protected(&p) {
                return vec![];
            }

            if t.category == "cache" && expanded.contains("Profiles") {
                // Firefox: 需要遍历 profiles 目录
                if let Ok(entries) = std::fs::read_dir(&p) {
                    let results: Vec<PathBuf> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_name().to_string_lossy().contains("default"))
                        .map(|e| e.path().join("cache2").join("entries"))
                        .collect();
                    if results.is_empty() {
                        // 未找到 profile 目录，不返回父路径
                        vec![]
                    } else {
                        results
                    }
                } else {
                    // Firefox 未安装或 profiles 不可访问，跳过
                    vec![]
                }
            } else {
                vec![p]
            }
        })
        .collect()
}

/// 全局扫描防重入锁
static SCAN_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// 启动 C盘扫描任务
///
/// 扫描已 resolve 的安全路径，通过 mpsc 推送 ScanEvent。
/// 支持防重入（使用 AtomicBool 守卫）。
/// 返回 (cmd_tx, cancel_token)。
pub fn start_scan(
    tx: mpsc::Sender<ScanEvent>,
) -> Result<(mpsc::Sender<CleanCommand>, CancellationToken), String> {
    if SCAN_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err("Scan already in progress".into());
    }

    let (cmd_tx, cmd_rx) = mpsc::channel::<CleanCommand>();
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    let targets = get_clean_targets();
    let resolved = resolve_targets(&targets);
    if resolved.is_empty() {
        SCAN_IN_PROGRESS.store(false, Ordering::SeqCst);
        return Err("No scan targets available".into());
    }

    tokio::task::spawn_blocking(move || {
        // Guard: 退出时释放防重入锁
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

        let mut total_items = 0u64;
        let mut total_bytes = 0u64;
        let mut batch = Vec::with_capacity(BATCH_SIZE);

        for target in &resolved {
            if cancel_token_clone.is_cancelled() {
                // flush 剩余批次
                if !batch.is_empty() {
                    let _ = tx.send(ScanEvent::ItemsFound {
                        items: batch,
                        batch_complete: false,
                    });
                }
                let _ = tx.send(ScanEvent::Cancelled);
                return;
            }

            let category = targets
                .iter()
                .find(|t| {
                    expand_env(&t.path).to_lowercase() == target.to_string_lossy().to_lowercase()
                        || target
                            .to_string_lossy()
                            .to_lowercase()
                            .starts_with(&expand_env(&t.path).to_lowercase())
                })
                .map(|t| t.category.clone())
                .unwrap_or_default();

            let walk_dir = jwalk::WalkDir::new(target)
                .follow_links(false)
                .process_read_dir(|_depth, _path, _read_dir_state, children| {
                    children.retain(|e| e.is_ok());
                });

            for entry in walk_dir.into_iter().filter_map(|e| e.ok()) {
                if cancel_token_clone.is_cancelled() {
                    if !batch.is_empty() {
                        let _ = tx.send(ScanEvent::ItemsFound {
                            items: std::mem::take(&mut batch),
                            batch_complete: false,
                        });
                    }
                    let _ = tx.send(ScanEvent::Cancelled);
                    return;
                }

                if !entry.file_type().is_file() {
                    continue;
                }

                let size = match entry.metadata() {
                    Ok(m) => m.len(),
                    Err(_) => continue,
                };
                if size == 0 {
                    continue;
                }

                total_items += 1;
                total_bytes += size;

                batch.push(CleanItem {
                    path: entry.path(),
                    size_bytes: size,
                    level: SafetyLevel::Safe,
                    category: category.clone(),
                });

                if batch.len() >= BATCH_SIZE {
                    let _ = tx.send(ScanEvent::ItemsFound {
                        items: std::mem::take(&mut batch),
                        batch_complete: false,
                    });
                }

                if total_items % 1000 == 0 {
                    let _ = tx.send(ScanEvent::Progress {
                        scanned: total_items,
                        current: target.to_string_lossy().to_string(),
                    });
                }
            }
        }

        // 发送剩余批次
        if !batch.is_empty() {
            let _ = tx.send(ScanEvent::ItemsFound {
                items: batch,
                batch_complete: true,
            });
        }

        let _ = tx.send(ScanEvent::Done {
            total_items,
            total_bytes,
        });
    });

    // 后台命令处理器（spawn_blocking 因为 cmd_rx 是 std::sync::mpsc）
    let cancel_token_cmd = cancel_token.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CleanCommand::CancelScan => {
                    cancel_token_cmd.cancel();
                }
                CleanCommand::Shutdown => break,
                _ => {}
            }
        }
    });

    Ok((cmd_tx, cancel_token))
}

/// 删除文件（阻塞调用，使用 spawn_blocking 执行）
///
/// 降级策略：
/// 1. DeleteFileW — 立即删除
/// 2. MoveFileExW + MOVEFILE_DELAY_UNTIL_REBOOT — 被占用时降级
/// 3. 跳过
pub fn delete_files(paths: &[PathBuf]) -> DeleteResult {
    let targets = get_clean_targets();
    let mut result = DeleteResult::default();

    for path in paths {
        // 规范化路径防止 .. 遍历和正斜杠绕过
        let safe_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(e) => {
                result.failed += 1;
                result
                    .errors
                    .push(format!("Cannot resolve path {}: {e}", path.display()));
                continue;
            }
        };

        // 后端强制执行安全验证
        if is_path_protected(&safe_path) {
            result.failed += 1;
            result
                .errors
                .push(format!("Protected path: {}", safe_path.display()));
            continue;
        }
        if !is_path_allowed(&safe_path, &targets) {
            result.failed += 1;
            result
                .errors
                .push(format!("Path not in scan scope: {}", safe_path.display()));
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
    }

    result
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
    fn test_is_path_protected_system32() {
        assert!(is_path_protected(Path::new(
            r"C:\Windows\System32\kernel32.dll"
        )));
        assert!(is_path_protected(Path::new(r"C:\Windows\System32")));
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
        let targets = vec![ScanTarget {
            path: "%TEMP%".into(),
            level: SafetyLevel::Safe,
            category: "temp".into(),
            description: "",
        }];
        let temp = std::env::var("TEMP").unwrap();
        let test_path = PathBuf::from(&temp).join("test.txt");
        assert!(is_path_allowed(&test_path, &targets));
    }

    #[test]
    fn test_is_path_allowed_rejected() {
        let targets = vec![ScanTarget {
            path: "%TEMP%".into(),
            level: SafetyLevel::Safe,
            category: "temp".into(),
            description: "",
        }];
        assert!(!is_path_allowed(
            Path::new(r"C:\Windows\System32\test.dll"),
            &targets,
        ));
    }

    #[test]
    fn test_is_path_allowed_cache_category() {
        let targets = vec![ScanTarget {
            path: "%LOCALAPPDATA%\\Google\\Chrome".into(),
            level: SafetyLevel::Safe,
            category: "cache".into(),
            description: "",
        }];
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let test_path = PathBuf::from(&local).join("Google\\Chrome\\Cache\\f_000001");
        assert!(is_path_allowed(&test_path, &targets));
    }

    #[test]
    fn test_resolve_targets_excludes_forbidden() {
        let targets = vec![
            ScanTarget {
                path: "%TEMP%".into(),
                level: SafetyLevel::Safe,
                category: "temp".into(),
                description: "",
            },
            ScanTarget {
                path: "C:\\Windows\\System32".into(),
                level: SafetyLevel::Forbidden,
                category: "temp".into(),
                description: "",
            },
        ];
        let resolved = resolve_targets(&targets);
        assert!(
            resolved
                .iter()
                .any(|p| p.to_string_lossy().contains("Temp")),
            "should include Temp"
        );
        assert!(
            !resolved
                .iter()
                .any(|p| p.to_string_lossy().contains("System32")),
            "should NOT include System32"
        );
    }

    #[test]
    fn test_resolve_targets_skips_protected() {
        // SystemRoot 已经是 C:\Windows，直接拼接后缀
        let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        let protected = format!("{}\\System32", sys_root);
        let targets = vec![ScanTarget {
            path: protected,
            level: SafetyLevel::Safe,
            category: "temp".into(),
            description: "",
        }];
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
        };
        match d {
            ScanEvent::Done { total_items, .. } => assert_eq!(total_items, 1000),
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
}
