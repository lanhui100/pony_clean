#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::cleaner::CleanerState;
use commands::monitor::MonitorState;
use commands::window::EdgeCursorState;
use commands::window::install_hit_test_subclass;
use pony_core::monitor;
use std::sync::{Arc, RwLock};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .manage(EdgeCursorState::new())
        .setup(|app| {
            eprintln!("[PonyClean] Rust backend starting up...");

            let snapshot = Arc::new(RwLock::new(None));
            let (cmd_tx, handle) = monitor::start_shared(snapshot.clone());
            app.manage(MonitorState {
                snapshot,
                cmd_tx: Some(cmd_tx),
                thread: Some(handle),
            });
            app.manage(CleanerState {
                is_scanning: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                cancel_token: Arc::new(std::sync::Mutex::new(None)),
            });

            eprintln!("[PonyClean] Setup complete, opening window...");

            // Install WM_NCHITTEST subclass for click-through on transparent areas
            if let Err(e) = install_hit_test_subclass(app.handle()) {
                eprintln!("[PonyClean] Failed to install hit-test subclass: {}", e);
            }

            // DevTools 已移除 — 自动打开 DevTools 窗口会导致右上角短暂闪烁"分辨率"信息

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::monitor::get_processes,
            commands::monitor::kill_process,
            commands::cleaner::start_scan,
            commands::cleaner::cancel_scan,
            commands::cleaner::execute_clean,
            commands::cleaner::empty_recycle_bin,
            commands::cleaner::get_clean_logs,
            commands::window::quit_app,
            commands::window::get_system_idle_ms,
            commands::window::start_edge_cursor_detect,
            commands::window::stop_edge_cursor_detect,
            commands::window::set_hit_test_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
