// Render the final SVG master to a transparent 1024px PNG (CommonJS).
// Usage: node frontend/render-master.cjs <input.svg> <output.png>
const { chromium } = require('playwright-core');
const fs = require('fs');

(async () => {
  const [, , svgPath, outPath] = process.argv;
  const svg = fs.readFileSync(svgPath, 'utf8').replace('<svg ', '<svg style="display:block" ');
  const browser = await chromium.launch({ channel: 'msedge' });
  const page = await browser.newPage({ viewport: { width: 1024, height: 1024 } });
  await page.setContent(`<!doctype html><html><body style="margin:0;background:transparent">${svg}</body></html>`);
  await page.screenshot({ path: outPath, omitBackground: true, clip: { x: 0, y: 0, width: 1024, height: 1024 } });
  await browser.close();
  console.log('rendered', outPath);
})().catch((e) => { console.error(e); process.exit(1); });
