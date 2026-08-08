# TASK-015: Wave 3 — 托盘/通知/自启/配置持久化

## Basic Info
- Status: Validation
- Priority: P1
- Owner: @self
- Created: 2026-08-08
- Estimated: 6h
- Depends: TASK-013

## Goal
Wave 3 里程碑：托盘图标常驻、CPU/内存超阈值系统通知、开机自启、配置持久化。

## Output
- 托盘图标（显示/隐藏、退出菜单，左键单击切换窗口）
- 告警通知：CPU ≥ 80% / 内存 ≥ 85% 时系统通知（阈值可配置，状态变化去重）
- 开机自启：HKCU Run 键（随配置保存）
- 配置持久化：`app_config_dir/config.json`（get_config / set_config 命令）

## Acceptance
1. 托盘图标常驻，左键/菜单可切换胶囊窗口显示，菜单可退出
2. 超阈值时仅发送一次通知，恢复后再次超阈值可重新通知
3. 开机自启写入/删除 HKCU Run 键
4. 配置读写 JSON 持久化，默认值 80/85
5. `npx vue-tsc --noEmit`、`npm run build`、`cargo check -p pony_clean` 通过

## Validation
- `npx vue-tsc --noEmit` — Pass
- `npm run build` — Pass
- `cargo check -p pony_clean` — Pass
- 手动 QA — 待执行（托盘菜单、通知弹出、自启注册表、配置读写）

## Next Action
- 手动 QA：托盘图标显示、菜单切换、通知触发（dev 环境 toast 可能不可用，需打包验证）
- 后续：设置面板 UI（阈值调节入口）