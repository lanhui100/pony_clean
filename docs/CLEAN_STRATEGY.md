# PonyClean C 盘清理策略 v2

> 基于社区头部开源项目的调研与对比分析，经 3 路对抗审查（安全/架构/工程）调优，采纳 95% 建议后形成本版。

---

## 1. 社区调研摘要

### 1.1 头部项目概览

| 项目 | 语言 | Stars | 目标数 | 核心策略 |
|------|------|-------|:------:|---------|
| **BleachBit** | Python | 5.9k | 1000+ | CleanerML 分类清理器 + 预览 + 安全粉碎 |
| **burnbytes** | C# | 251 | ~20 | 1:1 替代 cleanmgr.exe + Storage Sense |
| **FluentCleaner** | C# (WinUI) | 新 | 1000+ | winapp2.ini 社区规则 + 独立解析器 |
| **Cleanmgr+** | GSC | 754 | ~20 | cleanmgr 增强 + 社区脚本 |
| **InstallerClean** | C# | 95 | 1 | MSI 孤立文件深度清理 |
| **WinCleanCat** | C# | ~50 | ~15 | 系统垃圾 + 大文件 + 重复文件 |

### 1.2 核心发现

- **BleachBit** 用 CleanerML XML 定义 1000+ 清理器，18 年迭代，社区驱动
- **burnbytes/FluentCleaner** 用 winapp2.ini 社区规则库覆盖数千应用
- **Windows Disk Cleanup** 提供系统级清理（WU/错误报告/驱动包/Windows.old/缩略图）
- 共识：清理工具的覆盖率直接影响用户感知价值
- 安全策略共同点：操作前预览、不碰注册表、不自动删除

### 1.3 差距总结

v1 缺少：缩略图缓存、错误报告/转储、Delivery Optimization、Windows.old、日志文件、字体缓存、应用缓存（Discord/Steam/IM）、下载 >90 天、运行时缓存。

---

## 2. v1 回顾与差距

### 2.1 已有能力

- 15 个扫描目标，三级安全体系（Safe/Confirm/Forbidden）
- 12 条受保护路径，jwalk 并行遍历，3 层删除降级
- COM 初始化的回收站清空，mpsc 事件 + CancellationToken
- JSON 配置持久化，1KB 最小文件过滤，300K 扫描上限

### 2.2 关键差距

| 维度 | 现状 | 目标 | 优先级 |
|------|------|------|--------|
| 清理目标数 | 15 | 40+ | P0 |
| 系统清理 | 部分 | 覆盖 Disk Cleanup | P0 |
| 受保护路径 | 12 | 20+ | P0 |
| 删除前审计 | 无 | DPAPI 操作日志 | P0 |
| 应用覆盖 | 3 浏览器 | 10+ 应用 | P1 |

---

## 3. 新版清理策略

### 3.1 总纲

1. **安全第一**: 三级分级 + 受保护路径强制执行，不引入社区规则
2. **覆盖面为先**: 先补齐系统级再扩展应用层
3. **P0 优先**: Phase 2 不启动直到 P0 修复验收关闭

### 3.2 对抗审查调优摘要

3 路审查（安全 17 项 / 架构 15 项 / 工程 17 项）后主要变更:

| 项目 | 原方案 | 调优后 | 原因 |
|------|--------|--------|------|
| `winevt\Logs` | Confirm | **PROTECTED** | 安全日志销毁=取证破坏 |
| `catroot2` | Confirm | **PROTECTED** | 签名数据库损坏 |
| `spool\drivers` | Confirm | **PROTECTED** | 打印机驱动永久失效 |
| `spool\printers` | Safe | **Confirm** | 活跃打印中断 |
| `SleepStudy` | target+protected | **仅 PROTECTED** | 自相矛盾 |
| `%USERPROFILE%\Videos` | Confirm | **移除** | 误删个人媒体 |
| 安全擦除 | P2 | **移除** | SSD 无意义 |
| winapp2.ini | P2 | **移除** | 80h 成本/风险不匹配 |
| CLI+Scheduler | P1 | **v3** | 功能未稳 |
| 大文件扫描 | Phase 3 | **v3** | 独立架构模块 |

### 3.3 Phase 1 目标（28 新增 + 15 已有 = 43 目标）

#### 3.3.1 Windows 系统垃圾（14 项）

| # | 路径 | 级别 | 类别 |
|---|------|------|------|
| 1 | `%WINDIR%\System32\LogFiles` | Confirm | logs |
| 2 | `%WINDIR%\Logs` | Confirm | logs |
| 3 | `%LOCALAPPDATA%\Microsoft\Windows\WER` | Safe | logs |
| 4 | `%ALLUSERSPROFILE%\Microsoft\Windows\WER` | Safe | logs |
| 5 | `%LOCALAPPDATA%\Temp` (匹配 `*WER*`) | Safe | logs |
| 6 | `%WINDIR%\Temp` (匹配 `*WER*`) | Safe | logs |
| 7 | `%WINDIR%\System32\sru` | Confirm | logs |
| 8 | `%LOCALAPPDATA%\Microsoft\Windows\INetCache\IE` | Safe | cache |
| 9 | `%WINDIR%\System32\oobe\info` | Safe | temp |
| 10 | `%WINDIR%\System32\NtmsData` | Safe | temp |
| 11 | `%WINDIR%\Downloaded Program Files` | Confirm | temp |
| 12 | `%WINDIR%\System32\Macromed\Flash` | Safe | cache |
| 13 | `%SYSTEMDRIVE%\$SysReset` | Confirm | temp |
| 14 | `%SYSTEMDRIVE%\$Windows.~BT` | Confirm | temp |

#### 3.3.2 更新与系统保护（3 项）

| # | 路径 | 级别 | 类别 | 说明 |
|---|------|------|------|------|
| 15 | `%WINDIR%\SoftwareDistribution\DataStore` | Forbidden | cache | 暂未实现 |
| 16 | `%WINDIR%\System32\spool\SERVERS` | Safe | temp | |
| 17 | `%WINDIR%\System32\MsDtc\Trace` | Safe | logs | |

#### 3.3.3 UWP 缓存（5 项，限 50 包）

| # | 路径 | 级别 | 类别 |
|---|------|------|------|
| 18 | `%LOCALAPPDATA%\Packages\*\AC\Temp` | Safe | temp |
| 19 | `%LOCALAPPDATA%\Packages\*\AC\INetCache` | Safe | cache |
| 20 | `%LOCALAPPDATA%\Packages\*\LocalCache` | Safe | cache |
| 21 | `%LOCALAPPDATA%\Microsoft\Windows\AppCache` | Safe | cache |
| 22 | `%LOCALAPPDATA%\Microsoft\TerminalServer Client\Cache` | Safe | cache |

#### 3.3.4 用户数据（6 项）

| # | 路径 | 级别 | 类别 |
|---|------|------|------|
| 23 | `%USERPROFILE%\Downloads` (>90d) | Confirm | temp |
| 24 | `%USERPROFILE%\AppData\Local\CrashDumps` | Safe | logs |
| 25 | `%LOCALAPPDATA%\Temp` (匹配 `*.etl`) | Safe | logs |
| 26 | `%LOCALAPPDATA%\Temp` (匹配 `*.log`) | Safe | logs |
| 27 | `%LOCALAPPDATA%\Microsoft\Media Player` | Safe | cache |
| 28 | `%LOCALAPPDATA%\Microsoft\Windows\Caches` | Safe | cache |

### 3.4 实现约束

| 约束 | 取值 |
|------|------|
| 最小文件阈值 | cache:512B, temp:1024B, logs:4096B |
| 每 target 上限 | 50,000 项 |
| 全局上限 | 1,000,000 项（超限时 UI 提示省略数） |
| UWP 包枚举上限 | 50 个 |
| 日志过期过滤 | 最后修改 >90 天 |
| 通配符 (`*WER*`/`*.etl`/`*.log`) | jwalk 后置 glob 过滤 |

**`is_path_allowed` 加固**: 追加分隔符边界检查防止 `Temp_malicious` 绕过:

```rust
path_str.starts_with(&expanded)
    && (path_str.len() == expanded.len()
        || path_str.as_bytes().get(expanded.len()) == Some(&b'\\'))
```

**环境变量注入防御**: `expand_env` 后 `canonicalize` 并验证仍在目标前缀内。

### 3.5 Phase 2（P0 关闭后启动）

前置条件: OPTIMIZATION_PLAN.md 全部 P0 项验收关闭。

- 2a: 浏览器扩展缓存（Media Cache / GPUCache / Service Worker）
- 2b: Chromium 衍生浏览器（Opera / Brave / Vivaldi）
- 2c: 开发工具缓存（npm / pip / Cargo / Gradle）
- 2d: 通信应用（Discord / Slack / VLC / WeChat）

---

### 3.6 删除策略

```rust
// 三层降级（保留 v1）
1. DeleteFileW
2. MoveFileExW DELAY_UNTIL_REBOOT
3. Skip
```

#### 3.6.1 Windows.old（Confirm，需管理员）

- 检测 `C:\Windows.old` / `C:\$WINDOWS.~BT` / `C:\ESD`
- 创建 >10 天可清理（Microsoft 建议保留期）
- 集成: `std::process::Command("dism")` + locale 无关退出码解析
- 状态: 策略已定，代码未实现（已知架构缺口）

#### 3.6.2 DataStore 特殊处理

当前状态: **暂未实现**。计划: ① 检测 wuauserv 状态 ② `net stop wuauserv` ③ 删除 ④ 重启服务

---

### 3.7 安全体系加固

#### 3.7.1 受保护路径新增

```rust
// 此前被错误标记为清理目标
winevt\Logs                    // 事件日志 — 取证破坏
catroot2                       // 签名目录数据库
spool\drivers                  // 打印机驱动二进制

// 遗漏的系统关键路径
System32\Tasks                 // 计划任务
System32\drivers\etc           // hosts 网络配置
System32\CodeIntegrity         // 代码完整性
System32\Licensing             // 授权/激活
System Volume Information      // 卷影副本
Windows\CSC                    // 脱机文件缓存
Windows\Registration           // COM 注册
System32\config                // SAM/SECURITY
System32\GroupPolicy           // 组策略
Config.Msi                     // Installer 缓存
ProgramData\USOShared          // Update Orchestrator
Users\Public\AccountPictures
```

**SleepStudy 矛盾修复**: 从 PROTECTED 移除，改为代码级仅保护目录本身。

#### 3.7.2 删除操作日志

- `%LOCALAPPDATA%\PonyClean\clean_log.jsonl`
- `CryptProtectData` (DPAPI) 加密
- DACL: 当前用户 (R) + SYSTEM (F)
- 时间戳 + 路径 + 大小 + 结果，永久保留

#### 3.7.3 架构远期（target >50 时）

1. 目标定义外部化为 `targets/*.toml`
2. Firefox/Gecko 规则配置化（非控制流）
3. ScanTarget 加 `id` 字段，配置基于 id
4. `CleanerEngine` 结构体统一 Tauri/CLI 接口

---

### 3.8 类别体系重构（6 类）

| 类别 | 颜色 | 默认勾选 | 最小阈值 |
|------|------|:--------:|:--------:|
| `temp` | blue | Yes | 1024 B |
| `cache` | purple | Yes | 512 B |
| `logs` | amber | No | 4096 B |
| `prefetch` | green | No | 1024 B |
| `recycle_bin` | orange | Yes | - |
| `old_install` | red | No | - |

`update_cache` 归入 `cache`。`dev_cache` 推迟到 Phase 2。

---

### 3.9 实施路线图

| 阶段 | 内容 | 工时 |
|------|------|:----:|
| **S1** | 28 目标追加 + 约束实现 | 8h |
| **S2** | 逐目标验证 + 边界测试 | 8h |
| **S3** | 保护路径补充 + 匹配加固 | 2h |
| **S4** | 操作日志 + 确认弹窗增强 | 3h |
| **S5** | 集成测试全量覆盖 | 4h |
| **总计** | v2 Phase 1 | **~25h** |

**P0 门禁**: S1 前必须关闭全部 P0 项（阈值/进度/level/上限/竞态/弹窗）。

### 3.10 排除范围

安全擦除（SSD 无效）、winapp2.ini（80h 成本风险不匹配）、CLI/Scheduler（v3）、大文件扫描（v3）、重复文件、注册表、还原点、驱动管理。

---

## 4. 相关文档

- S1 实施规格: `04_SPECS/SPEC-S1-Phase1.md`
- 优化计划: `docs/OPTIMIZATION_PLAN.md`

## 5. 参考

1. [BleachBit](https://github.com/bleachbit/bleachbit) 5.9k stars
2. [burnbytes](https://github.com/builtbybel/burnbytes) 251 stars
3. [FluentCleaner](https://github.com/builtbybel/FluentCleaner)
4. [Cleanmgr+](https://github.com/builtbybel/CleanmgrPlus) 754 stars
5. [InstallerClean](https://github.com/no-faff/InstallerClean) 95 stars
6. [WinCleanCat](https://github.com/shaoyidi/WinCleanCat)
7. [Windows Disk Cleanup docs](https://learn.microsoft.com/en-us/windows-server/storage/disk-management/clean-up-drive)
8. pony_clean `cleaner.rs` v1 (962 lines)
9. pony_clean `OPTIMIZATION_PLAN.md`
10. 对抗审查报告: Security 17 项 / Architecture 15 项 / Engineering 17 项
