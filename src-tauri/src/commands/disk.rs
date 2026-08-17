use pony_core::cleaner::DeleteResult;
use pony_core::disk::{self, DiskEvent};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// 磁盘分析状态（单扫描锁：大文件 + 目录占用合并为一次遍历，TASK-026）
pub struct DiskState {
    pub is_scanning: Arc<AtomicBool>,
    pub cancel_flag: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    pub scan_root: Arc<Mutex<Option<PathBuf>>>,
}

/// 用户目录（大文件扫描默认根）
fn user_profile_root() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("C:\\"))
}

/// 事件转发线程：mpsc → Tauri 事件。
///
/// 进度/done/error 走 `disk-user-*` 通道；数据事件保留
/// `disk-large-files` / `disk-dir-usage`（前端分区块渲染）。
fn spawn_emitter(
    app_handle: AppHandle,
    rx: std::sync::mpsc::Receiver<DiskEvent>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for ev in rx {
            match ev {
                DiskEvent::Progress { scanned, current } => {
                    let _ = app_handle.emit(
                        "disk-user-progress",
                        serde_json::json!({ "scanned": scanned, "current": current }),
                    );
                }
                DiskEvent::LargeFiles { files } => {
                    let _ =
                        app_handle.emit("disk-large-files", serde_json::json!({ "files": files }));
                }
                DiskEvent::DirUsage { dirs } => {
                    let _ = app_handle.emit("disk-dir-usage", serde_json::json!({ "dirs": dirs }));
                }
                DiskEvent::Done => {
                    let _ = app_handle.emit("disk-user-done", serde_json::json!({}));
                }
                DiskEvent::Error(msg) => {
                    let _ =
                        app_handle.emit("disk-user-error", serde_json::json!({ "message": msg }));
                }
            }
        }
    })
}

/// 启动用户目录合并扫描（大文件 + 目录占用，一趟遍历）
#[tauri::command]
pub async fn start_user_scan(
    app: AppHandle,
    state: State<'_, DiskState>,
    min_bytes_mb: Option<u64>,
    max_depth: Option<usize>,
) -> Result<(), String> {
    if state.is_scanning.swap(true, Ordering::SeqCst) {
        return Err("Scan already in progress".into());
    }

    let is_scanning = state.is_scanning.clone();
    let cancel_holder = state.cancel_flag.clone();
    let root_holder = state.scan_root.clone();
    let app_handle = app.clone();
    // TASK-028：参数可选；未传时读配置（设置面板可调），统一 clamp 防越界
    let (cfg_mb, cfg_depth) = pony_core::cleaner::load_config().disk_scan_params();
    let min_bytes = min_bytes_mb
        .unwrap_or(cfg_mb)
        .clamp(50, 10_000)
        .saturating_mul(1024 * 1024);
    let depth = max_depth.unwrap_or(cfg_depth).clamp(1, 5);
    let root = user_profile_root();
    root_holder.lock().unwrap().replace(root.clone());

    tokio::task::spawn_blocking(move || {
        struct ScanGuard {
            flag: Arc<AtomicBool>,
        }
        impl Drop for ScanGuard {
            fn drop(&mut self) {
                self.flag.store(false, Ordering::SeqCst);
            }
        }
        let _guard = ScanGuard {
            flag: is_scanning.clone(),
        };

        let cancel = Arc::new(AtomicBool::new(false));
        *cancel_holder.lock().unwrap() = Some(cancel.clone());

        let (tx, rx) = std::sync::mpsc::channel::<DiskEvent>();
        let emit_handle = spawn_emitter(app_handle, rx);
        disk::scan_user_dir(tx, &root, min_bytes, &cancel, 1000, depth);
        let _ = emit_handle.join();
    });

    Ok(())
}

/// 取消磁盘扫描
#[tauri::command]
pub fn cancel_disk_scan(state: State<'_, DiskState>) -> Result<(), String> {
    if let Some(flag) = state.cancel_flag.lock().unwrap().take() {
        flag.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err("No scan in progress".into())
    }
}

/// 删除大文件（安全通道：受保护路径 + 扫描根验证 + 审计日志）
#[tauri::command]
pub async fn delete_large_files(
    state: State<'_, DiskState>,
    paths: Vec<String>,
) -> Result<DeleteResult, String> {
    let root = state
        .scan_root
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(user_profile_root);
    let pathbufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let root2 = root.clone();
    let inner = tokio::task::spawn_blocking(move || disk::delete_large_files(&pathbufs, &root2))
        .await
        .map_err(|e| e.to_string())?;
    Ok(inner)
}
