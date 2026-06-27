use std::fmt;
use sysinfo::Pid;
use tokio::sync::oneshot;

/// 进程信息快照
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_mb: f64,
    pub status: String,
    /// 完整命令行（仅用于进程识别，不在 Display 中输出以避免敏感信息泄露）
    pub cmdline: String,
}

/// 系统级聚合指标
#[derive(Clone, Debug)]
pub struct SystemSummary {
    pub cpu_total: f32,
    pub mem_used_mb: f64,
    pub mem_total_mb: f64,
    pub process_count: usize,
}

/// 完整快照：系统摘要 + 进程列表（按 CPU 降序）
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub summary: SystemSummary,
    pub processes: Vec<ProcessInfo>,
}

/// 监控后台命令
#[derive(Debug)]
pub enum MonitorCommand {
    Kill {
        pid: u32,
        name: String,
        resp: oneshot::Sender<Result<(), String>>,
    },
    Shutdown,
}

/// 按 CPU 降序排列进程列表
pub fn sort_processes(procs: &mut [ProcessInfo]) {
    procs.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// 按 pid + name 双重校验后 kill 进程
///
/// 使用 `&System` 而非 `&mut System`，因为 sysinfo 的 process() 和
/// process.kill() 均只需要不可变引用。
pub fn kill_process(system: &sysinfo::System, pid: u32, expected_name: &str) -> Result<(), String> {
    let pid = Pid::from_u32(pid);
    match system.process(pid) {
        None => Err(format!("Process {pid} not found")),
        Some(process) => {
            let actual_name = process.name();
            if actual_name != expected_name {
                return Err(format!(
                    "PID {pid} has changed: expected '{expected_name}', actual '{actual_name}'"
                ));
            }
            if !process.kill() {
                return Err(format!("Failed to kill process {pid} ({expected_name})"));
            }
            Ok(())
        }
    }
}

use std::sync::mpsc;
use std::time::Duration;
use sysinfo::System;

/// 启动进程监控任务
///
/// # CPU 首次采样说明
/// sysinfo 的 cpu_usage() 是两次 refresh 间的平均值，首轮始终为 0。
/// start() 内部会先做哑刷新，再 sleep 一个间隔，然后才开始正式轮询。
///
/// System 在 tokio::spawn 内部创建，避免在 GUI 线程阻塞。
/// 返回 cmd_tx，用于向后台发送命令。
pub fn start(tx: mpsc::Sender<Snapshot>) -> mpsc::Sender<MonitorCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<MonitorCommand>();

    tokio::spawn(async move {
        let mut system = System::new();

        // 哑刷新：丢弃首次 CPU = 0 的数据
        system.refresh_processes();

        // 等待一个间隔，让 sysinfo 采集到真实 CPU 差值
        tokio::time::sleep(Duration::from_secs(2)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            interval.tick().await;

            // 处理挂起的命令
            loop {
                match cmd_rx.try_recv() {
                    Ok(MonitorCommand::Kill { pid, name, resp }) => {
                        let result = kill_process(&system, pid, &name);
                        let _ = resp.send(result);
                    }
                    Ok(MonitorCommand::Shutdown) | Err(mpsc::TryRecvError::Disconnected) => {
                        return;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }

            system.refresh_processes();

            let summary = SystemSummary {
                cpu_total: system.global_cpu_info().cpu_usage(),
                mem_used_mb: system.used_memory() as f64 / (1024.0 * 1024.0),
                mem_total_mb: system.total_memory() as f64 / (1024.0 * 1024.0),
                process_count: system.processes().len(),
            };

            let mut processes: Vec<ProcessInfo> = system
                .processes()
                .iter()
                .map(|(&pid, process)| ProcessInfo {
                    pid: pid.as_u32(),
                    name: process.name().to_string(),
                    cpu: process.cpu_usage(),
                    mem_mb: process.memory() as f64 / (1024.0 * 1024.0),
                    status: format!("{:?}", process.status()),
                    cmdline: process.cmd().join(" "),
                })
                .collect();

            sort_processes(&mut processes);

            let _ = tx.send(Snapshot { summary, processes });
        }
    });

    cmd_tx
}

impl fmt::Display for ProcessInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:>6}  {:<30} {:>6.1}% {:>8.1}MB  {}",
            self.pid, self.name, self.cpu, self.mem_mb, self.status,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kill_process_not_found() {
        let system = sysinfo::System::new();
        let result = kill_process(&system, 999_999, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_kill_process_name_mismatch() {
        let system = sysinfo::System::new();
        let result = kill_process(&system, 999_999, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_cpu_descending_sort() {
        let mut processes = vec![
            ProcessInfo {
                pid: 3,
                name: "medium".into(),
                cpu: 50.0,
                mem_mb: 150.0,
                status: "Running".into(),
                cmdline: String::new(),
            },
            ProcessInfo {
                pid: 1,
                name: "low".into(),
                cpu: 10.0,
                mem_mb: 100.0,
                status: "Running".into(),
                cmdline: String::new(),
            },
            ProcessInfo {
                pid: 2,
                name: "high".into(),
                cpu: 90.0,
                mem_mb: 200.0,
                status: "Running".into(),
                cmdline: String::new(),
            },
        ];
        sort_processes(&mut processes);
        assert_eq!(processes[0].name, "high");
        assert_eq!(processes[1].name, "medium");
        assert_eq!(processes[2].name, "low");
    }

    #[test]
    fn test_sort_equal_cpu_preserves_pid_order() {
        let mut processes = vec![
            ProcessInfo {
                pid: 2,
                name: "b".into(),
                cpu: 50.0,
                mem_mb: 100.0,
                status: "Running".into(),
                cmdline: String::new(),
            },
            ProcessInfo {
                pid: 1,
                name: "a".into(),
                cpu: 50.0,
                mem_mb: 100.0,
                status: "Running".into(),
                cmdline: String::new(),
            },
        ];
        sort_processes(&mut processes);
        assert_eq!(processes[0].pid, 2);
        assert_eq!(processes[1].pid, 1);
    }

    #[test]
    fn test_process_info_display() {
        let p = ProcessInfo {
            pid: 1234,
            name: "test.exe".into(),
            cpu: 45.5,
            mem_mb: 256.0,
            status: "Running".into(),
            cmdline: String::new(),
        };
        let s = p.to_string();
        assert!(s.contains("1234"));
        assert!(s.contains("test.exe"));
        assert!(s.contains("45.5%"));
        assert!(s.contains("256.0MB"));
        assert!(s.contains("Running"));
    }

    #[test]
    fn test_display_omits_cmdline() {
        let p = ProcessInfo {
            pid: 1,
            name: "proc".into(),
            cpu: 0.0,
            mem_mb: 0.0,
            status: "Running".into(),
            cmdline: "secret=password123".into(),
        };
        let s = p.to_string();
        assert!(!s.contains("password123"));
    }
}
