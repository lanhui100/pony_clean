#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::cleaner::CleanerState;
use commands::disk::DiskState;
use commands::monitor::MonitorState;
use commands::window::EdgeCursorState;
use commands::window::install_hit_test_subclass;
use pony_core::monitor;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use tauri::Emitter;
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

/// 为灵动岛窗口应用「毛玻璃」（HWND 层级，SWCA 原生 API）。
///
/// 层级说明：Windows 毛玻璃是窗口级效果，CSS backdrop-filter 无法模糊
/// 窗口背后的桌面，必须对 HWND 调用系统 API。
/// SPEC-029 修订：此前用 DwmEnableBlurBehindWindow（区域化 hRgnBlur）在透明
/// WebView2 窗口上实测**不出毛玻璃**（接口返回成功但无效果），面板变为透明。
/// 恢复 SWCA（ACCENT_ENABLE_ACRYLICBLURBEHIND）——这是本透明窗口上历史验证
/// 真正生效的方案（TASK-022）。形状统一由圆角 Region + CSS 圆角壳层负责；
/// SWCA 染色取接近面板深色的低饱和值。
///
/// 失败时回退 Blur，再失败回退 CSS 拟态玻璃。
fn apply_island_vibrancy(app: &tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        use commands::window::{apply_acrylic_swca, apply_blur_swca, get_hwnd_for_label};
        if let Some(hwnd) = get_hwnd_for_label(app, "island") {
            // RGBA 着色：接近主题深色 hsl(30 12% 9%)，alpha 145 兼顾模糊透出与可读性
            match apply_acrylic_swca(hwnd, (26, 24, 21, 145)) {
                Ok(()) => eprintln!("[PonyClean] Acrylic applied to island window"),
                Err(e) => {
                    eprintln!("[PonyClean] Acrylic failed, falling back to Blur: {}", e);
                    match apply_blur_swca(hwnd, (26, 24, 21, 135)) {
                        Ok(()) => eprintln!("[PonyClean] Blur applied to island window"),
                        Err(e2) => eprintln!(
                            "[PonyClean] Blur failed too (fallback to CSS glass): {}",
                            e2
                        ),
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let reset_pos = MenuItem::with_id(app, "reset_pos", "重置胶囊位置", true, None::<&str>)?;
    let show_hide = MenuItem::with_id(app, "toggle", "显示/隐藏", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&reset_pos, &show_hide, &quit])?;

    let tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_main_window(app),
            "reset_pos" => {
                // 通知前端把胶囊重置到主屏顶部居中（找回跑丢的胶囊）
                if let Some(capsule) = app.get_webview_window("capsule") {
                    let _ = capsule.emit("reset-capsule-position", ());
                    let _ = capsule.show();
                }
            }
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
    // 提权子进程模式：由 startup_item_elevated 以管理员身份启动，
    // 执行关闭/打开自启动后写入结果文件并退出（不创建任何窗口）
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() >= 5 && args[1] == "--elevated-startup" {
            let code = pony_core::startup::run_elevated_startup(&args[2], &args[3], &args[4]);
            std::process::exit(code);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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
            app.manage(DiskState {
                is_scanning: Arc::new(AtomicBool::new(false)),
                cancel_flag: Arc::new(Mutex::new(None)),
                scan_root: Arc::new(Mutex::new(None)),
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

            // 灵动岛毛玻璃：HWND 层级 Acrylic（DWM 合成），必须在 hit-test 之后应用
            apply_island_vibrancy(app.handle());

            // 延迟二次清理标题栏：Tauri/WebView2 可能在窗口显示后重置窗口样式
            // （decorations 处理、WebView 初始化等），500ms 后强制再清理一次。
            {
                let app = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(800));
                    #[cfg(target_os = "windows")]
                    {
                        use commands::window::{get_hwnd_for_label, strip_title_bar};
                        for label in ["capsule", "island"] {
                            if let Some(hwnd) = get_hwnd_for_label(&app, label) {
                                unsafe { strip_title_bar(hwnd) };
                            }
                        }
                        eprintln!("[PonyClean] Title bar stripped (delayed pass)");
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let _ = &app;
                    }
                });
            }

            // DevTools 已移除 — 自动打开 DevTools 窗口会导致右上角短暂闪烁"分辨率"信息

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::monitor::get_processes,
            commands::monitor::kill_process,
            commands::monitor::trim_memory,
            commands::monitor::get_process_icon,
            commands::cleaner::start_scan,
            commands::cleaner::cancel_scan,
            commands::cleaner::execute_clean,
            commands::cleaner::empty_recycle_bin,
            commands::cleaner::get_clean_logs,
            commands::cleaner::get_clean_config,
            commands::cleaner::save_clean_config,
            commands::disk::start_user_scan,
            commands::disk::cancel_disk_scan,
            commands::disk::delete_large_files,
            commands::window::quit_app,
            commands::window::set_island_expanded,
            commands::window::get_system_idle_ms,
            commands::window::start_edge_cursor_detect,
            commands::window::stop_edge_cursor_detect,
            commands::window::set_capsule_geometry,
            commands::window::get_monitor_work_area,
            commands::window::log_frontend,
            commands::config::get_config,
            commands::config::set_config,
            commands::startup::list_startup_items,
            commands::startup::disable_startup_item,
            commands::startup::enable_startup_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
