// tests/test_dragdrop_dispatch.js — Node 内置测试，无第三方依赖
// handleDroppedEntries 按条目类型分派：带 path 的文本走原路径打开（可写回），
// 不带 path 的走上传；文档走转换；ZIP 走解压；文件夹打开文件树。
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

function load() {
  const source = fs.readFileSync('assets/js/core/dragdrop.js', 'utf8');
  const calls = [];
  const context = vm.createContext({
    window: {},
    console,
    calls,
    CONVERT_BINARY_RE: /\.(docx?|pdf|pptx?|xlsx?|epub|mobi)$/i,
    IMG_RE: /\.(png|jpe?g|gif|webp|bmp)$/i,
    $: () => null,
    showToast: () => {},
    uploadFile: async f => { calls.push(['upload', f.name]); return 'C:/uploads/' + f.name; },
    loadFile: async (p, o) => { calls.push(['load', p, !!(o && o.browserCopy)]); },
    convertOrOcr: (p, m) => { calls.push(['convert', p, m]); },
    enqueueBatchFiles: (ps, conv) => { calls.push(['batch', ps.slice(), conv]); },
    listFolder: async d => { calls.push(['folder', d]); },
    apiFetch: async (url, init) => {
      calls.push(['api', url, init && init.headers && init.headers['Content-Type']]);
      return { json: async () => ({ ok: true, paths: ['C:/x/a.md'], skipped: 0 }) };
    },
  });
  vm.runInContext(source, context);
  return context;
}

test('native paths open in place, documents convert, zip extracts, folders list', async () => {
  const c = load();
  await c.handleDroppedEntries([
    { name: 'notes.md', path: 'D:/docs/notes.md' },
    { name: 'report.docx', path: 'D:/docs/report.docx' },
    { name: 'bundle.zip', path: 'D:/docs/bundle.zip' },
    { name: 'docs', path: 'D:/docs', isDir: true },
  ]);
  const calls = JSON.parse(JSON.stringify(c.calls));
  assert.deepEqual(calls.find(x => x[0] === 'folder'), ['folder', 'D:/docs']);
  assert.deepEqual(calls.find(x => x[0] === 'load'), ['load', 'D:/docs/notes.md', false]);
  assert.deepEqual(calls.find(x => x[0] === 'convert'), ['convert', 'D:/docs/report.docx', 'convert']);
  assert.deepEqual(calls.find(x => x[0] === 'api'), ['api', '/api/batch/extract-zip', 'application/json']);
  assert.deepEqual(calls.find(x => x[0] === 'batch'), ['batch', ['C:/x/a.md'], false]);
  assert.ok(!calls.some(x => x[0] === 'upload'), 'paths are never re-uploaded');
});

test('browser drops without a path upload a copy', async () => {
  const c = load();
  await c.handleDroppedEntries([
    { name: 'a.md', path: '', file: { name: 'a.md' } },
    { name: 'b.pdf', path: '', file: { name: 'b.pdf' } },
    { name: 'c.pdf', path: '', file: { name: 'c.pdf' } },
  ]);
  const calls = JSON.parse(JSON.stringify(c.calls));
  assert.deepEqual(calls.filter(x => x[0] === 'upload').map(x => x[1]), ['a.md', 'b.pdf', 'c.pdf']);
  assert.deepEqual(calls.find(x => x[0] === 'load'), ['load', 'C:/uploads/a.md', true]);
  assert.deepEqual(calls.find(x => x[0] === 'batch'), ['batch', ['C:/uploads/b.pdf', 'C:/uploads/c.pdf'], false]);
});

test('the native bridge forwards drops with their real paths', async () => {
  const c = load();
  c.window.__readmdNativeDrop({ paths: [{ path: 'E:/x/readme.md', name: 'readme.md', isDir: false }] });
  await new Promise(r => setTimeout(r, 10));
  assert.deepEqual(JSON.parse(JSON.stringify(c.calls)), [['load', 'E:/x/readme.md', false]]);
});
