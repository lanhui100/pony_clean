# TASK-031: CNB 流水线自动同步 updater/latest.json

## Basic Info
- Status: Dropped
- Priority: P1
- Owner: @self
- Created: 2026-08-22
- Dropped: 2026-08-22
- Estimated: —
- Depends: 无（关联 TASK-030 版本管理体系）
- Complexity: B
- Spec: 未创建

**Drop 原因**：用户决策（2026-08-22）——发版与更新源**收敛为 GitHub 单平台**（ADR-013），
CNB 推送/发版整体移除，本卡「同步 CNB 清单」的目标随之失效。连带清理已直接执行：
`tauri.conf.json` 移除 CNB 端点、删除 `.cnb.yml` 与陈旧 `updater/latest.json`（v0.1.2）、
README 下载链接改指 GitHub Releases、VERSIONING/RELEASE_BUILD/RELEASE_CHECKLIST/docs README
同步更新、`git remote remove cnb`。

## Goal
保证每次发版后，CNB main 分支的 `updater/latest.json` 与最新发布版本一致（版本号 / pub_date / 双平台可用的下载 URL），消除「备用更新端点清单滞后」这一整类故障。

## 背景
updater 双端点的备用源为 `https://cnb.cool/lanhui100/pony_clean/-/git/raw/main/updater/latest.json`，
由 CNB 流水线（`.cnb.yml` tag_push 末段「生成并提交更新清单」stage）生成并推送 main 维护。
**2026-08-22 实测：该清单停留在 v0.1.2（pub_date 2026-08-19）**——0.1.2 之后的所有发版均未推 CNB，
自动同步随之停摆。实测后果链：用户在 v0.3.3 点「下载并安装」，GitHub 资产域名被网络阻断后重试
回落到陈旧备用清单，旧代码将其误判为「已是最新版本」（前端守卫已在本次 bugfix 中修复——不再
撒谎、改为诚实报错，但**根因未除**：备用源自 0.1.2 起就不可用，v0.3.5 发版若仍不推 CNB，
重试换源永远救不回来）。

## 进展记录（2026-08-22）
- 已确认 `.cnb.yml` tag_push 流水线含完整补偿链路：交叉构建 → Release → 上传产物 →
  自动生成 latest.json 推送 main（流水线机器人自带 `CNB_TOKEN`）。**补偿修复 = 把 main + tag
  推到 CNB 触发流水线即可，无需手工造清单**。
- 用户提供的令牌 `dsh` 试推 main/tag 均 403（无仓库写权限）；API 反查确认 scope 受限。
- cnb 远端无 `v0.3.4` tag、main 在 `cf4748f`（远落后本地 `87de4ff`），推送无冲突风险。

## Acceptance
1. 发版（push `v*` tag）后，CNB 流水线自动将 `updater/latest.json` 同步到 main：
   `version` == tag 版本、`pub_date` 为 RFC3339、`platforms.*.url` 指向实际可达的安装包产物
   （GitHub Release 或 CNB Release 资产）
2. 幂等：同一 tag 重复触发不产生空提交、不与并行写入冲突（冲突时以最新 tag 版本为准）
3. 同步失败必须可见（流水线 job 红灯），不允许静默成功
4. 补偿修复：一次性把当前滞后的清单补齐到 0.3.4（三要素同上），并用
   `GET raw/main/updater/latest.json` 实测验证
5. 文档同步：`docs/VERSIONING.md` 双平台章节写明该机制与「跳过任一平台推送」的后果；
   如实现依赖凭据/令牌，记录配置位置（不含明文密钥）
6. 门禁：`.cnb.yml` 语法校验通过；如引入脚本，附带单元测试

## Non-Goal
不改 updater 前端逻辑（useUpdater.ts 守卫已落地）；不动签名密钥体系；不做多版本清单回溯。

## Validation Evidence
_(待实施后填写)_

## Next Action

无（已 Dropped）。若未来需要恢复双平台分发，从 ADR-013 的后果清单反向回滚：
重建 `.cnb.yml`（git 历史可考）、恢复备用端点、补齐 CNB 写权限令牌。

## Resume Hint

本卡已废弃。相关有效资产：`docs/DESIGN.md` ADR-013（决策与后果）、`docs/VERSIONING.md`
单平台发版流程；updater 前端守卫与超时修复仍在（useUpdater.ts），不依赖 CNB。
