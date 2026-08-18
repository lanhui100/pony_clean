//! Windows 进程图标提取：读取 exe 文件内嵌图标，编码为 PNG base64 data URL。
//!
//! 使用 `ExtractIconExW` 获取 HICON，通过 GDI GetDIBits 提取 RGBA 像素，
//! 用 `image` crate 编码 PNG，最终输出 `data:image/png;base64,...` 格式。
//!
//! 仅 Windows 平台可用，非 Windows 返回空桩。

#![cfg(windows)]

use base64::Engine;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 从 exe 路径提取图标，返回 base64 编码的 PNG data URL。
///
/// 返回格式：`data:image/png;base64,iVBOR...`
/// 提取失败（如系统进程、无图标资源、权限不足）返回 `None`。
pub fn extract_exe_icon_png(exe_path: &str) -> Option<String> {
    let wide: Vec<u16> = std::path::Path::new(exe_path)
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut hicon_large = HICON::default();
        let mut hicon_small = HICON::default();

        let count = ExtractIconExW(
            windows::core::PCWSTR::from_raw(wide.as_ptr()),
            0,
            Some(&mut hicon_large),
            Some(&mut hicon_small),
            1,
        );

        if count == 0 {
            return None;
        }

        // 优先使用小图标 (32x32)，释放大图标
        let hicon = hicon_small;
        if !hicon_large.is_invalid() {
            let _ = DestroyIcon(hicon_large);
        }

        if hicon.is_invalid() {
            return None;
        }

        let result = hicon_to_png_base64(hicon);
        let _ = DestroyIcon(hicon);
        result
    }
}

/// 将 HICON 转换为 RGBA 像素，编码为 PNG base64。
unsafe fn hicon_to_png_base64(hicon: HICON) -> Option<String> {
    let mut icon_info = ICONINFO::default();
    if GetIconInfo(hicon, &mut icon_info).is_err() {
        return None;
    }

    // 获取彩色位图信息
    let mut bmp = BITMAP::default();
    if GetObjectW(
        icon_info.hbmColor,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut std::ffi::c_void),
    ) == 0
    {
        let _ = DeleteObject(icon_info.hbmColor);
        let _ = DeleteObject(icon_info.hbmMask);
        return None;
    }

    let w = bmp.bmWidth as u32;
    let h = bmp.bmHeight as u32;

    if w == 0 || h == 0 {
        let _ = DeleteObject(icon_info.hbmColor);
        let _ = DeleteObject(icon_info.hbmMask);
        return None;
    }

    // 创建兼容 DC 并选择位图
    let hdc = CreateCompatibleDC(None);
    if hdc.is_invalid() {
        let _ = DeleteObject(icon_info.hbmColor);
        let _ = DeleteObject(icon_info.hbmMask);
        return None;
    }

    let _old = SelectObject(hdc, icon_info.hbmColor);

    // 准备 BITMAPINFO（32-bit top-down DIB）
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = w as i32;
    bmi.bmiHeader.biHeight = -(h as i32); // top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB.0;

    let row_size = w as usize * 4;
    let pixel_size = row_size * h as usize;
    let mut pixels_bgra = vec![0u8; pixel_size];

    let result = GetDIBits(
        hdc,
        icon_info.hbmColor,
        0,
        h,
        Some(pixels_bgra.as_mut_ptr() as *mut std::ffi::c_void),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    // 清理 GDI 对象
    let _ = DeleteDC(hdc);
    let _ = DeleteObject(icon_info.hbmColor);
    let _ = DeleteObject(icon_info.hbmMask);

    if result == 0 {
        return None;
    }

    // BGRA → RGBA 转换
    let mut pixels_rgba = Vec::with_capacity(pixel_size);
    for chunk in pixels_bgra.chunks_exact(4) {
        pixels_rgba.push(chunk[2]); // R
        pixels_rgba.push(chunk[1]); // G
        pixels_rgba.push(chunk[0]); // B
        pixels_rgba.push(chunk[3]); // A
    }

    // 用 image crate 编码 PNG
    let img = image::RgbaImage::from_raw(w, h, pixels_rgba)?;
    let mut png_buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png_buf), image::ImageFormat::Png)
        .ok()?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_buf);
    Some(format!("data:image/png;base64,{b64}"))
}