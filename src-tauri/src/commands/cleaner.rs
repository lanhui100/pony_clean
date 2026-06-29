use pony_core::cleaner::{self, CleanItem, DeleteProgress, DeleteResult};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

pub struct CleanerState {
    pub is_scanning: Arc<AtomicBool>,
    pub cancel_token: Arc<Mutex<Option<CancellationToken>>>,
}

#[tauri::command]
pub async fn start_scan(app: AppHandle, state: State<'_, CleanerState>) -> Result<(), String> {
    if state.is_scanning.swap(true, Ordering::SeqCst) {
        return Err("Scan already in progress".into());
    }

    let is_scanning = state.is_scanning.clone();
    let cancel_token_holder = state.cancel_token.clone();
    let app_handle = app.clone();

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

        let (tx, rx) = std::sync::mpsc::channel();
        match cleaner::start_scan(tx) {
            Ok((cmd, cancel_token)) => {
                *cancel_token_holder.lock().unwrap() = Some(cancel_token);
                let mut accumulated: Vec<CleanItem> = Vec::new();
                loop {
                    match rx.recv() {
                        Ok(cleaner::ScanEvent::Progress { scanned, current }) => {
                            let _ = app_handle.emit(
                                "scan-progress",
                                serde_json::json!({
                                    "scanned": scanned, "current": current
                                }),
                            );
                        }
                        Ok(cleaner::ScanEvent::ItemsFound { items, .. }) => {
                            let batch: Vec<CleanItem> = items;
                            let total = accumulated.iter().map(|i| i.size_bytes).sum::<u64>()
                                + batch.iter().map(|i| i.size_bytes).sum::<u64>();
                            accumulated.extend(batch.iter().cloned());
                            let _ = app_handle.emit("scan-items", serde_json::json!({
                                "items": batch,
                                "total_bytes": total
                            }));
                        }
                        Ok(cleaner::ScanEvent::Done { skipped_small, .. }) => {
                            let total: u64 = accumulated.iter().map(|i| i.size_bytes).sum();
                            let _ = app_handle.emit(
                                "scan-done",
                                serde_json::json!({
                                    "total_items": accumulated.len(), "total_bytes": total,
                                    "skipped_small": skipped_small
                                }),
                            );
                            break;
                        }
                        Ok(cleaner::ScanEvent::Cancelled) => {
                            let _ = app_handle.emit("scan-cancelled", serde_json::json!({}));
                            break;
                        }
                        Ok(cleaner::ScanEvent::Warning(msg)) => {
                            tracing::warn!("Scan warning: {msg}");
                            let _ = app_handle.emit("scan-warning", serde_json::json!({ "message": msg }));
                        }
                        Err(_) => break,
                    }
                }
                let _ = cmd.send(cleaner::CleanCommand::Shutdown);
            }
            Err(e) => {
                let _ = app_handle.emit("scan-error", serde_json::json!({ "message": e }));
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, CleanerState>) -> Result<(), String> {
    if let Some(token) = state.cancel_token.lock().unwrap().take() {
        token.cancel();
        Ok(())
    } else {
        Err("No scan in progress".into())
    }
}

#[tauri::command]
pub async fn execute_clean(app: AppHandle, paths: Vec<String>) -> Result<DeleteResult, String> {
    let pathbufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let app_handle = app.clone();
    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<DeleteProgress>();

    let progress_handle = tokio::task::spawn_blocking(move || {
        for p in progress_rx {
            let _ = app_handle.emit("delete-progress", serde_json::json!({
                "done": p.done, "total": p.total, "current": p.current
            }));
        }
    });

    let result = tokio::task::spawn_blocking(move || {
        cleaner::delete_files_with_progress(&pathbufs, Some(progress_tx))
    })
        .await
        .map_err(|e| e.to_string())?;

    let _ = progress_handle.await;
    Ok(result)
}

#[tauri::command]
pub async fn empty_recycle_bin() -> Result<(), String> {
    tokio::task::spawn_blocking(cleaner::empty_recycle_bin)
        .await
        .map_err(|e| e.to_string())?
}
