//! 内存整理：对非关键进程调用 `EmptyWorkingSet` 释放工作集。
//!
//! `EmptyWorkingSet` 是 Windows 官方 API，将进程工作集裁剪到系统最小值，
//! 释放物理内存供其他进程使用，不结束任何进程、无数据丢失风险。

use serde::Serialize;
use sysinfo::{Pid, System};

/// 内存整理结果
#[derive(Clone, Debug, Default, Serialize)]
pub struct TrimResult {
    pub attempted: usize,
    pub success: usize,
    pub failed: usize,
    pub skipped: usize,
    /// 整理前后系统物理内存使用差值（MB，负值钳制为 0）
    pub freed_mb: f64,
}

/// 系统关键进程 PID（System Idle 0 / System 4 / Registry 8 等），跳过整理
fn is_critical_pid(pid: u32) -> bool {
    pid <= 8
}

/// 判断进程是否应跳过（系统关键进程 / 当前进程 / 会话 0 系统服务进程）
fn should_skip(system: &System, pid: u32) -> bool {
    if is_critical_pid(pid) || pid == std::process::id() {
        return true;
    }
    // 会话 0 为系统服务会话，整理无收益且大概率无权限
    system
        .process(Pid::from_u32(pid))
        .and_then(|p| p.session_id())
        .is_some_and(|sid| sid.as_u32() == 0)
}

/// 对单个进程调用 `EmptyWorkingSet` 释放工作集
#[cfg(target_os = "windows")]
fn trim_process(pid: u32) -> Result<(), String> {
    use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA,
    };

    let access = PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA;
    let handle = unsafe { OpenProcess(access, false, pid) };
    if handle.is_err() {
        return Err(format!("无法打开进程 {pid}"));
    }
    let handle = handle.unwrap();
    let result = unsafe { EmptyWorkingSet(handle) };
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
    result.map_err(|e| format!("EmptyWorkingSet 失败 {pid}: {e}"))
}

#[cfg(not(target_os = "windows"))]
fn trim_process(pid: u32) -> Result<(), String> {
    let _ = pid;
    Err("内存整理仅支持 Windows".into())
}

/// 整理所有非关键进程的工作集，返回整理统计
pub fn trim_all(system: &mut System) -> TrimResult {
    let before = system.used_memory();
    let mut result = TrimResult::default();

    let pids: Vec<u32> = system.processes().keys().map(|p| p.as_u32()).collect();
    for pid in pids {
        if should_skip(system, pid) {
            result.skipped += 1;
            continue;
        }
        result.attempted += 1;
        match trim_process(pid) {
            Ok(()) => result.success += 1,
            Err(_) => result.failed += 1,
        }
    }

    system.refresh_memory();
    let after = system.used_memory();
    result.freed_mb = before.saturating_sub(after) as f64 / (1024.0 * 1024.0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical_pid_skip() {
        assert!(is_critical_pid(0));
        assert!(is_critical_pid(4));
        assert!(is_critical_pid(8));
        assert!(!is_critical_pid(9));
        assert!(!is_critical_pid(1234));
    }

    #[test]
    fn test_should_skip_self() {
        let system = sysinfo::System::new();
        assert!(should_skip(&system, std::process::id()));
    }

    #[test]
    fn test_should_skip_missing_pid() {
        // 不存在的 PID 不应被跳过（会走 OpenProcess 失败路径）
        let system = sysinfo::System::new();
        assert!(!should_skip(&system, 999_999));
    }
}
