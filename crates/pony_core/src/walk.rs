//! jwalk 目录遍历的专用 rayon 池。
//!
//! 背景（漏扫根因）：jwalk 默认 `Parallelism::RayonDefaultPool { busy_timeout: 1s }`
//! 共享进程级 rayon 全局池。cleaner 以 SCAN_PARALLELISM=4 线程并发起多个 WalkDir，
//! 大目标（如 cargo registry，单次可达数秒）占满全局池后，其他目标的根目录读取
//! 排队超过 1s 即被 jwalk 以 `ThreadpoolBusy` 中止——整个目标静默归零（depth 0），
//! 表现为"浏览器缓存只有 1.8MB"。
//!
//! 方案：全部 WalkDir 统一走本模块的专用池 + `busy_timeout: None`（不做超时中止）。
//! 遍历任务由 std::thread / tokio blocking 线程驱动，不在池内嵌套等待，
//! 无死锁前提，排队只会等待不会丢失。

use std::sync::{Arc, LazyLock};

/// 专用遍历池：cleaner 4 并发 + 磁盘分析并发共用，8 线程 + work-stealing 足够
static WALK_POOL: LazyLock<Arc<rayon::ThreadPool>> = LazyLock::new(|| {
    Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .thread_name(|i| format!("pony-walk-{i}"))
            .build()
            .expect("build pony walk thread pool"),
    )
});

/// 所有 jwalk::WalkDir 应统一附加 `.parallelism(walk_parallelism())`
pub fn walk_parallelism() -> jwalk::Parallelism {
    jwalk::Parallelism::RayonExistingPool {
        pool: WALK_POOL.clone(),
        busy_timeout: None,
    }
}
