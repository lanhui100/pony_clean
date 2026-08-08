use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::Serialize;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

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
    // Hit-test functions
    fn ScreenToClient(hWnd: isize, lpPoint: *mut POINT) -> i32;
    fn GetClientRect(hWnd: isize, lpRect: *mut RECT) -> i32;
    fn SetPropW(hWnd: isize, lpString: *const u16, hData: isize) -> i32;
    fn GetPropW(hWnd: isize, lpString: *const u16) -> isize;
    #[allow(dead_code)]
    fn RemovePropW(hWnd: isize, lpString: *const u16) -> isize;
    fn GetWindowLongPtrW(hWnd: isize, nIndex: i32) -> isize;
    fn SetWindowLongPtrW(hWnd: isize, nIndex: i32, dwNewLong: isize) -> isize;
    fn SetWindowPos(
        hWnd: isize,
        hWndInsertAfter: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
    fn GetWindowRect(hWnd: isize, lpRect: *mut RECT) -> i32;
    fn RedrawWindow(hWnd: isize, lprcUpdate: *const RECT, hrgnUpdate: isize, flags: u32) -> i32;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(
        h_wnd: isize,
        pfn_subclass: SUBCLASSPROC,
        u_id_subclass: usize,
        dw_ref_data: usize,
    ) -> i32;
    fn DefSubclassProc(h_wnd: isize, u_msg: u32, w_param: usize, l_param: isize) -> isize;
    #[allow(dead_code)]
    fn RemoveWindowSubclass(h_wnd: isize, pfn_subclass: SUBCLASSPROC, u_id_subclass: usize) -> i32;
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: isize,
        dw_attribute: u32,
        pv_attribute: *const std::ffi::c_void,
        cb_attribute: u32,
    ) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateRoundRectRgn(x1: i32, y1: i32, x2: i32, y2: i32, w: i32, h: i32) -> isize;
    fn SetWindowRgn(hWnd: isize, hRgn: isize, bRedraw: i32) -> i32;
}

type SUBCLASSPROC = unsafe extern "system" fn(
    h_wnd: isize,
    u_msg: u32,
    w_param: usize,
    l_param: isize,
    u_id_subclass: usize,
    dw_ref_data: usize,
) -> isize;

const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;
const EDGE_THRESHOLD: i32 = 20;

// Logical window dimensions (CSS pixels)
const LOGICAL_W: i32 = 315;
const CAPSULE_LOGICAL_W: i32 = 166; // window width (extra 6px for pill anti-aliasing)
const CAPSULE_W: i32 = 160;         // visual pill width
const CAPSULE_H: i32 = 40;          // visual pill height
const ISLAND_RADIUS: i32 = 16;
const CAPSULE_RADIUS: i32 = 20;

// Window property name for hit-test mode
const HT_MODE_PROP: &str = "PonyCleanHitMode\0";

// Hit-test mode values stored in window property
const HT_MODE_CAPSULE: isize = 0;
const HT_MODE_FULL: isize = 1;

#[cfg(target_os = "windows")]
fn lparam_screen_point(lparam: isize) -> POINT {
    let raw = lparam as i32;
    POINT {
        x: (raw as u16) as i16 as i32,
        y: ((raw >> 16) as u16) as i16 as i32,
    }
}

#[cfg(target_os = "windows")]
unsafe fn cursor_in_capsule_region(hwnd: isize, pt: POINT) -> bool {
    let mut wr = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetWindowRect(hwnd, &mut wr) } == 0 {
        return false;
    }

    let phys_w = wr.right - wr.left;
    if phys_w <= 0 {
        return false;
    }

    let dpr = phys_w as f32 / CAPSULE_LOGICAL_W as f32;
    let c_w = (CAPSULE_LOGICAL_W as f32 * dpr).round() as i32;
    let c_h = (CAPSULE_H as f32 * dpr).round() as i32;
    let c_x = wr.left + (phys_w - c_w) / 2;

    pt.x >= c_x && pt.x < c_x + c_w && pt.y >= wr.top && pt.y < wr.top + c_h
}

#[cfg(target_os = "windows")]
unsafe fn redraw_window_frame(hwnd: isize) {
    const RDW_FRAME: u32 = 0x0400;
    const RDW_INVALIDATE: u32 = 0x0001;
    const RDW_UPDATENOW: u32 = 0x0100;
    unsafe {
        RedrawWindow(
            hwnd,
            std::ptr::null(),
            0,
            RDW_FRAME | RDW_INVALIDATE | RDW_UPDATENOW,
        );
    }
}

#[cfg(target_os = "windows")]
unsafe fn apply_window_region(hwnd: isize, mode: isize) {
    let mut cr = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetClientRect(hwnd, &mut cr) } == 0 {
        return;
    }

    let phys_w = cr.right - cr.left;
    let phys_h = cr.bottom - cr.top;
    if phys_w <= 0 || phys_h <= 0 {
        return;
    }

    let dpr = phys_w as f32 / LOGICAL_W as f32;
    let region = if mode == HT_MODE_FULL {
        // CreateRoundRectRgn expects the corner ellipse size, not the CSS radius.
        let ellipse = (ISLAND_RADIUS as f32 * dpr * 2.0).round() as i32;
        unsafe { CreateRoundRectRgn(0, 0, phys_w, phys_h, ellipse, ellipse) }
    } else {
        let c_w = (CAPSULE_W as f32 * dpr).round() as i32;
        let c_h = (CAPSULE_H as f32 * dpr).round() as i32;
        let c_x = (phys_w - c_w) / 2;
        unsafe { CreateRoundRectRgn(c_x, 0, c_x + c_w, c_h, c_h, c_h) }
    };

    if region != 0 {
        unsafe {
            SetWindowRgn(hwnd, region, 1);
            redraw_window_frame(hwnd);
        }
    }
}

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
    get_hwnd_for_label(app, "capsule")
}

#[cfg(target_os = "windows")]
fn get_hwnd_for_label(app: &AppHandle, label: &str) -> Option<isize> {
    let window = app.get_webview_window(label)?;
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
unsafe fn apply_full_round_region(hwnd: isize, logical_w: i32, radius: i32) {
    let mut cr = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetClientRect(hwnd, &mut cr) } == 0 {
        return;
    }

    let phys_w = cr.right - cr.left;
    let phys_h = cr.bottom - cr.top;
    if phys_w <= 0 || phys_h <= 0 {
        return;
    }

    let dpr = phys_w as f32 / logical_w as f32;
    let ellipse = (radius as f32 * dpr * 2.0).round() as i32;
    let region = unsafe { CreateRoundRectRgn(0, 0, phys_w, phys_h, ellipse, ellipse) };
    if region != 0 {
        unsafe {
            SetWindowRgn(hwnd, region, 1);
            redraw_window_frame(hwnd);
        }
    }
}

// ─── WM_NCHITTEST subclass ───

/// Window subclass procedure that handles WM_NCHITTEST.
/// The actual click-through behavior is provided by SetWindowRgn. Returning
/// HTNOWHERE here is only a defensive fallback for stale or failed regions.
#[cfg(target_os = "windows")]
unsafe extern "system" fn hit_test_subclass(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _u_id: usize,
    _dw_ref: usize,
) -> isize {
    const WM_NCHITTEST: u32 = 0x0084;
    const WM_ERASEBKGND: u32 = 0x0014;
    const HTCLIENT: isize = 1;
    // HTNOWHERE silently drops the click without passing it through. Do not rely
    // on it for click-through; SetWindowRgn must match the visible surface.
    // Using HTTRANSPARENT would trigger Windows 11 to show a title-bar
    // preview on always-on-top windows when they lose focus.
    const HTNOWHERE: isize = -2;

    // Prevent Windows from painting the default window background.
    // Without this, the OS fills the window with the class background
    // brush (typically gray/white), which shows through the transparent
    // WebView2 corners.
    if msg == WM_ERASEBKGND {
        return 1;
    }

    if msg == WM_NCHITTEST {
        // Get current hit-test mode
        let prop_name: Vec<u16> = HT_MODE_PROP.encode_utf16().collect();
        let mode = unsafe { GetPropW(hwnd, prop_name.as_ptr()) };

        if mode == HT_MODE_FULL {
            return HTCLIENT;
        }

        // Capsule mode: only the centered 160×40 (logical) area is interactive
        let mut pt = lparam_screen_point(lparam);
        if unsafe { ScreenToClient(hwnd, &mut pt) } == 0 {
            return unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        }

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetClientRect(hwnd, &mut rect) } == 0 {
            return unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        }

        let phys_w = (rect.right - rect.left) as f32;
        let dpr = phys_w / LOGICAL_W as f32;

        // Check vertical: must be within capsule height
        let c_h = (CAPSULE_H as f32 * dpr) as i32;
        if pt.y < 0 || pt.y >= c_h {
            return HTNOWHERE;
        }

        // Check horizontal: must be within centered capsule width
        let c_w = (CAPSULE_W as f32 * dpr) as i32;
        let c_x = (phys_w as i32 - c_w) / 2;
        if pt.x < c_x || pt.x >= c_x + c_w {
            return HTNOWHERE;
        }

        return HTCLIENT;
    }

    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Prepare the floating windows for transparent, borderless rendering.
#[cfg(target_os = "windows")]
pub fn install_hit_test_subclass(app: &AppHandle) -> Result<(), String> {
    let windows = [
        ("capsule", CAPSULE_LOGICAL_W, CAPSULE_RADIUS),
        ("island", LOGICAL_W, ISLAND_RADIUS),
    ];

    for (label, logical_w, radius) in windows {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_decorations(false);
            let _ = window.set_effects(None);
        }

        let Some(hwnd) = get_hwnd_for_label(app, label) else {
            continue;
        };

        unsafe {
            let style = GetWindowLongPtrW(hwnd, -16);

            if SetWindowSubclass(hwnd, hit_test_subclass as SUBCLASSPROC, 0, 0) == 0 {
                return Err(format!("SetWindowSubclass failed for {label}"));
            }
            let prop_name: Vec<u16> = HT_MODE_PROP.encode_utf16().collect();
            SetPropW(hwnd, prop_name.as_ptr(), HT_MODE_FULL);

            remove_dwm_border(hwnd);

            const GWL_STYLE: i32 = -16;
            const WS_CAPTION: isize = 0x00C00000;
            const WS_THICKFRAME: isize = 0x00040000;
            const WS_SYSMENU: isize = 0x00080000;
            const WS_MINIMIZEBOX: isize = 0x00020000;
            const WS_MAXIMIZEBOX: isize = 0x00010000;
            let new_style = style
                & !(WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
            if new_style != style {
                SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
                const SWP_FRAMECHANGED: u32 = 0x0020;
                const SWP_NOMOVE: u32 = 0x0002;
                const SWP_NOSIZE: u32 = 0x0001;
                const SWP_NOZORDER: u32 = 0x0004;
                SetWindowPos(
                    hwnd,
                    0,
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                );
            }

            const GWL_EXSTYLE: i32 = -20;
            const WS_EX_TOOLWINDOW: isize = 0x00000080;
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if (ex_style & WS_EX_TOOLWINDOW) == 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_TOOLWINDOW);
            }

            apply_full_round_region(hwnd, logical_w, radius);
            eprintln!("[PonyClean] Prepared floating window: {label}");
        }
    }

    Ok(())
}

/// Use DwmSetWindowAttribute to make the window border transparent.
#[cfg(target_os = "windows")]
unsafe fn remove_dwm_border(hwnd: isize) {
    const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;

    // DWMWA_BORDER_COLOR = 34 — suppress the DWM frame border.
    const DWMWA_BORDER_COLOR: u32 = 34;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &DWMWA_COLOR_NONE as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }

    // DWMWA_CAPTION_COLOR = 35 — keep any cached caption repaint dark.
    const DWMWA_CAPTION_COLOR: u32 = 35;
    let caption_color: u32 = 0x00000000;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &caption_color as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }

    // DWMWA_USE_IMMERSIVE_DARK_MODE = 20
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    let dark_mode: i32 = 1;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn install_hit_test_subclass(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn set_hit_test_mode(
    app: AppHandle,
    state: tauri::State<'_, EdgeCursorState>,
    mode: String,
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
            let prop_name: Vec<u16> = HT_MODE_PROP.encode_utf16().collect();
            if mode == "full" {
                SetPropW(hwnd, prop_name.as_ptr(), HT_MODE_FULL);
                apply_window_region(hwnd, HT_MODE_FULL);
            } else {
                SetPropW(hwnd, prop_name.as_ptr(), HT_MODE_CAPSULE);
                apply_window_region(hwnd, HT_MODE_CAPSULE);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, &state, mode);
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
    #[cfg(target_os = "windows")]
    let hwnd = {
        let hwnd = get_hwnd(&app);
        if let Some(hwnd) = hwnd {
            *state.hwnd.lock().unwrap() = Some(hwnd);
        }
        hwnd
    };

    std::thread::spawn(move || {
        let mut was_at_edge = false;
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            #[cfg(target_os = "windows")]
            let (at_edge, payload) = is_cursor_at_edge(hwnd);
            #[cfg(not(target_os = "windows"))]
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

#[cfg(target_os = "windows")]
fn is_cursor_at_edge(hwnd: Option<isize>) -> (bool, EdgeCursorPayload) {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return (
                false,
                EdgeCursorPayload {
                    cursor_x: 0,
                    cursor_y: 0,
                    mon_left: 0,
                    mon_top: 0,
                    mon_right: 0,
                    mon_bottom: 0,
                },
            );
        }

        let sw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let sh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if sw <= 0 || sh <= 0 {
            return (
                false,
                EdgeCursorPayload {
                    cursor_x: pt.x,
                    cursor_y: pt.y,
                    mon_left: 0,
                    mon_top: 0,
                    mon_right: 0,
                    mon_bottom: 0,
                },
            );
        }

        let h_mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        if h_mon == 0 {
            let in_left = pt.x < EDGE_THRESHOLD;
            let in_right = pt.x > sw - EDGE_THRESHOLD;
            let in_top = pt.y < EDGE_THRESHOLD;
            let in_bottom = pt.y > sh - EDGE_THRESHOLD;
            let at_edge = in_left || in_right || in_top || in_bottom;
            return (
                at_edge,
                EdgeCursorPayload {
                    cursor_x: pt.x,
                    cursor_y: pt.y,
                    mon_left: 0,
                    mon_top: 0,
                    mon_right: 0,
                    mon_bottom: 0,
                },
            );
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
            return (
                false,
                EdgeCursorPayload {
                    cursor_x: pt.x,
                    cursor_y: pt.y,
                    mon_left: 0,
                    mon_top: 0,
                    mon_right: 0,
                    mon_bottom: 0,
                },
            );
        }

        let in_top = pt.y - mi.rc_monitor.top < EDGE_THRESHOLD;
        let in_capsule = hwnd
            .map(|hwnd| cursor_in_capsule_region(hwnd, pt))
            .unwrap_or(true);
        let at_edge = in_top && in_capsule;

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

#[cfg(not(target_os = "windows"))]
fn is_cursor_at_edge() -> (bool, EdgeCursorPayload) {
    (
        false,
        EdgeCursorPayload {
            cursor_x: 0,
            cursor_y: 0,
            mon_left: 0,
            mon_top: 0,
            mon_right: 0,
            mon_bottom: 0,
        },
    )
}
