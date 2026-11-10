// Cold browser contexts verify the first Rune frame without credentials or hardware.
import { chromium } from 'playwright';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const [origin, count = '3'] = process.argv.slice(2);
const attempts = Number(count);
if (!Number.isInteger(attempts) || attempts < 1 || attempts > 20) throw new Error('Use 1–20 cold starts');
if (!origin) throw new Error('Usage: node startup.mjs HTTPS_ORIGIN');
const timings = [];
for (let i = 0; i < attempts; i++) {
  const browser = await chromium.launch({ headless: true, args: ['--no-proxy-server', '--enable-unsafe-swiftshader'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1180, height: 900 } });
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    page.on('requestfailed', r => errors.push(`${r.url()}: ${r.failure()?.errorText}`));
    const start = Date.now();
    const navigation = await page.goto(origin, { waitUntil: 'networkidle', timeout: 15000 });
    await page.getByText('Access token', { exact: true }).waitFor({ timeout: 15000 }).catch(async error => {
      console.error(JSON.stringify({ attempt: i + 1, navigationStatus: navigation?.status(), errors, text: await page.locator('body').innerText(), canvas: await page.locator('canvas').count() }));
      await page.screenshot({ path: join(tmpdir(), `scanner-startup-${i + 1}-failure.png`) });
      throw error;
    });
    await page.getByRole('button', { name: 'Connect', exact: true }).waitFor();
    if (errors.length) throw new Error(errors.join('; '));
    timings.push(Date.now() - start);
  } finally { await browser.close(); }
}
console.log(JSON.stringify({ coldStarts: timings.length, milliseconds: timings }));
