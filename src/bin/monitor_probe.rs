//! monitor_probe: 验证进程监控模块的独立入口
//!
//! 每 2s 打印一次进程快照，Ctrl+C 退出。

use std::sync::mpsc;
use pony_clean::monitor;

fn main() {
    tracing_subscriber::fmt::init();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    let _guard = rt.enter();

    let (tx, rx) = mpsc::channel::<monitor::Snapshot>();
    let _cmd_tx = monitor::start(tx);

    println!("PonyClean Monitor Probe — 每 2s 刷新一次，按 Ctrl+C 退出\n");

    for snapshot in rx {
        let summary = &snapshot.summary;
        println!(
            "CPU: {:>5.1}%  |  MEM: {:>8.1}MB / {:>8.1}MB  |  Processes: {}",
            summary.cpu_total, summary.mem_used_mb, summary.mem_total_mb, summary.process_count,
        );
        println!("{:-^80}", "");
        println!("{:>6}  {:<30} {:>6} {:>8}  {}", "PID", "NAME", "CPU%", "MEM", "STATUS");
        println!("{:-^80}", "");

        for p in &snapshot.processes {
            println!("{}", p);
        }
        println!();
    }
}
