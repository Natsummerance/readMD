const { _electron } = require('@playwright/test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
(async () => {
  const root = path.resolve(__dirname, '../packages/readmd-hermes-pet-adapter');
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'readmd-pet-smoke-'));
  const bridge = path.join(temp, 'bridge.json');
  const state = { format_version: 1, visible: true, renderer: 'live2d', info: { scale: .33, opacity: 1 } };
  fs.writeFileSync(bridge, JSON.stringify(state));
  const env = { ...process.env, READMD_PET_BRIDGE_FILE: bridge };
  delete env.ELECTRON_RUN_AS_NODE;
  const app = await _electron.launch({ executablePath: path.join(root, 'node_modules/electron/dist/electron.exe'), args: [path.join(root, 'dist')], env });
  try {
    const page = await app.firstWindow();
    await page.waitForFunction(() => document.body.dataset.live2dReady === 'true', { timeout: 30000 });
    const canvas = await page.locator('canvas').evaluate(el => ({ width: el.width, height: el.height }));
    if (!canvas.width || !canvas.height) throw new Error('empty Live2D canvas');
    state.renderer = 'hermes-sprite';
    fs.writeFileSync(bridge, JSON.stringify(state));
    await page.waitForURL(/renderer=hermes-sprite/);
    await page.locator('canvas').waitFor();
    console.log(JSON.stringify({ live2dLoaded: true, canvas, hermesMounted: true }));
  } finally { await app.close(); }
})().catch(error => { console.error(error.message); process.exitCode = 1; });
