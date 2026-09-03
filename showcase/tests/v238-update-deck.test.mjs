import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { loadManifest, updateListMarkdown } = require('../update-v238/deck-lib.cjs');
const showcaseRoot = path.resolve(import.meta.dirname, '..');
const manifestPath = path.join(showcaseRoot, 'update-v238', 'release-delta.json');

test('V2.3.8 update deck has a bounded, bilingual, non-repeating evidence plan', () => {
  const manifest = loadManifest(manifestPath);
  assert.equal(manifest.release, 'v2.3.8');
  assert.equal(manifest.range, 'v2.3.7..v2.3.8');
  assert.equal(manifest.pages.length, 14);
  assert.deepEqual(new Set(manifest.pages.map((page) => page.shot_id || page.source_id)).size, manifest.pages.length);
  assert.ok(manifest.pages.every((page) => page.copy['zh-CN'].title.length >= 4));
  assert.ok(manifest.pages.every((page) => page.copy.en.title.length >= 4));
});

test('V2.3.8 update list is deterministically derived from the deck manifest', () => {
  const manifest = loadManifest(manifestPath);
  const chinese = updateListMarkdown(manifest, 'zh-CN');
  const english = updateListMarkdown(manifest, 'en');
  assert.match(chinese, /ReadMD v2\.3\.8/);
  assert.match(english, /ReadMD v2\.3\.8/);
  assert.match(chinese, /v2\.3\.7\.\.v2\.3\.8/);
  assert.match(english, /v2\.3\.7\.\.v2\.3\.8/);
  assert.equal(fs.existsSync(path.join(showcaseRoot, 'update-v238', 'DESIGN.md')), true);
});
