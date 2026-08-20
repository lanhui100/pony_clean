# Release Checklist

## Release Steps（版本与发布）

发版前先确认版本一致性与 CHANGELOG，完整流程见 [docs/VERSIONING.md](docs/VERSIONING.md)。

1. [ ] `node scripts/check-version.mjs` — 三处清单 + Cargo.lock 版本一致（exit 0）
2. [ ] `CHANGELOG.md` 的 `[Unreleased]` 已记录本次全部变更（非空，bump 强制校验）
3. [ ] `node scripts/bump-version.mjs <新版本>` — 同步三处清单 + Cargo.lock + 归档 CHANGELOG
4. [ ] `cargo tauri build` — 打包（MSI 产品版本 = tauri.conf.json version）
5. [ ] 执行下方 Pre-release Manual Verification（QA 后如需代码修复：先单独提交修复）
6. [ ] 工作树仅剩版本文件改动（其他 WIP 用 `git stash -u` 暂存）后执行
     `node scripts/bump-version.mjs <新版本> --commit --tag` — 生成 `chore(release): vX.Y.Z` 提交 + `vX.Y.Z` tag
7. [ ] `git push --follow-tags` — 推送提交与 tag（勿漏）；`git ls-remote --tags origin` 验证 tag 已推
8. [ ] 推送后确认**双平台流水线**自动构建成功（无需手动上传产物）：
     - GitHub Actions：Actions 页面 `Build Windows Installers` 全绿；[GitHub Release](https://github.com/lanhui100/pony_clean/releases) 附件含 `x64-setup.exe` + `arm64-setup.exe` + 各自 `.sig`（约 8 分钟）
     - CNB：流水线构建成功；CNB Release 附件齐全 + `updater/latest.json` 已更新提交 main
9. [ ] 若 GitHub Release 附件缺失/构建失败，按 [docs/VERSIONING.md](docs/VERSIONING.md)「双平台发版」注意事项排查（tag 指向、版本守卫、签名密钥）

## Pre-release Manual Verification

### Windows Compatibility
- [ ] Windows 11 全新安装（无浏览器、无 UWP）— 扫描正常完成
- [ ] Windows 10 日常使用 6 个月（Chrome/Edge/FF/Discord）— 扫描正常
- [ ] 企业域环境（GPO 锁定 WU/Delivery Optimization）— 非管理员降级正确

### Functional Verification
- [ ] 扫描 → 选择 → 清理 → 日志记录，全链路通过
- [ ] 扫描中途取消 → 部分结果展示 + 可重新发起
- [ ] UWP 包 >50 场景 → 只枚举 50 个

### Security Verification
- [ ] 保护路径下文件不被扫描发现
- [ ] 环境变量注入 TEMP=C: → 被防御拒绝
- [ ] 路径尾部空格/点号 → 保护不被绕过
