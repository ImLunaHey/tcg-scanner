// Development-only browser capture/lifecycle verification. The fixture's camera
// endpoint uses real Rust perspective extraction and a synthetic identity.
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
const [origin, screenshots] = process.argv.slice(2);
if (!origin) throw new Error('Usage: node camera.mjs TAILSCALE_ORIGIN [SCREENSHOTS]');
const browser = await chromium.launch({ headless: true, args: ['--no-proxy-server', '--enable-unsafe-swiftshader'] });
try {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 1 });
  const errors = [], requests = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('request', request => {
    if (!request.url().endsWith('/api/v1/camera/identify')) return;
    const value = request.postDataJSON();
    if (request.headers().authorization || !Buffer.from(value.base64, 'base64').subarray(0, 2).equals(Buffer.from([255, 216])))
      throw new Error('Camera lookup must send an anonymous JPEG');
    if (Buffer.from(value.base64, 'base64').length > 512 * 1024) throw new Error('Oversized camera image');
    requests.push({ language: value.language, bytes: Buffer.from(value.base64, 'base64').length });
  });
  await page.addInitScript(() => {
    const fixture = window.cameraFixture = { tracks: [], pending: undefined, hold: true, streams: [], torch: false, version: 0 };
    function scene(canvas) {
      const c = canvas.getContext('2d');
      c.fillStyle = '#202020'; c.fillRect(0, 0, 600, 800);
      c.fillStyle = fixture.version % 2 ? '#80a0ff' : '#fff3bb'; c.fillRect(100, 70, 400, 660);
      c.fillStyle = '#305080'; c.fillRect(120, 180, 360, 300);
      c.fillStyle = '#202020'; c.font = '28px sans-serif'; c.fillText('Camera fixture', 125, 140);
      c.font = '22px sans-serif'; c.fillText('001 / 001', 140, 660);
    }
    navigator.mediaDevices.getUserMedia = async constraints => {
      if (constraints.audio !== false) throw new Error('Audio must not be requested');
      const canvas = document.createElement('canvas'); canvas.width = 600; canvas.height = 800;
      scene(canvas);
      const stream = canvas.captureStream(5), track = stream.getVideoTracks()[0];
      fixture.tracks.push(track); fixture.streams.push(stream);
      const timer = setInterval(() => scene(canvas), 100);
      const stop = track.stop.bind(track);
      track.stop = () => { clearInterval(timer); stop(); };
      const getSettings = track.getSettings.bind(track);
      track.getCapabilities = () => ({ torch: true });
      track.getSettings = () => ({ ...getSettings(), torch: fixture.torch });
      track.applyConstraints = async value => { fixture.torch = value.advanced[0].torch; };
      if (fixture.hold) await new Promise(resolve => { fixture.pending = resolve; });
      return stream;
    };
  });
  await page.goto(`${origin}/camera`, { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Start camera', exact: true }).waitFor({timeout:5000}).catch(async error => {
    console.error(JSON.stringify({ errors, state: await page.evaluate(() => ({ url:location.href, secure:isSecureContext, body:document.body.innerText })) }));
    throw error;
  });
  await click(page, 'Start camera');
  await page.waitForFunction(() => Boolean(window.cameraFixture.pending), {}, { timeout: 5000 }).catch(async error => {
    console.error(JSON.stringify({ errors, state: await page.evaluate(() => ({ secure: isSecureContext, hidden: document.hidden, online: navigator.onLine, tracks: cameraFixture.tracks.length, body: document.body.innerText })) }));
    throw error;
  });
  await click(page, 'Stop camera');
  await page.evaluate(() => { cameraFixture.hold = false; cameraFixture.pending(); });
  await page.waitForFunction(() => cameraFixture.tracks.length === 1 && cameraFixture.tracks.every(t => t.readyState === 'ended'));
  await page.getByRole('button', { name: 'Start camera', exact: true }).waitFor();
  const first = await Promise.all([lookup(page), click(page, 'Start camera')]).then(([r]) => r.json());
  if (first.state !== 'matched' || first.candidates[0].id !== 'synthetic-camera') throw new Error(`Unexpected camera result ${JSON.stringify(first)}`);
  await page.getByRole('button', { name: 'View Synthetic camera card details', exact: true }).waitFor();
  await click(page, 'Torch on');
  await page.getByRole('button', { name: 'Torch off', exact: true }).waitFor();
  await click(page, 'Torch off');
  await page.getByRole('button', { name: 'Torch on', exact: true }).waitFor();
  if (screenshots) { await mkdir(screenshots, { recursive: true }); await page.screenshot({ path: join(screenshots, 'camera-live.png') }); }
  await click(page, 'View Synthetic camera card details');
  await page.getByRole('button', { name: 'Back to camera', exact: true }).waitFor();
  const count = requests.length;
  await page.evaluate(() => { cameraFixture.version++; });
  await page.waitForTimeout(1600);
  if (requests.length !== count) throw new Error('Details did not pause lookup');
  if (screenshots) await page.screenshot({ path: join(screenshots, 'camera-details.png') });
  await Promise.all([lookup(page), click(page, 'Back to camera')]);
  // A hidden page closes the actual camera tracks, including while lookup runs.
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: true }); document.dispatchEvent(new Event('visibilitychange')); });
  await page.waitForFunction(() => cameraFixture.tracks.every(t => t.readyState === 'ended'));
  await page.getByRole('button', { name: 'Start camera', exact: true }).waitFor();
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: false }); document.dispatchEvent(new Event('visibilitychange')); });
  await page.waitForTimeout(400);
  if (await page.getByRole('button', { name: 'Stop camera', exact: true }).count()) throw new Error('Returning to page silently restarted camera');
  await click(page, 'Korean');
  await Promise.all([lookup(page), click(page, 'Start camera')]);
  if (requests.at(-1).language !== 'ko') throw new Error('Physical card language was not applied');
  await page.evaluate(() => window.dispatchEvent(new Event('offline')));
  await page.waitForFunction(() => cameraFixture.tracks.every(t => t.readyState === 'ended'));
  await page.getByRole('button', { name: 'Start camera', exact: true }).waitFor();
  const state = await page.evaluate(() => ({ tracks: cameraFixture.tracks.map(t => t.readyState), videos: document.querySelectorAll('.camera-source').length, local: localStorage.length, session: sessionStorage.length }));
  if (errors.length || state.videos || state.local || state.session) throw new Error(JSON.stringify({ errors, state }));
  console.log(JSON.stringify({ requests, state, errors }, null, 2));
} finally { await browser.close(); }
function click(page, name) { return page.getByRole('button', { name, exact: true }).dispatchEvent('click'); }
function lookup(page) { return page.waitForResponse(r => r.url().endsWith('/api/v1/camera/identify') && r.status() === 200, { timeout: 30000 }); }
