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

/// 大文件信息
#[derive(Clone, Debug, Serialize)]
pub struct LargeFile {
    pub path: String,
    pub size_bytes: u64,
    pub modified_secs: i64,
    pub kind: LargeFileKind,
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
        if size < min_bytes {
            continue;
        }
        let name = entry.file_name.to_string_lossy().to_string();
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        files.push(LargeFile {
            path: entry.path().to_string_lossy().to_string(),
            size_bytes: size,
            modified_secs: modified,
            kind: LargeFileKind::from_name(&name),
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

        match std::fs::remove_file(&safe_path) {
            Ok(()) => {
                result.success += 1;
                deleted.push(safe_path);
            }
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!("{e}"));
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
