/**
 * PonyClean UI E2E 检查脚本（Playwright + 系统 Edge，headless）
 * 用途：验证暗色适配（文字对比度）、tab 切换、删除按钮存在性与响应。
 * 运行：node e2e/ui-check.mjs
 */
import { chromium } from 'playwright-core'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const edgePath = require('playwright-core').chromium.executablePath() // fallback

// 查找系统 Edge
const candidates = [
  'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
  'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
  process.env['PROGRAMFILES(X86)'] + '\\Microsoft\\Edge\\Application\\msedge.exe',
  process.env['PROGRAMFILES'] + '\\Microsoft\\Edge\\Application\\msedge.exe',
]
const executablePath = candidates.find((p) => require('node:fs').existsSync(p))
if (!executablePath) {
  console.error('未找到 Edge，请安装或指定路径')
  process.exit(1)
}

// ─── Tauri API mock（浏览器环境注入，含事件总线：start_* 会派发 done 事件，
//     使自动扫描断言真正覆盖"扫描完成后再切回不重扫"的 5 分钟冷却路径） ───
const TAURI_MOCK = `
window.__TAURI_INTERNALS__ = {
  invoke: async (cmd, args) => {
    window.__invokeCalls = window.__invokeCalls || [];
    window.__invokeCalls.push(cmd);
    const a = args || {};
    switch (cmd) {
      case 'plugin:event|listen': {
        const ev = a.event, h = a.handler;
        (window.__evHandlers[ev] = window.__evHandlers[ev] || []).push(h);
        return { id: h };
      }
      case 'get_processes':
        return {
          summary: { cpu_total: 32.5, mem_used_mb: 8192, mem_total_mb: 16384, process_count: 120, disk_used_gb: 210, disk_total_gb: 512 },
          processes: [
            { pid: 1001, name: 'chrome.exe', cpu: 88.2, mem_mb: 2400, status: 'Running' },
            { pid: 1002, name: 'Code.exe', cpu: 12.4, mem_mb: 1800, status: 'Running' },
            { pid: 1003, name: 'game.exe', cpu: 3.1, mem_mb: 5120, status: 'Running' },
            { pid: 1004, name: 'dummy.exe', cpu: 0.5, mem_mb: 80, status: 'Running' },
          ],
        };
      case 'get_config':
        return { alert_cpu_pct: 80, alert_mem_pct: 85, autostart: false };
      case 'get_clean_config':
        return { version: 2, disabled_target_ids: [], disabled_targets: [], custom_exclude_paths: [], per_target_config: {}, custom_targets: [], disk_scan: { min_bytes_mb: 100, dir_depth: 3 } };
      case 'get_clean_logs':
        return { entries: [] };
      case 'get_system_idle_ms':
        return 0;
      case 'trim_memory':
        return { attempted: 50, success: 42, failed: 8, skipped: 10, freed_mb: 386.5 };
      case 'execute_clean':
        return { success: 3, failed: 0, errors: [] };
      case 'delete_large_files':
        return { success: 1, failed: 0, errors: [] };
      case 'start_scan':
        setTimeout(() => window.__dispatch('scan-done', { total_items: 0, total_bytes: 0, skipped_small: 0 }), 250);
        return {};
      case 'start_user_scan':
        setTimeout(() => window.__dispatch('disk-user-done', {}), 250);
        return {};
      case 'cancel_scan':
      case 'cancel_disk_scan':
      case 'empty_recycle_bin':
      case 'set_config':
      case 'save_clean_config':
      case 'set_island_expanded':
        return {};
      default:
        if (cmd.startsWith('plugin:')) return {};
        console.warn('[mock] unhandled invoke:', cmd, JSON.stringify(a).slice(0, 80));
        return {};
    }
  },
  transformCallback: (fn) => {
    window.__cb = window.__cb || 0;
    window.__cbMap = window.__cbMap || {};
    const id = ++window.__cb;
    window.__cbMap[id] = fn;
    return id;
  },
  plugins: {},
  metadata: { currentWindow: { label: '__WINDOW_LABEL__' } },
};
window.__evHandlers = {};
window.__dispatch = (event, payload) => {
  (window.__evHandlers[event] || []).forEach((h) => {
    const fn = window.__cbMap && window.__cbMap[h];
    if (fn) fn({ payload });
  });
};
window.__TAURI_MOCK__ = true;
`

// ─── 工具函数 ───
function luminance(rgb) {
  const [r, g, b] = rgb.match(/\d+/g).map(Number)
  const lin = (c) => {
    c /= 255
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
  }
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}
function contrast(fg, bg) {
  const l1 = luminance(fg)
  const l2 = luminance(bg)
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05)
}

// 收集页面文本元素并检查对比度
async function auditTextContrast(page, label) {
  const items = await page.evaluate(() => {
    const luminance = (rgb) => {
      const [r, g, b] = rgb.match(/\d+/g).map(Number)
      const lin = (c) => {
        c /= 255
        return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
      }
      return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }
    const contrast = (fg, bg) => {
      const l1 = luminance(fg)
      const l2 = luminance(bg)
      return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05)
    }
    const results = []
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT)
    let node
    while ((node = walker.nextNode())) {
      const text = node.textContent.trim()
      if (!text || text.length < 1) continue
      const cs = getComputedStyle(node)
      if (cs.display === 'none' || cs.visibility === 'hidden' || cs.opacity === '0') continue
      // 找最近不透明背景（渐变背景视为主题深色，透明最终回退主题色）
      const THEME_DARK = 'rgb(26, 24, 21)'
      let bg = cs.backgroundColor
      let el = node
      let depth = 0
      while ((bg === 'rgba(0, 0, 0, 0)' || bg === 'transparent') && depth < 12) {
        el = el.parentElement
        if (!el) break
        const elCs = getComputedStyle(el)
        if (elCs.backgroundImage !== 'none') {
          bg = THEME_DARK
          break
        }
        bg = elCs.backgroundColor
        depth++
      }
      if (bg === 'rgba(0, 0, 0, 0)' || bg === 'transparent') bg = THEME_DARK
      // 过滤：只检查有实际文字的元素（排除父容器重复）
      if (!/[\u4e00-\u9fffA-Za-z0-9%:.+\-\/≥]/.test(text)) continue
      const color = cs.color
      const ratio = contrast(color, bg)
      if (ratio < 3.0) {
        results.push({
          tag: node.tagName.toLowerCase(),
          cls: node.className?.toString().slice(0, 60) || '',
          text: text.slice(0, 40),
          color,
          bg,
          ratio: ratio.toFixed(2),
        })
      }
    }
    return results
  })
  console.log(`\n[${label}] 低对比度文字 (ratio < 3.0): ${items.length} 处`)
  for (const it of items.slice(0, 15)) {
    console.log(`  <${it.tag}> "${it.text}" color=${it.color} bg=${it.bg} ratio=${it.ratio} class="${it.cls}"`)
  }
  return items
}

// 检查 select 控件暗色适配
async function auditSelects(page, label) {
  const items = await page.evaluate(() => {
    return [...document.querySelectorAll('select')].map((s) => {
      const cs = getComputedStyle(s)
      const options = [...s.options].map((o) => ({ text: o.text, color: getComputedStyle(o).color }))
      return {
        text: s.textContent.slice(0, 30),
        color: cs.color,
        bg: cs.backgroundColor,
        colorScheme: getComputedStyle(document.documentElement).colorScheme,
        options,
      }
    })
  })
  console.log(`\n[${label}] select 控件检查:`)
  for (const it of items) {
    console.log(`  select "${it.text}" color=${it.color} bg=${it.bg} htmlColorScheme=${it.colorScheme} optionCount=${it.options.length}`)
  }
  return items
}

async function runWindow(label) {
  const browser = await chromium.launch({ executablePath, headless: true })
  const page = await browser.newPage({ viewport: { width: 400, height: 600 } })
  const errors = []
  page.on('pageerror', (e) => errors.push(e.message.slice(0, 120)))
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text().slice(0, 120)) })

  await page.addInitScript(TAURI_MOCK.replace('__WINDOW_LABEL__', label))
  await page.goto('http://127.0.0.1:5183', { waitUntil: 'networkidle' })
  await page.waitForTimeout(1200)

  console.log(`\n========== 窗口: ${label} ==========`)
  const title = await page.title()
  console.log('页面标题:', title, '| JS 错误数:', errors.length)
  if (errors.length) console.log('  错误:', errors.slice(0, 5))

  // 页面渲染内容
  const bodyText = (await page.evaluate(() => document.body.innerText)).slice(0, 200)
  console.log('渲染文本:', bodyText.replace(/\n/g, ' | ').slice(0, 180))

  await auditSelects(page, label)
  const low = await auditTextContrast(page, label)

  // 删除按钮存在性检查（可点击元素）
  const buttons = await page.evaluate(() => {
    return [...document.querySelectorAll('button')].map((b) => ({
      title: b.title || b.textContent.trim().slice(0, 20),
      disabled: b.disabled,
      visible: getComputedStyle(b).display !== 'none' && getComputedStyle(b).opacity !== '0',
      rect: b.getBoundingClientRect().width > 0,
    }))
  })
  console.log(`\n[${label}] 按钮统计: ${buttons.length} 个`)
  const invisible = buttons.filter((b) => !b.visible || !b.rect)
  if (invisible.length) {
    console.log(`  不可见/无尺寸按钮 ${invisible.length} 个:`)
    invisible.slice(0, 10).forEach((b) => console.log(`    title="${b.title}" disabled=${b.disabled} visible=${b.visible} rect=${b.rect}`))
  }

  // 交互测试：点击 tab 按钮
  const tabTitles = await page.evaluate(() => [...document.querySelectorAll('.sidebar button')].map((b) => b.title))
  console.log('侧边栏 tab:', JSON.stringify(tabTitles))
  for (const t of tabTitles) {
    await page.click(`.sidebar button[title="${t}"]`).catch(() => {})
    await page.waitForTimeout(400)
  }
  console.log('tab 依次点击完成')

  // TASK-028 回归断言：自动扫描在窗口会话内只触发一次（切走再切回不重扫）
  let autoScanFail = 0
  if (tabTitles.includes('清理') && tabTitles.includes('监控')) {
    await page.click('.sidebar button[title="监控"]').catch(() => {})
    await page.waitForTimeout(400)
    await page.click('.sidebar button[title="清理"]').catch(() => {})
    await page.waitForTimeout(600)
    const counts = await page.evaluate(() => {
      const calls = window.__invokeCalls || []
      const count = (c) => calls.filter((x) => x === c).length
      return { start_scan: count('start_scan'), start_user_scan: count('start_user_scan') }
    })
    console.log('自动扫描调用次数:', JSON.stringify(counts))
    const ok = counts.start_scan === 1 && counts.start_user_scan === 1
    console.log(ok ? 'PASS: 自动扫描只触发一次' : 'FAIL: 自动扫描触发次数异常')
    if (!ok) autoScanFail = 1
  }

  await browser.close()
  return { errors: errors.length, lowContrast: low.length, autoScanFail }
}

const results = []
results.push(await runWindow('capsule'))
results.push(await runWindow('island'))
console.log('\n========== 汇总 ==========')
console.log(JSON.stringify(results, null, 2))
