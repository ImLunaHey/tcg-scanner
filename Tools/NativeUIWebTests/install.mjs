// Development-only HTTPS service-worker/offline verification.
import { chromium } from 'playwright';
const [origin] = process.argv.slice(2);
if (!origin?.startsWith('https://')) throw new Error('Usage: node install.mjs HTTPS_TAILSCALE_ORIGIN');
const browser = await chromium.launch({ headless: true, args: ['--no-proxy-server', '--enable-unsafe-swiftshader'] });
try {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 } });
  const page = await context.newPage(), errors = [];
  page.setDefaultTimeout(20000);
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${origin}/camera`);
  await page.getByRole('button', { name: 'Start camera', exact: true }).waitFor();
  await page.waitForFunction(async () => {
    const registration = await navigator.serviceWorker.getRegistration('/camera');
    return registration?.active?.state === 'activated';
  });
  const manifest = await (await page.request.get(`${origin}/camera.webmanifest`)).json();
  if (manifest.start_url !== '/camera' || manifest.scope !== '/camera' || manifest.display !== 'standalone') throw new Error('Wrong installed camera entry');
  for (const icon of manifest.icons) {
    const response = await page.request.get(new URL(icon.src, origin).href);
    if (!response.ok() || response.headers()['content-type'] !== 'image/png') throw new Error('Missing install icon');
  }
  // An authenticated-only route still rejects anonymous callers through the SW.
  const privateStatus = await page.evaluate(async () => (await fetch('/api/v1/status')).status);
  if (privateStatus !== 401) throw new Error('Unexpected anonymous library authority');
  await context.setOffline(true);
  const response = await page.goto(`${origin}/camera`);
  if (response.status() !== 503) throw new Error('Expected offline navigation explanation');
  await page.getByRole('heading', { name: 'Scanner is offline' }).waitFor().catch(async error => { console.error({ html: await page.content(), response: await response.text() }); throw error; });
  const storage = await page.evaluate(async () => ({ caches: await caches.keys(), databases: await indexedDB.databases(), local: localStorage.length, session: sessionStorage.length }));
  if (storage.caches.length || storage.databases.length || storage.local || storage.session) throw new Error('Camera created application storage');
  await context.setOffline(false);
  await page.getByRole('link', { name: 'Try again' }).click();
  await page.getByRole('button', { name: 'Start camera', exact: true }).waitFor();
  if (errors.length) throw new Error(JSON.stringify(errors));
  console.log(JSON.stringify({ scope: manifest.scope, offlineStatus: response.status(), storage, errors }, null, 2));
} finally { await browser.close(); }
