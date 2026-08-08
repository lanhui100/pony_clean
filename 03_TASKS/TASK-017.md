# TASK-017: 清理路径规则可配置 — 自定义清理目标

## Basic Info
- Status: Done
- Priority: P1
- Owner: @self
- Created: 2026-08-08
- Estimated: 5h
- Depends: TASK-016

## Goal
M4 里程碑：清理路径规则可配置，用户可添加自己的目录作为清理目标。

## Output
- `ScanTarget` 静态字段重构为 `String`（id/description/glob/service/browser），支持运行期自定义
- `PonyConfig.custom_targets` + `get_filtered_targets` 合并自定义目标
- `get_clean_config` / `save_clean_config` 命令
- 设置面板自定义规则管理 UI（添加/启用/删除，分类与级别选择）

## Acceptance
1. 用户可添加自定义清理目录（支持 %ENV% 展开），扫描时纳入
2. 自定义目标仍受受保护路径检查 + 环境变量注入防御
3. 禁用/Forbidden/id 冲突的自定义目标被过滤
4. 设置面板可管理规则并持久化
5. fmt / clippy / test / vue-tsc / build 全通过

## Validation
- `cargo fmt --check` — Pass
- `cargo clippy -p pony_core -p pony_clean` — Pass（0 警告）
- `cargo test -p pony_core` — Pass（69 单元 + 6 集成，含 4 项新增）
- `npx vue-tsc --noEmit` — Pass
- `npm run build` — Pass
- 手动 QA — 待执行（添加规则 → 扫描验证自定义目录出现）

## Next Action
随 TASK-013/015/016 一并手动 QA。