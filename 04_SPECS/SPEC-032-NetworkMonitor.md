# SPEC-032: 网络监控 — 胶囊/贴边连接点指示器 + 监控页 NET 指示

> Rev.2（2026-08-22）：吸收双路对抗审核（@architect / @reviewer-b）结论。
> 关键修订：探针全 443 化、deadline 制并行探测、逐周期 refresh_list、
> QualityGate 提交清 pending、吞吐采样 elapsed 下限、UI 承诺收缩（bar 无
> tooltip/无 glow）、验收时限重算。

## 1. 背景与目标

PonyClean 当前监控 CPU / 内存 / 磁盘，但对网络状态无感知。用户需求（2 点）：

1. **胶囊态与贴边态**在与屏幕顶边的**连接处中央**增加点状指示器：
   网络联通=绿色、断联=红色、网络状况差=黄色；配色与现有方案一致。
2. **监控页头部**在 CPU / MEM 下方增加合适的网络监控指示器形态。

目标：以最小改动为既有数据流（pony_core 后台线程 → Snapshot → Tauri invoke → useMonitor 轮询）
追加网络健康度 + 吞吐量采集，并驱动两处 UI。

## 2. 范围与非目标

### 范围
- `crates/pony_core` 新增 `netmon` 模块：TCP 连通性探测、质量分类、滞回滤波、吞吐量采样助手。
- `SystemSummary` 序列化结构扩展 4 个字段（向后兼容：纯新增）。
- 前端新增 `NetDot.vue` 共用点状指示器；`CapsuleBar.vue` / `EdgeBar.vue` 连接处中央渲染；
  `MonitorPanel.vue` 头部 CPU/MEM 下方追加 NET 行；`useMonitor.ts` 类型与 computed 扩展。

### 非目标
- 不做逐进程网速统计、不做流量历史图表。
- 不引入 ICMP/ping 依赖，不引入新 crate。
- 不改动窗口形态状态机（pill/bar/island 语义与几何常量不变）。
- IslandSummary（概要态三行条）不在本次范围。
- 根目录孤儿测试文件 `tests/integration_monitor.rs` 不在本任务处理（无包归属，不参与编译）。

### 已知限制（记录在案，QA 不作为 bug）
1. **误报离线**：企业策略拦截直连 IP 出站时全部探针失败 → 显示离线。
2. **假阳在线**：captive portal / 透明代理会对任意 IP 完成 TCP 握手 → 显示良好但实际不可用。
   （本探测语义是「公网连通性」，非「互联网可用性」，tooltip 文案据此措辞。）
3. **睡眠唤醒**首轮可能瞬时红（Offline 立即生效 + Wi-Fi 重连中），下一周期自愈。
4. **固定列表（pin）期间** NET 行随快照冻结，属预期（与 CPU/MEM 一致）；NET 数值加淡化提示。
5. Offline 时速率显示 `—` 会隐藏真实存在的 LAN 流量（吞吐统计全部非回环接口，
   质量探的是公网）——tooltip 注明「公网连通性」口径。

## 3. 系统/用户行为

| 场景 | 表现 |
|---|---|
| 公网连通正常且最优延迟 < 600ms | 点状指示器绿色；NET 行显示 ↓/↑ 实时速率 |
| 高延迟（最优成功延迟 ≥ 600ms）或多端点半数以上不可达 | 黄色；速率照常显示 |
| 全部探针不可达 | 红色；NET 行速率显示 `—` |
| 启动后首个探测完成前 / 后端字段缺失 | 中性灰点（检测中）；速率显示 `—` |
| hover 胶囊点或面板 NET 标签 | title 提示：质量 + 延迟 + 「公网连通性」口径 |
| 贴边条（bar）形态 | 仅颜色指示（10px 条 hover 500ms 即展开为胶囊，tooltip 不可达，刻意不支持） |

## 4. 技术方案

### 4.1 后端 `crates/pony_core/src/netmon.rs`

```rust
pub enum NetQuality { Good, Poor, Offline }   // #[serde(rename_all="lowercase")]，Copy
pub struct NetHealth { pub quality: NetQuality, pub latency_ms: Option<u32> }
```

- **探测端点（全 443，免 DNS 直连，规避 CN 对 TCP 53 出站的常态过滤）**：
  `1.1.1.1:443`（Cloudflare）、`223.5.5.5:443`（AliDNS DoH）、`120.53.53.53:443`（DNSPod DoH）。
- **探测执行**：std-only `TcpStream::connect_timeout`；单端点超时 900ms；
  **三探针并行**（每端点一个短命 std::thread，join 收集）→ 全灭最坏 ~0.9s 而非串行 2.7s；
  返回各端点握手耗时 `Option<u64>` ms。延迟转换 `u32::try_from(ms).unwrap_or(u32::MAX)`，禁止裸 `as`。
- **分类**（纯函数 `classify_probe(&[Option<u64>]) -> NetQuality`）：
  - 空集 或 全部失败 → `Offline`；
  - 成功数 ≤ 总数一半（公式判定，非硬编码 ≤1）→ `Poor`；
  - 最优成功延迟 ≥ `POOR_LATENCY_MS`(600)（含恰好 600）→ `Poor`；
  - 其余 → `Good`。
- **滞回**（`QualityGate`）：首读立即生效；**任何提交（首读 / Offline / 滞回翻转）都清空
  pending 判定与计数**；`Offline` 方向零滞回立即生效；Good⇄Poor 切换需连续 2 次同判定。
  回归锚点：`Good→Poor(pending)→Offline→Poor→Poor` 第二个 Poor 才翻转。
- **探测线程** `start_probe(Arc<RwLock<Option<NetHealth>>>) -> JoinHandle`：
  独立 std::thread；**deadline 制调度**——周期从探测开始时刻起算（`Instant + interval`，
  禁止 work-then-sleep），默认 5s；首跑立即。写共享 `RwLock<Option<NetHealth>>`
  （None = 尚未完成首次探测）。
- **吞吐量采样器** `ThroughputSampler`（内部持有 Networks + 基线时刻 + 上次有效采样
  时刻与读数；`sample()` 返回 `(f64, f64)` B/s，测试经 `sample_at(now)` 时钟 seam 注入）：
  - **逐周期调用 `refresh_list()`**（sysinfo 0.30 中它本身即差分刷新：
    已存在接口做 old/new 差分、新接口以 old=current 入表首拍增量 0、消失接口被清除），
    从而支持 VPN 建立/网卡热插拔后集合自动更新；不得只调一次 `refresh_list` 后反复 `refresh()`。
  - **elapsed 下限 500ms（含首拍）**：距基线或上次有效采样不足则本轮不折算、不推进
    时间戳、沿用上次读数（防命令打断 recv_timeout 造成的毫秒级差分噪声尖峰）。
  - 差分取自 `NetworkData::received()/transmitted()`（自上次刷新字节增量；
    Windows 实现 saturating_sub，计数回退得 0 不产生巨值）。
  - 跳过回环接口：ASCII case-insensitive 含 `loopback` 或名为 `lo`/`lo0`
    （best-effort 兜底；Windows 枚举本身已滤软件/环回接口，且 FriendlyName 是
    本地化字符串，勿依赖其精确拼写）。

### 4.2 `monitor.rs` 集成

`SystemSummary` 新增字段（serde 冒烟契约：四键恒存在，`net_quality` 未就绪时字面 `null`）：

```rust
pub net_down_bps: f64,
pub net_up_bps: f64,
pub net_quality: Option<NetQuality>,   // None = 尚未完成首次探测
pub net_latency_ms: Option<u32>,
```

- `start_shared`（Tauri 路径）与 `start`（channel 路径，当前无存活消费方，
  经共享助手保持行为一致、各自 ~5 行包装）：进入 loop **之前**初始化
  Networks 基线与 probe 共享态（保证首帧 `net_quality=None / bps=0.0`，
  不依赖 first_run 跳过逻辑）；每轮读 probe 锁取值后立刻 drop 再构造 summary，
  **两锁不得嵌套持有**；锁 poisoned 时沿用 `if let Ok(guard)` 静默降级并注释。
- 探测线程生命周期随进程，无需显式停机（Tauri 进程常驻；channel 路径进程退出即回收）。
- 单测**禁止**依赖真实出网（CI 沙箱全灭会被判 Offline）；网络相关只留纯函数与手动 QA。

### 4.3 Tauri 壳层

零改动 —— 快照经现有 `get_processes` 下发，前端轮询自然携带新字段。

### 4.4 前端

- **`useMonitor.ts`**：`SystemSummary` 增加 4 字段（TS：`net_quality: 'good'|'poor'|'offline'|null`、
  `net_latency_ms: number|null` 等）；导出 computed `netQuality/netLatencyMs/netDownBps/netUpBps`，
  内部 `?? null` 兜底旧后端缺字段；fmSpeed 对非有限值返回 `—`（防 NaN 渲染）。
- **新组件 `components/NetDot.vue`**：props `{ status, latencyMs?, size? }`。
  - 颜色映射沿用现状三族：`good→bg-green-600`、`poor→bg-amber-600`、
    `offline→bg-red-500`、`null→bg-white/25`（检测中）。
  - **胶囊/贴边内：无外发光**（两层根容器均 `overflow:hidden`，box-shadow 必裁）；
    面板 NET 行内允许柔和 glow（无裁剪上下文）。呼吸脉动动画仅在 poor/offline 态启用，
    且组件自带 `@media (prefers-reduced-motion: reduce)` 直接关动画（全局兜底不含
    iteration-count，不可依赖）。
  - **形态扩展（2026-08-26，用户反馈）**：`variant="bar"` 竖向指示条 —— 总宽 6px
    （border-box 含左右各 1px 分隔线色浅边线 `rgba(255,255,255,0.10)`，色芯 4px）、
    高度由父级 class 注入、微圆角；色芯用状态色 `@75%` 对齐进度填充强度，
    检测中态 white/25 保证分隔永不消失；dot 形态保持原语义不变。
- **`CapsuleBar.vue`**：props 增 `netStatus/netLatencyMs`。
  初版：顶缘内侧定位 → **修订一（2026-08-24）**：中线垂直居中圆点 →
  **修订二（2026-08-26，用户反馈，现行）**：中央圆点与 1px 分隔线一并替换为
  `NetDot variant="bar"`（h-5 垂直居中，沿用旧分隔线高度），兼作 CPU/MEM 分隔；
  文档流内 flex 子元素（无需绝对定位/z 层），morph 随容器缩放天然连续；
  带 title、点击冒泡触发展开属预期。
- **`EdgeBar.vue`**：props 同 `netStatus`（无 `netLatencyMs`——bar 态无 tooltip 场景，
  延迟无处消费）；原 `.sep` 分隔线与中央圆点替换为 `NetDot variant="bar"`
  通高（h-full）兼作分隔；`pointer-events:none`、无 title（见 §3）。
- **morph 瞬态**：非均匀 scale 会在 300ms 过渡内把竖条瞬态压扁，接受不补偿（过度工程）。
- **`CapsuleWindow.vue`**：从 `useMonitor()` 取 computed 传给两个 Bar。
- **`MonitorPanel.vue`**：CPU/MEM 行下方追加同构 NET 行（label 在上、数值在下、双列 gap-10）：
  左「↓ DOWN」右「↑ UP」；label 行内嵌 NetDot（带 title 与 glow）；
  `fmSpeed()` 自动 B/s→KB/s→MB/s；offline/未就绪显示 `—`；`/s` 后缀小字号复用 `%` 模式；
  paused 时整行 opacity-50 表示冻结。

## 5. 影响面与依赖

- 序列化结构变化为**纯新增字段**：旧前端读新后端不受影响（多余 JSON 键被忽略）；
  新前端读旧后端由 computed `?? null` 兜底（灰点/`—`）。
- `pony_core` 保持零 Tauri 依赖；仅用 std + sysinfo 已有依赖。
- 开销可忽略：GetIfTable2 每 ~2s + 每 5s 3 个并行 SYN
  （个别第三方防火墙可能对未签名应用弹拦截提示，文档备注）。

## 6. 任务拆解与并行边界

| # | 任务 | 并行性 |
|---|---|---|
| 1 | netmon.rs + 纯函数单测 | 可先行 |
| 2 | monitor.rs 两路径集成 + lib.rs 注册 | 依赖 #1 |
| 3 | useMonitor.ts + NetDot.vue + CapsuleBar/EdgeBar/CapsuleWindow | 与 #1 无依赖可并行 |
| 4 | MonitorPanel.vue NET 行 | 依赖 #3 类型 |
| 5 | 门禁 + 文档收口 | 串行最后 |

## 7. 风险、回滚

- 风险：直连 IP 被拦截 → 误报离线；captive portal → 假阳良好（§2 已知限制）。
- 风险：断连暗窗后恢复的首个大差分帧可能表现为单帧高速率尖峰
  （真实发生流量压缩进一个间隔）；elapsed 下限不覆盖此场景，接受为残余风险。
- 回滚：revert 单个 commit 即可；序列化新增字段对旧代码无害。

## 8. 测试计划

**单元（cargo test -p pony_core，全部纯函数，不出网）：**
1. `classify_probe(&[])` → Offline（空集语义）。
2. classify_probe 全分支：全失败 / 成功数 ≤ 半数 / 最优延迟恰 600ms / 正常。
3. QualityGate：三态首读各自立即生效；Good⇄Poor 交替振荡永不翻转；
   **pending 残留回归** `Good→Poor→Offline→Poor→Poor` 第二次 Poor 才翻转；
   Poor→Offline、Poor→Good 双确认恢复、**反向残留**
   `Offline→Poor(pending)→Offline→Poor→Poor` 迁移各一测。
4. ThroughputSampler（经 `sample_at(now)` 时钟 seam）：首拍 <500ms 跳过且时间戳
   不推进；正常间隔推进时间戳、<500ms 再采沿用上次读数；零流量 → 0.0。
5. loopback 大小写与短名：`"Loopback Pseudo-Interface 1"`、`lo`、`lo0` 被跳过。
6. serde 冒烟：`NetQuality::Good` → `"good"`；summary 四新键恒存在、未就绪为 `null`。
7. probe_all 的「长度恒等于端点数」契约由常量数组 + 两段式收集结构性保证，
   不做真实出网断言（SPEC §4.2 约束）。

**门禁**：`cargo fmt --check`、`cargo clippy -p pony_core -p pony_clean`、
`cargo check -p pony_core -p pony_clean`、`cargo test -p pony_core`、
`npx vue-tsc --noEmit`、`npm run build`。

**手动 QA（留给用户）**：
- 断网/恢复观察红⇄绿实测时延；限速/高延迟环境观察黄；
- pill⇄bar morph 中指示器表现；hover tooltip 文案；
- 启动后连接 VPN / 插拔网线，速率应出现（refresh_list 逐周期回归）；
- captive portal 环境、睡眠唤醒、中文系统接口名；
- 钉住列表时 NET 行冻结呈淡化是否符合预期文案。

## 9. 验收标准

1. 胶囊与贴边条中央为竖向网络指示条（2026-08-26 用户反馈修订，替代圆点）：总宽 6px、
   左右浅边线兼作 CPU/MEM 分隔边界，胶囊内 20px 居中 / 贴边条通高；morph 无瞬跳；
   good/poor/offline/检测中四态颜色符合 §3 且与现有绿/黄/红三族一致；bar 态无 tooltip、无外发光（设计裁定）。
2. 监控页头部 CPU/MEM 正下方出现 ↓/↑ 双列 NET 行，样式与 CPU/MEM 同构；
   offline/未就绪速率为 `—`；paused 淡化。
3. **时延预算（deadline 制 + 并行探测 + Offline 零滞回下核算）**：
   后端 `net_quality` 断网后 ≤8s 翻转为 Offline（快照级）；
   UI 点状变红 ≤12s；恢复变绿 ≤15s（双次确认滞回下最坏 ≈14s、均值 ~10s，QA 按 15s 判线）。
4. §8 单测全绿 + 全部门禁通过。

## 10. 审核记录

- Spec 对抗审核 Rev.1：@architect（有条件通过）/ @reviewer-b（有条件通过）并行完成。
  采纳情况：B-P0-1、A-P0-1、A-P0-2、B-P1-1~3、B-P2-1~3、A-P2-1~4 及双方 P3 全部采纳；
  不采纳项：单向滞回简化（双向门控小且有测试）、MAC 地址回环判定（平台枚举已过滤，
  名过滤仅兜底）、glow 径向渐变替代（选实心点更简）、morph 反向缩放补偿（过度工程）。
- 代码双审（Rev.2 后）：@code-reviewer-a / @code-reviewer-b 均有条件通过，已回改：
  - **P1**：probe_all 惰性迭代器退化为串行 → 两段式先全 spawn 再统一 join（A）；
  - **P2**：spawn 失败端点以 None 占位保分母恒等（A+B）；SPEC §9-3 恢复预算
    ≤12s→≤15s 数学修正（B）；补 ThroughputSampler 地板测试 + `sample_at` seam（B+A）；
  - **P3 采纳**：首拍地板保护、lo/lo0 匹配、在线零速率 `0 B/s` 与 offline 区分、
    net_* TS 类型可选化、NetDot size 钳制、monitor.rs doc 去重/`let _` 简化、
    start() 探测线程生命周期注记、滞回期 tooltip 异轮次说明；
  - **不采纳**：UP 列占位错位说（两列结构对称，flex gap 对全部子元素生效）、
    probe_all 出网契约单测（违反 §4.2 不出网约束，结构性保证）、thread::scope 重构
    （行为等价，收益仅为防未来早退）、read_net_health 返回 NetHealth clone（维持元组）。
