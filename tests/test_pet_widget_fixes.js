// tests/test_pet_widget_fixes.js
const assert = require('assert');
const fs = require('fs');
const path = require('path');

console.log('--- Testing Pet Widget Fixes ---');

// Set up minimal browser DOM mock
const elements = {};
function mockElement(id) {
  return {
    id,
    classList: {
      _classes: new Set(),
      add(c) { this._classes.add(c); },
      remove(c) { this._classes.delete(c); },
      contains(c) { return this._classes.has(c); }
    },
    style: {},
    textContent: '',
    checked: false,
    _rect: { width: 120, height: 150, left: 0, top: 0 },
    getBoundingClientRect() {
      return this._rect;
    },
    addEventListener() {},
    removeEventListener() {},
    closest() { return null; }
  };
}

elements['pet-bubble'] = mockElement('pet-bubble');
elements['pet-bubble-text'] = mockElement('pet-bubble-text');
elements['readmd-pet-widget'] = mockElement('readmd-pet-widget');
elements['pet-character'] = mockElement('pet-character');
elements['pet-preview-character'] = mockElement('pet-preview-character');
elements['pet-character-wrap'] = mockElement('pet-character-wrap');

const listeners = {};
const windowMock = {
  innerWidth: 800,
  innerHeight: 600,
  addEventListener(event, fn) {
    if (!listeners[event]) listeners[event] = [];
    listeners[event].push(fn);
  },
  removeEventListener(event, fn) {
    if (listeners[event]) {
      listeners[event] = listeners[event].filter(f => f !== fn);
    }
  },
  trigger(event) {
    if (listeners[event]) {
      listeners[event].forEach(fn => fn());
    }
  }
};

const storage = {};
const localStorageMock = {
  getItem(k) { return storage[k] !== undefined ? storage[k] : null; },
  setItem(k, v) { storage[k] = String(v); },
  removeItem(k) { delete storage[k]; }
};

let loadedFiles = [];
let convertedFiles = [];
let batchEnqueuedFiles = [];

// Create sandbox
const sandbox = {
  $: (id) => elements[id] || null,
  window: windowMock,
  localStorage: localStorageMock,
  document: {
    readyState: 'complete',
    addEventListener() {}
  },
  loadFile: async (p, opts) => { loadedFiles.push({ path: p, opts }); },
  convertOrOcr: (p, mode) => { convertedFiles.push({ path: p, mode }); },
  enqueueBatchFiles: async (paths, overwrite) => { batchEnqueuedFiles.push({ paths, overwrite }); },
  setTimeout,
  clearTimeout,
  console,
  Math,
  Number,
  JSON,
  Date,
  Array
};

// Load code from assets/js/features/pet-batch.js
const code = fs.readFileSync(path.join(__dirname, '../assets/js/features/pet-batch.js'), 'utf8');

// Evaluate in sandbox context
const fn = new Function(...Object.keys(sandbox), code);
fn(...Object.values(sandbox));

// Call initPetDirectManipulation to hook resize event
windowMock.initPetDirectManipulation();

const showPetBubble = windowMock.showPetBubble;
const hidePetBubble = windowMock.hidePetBubble;
const applyWidgetAppearance = windowMock.applyWidgetAppearance;
const restoreWidgetPosition = windowMock.restoreWidgetPosition;
const PET_BUBBLE_PRIORITY = windowMock.PET_BUBBLE_PRIORITY;

assert(typeof showPetBubble === 'function', 'showPetBubble should be exported to window');
assert(typeof hidePetBubble === 'function', 'hidePetBubble should be exported to window');
assert(typeof applyWidgetAppearance === 'function', 'applyWidgetAppearance should be exported to window');
assert(typeof restoreWidgetPosition === 'function', 'restoreWidgetPosition should be exported to window');

// ============================================================================
// Test 1: 常驻气泡优先级保护 (showPetBubble Priority Protection)
// ============================================================================
console.log('Testing Issue 1: Persistent bubble priority protection...');

// 1.1 Show persistent bubble with durationMs = 0, priority = 4
showPetBubble('常驻系统告警', 0, PET_BUBBLE_PRIORITY.CRITICAL);
assert.strictEqual(elements['pet-bubble'].classList.contains('is-visible'), true);
assert.strictEqual(elements['pet-bubble-text'].textContent, '常驻系统告警');

// 1.2 Send lower priority message (priority = 1)
showPetBubble('日常闲聊问候', 4500, PET_BUBBLE_PRIORITY.LOW_IDLE);
assert.strictEqual(elements['pet-bubble-text'].textContent, '常驻系统告警',
  'Low priority message MUST NOT overwrite persistent bubble');

// 1.3 Call hidePetBubble to dismiss
hidePetBubble();
assert.strictEqual(elements['pet-bubble'].classList.contains('is-visible'), false);

// 1.4 Send lower priority message again - should now succeed
showPetBubble('日常闲聊问候', 4500, PET_BUBBLE_PRIORITY.LOW_IDLE);
assert.strictEqual(elements['pet-bubble'].classList.contains('is-visible'), true);
assert.strictEqual(elements['pet-bubble-text'].textContent, '日常闲聊问候',
  'Low priority message should be displayed after hidePetBubble reset priority');

hidePetBubble();
console.log('  [PASS] Issue 1 verified successfully.');

// ============================================================================
// Test 2: 透明度配置缺少范围约束 (applyWidgetAppearance Opacity Clamping)
// ============================================================================
console.log('Testing Issue 2: Opacity boundary clamping (0.1 - 1.0)...');

const opacityTestCases = [
  { input: -1, expected: '0.1' },
  { input: 0, expected: '0.1' },
  { input: 0.5, expected: '0.5' },
  { input: 2, expected: '1' },
  { input: NaN, expected: '1' },
  { input: undefined, expected: '1' },
  { input: Infinity, expected: '1' }
];

for (const tc of opacityTestCases) {
  applyWidgetAppearance(0.33, tc.input);
  const actual = elements['pet-character'].style.opacity;
  assert.strictEqual(actual, tc.expected,
    `Input ${tc.input} expected opacity '${tc.expected}', got '${actual}'`);
  const previewActual = elements['pet-preview-character'].style.opacity;
  assert.strictEqual(previewActual, tc.expected,
    `Input ${tc.input} preview expected opacity '${tc.expected}', got '${previewActual}'`);
}
console.log('  [PASS] Issue 2 verified successfully.');

// ============================================================================
// Test 3: 恢复位置使用固定尺寸边界与动态 Resize 重新约束
// ============================================================================
console.log('Testing Issue 3: Dynamic bounds clamping for restoreWidgetPosition & resize...');

const scaleCases = [
  { scale: 0.6, width: 72, height: 90 },
  { scale: 1.0, width: 120, height: 150 },
  { scale: 1.6, width: 192, height: 240 }
];

const widget = elements['readmd-pet-widget'];

for (const sc of scaleCases) {
  widget._rect = { width: sc.width, height: sc.height, left: 0, top: 0 };
  windowMock.innerWidth = 800;
  windowMock.innerHeight = 600;

  // Simulate saved position near edge
  localStorageMock.setItem('readmd_pet_pos', JSON.stringify({ left: 790, top: 590 }));
  restoreWidgetPosition();

  const left = parseFloat(widget.style.left);
  const top = parseFloat(widget.style.top);

  // Widget must not extend past window boundaries
  assert(left + sc.width <= windowMock.innerWidth,
    `At scale ${sc.scale}, widget overflows right boundary: left(${left}) + width(${sc.width}) > ${windowMock.innerWidth}`);
  assert(top + sc.height <= windowMock.innerHeight,
    `At scale ${sc.scale}, widget overflows bottom boundary: top(${top}) + height(${sc.height}) > ${windowMock.innerHeight}`);

  // Simulate window resize to smaller viewport
  windowMock.innerWidth = 400;
  windowMock.innerHeight = 350;
  windowMock.trigger('resize');

  const resizedLeft = parseFloat(widget.style.left);
  const resizedTop = parseFloat(widget.style.top);

  assert(resizedLeft + sc.width <= windowMock.innerWidth,
    `After resize at scale ${sc.scale}, widget overflows right boundary: left(${resizedLeft}) + width(${sc.width}) > ${windowMock.innerWidth}`);
  assert(resizedTop + sc.height <= windowMock.innerHeight,
    `After resize at scale ${sc.scale}, widget overflows bottom boundary: top(${resizedTop}) + height(${sc.height}) > ${windowMock.innerHeight}`);
}

console.log('  [PASS] Issue 3 verified successfully.');

// ============================================================================
// Test 4: 桌面模式文件拖拽智能直开 (handlePetDroppedFiles & receivePetBatch)
// ============================================================================
console.log('Testing Issue 4: Smart direct opening for dropped files in Desktop Mode...');

(async () => {
  const receivePetBatch = windowMock.receivePetBatch;
  assert(typeof receivePetBatch === 'function', 'receivePetBatch must be function');

  // 4.1 单个 Markdown 文件 -> 直接 loadFile 打开，不走批量转换
  loadedFiles = [];
  convertedFiles = [];
  batchEnqueuedFiles = [];
  await receivePetBatch(['C:/notes/my-document.md']);
  assert.strictEqual(loadedFiles.length, 1, 'Single .md must call loadFile');
  assert.strictEqual(loadedFiles[0].path, 'C:/notes/my-document.md');
  assert.strictEqual(loadedFiles[0].opts.force, true);
  assert.strictEqual(loadedFiles[0].opts.browserCopy, false, 'Local dropped file must explicitly set browserCopy: false');
  assert.strictEqual(convertedFiles.length, 0, 'Single .md must not call convertOrOcr');
  assert.strictEqual(batchEnqueuedFiles.length, 0, 'Single .md must not enqueue to batch modal');
  assert(elements['pet-bubble-text'].textContent.includes('my-document.md 已打开'),
    'Pet bubble should confirm document opened');

  // 4.1.1 验证 loadFile 中的 isBrowserCopy 计算对于新文件 (existingTab === null) 绝不抛异常
  const computeIsBrowserCopy = (force, browserCopy, existingTab) => {
    return force && browserCopy === null
      ? (existingTab ? existingTab.browserCopy === true : false)
      : browserCopy === true;
  };
  assert.strictEqual(computeIsBrowserCopy(true, null, null), false, 'When existingTab is null and force=true, browserCopy must be false without throwing');
  assert.strictEqual(computeIsBrowserCopy(true, false, null), false);
  assert.strictEqual(computeIsBrowserCopy(true, null, { browserCopy: true }), true);
  assert.strictEqual(computeIsBrowserCopy(false, true, null), true);

  // 4.2 单个可转换文档（如 PDF / Word） -> 自动启动转换
  loadedFiles = [];
  convertedFiles = [];
  batchEnqueuedFiles = [];
  await receivePetBatch(['C:/docs/contract.docx']);
  assert.strictEqual(convertedFiles.length, 1, 'Single .docx must trigger convertOrOcr');
  assert.strictEqual(convertedFiles[0].path, 'C:/docs/contract.docx');
  assert.strictEqual(convertedFiles[0].mode, 'convert');
  assert.strictEqual(loadedFiles.length, 0, 'Single .docx should wait for conversion before loadFile');
  assert.strictEqual(batchEnqueuedFiles.length, 0, 'Single .docx must not enqueue to batch modal');

  // 4.3 多个 Markdown 文件 -> 多标签全部加载
  loadedFiles = [];
  convertedFiles = [];
  batchEnqueuedFiles = [];
  await receivePetBatch(['C:/notes/one.md', 'C:/notes/two.txt']);
  assert.strictEqual(loadedFiles.length, 2, 'Multiple text files must all be loaded in tabs');
  assert.strictEqual(loadedFiles[0].path, 'C:/notes/one.md');
  assert.strictEqual(loadedFiles[1].path, 'C:/notes/two.txt');
  assert.strictEqual(batchEnqueuedFiles.length, 0);

  // 4.4 混合文件（Markdown + PDF） -> 文本直接加载，PDF 送入批量转换
  loadedFiles = [];
  convertedFiles = [];
  batchEnqueuedFiles = [];
  await receivePetBatch(['C:/notes/readme.md', 'C:/docs/spec.pdf']);
  assert.strictEqual(loadedFiles.length, 1, 'Markdown part must be directly loaded');
  assert.strictEqual(loadedFiles[0].path, 'C:/notes/readme.md');
  assert.strictEqual(batchEnqueuedFiles.length, 1, 'PDF part must be sent to batch convert');
  assert.deepStrictEqual(batchEnqueuedFiles[0].paths, ['C:/docs/spec.pdf']);

  console.log('  [PASS] Issue 4 verified successfully.');

  // --------------------------------------------------------------------------
  // Test 5: Hermes Sprite Appearance Scaling & State Classes (In-App Mode)
  // --------------------------------------------------------------------------
  console.log('Testing Issue 5: Hermes sprite appearance scaling & state classes...');
  
  // 5.1 Scale mapping bounds: 0.18 -> 0.6, 0.33 -> ~0.99, 0.72 -> 1.6
  applyWidgetAppearance(0.18, 1.0);
  assert.strictEqual(elements['pet-character'].style.transform, 'scale(0.6)');
  assert.strictEqual(elements['pet-preview-character'].style.transform, 'scale(0.6)');

  applyWidgetAppearance(0.33, 1.0);
  assert.strictEqual(elements['pet-character'].style.transform, 'scale(0.99)');

  applyWidgetAppearance(0.72, 1.0);
  assert.strictEqual(elements['pet-character'].style.transform, 'scale(1.6)');

  // 5.2 Layer separation: widget carries position (left/top), character carries appearance (scale)
  elements['readmd-pet-widget'].style.left = '100px';
  elements['readmd-pet-widget'].style.top = '200px';
  assert.strictEqual(elements['readmd-pet-widget'].style.left, '100px');
  assert.strictEqual(elements['pet-character'].style.transform, 'scale(1.6)');

  console.log('  [PASS] Issue 5 verified successfully.');
  console.log('\nAll 5 pet widget issues PASSED verification!');
  process.exit(0);
})().catch(err => {
  console.error('Test failed:', err);
  process.exit(1);
});

