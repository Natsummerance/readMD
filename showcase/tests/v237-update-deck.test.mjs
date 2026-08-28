import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { LAYOUTS, loadManifest, updateListMarkdown } = require('../update-v237/deck-lib.cjs');
const showcaseRoot = path.resolve(import.meta.dirname, '..');
const manifestPath = path.join(showcaseRoot, 'update-v237', 'release-delta.json');

test('V2.3.7 update deck has a bounded, bilingual, non-repeating evidence plan', () => {
  const manifest = loadManifest(manifestPath);
  assert.equal(manifest.release, 'v2.3.7');
  assert.equal(manifest.range, 'v2.3.6..v2.3.7');
  assert.equal(manifest.pages.length, 14);
  assert.deepEqual(new Set(manifest.pages.map((page) => page.shot_id || page.source_id)).size, manifest.pages.length);
  assert.ok(manifest.pages.every((page) => LAYOUTS.has(page.layout)));
  assert.ok(manifest.pages.every((page) => page.copy['zh-CN'].title.length >= 4));
  assert.ok(manifest.pages.every((page) => page.copy.en.title.length >= 4));
});

test('V2.3.7 update list is deterministically derived from the deck manifest', () => {
  const manifest = loadManifest(manifestPath);
  const chinese = updateListMarkdown(manifest, 'zh-CN');
  const english = updateListMarkdown(manifest, 'en');
  assert.match(chinese, /ReadMD 2\.3\.7/);
  assert.match(english, /ReadMD 2\.3\.7/);
  assert.match(chinese, /版本范围：`v2\.3\.6\.\.v2\.3\.7`/);
  assert.match(english, /Release range: `v2\.3\.6\.\.v2\.3\.7`/);
  assert.doesNotMatch(english, /[\u4e00-\u9fff]/);
  assert.equal((chinese.match(/^\d+\. /gm) || []).length, manifest.pages.length);
  assert.equal((english.match(/^\d+\. /gm) || []).length, manifest.pages.length);
  assert.equal(fs.existsSync(path.join(showcaseRoot, 'update-v237', 'DESIGN.md')), true);
});
