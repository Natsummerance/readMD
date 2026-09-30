import assert from 'node:assert/strict';
import test from 'node:test';
import { scanBuffer } from '../privacy-scan.mjs';

const scan = (label, text, opts) => scanBuffer(label, Buffer.from(text, 'utf8'), opts);
const fakeKey = 'sk-' + 'A'.repeat(24);

test('flags key-shaped strings and developer paths', () => {
  assert.match(scan('src/a.js', `const k = "${fakeKey}"`)[0], /plaintext API key/);
  assert.match(scan('src/a.js', 'root = "D:/Users/someone/x"')[0], /local absolute path/);
  assert.match(scan('src/a.js', 'p = "/home/dev/Projects/x"')[0], /local absolute path/);
});

test('retired provider markers, except the audited attribution files', () => {
  const marker = 'cc' + '-switch';
  assert.match(scan('src/a.js', `x ${marker}`)[0], /retired provider marker/);
  assert.deepEqual(scan('assets/providers/provider-catalog.json', `x ${marker}`), []);
});

test('sensitive document names fail before any skip rule', () => {
  assert.match(scan('tests/北京交通大学软件学院毕业实习文档.docx', '')[0], /sensitive/);
});

test('vendored, test, binary and dist content is skipped as before', () => {
  for (const label of ['assets/vendor/x.js', 'assets/upstream/a/b.md', 'tests/x.js', 'ui-tests/y.js', 'a/b.png', 'x.zip']) {
    assert.deepEqual(scan(label, fakeKey), [], label);
  }
  assert.deepEqual(scan('dist/app.js', 'C:/Users/me/x'), []);
  assert.deepEqual(scan('app.js', 'C:/Users/me/x', { sourceTree: false }), []);
});
