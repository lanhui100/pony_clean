//! 网络监控：公网连通性探测（TCP 直连 443）+ 网卡吞吐量采样。
//!
//! 设计要点（SPEC-032 Rev.2，含双路审核采纳项）：
//! - 探测端点全部走 **443 端口**并免 DNS 直连固定 IP——规避 CN 网络对 TCP 53
//!   出站的常态过滤导致的系统性假黄；
//! - 三探针**并行** connect（每端点一个短命线程 join 收集），全灭最坏 ~0.9s；
//! - 探测线程采用 **deadline 制调度**（周期从探测开始时刻起算），禁止 work-then-sleep；
//! - [`QualityGate`] 滞回：首读与 Offline 立即生效，Good⇄Poor 需连续 2 次同判定，
//!   **任何提交都会清空 pending 判定与计数**（防残留计数提前翻转）；
//! - [`ThroughputSampler`] 逐周期调用 `Networks::refresh_list()`（sysinfo 0.30 中它
//!   本身即差分刷新：新接口以 old=current 入表、消失接口被清除，支持 VPN 建立/
//!   网卡热插拔）；距基线/上次有效采样不足 500ms 则跳过本轮（含首拍，防命令打断
//!   recv_timeout 造成的毫秒级差分噪声尖峰）。

use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::Networks;

/// 探测超时：单个端点 TCP 握手预算
const PROBE_TIMEOUT: Duration = Duration::from_millis(900);

/// 「网络状况差」的最优延迟阈值（毫秒，含恰好等于）
pub const POOR_LATENCY_MS: u64 = 600;

/// 探测周期（deadline 制，从探测开始时刻起算）
pub const PROBE_INTERVAL: Duration = Duration::from_secs(5);

/// 吞吐采样的最小间隔地板：间隔不足时跳过折算、沿用上次读数
const MIN_SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

/// 探测端点：全 443、免 DNS（Cloudflare / AliDNS DoH / DNSPod DoH，CN 友好）
const PROBE_ENDPOINTS: [SocketAddr; 3] = [
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(223, 5, 5, 5)), 443),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(120, 53, 53, 53)), 443),
];

/// 公网连通性质量（序列化为小写字符串供前端消费）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NetQuality {
    /// 连通良好
    Good,
    /// 状况差（高延迟或多端点半数以上不可达）
    Poor,
    /// 全部探针不可达（断联）
    Offline,
}

/// 单轮探测产出的健康快照（滞回滤波后）
#[derive(Clone, Debug, Serialize)]
pub struct NetHealth {
    /// 当前生效质量
    pub quality: NetQuality,
    /// 最优成功握手延迟（毫秒）；全部失败时为 None
    pub latency_ms: Option<u32>,
}

/// 对单个端点做 TCP 连通性探测，返回握手耗时（毫秒）
///
/// 使用 `connect_timeout` + 固定 IP（免 DNS），失败返回 None。
/// 注意：这是「公网连通性」探测，不是「互联网可用性」探测——captive portal
/// 可能对任意 IP 完成握手（已知限制，见 SPEC-032 §2）。
fn probe_endpoint(addr: SocketAddr) -> Option<u64> {
    let began = Instant::now();
    TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).ok()?;
    Some(u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX))
}

/// 并行探测全部端点，返回按端点顺序排列的握手耗时
///
/// **两段式**：先把全部端点的线程 spawn 出来（collect 强制求值），再统一 join。
/// 不可写成 spawn→join 的惰性迭代器链——那会退化成串行探测（代码审核 P1 修正）。
/// spawn 失败的端点以 None 占位，保证「成功数/总数」的分母恒等于端点数
/// （否则分类比例失真）；总耗时受最慢单端点约束（~PROBE_TIMEOUT）而非串行累加。
pub fn probe_all() -> Vec<Option<u64>> {
    let handles: Vec<Option<thread::JoinHandle<Option<u64>>>> = PROBE_ENDPOINTS
        .iter()
        .map(|&addr| {
            thread::Builder::new()
                .name("pony-net-probe".into())
                .spawn(move || probe_endpoint(addr))
                .ok()
        })
        .collect();
    handles
        .into_iter()
        .map(|handle| match handle {
            Some(h) => h.join().unwrap_or(None),
            // spawn 失败（OS 资源耗尽等）：按该端点探测失败计入
            None => None,
        })
        .collect()
}

/// 综合多端点探测结果判定质量（纯函数，可单测）
///
/// 规则（SPEC-032 §4.1）：空集或全部失败 → Offline；成功数 ≤ 总数一半 → Poor；
/// 最优成功延迟 ≥ [`POOR_LATENCY_MS`]（含恰好等于）→ Poor；其余 Good。
pub fn classify_probe(results: &[Option<u64>]) -> NetQuality {
    if results.is_empty() {
        return NetQuality::Offline;
    }
    let successes: Vec<u64> = results.iter().filter_map(|r| *r).collect();
    if successes.is_empty() {
        return NetQuality::Offline;
    }
    if successes.len() * 2 <= results.len() {
        return NetQuality::Poor;
    }
    let best = successes.iter().min().copied().unwrap_or(u64::MAX);
    if best >= POOR_LATENCY_MS {
        return NetQuality::Poor;
    }
    NetQuality::Good
}

/// 质量滞回滤波器
///
/// - 首读立即生效；
/// - `Offline` 方向零滞回，立即生效；
/// - Good⇄Poor 切换需连续 2 次相同判定；
/// - **任何提交（首读 / Offline / 滞回翻转）都清空 pending 判定与计数**。
#[derive(Debug, Default)]
pub struct QualityGate {
    current: Option<NetQuality>,
    pending: Option<(NetQuality, u32)>,
}

impl QualityGate {
    /// 创建空门控（尚未有任何判定）
    pub fn new() -> Self {
        Self::default()
    }

    /// 推入一轮判定，返回当前生效质量
    pub fn push(&mut self, quality: NetQuality) -> NetQuality {
        match self.current {
            // 首读立即提交
            None => {
                self.current = Some(quality);
                self.pending = None;
            }
            Some(current) => {
                if quality == current || quality == NetQuality::Offline {
                    // 同态确认或断联零滞回：立即提交并清 pending
                    self.current = Some(quality);
                    self.pending = None;
                } else {
                    // Good⇄Poor（含从 Offline 部分恢复）：需连续 2 次同判定
                    let count = match self.pending {
                        Some((pending_q, c)) if pending_q == quality => c + 1,
                        _ => 1,
                    };
                    if count >= 2 {
                        self.current = Some(quality);
                        self.pending = None;
                    } else {
                        self.pending = Some((quality, count));
                    }
                }
            }
        }
        self.current.unwrap_or(quality)
    }

    /// 当前生效质量（未完成首次判定时为 None）
    pub fn get(&self) -> Option<NetQuality> {
        self.current
    }
}

/// 启动公网探测后台线程（deadline 制调度）
///
/// 每 [`PROBE_INTERVAL`] 一轮（从探测开始时刻起算），结果经滞回滤波后写入共享
/// 状态（None = 尚未完成首次探测）。锁 poisoned 时静默跳过本轮写入。
///
/// 线程随进程生命周期运行（Tauri 常驻；返回的 JoinHandle 可丢弃即分离，
/// 进程退出自动回收）。阻塞最坏 ~0.9s 发生在独立线程，不影响监控循环。
#[must_use]
pub fn start_probe(state: Arc<RwLock<Option<NetHealth>>>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut gate = QualityGate::new();
        loop {
            let began = Instant::now();
            let results = probe_all();
            let quality = gate.push(classify_probe(&results));
            let latency_ms = results
                .iter()
                .filter_map(|r| *r)
                .min()
                .and_then(|ms| u32::try_from(ms).ok());
            if let Ok(mut guard) = state.write() {
                *guard = Some(NetHealth {
                    quality,
                    latency_ms,
                });
            }
            // deadline 制：不足一个周期才补睡，保证节拍恒定
            let elapsed = began.elapsed();
            if elapsed < PROBE_INTERVAL {
                thread::sleep(PROBE_INTERVAL - elapsed);
            }
        }
    })
}

/// 回环接口名匹配（best-effort 兜底）
///
/// 覆盖三类命名：显式含 `loopback`（ASCII 大小写不敏感，如 Windows
/// 「Loopback Pseudo-Interface 1」）与 Linux/macOS 惯用短名 `lo`/`lo0`。sysinfo 的
/// Windows 枚举本身已过滤软件/环回接口，且 FriendlyName 是本地化字符串，
/// 此处仅作其他平台的兜底防线。
fn is_loopback_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("loopback") || lower == "lo" || lower == "lo0"
}

/// 折算字节速率（B/s）。elapsed 非法（非有限 / ≤0）时返回 0.0。
fn compute_bps(delta_bytes: u64, elapsed_secs: f64) -> f64 {
    if !elapsed_secs.is_finite() || elapsed_secs <= 0.0 {
        return 0.0;
    }
    delta_bytes as f64 / elapsed_secs
}

/// 汇总各接口的字节增量（跳过回环接口）。参数为 (接口名, 下行增量, 上行增量)。
///
/// sysinfo 的 Windows 实现用 saturating_sub 维护差分：计数回退得 0 而非巨值，
/// 此处只需累加。
fn collect_sample<'a, I>(ifaces: I) -> (u64, u64)
where
    I: Iterator<Item = (&'a str, u64, u64)>,
{
    let mut down = 0u64;
    let mut up = 0u64;
    for (name, rx, tx) in ifaces {
        if is_loopback_name(name) {
            continue;
        }
        down = down.saturating_add(rx);
        up = up.saturating_add(tx);
    }
    (down, up)
}

/// 网卡吞吐量采样器
///
/// 内部持有 `Networks` 与上次有效采样时刻；[`Self::sample`] 返回
/// `(下行 B/s, 上行 B/s)`。逐周期调用 `refresh_list()`（差分刷新，支持接口
/// 热插拔/VPN 建立）；距上次有效采样不足 500ms 时跳过本轮——不刷新、不推进
/// 时间戳、沿用上次读数。
pub struct ThroughputSampler {
    networks: Networks,
    created_at: Instant,
    last_sample: Option<Instant>,
    down_bps: f64,
    up_bps: f64,
}

impl Default for ThroughputSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl ThroughputSampler {
    /// 创建采样器并对网卡列表建立基线（基线拍增量为 0，不会产生启动尖峰）
    pub fn new() -> Self {
        let mut networks = Networks::new();
        networks.refresh_list();
        Self {
            networks,
            created_at: Instant::now(),
            last_sample: None,
            down_bps: 0.0,
            up_bps: 0.0,
        }
    }

    /// 采样一次，返回 (下行 B/s, 上行 B/s)
    pub fn sample(&mut self) -> (f64, f64) {
        self.sample_at(Instant::now())
    }

    /// [`Self::sample`] 的可注入时钟实现（测试 seam）
    fn sample_at(&mut self, now: Instant) -> (f64, f64) {
        // 间隔地板：距基线/上次有效采样不足 MIN_SAMPLE_INTERVAL 时跳过本轮，
        // 不刷新、不推进时间戳、沿用上次读数（防命令打断 recv_timeout 造成的
        // 毫秒级差分噪声；首拍同样受保护，沿用初始 0.0）
        let reference = self.last_sample.unwrap_or(self.created_at);
        if now.duration_since(reference) < MIN_SAMPLE_INTERVAL {
            return (self.down_bps, self.up_bps);
        }
        self.networks.refresh_list();
        let (down, up) = collect_sample(
            self.networks
                .iter()
                .map(|(name, data)| (name.as_str(), data.received(), data.transmitted())),
        );
        let elapsed_secs = match self.last_sample {
            Some(last) => now.duration_since(last).as_secs_f64(),
            None => now.duration_since(self.created_at).as_secs_f64(),
        };
        self.down_bps = compute_bps(down, elapsed_secs);
        self.up_bps = compute_bps(up, elapsed_secs);
        self.last_sample = Some(now);
        (self.down_bps, self.up_bps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── classify_probe ──────────────────────────────────────────────

    #[test]
    fn test_classify_empty_is_offline() {
        assert_eq!(classify_probe(&[]), NetQuality::Offline);
    }

    #[test]
    fn test_classify_all_failed_is_offline() {
        assert_eq!(classify_probe(&[None, None, None]), NetQuality::Offline);
    }

    #[test]
    fn test_classify_minority_success_is_poor() {
        // 1/3 成功：≤ 总数一半 → Poor
        assert_eq!(classify_probe(&[Some(10), None, None]), NetQuality::Poor);
        // 2/3 成功且延迟低 → Good（阈值公式而非硬编码 ≤1）
        assert_eq!(
            classify_probe(&[Some(10), Some(20), None]),
            NetQuality::Good
        );
    }

    #[test]
    fn test_classify_boundary_exactly_600ms_is_poor() {
        assert_eq!(
            classify_probe(&[Some(599), Some(599), Some(599)]),
            NetQuality::Good
        );
        assert_eq!(
            classify_probe(&[Some(600), Some(600), Some(600)]),
            NetQuality::Poor
        );
    }

    // ── QualityGate ─────────────────────────────────────────────────

    #[test]
    fn test_gate_first_read_commits_immediately() {
        for q in [NetQuality::Good, NetQuality::Poor, NetQuality::Offline] {
            let mut gate = QualityGate::new();
            assert_eq!(gate.get(), None);
            assert_eq!(gate.push(q), q);
            assert_eq!(gate.get(), Some(q));
        }
    }

    #[test]
    fn test_gate_offline_is_immediate() {
        let mut gate = QualityGate::new();
        gate.push(NetQuality::Good);
        assert_eq!(gate.push(NetQuality::Offline), NetQuality::Offline);
    }

    #[test]
    fn test_gate_good_poor_needs_two_confirmations() {
        let mut gate = QualityGate::new();
        gate.push(NetQuality::Good);
        // 第一次 Poor 仅挂起
        assert_eq!(gate.push(NetQuality::Poor), NetQuality::Good);
        // 第二次 Poor 才翻转
        assert_eq!(gate.push(NetQuality::Poor), NetQuality::Poor);
    }

    #[test]
    fn test_gate_oscillation_never_flips() {
        let mut gate = QualityGate::new();
        gate.push(NetQuality::Good);
        for _ in 0..10 {
            assert_eq!(gate.push(NetQuality::Poor), NetQuality::Good);
            assert_eq!(gate.push(NetQuality::Good), NetQuality::Good);
        }
    }

    #[test]
    fn test_gate_commit_clears_pending_residual() {
        // 回归锚点（B-P1-1）：Good→Poor(pending)→Offline(提交清 pending)→
        // Poor→Poor 第二次才翻转；若 pending 残留会提前翻转
        let mut gate = QualityGate::new();
        gate.push(NetQuality::Good);
        assert_eq!(gate.push(NetQuality::Poor), NetQuality::Good); // pending=(Poor,1)
        assert_eq!(gate.push(NetQuality::Offline), NetQuality::Offline); // 清 pending
        assert_eq!(gate.push(NetQuality::Poor), NetQuality::Offline); // pending=(Poor,1)，不得翻转
        assert_eq!(gate.push(NetQuality::Poor), NetQuality::Poor); // 第二次才翻转
    }

    #[test]
    fn test_gate_recovery_needs_two_confirmations() {
        // Offline→Good 走滞回（恢复方向防抖，验收预算按 ≤2 周期核算）
        let mut gate = QualityGate::new();
        gate.push(NetQuality::Offline);
        assert_eq!(gate.push(NetQuality::Good), NetQuality::Offline);
        assert_eq!(gate.push(NetQuality::Good), NetQuality::Good);
    }

    #[test]
    fn test_gate_poor_to_offline_immediate() {
        // spec §8：Poor→Offline 显式迁移（Offline 方向零滞回，与来源态无关）
        let mut gate = QualityGate::new();
        gate.push(NetQuality::Poor);
        assert_eq!(gate.push(NetQuality::Offline), NetQuality::Offline);
        assert_eq!(gate.push(NetQuality::Offline), NetQuality::Offline);
    }

    #[test]
    fn test_gate_poor_recovery_needs_two_confirmations() {
        // Poor→Good 同样走双次确认滞回
        let mut gate = QualityGate::new();
        gate.push(NetQuality::Poor);
        assert_eq!(gate.push(NetQuality::Good), NetQuality::Poor);
        assert_eq!(gate.push(NetQuality::Good), NetQuality::Good);
    }

    #[test]
    fn test_gate_reverse_pending_residual_cleared() {
        // 反向残留回归：Offline→Poor(pending)→Offline(提交清 pending)→Poor→Poor
        // 第二次 Poor 才翻转；若 pending 残留会提前翻转
        let mut gate = QualityGate::new();
        gate.push(NetQuality::Offline);
        assert_eq!(gate.push(NetQuality::Poor), NetQuality::Offline); // pending=(Poor,1)
        assert_eq!(gate.push(NetQuality::Offline), NetQuality::Offline); // 清 pending
        assert_eq!(gate.push(NetQuality::Poor), NetQuality::Offline); // pending=(Poor,1)
        assert_eq!(gate.push(NetQuality::Poor), NetQuality::Poor); // 第二次才翻转
    }

    // ── 吞吐量采样 ──────────────────────────────────────────────────

    #[test]
    fn test_compute_bps_basic_and_zero() {
        assert!((compute_bps(2000, 2.0) - 1000.0).abs() < f64::EPSILON);
        assert_eq!(compute_bps(0, 2.0), 0.0);
    }

    #[test]
    fn test_compute_bps_invalid_elapsed_returns_zero() {
        assert_eq!(compute_bps(1000, 0.0), 0.0);
        assert_eq!(compute_bps(1000, -1.0), 0.0);
        assert_eq!(compute_bps(1000, f64::NAN), 0.0);
    }

    #[test]
    fn test_collect_sample_skips_loopback_case_insensitive() {
        // Windows 回环伪接口实名「Loopback Pseudo-Interface 1」（大写 L）
        let (down, up) = collect_sample(
            [
                ("Loopback Pseudo-Interface 1", 9999u64, 9999u64),
                ("以太网", 100u64, 50u64),
                ("lo", 123u64, 123u64),
            ]
            .into_iter(),
        );
        assert_eq!(down, 100);
        assert_eq!(up, 50);
    }

    #[test]
    fn test_collect_sample_saturating_on_huge_values() {
        let (down, _) =
            collect_sample([("eth0", u64::MAX, 0u64), ("eth1", 1u64, 0u64)].into_iter());
        assert_eq!(down, u64::MAX); // 不回绕
    }

    #[test]
    fn test_sampler_first_sample_within_floor_is_skipped() {
        // 首拍距构造基线 <500ms：跳过、不推进时间戳、沿用初始 0.0
        let mut sampler = ThroughputSampler::new();
        let early = sampler.created_at + Duration::from_millis(300);
        let before = sampler.last_sample;
        let (down, up) = sampler.sample_at(early);
        assert_eq!((down, up), (0.0, 0.0));
        assert_eq!(sampler.last_sample, before);
        assert_eq!(sampler.last_sample, None);
    }

    #[test]
    fn test_sampler_normal_interval_advances_timestamp() {
        let mut sampler = ThroughputSampler::new();
        let t1 = sampler.created_at + Duration::from_secs(2);
        let _ = sampler.sample_at(t1);
        assert_eq!(sampler.last_sample, Some(t1));
        // 距上次有效采样 <500ms：跳过且时间戳不推进（代码审核 P2-1 seam 用例）
        let (down, up) = sampler.sample_at(t1 + Duration::from_millis(300));
        assert_eq!((down, up), (sampler.down_bps, sampler.up_bps));
        assert_eq!(sampler.last_sample, Some(t1));
    }

    #[test]
    fn test_sampler_normal_interval_yields_finite_nonnegative() {
        // 正常间隔采样：读数必须有限且非负。
        // 注意：sample_at 内部走真实 refresh_list，联网机器上测试窗口内可能有
        // 后台流量，故不断言具体数值（零流量折算由 compute_bps 单测覆盖）。
        let mut sampler = ThroughputSampler::new();
        let t1 = sampler.created_at + Duration::from_secs(2);
        let (down, up) = sampler.sample_at(t1);
        assert!(down.is_finite() && down >= 0.0, "down={down}");
        assert!(up.is_finite() && up >= 0.0, "up={up}");
        assert_eq!(sampler.last_sample, Some(t1));
    }

    // ── 序列化契约 ──────────────────────────────────────────────────

    #[test]
    fn test_net_quality_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&NetQuality::Good).unwrap(),
            "\"good\""
        );
        assert_eq!(
            serde_json::to_string(&NetQuality::Poor).unwrap(),
            "\"poor\""
        );
        assert_eq!(
            serde_json::to_string(&NetQuality::Offline).unwrap(),
            "\"offline\""
        );
    }

    #[test]
    fn test_net_health_unready_serializes_null_latency() {
        let health = NetHealth {
            quality: NetQuality::Good,
            latency_ms: None,
        };
        let value = serde_json::to_value(&health).unwrap();
        assert_eq!(value["latency_ms"], serde_json::Value::Null);
    }
}
