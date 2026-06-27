# TASK-002 进程监控模块

## Basic Info
- ID: TASK-002
- 状态: Backlog
- 优先级: P0
- 负责人: @self
- 创建日期: 2026-06-27
- 更新日期: 2026-06-27
- 预估工时: 5h
- 依赖: TASK-001

## Goal
实现进程监控核心模块：每 2s 轮询所有进程的 CPU 和内存占用，检测异常超高进程，支持通过 UI 发送的指令 kill 进程。所有数据通过非阻塞 channel 推送到 UI 侧。

## Output
- `src/monitor.rs` — 完整实现
- `src/bin/monitor_probe.rs` — 独立验证入口（CLI 打印快照，Ctrl+C 退出）

## 验收标准
1. 每 2s 输出一份进程快照，按 CPU 降序排列，包含 pid / name / cpu% / mem_mb / status / cmdline
2. CPU > 80% 或 内存 > 500MB 的进程在快照中保留原始数据，由 UI 侧计算高亮标记
3. 支持按 pid + name 双重校验 kill，通过 oneshot channel 返回 kill 结果
4. 数据通过 `std::sync::mpsc::Sender` 发送（UI 侧用 `try_recv()` 非阻塞读取）
5. 监控任务可被取消（drop cmd_rx 时自动退出循环）
6. 首次采样的 CPU 值在注释中说明为 0，跳过第一轮阈值判断
7. 附带系统级聚合指标（总 CPU / 总内存 / 总进程数），与进程列表一同发送

## 接口设计

```rust
// monitor.rs

use std::sync::mpsc;
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, System};
use tokio::sync::oneshot;

/// 进程信息快照
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,                  // 平台无关：Windows u32, Linux i32 → 统一为 u32
    pub name: String,
    pub cpu: f32,                  // 自上次 refresh 的平均占用（首轮为 0）
    pub mem_mb: f64,               // 内存 MB
    pub status: String,            // "Running" / "Sleeping" 等
    pub cmdline: String,           // 完整命令行，用于区分同名进程
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
    /// 带进程名校验的 kill：pid + name 都匹配才执行，结果通过 oneshot 返回
    Kill {
        pid: u32,
        name: String,
        resp: oneshot::Sender<Result<(), String>>,
    },
    Shutdown,
}

/// 启动进程监控任务
///
/// System 在 tokio::spawn 内部创建，避免在 GUI 线程阻塞。
/// 返回 cmd_tx，用于向后台发送命令。
///
/// # CPU 首次采样说明
/// sysinfo 的 cpu_usage() 是两次 refresh 间的平均值，首轮始终为 0。
/// start() 内部会先做一次哑刷新跳过首个 tick。
pub fn start(tx: mpsc::Sender<Snapshot>) -> mpsc::Sender<MonitorCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<MonitorCommand>();

    tokio::spawn(async move {
        let mut system = System::new();

        // 哑刷新：丢弃首次 CPU = 0 的数据
        system.refresh_processes(ProcessRefreshKind::new().with_cpu().with_memory());

        // 等待第一个间隔，让 sysinfo 采集到真实 CPU 差值
        tokio::time::sleep(Duration::from_secs(2)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    system.refresh_processes(
                        ProcessRefreshKind::new().with_cpu().with_memory()
                    );

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
                            name: process.name().unwrap_or("?").to_string(),
                            cpu: process.cpu_usage(),
                            mem_mb: process.memory() as f64 / (1024.0 * 1024.0),
                            status: format!("{:?}", process.status()),
                            cmdline: process.cmd().join(" "),
                        })
                        .collect();

                    // 按 CPU 降序排列，UI 侧直接使用
                    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));

                    let _ = tx.send(Snapshot { summary, processes });
                }
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(MonitorCommand::Kill { pid, name, resp }) => {
                            let result = kill_process(&mut system, pid, &name);
                            let _ = resp.send(result);
                        }
                        Some(MonitorCommand::Shutdown) | None => break,
                    }
                }
            }
        }
    });

    cmd_tx
}

/// 按 pid + name 双重校验后 kill 进程
fn kill_process(system: &mut System, pid: u32, expected_name: &str) -> Result<(), String> {
    let pid = Pid::from_u32(pid);
    match system.process(pid) {
        None => Err(format!("Process {pid} not found")),
        Some(process) => {
            let actual_name = process.name().unwrap_or("?");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kill_process_not_found() {
        let mut system = System::new();
        // 不存在的 PID 应返回错误，不 panic
        let result = kill_process(&mut system, 999_999, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_cpu_descending_sort() {
        let mut processes = vec![
            ProcessInfo { pid: 1, name: "low".into(), cpu: 10.0, mem_mb: 100.0, status: "Running".into(), cmdline: "".into() },
            ProcessInfo { pid: 2, name: "high".into(), cpu: 90.0, mem_mb: 200.0, status: "Running".into(), cmdline: "".into() },
        ];
        processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(processes[0].name, "high");
        assert_eq!(processes[1].name, "low");
    }
}
```

## 阈值常量（UI 侧使用）
- CPU 超高阈值: 80.0 (%)
- 内存超高阈值: 500.0 (MB)
- 轮询间隔: 2s

## 测试策略
- **纯函数测试**（可测）: 排序逻辑、Kill 命令路由、错误处理分支
- **不测**（系统调用）: sysinfo 本身的进程遍历正确性、Windows kill 权限
- **验证入口**: `src/bin/monitor_probe.rs` — 导入 monitor::start，println 打印快照，Ctrl+C 退出

## 与 TASK-004 的接口协定
- `monitor::start()` 返回 `mpsc::Sender<MonitorCommand>` 给 UI 层
- UI 层持有 `std::sync::mpsc::Receiver<Snapshot>`
- UI 在每帧调用 `rx.try_recv()` 非阻塞获取最新快照
- `is_high` 标记由 UI 侧根据阈值计算，后台只推送原始数据

## Current Progress
- 尚未开始

## Next Action
等待 TASK-001 完成后，在 `src/monitor.rs` 中实现上述接口，并创建 `src/bin/monitor_probe.rs` 验证入口。

## Resume Hint
打开 `src/monitor.rs`。先实现数据结构和常量，然后实现 `kill_process()` 纯函数，接着实现 `start()` 异步任务。注意在 tokio::spawn 内部创建 System，使用 `ProcessRefreshKind` 而非 `refresh_all()`。完成后创建 monitor_probe.rs 手动验证。
