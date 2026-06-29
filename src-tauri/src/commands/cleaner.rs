use pony_core::cleaner::{self, CleanItem, DeleteResult};
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
                            accumulated.extend(items);
                            let _ = app_handle.emit("scan-items", serde_json::json!({
                                "items": accumulated,
                                "total_bytes": accumulated.iter().map(|i| i.size_bytes).sum::<u64>()
                            }));
                        }
                        Ok(cleaner::ScanEvent::Done { .. }) => {
                            let total: u64 = accumulated.iter().map(|i| i.size_bytes).sum();
                            let _ = app_handle.emit(
                                "scan-done",
                                serde_json::json!({
                                    "total_items": accumulated.len(), "total_bytes": total
                                }),
                            );
                            break;
                        }
                        Ok(cleaner::ScanEvent::Cancelled) => break,
                        Ok(cleaner::ScanEvent::Warning(msg)) => {
                            tracing::warn!("Scan warning: {msg}")
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
        is_scanning.store(false, Ordering::SeqCst);
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
pub async fn execute_clean(paths: Vec<String>) -> Result<DeleteResult, String> {
    let pathbufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let result = tokio::task::spawn_blocking(move || cleaner::delete_files(&pathbufs))
        .await
        .map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
pub async fn empty_recycle_bin() -> Result<(), String> {
    tokio::task::spawn_blocking(cleaner::empty_recycle_bin)
        .await
        .map_err(|e| e.to_string())?
}
