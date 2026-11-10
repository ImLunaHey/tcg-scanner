// Headless browser test with intercepted local assets. No server or external
// image requests; exercises the actual browser Web Worker + WASM build.
import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from '@playwright/test';
const [input, output] = process.argv.slice(2);
if (!output) throw new Error('Usage: node browser-smoke.mjs INPUT_JSON OUTPUT_JSON');
const root = path.dirname(new URL(import.meta.url).pathname);
const items = JSON.parse(await fs.readFile(input)).filter(x => ['current-006','regression-022','gap-004'].includes(x.id));
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  await page.route('**/*', async route => {
    const url = new URL(route.request().url());
    if (url.origin !== 'https://recognition.test') throw new Error(`Unexpected network request ${url}`);
    const pathname = url.pathname;
    if (pathname === '/') return route.fulfill({ contentType: 'text/html', body: '<script src="/tesseract.min.js"></script>' });
    let file;
    if (['/tesseract.min.js','/worker.min.js'].includes(pathname)) file = path.join(root, 'node_modules/tesseract.js/dist', path.basename(pathname));
    else if (pathname.startsWith('/core/')) file = path.join(root, 'node_modules/tesseract.js-core', path.basename(pathname));
    else if (pathname === '/lang/eng.traineddata') file = path.join(path.dirname(input), 'eng.traineddata');
    else if (pathname.startsWith('/image/')) file = items.find(x => x.id === path.basename(pathname))?.path;
    if (!file) return route.abort();
    const contentType = pathname.endsWith('.js') ? 'text/javascript' : pathname.endsWith('.wasm') ? 'application/wasm' : 'application/octet-stream';
    await route.fulfill({ contentType, body: await fs.readFile(file) });
  });
  await page.goto('https://recognition.test');
  const result = await page.evaluate(async ids => {
    const before = performance.now();
    const worker = await Tesseract.createWorker('eng', 1, { workerPath: '/worker.min.js', corePath: '/core', langPath: '/lang', gzip: false, workerBlobURL: false });
    await worker.setParameters({ tessedit_pageseg_mode: '11', preserve_interword_spaces: '1', user_defined_dpi: '300' });
    const startupSeconds = (performance.now() - before) / 1000;
    const results = [];
    for (const id of ids) {
      const start = performance.now();
      const { data } = await worker.recognize('/image/' + id);
      results.push({ id, text: data.text, seconds: (performance.now() - start) / 1000 });
    }
    await worker.terminate();
    return { startupSeconds, results };
  }, items.map(x => x.id));
  if (result.results.length !== 3 || !result.results.find(x => x.id === 'current-006')?.text.includes('Mewtwo') || !result.results.find(x => x.id === 'regression-022')?.text.includes('Dunsparce')) throw new Error('Browser OCR smoke failed');
  await fs.writeFile(output, JSON.stringify({ browser: browser.version(), ...result }, null, 2));
  console.log(JSON.stringify(result));
} finally { await browser.close(); }
