#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::cleaner::CleanerState;
use commands::monitor::MonitorState;
use commands::window::EdgeCursorState;
use commands::window::install_hit_test_subclass;
use pony_core::monitor;
use std::sync::{Arc, RwLock};
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

/// 切换主窗口（胶囊）显示/隐藏，同时隐藏灵动岛
fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(island) = app.get_webview_window("island") {
        let _ = island.hide();
    }
    if let Some(capsule) = app.get_webview_window("capsule") {
        if capsule.is_visible().unwrap_or(false) {
            let _ = capsule.hide();
        } else {
            let _ = capsule.show();
            let _ = capsule.set_focus();
        }
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_hide = MenuItem::with_id(app, "toggle", "显示/隐藏", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_hide, &quit])?;

    let tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    // 持有托盘引用，防止被 drop 后图标消失
    app.manage(tray);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
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

            if let Err(e) = setup_tray(app) {
                eprintln!("[PonyClean] Failed to setup tray: {}", e);
            }

            eprintln!("[PonyClean] Setup complete, opening window...");

            if let Some(island) = app.get_webview_window("island") {
                let _ = island.hide();
            }

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
            commands::monitor::trim_memory,
            commands::cleaner::start_scan,
            commands::cleaner::cancel_scan,
            commands::cleaner::execute_clean,
            commands::cleaner::empty_recycle_bin,
            commands::cleaner::get_clean_logs,
            commands::cleaner::get_clean_config,
            commands::cleaner::save_clean_config,
            commands::window::quit_app,
            commands::window::set_island_expanded,
            commands::window::get_system_idle_ms,
            commands::window::start_edge_cursor_detect,
            commands::window::stop_edge_cursor_detect,
            commands::window::set_hit_test_mode,
            commands::config::get_config,
            commands::config::set_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
