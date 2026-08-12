# SPEC-025: Windows Update 缓存 + DataStore 清理

- 状态: Draft（待对抗审查 + 安全审查）
- 关联: TASK-025
- 日期: 2026-08-08

## 1. 背景与目标
Windows Update 缓存（`SoftwareDistribution\Download`）常积攒数 GB，属最高收益清理目标。DataStore 为更新数据库（含失效记录）。目标：新增两个清理目标，DataStore 走"停服务 → 删 → 启服务"受控流程。

## 2. 范围与非目标
- 范围：`crates/pony_core/src/cleaner.rs`（ScanTarget + 服务控制）
- 非目标：不做 Windows.old / DISM（独立任务）；不做通用服务控制框架；不做 wuauserv 状态持久化迁移

## 3. 用户/系统行为
- 扫描结果新增分类归属：`wu_download`（Category::Cache，Safe）、`wu_datastore`（Category::Cache，Confirm）
- 删除 DataStore 内容时：先检测 wuauserv 状态 → 运行中则停止 → 删除 `DataStore\*.db` 等文件 → 恢复服务原状态
- 服务操作任何一步失败：不删文件，报错，尽力恢复服务状态

## 4. 技术方案与替代
- **服务控制**：优先用 `windows` crate 的 Service Control Manager API（OpenSCManager/OpenService/ControlService/StartService）—— **需在 `crates/pony_core/Cargo.toml` 增加 `Win32_System_Services` feature**
  - 备选：`net stop/start wuauserv`（std::process::Command）—— 解析输出脆弱，仅作 fallback
- **权限前置检测**：停止服务/写 SoftwareDistribution 前检测权限；权限不足（非管理员）→ **不删、明确报错"需要管理员权限"**，不得静默失败
- **服务恢复为硬要求**：服务停止后用 Drop guard 保证删除路径（含 panic）结束后恢复服务原状态；无法恢复时记录并提示用户
- **DataStore 删除粒度**：仅删目录内文件（*.db / *.jrs / *.blb / *.log），保留目录本身与子目录结构
- 受保护路径校验沿用 `is_path_allowed`：新增 target 需加入 `get_clean_targets`，删除时自动通过校验
- **替代**：整目录删除 → 风险高（服务可能重建失败），否决；仅清文件内容

## 5. 影响面与依赖
- **目标数不变**（wu_download / wu_datastore 已存在）：`wu_datastore` 从 Forbidden 转 Confirm 后参与扫描与删除
- 服务控制需 `#[cfg(windows)]` 隔离；Cargo 新增 `Win32_System_Services` + `Win32_Security` feature
- `test_wu_targets_config` 新增；resolve/计数测试不受影响（总数不变）

## 6. 任务拆解与并行边界
- 与 TASK-023/024/026/027 独立文件为主（cleaner.rs 与 TASK-027 同为 cleaner.rs —— 注意 TASK-027 并行改造也改 cleaner.rs，需串行或合并）

## 7. 风险、回滚与迁移
- 服务控制失败 → 数据不删 + 服务状态恢复（尽力而为）；极端失败恢复不了时记录并提示用户重启服务
- 安全审查重点：服务停止窗口期、权限边界（需管理员？wuauserv 停止通常需管理员 —— 若权限不足则明确报错提示）
- 回滚：还原 wu_download / wu_datastore 级别并移除服务控制集成

## 8. 测试计划
- 单测：target 存在与级别；服务控制函数在非 Windows 用 mock 分支；Windows 集成留手动 QA
- 安全审查 pass：服务操作错误路径、权限不足路径

## 9. 验收标准
见 TASK-025 Acceptance。

## 10. 审核记录
（审查后填写）
