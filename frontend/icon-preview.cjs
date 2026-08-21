// PonyClean icon preview pipeline (CommonJS — frontend is an ESM package).
// For each input SVG renders: 1) full-size transparent PNG, 2) multi-size
// legibility grid over light/dark backgrounds.
// Usage: node frontend/icon-preview.cjs [svg ...]   (default: all candidates)
const { chromium } = require('playwright-core');
const fs = require('fs');
const path = require('path');

const candDir = 'docs/design/icon-candidates';
const inputs = process.argv.slice(2).length
  ? process.argv.slice(2)
  : fs.readdirSync(candDir)
      .filter((f) => f.startsWith('candidate-') && f.endsWith('.svg'))
      .map((f) => path.join(candDir, f));

(async () => {
  const browser = await chromium.launch({ channel: 'msedge' });

  for (const svgPath of inputs) {
    const base = path.basename(svgPath, '.svg');
    const svg = fs.readFileSync(svgPath, 'utf8').replace('<svg ', '<svg style="display:block" ');

    // 1) full-size transparent render
    const page = await browser.newPage({ viewport: { width: 1024, height: 1024 } });
    await page.setContent(`<!doctype html><html><body style="margin:0;background:transparent">${svg}</body></html>`);
    const fullOut = path.join(candDir, `${base}-preview.png`);
    await page.screenshot({ path: fullOut, omitBackground: true, clip: { x: 0, y: 0, width: 1024, height: 1024 } });
    console.log('rendered', fullOut);

    // 2) size-legibility grid over light/dark backgrounds
    const sizes = [256, 128, 64, 32, 16];
    const bgs = [['light', '#f5f0e8'], ['dark', '#202124']];
    const page2 = await browser.newPage({ viewport: { width: 1200, height: 400 } });
    const cells = [];
    for (const [bgName, bg] of bgs) {
      for (const s of sizes) {
        cells.push(
          `<div style="width:200px;height:180px;display:flex;flex-direction:column;align-items:center;justify-content:center;background:${bg}">
             <div style="width:${s}px;height:${s}px">${svg.replace('width="1024" height="1024"', `width="${s}" height="${s}"`)}</div>
             <span style="font:11px sans-serif;color:#888;margin-top:6px">${s}px ${bgName}</span>
           </div>`
        );
      }
    }
    await page2.setContent(`<!doctype html><body style="margin:0;display:flex;flex-wrap:wrap">${cells.join('')}</body>`);
    const gridOut = path.join(candDir, `${base}-sizes.png`);
    await page2.screenshot({ path: gridOut, fullPage: true });
    console.log('rendered', gridOut);

    await page.close();
    await page2.close();
  }

  await browser.close();
})().catch((e) => { console.error(e); process.exit(1); });
