/**
 * PonyClean 端到端交互测试 v2 — 删除按钮响应 + 暗色适配
 * 运行：node e2e/ui-interact.mjs
 * 覆盖：清理面板（扫描→勾选→删除→确认弹窗）、分析面板（大文件→删除二次确认）、
 *       kill 按钮可见性、select 暗色适配
 */
import { chromium } from 'playwright-core'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const fs = require('node:fs')
const candidates = [
  'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
  'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
  process.env['PROGRAMFILES(X86)'] + '\\Microsoft\\Edge\\Application\\msedge.exe',
  process.env['PROGRAMFILES'] + '\\Microsoft\\Edge\\Application\\msedge.exe',
]
const executablePath = candidates.find((p) => fs.existsSync(p))
if (!executablePath) { console.error('未找到 Edge'); process.exit(1) }

const TAURI_MOCK = `
window.__cbStore = [];
window.__evtListeners = {};
function __fire(event, payload) {
  const h = window.__evtListeners[event];
  if (h != null && window.__cbStore[h]) window.__cbStore[h]({ event, payload });
}
window.__TAURI_INTERNALS__ = {
  invoke: async (cmd, args) => {
    const a = args || {};
    switch (cmd) {
      case 'get_processes':
        return { summary: { cpu_total: 32.5, mem_used_mb: 8192, mem_total_mb: 16384, process_count: 120, disk_used_gb: 210, disk_total_gb: 512 },
          processes: [
            { pid: 1001, name: 'chrome.exe', cpu: 88.2, mem_mb: 2400, status: 'Running' },
            { pid: 1002, name: 'Code.exe', cpu: 12.4, mem_mb: 1800, status: 'Running' },
            { pid: 1003, name: 'game.exe', cpu: 3.1, mem_mb: 5120, status: 'Running' },
          ] };
      case 'get_config': return { alert_cpu_pct: 80, alert_mem_pct: 85, autostart: false };
      case 'get_clean_config': return { version: 2, disabled_target_ids: [], disabled_targets: [], custom_exclude_paths: [], per_target_config: {}, custom_targets: [] };
      case 'get_clean_logs': return { entries: [] };
      case 'get_system_idle_ms': return 0;
      case 'trim_memory': return { attempted: 50, success: 42, failed: 8, skipped: 10, freed_mb: 386.5 };
      case 'execute_clean': return { success: 3, failed: 0, errors: [] };
      case 'delete_large_files': return { success: 1, failed: 0, errors: [] };
      case 'plugin:event|listen':
        window.__evtListeners[a.event] = a.handler;
        return () => {};
      case 'start_scan':
        setTimeout(() => {
          __fire('scan-items', { items: [
            { path: 'C:\\\\Users\\\\tester\\\\AppData\\\\Local\\\\Temp\\\\tmp1.log', size_bytes: 40960, level: 'safe', category: 'temp' },
            { path: 'C:\\\\Users\\\\tester\\\\AppData\\\\Local\\\\Temp\\\\tmp2.tmp', size_bytes: 81920, level: 'safe', category: 'temp' },
            { path: 'C:\\\\Users\\\\tester\\\\Downloads\\\\old_installer.exe', size_bytes: 104857600, level: 'confirm', category: 'old_install' },
          ], total_bytes: 104988672 });
          __fire('scan-done', { total_items: 3, total_bytes: 104988672, skipped_small: 5 });
        }, 400);
        return {};
      case 'cancel_scan': return {};
      case 'start_large_scan':
        setTimeout(() => {
          __fire('disk-large-files', { files: [
            { path: 'C:\\\\Users\\\\tester\\\\Downloads\\\\movie.mkv', size_bytes: 2147483648, modified_secs: 1700000000, kind: 'video' },
            { path: 'C:\\\\Users\\\\tester\\\\Documents\\\\backup.zip', size_bytes: 734003200, modified_secs: 1690000000, kind: 'archive' },
            { path: 'C:\\\\Users\\\\tester\\\\Desktop\\\\setup.exe', size_bytes: 629145600, modified_secs: 1680000000, kind: 'installer' },
          ] });
          __fire('disk-done', {});
        }, 400);
        return {};
      case 'start_dir_scan':
        setTimeout(() => {
          __fire('disk-dir-usage', { dirs: [
            { path: 'C:\\\\Users\\\\tester\\\\Downloads', size_bytes: 4294967296, file_count: 1200 },
            { path: 'C:\\\\Users\\\\tester\\\\Documents', size_bytes: 2147483648, file_count: 800 },
            { path: 'C:\\\\Users\\\\tester\\\\Desktop', size_bytes: 1073741824, file_count: 60 },
          ] });
          __fire('disk-done', {});
        }, 400);
        return {};
      case 'cancel_disk_scan': return {};
      case 'set_config': return {};
      case 'save_clean_config': return {};
      case 'set_island_expanded': return {};
      default:
        if (cmd.startsWith('plugin:')) return {};
        console.warn('[mock] unhandled:', cmd);
        return {};
    }
  },
  transformCallback: (fn) => { window.__cbStore.push(fn); return window.__cbStore.length - 1; },
  plugins: {},
  metadata: { currentWindow: { label: '__WINDOW_LABEL__' } },
};
window.__TAURI_MOCK__ = true;
`

const browser = await chromium.launch({ executablePath, headless: true })
const page = await browser.newPage({ viewport: { width: 400, height: 640 } })
const jsErrors = []
page.on('pageerror', (e) => jsErrors.push(e.message.slice(0, 100)))
page.on('console', (m) => { if (m.type() === 'error') jsErrors.push(m.text().slice(0, 100)) })

await page.addInitScript(TAURI_MOCK.replace('__WINDOW_LABEL__', 'island'))
await page.goto('http://127.0.0.1:5183', { waitUntil: 'networkidle' })
await page.waitForTimeout(800)
// 模拟胶囊点击：触发 island-enter 使面板滑入可见
await page.evaluate(() => {
  const h = window.__evtListeners['island-enter']
  if (h != null && window.__cbStore[h]) window.__cbStore[h]({})
})
await page.waitForTimeout(800)

let pass = 0, fail = 0
const ok = (cond, msg) => { cond ? (pass++, console.log(`  [PASS] ${msg}`)) : (fail++, console.log(`  [FAIL] ${msg}`)) }

// ─── 1. 监控 tab：kill 按钮可见性 ───
console.log('\n[1] 监控面板 — kill 按钮')
await page.click('.sidebar button[title="监控"]', { timeout: 3000 }).catch(() => {})
await page.waitForTimeout(500)
const killVisible = await page.evaluate(() => {
  const btn = [...document.querySelectorAll('button')].find((b) => (b.title || '').startsWith('终止'))
  return btn ? { visible: getComputedStyle(btn).opacity !== '0' && btn.getBoundingClientRect().width > 0, title: btn.title } : null
})
ok(killVisible?.visible === true, `kill 按钮可见（title="${killVisible?.title}"）`)

// ─── 2. 清理 tab：扫描 → 分组 → 勾选 → 删除弹窗 ───
console.log('\n[2] 清理面板 — 删除流程')
await page.click('.sidebar button[title="清理"]', { timeout: 3000 }).catch(() => {})
await page.waitForTimeout(600)
const panelText = await page.evaluate(() => document.body.innerText.slice(0, 150))
console.log(`  面板文本: ${panelText.replace(/\n/g, ' | ')}`)
const scanBtn = await page.evaluate(() => {
  const b = document.querySelector('button[title="开始扫描"]')
  return b ? { title: b.title, rect: b.getBoundingClientRect().width > 0 } : null
})
ok(scanBtn?.rect === true, `扫描按钮存在（title="${scanBtn?.title}"）`)
// 点击扫描
const clicked = await page.evaluate(() => {
  const b = document.querySelector('button[title="开始扫描"]')
  if (b) { b.click(); return true }
  return false
})
ok(clicked, '点击扫描按钮')
await page.waitForTimeout(1500)
const scannedState = await page.evaluate(() => document.body.innerText.includes('可清理'))
ok(scannedState, '扫描完成显示「可清理」')
// 勾选第一项（分类级 Checkbox：button[role=checkbox]，扫描结果分组头部）
const checked = await page.evaluate(() => {
  const cb = document.querySelector('button[role="checkbox"]')
  if (cb) { cb.click(); return true }
  return false
})
ok(checked, '勾选清理项')
await page.waitForTimeout(200)
const selectedText = await page.evaluate(() => {
  const t = document.body.innerText
  const m = t.match(/已选 (\d+) 项/)
  return m ? m[1] : null
})
console.log(`  已选: ${selectedText} 项`)
// 点击删除按钮 → 应出现确认弹窗
const deleteClicked = await page.evaluate(() => {
  const btn = [...document.querySelectorAll('button')].find((b) => b.title === '清理选中' || b.textContent.includes('清理'))
  if (btn) { btn.click(); return true }
  return false
})
ok(deleteClicked, '点击清理按钮')
await page.waitForTimeout(500)
const confirmDialog = await page.evaluate(() => document.body.innerText.includes('确认清理'))
ok(confirmDialog, '出现「确认清理」弹窗')

// ─── 3. 分析 tab：大文件 → 行内删除二次确认 ───
console.log('\n[3] 分析面板 — 大文件删除')
await page.click('.sidebar button[title="分析"]', { timeout: 3000 }).catch(() => {})
await page.waitForTimeout(600)
// 大文件视图的扫描按钮：OptionPicker 所在行内、非 picker 自身的按钮
const startLarge2 = await page.evaluate(() => {
  const pickers = [...document.querySelectorAll('.option-picker')]
  for (const p of pickers) {
    if (p.textContent.includes('MB')) {
      const row = p.parentElement
      const btns = row ? [...row.querySelectorAll('button')] : []
      const scanBtn = btns.find((b) => !b.closest('.option-picker'))
      if (scanBtn) { scanBtn.click(); return true }
    }
  }
  return false
})
ok(startLarge2, '点击大文件扫描按钮')
await page.waitForTimeout(1500)
const hasLargeFiles = await page.evaluate(() => document.body.innerText.includes('movie.mkv'))
ok(hasLargeFiles, '大文件列表渲染（movie.mkv 2.0 GB）')
// 行内删除：第一次点击 → 确认态（title 变「再次点击确认删除」）
const firstClick = await page.evaluate(() => {
  const row = [...document.querySelectorAll('div')].find((d) => d.textContent.includes('movie.mkv'))
  const delBtn = row?.querySelector('button[title="删除"]')
  if (delBtn) { delBtn.click(); return true }
  return false
})
ok(firstClick, '行内删除按钮点击（第一次）')
await page.waitForTimeout(200)
const confirmState = await page.evaluate(() => {
  const row = [...document.querySelectorAll('div')].find((d) => d.textContent.includes('movie.mkv'))
  const btn = row?.querySelector('button[title="再次点击确认删除"]')
  return btn ? btn.title : null
})
ok(confirmState === '再次点击确认删除', `进入确认态（title="${confirmState}"）`)
// 第二次点击 → 执行删除
const secondClick = await page.evaluate(() => {
  const row = [...document.querySelectorAll('div')].find((d) => d.textContent.includes('movie.mkv'))
  const btn = row?.querySelector('button[title="再次点击确认删除"]')
  if (btn) { btn.click(); return true }
  return false
})
ok(secondClick, '第二次点击执行删除')
await page.waitForTimeout(600)
const deleted = await page.evaluate(() => !document.body.innerText.includes('movie.mkv'))
ok(deleted, '删除后列表移除 movie.mkv')

// ─── 4. 目录占用 ───
console.log('\n[4] 分析面板 — 目录占用')
const before4 = await page.evaluate(() => {
  const active = [...document.querySelectorAll('.sidebar button')].find((b) => b.className.includes('bg-primary/15'))?.title
  return { active, hasAnalysis: document.body.innerText.includes('目录占用') }
})
console.log(`  [4] 开始时 activeTab=${before4.active} 有分析面板=${before4.hasAnalysis}`)
const dirTab = await page.evaluate(() => {
  const btns = [...document.querySelectorAll('button')]
  const b = btns.find((x) => x.textContent.includes('目录占用'))
  if (b) { b.click(); return true }
  return false
})
ok(dirTab, '切换到目录占用视图')
await page.waitForTimeout(300)
const afterTab = await page.evaluate(() => {
  const active = [...document.querySelectorAll('.sidebar button')].find((b) => b.className.includes('bg-primary/15'))?.title
  return { active, text: document.body.innerText.slice(0, 80).replace(/\n/g, ' | ') }
})
console.log(`  点击后 activeTab=${afterTab.active} 文本=${afterTab.text}`)
// 目录扫描按钮：找包含「用户目录」文本的最小容器（无子 div）内的按钮
const dirScan = await page.evaluate(() => {
  const divs = [...document.querySelectorAll('div')]
  const row = divs.find((d) => d.textContent.includes('用户目录') && !d.querySelector('div') && d.querySelector('button'))
  const btn = row?.querySelector('button')
  if (btn) { btn.click(); return true }
  return false
})
ok(dirScan, '点击目录扫描')
await page.waitForTimeout(1500)
const dirText = await page.evaluate(() => document.body.innerText.slice(0, 150).replace(/\n/g, ' | '))
console.log(`  目录视图文本: ${dirText}`)
const dirsShown = await page.evaluate(() => document.body.innerText.includes('Downloads'))
ok(dirsShown, '目录占用榜单渲染（Downloads）')

// ─── 5. 设置 tab：OptionPicker 暗色适配（原生 select 已移除） ───
console.log('\n[5] 设置面板 — OptionPicker 暗色适配')
await page.click('.sidebar button[title="设置"]', { timeout: 3000 }).catch(() => {})
await page.waitForTimeout(500)
// 页面不应再有任何原生 select
const nativeSelects = await page.evaluate(() => document.querySelectorAll('select').length)
ok(nativeSelects === 0, `原生 select 已移除（当前 ${nativeSelects} 个）`)
// 点击「+」展开添加规则表单，等待 OptionPicker 出现
await page.evaluate(() => {
  const btns = [...document.querySelectorAll('button')]
  const plus = btns.find((b) => b.title === '添加规则')
  if (plus) { plus.click(); return true }
  return false
})
// 轮询等待 OptionPicker 渲染（最多 2s）
let pickerCount = 0
for (let i = 0; i < 10; i++) {
  pickerCount = await page.evaluate(() => document.querySelectorAll('.option-picker').length)
  if (pickerCount > 0) break
  await page.waitForTimeout(200)
}
ok(pickerCount >= 1, `OptionPicker 数量: ${pickerCount}`)
// 点击第一个 OptionPicker 展开选项列表，检查暗色
const pickerAudit = await page.evaluate(() => {
  const rootCs = getComputedStyle(document.documentElement)
  const picker = document.querySelector('.option-picker button')
  const cs = getComputedStyle(picker)
  // 展开
  picker.click()
  return { colorScheme: rootCs.colorScheme, btnColor: cs.color }
})
await page.waitForTimeout(200)
const optionList = await page.evaluate(() => {
  const opts = [...document.querySelectorAll('.option-picker .absolute button')]
  return opts.map((o) => getComputedStyle(o).color)
})
ok(pickerAudit.colorScheme === 'dark', `html color-scheme: ${pickerAudit.colorScheme}`)
ok(pickerAudit.btnColor !== 'rgb(0, 0, 0)', `OptionPicker 按钮文字色: ${pickerAudit.btnColor}`)
ok(optionList.length > 0 && optionList.every((c) => c !== 'rgb(0, 0, 0)'), `选项列表文字色: ${JSON.stringify(optionList.slice(0, 3))}`)

// ─── 汇总 ───
console.log(`\n========== 汇总: ${pass} 通过 / ${fail} 失败 ==========`)
if (jsErrors.length) {
  console.log('JS 错误:', jsErrors.slice(0, 5))
}
await browser.close()
process.exit(fail > 0 ? 1 : 0)
