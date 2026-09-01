# Agent Note: 升格 L2——决策格式门 + 负样本 spec + 本地钩子挂载

Status: implemented

## Problem

pony_clean 已有真实门禁（秒级 pre-commit / 10秒级 pre-push / CI 分钟级），但
meta.yaml 声明 L1——机械判据只验到决策入册（L1），"承诺可验"（L2）的判据
（2.1 gates 实质文件 / 2.2 脚本可跑 / 2.3 负样本 spec / 2.4 本地钩子）未启用。
最值得守的机械承诺是"ADR 形状合法"（verify-note.sh 已在守），缺的是把它正式
落成 .meta/gates/ 门禁 + 负样本证明。

## Decision

升格 L2：

1. .meta/gates/notes-format.sh：决策格式门，代理执行框架交付的
   .agents/skills/write-adr/verify-note.sh（唯一真源，避免复制漂移）。
2. .meta/gates/notes-format.spec.sh：可执行负样本 spec（正 1 / 负 3——
   坏文件名 1.6、implemented 含 Proposal 1.8、Status 不符 1.7，真实构造并断言非零拒绝）。
3. .githooks/pre-commit 挂载门禁（秒级：fmt + 空白 + 决策格式门）。
4. gates/README 同步新模板（P7 三层教学）。
5. meta.yaml level: 2。

## Alternatives considered

- 只声明 L2 不建门禁：判据 2.1/2.3 立即 FAIL，假声明，否决。
- 新建自包含门禁脚本复制 verify-note 逻辑：复制即漂移，否决——代理执行唯一真源。
- 把 verify-note.sh 移到 .meta/gates/：它是 write-adr 技能的组成部分（技能自证），
  门禁代理引用即可，不搬移，否决。

## Consequences

- ponygo status 验 2.1-2.4：全 PASS（门禁 + spec + 钩子齐备）。
- 新决策提交时被秒级门拦形状违例；负样本 spec 随时可跑证明门未失效（门被门检验，P2）。
