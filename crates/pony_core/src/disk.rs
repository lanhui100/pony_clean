//! 磁盘分析（Phase 3）：大文件扫描 + 目录空间占用分析。
//!
//! 与 `cleaner` 的关系：`cleaner` 专注于系统垃圾清理，本模块专注于
//! 定位用户数据中的空间黑洞（大文件、大目录），删除走安全通道
//! （受保护路径检查 + 审计日志）。

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;

/// 大文件类型（按扩展名推断）
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LargeFileKind {
    Video,
    Archive,
    Installer,
    Image,
    Document,
    Other,
}

impl LargeFileKind {
    fn from_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        let ext = lower.rsplit('.').next().unwrap_or("");
        match ext {
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "ts" | "m4v" | "rmvb"
            | "mpg" | "mpeg" => LargeFileKind::Video,
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" | "cab" | "wim" | "tgz"
            | "zst" => LargeFileKind::Archive,
            "exe" | "msi" | "msp" | "appx" | "msix" | "apk" | "dmg" => LargeFileKind::Installer,
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "raw" | "cr2" | "nef" | "heic"
            | "psd" => LargeFileKind::Image,
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "epub"
            | "csv" => LargeFileKind::Document,
            _ => LargeFileKind::Other,
        }
    }
}

/// 大文件删除风险级别
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LargeFileLevel {
    /// 低风险：用户数据（文档/视频/压缩包等），默认勾选
    Safe,
    /// 高风险：安装包/程序本体、应用数据（AppData），默认不勾选，删除需二次确认
    Confirm,
}

/// 判断大文件的删除风险级别：
/// - Confirm：安装包/程序本体（exe/msi 等，可能是运行中程序，无法可靠区分）或应用数据目录（AppData，删除可能损坏应用）
/// - Safe：其余用户数据
fn risk_level(path: &str, kind: &LargeFileKind) -> LargeFileLevel {
    if *kind == LargeFileKind::Installer {
        return LargeFileLevel::Confirm;
    }
    let lower = path.to_lowercase();
    if lower.contains("\\appdata\\") {
        return LargeFileLevel::Confirm;
    }
    LargeFileLevel::Safe
}

/// 大文件信息
#[derive(Clone, Debug, Serialize)]
pub struct LargeFile {
    pub path: String,
    pub size_bytes: u64,
    pub modified_secs: i64,
    pub kind: LargeFileKind,
    /// 删除风险级别（前端据此控制默认勾选与二次确认）
    pub level: LargeFileLevel,
}

/// 目录空间占用
#[derive(Clone, Debug, Serialize)]
pub struct DirUsage {
    pub path: String,
    pub size_bytes: u64,
    pub file_count: u64,
}

/// 磁盘分析进度事件
#[derive(Clone, Debug)]
pub enum DiskEvent {
    /// 扫描进度
    Progress {
        scanned: u64,
        current: String,
    },
    /// 大文件批次（扫描过程中分批推送）
    LargeFiles {
        files: Vec<LargeFile>,
    },
    /// 目录占用结果（扫描完成后一次性推送 Top N）
    DirUsage {
        dirs: Vec<DirUsage>,
    },
    Done,
    Error(String),
}

/// 遍历时跳过的目录名
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "__pycache__",
    ".svn",
    "$RECYCLE.BIN",
];

/// 用户目录内由 cleaner 负责的垃圾临时区（AppData\Local\Temp），大文件扫描跳过避免重复列出
const SKIP_TEMP_DIRS: &[&str] = &["temp"];

/// 用户目录内不应列出/删除的系统文件（注册表 hive 等，误删损坏用户配置）
const SKIP_SYSTEM_FILES: &[&str] = &[
    "ntuser.dat",
    "ntuser.dat.log1",
    "ntuser.dat.log2",
    "usrclass.dat",
    "ntuser.ini",
];

/// 推送进度事件（节流：每 200 文件一次）
fn send_progress(tx: &Sender<DiskEvent>, scanned: u64, current: &str, last_sent: &mut u64) {
    if scanned - *last_sent >= 200 || scanned == 0 {
        let _ = tx.send(DiskEvent::Progress {
            scanned,
            current: current.to_string(),
        });
        *last_sent = scanned;
    }
}

/// 扫描目录下所有大于 `min_bytes` 的文件（并行遍历，按大小降序）
///
/// 事件流：`Progress`（节流）→ `LargeFiles`（分批）→ `Done`。
/// `cancel` 为 true 时提前结束（不再推送事件）。
pub fn scan_large_files(
    tx: Sender<DiskEvent>,
    root: &Path,
    min_bytes: u64,
    cancel: &AtomicBool,
    max_files: usize,
) -> Vec<LargeFile> {
    let mut files: Vec<LargeFile> = Vec::new();
    let mut scanned = 0u64;
    let mut last_sent = 0u64;

    let walker = jwalk::WalkDir::new(root)
        .follow_links(false)
        .process_read_dir(|_depth, _path, _state, children| {
            let is_local_temp = _path
                .to_string_lossy()
                .to_lowercase()
                .ends_with("appdata\\local");
            children.retain(|e| {
                e.as_ref().ok().is_none_or(|entry| {
                    let name = entry.file_name.to_string_lossy();
                    if SKIP_DIRS.contains(&name.as_ref()) {
                        return false;
                    }
                    // AppData\Local\Temp 由 cleaner 负责，大文件扫描跳过
                    if is_local_temp && SKIP_TEMP_DIRS.iter().any(|t| name.eq_ignore_ascii_case(t))
                    {
                        return false;
                    }
                    true
                })
            });
            children.retain(|e| e.is_ok());
        });

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        scanned += 1;
        send_progress(
            &tx,
            scanned,
            &entry.file_name.to_string_lossy(),
            &mut last_sent,
        );

        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let size = meta.len();
        if size < min_bytes {
            continue;
        }
        let name = entry.file_name.to_string_lossy().to_string();
        // 跳过系统 hive 文件（误删损坏用户配置）
        if SKIP_SYSTEM_FILES
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
        {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let path_str = entry.path().to_string_lossy().to_string();
        let kind = LargeFileKind::from_name(&name);
        files.push(LargeFile {
            path: path_str.clone(),
            size_bytes: size,
            modified_secs: modified,
            level: risk_level(&path_str, &kind),
            kind,
        });
        // 分批推送（每 50 个）
        if files.len() % 50 == 0 {
            let batch = files.split_off(files.len() - 50);
            let _ = tx.send(DiskEvent::LargeFiles { files: batch });
        }
        if files.len() >= max_files {
            break;
        }
    }

    if !files.is_empty() {
        files.sort_by_key(|f| std::cmp::Reverse(f.size_bytes));
        let _ = tx.send(DiskEvent::LargeFiles {
            files: files.clone(),
        });
    }
    let _ = tx.send(DiskEvent::Done);
    files
}

/// 合并扫描：单次遍历同时产出大文件（≥min_bytes）与目录占用（父目录深度 ≤dir_depth，TASK-026）
///
/// 事件流：`Progress`（节流）→ `LargeFiles`（分批）→ `DirUsage`（结束一次性）→ `Done`。
/// 与旧双函数（`scan_large_files` + `scan_dir_usage`）行为等价，仅省一次全目录遍历。
pub fn scan_user_dir(
    tx: Sender<DiskEvent>,
    root: &Path,
    min_bytes: u64,
    cancel: &AtomicBool,
    max_files: usize,
    dir_depth: usize,
) -> (Vec<LargeFile>, Vec<DirUsage>) {
    let mut files: Vec<LargeFile> = Vec::new();
    let mut usage: std::collections::HashMap<String, (u64, u64)> = std::collections::HashMap::new();
    let mut scanned = 0u64;
    let mut last_sent = 0u64;

    let walker = jwalk::WalkDir::new(root)
        .follow_links(false)
        .process_read_dir(|_depth, _path, _state, children| {
            let is_local_temp = _path
                .to_string_lossy()
                .to_lowercase()
                .ends_with("appdata\\local");
            children.retain(|e| {
                e.as_ref().ok().is_none_or(|entry| {
                    let name = entry.file_name.to_string_lossy();
                    if SKIP_DIRS.contains(&name.as_ref()) {
                        return false;
                    }
                    // AppData\Local\Temp 由 cleaner 负责，大文件扫描跳过
                    if is_local_temp && SKIP_TEMP_DIRS.iter().any(|t| name.eq_ignore_ascii_case(t))
                    {
                        return false;
                    }
                    true
                })
            });
            children.retain(|e| e.is_ok());
        });

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        scanned += 1;
        send_progress(
            &tx,
            scanned,
            &entry.file_name.to_string_lossy(),
            &mut last_sent,
        );

        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let size = meta.len();
        let name = entry.file_name.to_string_lossy().to_string();
        // 跳过系统 hive 文件（误删损坏用户配置）
        if SKIP_SYSTEM_FILES
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
        {
            continue;
        }

        // ── 目录占用聚合（仅父目录深度 < dir_depth，与旧 max_depth 语义一致）──
        let mut dir = entry.path();
        dir.pop();
        let parent_depth = entry.depth().saturating_sub(1);
        if parent_depth < dir_depth {
            let key = dir.to_string_lossy().to_string();
            let (s, c) = usage.entry(key).or_insert((0, 0));
            *s += size;
            *c += 1;
        }

        // ── 大文件收集（全深，阈值/风险分级/跳过逻辑不变）──
        if size < min_bytes {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let path_str = entry.path().to_string_lossy().to_string();
        let kind = LargeFileKind::from_name(&name);
        files.push(LargeFile {
            path: path_str.clone(),
            size_bytes: size,
            modified_secs: modified,
            level: risk_level(&path_str, &kind),
            kind,
        });
        // 分批推送（每 50 个）
        if files.len() % 50 == 0 {
            let batch = files.split_off(files.len() - 50);
            let _ = tx.send(DiskEvent::LargeFiles { files: batch });
        }
        if files.len() >= max_files {
            break;
        }
    }

    if !files.is_empty() {
        files.sort_by_key(|f| std::cmp::Reverse(f.size_bytes));
        let _ = tx.send(DiskEvent::LargeFiles {
            files: files.clone(),
        });
    }
    let mut dirs: Vec<DirUsage> = usage
        .into_iter()
        .map(|(path, (size_bytes, file_count))| DirUsage {
            path,
            size_bytes,
            file_count,
        })
        .collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.size_bytes));
    dirs.truncate(100);
    let _ = tx.send(DiskEvent::DirUsage { dirs: dirs.clone() });
    let _ = tx.send(DiskEvent::Done);
    (files, dirs)
}

/// 扫描目录占用：按目录聚合文件大小（限深度，只返回有文件的目录）
///
/// 事件流：`Progress`（节流）→ `DirUsage`（Top 100，降序）→ `Done`。
pub fn scan_dir_usage(
    tx: Sender<DiskEvent>,
    root: &Path,
    max_depth: usize,
    cancel: &AtomicBool,
) -> Vec<DirUsage> {
    let mut usage: std::collections::HashMap<String, (u64, u64)> = std::collections::HashMap::new();
    let mut scanned = 0u64;
    let mut last_sent = 0u64;

    let walker = jwalk::WalkDir::new(root)
        .follow_links(false)
        .max_depth(max_depth)
        .process_read_dir(|_depth, _path, _state, children| {
            children.retain(|e| {
                e.as_ref().ok().is_none_or(|entry| {
                    let name = entry.file_name.to_string_lossy();
                    !SKIP_DIRS.contains(&name.as_ref())
                })
            });
            children.retain(|e| e.is_ok());
        });

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        scanned += 1;
        send_progress(
            &tx,
            scanned,
            &entry.file_name.to_string_lossy(),
            &mut last_sent,
        );

        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let size = meta.len();
        let name = entry.file_name.to_string_lossy();
        // 系统 hive 文件不参与目录占用统计（与 scan_user_dir 语义一致）
        if SKIP_SYSTEM_FILES
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
        {
            continue;
        }
        let mut dir = entry.path();
        dir.pop();
        let key = dir.to_string_lossy().to_string();
        let (s, c) = usage.entry(key).or_insert((0, 0));
        *s += size;
        *c += 1;
    }

    let mut dirs: Vec<DirUsage> = usage
        .into_iter()
        .map(|(path, (size_bytes, file_count))| DirUsage {
            path,
            size_bytes,
            file_count,
        })
        .collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.size_bytes));
    dirs.truncate(100);
    let _ = tx.send(DiskEvent::DirUsage { dirs: dirs.clone() });
    let _ = tx.send(DiskEvent::Done);
    dirs
}

/// 删除大文件（安全通道：受保护路径检查 + 扫描根前缀验证 + 审计日志）
pub fn delete_large_files(paths: &[PathBuf], scan_root: &Path) -> crate::cleaner::DeleteResult {
    let root_canon = std::fs::canonicalize(scan_root).unwrap_or_else(|_| scan_root.to_path_buf());
    let root_str = root_canon.to_string_lossy().to_lowercase();
    let mut result = crate::cleaner::DeleteResult::default();
    let total = paths.len() as u64;
    let mut done = 0u64;

    let mut deleted: Vec<PathBuf> = Vec::new();
    for path in paths {
        done += 1;
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
        let safe_str = safe_path.to_string_lossy().to_lowercase();
        // 必须在扫描根内（防越权删除任意路径）
        if !safe_str.starts_with(&root_str) {
            result.failed += 1;
            result
                .errors
                .push(format!("Path outside scan root: {}", safe_path.display()));
            continue;
        }
        if crate::cleaner::is_path_protected(&safe_path) {
            result.failed += 1;
            result
                .errors
                .push(format!("Protected path: {}", safe_path.display()));
            continue;
        }
        // 系统 hive 文件纵深防御（扫描时已排除，此处兜底）
        if let Some(fname) = safe_path.file_name() {
            let fname = fname.to_string_lossy();
            if SKIP_SYSTEM_FILES
                .iter()
                .any(|s| fname.eq_ignore_ascii_case(s))
            {
                result.failed += 1;
                result.errors.push(format!(
                    "System file not deletable: {}",
                    safe_path.display()
                ));
                continue;
            }
        }

        match std::fs::remove_file(&safe_path) {
            Ok(()) => {
                result.success += 1;
                deleted.push(safe_path);
            }
            Err(e) => {
                result.failed += 1;
                // 占用文件给出明确原因（不做延迟删除，保持简单）
                if crate::cleaner::is_file_busy(&safe_path) {
                    result
                        .errors
                        .push(format!("文件被进程占用，无法删除: {}", safe_path.display()));
                } else {
                    result.errors.push(format!("{e}"));
                }
            }
        }
    }

    // 审计日志（与 cleaner 共用格式）
    if !deleted.is_empty() {
        let entry = crate::cleaner::CleanLogEntry {
            timestamp: crate::cleaner::timestamp_now(),
            total_files: total,
            total_bytes: paths
                .iter()
                .filter_map(|p| p.metadata().ok())
                .map(|m| m.len())
                .sum(),
            success: result.success,
            failed: result.failed,
            errors: result.errors.clone(),
            by_category: std::collections::HashMap::new(),
        };
        let _ = crate::cleaner::append_clean_log(&entry);
    }

    let _ = done;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_kind() {
        assert_eq!(LargeFileKind::from_name("movie.mp4"), LargeFileKind::Video);
        assert_eq!(
            LargeFileKind::from_name("backup.zip"),
            LargeFileKind::Archive
        );
        assert_eq!(
            LargeFileKind::from_name("setup.exe"),
            LargeFileKind::Installer
        );
        assert_eq!(LargeFileKind::from_name("photo.JPG"), LargeFileKind::Image);
        assert_eq!(
            LargeFileKind::from_name("report.pdf"),
            LargeFileKind::Document
        );
        assert_eq!(LargeFileKind::from_name("data.bin"), LargeFileKind::Other);
        assert_eq!(LargeFileKind::from_name("noext"), LargeFileKind::Other);
    }

    #[test]
    fn test_risk_level() {
        // 安装包/程序本体 → Confirm
        assert_eq!(
            risk_level(
                "C:\\Users\\u\\Downloads\\setup.exe",
                &LargeFileKind::Installer
            ),
            LargeFileLevel::Confirm
        );
        // AppData 应用数据 → Confirm
        assert_eq!(
            risk_level(
                "C:\\Users\\u\\AppData\\Local\\WeChat\\big.db",
                &LargeFileKind::Other
            ),
            LargeFileLevel::Confirm
        );
        // 普通用户数据 → Safe
        assert_eq!(
            risk_level("C:\\Users\\u\\Documents\\movie.mp4", &LargeFileKind::Video),
            LargeFileLevel::Safe
        );
        assert_eq!(
            risk_level(
                "C:\\Users\\u\\Downloads\\backup.zip",
                &LargeFileKind::Archive
            ),
            LargeFileLevel::Safe
        );
    }

    #[test]
    fn test_scan_large_files_skips_local_temp_and_system_files() {
        let dir = tempfile::tempdir().unwrap();
        // AppData\Local\Temp（垃圾区，归 cleaner）应跳过
        let temp_dir = dir.path().join("AppData").join("Local").join("Temp");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("huge.tmp"), vec![0u8; 2048]).unwrap();
        // 系统 hive 文件应跳过
        std::fs::write(dir.path().join("NTUSER.DAT"), vec![0u8; 2048]).unwrap();
        // 普通大文件应列出
        std::fs::write(dir.path().join("big.bin"), vec![0u8; 2048]).unwrap();

        let (tx, _rx) = std::sync::mpsc::channel();
        let cancel = AtomicBool::new(false);
        let files = scan_large_files(tx, dir.path(), 1024, &cancel, 100);

        assert_eq!(files.len(), 1, "only big.bin should be listed");
        assert_eq!(files[0].path, dir.path().join("big.bin").to_string_lossy());
    }

    #[test]
    fn test_scan_large_files_finds_big() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.bin");
        let small = dir.path().join("small.txt");
        std::fs::write(&big, vec![0u8; 1024 * 1024]).unwrap();
        std::fs::write(&small, vec![0u8; 10]).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = AtomicBool::new(false);
        let files = scan_large_files(tx, dir.path(), 1024, &cancel, 100);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].size_bytes, 1024 * 1024);
        // 事件流含 Done
        let mut saw_done = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, DiskEvent::Done) {
                saw_done = true;
            }
        }
        assert!(saw_done, "Done event should be sent");
    }

    #[test]
    fn test_scan_large_files_cancel() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..50 {
            std::fs::write(dir.path().join(format!("f{i}.bin")), vec![0u8; 2048]).unwrap();
        }
        let (tx, _rx) = std::sync::mpsc::channel();
        let cancel = AtomicBool::new(true); // 立即取消
        let files = scan_large_files(tx, dir.path(), 1024, &cancel, 100);
        assert!(files.is_empty(), "cancelled scan should return nothing");
    }

    #[test]
    fn test_scan_dir_usage_aggregates() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.path().join("a.bin"), vec![0u8; 100]).unwrap();
        std::fs::write(sub.join("b.bin"), vec![0u8; 200]).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = AtomicBool::new(false);
        let dirs = scan_dir_usage(tx, dir.path(), 3, &cancel);

        let sub_entry = dirs
            .iter()
            .find(|d| d.path.ends_with("sub"))
            .expect("sub dir should be listed");
        assert_eq!(sub_entry.size_bytes, 200);
        assert_eq!(sub_entry.file_count, 1);

        let mut saw_done = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, DiskEvent::Done) {
                saw_done = true;
            }
        }
        assert!(saw_done);
    }

    #[test]
    fn test_scan_user_dir_matches_old_functions() {
        // TASK-026: 合并单遍历与旧双函数结果一致
        let dir = tempfile::tempdir().unwrap();
        let sub1 = dir.path().join("sub1");
        let deep = dir.path().join("sub2").join("deep");
        std::fs::create_dir_all(&sub1).unwrap();
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(sub1.join("a.bin"), vec![0u8; 2000]).unwrap();
        std::fs::write(deep.join("b.bin"), vec![0u8; 3000]).unwrap();
        std::fs::write(dir.path().join("small.txt"), vec![0u8; 100]).unwrap();
        // Temp 跳过（垃圾区归 cleaner）
        let temp_dir = dir.path().join("AppData").join("Local").join("Temp");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("huge.tmp"), vec![0u8; 5000]).unwrap();
        // 系统 hive 跳过
        std::fs::write(dir.path().join("NTUSER.DAT"), vec![0u8; 5000]).unwrap();

        let cancel = AtomicBool::new(false);
        let (tx1, _) = std::sync::mpsc::channel();
        let old_files = scan_large_files(tx1, dir.path(), 1024, &cancel, 100);
        let (tx2, _) = std::sync::mpsc::channel();
        let old_dirs = scan_dir_usage(tx2, dir.path(), 3, &cancel);
        let (tx3, _) = std::sync::mpsc::channel();
        let (new_files, new_dirs) = scan_user_dir(tx3, dir.path(), 1024, &cancel, 100, 3);

        let old_f: std::collections::HashSet<(String, u64)> = old_files
            .iter()
            .map(|f| (f.path.clone(), f.size_bytes))
            .collect();
        let new_f: std::collections::HashSet<(String, u64)> = new_files
            .iter()
            .map(|f| (f.path.clone(), f.size_bytes))
            .collect();
        assert_eq!(old_f, new_f, "large files must match old scan_large_files");

        let old_d: std::collections::HashSet<(String, u64, u64)> = old_dirs
            .iter()
            .map(|d| (d.path.clone(), d.size_bytes, d.file_count))
            .collect();
        let new_d: std::collections::HashSet<(String, u64, u64)> = new_dirs
            .iter()
            .map(|d| (d.path.clone(), d.size_bytes, d.file_count))
            .collect();
        assert_eq!(old_d, new_d, "dir usage must match old scan_dir_usage");
    }

    #[test]
    fn test_delete_large_files_outside_root_rejected() {
        let outside = tempfile::tempdir().unwrap();
        let f = outside.path().join("x.bin");
        std::fs::write(&f, vec![0u8; 10]).unwrap();

        let root = tempfile::tempdir().unwrap();
        let result = delete_large_files(&[f.clone()], root.path());
        assert_eq!(result.failed, 1);
        assert_eq!(result.success, 0);
        assert!(f.exists(), "file outside root must not be deleted");
    }

    #[test]
    fn test_delete_large_files_within_root_ok() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("big.bin");
        std::fs::write(&f, vec![0u8; 1024]).unwrap();

        let result = delete_large_files(&[f.clone()], dir.path());
        assert_eq!(result.success, 1);
        assert!(!f.exists(), "file within root should be deleted");
    }
}
