use pony_core::startup::{StartupAction, StartupItem};

/// 枚举第三方开机自启动项（含已关闭项）
#[tauri::command]
pub fn list_startup_items() -> Vec<StartupItem> {
    pony_core::startup::list_startup_items()
}

/// 关闭一个开机自启动项（禁用但保留，可重新打开）。
///
/// 系统级项（HKLM / 公共启动文件夹）在当前进程无管理员权限时，自动以
/// 管理员权限启动隐藏子进程执行（触发 UAC 确认），无需用户手动提权。
#[tauri::command]
pub fn disable_startup_item(item: StartupItem) -> Result<(), String> {
    match pony_core::startup::disable_startup_item(&item) {
        Ok(()) => Ok(()),
        // 需要管理员权限的项：自动提权子进程执行
        Err(_) if item.requires_admin => {
            pony_core::startup::startup_item_elevated(StartupAction::Disable, &item)
        }
        Err(e) => Err(e),
    }
}

/// 重新打开一个已关闭的开机自启动项。
///
/// 系统级项在当前进程无管理员权限时，自动提权子进程执行（触发 UAC 确认）。
#[tauri::command]
pub fn enable_startup_item(item: StartupItem) -> Result<(), String> {
    match pony_core::startup::enable_startup_item(&item) {
        Ok(()) => Ok(()),
        Err(_) if item.requires_admin => {
            pony_core::startup::startup_item_elevated(StartupAction::Enable, &item)
        }
        Err(e) => Err(e),
    }
}
