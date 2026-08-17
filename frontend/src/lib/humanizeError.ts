/**
 * 删除失败错误中文化（TASK-028）。
 *
 * 后端 `DeleteResult.errors` 是未脱敏的原文（含完整路径），此处先剥离路径再
 * 前缀/包含匹配为中文。中文规则优先于英文规则（后端部分错误已中文）。
 * 匹配不到时截断保留原文，绝不吞掉失败信息（失败计数仍由调用方展示）。
 */

/** 剥离 Windows 路径（盘符路径或 UNC 路径），替换为省略号，防 toast 透出用户全路径 */
export function stripPath(raw: string): string {
  return raw
    .replace(/[A-Za-z]:[\\/][^\s]*/g, '…')
    .replace(/\\\\[^\s]*/g, '…')
    .trim()
}

/** 中文包含匹配（优先，后端已中文化的错误） */
const ZH_RULES: Array<[RegExp, string]> = [
  [/文件被进程占用/, '文件被进程占用，已安排重启后删除'],
  [/服务无法停止/, '系统服务无法停止，相关文件已跳过'],
  [/无法停止服务/, '无法停止系统服务，相关文件已跳过'],
  [/删除后无法恢复服务/, '清理后系统服务未能自动恢复'],
  [/无法恢复服务/, '清理后系统服务未能自动恢复'],
]

/** 英文前缀/包含匹配（兜底） */
const EN_RULES: Array<[RegExp, string]> = [
  [/cannot resolve path/i, '文件无法访问，已跳过'],
  [/protected path/i, '受保护路径，已跳过'],
  [/not in scan scope/i, '不在可清理范围，已跳过'],
  [/outside scan root/i, '超出扫描范围，已跳过'],
  [/system file not deletable/i, '系统文件不可删除'],
  [/path contains null byte/i, '非法路径，已跳过'],
  [/movefileexw failed/i, '延迟删除安排失败'],
  [/scan already in progress/i, '扫描已在进行中'],
  [/no scan in progress/i, '没有进行中的扫描'],
  [/no scan targets available/i, '没有可扫描的目标'],
]

export function humanizeError(raw: string): string {
  const s = stripPath(raw)
  for (const [re, zh] of ZH_RULES) {
    if (re.test(s)) return zh
  }
  for (const [re, zh] of EN_RULES) {
    if (re.test(s)) return zh
  }
  return s.slice(0, 60)
}

/** 批量映射并去重（toast 展示用） */
export function humanizeErrors(errors: string[], max = 5): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const e of errors) {
    const zh = humanizeError(e)
    if (seen.has(zh)) continue
    seen.add(zh)
    out.push(zh)
    if (out.length >= max) break
  }
  return out
}
