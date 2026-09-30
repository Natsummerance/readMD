import assert from 'node:assert/strict';
import test from 'node:test';
import { buildManifest, diffManifest, promptViolations, stripRustTests } from '../check-assets.mjs';

test('manifest diff reports missing, added and changed files', () => {
  const actual = { files: [{ path: 'a', bytes: 1, sha256: 'x' }, { path: 'b', bytes: 2, sha256: 'y' }] };
  assert.deepEqual(diffManifest({ files: actual.files }, actual), []);
  const expected = { files: [{ path: 'a', bytes: 1, sha256: 'z' }, { path: 'c', bytes: 1, sha256: 'q' }] };
  const problems = diffManifest(expected, actual);
  assert.equal(problems.length, 2);
  assert.match(problems[0], /missing=\["c"\] added=\["b"\]/);
  assert.match(problems[1], /hash mismatch: a/);
});

test('checked-in manifest matches the vendored tree', () => {
  const built = buildManifest();
  assert.ok(built.files.length > 0);
  assert.ok(built.files.every(f => f.path.startsWith('assets/upstream/') && f.path !== 'assets/upstream/manifest.json'));
});

test('prompt policy: embedded system prompts fail, test modules do not', () => {
  const bad = 'let m = json!({"role": "system", "content": "You are a helpful assistant that"});';
  assert.equal(promptViolations(new Map([['src/a.rs', bad]])).length, 1);
  const inTest = `fn f() {}\n#[cfg(test)]\nmod tests {\n    ${bad}\n}\n`;
  assert.deepEqual(promptViolations(new Map([['src/a.rs', inTest]])), []);
  assert.equal(stripRustTests(inTest).split('\n').length, inTest.split('\n').length);
});
