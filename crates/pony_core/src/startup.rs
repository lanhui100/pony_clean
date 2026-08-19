//! 开机自启动项管理：枚举非 Windows 系统的第三方自启动应用，支持关闭/重新打开。
//!
//! 数据来源（Windows）：
//! - 注册表 `Run` 键：HKCU（用户级）、HKLM 与 Wow6432Node（机器级）
//! - 启动文件夹：用户级（%APPDATA%\...\Startup）、公共级（%ProgramData%\...\Startup）
//!
//! 禁用（关闭但保留、可再打开）机制：
//! - 注册表项：值移动到 `Run\PonyCleanDisabled` 子键（子键不会被系统执行）
//! - 启动文件夹项：文件重命名为 `.disabled.` 后缀（扩展名改变，不再执行）
//!
//! Windows 系统自带的启动项（路径位于系统目录 / Microsoft 目录，或名称含
//! microsoft / windows / onedrive 等）会被过滤，只返回第三方应用。
//! 其他工具遗留的 `xxx_disabled` Run 值会被识别为对应应用的"已关闭"形态。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 自启动项来源
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StartupSource {
    /// 当前用户注册表 Run 键（HKCU），用户级可直接修改
    RegistryUser,
    /// 本机注册表 Run 键（HKLM / Wow6432Node），修改需要管理员权限
    RegistryMachine,
    /// 用户启动文件夹
    FolderUser,
    /// 公共启动文件夹，修改需要管理员权限
    FolderMachine,
}

/// 一个第三方开机自启动项
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartupItem {
    /// 显示名称（注册表值名 / 启动文件夹文件名去扩展名）
    pub name: String,
    /// 启动命令（注册表来源；快捷方式来源为空）
    pub command: String,
    /// 解析出的可执行文件路径（尽力解析，可能为空）
    pub exe_path: String,
    /// 来源
    pub source: StartupSource,
    /// 关闭该项是否需要管理员权限
    pub requires_admin: bool,
    /// 是否处于启用状态（false = 已关闭，可通过开关重新打开）
    pub enabled: bool,
    /// 应用微缩图标（ICO data URL，如 data:image/x-icon;base64,...）
    pub icon: Option<String>,
    /// 存储位置的实际名称（与 name 不同时才有值，如遗留的 `xxx_disabled` 值 / 完整文件名）。
    /// 序列化时省略 `None`；反序列化时缺失则视为 `None`（否则 list → invoke 回传会
    /// 因缺字段报 `invalid args`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reg_name: Option<String>,
    /// 原始值是否为 REG_EXPAND_SZ（恢复时保留类型，避免环境变量不展开）。
    /// 序列化时省略 `false`；反序列化时缺失则视为 `false`。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub expand_sz: bool,
}

/// 枚举当前全部第三方开机自启动项（含已关闭项，过滤 Windows 系统项）。
///
/// 任一来源读取失败时静默跳过（如 HKLM 无权限），不阻断整体结果。
pub fn list_startup_items() -> Vec<StartupItem> {
    #[cfg(target_os = "windows")]
    {
        platform::list()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// 关闭一个开机自启动项（禁用但保留，可随时重新打开）；失败时返回人类可读的原因。
///
/// 注册表项：值移动到 `Run\PonyCleanDisabled` 子键（该子键不会被系统执行）；
/// 启动文件夹项：文件重命名为 `.disabled.` 后缀（扩展名改变，不再执行）。
pub fn disable_startup_item(item: &StartupItem) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        platform::disable(item)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = item;
        Err("当前平台不支持".to_string())
    }
}

/// 重新打开一个已关闭的开机自启动项；失败时返回人类可读的原因。
pub fn enable_startup_item(item: &StartupItem) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        platform::enable(item)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = item;
        Err("当前平台不支持".to_string())
    }
}

/// 提权子进程动作
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupAction {
    /// 关闭开机自启动
    Disable,
    /// 重新打开开机自启动
    Enable,
}

impl StartupAction {
    /// 命令行动作名（`--elevated-startup` 子进程参数）
    pub fn as_str(self) -> &'static str {
        match self {
            StartupAction::Disable => "disable",
            StartupAction::Enable => "enable",
        }
    }

    /// 从命令行动作名解析
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "disable" => Some(StartupAction::Disable),
            "enable" => Some(StartupAction::Enable),
            _ => None,
        }
    }
}

/// 以管理员权限执行关闭/打开一个系统级自启动项（自动提权子进程，触发 UAC 确认）。
///
/// 适用场景：HKLM 注册表 / 公共启动文件夹等需要管理员权限的项，在当前进程
/// 无管理员权限时直接操作会失败。本函数通过 `ShellExecuteW("runas")` 启动
/// 当前可执行文件的隐藏子进程（`--elevated-startup` 模式），由子进程
/// 以管理员身份执行并把结果写入临时文件，本进程轮询等待结果。
///
/// 用户取消 UAC 或超时（30s）时返回错误。
pub fn startup_item_elevated(action: StartupAction, item: &StartupItem) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        platform::elevated_run(action, item)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (action, item);
        Err("当前平台不支持".to_string())
    }
}

/// 提权子进程入口：按动作解码请求并执行，把结果写入结果文件（`--elevated-startup` 模式）。
///
/// 返回进程退出码（0 = 成功，非 0 = 失败），供主进程等待时读取。
pub fn run_elevated_startup(action: &str, b64_request: &str, result_file: &str) -> i32 {
    #[cfg(target_os = "windows")]
    {
        platform::run_elevated_startup(action, b64_request, result_file)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (action, b64_request, result_file);
        1
    }
}

/* ═══════════ 跨平台纯函数（可单测） ═══════════ */

/// 展开常见环境变量（一次替换，未覆盖的 %VAR% 原样保留；变量名大小写不敏感）
fn expand_env(input: &str) -> String {
    const VARS: [(&str, &str); 8] = [
        ("%SystemRoot%", "SystemRoot"),
        ("%WINDIR%", "WINDIR"),
        ("%ProgramFiles%", "ProgramFiles"),
        ("%ProgramFiles(x86)%", "ProgramFiles(x86)"),
        ("%APPDATA%", "APPDATA"),
        ("%LOCALAPPDATA%", "LOCALAPPDATA"),
        ("%USERPROFILE%", "USERPROFILE"),
        ("%PUBLIC%", "PUBLIC"),
    ];
    let mut out = input.to_string();
    for (token, var) in VARS {
        if let Ok(val) = std::env::var(var) {
            // 先替换小写变体（注册表中常见 `%windir%`），再替换原样
            let lower = token.to_lowercase();
            if lower != *token {
                out = out.replace(&lower, &val);
            }
            out = out.replace(token, &val);
        }
    }
    out
}

/// 从启动命令中尽力解析可执行文件路径（处理引号、参数、.exe 截断）
fn parse_command_exe(command: &str) -> Option<String> {
    let cmd = expand_env(command.trim());
    if cmd.is_empty() {
        return None;
    }
    // 引号包裹（Windows 含空格路径的常规写法）：取引号内整体
    if let Some(rest) = cmd.strip_prefix('"') {
        let inner = rest.split('"').next().unwrap_or("").trim();
        if !inner.is_empty() {
            return Some(inner.to_string());
        }
    }
    // 无引号：在整条命令中定位 .exe 并截断（忽略后续参数）
    let lower = cmd.to_lowercase();
    if let Some(idx) = lower.find(".exe") {
        return Some(cmd[..idx + 4].to_string());
    }
    // 退而求其次：第一个空白分隔 token
    let token = cmd.split_whitespace().next().unwrap_or("");
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// 判断是否为 Windows 系统自带的启动项（名称或可执行路径命中系统特征）
fn is_system_item(name: &str, exe: Option<&str>) -> bool {
    let lower_name = name.to_lowercase();
    if ["microsoft", "windows", "onedrive"]
        .iter()
        .any(|k| lower_name.contains(k))
    {
        return true;
    }
    if let Some(exe) = exe {
        let lower = exe.to_lowercase().replace('/', "\\");
        if lower.contains("\\windows\\")
            || lower.contains("\\microsoft\\")
            || lower.contains("windowsapps")
        {
            return true;
        }
    }
    false
}

/// 恢复写回 Run 键时使用的值名（循环去掉其他工具遗留的 `_disabled` 后缀，
/// 兼容 `xxx_disabled_disabled` 这类双重残留）
fn restore_reg_name(reg_name: &str) -> String {
    let mut name = reg_name.to_string();
    while let Some(rest) = name.strip_suffix("_disabled") {
        if rest.is_empty() {
            break;
        }
        name = rest.to_string();
    }
    name
}

/// 判断注册表值名是否为指定应用的自启动注册（原名或带 `_disabled` 后缀的变体）。
///
/// Windows 对 Run 键不识别 `_disabled` 后缀，任何变体登录时都会被执行，
/// 因此禁用时须把该应用的全部变体一并清除。
fn is_same_reg_app(value_name: &str, item_name: &str) -> bool {
    restore_reg_name(value_name) == item_name
}

/// 启动文件夹文件禁用后的文件名（追加 `.disabled` 扩展名，扩展名不再是
/// `.lnk`/`.exe` 等可执行形态，登录时不会被 explorer 执行）：
/// `WeChat.lnk` → `WeChat.lnk.disabled`
fn disable_file_name(fname: &str) -> String {
    format!("{fname}.disabled")
}

/// 启动文件夹文件恢复原名的文件名：`WeChat.disabled.lnk` → `WeChat.lnk`，`NoExt.disabled` → `NoExt`
fn enable_file_name(fname: &str) -> String {
    let p = Path::new(fname);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    // 两种情况：`X.disabled.ext`（stem 含 .disabled）或 `X.disabled`（ext 就是 disabled）
    if let Some(rest) = stem.strip_suffix(".disabled") {
        match p.extension() {
            Some(ext) => format!("{}.{}", rest, ext.to_string_lossy()),
            None => rest.to_string(),
        }
    } else if p
        .extension()
        .map(|e| e.eq_ignore_ascii_case("disabled"))
        .unwrap_or(false)
    {
        stem
    } else {
        fname.to_string()
    }
}

/// 判断启动文件夹文件名是否为已禁用形态。
///
/// 只有扩展名为 `.disabled`（如 `WeChat.lnk.disabled`）才不会被系统执行；
/// `WeChat.disabled.lnk` 这类旧工具残留的中间改名形态扩展名仍是 `.lnk`，
/// 登录时照样被执行，因此视为**启用**（可一键再次关闭并转为有效形态）。
fn is_disabled_file(fname: &str) -> bool {
    Path::new(fname)
        .extension()
        .map(|e| e.eq_ignore_ascii_case("disabled"))
        .unwrap_or(false)
}

/// 从 .lnk 二进制中启发式提取 UTF-16LE 的绝对路径（"X:\..."）。
///
/// Shell Link 格式复杂，这里用社区常用启发式：扫描字节流中形如
/// `X\0:\0\\0` 的 UTF-16LE 序列并连续读取 ASCII 字符，优先返回真实存在的路径。
fn lnk_target_path(bytes: &[u8]) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i + 5 < bytes.len() {
        if bytes[i + 1] == 0
            && bytes[i + 2] == b':'
            && bytes[i + 3] == 0
            && bytes[i + 4] == b'\\'
            && bytes[i + 5] == 0
            && bytes[i].is_ascii_alphabetic()
        {
            let mut path = String::with_capacity(64);
            path.push(bytes[i] as char);
            path.push(':');
            path.push('\\');
            let mut j = i + 6;
            while j + 1 < bytes.len() {
                let lo = bytes[j];
                let hi = bytes[j + 1];
                if hi != 0 || lo == 0 || lo.is_ascii_control() {
                    break;
                }
                path.push(lo as char);
                j += 2;
            }
            if path.len() > 3 {
                candidates.push(path);
            }
            i = j.saturating_add(2);
        } else {
            i += 2;
        }
    }
    for c in &candidates {
        if Path::new(c).exists() {
            return Some(c.clone());
        }
    }
    candidates.into_iter().next()
}

/// 按（来源, 名称）去重：同源同名（如 `Feishu` 与 `Feishu_disabled` 变体）合并为一项，
/// 任一变体处于启用态则整体视为启用（系统会执行所有未被 StartupApproved 标记禁用的值）
fn dedup_items(items: &mut Vec<StartupItem>) {
    let mut first_idx = std::collections::HashMap::<(StartupSource, String), usize>::new();
    let mut idx = 0;
    while idx < items.len() {
        let key = (items[idx].source, items[idx].name.clone());
        if let Some(&first) = first_idx.get(&key) {
            if items[idx].enabled && !items[first].enabled {
                items[first].enabled = true;
            }
            items.remove(idx);
        } else {
            first_idx.insert(key, idx);
            idx += 1;
        }
    }
}

/* ═══════════ base64（提权请求传输用，避免命令行引号转义问题） ═══════════ */

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// 标准 base64 编码
fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        out.push(BASE64_ALPHABET[(b[0] >> 2) as usize] as char);
        out.push(BASE64_ALPHABET[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALPHABET[(((b[1] & 0x0F) << 2) | (b[2] >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(b[2] & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// 标准 base64 解码（忽略填充，非法字符返回 None）
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut buf = [0u8; 4];
    let mut count = 0usize;
    for ch in s.bytes() {
        if ch == b'=' {
            break;
        }
        let val = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buf[count] = val;
        count += 1;
        if count == 4 {
            out.push((buf[0] << 2) | (buf[1] >> 4));
            out.push(((buf[1] & 0x0F) << 4) | (buf[2] >> 2));
            out.push(((buf[2] & 0x03) << 6) | buf[3]);
            count = 0;
        }
    }
    match count {
        2 => out.push((buf[0] << 2) | (buf[1] >> 4)),
        3 => {
            out.push((buf[0] << 2) | (buf[1] >> 4));
            out.push(((buf[1] & 0x0F) << 4) | (buf[2] >> 2));
        }
        1 => return None,
        _ => {}
    }
    Some(out)
}

/// 组装 ICO 文件字节（ICONDIR + 32bpp DIB + AND mask），供图标提取使用
fn ico_from_bgra(width: u32, height: u32, bgra: &[u8]) -> Vec<u8> {
    let mask_row = width.div_ceil(32) * 4;
    let mask_len = (mask_row * height) as usize;
    let data_len = 40 + bgra.len() + mask_len;
    let mut out = Vec::with_capacity(22 + data_len);
    // ICONDIR
    out.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    // ICONDIRENTRY
    out.push(if width >= 256 { 0 } else { width as u8 });
    out.push(if height >= 256 { 0 } else { height as u8 });
    out.push(0);
    out.push(0);
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bpp
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes()); // offset
    // BITMAPINFOHEADER（XOR + AND 两倍高）
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&((height as i32) * 2).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(bgra.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    // XOR 像素（BGRA）
    out.extend_from_slice(bgra);
    // AND mask（32bpp 图标以 alpha 为准，全 0）
    out.extend(std::iter::repeat_n(0u8, mask_len));
    out
}

/* ═══════════ Windows 平台实现 ═══════════ */

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::HICON;
    use winreg::RegKey;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, RegType,
    };
    use winreg::reg_key::HKEY;
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const RUN_KEY_32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
    /// 禁用项存放子键（Run 键的子键不会被系统执行，因此禁用不会失效）
    const DISABLED_SUBKEY: &str = "PonyCleanDisabled";

    pub(super) fn list() -> Vec<StartupItem> {
        // 任务管理器式禁用标记（StartupApproved，首字节 0x03 = 禁用）：
        // Windows 只认这里的标记，`_disabled` 后缀等改名方式不会被系统执行豁免
        let disabled_names = startup_approved_disabled();
        let mut items = Vec::new();
        for (root, sub, source, admin) in [
            (
                HKEY_CURRENT_USER,
                RUN_KEY,
                StartupSource::RegistryUser,
                false,
            ),
            (
                HKEY_LOCAL_MACHINE,
                RUN_KEY,
                StartupSource::RegistryMachine,
                true,
            ),
            (
                HKEY_LOCAL_MACHINE,
                RUN_KEY_32,
                StartupSource::RegistryMachine,
                true,
            ),
        ] {
            collect_registry(root, sub, source, admin, &disabled_names, &mut items);
            collect_registry_disabled(root, sub, source, admin, &mut items);
        }
        if let Ok(dir) = startup_folder(StartupSource::FolderUser) {
            collect_folder(
                &dir,
                StartupSource::FolderUser,
                false,
                &disabled_names,
                &mut items,
            );
        }
        if let Ok(dir) = startup_folder(StartupSource::FolderMachine) {
            collect_folder(
                &dir,
                StartupSource::FolderMachine,
                true,
                &disabled_names,
                &mut items,
            );
        }
        dedup_items(&mut items);
        items.sort_by_key(|a| a.name.to_lowercase());
        // 提取应用微缩图标
        for item in &mut items {
            if !item.exe_path.is_empty() {
                item.icon = extract_icon_data_url(&item.exe_path);
            }
        }
        items
    }

    pub(super) fn disable(item: &StartupItem) -> Result<(), String> {
        match item.source {
            StartupSource::RegistryUser => registry_move(HKEY_CURRENT_USER, RUN_KEY, item),
            StartupSource::RegistryMachine => {
                // 两处（64 位 / 32 位视角）都尝试
                let mut done = false;
                let mut last_err = None;
                for sub in [RUN_KEY, RUN_KEY_32] {
                    match registry_move(HKEY_LOCAL_MACHINE, sub, item) {
                        Ok(()) => done = true,
                        Err(e) => last_err = Some(e),
                    }
                }
                if done {
                    Ok(())
                } else {
                    Err(last_err.unwrap_or_else(|| admin_required_msg(&item.name, "关闭")))
                }
            }
            StartupSource::FolderUser | StartupSource::FolderMachine => {
                let dir = startup_folder(item.source)?;
                let fname = item.reg_name.as_deref().unwrap_or(&item.name);
                let src = dir.join(fname);
                if !src.exists() {
                    return Err(format!("未找到「{}」的开机自启动项", item.name));
                }
                let dst = dir.join(disable_file_name(fname));
                std::fs::rename(&src, &dst).map_err(|e| {
                    if item.requires_admin {
                        admin_required_msg(&item.name, "关闭")
                    } else {
                        format!("关闭「{}」的开机自启动失败: {e}", item.name)
                    }
                })?;
                Ok(())
            }
        }
    }

    pub(super) fn enable(item: &StartupItem) -> Result<(), String> {
        match item.source {
            StartupSource::RegistryUser => registry_restore(HKEY_CURRENT_USER, RUN_KEY, item),
            StartupSource::RegistryMachine => {
                let mut done = false;
                let mut last_err = None;
                for sub in [RUN_KEY, RUN_KEY_32] {
                    match registry_restore(HKEY_LOCAL_MACHINE, sub, item) {
                        Ok(()) => done = true,
                        Err(e) => last_err = Some(e),
                    }
                }
                if done {
                    Ok(())
                } else {
                    Err(last_err.unwrap_or_else(|| admin_required_msg(&item.name, "打开")))
                }
            }
            StartupSource::FolderUser | StartupSource::FolderMachine => {
                let dir = startup_folder(item.source)?;
                let fname = item.reg_name.as_deref().unwrap_or(&item.name);
                let src = dir.join(fname);
                if !src.exists() {
                    return Err(format!("未找到「{}」的开机自启动项", item.name));
                }
                let dst = dir.join(enable_file_name(fname));
                std::fs::rename(&src, &dst).map_err(|e| {
                    if item.requires_admin {
                        admin_required_msg(&item.name, "打开")
                    } else {
                        format!("打开「{}」的开机自启动失败: {e}", item.name)
                    }
                })?;
                Ok(())
            }
        }
    }

    /// 注册表项：从 Run 键移入 `Run\PonyCleanDisabled` 子键（禁用）。
    ///
    /// 同名应用的多种注册形态（原名与 `_disabled` 后缀变体）都会被执行，
    /// 因此一次性全部清除，再以规范名写入禁用子键，避免残留值导致禁用失效。
    fn registry_move(root: HKEY, run_subkey: &str, item: &StartupItem) -> Result<(), String> {
        // 删除 Run 键下该应用的全部变体值（原名与 `_disabled` 后缀变体）
        let run = RegKey::predef(root)
            .open_subkey_with_flags(run_subkey, KEY_QUERY_VALUE | KEY_SET_VALUE)
            .map_err(|e| format!("打开 Run 键失败: {e}"))?;
        let to_delete: Vec<String> = run
            .enum_values()
            .flatten()
            .map(|(name, _)| name)
            .filter(|name| super::is_same_reg_app(name, &item.name))
            .collect();
        for name in &to_delete {
            match run.delete_value(name) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("关闭「{}」的开机自启动失败: {e}", item.name)),
            }
        }
        // 写入禁用子键（先清理该应用旧变体，再以规范名写入，保持幂等）
        let disabled_path = format!(r"{run_subkey}\{DISABLED_SUBKEY}");
        let disabled = RegKey::predef(root)
            .create_subkey_with_flags(&disabled_path, KEY_QUERY_VALUE | KEY_SET_VALUE)
            .map_err(|e| format!("创建禁用存储失败: {e}"))?
            .0;
        let stale: Vec<String> = disabled
            .enum_values()
            .flatten()
            .map(|(name, _)| name)
            .filter(|name| super::is_same_reg_app(name, &item.name))
            .collect();
        for name in stale {
            let _ = disabled.delete_value(&name);
        }
        disabled
            .set_raw_value(&item.name, &reg_value(&item.command, item.expand_sz))
            .map_err(|e| format!("关闭「{}」的开机自启动失败: {e}", item.name))?;
        // 清除 StartupApproved 残留标记（该标记与值名绑定，禁用后已无意义）
        remove_startup_approved_marker(&item.name);
        Ok(())
    }

    /// 注册表项：从 `Run\PonyCleanDisabled` 子键移回 Run 键（重新打开）。
    ///
    /// 先清理禁用子键与 Run 键中的全部同名变体，再以规范名写回，
    /// 并删除 StartupApproved 中的禁用标记（否则系统仍会跳过该值）。
    fn registry_restore(root: HKEY, run_subkey: &str, item: &StartupItem) -> Result<(), String> {
        // 清理禁用子键中该应用的全部变体（幂等）。
        // 注意：枚举值需要 KEY_QUERY_VALUE，仅 KEY_SET_VALUE 会导致
        // RegEnumValueW 持续返回 ACCESS_DENIED，winreg 迭代器无限重试。
        let disabled_path = format!(r"{run_subkey}\{DISABLED_SUBKEY}");
        if let Ok(disabled) = RegKey::predef(root)
            .open_subkey_with_flags(&disabled_path, KEY_QUERY_VALUE | KEY_SET_VALUE)
        {
            let stale: Vec<String> = disabled
                .enum_values()
                .flatten()
                .map(|(name, _)| name)
                .filter(|name| super::is_same_reg_app(name, &item.name))
                .collect();
            for name in stale {
                let _ = disabled.delete_value(&name);
            }
        }
        // 清理 Run 键中残留的同名变体（防止恢复后双重启动）
        if let Ok(run) =
            RegKey::predef(root).open_subkey_with_flags(run_subkey, KEY_QUERY_VALUE | KEY_SET_VALUE)
        {
            let stale: Vec<String> = run
                .enum_values()
                .flatten()
                .map(|(name, _)| name)
                .filter(|name| super::is_same_reg_app(name, &item.name))
                .collect();
            for name in stale {
                match run.delete_value(&name) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("打开「{}」的开机自启动失败: {e}", item.name)),
                }
            }
        }
        // 写回 Run（规范名）
        let run = RegKey::predef(root)
            .open_subkey_with_flags(run_subkey, KEY_SET_VALUE)
            .map_err(|e| format!("打开 Run 键失败: {e}"))?;
        run.set_raw_value(&item.name, &reg_value(&item.command, item.expand_sz))
            .map_err(|e| format!("打开「{}」的开机自启动失败: {e}", item.name))?;
        // 删除 StartupApproved 禁用标记（否则系统仍跳过该值，恢复失效）
        remove_startup_approved_marker(&item.name);
        Ok(())
    }

    /// 构造注册表字符串值（保留 REG_EXPAND_SZ 类型，避免环境变量不展开）
    fn reg_value(command: &str, expand: bool) -> winreg::RegValue {
        let mut bytes: Vec<u8> = command
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        bytes.extend_from_slice(&[0, 0]); // 以 \0 结尾
        winreg::RegValue {
            bytes,
            vtype: if expand {
                RegType::REG_EXPAND_SZ
            } else {
                RegType::REG_SZ
            },
        }
    }

    /// 系统级启动项（HKLM / 公共启动文件夹）在无管理员权限时的统一提示
    fn admin_required_msg(name: &str, verb: &str) -> String {
        format!(
            "{verb}「{}」的开机自启动需要管理员权限：请以管理员身份运行 PonyClean 后重试，\
             或在「任务管理器 → 启动应用」中操作",
            name
        )
    }

    /* ─── 自动提权（ShellExecute runas 子进程） ─── */

    const ELEVATED_FLAG: &str = "--elevated-startup";
    const ELEVATED_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    /// 以管理员权限执行关闭/打开：启动提权子进程并等待其结果
    pub(super) fn elevated_run(action: StartupAction, item: &StartupItem) -> Result<(), String> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
        use windows::core::{PCWSTR, w};

        // 请求与结果文件（结果文件名同时充当本次提权的 token）
        let request = serde_json::to_string(item).map_err(|e| format!("序列化请求失败: {e}"))?;
        let request_b64 = base64_encode(request.as_bytes());
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let token = format!("{}_{}", std::process::id(), nanos);
        let result_file = std::env::temp_dir().join(format!("ponyclean_elevate_{token}.json"));
        let result_file_str = result_file.to_string_lossy().into_owned();

        let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;
        let params = format!(
            "{ELEVATED_FLAG} {} {request_b64} \"{result_file_str}\"",
            action.as_str()
        );

        // 参数转 UTF-16（exe 与 params 均含非 ASCII 时需宽字符）
        let exe_wide: Vec<u16> = exe.to_string_lossy().encode_utf16().collect();
        let params_wide: Vec<u16> = params.encode_utf16().collect();

        let ret = unsafe {
            ShellExecuteW(
                HWND(0),
                PCWSTR(w!("runas").as_ptr()),
                PCWSTR(exe_wide.as_ptr()),
                PCWSTR(params_wide.as_ptr()),
                PCWSTR::null(),
                SW_HIDE,
            )
        };
        let code = ret.0 as isize;
        if code <= 32 {
            // 1223 = ERROR_CANCELLED（用户在 UAC 弹窗中拒绝）
            if code == 1223 {
                return Err("未获得管理员授权，操作已取消".to_string());
            }
            return Err(format!("启动管理员进程失败（错误码 {code}）"));
        }

        // 轮询等待提权子进程写入结果文件
        let deadline = std::time::Instant::now() + ELEVATED_TIMEOUT;
        loop {
            if result_file.exists() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err("等待管理员授权超时，请重试".to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        let outcome = std::fs::read_to_string(&result_file)
            .map_err(|e| format!("读取提权结果失败: {e}"))
            .and_then(|data| {
                let v: serde_json::Value =
                    serde_json::from_str(&data).map_err(|e| format!("解析提权结果失败: {e}"))?;
                if v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) {
                    Ok(())
                } else {
                    Err(v
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("未知错误")
                        .to_string())
                }
            });
        let _ = std::fs::remove_file(&result_file);
        outcome
    }

    /// 提权子进程主体：解码请求 → 按动作执行 → 写入结果文件
    pub(super) fn run_elevated_startup(action: &str, b64_request: &str, result_file: &str) -> i32 {
        let outcome = StartupAction::parse(action)
            .ok_or_else(|| "未知动作".to_string())
            .and_then(|action| {
                base64_decode(b64_request)
                    .ok_or_else(|| "请求解码失败".to_string())
                    .and_then(|bytes| {
                        serde_json::from_slice::<StartupItem>(&bytes).map_err(|e| e.to_string())
                    })
                    .and_then(|item| match action {
                        StartupAction::Disable => disable(&item),
                        StartupAction::Enable => enable(&item),
                    })
            });

        let payload = match &outcome {
            Ok(()) => serde_json::json!({ "ok": true }),
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        };
        let _ = std::fs::write(result_file, payload.to_string());
        if outcome.is_ok() { 0 } else { 1 }
    }

    /// 读取一个 Run 键下的第三方启动项（启用项；遗留 `xxx_disabled` 值按其真实
    /// 执行状态展示：StartupApproved 标记为禁用的值不执行，其余一律执行）
    fn collect_registry(
        root: HKEY,
        subkey: &str,
        source: StartupSource,
        admin: bool,
        disabled_names: &std::collections::HashSet<String>,
        out: &mut Vec<StartupItem>,
    ) {
        let Ok(root) = RegKey::predef(root).open_subkey(subkey) else {
            return;
        };
        for entry in root.enum_values() {
            let Ok((reg_name, value)) = entry else {
                continue;
            };
            if reg_name.trim().is_empty() {
                continue;
            }
            // 仅处理字符串值（REG_SZ / REG_EXPAND_SZ）；
            // 注册表字符串为 UTF-16LE，必须经 winreg Display 解码（UTF-8 直读会乱码）
            let command = match value.vtype {
                RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
                    value.to_string().trim_end_matches('\0').to_string()
                }
                _ => continue,
            };
            // 显示名：循环去掉其他工具遗留的 `_disabled` 后缀（如 `Feishu_disabled`）
            let name = super::restore_reg_name(&reg_name);
            // 启用判定：以 StartupApproved 禁用标记为准；`_disabled` 后缀只是
            // 其他工具的无效残留，Windows 登录时照样执行该值
            let enabled = !disabled_names.contains(&reg_name);
            let exe = parse_command_exe(&command);
            if is_system_item(&name, exe.as_deref()) {
                continue;
            }
            out.push(StartupItem {
                name,
                command,
                exe_path: exe.unwrap_or_default(),
                source,
                requires_admin: admin,
                enabled,
                icon: None,
                reg_name: Some(reg_name),
                expand_sz: value.vtype == RegType::REG_EXPAND_SZ,
            });
        }
    }

    /// 读取 `Run\PonyCleanDisabled` 子键下的已关闭项
    fn collect_registry_disabled(
        root: HKEY,
        run_subkey: &str,
        source: StartupSource,
        admin: bool,
        out: &mut Vec<StartupItem>,
    ) {
        let disabled_path = format!(r"{run_subkey}\{DISABLED_SUBKEY}");
        let Ok(disabled) = RegKey::predef(root).open_subkey(&disabled_path) else {
            return;
        };
        for entry in disabled.enum_values() {
            let Ok((reg_name, value)) = entry else {
                continue;
            };
            if reg_name.trim().is_empty() {
                continue;
            }
            let command = match value.vtype {
                RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
                    value.to_string().trim_end_matches('\0').to_string()
                }
                _ => continue,
            };
            let name = super::restore_reg_name(&reg_name);
            let exe = parse_command_exe(&command);
            if is_system_item(&name, exe.as_deref()) {
                continue;
            }
            out.push(StartupItem {
                name,
                command,
                exe_path: exe.unwrap_or_default(),
                source,
                requires_admin: admin,
                enabled: false,
                icon: None,
                reg_name: Some(reg_name),
                expand_sz: value.vtype == RegType::REG_EXPAND_SZ,
            });
        }
    }

    /// 读取一个启动文件夹下的第三方启动项（.lnk 会尽力解析目标路径）
    fn collect_folder(
        dir: &Path,
        source: StartupSource,
        admin: bool,
        disabled_names: &std::collections::HashSet<String>,
        out: &mut Vec<StartupItem>,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let fname = entry.file_name().to_string_lossy().into_owned();
            if fname.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            let is_disabled = is_disabled_file(&fname);
            // 显示名 = 原始文件名去 `.disabled` 段、再去扩展名
            let clean = if is_disabled {
                enable_file_name(&fname)
            } else {
                fname.clone()
            };
            let name = Path::new(&clean)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| clean.clone());
            let exe = if path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("lnk"))
                .unwrap_or(false)
            {
                std::fs::read(&path)
                    .ok()
                    .and_then(|bytes| lnk_target_path(&bytes))
            } else {
                Some(path.to_string_lossy().into_owned())
            };
            if is_system_item(&name, exe.as_deref()) {
                continue;
            }
            // 禁用判定：扩展名为 `.disabled`（改名后不执行），或
            // 任务管理器在 StartupApproved\StartupFolder 中标记为禁用
            let enabled = !is_disabled && !disabled_names.contains(&fname);
            out.push(StartupItem {
                name,
                command: String::new(),
                exe_path: exe.unwrap_or_default(),
                source,
                requires_admin: admin,
                enabled,
                icon: None,
                reg_name: Some(fname),
                expand_sz: false,
            });
        }
    }

    /// 启动文件夹路径（按来源区分用户级 / 公共级）
    fn startup_folder(source: StartupSource) -> Result<PathBuf, String> {
        let base = match source {
            StartupSource::FolderUser => {
                std::env::var("APPDATA").map_err(|_| "无法获取 %APPDATA%".to_string())?
            }
            StartupSource::FolderMachine => std::env::var("ProgramData")
                .or_else(|_| std::env::var("ALLUSERSPROFILE"))
                .map_err(|_| "无法获取 %ProgramData%".to_string())?,
            _ => return Err("非文件夹来源".to_string()),
        };
        Ok(PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs\Startup"))
    }

    /// 读取 Explorer\StartupApproved 下被标记为"禁用"的值名集合。
    ///
    /// 任务管理器/设置禁用启动项的机制：Run 值与启动文件夹文件保留原名，
    /// 在 `StartupApproved\{Run,Run32,StartupFolder}` 写入同名 REG_BINARY 值，
    /// 12 字节中首字节为 0x03（奇数）表示禁用、0x02 表示启用、缺失默认启用。
    /// 系统登录时据此跳过对应项——这是 Windows 唯一识别的禁用标记。
    fn startup_approved_disabled() -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            for sub in ["Run", "Run32", "StartupFolder"] {
                let path = format!(
                    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{sub}"
                );
                let Ok(key) = RegKey::predef(root).open_subkey(&path) else {
                    continue;
                };
                for entry in key.enum_values().flatten() {
                    let (name, value) = entry;
                    if value.bytes.first().map(|b| b & 1 == 1).unwrap_or(false) {
                        out.insert(name);
                    }
                }
            }
        }
        out
    }

    /// 删除 StartupApproved 中的同名禁用标记（幂等）。
    ///
    /// 启用恢复后若残留标记，系统仍会跳过该值导致恢复失效；禁用后清理
    /// 可避免标记与新的规范值名失配造成状态混乱。
    fn remove_startup_approved_marker(name: &str) {
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            for sub in ["Run", "Run32", "StartupFolder"] {
                let path = format!(
                    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{sub}"
                );
                if let Ok(key) = RegKey::predef(root).open_subkey_with_flags(&path, KEY_SET_VALUE) {
                    let _ = key.delete_value(name);
                }
            }
        }
    }

    /* ─── 应用微缩图标提取（exe → ICO bytes → data URL） ─── */

    /// 提取可执行文件的小图标，返回 ICO data URL
    fn extract_icon_data_url(exe_path: &str) -> Option<String> {
        let ico = extract_icon_ico(exe_path)?;
        Some(format!("data:image/x-icon;base64,{}", base64_encode(&ico)))
    }

    /// 用 SHGetFileInfo + GetDIBits 提取 exe 的 16x16 图标并组装为 ICO 字节
    fn extract_icon_ico(exe_path: &str) -> Option<Vec<u8>> {
        use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
        use windows::Win32::UI::Shell::SHFILEINFOW;
        use windows::Win32::UI::Shell::SHGetFileInfoW;
        use windows::Win32::UI::Shell::{SHGFI_ICON, SHGFI_SMALLICON, SHGFI_USEFILEATTRIBUTES};
        use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
        use windows::core::PCWSTR;

        let wide: Vec<u16> = exe_path.encode_utf16().collect();
        let mut sfi = SHFILEINFOW::default();
        let ret = unsafe {
            SHGetFileInfoW(
                PCWSTR(wide.as_ptr()),
                FILE_ATTRIBUTE_NORMAL,
                Some(&mut sfi as *mut SHFILEINFOW),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES,
            )
        };
        if ret == 0 || sfi.hIcon.0 == 0 {
            return None;
        }
        let hicon = sfi.hIcon;
        let result = icon_to_ico(hicon);
        unsafe {
            let _ = DestroyIcon(hicon);
        }
        result
    }

    /// 从 HICON 提取 32bpp BGRA 像素并组装 ICO 文件字节
    fn icon_to_ico(hicon: HICON) -> Option<Vec<u8>> {
        use windows::Win32::Graphics::Gdi::{
            BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS,
            DeleteDC, GetDIBits, GetObjectW, SelectObject,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

        let mut info = ICONINFO::default();
        if unsafe { GetIconInfo(hicon, &mut info) }.is_err() {
            return None;
        }
        // 仅支持 32bpp 彩色位图（mono mask 方案直接放弃）
        if info.hbmColor.0 == 0 {
            return None;
        }
        let hbm = info.hbmColor;

        let mut bmp = BITMAP::default();
        let got = unsafe {
            GetObjectW(
                hbm,
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut BITMAP as *mut core::ffi::c_void),
            )
        };
        if got == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
            return None;
        }
        let (w, h) = (bmp.bmWidth as u32, bmp.bmHeight as u32);

        let dc = unsafe { CreateCompatibleDC(windows::Win32::Graphics::Gdi::HDC::default()) };
        if dc.is_invalid() {
            return None;
        }
        let old = unsafe { SelectObject(dc, windows::Win32::Graphics::Gdi::HGDIOBJ(hbm.0)) };
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32), // 自顶向下，输出行序即显示顺序
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: w * h * 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let lines = unsafe {
            GetDIBits(
                dc,
                hbm,
                0,
                h,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };
        unsafe {
            let _ = SelectObject(dc, old);
            let _ = DeleteDC(dc);
        }
        if lines == 0 {
            return None;
        }
        Some(super::ico_from_bgra(w, h, &buf))
    }
}

/* ═══════════ 单元测试 ═══════════ */

#[cfg(test)]
mod tests {
    use super::*;

    /// 修复回归：list → 前端 → invoke 回传链路。可选字段在序列化时被省略
    /// （skip_serializing_if），反序列化必须容忍缺失，否则关闭启动项时报
    /// `invalid args 'item' for command 'disable_startup_item'`。
    #[test]
    fn startup_item_serde_roundtrip_missing_optional() {
        let src = StartupItem {
            name: "WeChat".into(),
            command: r#""C:\Program Files\Tencent\WeChat\WeChat.exe""#.into(),
            exe_path: r"C:\Program Files\Tencent\WeChat\WeChat.exe".into(),
            source: StartupSource::RegistryUser,
            requires_admin: false,
            enabled: true,
            icon: Some("data:image/x-icon;base64,AAAA".into()),
            reg_name: None,
            expand_sz: false,
        };
        // 序列化契约：reg_name/expand_sz 必须被省略（前端拿到的是缺字段的对象）
        let json = serde_json::to_string(&src).unwrap();
        assert!(!json.contains("reg_name"), "reg_name=None 应被省略: {json}");
        assert!(!json.contains("expand_sz"), "expand_sz=false 应被省略: {json}");
        // 缺字段反序列化必须成功（修复 invalid args）
        let back: StartupItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "WeChat");
        assert_eq!(back.reg_name, None);
        assert!(!back.expand_sz);
        assert_eq!(back.source, StartupSource::RegistryUser);
    }

    /// 含全部字段时反序列化正常（reg_name / expand_sz 有值场景）
    #[test]
    fn startup_item_serde_roundtrip_full_fields() {
        let src = StartupItem {
            name: "Feishu".into(),
            command: r#""C:\Apps\Feishu\Feishu.exe""#.into(),
            exe_path: r"C:\Apps\Feishu\Feishu.exe".into(),
            source: StartupSource::RegistryMachine,
            requires_admin: true,
            enabled: false,
            icon: None,
            reg_name: Some("Feishu_disabled".into()),
            expand_sz: true,
        };
        let json = serde_json::to_string(&src).unwrap();
        assert!(json.contains("reg_name"));
        assert!(json.contains("expand_sz"));
        let back: StartupItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back.reg_name.as_deref(), Some("Feishu_disabled"));
        assert!(back.expand_sz);
        assert_eq!(back.source, StartupSource::RegistryMachine);
    }

    #[test]
    fn parse_quoted_command() {
        assert_eq!(
            parse_command_exe(r#""C:\Program Files\App\app.exe" --minimized"#).as_deref(),
            Some(r"C:\Program Files\App\app.exe")
        );
    }

    #[test]
    fn parse_unquoted_with_args() {
        assert_eq!(
            parse_command_exe(r"C:\Tools\thing.exe /silent").as_deref(),
            Some(r"C:\Tools\thing.exe")
        );
    }

    #[test]
    fn parse_quoted_path_without_extension() {
        // 含空格路径通常带引号，即使无扩展名也返回引号内整体
        assert_eq!(
            parse_command_exe(r#""C:\Program Files\App\launcher" --flag"#).as_deref(),
            Some(r"C:\Program Files\App\launcher")
        );
    }

    #[test]
    fn parse_empty_returns_none() {
        assert_eq!(parse_command_exe("   "), None);
        assert_eq!(parse_command_exe(""), None);
    }

    #[test]
    fn expand_env_replaces_known_vars() {
        temp_env::with_var("SystemRoot", Some(r"C:\Windows"), || {
            let out = expand_env(r#""%SystemRoot%\System32\svchost.exe" -k"#);
            assert!(!out.contains("%SystemRoot%"));
            assert!(out.starts_with(r#""C:\Windows\System32"#));
        });
        // 大小写不敏感：`%windir%` 等小写变体同样展开
        temp_env::with_var("WINDIR", Some(r"C:\Windows"), || {
            let out = expand_env(r"%windir%\system32\SecurityHealthSystray.exe");
            assert_eq!(out, r"C:\Windows\system32\SecurityHealthSystray.exe");
        });
    }

    #[test]
    fn system_item_by_name() {
        assert!(is_system_item("MicrosoftEdgeAutoLaunch", None));
        assert!(is_system_item(
            "WindowsUpdate",
            Some(r"C:\Windows\system32\wuauclt.exe")
        ));
        assert!(is_system_item(
            "OneDrive",
            Some(r"C:\Users\me\AppData\Local\OneDrive\OneDrive.exe")
        ));
    }

    #[test]
    fn system_item_by_path() {
        assert!(is_system_item("Foo", Some(r"C:\Windows\System32\foo.exe")));
        assert!(is_system_item(
            "Foo",
            Some(r"C:\Program Files\Microsoft\Edge\msedge.exe")
        ));
        assert!(is_system_item(
            "Foo",
            Some(r"C:\Program Files\WindowsApps\App\app.exe")
        ));
    }

    #[test]
    fn third_party_item_kept() {
        assert!(!is_system_item(
            "WeChat",
            Some(r"D:\Tencent\WeChat\WeChat.exe")
        ));
        assert!(!is_system_item(
            "Steam",
            Some(r"C:\Program Files (x86)\Steam\steam.exe")
        ));
    }

    #[test]
    fn lnk_path_extraction() {
        // 构造 UTF-16LE 的 "C:\Tools\app.exe" + 结尾空字符
        let mut bytes = Vec::new();
        for ch in r"C:\Tools\app.exe".encode_utf16() {
            bytes.extend_from_slice(&ch.to_le_bytes());
        }
        bytes.extend_from_slice(&[0, 0]);
        // 前面混入垃圾字节
        let mut data = vec![0xAB, 0xCD, 1, 2, 3, 4];
        data.extend_from_slice(&bytes);
        assert_eq!(lnk_target_path(&data).as_deref(), Some(r"C:\Tools\app.exe"));
    }

    #[test]
    fn restore_reg_name_strips_legacy_suffix() {
        assert_eq!(restore_reg_name("WeChat"), "WeChat");
        assert_eq!(restore_reg_name("WeChat_disabled"), "WeChat");
        // 双重残留后缀循环去掉
        assert_eq!(
            restore_reg_name("AcAppDaemon_disabled_disabled"),
            "AcAppDaemon"
        );
        assert_eq!(restore_reg_name("_disabled"), "_disabled");
    }

    #[test]
    fn same_reg_app_matches_all_variants() {
        assert!(is_same_reg_app("Feishu", "Feishu"));
        assert!(is_same_reg_app("Feishu_disabled", "Feishu"));
        assert!(is_same_reg_app("DeskGo_disabled", "DeskGo"));
        assert!(is_same_reg_app(
            "AcAppDaemon_disabled_disabled",
            "AcAppDaemon"
        ));
        assert!(!is_same_reg_app("OneDrive", "Feishu"));
        assert!(!is_same_reg_app("FeishuDaemon", "Feishu"));
    }

    #[test]
    fn folder_file_name_roundtrip() {
        // 新形态：`.disabled` 作为扩展名（系统不执行）
        assert_eq!(disable_file_name("WeChat.lnk"), "WeChat.lnk.disabled");
        assert_eq!(disable_file_name("App.exe"), "App.exe.disabled");
        assert_eq!(disable_file_name("NoExt"), "NoExt.disabled");
        assert_eq!(enable_file_name("WeChat.lnk.disabled"), "WeChat.lnk");
        assert_eq!(enable_file_name("App.exe.disabled"), "App.exe");
        assert_eq!(enable_file_name("NoExt.disabled"), "NoExt");
        assert!(is_disabled_file("WeChat.lnk.disabled"));
        assert!(is_disabled_file("NoExt.disabled"));
        assert!(!is_disabled_file("WeChat.lnk"));
        // 旧工具中间改名形态（扩展名仍是 .lnk）：系统照常执行，视为启用
        assert!(!is_disabled_file("WeChat.disabled.lnk"));
        // 兼容旧形态的恢复（显示名）
        assert_eq!(enable_file_name("WeChat.disabled.lnk"), "WeChat.lnk");
    }

    #[test]
    fn dedup_keeps_first() {
        let mut items = vec![
            StartupItem {
                name: "AppA".into(),
                command: "cmd1".into(),
                exe_path: "".into(),
                source: StartupSource::RegistryMachine,
                requires_admin: true,
                enabled: true,
                icon: None,
                reg_name: None,
                expand_sz: false,
            },
            StartupItem {
                name: "AppA".into(),
                command: "cmd2".into(),
                exe_path: "".into(),
                source: StartupSource::RegistryMachine,
                requires_admin: true,
                enabled: true,
                icon: None,
                reg_name: None,
                expand_sz: false,
            },
            StartupItem {
                name: "AppB".into(),
                command: "".into(),
                exe_path: "".into(),
                source: StartupSource::FolderUser,
                requires_admin: false,
                enabled: true,
                icon: None,
                reg_name: None,
                expand_sz: false,
            },
        ];
        dedup_items(&mut items);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].command, "cmd1");
    }

    #[test]
    fn dedup_enabled_wins_when_any_variant_enabled() {
        // `Feishu`（StartupApproved 禁用）与 `Feishu_disabled`（无标记，系统执行）
        // 并存时，合并结果显示启用——与系统真实行为一致
        let mut items = vec![
            StartupItem {
                name: "Feishu".into(),
                command: "cmd1".into(),
                exe_path: "".into(),
                source: StartupSource::RegistryUser,
                requires_admin: false,
                enabled: false,
                icon: None,
                reg_name: Some("Feishu".into()),
                expand_sz: false,
            },
            StartupItem {
                name: "Feishu".into(),
                command: "cmd2".into(),
                exe_path: "".into(),
                source: StartupSource::RegistryUser,
                requires_admin: false,
                enabled: true,
                icon: None,
                reg_name: Some("Feishu_disabled".into()),
                expand_sz: false,
            },
        ];
        dedup_items(&mut items);
        assert_eq!(items.len(), 1);
        assert!(items[0].enabled);
        assert_eq!(items[0].reg_name.as_deref(), Some("Feishu"));
    }

    #[test]
    fn base64_roundtrip() {
        let cases = [
            b"".as_slice(),
            b"a",
            b"ab",
            b"abc",
            b"hello world",
            "开机自启动项测试".as_bytes(),
        ];
        for c in cases {
            let encoded = base64_encode(c);
            assert_eq!(base64_decode(&encoded).as_deref(), Some(c), "case: {c:?}");
        }
    }

    #[test]
    fn base64_rejects_invalid() {
        assert_eq!(base64_decode("!!!"), None);
        assert_eq!(base64_decode("QQ"), Some(b"A".to_vec()));
    }

    #[test]
    fn ico_layout_has_valid_header() {
        // 4x4 BGRA → ICO：头部 + entry + DIB + mask
        let ico = ico_from_bgra(4, 4, &[0u8; 4 * 4 * 4]);
        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 1, 0]);
        assert_eq!(ico[6], 4); // width
        assert_eq!(ico[7], 4); // height
        assert_eq!(u16::from_le_bytes([ico[10], ico[11]]), 1); // planes
        assert_eq!(u16::from_le_bytes([ico[12], ico[13]]), 32); // bpp
        let data_len = u32::from_le_bytes([ico[14], ico[15], ico[16], ico[17]]);
        assert_eq!(data_len as usize, ico.len() - 22);
    }
}
