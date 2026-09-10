// Copy to tests/test_audit_live2d.js in the pinned ReadMD checkout.
// Prerequisite: npm ci --prefix packages/readmd-hermes-pet-adapter --ignore-scripts
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { createRequire } = require('node:module');
const root = path.resolve(__dirname, '..');
let esbuild;
try { esbuild = require('esbuild'); }
catch { esbuild = createRequire(path.join(root, 'packages/readmd-hermes-pet-adapter/package.json'))('esbuild'); }

test('test_reproduce_bug_004: initial transparent margin must pass through', async () => {
  const calls = [];
  const listeners = {};
  const model = {
    width: 100, height: 100, scale: { x: 1, set() {} },
    hitTest: () => [], getBounds: () => ({ contains: () => false }),
    expression() {},
  };
  const pixi = { Ticker: {}, Application: class {
    constructor() {
      this.screen = { width: 300, height: 420 };
      this.stage = { addChild() {} };
      this.ticker = { started: true, start() {}, stop() {} };
      this.view = {};
    }
  }};
  const live = { Live2DModel: { registerTicker() {}, from: async () => model } };
  const source = fs.readFileSync(path.join(root,
    'packages/readmd-hermes-pet-adapter/src/live2d/stage.ts'), 'utf8');
  const compiled = esbuild.transformSync(source, { loader: 'ts', format: 'cjs', target: 'es2020', supported: { 'dynamic-import': false } }).code;
  const module = { exports: {} };
  const body = { style: {}, dataset: {}, replaceChildren() {} };
  const context = {
    module, exports: module.exports, URL,
    require: name => {
      if (name === 'pixi.js') return pixi;
      if (name === 'pixi-live2d-display/cubism4') return live;
      throw new Error('unexpected dependency: ' + name);
    },
    window: {
      devicePixelRatio: 1, Live2DCubismCore: {},
      location: { href: 'file:///adapter/renderer/index.html' },
      addEventListener: (name, fn) => { listeners[name] = fn; },
      hermesDesktop: { petOverlay: {
        setIgnoreMouse: value => calls.push(value),
        onState() {}, control() {}, setBounds() {},
      } },
    },
    document: { body, visibilityState: 'visible', getElementById: () => body, addEventListener() {} },
    XMLHttpRequest: class {
      open() {}
      send() { this.responseText = '{"entry":"model.json"}'; this.onload(); }
    },
  };
  vm.runInNewContext(compiled, context);
  await module.exports.mountLive2dStage();
  listeners.pointermove({ clientX: 0, clientY: 0 });
  assert.ok(calls.includes(true), 'native click-through was never initialized');
});
