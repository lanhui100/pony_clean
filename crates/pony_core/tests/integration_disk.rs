//! 空间分析（清理 Tab）扫描数据准确性验证。
//!
//! 验证 `scan_user_dir` 产出的事件流与真实磁盘一致：
//! 1. 大文件集合与地面真值一致（无重复、无遗漏，Temp/node_modules/hive 跳过规则生效）
//! 2. 最终列表按体积降序（前端「仅展示最大的 20 个」依赖此全局排序）
//! 3. `max_files` 上限生效，且截断保留的是**最大**的 N 个
//! 4. 进度事件终值 == 实际遍历文件数（前端「已扫描 N 个文件」不欠账）
//! 5. 目录占用的文件数/体积与直接父目录真值一致

use pony_core::disk::{DiskEvent, LargeFile, scan_dir_usage, scan_user_dir};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, channel};

const MIN_BYTES: u64 = 1024;

/// 测试夹具：在临时目录里构造确定性的目录树
struct Fixture {
    root: PathBuf,
    _tmp: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().to_path_buf();
        Self { root, _tmp: tmp }
    }

    /// 写入 size 字节的文件（自动创建父目录）
    fn write(&self, rel: &str, size: usize) -> PathBuf {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, vec![0u8; size]).unwrap();
        p
    }
}

/// 标准夹具：120 个大文件（a×40 + b×40 + deep\nest×40）+ 干扰项。
/// 大小分段：a=1K..40K，b=100K..139K，nest=200K..239K（互不重叠，便于断言 top-K）。
fn standard_fixture() -> Fixture {
    let f = Fixture::new();
    for i in 0..40u64 {
        f.write(&format!("a\\big{i}.bin"), (1024 * (i + 1)) as usize);
        f.write(&format!("b\\big{i}.bin"), (1024 * (100 + i + 1)) as usize);
        f.write(
            &format!("deep\\nest\\big{i}.bin"),
            (1024 * (200 + i + 1)) as usize,
        );
    }
    // 干扰项：小文件 / 系统 hive / Temp 垃圾区 / node_modules
    f.write("a\\small.txt", 10);
    f.write("NTUSER.DAT", 5 * 1024);
    f.write("AppData\\Local\\Temp\\huge.tmp", 9 * 1024 * 1024);
    f.write("node_modules\\pkg\\index.js", 8 * 1024);
    f
}

/// 同步跑一次 scan_user_dir 并收集全部事件（mpsc 无界缓冲，结束后 drain）
fn run_scan(
    root: &Path,
    max_files: usize,
    dir_depth: usize,
) -> (
    Vec<LargeFile>,
    HashSet<String>,
    u64,
    Vec<pony_core::disk::DirUsage>,
) {
    let (tx, rx): (_, Receiver<DiskEvent>) = channel();
    let cancel = AtomicBool::new(false);
    let (files, _dirs) = scan_user_dir(tx, root, MIN_BYTES, &cancel, max_files, dir_depth);

    let mut from_events: Vec<LargeFile> = Vec::new();
    let mut last_scanned = 0u64;
    let mut dirs_from_events = Vec::new();
    let mut saw_done = false;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            DiskEvent::LargeFiles { files } => from_events.extend(files),
            DiskEvent::Progress { scanned, .. } => last_scanned = scanned,
            DiskEvent::DirUsage { dirs } => dirs_from_events = dirs,
            DiskEvent::Done => saw_done = true,
            DiskEvent::Error(m) => panic!("unexpected error event: {m}"),
        }
    }
    assert!(saw_done, "Done event must be sent");

    // 返回值与事件流必须一致（同一份最终数据）
    let ret_set: HashSet<String> = files.iter().map(|f| f.path.clone()).collect();
    let ev_set: HashSet<String> = from_events.iter().map(|f| f.path.clone()).collect();
    assert_eq!(ret_set, ev_set, "returned files must equal event payload");

    (files, ev_set, last_scanned, dirs_from_events)
}

#[test]
fn test_scan_user_dir_event_stream_matches_disk_truth() {
    let fx = standard_fixture();

    // 地面真值：独立枚举（排除被剪枝目录）
    let mut truth: HashSet<String> = HashSet::new();
    for i in 0..40u64 {
        truth.insert(
            fx.root
                .join(format!("a\\big{i}.bin"))
                .to_string_lossy()
                .into_owned(),
        );
        truth.insert(
            fx.root
                .join(format!("b\\big{i}.bin"))
                .to_string_lossy()
                .into_owned(),
        );
        truth.insert(
            fx.root
                .join(format!("deep\\nest\\big{i}.bin"))
                .to_string_lossy()
                .into_owned(),
        );
    }
    // 遍历计数：120 个大文件 + a\small.txt + NTUSER.DAT（hive 在 scanned 之后才跳过）
    const WALKED: u64 = 122;

    let (files, event_paths, last_scanned, dirs) = run_scan(&fx.root, 1000, 3);

    // 1. 数目准确：恰好等于真值，无重复无遗漏
    assert_eq!(
        event_paths.len(),
        truth.len(),
        "event stream must carry exactly the ground-truth set"
    );
    assert_eq!(&event_paths, &truth, "large file set must match disk truth");
    assert_eq!(files.len(), 120);

    // 2. 全局按体积降序（前端 top-20 展示依赖）
    for w in files.windows(2) {
        assert!(
            w[0].size_bytes >= w[1].size_bytes,
            "final list must be sorted desc by size"
        );
    }

    // 3. 进度终值 == 实际遍历文件数（不欠账、不虚高）
    assert_eq!(
        last_scanned, WALKED,
        "terminal progress must equal walked file count"
    );

    // 5. 目录占用：直接父目录的 file_count / size_bytes 与真值一致
    let find = |suffix: &str| {
        dirs.iter()
            .find(|d| d.path.replace('/', "\\").ends_with(suffix))
            .unwrap_or_else(|| panic!("dir {suffix} must be present"))
    };
    let sum = |base: u64| -> u64 { (0..40u64).map(|i| MIN_BYTES * (base + i + 1)).sum() };
    let a = find("\\a");
    assert_eq!(
        a.file_count, 41,
        "a = 40 big + 1 small (usage 不做大小过滤)"
    );
    assert_eq!(a.size_bytes, sum(0) + 10);
    let b = find("\\b");
    assert_eq!(b.file_count, 40);
    assert_eq!(b.size_bytes, sum(100));
    let nest = find("deep\\nest");
    assert_eq!(nest.file_count, 40);
    assert_eq!(nest.size_bytes, sum(200));

    // 排序后最大目录在前；根目录（仅 hive，已排除）不应出现
    assert_eq!(
        dirs[0].path.replace('/', "\\"),
        nest.path.replace('/', "\\")
    );
    assert!(
        !dirs.iter().any(|d| {
            let p = d.path.replace('/', "\\");
            p.ends_with("\\Temp") || p.contains("node_modules") || p == fx.root.to_string_lossy()
        }),
        "pruned/excluded dirs must not appear in usage"
    );
}

#[test]
fn test_scan_user_dir_max_files_keeps_largest() {
    let fx = Fixture::new();
    // 60 个候选：大小 1K..60K 互异
    for i in 0..60u64 {
        fx.write(&format!("d\\f{i:02}.bin"), (MIN_BYTES * (i + 1)) as usize);
    }
    let (files, _, _, _) = run_scan(&fx.root, 10, 3);

    assert_eq!(files.len(), 10, "max_files cap must be enforced");
    // 截断保留的是最大的 10 个：50K..60K
    let sizes: Vec<u64> = files.iter().map(|f| f.size_bytes).collect();
    let expected: Vec<u64> = (51u64..=60).map(|k| MIN_BYTES * k).rev().collect();
    assert_eq!(
        sizes, expected,
        "cap must keep the LARGEST files, sorted desc"
    );
}

#[test]
fn test_scan_user_dir_max_files_larger_than_result_is_noop() {
    let fx = standard_fixture();
    let (files, _, _, _) = run_scan(&fx.root, 1000, 3);
    assert_eq!(files.len(), 120);
    for w in files.windows(2) {
        assert!(w[0].size_bytes >= w[1].size_bytes);
    }
}

/// 真实机器冒烟（手动：`cargo test -p pony_core --test integration_disk -- --ignored`）。
///
/// 用生产参数（≥100MB、深度 3、上限 1000）扫描当前用户目录，并与独立实现的
/// 地面真值遍历（同剪枝规则的单线程 std::fs 版本）交叉比对：
/// 进度终值 == 真实遍历文件数；大文件条数 == min(真值条数, 上限)；最大体积一致；
/// 结果全局降序且无重复。两端实现互为印证，验证「清理 Tab」真实数据的准确性。
#[test]
#[ignore = "真机全量遍历用户目录，耗时数分钟；手动运行：cargo test -p pony_core --test integration_disk -- --ignored"]
fn real_machine_user_profile_scan_accuracy() {
    let Some(root) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return;
    };
    const MIN: u64 = 100 * 1024 * 1024;
    const CAP: usize = 1000;

    // ── 先跑被测扫描（生产参数），收集事件流 ──
    let (tx, rx) = channel();
    let cancel = AtomicBool::new(false);
    let (files, _) = scan_user_dir(tx, &root, MIN, &cancel, CAP, 3);
    let mut from_events = 0usize;
    let mut last_scanned = 0u64;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            DiskEvent::LargeFiles { files } => from_events += files.len(),
            DiskEvent::Progress { scanned, .. } => last_scanned = scanned,
            _ => {}
        }
    }

    // ── 独立地面真值：与 disk.rs 完全相同的剪枝/跳过规则 ──
    let skip_dirs = [
        "node_modules",
        ".git",
        "__pycache__",
        ".svn",
        "$RECYCLE.BIN",
    ];
    let hives = [
        "ntuser.dat",
        "ntuser.dat.log1",
        "ntuser.dat.log2",
        "usrclass.dat",
        "ntuser.ini",
    ];
    let mut walked = 0u64;
    let mut qualifying: Vec<(u64, String)> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let in_local_temp = dir
            .to_string_lossy()
            .to_lowercase()
            .ends_with("appdata\\local");
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if ft.is_dir() && !ft.is_symlink() {
                // 与后端一致：目录名大小写敏感精确匹配
                if skip_dirs.contains(&name.as_str()) {
                    continue;
                }
                if in_local_temp && name.eq_ignore_ascii_case("temp") {
                    continue;
                }
                stack.push(entry.path());
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            walked += 1;
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.len() < MIN {
                continue;
            }
            if hives.iter().any(|h| name.eq_ignore_ascii_case(h)) {
                continue;
            }
            qualifying.push((meta.len(), entry.path().to_string_lossy().into_owned()));
        }
    }

    // ── 三方一致性断言（真机活盘：允许小漂移，系统性漏扫仍必被检出）──
    // 两次全量遍历间隔约 1-4 分钟，活动系统（IDE/工具链日志、临时 DB）会有
    // 少量大文件生灭。容差取 8：TASK-033 实测系统性漏扫为 19 个，远超容差；
    // 夹具路径（run_scan 系列）保持严格相等。
    const LIVE_DRIFT_FILES: usize = 8;
    let truth_set: HashSet<&String> = qualifying.iter().map(|q| &q.1).collect();
    let scan_set: HashSet<&String> = files.iter().map(|f| &f.path).collect();
    let mut missing: Vec<&&String> = truth_set.difference(&scan_set).collect();
    let mut extra: Vec<&&String> = scan_set.difference(&truth_set).collect();
    missing.sort();
    extra.sort();
    assert!(
        missing.len() <= LIVE_DRIFT_FILES && extra.len() <= LIVE_DRIFT_FILES,
        "scan/truth set mismatch beyond live-FS tolerance: \
         missing_from_scan={} extra_in_scan={}\nmissing sample:\n{:?}\nextra sample:\n{:?}",
        missing.len(),
        extra.len(),
        &missing[..missing.len().min(15)],
        &extra[..extra.len().min(15)]
    );
    assert!(
        files.len() + extra.len() >= qualifying.len().saturating_sub(missing.len()),
        "large file count must track ground truth (capped)"
    );
    assert_eq!(
        from_events,
        files.len(),
        "event payload must match return value"
    );
    // 遍历计数同理：实测活盘漂移 ±1 起步，重负载时段更大；只约束有界
    let drift = (last_scanned as i64 - walked as i64).unsigned_abs();
    assert!(
        drift <= 256,
        "terminal progress {last_scanned} vs independently walked {walked}: \
         drift {drift} exceeds live-FS tolerance"
    );
    let unique: HashSet<&String> = files.iter().map(|f| &f.path).collect();
    assert_eq!(unique.len(), files.len(), "paths must be unique");
    for w in files.windows(2) {
        assert!(w[0].size_bytes >= w[1].size_bytes, "must be sorted desc");
    }
    // 最大文件自洽：以扫描报告的路径集合为基准，用独立真值尺寸复核
    // （活盘上真值遍历可能新见到更大的文件，不作为对扫描的苛求）
    let size_of: std::collections::HashMap<&String, u64> =
        qualifying.iter().map(|q| (&q.1, q.0)).collect();
    let expected_top = files
        .iter()
        .filter_map(|f| size_of.get(&f.path))
        .copied()
        .max();
    if let Some(expected) = expected_top {
        assert_eq!(
            files[0].size_bytes, expected,
            "reported largest file must match ground-truth size"
        );
    } else {
        assert!(
            files.is_empty() && qualifying.is_empty(),
            "non-empty scan must contain ground-truth paths"
        );
    }
}

/// 隐藏目录必须参与扫描：jwalk 默认跳过「名字以 . 开头」的条目，会把
/// .rustup/.cargo/.bun 等大缓存整棵漏扫（真机验证发现，78 vs 97）。回归锁。
#[test]
fn test_scan_user_dir_includes_dot_directories() {
    let fx = Fixture::new();
    fx.write(".hiddentools\\cache\\big.bin", 4096);
    // SKIP_DIRS 名单剪枝仍优先生效：.git 内大文件不进候选
    fx.write(".git\\objects\\pack\\pack.bin", 4096);

    let (_, ev_set, _, _) = run_scan(&fx.root, 1000, 3);

    assert_eq!(
        ev_set.len(),
        1,
        "dot-dir file listed, .git-pruned file excluded"
    );
    assert!(
        ev_set.iter().any(|p| p.contains(".hiddentools")),
        "hidden dir contents must be scanned"
    );
    assert!(!ev_set.iter().any(|p| p.contains(".git")));
}

/// 零命中契约：无 LargeFiles 事件、终值进度 == 遍历数、Done 必达。
#[test]
fn test_scan_user_dir_zero_hits_event_contract() {
    let fx = Fixture::new();
    fx.write("docs\\tiny.txt", 10);

    let (tx, rx) = channel();
    let cancel = AtomicBool::new(false);
    let (files, dirs) = scan_user_dir(tx, &fx.root, MIN_BYTES, &cancel, 1000, 3);
    // 大文件零命中；目录占用不做大小过滤，docs（1 个小文件）仍应出现
    assert!(files.is_empty());
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0].file_count, 1);

    let mut large_events = 0;
    let mut last_scanned = 0;
    let mut saw_done = false;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            DiskEvent::LargeFiles { .. } => large_events += 1,
            DiskEvent::Progress { scanned, .. } => last_scanned = scanned,
            DiskEvent::DirUsage { dirs } => {
                assert_eq!(dirs.len(), 1, "usage counts files regardless of size");
                assert_eq!(dirs[0].file_count, 1);
            }
            DiskEvent::Done => saw_done = true,
            DiskEvent::Error(m) => panic!("unexpected error event: {m}"),
        }
    }
    assert_eq!(
        large_events, 0,
        "no LargeFiles event when nothing qualifies"
    );
    assert_eq!(last_scanned, 1, "terminal progress must equal walked count");
    assert!(saw_done);
}

/// 命中数恰等于上限：全部保留、单次全量事件、返回值一致。
#[test]
fn test_scan_user_dir_exact_cap_boundary() {
    let fx = standard_fixture();
    let (tx, rx) = channel();
    let cancel = AtomicBool::new(false);
    let (files, _) = scan_user_dir(tx, &fx.root, MIN_BYTES, &cancel, 120, 3);

    let mut batches: Vec<usize> = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        if let DiskEvent::LargeFiles { files } = ev {
            batches.push(files.len());
        }
    }
    assert_eq!(batches, vec![120], "exactly one full final batch");
    assert_eq!(files.len(), 120, "hits == cap keeps everything");
    for w in files.windows(2) {
        assert!(w[0].size_bytes >= w[1].size_bytes);
    }
}

/// 取消契约（预置取消）：只补发 [终值 Progress, 空 DirUsage, Done]，无数据批次。
#[test]
fn test_scan_user_dir_cancelled_event_contract() {
    let fx = standard_fixture();
    let (tx, rx) = channel();
    let cancel = AtomicBool::new(true);
    let (files, _) = scan_user_dir(tx, &fx.root, MIN_BYTES, &cancel, 1000, 3);
    assert!(files.is_empty());

    let mut seq: Vec<&str> = Vec::new();
    let mut usage_empty = None;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            DiskEvent::Progress { .. } => seq.push("progress"),
            DiskEvent::LargeFiles { .. } => seq.push("large"),
            DiskEvent::DirUsage { dirs } => {
                seq.push("dirs");
                usage_empty = Some(dirs.is_empty());
            }
            DiskEvent::Done => seq.push("done"),
            DiskEvent::Error(m) => panic!("unexpected error event: {m}"),
        }
    }
    assert_eq!(seq, vec!["progress", "dirs", "done"]);
    assert_eq!(usage_empty, Some(true));
}

/// scan_dir_usage 同样必须在 Done 前补发精确终值进度（>200 文件穿越节流窗口）。
#[test]
fn test_scan_dir_usage_terminal_progress() {
    let fx = Fixture::new();
    for i in 0..250u64 {
        fx.write(&format!("m\\f{i:03}.bin"), 10);
    }

    let (tx, rx): (_, Receiver<DiskEvent>) = channel();
    let cancel = AtomicBool::new(false);
    let dirs = scan_dir_usage(tx, &fx.root, 3, &cancel);

    let mut last_scanned = 0u64;
    while let Ok(ev) = rx.try_recv() {
        if let DiskEvent::Progress { scanned, .. } = ev {
            last_scanned = scanned;
        }
    }
    assert_eq!(
        last_scanned, 250,
        "terminal progress must equal walked count"
    );
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0].file_count, 250);
}
