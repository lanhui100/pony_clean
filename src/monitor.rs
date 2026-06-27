use std::fmt;
use std::sync::mpsc;
use std::time::Duration;
use sysinfo::Pid;
use sysinfo::System;
use tokio::sync::oneshot;

/// 进程信息快照
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_mb: f64,
    pub status: String,
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
    procs.sort_by(|a, b| match (a.cpu.is_nan(), b.cpu.is_nan()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => b
            .cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal),
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

/// 启动进程监控任务（独立后台线程）
///
/// # CPU 首次采样说明
/// sysinfo 的 cpu_usage() 是两次 refresh 间的平均值，首轮始终为 0。
/// start() 内部会先做哑刷新，等待一个间隔后才开始正式轮询。
///
/// 使用 std::thread 而非 tokio::spawn，避免阻塞 tokio 工作线程。
/// UI 侧通过 std::sync::mpsc::Sender::try_send() 非阻塞发送命令。
#[must_use]
pub fn start(
    tx: mpsc::Sender<Snapshot>,
) -> (mpsc::Sender<MonitorCommand>, std::thread::JoinHandle<()>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<MonitorCommand>();

    let handle = std::thread::spawn(move || {
        let mut system = System::new();
        let mut first_run = true;

        loop {
            // 500ms 子间隔轮询，避免 Shutdown 响应延迟过长
            for _ in 0..10 {
                match cmd_rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(cmd) => {
                        if handle_command(Some(cmd), &system) {
                            return;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }

            system.refresh_cpu();
            system.refresh_processes();
            system.refresh_memory();

            // 首次 refresh 的 CPU 值为 0，跳过
            if first_run {
                first_run = false;
                continue;
            }

            // 处理 NaN：sysinfo 在进程退出等边界情况可能返回 NaN
            let cpu_total = system.global_cpu_info().cpu_usage();
            let cpu_total = if cpu_total.is_nan() { 0.0 } else { cpu_total };

            let summary = SystemSummary {
                cpu_total,
                mem_used_mb: system.used_memory() as f64 / (1024.0 * 1024.0),
                mem_total_mb: system.total_memory() as f64 / (1024.0 * 1024.0),
                process_count: system.processes().len(),
            };

            let count = system.processes().len();
            let mut processes = Vec::with_capacity(count);
            for (&pid, process) in system.processes().iter() {
                processes.push(ProcessInfo {
                    pid: pid.as_u32(),
                    name: process.name().to_string(),
                    cpu: if process.cpu_usage().is_nan() {
                        0.0
                    } else {
                        process.cpu_usage()
                    },
                    mem_mb: process.memory() as f64 / (1024.0 * 1024.0),
                    status: format!("{:?}", process.status()),
                });
            }

            sort_processes(&mut processes);
            let _ = tx.send(Snapshot { summary, processes });
        }
    });

    (cmd_tx, handle)
}

/// 处理命令，返回 true 表示应当退出
fn handle_command(cmd: Option<MonitorCommand>, system: &sysinfo::System) -> bool {
    match cmd {
        Some(MonitorCommand::Kill { pid, name, resp }) => {
            let result = kill_process(system, pid, &name);
            if let Err(e) = resp.send(result) {
                tracing::warn!("Failed to send kill result: receiver dropped ({:?})", e);
            }
            false
        }
        Some(MonitorCommand::Shutdown) | None => true,
    }
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
    fn test_kill_process_error_format() {
        let system = sysinfo::System::new();
        let result = kill_process(&system, 999_999, "test");
        let err = result.unwrap_err();
        assert!(err.contains("999999"), "error should mention PID");
        assert!(
            err.contains("not found"),
            "error should mention 'not found'"
        );
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
            },
            ProcessInfo {
                pid: 1,
                name: "low".into(),
                cpu: 10.0,
                mem_mb: 100.0,
                status: "Running".into(),
            },
            ProcessInfo {
                pid: 2,
                name: "high".into(),
                cpu: 90.0,
                mem_mb: 200.0,
                status: "Running".into(),
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
            },
            ProcessInfo {
                pid: 1,
                name: "a".into(),
                cpu: 50.0,
                mem_mb: 100.0,
                status: "Running".into(),
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
        };
        let s = p.to_string();
        assert!(s.contains("1234"));
        assert!(s.contains("test.exe"));
        assert!(s.contains("45.5%"));
        assert!(s.contains("256.0MB"));
        assert!(s.contains("Running"));
    }
}
