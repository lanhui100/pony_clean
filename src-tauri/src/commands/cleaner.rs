use pony_core::cleaner::{self, CleanItem, DeleteProgress, DeleteResult, ScanWarning};
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
                            let _ = app_handle.emit(
                                "scan-items",
                                serde_json::json!({
                                    "items": batch,
                                    "total_bytes": total
                                }),
                            );
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
                        Ok(cleaner::ScanEvent::Warning(w)) => {
                            let payload = match &w {
                                ScanWarning::MaxItemsReached { target_id, items } => {
                                    serde_json::json!({
                                        "type": "max_items_reached", "target_id": target_id, "items": items
                                    })
                                }
                                ScanWarning::PermissionDenied { target_id, path } => {
                                    serde_json::json!({
                                        "type": "permission_denied", "target_id": target_id, "path": path
                                    })
                                }
                                ScanWarning::GlobNoMatch { target_id, pattern } => {
                                    serde_json::json!({
                                        "type": "glob_no_match", "target_id": target_id, "pattern": pattern
                                    })
                                }
                                ScanWarning::ServiceStopFailed {
                                    target_id,
                                    service,
                                    reason,
                                } => serde_json::json!({
                                    "type": "service_stop_failed", "target_id": target_id, "service": service, "reason": reason
                                }),
                                ScanWarning::EnvInjectionDetected { target_id, path } => {
                                    serde_json::json!({
                                        "type": "env_injection_detected", "target_id": target_id, "path": path
                                    })
                                }
                            };
                            tracing::warn!("Scan warning: {w:?}");
                            let _ = app_handle.emit("scan-warning", payload);
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
            let _ = app_handle.emit(
                "delete-progress",
                serde_json::json!({
                    "done": p.done, "total": p.total, "current": p.current
                }),
            );
        }
    });

    // 删除前收集分类统计和总大小
    let num_files = pathbufs.len() as u64;
    let mut by_cat = std::collections::HashMap::new();
    let mut total_bytes = 0u64;
    for p in &pathbufs {
        if let Ok(meta) = p.metadata() {
            total_bytes += meta.len();
        }
        let entry = by_cat
            .entry("other".to_string())
            .or_insert_with(|| cleaner::CategorySummary { files: 0, bytes: 0 });
        entry.files += 1;
        if let Ok(meta) = p.metadata() {
            entry.bytes += meta.len();
        }
    }

    let result = tokio::task::spawn_blocking(move || {
        cleaner::delete_files_with_progress(&pathbufs, Some(progress_tx))
    })
    .await
    .map_err(|e| e.to_string())?;

    let _ = progress_handle.await;

    // 写入审计日志（错误信息已脱敏 — 只保留错误原因，移除具体路径）
    fn sanitize_error(e: &str) -> String {
        // 常见错误格式: "Protected path: C:\Users\..."、"Cannot resolve path C:\foo: os error 2"
        let lower = e.to_lowercase();
        if lower.contains("protected") {
            "protected path".to_string()
        } else if lower.contains("not in scan scope") {
            "path not in scan scope".to_string()
        } else if lower.contains("cannot resolve") {
            "cannot resolve path".to_string()
        } else if let Some(fname) = std::path::Path::new(e).file_name() {
            // 纯路径格式 → 只保留文件名
            fname.to_string_lossy().to_string()
        } else {
            // fallback: 截断到 30 字符
            e.chars().take(30).collect()
        }
    }
    let sanitized_errors: Vec<String> = result.errors.iter().map(|e| sanitize_error(e)).collect();
    let log_entry = cleaner::CleanLogEntry {
        timestamp: cleaner::timestamp_now(),
        total_files: num_files,
        total_bytes,
        success: result.success,
        failed: result.failed,
        errors: sanitized_errors,
        by_category: by_cat,
    };
    let _ = cleaner::append_clean_log(&log_entry);

    Ok(result)
}

#[tauri::command]
pub async fn empty_recycle_bin() -> Result<(), String> {
    tokio::task::spawn_blocking(cleaner::empty_recycle_bin)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_clean_logs(limit: Option<usize>) -> Result<cleaner::CleanLogSummary, String> {
    cleaner::get_clean_logs(limit.unwrap_or(50))
}
