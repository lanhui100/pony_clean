use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

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

const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const EDGE_THRESHOLD: i32 = 20;

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

/// Polls `GetCursorPos` every 100ms in a background thread. Emits
/// `edge-cursor-enter` / `edge-cursor-leave` Tauri events when the
/// cursor enters or leaves the 20px screen-edge zone.
#[tauri::command]
pub fn start_edge_cursor_detect(
    app: AppHandle,
    state: tauri::State<'_, EdgeCursorState>,
) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Ok(()); // already running
    }

    let running = state.running.clone();
    let app = app.clone();

    std::thread::spawn(move || {
        let mut was_at_edge = false;
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }
            let at_edge = is_cursor_at_edge();
            if at_edge && !was_at_edge {
                let _ = app.emit("edge-cursor-enter", ());
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

fn is_cursor_at_edge() -> bool {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return false;
        }

        // Use actual screen resolution for edge detection
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        if sw <= 0 || sh <= 0 {
            return false;
        }

        let h_mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        if h_mon == 0 {
            // fallback: use system metrics
            let in_left = pt.x < EDGE_THRESHOLD;
            let in_right = pt.x > sw - EDGE_THRESHOLD;
            let in_top = pt.y < EDGE_THRESHOLD;
            let in_bottom = pt.y > sh - EDGE_THRESHOLD;
            return in_left || in_right || in_top || in_bottom;
        }

        let mut mi = MONITORINFO {
            cb_size: std::mem::size_of::<MONITORINFO>() as u32,
            rc_monitor: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rc_work: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dw_flags: 0,
        };
        if GetMonitorInfoW(h_mon, &mut mi) == 0 {
            return false;
        }

        let in_left = pt.x - mi.rc_monitor.left < EDGE_THRESHOLD;
        let in_right = mi.rc_monitor.right - pt.x < EDGE_THRESHOLD;
        let in_top = pt.y - mi.rc_monitor.top < EDGE_THRESHOLD;
        let in_bottom = mi.rc_monitor.bottom - pt.y < EDGE_THRESHOLD;
        in_left || in_right || in_top || in_bottom
    }
}

pub struct EdgeCursorState {
    pub running: Arc<AtomicBool>,
}

impl EdgeCursorState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}
