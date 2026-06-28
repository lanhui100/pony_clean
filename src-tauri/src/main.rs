#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::monitor::MonitorState;
use commands::cleaner::CleanerState;
use pony_core::monitor;
use std::sync::{Arc, RwLock};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
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

            #[cfg(debug_assertions)]
            {
                let win = app.get_webview_window("main").unwrap();
                win.open_devtools();
                eprintln!("[PonyClean] Devtools opened");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::monitor::get_processes,
            commands::monitor::kill_process,
            commands::cleaner::start_scan,
            commands::cleaner::cancel_scan,
            commands::cleaner::execute_clean,
            commands::cleaner::empty_recycle_bin,
            commands::window::quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
