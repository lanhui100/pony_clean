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
#[allow(clippy::upper_case_acronyms)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
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
    fn GetClassLongPtrW(hWnd: isize, nIndex: i32) -> isize;
    fn SetClassLongPtrW(hWnd: isize, nIndex: i32, dwNewLong: isize) -> isize;
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
    // Acrylic (SWCA)
    fn GetModuleHandleW(lpModuleName: *const u16) -> isize;
    fn GetProcAddress(hModule: isize, lpProcName: *const u8) -> *mut std::ffi::c_void;
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

#[allow(clippy::upper_case_acronyms)]
type SUBCLASSPROC = unsafe extern "system" fn(
    h_wnd: isize,
    u_msg: u32,
    w_param: usize,
    l_param: isize,
    u_id_subclass: usize,
    dw_ref_data: usize,
) -> isize;

// ─── Acrylic 毛玻璃（SWCA 手写实现） ───
// 不使用 window-vibrancy 的 apply_acrylic：其在 Win11 走
// DWMSBT_TRANSIENTWINDOW 路径，会强制 DWM 绘制系统标题栏（最小化/关闭/最大化按钮）。
// SWCA（SetWindowCompositionAttribute + ACCENT_ENABLE_ACRYLICBLURBEHIND）
// 是 Win10/11 通用的 Acrylic 实现，不触发标题栏。

/// ACCENT_POLICY（SetWindowCompositionAttribute 数据结构）
#[repr(C)]
pub struct AccentPolicy {
    pub accent_state: u32,
    pub accent_flags: u32,
    pub gradient_color: u32,
    pub animation_id: u32,
}

/// WCA_DATA（SetWindowCompositionAttribute 数据结构）
#[repr(C)]
pub struct WCA_DATA {
    pub attribute: u32,
    pub data: *mut AccentPolicy,
    pub size_of_data: u32,
}

const WCA_ACCENT_POLICY: u32 = 19;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
const ACCENT_ENABLE_BLURBEHIND: u32 = 3;

/// 对窗口应用 Acrylic 毛玻璃（SWCA 路径，不触发 DWM 标题栏）
///
/// `rgba` 为着色色值 (R, G, B, A)，A 越大背景越实、模糊越弱。
#[cfg(target_os = "windows")]
pub fn apply_acrylic_swca(hwnd: isize, rgba: (u8, u8, u8, u8)) -> Result<(), String> {
    apply_accent(hwnd, ACCENT_ENABLE_ACRYLICBLURBEHIND, rgba)
}

/// 对窗口应用 Blur 毛玻璃（SWCA 路径，Acrylic 不可用时的回退）
#[cfg(target_os = "windows")]
pub fn apply_blur_swca(hwnd: isize, rgba: (u8, u8, u8, u8)) -> Result<(), String> {
    apply_accent(hwnd, ACCENT_ENABLE_BLURBEHIND, rgba)
}

#[cfg(target_os = "windows")]
fn apply_accent(hwnd: isize, state: u32, rgba: (u8, u8, u8, u8)) -> Result<(), String> {
    // SetWindowCompositionAttribute 是 Win10 1809+ 的动态 API，
    // 不在 MSVC user32.lib 导入表中，必须 GetProcAddress 动态加载。
    type SwcaFn = unsafe extern "system" fn(isize, *mut WCA_DATA) -> i32;
    let swca: SwcaFn = unsafe {
        let user32 = GetModuleHandleW(
            [
                0x75, 0x73, 0x65, 0x72, 0x33, 0x32, 0x2e, 0x64, 0x6c, 0x6c, 0,
            ]
            .as_ptr(),
        );
        if user32 == 0 {
            return Err("GetModuleHandleW(user32) failed".into());
        }
        let name = b"SetWindowCompositionAttribute\0";
        let addr = GetProcAddress(user32, name.as_ptr());
        if addr.is_null() {
            return Err("SetWindowCompositionAttribute not found".into());
        }
        std::mem::transmute(addr)
    };

    let (r, g, b, a) = rgba;
    // gradient_color 格式：0xAARRGGBB
    let gradient = ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
    let mut policy = AccentPolicy {
        accent_state: state,
        accent_flags: 0,
        gradient_color: gradient,
        animation_id: 0,
    };
    let mut data = WCA_DATA {
        attribute: WCA_ACCENT_POLICY,
        data: &mut policy,
        size_of_data: std::mem::size_of::<AccentPolicy>() as u32,
    };
    let ret = unsafe { swca(hwnd, &mut data) };
    if ret == 0 {
        Err("SetWindowCompositionAttribute failed".into())
    } else {
        Ok(())
    }
}

const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;
const EDGE_THRESHOLD: i32 = 20;

// Logical window dimensions (CSS pixels)
//
// 面板即窗口（SPEC-029 二次修订）：SWCA Acrylic 是窗口级效果、铺满整个窗口
// 矩形且不被圆角 Region 裁剪。若窗口比 CSS 面板大（留阴影边距），边距区会露出
// 一圈直角毛玻璃（用户反馈的「外圈」）。故面板 CSS 占满窗口；阴影改用原生 DWM
// 阴影（CS_DROPSHADOW，跟随圆角 Region 投影，不占 CSS 边距）。
const LOGICAL_W: i32 = 315; // island 窗口宽 = 内容宽
const LOGICAL_H: i32 = 100; // island 概要态高度
const ISLAND_EXPANDED_H: i32 = 480; // island 展开态高度
const CAPSULE_W_LOGICAL: i32 = 166; // 胶囊窗口宽（含 3px 左右抗锯齿余量）
const CAPSULE_H_LOGICAL: i32 = 44; // 胶囊窗口高（含 2px 上下余量）
const PILL_W: i32 = 160; // 胶囊视觉宽度
const PILL_H: i32 = 40; // 胶囊视觉高度
const STRIP_THICK: i32 = 10; // 贴边进度条厚度

// Window property name for hit-test mode
const HT_MODE_PROP: &str = "PonyCleanHitMode\0";

// Hit-test mode values stored in window property
const HT_MODE_CAPSULE: isize = 0;
const HT_MODE_FULL: isize = 1;

// 胶囊窗口几何属性（形态 + 贴边方向），供 subclass 命中区域与原生命中区域计算
const GEO_FORM_PROP: &str = "PonyCleanGeoForm\0";
const GEO_EDGE_PROP: &str = "PonyCleanGeoEdge\0";
const FORM_PILL: isize = 0;
const FORM_BAR: isize = 1;
const EDGE_TOP: isize = 0;
const EDGE_BOTTOM: isize = 1;
const EDGE_LEFT: isize = 2;
const EDGE_RIGHT: isize = 3;

/// 胶囊窗口显示形态：胶囊（药丸）或贴边进度条
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CapsuleForm {
    #[default]
    Pill,
    Bar,
}

/// 屏幕边缘
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ScreenEdge {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// 胶囊窗口当前的形态 + 贴边方向
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CapsuleGeometry {
    pub form: CapsuleForm,
    pub edge: ScreenEdge,
}

#[cfg(target_os = "windows")]
fn lparam_screen_point(lparam: isize) -> POINT {
    let raw = lparam as i32;
    POINT {
        x: (raw as u16) as i16 as i32,
        y: ((raw >> 16) as u16) as i16 as i32,
    }
}

/// 读取胶囊窗口当前的形态/贴边方向（窗口属性）
#[cfg(target_os = "windows")]
unsafe fn read_capsule_geometry(hwnd: isize) -> CapsuleGeometry {
    let form_prop: Vec<u16> = GEO_FORM_PROP.encode_utf16().collect();
    let edge_prop: Vec<u16> = GEO_EDGE_PROP.encode_utf16().collect();
    let form = match unsafe { GetPropW(hwnd, form_prop.as_ptr()) } {
        FORM_BAR => CapsuleForm::Bar,
        _ => CapsuleForm::Pill,
    };
    let edge = match unsafe { GetPropW(hwnd, edge_prop.as_ptr()) } {
        EDGE_BOTTOM => ScreenEdge::Bottom,
        EDGE_LEFT => ScreenEdge::Left,
        EDGE_RIGHT => ScreenEdge::Right,
        _ => ScreenEdge::Top,
    };
    CapsuleGeometry { form, edge }
}

/// 窗口逻辑尺寸：竖边（左/右）旋转为 56×192，横边为 192×56
#[cfg(target_os = "windows")]
fn capsule_logical_size(edge: ScreenEdge) -> (i32, i32) {
    match edge {
        ScreenEdge::Left | ScreenEdge::Right => (CAPSULE_H_LOGICAL, CAPSULE_W_LOGICAL),
        _ => (CAPSULE_W_LOGICAL, CAPSULE_H_LOGICAL),
    }
}

/// 内容矩形（逻辑 px，相对窗口左上角）：胶囊居中，进度条贴边细条。
#[cfg(target_os = "windows")]
fn capsule_content_logical(geo: CapsuleGeometry) -> (i32, i32, i32, i32) {
    let (lw, lh) = capsule_logical_size(geo.edge);
    match geo.form {
        CapsuleForm::Pill => {
            // 竖边（左/右）时胶囊旋转：宽高互换（40 宽 × 160 高）
            let (pw, ph) = match geo.edge {
                ScreenEdge::Left | ScreenEdge::Right => (PILL_H, PILL_W),
                _ => (PILL_W, PILL_H),
            };
            let x = (lw - pw) / 2;
            let y = (lh - ph) / 2;
            (x, y, pw, ph)
        }
        CapsuleForm::Bar => match geo.edge {
            ScreenEdge::Top => (0, 0, lw, STRIP_THICK),
            ScreenEdge::Bottom => (0, lh - STRIP_THICK, lw, STRIP_THICK),
            ScreenEdge::Left => (0, 0, STRIP_THICK, lh),
            ScreenEdge::Right => (lw - STRIP_THICK, 0, STRIP_THICK, lh),
        },
    }
}

/// 计算胶囊窗口内容矩形（物理像素，相对窗口客户区），按实际客户区与逻辑尺寸换算 DPR
#[cfg(target_os = "windows")]
unsafe fn capsule_content_phys(hwnd: isize, geo: CapsuleGeometry) -> Option<(i32, i32, i32, i32)> {
    let mut cr = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetClientRect(hwnd, &mut cr) } == 0 {
        return None;
    }
    let phys_w = cr.right - cr.left;
    let phys_h = cr.bottom - cr.top;
    if phys_w <= 0 || phys_h <= 0 {
        return None;
    }
    let (lw, lh) = capsule_logical_size(geo.edge);
    let dpr_x = phys_w as f32 / lw as f32;
    let dpr_y = phys_h as f32 / lh as f32;
    let (lx, ly, cw, ch) = capsule_content_logical(geo);
    let x = (lx as f32 * dpr_x).round() as i32;
    let y = (ly as f32 * dpr_y).round() as i32;
    let w = (cw as f32 * dpr_x).round() as i32;
    let h = (ch as f32 * dpr_y).round() as i32;
    let x = x.max(0).min(phys_w);
    let y = y.max(0).min(phys_h);
    let w = w.min(phys_w - x);
    let h = h.min(phys_h - y);
    Some((x, y, w, h))
}

/// 为胶囊窗口应用与当前形态/贴边方向匹配的圆角区域（点击穿透 + 形状裁剪）。
///
/// Region = 内容矩形本身，圆角 = 短边（胶囊/进度条两端半圆）。
/// 不加阴影外扩：阴影由原生 DWM（CS_DROPSHADOW）按本 Region 形状投影。
#[cfg(target_os = "windows")]
unsafe fn apply_capsule_region(hwnd: isize) {
    let geo = unsafe { read_capsule_geometry(hwnd) };
    let Some((x, y, w, h)) = (unsafe { capsule_content_phys(hwnd, geo) }) else {
        return;
    };
    if w <= 0 || h <= 0 {
        return;
    }
    // 两端半圆（胶囊/进度条）：角椭圆直径取短边，横竖排均正确
    let radius = w.min(h);
    let region = unsafe { CreateRoundRectRgn(x, y, x + w, y + h, radius, radius) };
    if region != 0 {
        unsafe {
            SetWindowRgn(hwnd, region, 1);
            redraw_window_frame(hwnd);
        }
    }
}

#[cfg(target_os = "windows")]
unsafe fn cursor_in_capsule_region(hwnd: isize, pt: POINT) -> bool {
    let geo = unsafe { read_capsule_geometry(hwnd) };
    let Some((x, y, w, h)) = (unsafe { capsule_content_phys(hwnd, geo) }) else {
        return false;
    };
    let mut cpt = pt;
    if unsafe { ScreenToClient(hwnd, &mut cpt) } == 0 {
        return false;
    }
    cpt.x >= x && cpt.x < x + w && cpt.y >= y && cpt.y < y + h
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
    pub geo: Mutex<CapsuleGeometry>,
}

impl EdgeCursorState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            hwnd: Mutex::new(None),
            geo: Mutex::new(CapsuleGeometry::default()),
        }
    }
}

#[cfg(target_os = "windows")]
fn get_hwnd(app: &AppHandle) -> Option<isize> {
    get_hwnd_for_label(app, "capsule")
}

#[cfg(target_os = "windows")]
pub fn get_hwnd_for_label(app: &AppHandle, label: &str) -> Option<isize> {
    let window = app.get_webview_window(label)?;
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

/// 为 island 窗口应用**方角**区域：整个窗口矩形（面板=窗口）。
///
/// SPEC-029 终版（用户裁决）：SWCA Acrylic 是窗口级效果、铺满整个方形窗口且
/// 不被 Region 裁剪——若 Region/CSS 用圆角，四角必然露出底层 SWCA 的直角毛玻璃
/// （"两层不重叠"）。故面板采用**方角毛玻璃**：Region 方角 = SWCA 方角 = CSS
/// 方角，四角彻底一致、无分层。阴影由原生 DWM（CS_DROPSHADOW）按方角 Region 投影。
#[cfg(target_os = "windows")]
unsafe fn apply_island_region(hwnd: isize) {
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
    // 方角：椭圆直径 0 = 矩形区域（四角与 SWCA 直角一致，无分层）
    let region = unsafe { CreateRoundRectRgn(0, 0, phys_w, phys_h, 0, 0) };
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
    const WM_NCCALCSIZE: u32 = 0x0083;
    const WM_NCACTIVATE: u32 = 0x0086;
    const WM_NCPAINT: u32 = 0x0085;
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

    // WM_NCACTIVATE：阻止激活/失焦时 DWM 绘制标题栏激活态
    if msg == WM_NCACTIVATE {
        return 1;
    }

    // WM_NCPAINT：阻止非客户区（标题栏/边框）绘制
    if msg == WM_NCPAINT {
        return 0;
    }

    // WM_NCCALCSIZE = 0：客户区覆盖整个窗口（含标题栏区域）。
    // 保留 WS_CAPTION 样式位（Win11 上 SWCA Acrylic 依赖它生效），
    // 但通过让客户区吞掉非客户区，系统标题栏在视觉上完全隐藏。
    if msg == WM_NCCALCSIZE {
        return 0;
    }

    if msg == WM_NCHITTEST {
        // Get current hit-test mode
        let prop_name: Vec<u16> = HT_MODE_PROP.encode_utf16().collect();
        let mode = unsafe { GetPropW(hwnd, prop_name.as_ptr()) };

        if mode == HT_MODE_FULL {
            return HTCLIENT;
        }

        // Capsule mode: only the content rect (pill / edge strip) is interactive.
        // The rest of the window is click-through thanks to SetWindowRgn.
        let geo = unsafe { read_capsule_geometry(hwnd) };
        let Some((x, y, w, h)) = (unsafe { capsule_content_phys(hwnd, geo) }) else {
            return unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        };
        let mut pt = lparam_screen_point(lparam);
        if unsafe { ScreenToClient(hwnd, &mut pt) } == 0 {
            return unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        }
        if pt.x >= x && pt.x < x + w && pt.y >= y && pt.y < y + h {
            return HTCLIENT;
        }
        return HTNOWHERE;
    }

    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// 启用原生 Windows 阴影（CS_DROPSHADOW 类样式）。
///
/// 面板即窗口，无 CSS 阴影边距；阴影由 DWM 按窗口 Region 形状（圆角面板 /
/// 胶囊 / 进度条各自形状）投影。CS_DROPSHADOW 是经典 popup+region 方案。
#[cfg(target_os = "windows")]
unsafe fn enable_native_shadow(hwnd: isize) {
    const GCLP_STYLE: i32 = -26;
    const CS_DROPSHADOW: isize = 0x0002_0000;
    let style = unsafe { GetClassLongPtrW(hwnd, GCLP_STYLE) };
    if (style & CS_DROPSHADOW) == 0 {
        unsafe {
            SetClassLongPtrW(hwnd, GCLP_STYLE, style | CS_DROPSHADOW);
        }
    }
}

/// Prepare the floating windows for transparent, borderless rendering.
///
/// 面板即窗口（SPEC-029 二次修订）：无阴影边距，CSS 面板占满窗口；毛玻璃由
/// 窗口级 SWCA Acrylic（apply_island_vibrancy）铺满整个窗口（不被 Region 裁剪），
/// 因此窗口尺寸 = 面板尺寸、无外圈直角毛玻璃。圆角 Region + CSS 圆角壳层对齐；
/// 阴影用原生 CS_DROPSHADOW 按 Region 投影。
#[cfg(target_os = "windows")]
pub fn install_hit_test_subclass(app: &AppHandle) -> Result<(), String> {
    // 胶囊窗口：命中/裁剪区域由 set_capsule_geometry 按形态+贴边动态维护，
    // 这里只安装 subclass 并设置默认几何（胶囊/顶边）。
    if let Some(window) = app.get_webview_window("capsule") {
        let _ = window.set_decorations(false);
        let _ = window.set_effects(None);
    }
    if let Some(hwnd) = get_hwnd_for_label(app, "capsule") {
        unsafe {
            if SetWindowSubclass(hwnd, hit_test_subclass as SUBCLASSPROC, 0, 0) == 0 {
                return Err("SetWindowSubclass failed for capsule".into());
            }
            let prop_name: Vec<u16> = HT_MODE_PROP.encode_utf16().collect();
            SetPropW(hwnd, prop_name.as_ptr(), HT_MODE_CAPSULE);
            let form_prop: Vec<u16> = GEO_FORM_PROP.encode_utf16().collect();
            SetPropW(hwnd, form_prop.as_ptr(), FORM_PILL);
            let edge_prop: Vec<u16> = GEO_EDGE_PROP.encode_utf16().collect();
            SetPropW(hwnd, edge_prop.as_ptr(), EDGE_TOP);

            remove_dwm_border(hwnd);
            strip_title_bar(hwnd);

            const GWL_EXSTYLE: i32 = -20;
            const WS_EX_TOOLWINDOW: isize = 0x00000080;
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if (ex_style & WS_EX_TOOLWINDOW) == 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_TOOLWINDOW);
            }

            enable_native_shadow(hwnd);
            apply_capsule_region(hwnd);
            eprintln!("[PonyClean] Prepared floating window: capsule");
        }
    }

    // island 窗口：整窗圆角（16px）区域，命中区域为全窗口，原生阴影
    for label in ["island"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_decorations(false);
            let _ = window.set_effects(None);
        }

        let Some(hwnd) = get_hwnd_for_label(app, label) else {
            continue;
        };

        unsafe {
            if SetWindowSubclass(hwnd, hit_test_subclass as SUBCLASSPROC, 0, 0) == 0 {
                return Err(format!("SetWindowSubclass failed for {label}"));
            }
            let prop_name: Vec<u16> = HT_MODE_PROP.encode_utf16().collect();
            SetPropW(hwnd, prop_name.as_ptr(), HT_MODE_FULL);

            remove_dwm_border(hwnd);
            strip_title_bar(hwnd);

            const GWL_EXSTYLE: i32 = -20;
            const WS_EX_TOOLWINDOW: isize = 0x00000080;
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if (ex_style & WS_EX_TOOLWINDOW) == 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_TOOLWINDOW);
            }

            enable_native_shadow(hwnd);
            apply_island_region(hwnd);
            eprintln!("[PonyClean] Prepared floating window: {label}");
        }
    }

    Ok(())
}

/// 彻底移除窗口标题栏（样式位 + DWM 渲染策略）。
///
/// 组合拳（社区验证的完整方案）：
/// 1. 移除 WS_CAPTION 样式位（标题栏的样式基础）
/// 2. DWMWA_NCRENDERING_POLICY = DISABLED（DWM 不渲染非客户区）
/// 3. SWP_FRAMECHANGED 触发框架重算
///
/// 视觉拦截由 subclass 的 WM_NCACTIVATE / WM_NCPAINT / WM_NCCALCSIZE 负责。
#[cfg(target_os = "windows")]
pub unsafe fn strip_title_bar(hwnd: isize) {
    const GWL_STYLE: i32 = -16;
    const WS_CAPTION: isize = 0x00C00000;
    const WS_THICKFRAME: isize = 0x00040000;
    const WS_SYSMENU: isize = 0x00080000;
    const WS_MINIMIZEBOX: isize = 0x00020000;
    const WS_MAXIMIZEBOX: isize = 0x00010000;
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    let new_style =
        style & !(WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
    if new_style != style {
        unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, new_style) };
        const SWP_FRAMECHANGED: u32 = 0x0020;
        const SWP_NOMOVE: u32 = 0x0002;
        const SWP_NOSIZE: u32 = 0x0001;
        const SWP_NOZORDER: u32 = 0x0004;
        unsafe {
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
    }
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

    // DWMWA_NCRENDERING_POLICY = 2 — DWM 不渲染非客户区（标题栏/边框）。
    // 这是无边框窗口隐藏标题栏的标准开关：WM_NCCALCSIZE 返回值被 Win11 DWM
    // 忽略，必须显式禁用非客户区渲染。WS_CAPTION 样式位保留（SWCA Acrylic 依赖）。
    const DWMWA_NCRENDERING_POLICY: u32 = 2;
    const DWMNCRP_DISABLED: i32 = 2;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &DWMNCRP_DISABLED as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn install_hit_test_subclass(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// 更新胶囊窗口的形态（胶囊/进度条）与贴边方向，并同步原生圆角区域与命中区域。
///
/// 当前产品仅支持**顶部贴边**（前端恒传 edge="top"）；枚举保留 left/right/bottom
/// 仅为未来扩展预留。窗口 resize 后客户端矩形可能尚未稳定，延迟 40ms 再重算一次区域兜底。
#[tauri::command]
pub fn set_capsule_geometry(
    app: AppHandle,
    state: tauri::State<'_, EdgeCursorState>,
    form: String,
    edge: String,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        eprintln!("[PonyClean] set_capsule_geometry: form={form} edge={edge}");
        let geo = CapsuleGeometry {
            form: match form.as_str() {
                "bar" => CapsuleForm::Bar,
                "pill" => CapsuleForm::Pill,
                _ => return Err(format!("unknown capsule form: {form}")),
            },
            edge: match edge.as_str() {
                "bottom" => ScreenEdge::Bottom,
                "left" => ScreenEdge::Left,
                "right" => ScreenEdge::Right,
                "top" => ScreenEdge::Top,
                _ => return Err(format!("unknown screen edge: {edge}")),
            },
        };
        *state.geo.lock().unwrap() = geo;

        let hwnd = get_hwnd(&app).ok_or("capsule window not found")?;
        unsafe {
            let form_prop: Vec<u16> = GEO_FORM_PROP.encode_utf16().collect();
            let edge_prop: Vec<u16> = GEO_EDGE_PROP.encode_utf16().collect();
            SetPropW(
                hwnd,
                form_prop.as_ptr(),
                match geo.form {
                    CapsuleForm::Bar => FORM_BAR,
                    CapsuleForm::Pill => FORM_PILL,
                },
            );
            SetPropW(
                hwnd,
                edge_prop.as_ptr(),
                match geo.edge {
                    ScreenEdge::Bottom => EDGE_BOTTOM,
                    ScreenEdge::Left => EDGE_LEFT,
                    ScreenEdge::Right => EDGE_RIGHT,
                    ScreenEdge::Top => EDGE_TOP,
                },
            );
            apply_capsule_region(hwnd);
        }

        // 窗口尺寸变更后客户端矩形可能尚未稳定，延迟重算一次区域
        let app2 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            if let Some(hwnd) = get_hwnd_for_label(&app2, "capsule") {
                unsafe {
                    apply_capsule_region(hwnd);
                }
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, &state, form, edge);
    }

    Ok(())
}

/// 胶囊窗口所在显示器的工作区（物理像素，排除任务栏等系统区域）
#[derive(Clone, Serialize)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// 返回胶囊窗口所在显示器的工作区（物理像素）。
///
/// 贴边定位使用工作区而非完整显示器，避免胶囊/面板被任务栏遮挡。
#[tauri::command]
pub fn get_monitor_work_area(app: AppHandle) -> Result<WorkArea, String> {
    #[cfg(target_os = "windows")]
    {
        let Some(hwnd) = get_hwnd_for_label(&app, "capsule") else {
            return Err("capsule window not found".into());
        };
        let mut wr = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(hwnd, &mut wr) } == 0 {
            return Err("GetWindowRect failed".into());
        }
        let center = POINT {
            x: (wr.left + wr.right) / 2,
            y: (wr.top + wr.bottom) / 2,
        };
        let h_mon = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
        if h_mon == 0 {
            return Err("MonitorFromPoint failed".into());
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
        if unsafe { GetMonitorInfoW(h_mon, &mut mi) } == 0 {
            return Err("GetMonitorInfoW failed".into());
        }
        eprintln!(
            "[PonyClean] work area: l={} t={} r={} b={}",
            mi.rc_work.left, mi.rc_work.top, mi.rc_work.right, mi.rc_work.bottom
        );
        Ok(WorkArea {
            left: mi.rc_work.left,
            top: mi.rc_work.top,
            right: mi.rc_work.right,
            bottom: mi.rc_work.bottom,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("get_monitor_work_area is only supported on Windows".into())
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// 前端日志转发（dev 排查用）：把 WebView 的 console 输出到 Rust 终端，
/// 因为 WebView console 默认不显示在 dev:tauri 的终端里。
#[tauri::command]
pub fn log_frontend(level: String, message: String) {
    eprintln!("[PonyClean][frontend:{level}] {message}");
}

/// 切换 island 窗口的概要态/展开态高度，并重算圆角裁剪区域与区域化模糊。
///
/// 窗口物理高度 = 内容高度 + 阴影边距（顶边贴边无上边距）。
/// 展开态用于承载监控/清理面板，概要态仅显示摘要条。
#[tauri::command]
pub fn set_island_expanded(app: AppHandle, expanded: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let window = app
            .get_webview_window("island")
            .ok_or("island window not found")?;
        let h = if expanded {
            ISLAND_EXPANDED_H
        } else {
            LOGICAL_H
        };
        window
            .set_size(tauri::LogicalSize::new(LOGICAL_W, h))
            .map_err(|e| e.to_string())?;
        // 圆角 Region + 原生阴影：窗口 resize 后客户端矩形可能尚未稳定，
        // 立即应用一次，并延迟 40ms 重算兜底（与胶囊 set_capsule_geometry 同策略）。
        // 毛玻璃为窗口级 SWCA Acrylic（apply_island_vibrancy），无需随尺寸重设。
        if let Some(hwnd) = get_hwnd_for_label(&app, "island") {
            unsafe {
                apply_island_region(hwnd);
            }
        }
        let app2 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            if let Some(hwnd) = get_hwnd_for_label(&app2, "island") {
                unsafe {
                    apply_island_region(hwnd);
                }
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, expanded);
    }

    Ok(())
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
