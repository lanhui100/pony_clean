use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

#[repr(C)]
struct LastInputInfo {
    cb_size: u32,
    dw_time: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetLastInputInfo(pli: *mut LastInputInfo) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    fn GetTickCount() -> u32;
    fn MonitorFromPoint(pt: POINT, dwFlags: u32) -> isize;
    fn GetMonitorInfoW(hMonitor: isize, lpmi: *mut MONITORINFO) -> i32;
}

#[derive(Copy, Clone)]
#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct MONITORINFO {
    cb_size: u32,
    rc_monitor: RECT,
    rc_work: RECT,
    dw_flags: u32,
}

const MONITOR_DEFAULTTONEAREST: u32 = 2;
const EDGE_THRESHOLD: i32 = 12;

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

/// Polls `GetCursorPos` at 100ms intervals. Emits `edge-cursor-enter`
/// when the cursor is within EDGE_THRESHOLD px of any screen edge,
/// and `edge-cursor-leave` when it moves away.
/// Uses a `CancellationToken` so the caller can stop polling by invoking
/// `stop_edge_cursor_detect`.
#[tauri::command]
pub async fn start_edge_cursor_detect(
    app: AppHandle,
    state: tauri::State<'_, EdgeCursorState>,
) -> Result<(), String> {
    if state.running.load(Ordering::SeqCst) {
        return Ok(()); // already polling
    }

    let cancel = CancellationToken::new();
    *state.cancel.lock().await = Some(cancel.child_token());
    state.running.store(true, Ordering::SeqCst);

    let running = state.running.clone();
    let app_clone = app.clone();
    let cancel_token = cancel.clone();
    let mut was_at_edge = false;

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => break,
                _ = interval.tick() => {}
            }

            let at_edge = is_cursor_at_edge().unwrap_or(false);

            if at_edge && !was_at_edge {
                let _ = app_clone.emit("edge-cursor-enter", ());
            } else if !at_edge && was_at_edge {
                let _ = app_clone.emit("edge-cursor-leave", ());
            }
            was_at_edge = at_edge;
        }
        running.store(false, Ordering::SeqCst);
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_edge_cursor_detect(
    state: tauri::State<'_, EdgeCursorState>,
) -> Result<(), String> {
    if let Some(token) = state.cancel.lock().await.take() {
        token.cancel();
    }
    state.running.store(false, Ordering::SeqCst);
    Ok(())
}

fn is_cursor_at_edge() -> Result<bool, String> {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return Err("GetCursorPos failed".into());
        }

        let h_mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        if h_mon == 0 {
            // fallback: assume 1920×1080
            let in_x = pt.x < EDGE_THRESHOLD || pt.x > 1920 - EDGE_THRESHOLD;
            let in_y = pt.y < EDGE_THRESHOLD || pt.y > 1080 - EDGE_THRESHOLD;
            return Ok(in_x || in_y);
        }

        let mut mi = MONITORINFO {
            cb_size: std::mem::size_of::<MONITORINFO>() as u32,
            rc_monitor: RECT { left: 0, top: 0, right: 0, bottom: 0 },
            rc_work: RECT { left: 0, top: 0, right: 0, bottom: 0 },
            dw_flags: 0,
        };
        if GetMonitorInfoW(h_mon, &mut mi) == 0 {
            return Err("GetMonitorInfoW failed".into());
        }

        let in_left = pt.x - mi.rc_monitor.left < EDGE_THRESHOLD;
        let in_right = mi.rc_monitor.right - pt.x < EDGE_THRESHOLD;
        let in_top = pt.y - mi.rc_monitor.top < EDGE_THRESHOLD;
        let in_bottom = mi.rc_monitor.bottom - pt.y < EDGE_THRESHOLD;
        Ok(in_left || in_right || in_top || in_bottom)
    }
}

pub struct EdgeCursorState {
    pub running: Arc<AtomicBool>,
    pub cancel: Arc<Mutex<Option<CancellationToken>>>,
}

impl EdgeCursorState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(Mutex::new(None)),
        }
    }
}