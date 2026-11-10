import initialize from './scanner_ui.js';
try {
  await initialize();
  document.getElementById('loading')?.remove();
} catch {
  document.getElementById('loading').textContent = 'Scanner could not start. Try a browser with WebGPU or WebGL2 support, then reload.';
}
