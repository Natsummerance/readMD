// Native regression: a loaded model must occupy the visible viewport.
const { _electron } = require('@playwright/test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
(async () => {
  const root = path.resolve(__dirname, '../packages/readmd-hermes-pet-adapter');
  const output = fs.mkdtempSync(path.join(os.tmpdir(), 'readmd-live2d-layout-'));
  const bridge = path.join(output, 'bridge.json');
  fs.writeFileSync(bridge, JSON.stringify({format_version:1, visible:true, renderer:'live2d', info:{scale:.5,opacity:1}}));
  const env = {...process.env, READMD_PET_BRIDGE_FILE:bridge};
  delete env.ELECTRON_RUN_AS_NODE;
  const app = await _electron.launch({executablePath:path.join(root,'node_modules/electron/dist/electron.exe'),args:[path.join(root,'dist')],env});
  try {
    const page = await app.firstWindow();
    await page.waitForFunction(() => document.body.dataset.live2dReady === 'true');
    const layout = await page.evaluate(() => {
      const canvas = document.querySelector('canvas');
      const r = canvas.getBoundingClientRect();
      return {parent:canvas.parentElement.id, top:r.top, bottom:r.bottom, height:innerHeight};
    });
    assert.equal(layout.parent, 'root');
    assert.equal(layout.top, 0);
    assert.ok(layout.bottom <= layout.height + 1);
    await page.screenshot({path:path.join(output,'live2d.png')});
    console.log(JSON.stringify({layout, screenshot:path.join(output,'live2d.png')}));
  } finally { await app.close(); }
})().catch(error => {console.error(error);process.exitCode=1;});
