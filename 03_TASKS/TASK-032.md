# TASK-032: 网络监控 — 连接点点状指示器 + 监控页 NET 指示

## Basic Info
- Status: Validation（代码 + spec/代码双审闭环 + 门禁全绿；手动 QA 留待用户）
- Priority: P1
- Owner: @self（agent team 编排）
- Created: 2026-08-22
- Estimated: 4h
- Depends: 无（复用 TASK-002 监控数据流、SPEC-029 形态几何）
- Complexity: B
- Spec: `04_SPECS/SPEC-032-NetworkMonitor.md`

## Goal
1. 胶囊态与贴边态与屏幕顶边连接处中央增加点状网络指示器：联通绿 / 断联红 / 状况差黄，配色沿用现有三族（green-600 / amber-600 / red-500）。
2. 监控页头部 CPU/MEM 下方追加 ↓/↑ 双列 NET 指示行（实时速率 + 质量色点）。

## 背景
现有监控覆盖 CPU/MEM/磁盘，无网络感知。数据流已有稳定通道：
pony_core 后台线程 → Snapshot → `get_processes` invoke → useMonitor 共享轮询
（胶囊窗口与岛窗口各自挂载，2s/3s 自适应间隔），网络指标搭便车即可，
无需新增 command/event。

## Acceptance
1. pill 顶缘连接处中央、bar 中央可见实心 NetDot；good/poor/offline/检测中四态视觉符合 SPEC §3；
   bar 态无 tooltip、无外发光（设计裁定，overflow:hidden 裁剪规避）。
2. MonitorPanel 头部第三行 NET 与 CPU/MEM 行同构；offline/未就绪速率显示 `—`；paused 淡化。
3. 时延预算（deadline 制调度 + 三探针并行 + Offline 零滞回下核算）：
   后端 `net_quality` 断网后 ≤8s 翻转（快照级）；UI 点状变红 ≤12s；
   恢复变绿 ≤15s（双次确认滞回下最坏 ≈14s，QA 按 15s 判线）。
4. 门禁全绿：`cargo fmt --check`、`cargo clippy -p pony_core -p pony_clean`、
   `cargo test -p pony_core`、`npx vue-tsc --noEmit`、`npm run build`。
5. netmon 纯函数单测覆盖全分支且不含真实出网断言。

## Non-Goal
见 SPEC §2（逐进程网速、ICMP、历史图表、IslandSummary、状态机改动均不做）。

## Review Records
- Spec 对抗审核（Rev.1）：@architect 有条件通过 + @reviewer-b 有条件通过，并行完成。
  采纳：探针全 443 化（B-P0-1）、deadline 制+并行探测+验收时限重算（A-P0-1）、
  逐周期 refresh_list（A-P0-2/B-P2-4）、QualityGate 提交清 pending（B-P1-1）、
  elapsed 下限 500ms（B-P1-2/A-P2-2）、收缩 bar tooltip 承诺（B-P1-3）、
  实心点无外发光+morph 瞬态接受（B-P2-2/A-P2-1）、loopback 大小写不敏感（B-P2-1）、
  captive portal 假阳入已知限制（A-P2-3）及双方 P3。
  不采纳：单向滞回简化、MAC 回环判定、glow 径向渐变替代、morph 反向缩放补偿（理由见 SPEC §10）。
- 代码双审：@code-reviewer-a 有条件通过 + @code-reviewer-b 有条件通过。已全部回改：
  P1 probe_all 惰性求值退化串行 → 两段式 spawn/join；P2 spawn 失败 None 占位保分母；
  SPEC §9-3 恢复预算 ≤15s 数学修正；补 sample_at 时钟 seam + 采样器 3 用例 +
  QualityGate 三条迁移测试；P3 批量采纳（首拍地板/lo0/零速率区分/TS 可选化/size 钳制等）。
  不采纳：UP 列错位说（结构对称不成立）、probe_all 出网单测（违反不出网约束），
  理由记录于 SPEC §10。

## Validation Evidence
- `cargo fmt --check` ✅；`cargo clippy -p pony_core -p pony_clean` ✅ 0 warning；
- `cargo test -p pony_core` ✅ 129 lib + 6 integration 全绿（含 netmon 20 项新单测）；
- `cargo check -p pony_core -p pony_clean` ✅；`npx vue-tsc --noEmit` ✅；
- `npm run build` ✅（vite 构建需子进程管道，受限沙箱 EPERM，按政策升级权限后构建成功 1m2s）；
- 门禁后按代码审核意见回改并复跑：fmt/clippy/test/check 全绿（见会话记录）。

## Next Action
用户在真实 Windows 会话运行 `npm run dev:tauri` 手动 QA（SPEC §8 手动 QA 清单：
断网/恢复时延实测、限速环境黄色态、pill⇄bar morph 表现、VPN 热插拔速率出现、
captive portal/睡眠唤醒/中文接口名/paused 淡化）。

## Resume Hint
本卡代码与门禁已闭环；恢复时若 QA 报问题，先对照 SPEC-032 §2 已知限制甄别是否预期行为。
