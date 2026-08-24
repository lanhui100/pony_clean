use serde::Serialize;
use std::fmt;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use std::time::Duration;
use sysinfo::Disks;
use sysinfo::Pid;
use sysinfo::System;
use tokio::sync::oneshot;

/// 进程信息快照
#[derive(Clone, Debug, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_mb: f64,
    pub status: String,
    /// 可执行文件路径（用于提取图标），可能为 None（系统进程等）
    pub exe_path: Option<String>,
}

/// 系统级聚合指标
#[derive(Clone, Debug, Serialize)]
pub struct SystemSummary {
    pub cpu_total: f32,
    pub mem_used_mb: f64,
    pub mem_total_mb: f64,
    pub process_count: usize,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    /// 网络下行速率（B/s，全部非回环接口合计）
    pub net_down_bps: f64,
    /// 网络上行速率（B/s）
    pub net_up_bps: f64,
    /// 公网连通质量；None = 尚未完成首次探测（前端显示「检测中」）
    pub net_quality: Option<crate::netmon::NetQuality>,
    /// 最优探测延迟（毫秒）；未就绪或全失败时为 None
    pub net_latency_ms: Option<u32>,
}

/// 完整快照：系统摘要 + 进程列表（按 CPU 降序）
#[derive(Clone, Debug, Serialize)]
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
    Trim {
        resp: oneshot::Sender<Result<crate::memory::TrimResult, String>>,
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

/// 启动网络探测线程并返回共享健康状态（SPEC-032）
///
/// 探测线程随进程生命周期运行（JoinHandle 立即分离，进程退出自动回收）。
/// 必须在监控循环**进入前**调用，保证首帧快照 `net_quality=None`。
fn spawn_probe() -> Arc<RwLock<Option<crate::netmon::NetHealth>>> {
    let state = Arc::new(RwLock::new(None));
    // JoinHandle 丢弃即分离：探测线程随进程生命周期运行，进程退出自动回收
    let _ = crate::netmon::start_probe(Arc::clone(&state));
    state
}

/// 读取网络健康状态：锁取值后立刻 drop，不与 snapshot 写锁嵌套持有；
/// 锁中毒时静默降级为「未就绪」（沿用项目 `if let Ok(guard)` 容错模式）
fn read_net_health(
    probe_state: &RwLock<Option<crate::netmon::NetHealth>>,
) -> (Option<crate::netmon::NetQuality>, Option<u32>) {
    match probe_state.read() {
        Ok(guard) => guard
            .as_ref()
            .map_or((None, None), |h| (Some(h.quality), h.latency_ms)),
        Err(_) => (None, None),
    }
}

/// 启动进程监控任务（独立后台线程）
///
/// # CPU 首次采样说明
/// sysinfo 的 cpu_usage() 是两次 refresh 间的平均值，首轮始终为 0。
/// start() 内部会先做哑刷新，等待一个间隔后才开始正式轮询。
///
/// # 网络探测线程生命周期（SPEC-032）
/// 每次调用会拉起一条随**进程**存活的网络探测线程（Shutdown 不停它）。
/// 请勿在长驻进程内反复调用本函数；Tauri 壳层应使用 [`start_shared`]。
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
        let mut disks = Disks::new();
        // 网络监控（SPEC-032）：探测线程 + 吞吐采样器均在进入循环前初始化，
        // 保证首帧 net_quality=None / bps=0.0
        let probe_state = spawn_probe();
        let mut net_sampler = crate::netmon::ThroughputSampler::new();

        loop {
            // 500ms 子间隔轮询，避免 Shutdown 响应延迟过长
            for _ in 0..10 {
                match cmd_rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(cmd) => {
                        if handle_command(Some(cmd), &mut system) {
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

            // 归一化：sysinfo 的 process.cpu_usage() 返回跨所有核心的累加值
            // （最高 num_cpus × 100%），除以核心数使其与 global_cpu 同为 0-100% 刻度
            let num_cpus = system.cpus().len().max(1) as f32;

            // 获取 C 盘信息
            disks.refresh_list();
            let (disk_used_gb, disk_total_gb) = disks
                .list()
                .iter()
                .find(|d| {
                    let mp = d.mount_point().to_string_lossy();
                    mp == "C:" || mp == "C:\\" || mp.starts_with("C:\\")
                })
                .map(|d| {
                    let total = d.total_space() as f64;
                    let available = d.available_space() as f64;
                    let used = if total > 0.0 { total - available } else { 0.0 };
                    (used / 1_073_741_824.0, total / 1_073_741_824.0)
                })
                .unwrap_or((0.0, 0.0));

            // 网络吞吐与健康状态（采样器内部含 500ms 间隔地板防噪声尖峰）
            let (net_down_bps, net_up_bps) = net_sampler.sample();
            let (net_quality, net_latency_ms) = read_net_health(&probe_state);

            let summary = SystemSummary {
                cpu_total,
                mem_used_mb: system.used_memory() as f64 / (1024.0 * 1024.0),
                mem_total_mb: system.total_memory() as f64 / (1024.0 * 1024.0),
                process_count: system.processes().len(),
                disk_used_gb,
                disk_total_gb,
                net_down_bps,
                net_up_bps,
                net_quality,
                net_latency_ms,
            };

            let count = system.processes().len();
            let mut processes = Vec::with_capacity(count);
            for (&pid, process) in system.processes().iter() {
                processes.push(ProcessInfo {
                    pid: pid.as_u32(),
                    name: process.name().to_string(),
                    cpu: (if process.cpu_usage().is_nan() {
                        0.0
                    } else {
                        process.cpu_usage()
                    }) / num_cpus,
                    mem_mb: process.memory() as f64 / (1024.0 * 1024.0),
                    status: format!("{:?}", process.status()),
                    exe_path: process.exe().map(|p| p.to_string_lossy().to_string()),
                });
            }

            sort_processes(&mut processes);
            let _ = tx.send(Snapshot { summary, processes });
        }
    });

    (cmd_tx, handle)
}

/// 处理命令，返回 true 表示应当退出
fn handle_command(cmd: Option<MonitorCommand>, system: &mut sysinfo::System) -> bool {
    match cmd {
        Some(MonitorCommand::Kill { pid, name, resp }) => {
            let result = kill_process(system, pid, &name);
            if let Err(e) = resp.send(result) {
                tracing::warn!("Failed to send kill result: receiver dropped ({:?})", e);
            }
            false
        }
        Some(MonitorCommand::Trim { resp }) => {
            let result = crate::memory::trim_all(system);
            if let Err(e) = resp.send(Ok(result)) {
                tracing::warn!("Failed to send trim result: receiver dropped ({:?})", e);
            }
            false
        }
        Some(MonitorCommand::Shutdown) | None => true,
    }
}

/// 以共享状态模式启动监控（Tauri 后端路径）
///
/// 每个监控循环周期把最新 [`Snapshot`] 写入 `snapshot`；网络健康状态由内部
/// 探测线程维护（随进程存活，见 [`start`] 的生命周期说明）。
pub fn start_shared(
    snapshot: Arc<RwLock<Option<Snapshot>>>,
) -> (mpsc::Sender<MonitorCommand>, thread::JoinHandle<()>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<MonitorCommand>();
    let handle = thread::spawn(move || {
        let mut system = System::new();
        let mut disks = Disks::new();
        // 网络监控（SPEC-032）：探测线程 + 吞吐采样器均在进入循环前初始化，
        // 保证首帧 net_quality=None / bps=0.0（本路径无 first_run 跳过）
        let probe_state = spawn_probe();
        let mut net_sampler = crate::netmon::ThroughputSampler::new();
        loop {
            match cmd_rx.recv_timeout(Duration::from_millis(2000)) {
                Ok(cmd) => {
                    if handle_command(Some(cmd), &mut system) {
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }

            system.refresh_cpu();
            system.refresh_processes();
            system.refresh_memory();

            let cpu_total = system.global_cpu_info().cpu_usage();
            let cpu_total = if cpu_total.is_nan() { 0.0 } else { cpu_total };
            let num_cpus = system.cpus().len().max(1) as f32;

            disks.refresh_list();
            let (disk_used_gb, disk_total_gb) = disks
                .list()
                .iter()
                .find(|d| {
                    let mp = d.mount_point().to_string_lossy();
                    mp == "C:" || mp == "C:\\" || mp.starts_with("C:\\")
                })
                .map(|d| {
                    let total = d.total_space() as f64;
                    let available = d.available_space() as f64;
                    let used = if total > 0.0 { total - available } else { 0.0 };
                    (used / 1_073_741_824.0, total / 1_073_741_824.0)
                })
                .unwrap_or((0.0, 0.0));

            // 网络吞吐与健康状态（采样器内部含 500ms 间隔地板防噪声尖峰）
            let (net_down_bps, net_up_bps) = net_sampler.sample();
            let (net_quality, net_latency_ms) = read_net_health(&probe_state);

            let summary = SystemSummary {
                cpu_total,
                mem_used_mb: system.used_memory() as f64 / (1024.0 * 1024.0),
                mem_total_mb: system.total_memory() as f64 / (1024.0 * 1024.0),
                process_count: system.processes().len(),
                disk_used_gb,
                disk_total_gb,
                net_down_bps,
                net_up_bps,
                net_quality,
                net_latency_ms,
            };
            let mut processes = Vec::with_capacity(system.processes().len());
            for (&pid, process) in system.processes().iter() {
                processes.push(ProcessInfo {
                    pid: pid.as_u32(),
                    name: process.name().to_string(),
                    cpu: (if process.cpu_usage().is_nan() {
                        0.0
                    } else {
                        process.cpu_usage()
                    }) / num_cpus,
                    mem_mb: process.memory() as f64 / (1024.0 * 1024.0),
                    status: format!("{:?}", process.status()),
                    exe_path: process.exe().map(|p| p.to_string_lossy().to_string()),
                });
            }
            sort_processes(&mut processes);
            if let Ok(mut guard) = snapshot.write() {
                *guard = Some(Snapshot { summary, processes });
            }
        }
    });
    (cmd_tx, handle)
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
    fn test_summary_net_fields_serialized() {
        // SPEC-032 契约：四个新键恒存在；未就绪时 net_quality/net_latency_ms 为字面 null
        let s = SystemSummary {
            cpu_total: 1.0,
            mem_used_mb: 1.0,
            mem_total_mb: 2.0,
            process_count: 1,
            disk_used_gb: 1.0,
            disk_total_gb: 2.0,
            net_down_bps: 0.0,
            net_up_bps: 0.0,
            net_quality: None,
            net_latency_ms: None,
        };
        let v = serde_json::to_value(&s).unwrap();
        for key in [
            "net_down_bps",
            "net_up_bps",
            "net_quality",
            "net_latency_ms",
        ] {
            assert!(v.get(key).is_some(), "missing key {key}");
        }
        assert_eq!(v["net_quality"], serde_json::Value::Null);
        assert_eq!(v["net_latency_ms"], serde_json::Value::Null);
    }

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
                exe_path: None,
            },
            ProcessInfo {
                pid: 1,
                name: "low".into(),
                cpu: 10.0,
                mem_mb: 100.0,
                status: "Running".into(),
                exe_path: None,
            },
            ProcessInfo {
                pid: 2,
                name: "high".into(),
                cpu: 90.0,
                mem_mb: 200.0,
                status: "Running".into(),
                exe_path: None,
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
                exe_path: None,
            },
            ProcessInfo {
                pid: 1,
                name: "a".into(),
                cpu: 50.0,
                mem_mb: 100.0,
                status: "Running".into(),
                exe_path: None,
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
            exe_path: None,
        };
        let s = p.to_string();
        assert!(s.contains("1234"));
        assert!(s.contains("test.exe"));
        assert!(s.contains("45.5%"));
        assert!(s.contains("256.0MB"));
        assert!(s.contains("Running"));
    }
}
