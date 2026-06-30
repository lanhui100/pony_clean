use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use serde::Serialize;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[repr(C)]
struct LastInputInfo {
    cb_size: u32,
    dw_time: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct MONITORINFO {
    cb_size: u32,
    rc_monitor: RECT,
    rc_work: RECT,
    dw_flags: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetLastInputInfo(pli: *mut LastInputInfo) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    fn GetTickCount() -> u32;
    fn MonitorFromPoint(pt: POINT, dwFlags: u32) -> isize;
    fn GetMonitorInfoW(hMonitor: isize, lpmi: *mut MONITORINFO) -> i32;
    fn GetSystemMetrics(nIndex: i32) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateRoundRectRgn(x1: i32, y1: i32, x2: i32, y2: i32, w: i32, h: i32) -> isize;
    fn SetWindowRgn(hWnd: isize, hRgn: isize, bRedraw: i32) -> i32;
}

const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;
const EDGE_THRESHOLD: i32 = 20;

#[derive(Clone, Serialize)]
pub struct EdgeCursorPayload {
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub mon_left: i32,
    pub mon_top: i32,
    pub mon_right: i32,
    pub mon_bottom: i32,
}

pub struct EdgeCursorState {
    pub running: Arc<AtomicBool>,
    pub hwnd: Mutex<Option<isize>>,
}

impl EdgeCursorState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            hwnd: Mutex::new(None),
        }
    }
}

#[cfg(target_os = "windows")]
fn get_hwnd(app: &AppHandle) -> Option<isize> {
    let window = app.get_webview_window("main")?;
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

#[tauri::command]
pub fn set_capsule_hit_rect(
    app: AppHandle,
    state: tauri::State<'_, EdgeCursorState>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let hwnd = if let Some(hwnd) = *state.hwnd.lock().unwrap() {
            hwnd
        } else {
            let hwnd = get_hwnd(&app).ok_or("cannot get window handle")?;
            *state.hwnd.lock().unwrap() = Some(hwnd);
            hwnd
        };

        unsafe {
            if w == 0 && h == 0 {
                SetWindowRgn(hwnd, 0, 1);
            } else {
                let region = CreateRoundRectRgn(x, y, x + w, y + h, 20, 20);
                if region == 0 {
                    return Err("CreateRoundRectRgn failed".into());
                }
                SetWindowRgn(hwnd, region, 1);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, &state, x, y, w, h);
    }

    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub async fn get_system_idle_ms() -> Result<u64, String> {
    #[cfg(target_os = "windows")]
    {
        let mut li = LastInputInfo {
            cb_size: std::mem::size_of::<LastInputInfo>() as u32,
            dw_time: 0,
        };
        let ret = unsafe { GetLastInputInfo(&mut li) };
        if ret == 0 {
            return Err("GetLastInputInfo failed".into());
        }
        let now = unsafe { GetTickCount() };
        let diff = now.wrapping_sub(li.dw_time);
        Ok(diff as u64)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("get_system_idle_ms is only supported on Windows".into())
    }
}

#[tauri::command]
pub fn start_edge_cursor_detect(
    app: AppHandle,
    state: tauri::State<'_, EdgeCursorState>,
) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let running = state.running.clone();
    let app = app.clone();

    std::thread::spawn(move || {
        let mut was_at_edge = false;
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            let (at_edge, payload) = is_cursor_at_edge();

            if at_edge && !was_at_edge {
                let _ = app.emit("edge-cursor-enter", payload);
            } else if !at_edge && was_at_edge {
                let _ = app.emit("edge-cursor-leave", ());
            }
            was_at_edge = at_edge;
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    Ok(())
}

#[tauri::command]
pub fn stop_edge_cursor_detect(state: tauri::State<'_, EdgeCursorState>) -> Result<(), String> {
    state.running.store(false, Ordering::SeqCst);
    Ok(())
}

fn is_cursor_at_edge() -> (bool, EdgeCursorPayload) {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return (false, EdgeCursorPayload {
                cursor_x: 0, cursor_y: 0,
                mon_left: 0, mon_top: 0, mon_right: 0, mon_bottom: 0,
            });
        }

        // Use virtual screen bounds for fallback (covers all monitors)
        let sw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let sh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if sw <= 0 || sh <= 0 {
            return (false, EdgeCursorPayload {
                cursor_x: pt.x, cursor_y: pt.y,
                mon_left: 0, mon_top: 0, mon_right: 0, mon_bottom: 0,
            });
        }

        let h_mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        if h_mon == 0 {
            let in_left = pt.x < EDGE_THRESHOLD;
            let in_right = pt.x > sw - EDGE_THRESHOLD;
            let in_top = pt.y < EDGE_THRESHOLD;
            let in_bottom = pt.y > sh - EDGE_THRESHOLD;
            let at_edge = in_left || in_right || in_top || in_bottom;
            return (at_edge, EdgeCursorPayload {
                cursor_x: pt.x, cursor_y: pt.y,
                mon_left: 0, mon_top: 0, mon_right: 0, mon_bottom: 0,
            });
        }

        let mut mi = MONITORINFO {
            cb_size: std::mem::size_of::<MONITORINFO>() as u32,
            rc_monitor: RECT { left: 0, top: 0, right: 0, bottom: 0 },
            rc_work: RECT { left: 0, top: 0, right: 0, bottom: 0 },
            dw_flags: 0,
        };
        if GetMonitorInfoW(h_mon, &mut mi) == 0 {
            return (false, EdgeCursorPayload {
                cursor_x: pt.x, cursor_y: pt.y,
                mon_left: 0, mon_top: 0, mon_right: 0, mon_bottom: 0,
            });
        }

        let in_top = pt.y - mi.rc_monitor.top < EDGE_THRESHOLD;
        let at_edge = in_top;

        (
            at_edge,
            EdgeCursorPayload {
                cursor_x: pt.x,
                cursor_y: pt.y,
                mon_left: mi.rc_monitor.left,
                mon_top: mi.rc_monitor.top,
                mon_right: mi.rc_monitor.right,
                mon_bottom: mi.rc_monitor.bottom,
            },
        )
    }
}
