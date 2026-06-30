# Release Checklist

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
