// Copy to ReadMD tests/test_audit_pet.js; install the adapter's dev dependencies.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { createRequire } = require('node:module');

const root = path.resolve(__dirname, '..');
const adapterRequire = createRequire(path.join(root,
  'packages/readmd-hermes-pet-adapter/package.json'));
const { transformSync } = process.env.READMD_AUDIT_ESBUILD
  ? require(process.env.READMD_AUDIT_ESBUILD) : adapterRequire('esbuild');

test('test_reproduce_bug_006: first fullscreen snapshot stays hidden', async () => {
  let poll;
  let snapshot = { visible: true, fullscreen: true, info: {} };
  const windows = [];
  class Window {
    constructor(options) {
      this.visible = options.show !== false;
      this.bounds = options;
      this.webContents = { once() {}, send() {} };
      windows.push(this);
    }
    isDestroyed() { return false; }
    setOpacity() {}
    setAlwaysOnTop() {}
    on() {}
    loadFile() { return Promise.resolve(); }
    getBounds() { return this.bounds; }
    setBounds(bounds) { this.bounds = bounds; }
    showInactive() { this.visible = true; }
    hide() { this.visible = false; }
    close() { this.visible = false; }
  }
  const electron = {
    app: {
      commandLine: { getSwitchValue: () => '' },
      isReady: () => true, whenReady: () => Promise.resolve(),
      getAppPath: () => root, on() {}, exit() {},
    },
    BrowserWindow: Window, clipboard: {}, ipcMain: { on() {} },
    screen: {
      getAllDisplays: () => [{ workArea: { x: 0, y: 0, width: 1920, height: 1080 } }],
    },
  };
  const fakeFs = {
    readFileSync: file => file === '/bridge.json'
      ? JSON.stringify(snapshot) : Buffer.from('sprite'),
    writeFileSync() {},
  };
  const source = fs.readFileSync(path.join(root,
    'packages/readmd-hermes-pet-adapter/src/electron-main.ts'), 'utf8');
  const compiled = transformSync(source, { loader: 'ts', format: 'cjs' }).code;
  vm.runInNewContext(compiled, {
    require: name => {
      if (name === 'electron') return electron;
      if (name === 'node:fs') return fakeFs;
      if (name === 'node:path') return path;
      if (name.includes('pet-overlay-ipc')) return { registerPetOverlayIpc() {} };
      throw new Error('Unexpected import: ' + name);
    },
    process: { argv: [], env: { READMD_PET_BRIDGE_FILE: '/bridge.json' } },
    __dirname: root, exports: {}, module: { exports: {} },
    setInterval: callback => { poll = callback; return 1; }, clearInterval() {},
    console,
  });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(windows.length, 1);
  assert.equal(windows[0].visible, false);
  poll(); // Identical bridge bytes must not expose the window.
  assert.equal(windows[0].visible, false);
  snapshot = { ...snapshot, fullscreen: false };
  poll();
  assert.equal(windows[0].visible, true);
});
