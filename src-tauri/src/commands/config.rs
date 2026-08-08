use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 应用配置（持久化到 app_config_dir/config.json）
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// CPU 告警阈值（百分比）
    pub alert_cpu_pct: u8,
    /// 内存告警阈值（百分比）
    pub alert_mem_pct: u8,
    /// 开机自启
    pub autostart: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            alert_cpu_pct: 80,
            alert_mem_pct: 85,
            autostart: false,
        }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {e}"))?;
    Ok(dir.join("config.json"))
}

/// 读取配置（不存在时返回默认值）
#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    let path = config_path(&app)?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let data = std::fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("解析配置失败: {e}"))
}

/// 保存配置并同步开机自启状态
#[tauri::command]
pub fn set_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    let path = dir.join("config.json");
    let data = serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {e}"))?;
    std::fs::write(&path, data).map_err(|e| format!("写入配置失败: {e}"))?;

    set_autostart(config.autostart)
}

/// 设置开机自启（HKCU Run 键）
fn set_autostart(enabled: bool) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| format!("打开 Run 键失败: {e}"))?;

    if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;
        let cmd = format!("\"{}\"", exe.to_string_lossy());
        key.set_value("PonyClean", &cmd)
            .map_err(|e| format!("写入自启失败: {e}"))?;
    } else {
        match key.delete_value("PonyClean") {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("删除自启失败: {e}")),
        }
    }
    Ok(())
}
