// Development-only browser verification against scanner-http's synthetic fixture.
import { chromium } from 'playwright';
import { readFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
const [origin, tokenFile, screenshots] = process.argv.slice(2);
if (!origin || !tokenFile) throw new Error('Usage: node flow.mjs SERVICE_ORIGIN PRIVATE_TOKEN_FILE [SCREENSHOT_DIRECTORY]');
const token = (await readFile(tokenFile, 'utf8')).trim();
if (!/^[a-f0-9]{64}$/.test(token)) throw new Error('Invalid fixture credential');
const browser = await chromium.launch({ headless: true, args: ['--no-proxy-server', '--enable-unsafe-swiftshader'] });
try {
  const page = await browser.newPage({ viewport: { width: 1180, height: 900 }, deviceScaleFactor: 1 });
  const errors = [], methods = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  page.on('request', request => { if (request.url() === `${origin}/rpc`) methods.push(request.postDataJSON().method); });
  await page.goto(origin, { waitUntil: 'networkidle', timeout: 15000 });
  const label = await page.getByText('Access token', { exact: true }).boundingBox();
  if (!label) throw new Error('Missing credential field label');
  // Rune's text entry is drawn on the canvas; wait for its secure native input bridge.
  await page.mouse.click(label.x + 80, label.y + label.height + 40);
  await page.waitForFunction(() => document.activeElement.type === 'password', {}, { timeout: 3000 });
  await page.keyboard.insertText(token);
  const thumbnail = await Promise.all([
    response(page, 'v1.images.thumbnail'),
    page.getByRole('button', { name: 'Connect', exact: true }).dispatchEvent('click'),
  ]).then(([response]) => response.json());
  if (!thumbnail.result?.sha256) throw new Error('The service did not return a verified thumbnail');
  await page.getByText('Browser test card', { exact: true }).first().waitFor();
  if (await page.getByRole('button', { name: 'Finish unverified', exact: true }).count())
    await page.getByRole('button', { name: 'Finish unverified', exact: true }).dispatchEvent('click');
  if (await page.getByRole('button', { name: 'Condition unassessed', exact: true }).count())
    await page.getByRole('button', { name: 'Condition unassessed', exact: true }).dispatchEvent('click');
  const reviewed = await Promise.all([
    response(page, 'v1.images.review'),
    page.getByRole('button', { name: 'Confirm card', exact: true }).dispatchEvent('click'),
  ]).then(([response]) => response.json());
  const result = reviewed.result;
  if (result?.state !== 'confirmed' || result.selectedCardID !== 'fixture-1' || result.finish !== 'normal' || result.condition !== 'nearMint')
    throw new Error('The browser did not save the explicit fixture review');
  const inventory = await Promise.all([
    response(page, 'v1.inventory.list'),
    page.getByRole('button', { name: 'Inventory', exact: true }).dispatchEvent('click'),
  ]).then(([response]) => response.json());
  if (inventory.result?.totalCopies !== 1) throw new Error('Synthetic inventory must contain one physical copy');
  await page.getByRole('button', { name: /^Open batch / }).first().waitFor();
  if (screenshots) {
    await mkdir(screenshots, { recursive: true });
    await page.screenshot({ path: join(screenshots, 'inventory.png') });
  }
  await Promise.all([
    response(page, 'v1.jobs.get'),
    page.getByRole('button', { name: /^Open batch / }).first().dispatchEvent('click'),
  ]);
  await Promise.all([
    response(page, 'v1.profiles.list'),
    page.getByRole('button', { name: 'Settings', exact: true }).dispatchEvent('click'),
  ]);
  await page.getByRole('button', { name: 'New profile', exact: true }).dispatchEvent('click');
  await enter(page, 'Profile name', 'Browser fixture profile');
  await enter(page, 'DPI', '300');
  const created = await Promise.all([
    response(page, 'v1.profiles.create'),
    page.getByRole('button', { name: 'Save profile', exact: true }).dispatchEvent('click'),
  ]).then(([response]) => response.json());
  if (created.result?.dpi !== 300 || !created.result?.id) throw new Error('Profile creation failed');
  await page.getByText('Profile saved.', { exact: true }).waitFor();
  await enter(page, 'DPI', '600');
  const updated = await Promise.all([
    response(page, 'v1.profiles.update'),
    page.getByRole('button', { name: 'Save profile', exact: true }).dispatchEvent('click'),
  ]).then(([response]) => response.json());
  if (updated.result?.dpi !== 600 || updated.result?.id !== created.result.id) throw new Error('Profile update lost its identity');
  await page.getByText('Profile saved.', { exact: true }).waitFor();
  if (screenshots) await page.screenshot({ path: join(screenshots, 'settings.png') });
  const removed = await Promise.all([
    response(page, 'v1.profiles.delete'),
    page.getByRole('button', { name: 'Delete profile', exact: true }).dispatchEvent('click'),
  ]).then(([response]) => response.json());
  if (removed.result?.deleted !== true) throw new Error('Profile deletion failed');
  if (errors.length) throw new Error(`Browser errors: ${errors.join('; ')}`);
  const storage = await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length }));
  if (storage.local || storage.session || (await page.locator('body').innerText()).includes(token))
    throw new Error('A credential escaped its in-memory secure field');
  if (screenshots) {
    await mkdir(screenshots, { recursive: true });
    await page.screenshot({ path: join(screenshots, 'reviewed.png') });
  }
  console.log(JSON.stringify({ methods, review: result.state, finish: result.finish, condition: result.condition, inventoryCopies: inventory.result.totalCopies, storage, errors }, null, 2));
} finally { await browser.close(); }
function response(page, method) {
  return page.waitForResponse(response => response.url() === `${origin}/rpc` && response.request().postDataJSON().method === method, { timeout: 15000 });
}

async function enter(page, name, value) {
  const label = await page.getByText(name, { exact: true }).boundingBox();
  if (!label) throw new Error(`Missing ${name} field`);
  await page.mouse.click(label.x + 80, label.y + label.height + 40);
  await page.waitForFunction(() => document.activeElement.tagName === 'TEXTAREA', {}, { timeout: 3000 });
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.insertText(value);
}
