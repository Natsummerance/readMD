// Release and packaging contracts (ported from the retired pytest suite).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = rel => fs.readFileSync(path.join(root, rel), 'utf8');
const exists = rel => fs.existsSync(path.join(root, rel));

test('formal platform matrix is explicit and HarmonyOS is out of scope', () => {
  const matrix = JSON.parse(read('release/platform-matrix.json'));
  const ids = new Set(matrix.formal_support.map(item => item.id));
  for (const id of ['windows-x64', 'windows-arm64', 'macos-x64', 'macos-arm64', 'linux-x64', 'linux-arm64']) {
    assert.ok(ids.has(id), id);
  }
  assert.ok(matrix.out_of_scope.includes('HarmonyOS/OpenHarmony'));
  assert.equal(matrix.evidence_policy.release_blocked_until_complete, true);
  assert.ok(matrix.formal_support.every(item => item.native_backend));
  assert.equal(matrix.feature_matrix.no_implicit_empty_cells, true);
  assert.equal(matrix.feature_matrix.default_status, 'pending-native-evidence');
  for (const f of ['open', 'edit', 'save', 'preview', 'convert', 'export', 'ocr', 'ai', 'skills', 'vsix', 'mcp']) {
    assert.ok(matrix.features.includes(f), f);
  }
});

test('release workflow ships no HAP source archive and needs no Python', () => {
  const workflow = read('.github/workflows/release.yml');
  assert.ok(!workflow.includes('ReadMD-harmonyos'));
  assert.ok(!/setup-python|readmd_mcp_server\.py|requirements-common\.txt/.test(workflow));
  assert.ok(!read('scripts/linux/build_linux.sh').includes('.hap'));
  assert.ok(!read('release/release_notes.md').includes('.hap'));
  for (const name of ['README.md', 'README.en.md', 'README.ja.md', 'README.zh-TW.md']) {
    assert.ok(!read(name).includes('releases/latest/download/ReadMD-harmonyos'), name);
  }
});

test('release workflow builds every desktop target from the Rust workspace', () => {
  const workflow = read('.github/workflows/release.yml');
  assert.match(workflow, /tags:\s*\[\s*'v\*',\s*'V\*'\s*\]/);
  for (const os of ['windows', 'ubuntu', 'macos']) assert.match(workflow, new RegExp(os + '-'), os);
  assert.match(workflow, /cargo build/);
  assert.match(workflow, /body_path:\s*RELEASE_NOTES\.md/);
  assert.ok(exists('RELEASE_NOTES.md'));
});

test('HarmonyOS scaffold stays structurally buildable', () => {
  const h = 'packages/harmonyos-app/';
  for (const rel of [
    'build-profile.json5', 'hvigorfile.ts', 'oh-package.json5', 'entry/build-profile.json5',
    'entry/hvigorfile.ts', 'entry/oh-package.json5', 'entry/src/main/module.json5',
    'entry/src/main/resources/base/profile/main_pages.json',
  ]) assert.ok(exists(h + rel), rel);
  const res = h + 'entry/src/main/resources/base/';
  const names = new Set(JSON.parse(read(res + 'element/string.json')).string.map(i => i.name));
  for (const n of ['module_desc', 'EntryAbility_desc', 'EntryAbility_label']) assert.ok(names.has(n), n);
  assert.ok(JSON.parse(read(res + 'element/color.json')).color.some(i => i.name === 'start_window_background'));
  assert.ok(exists(res + 'media/icon.png'));
  assert.ok(exists(h + 'AppScope/resources/base/media/app_icon.png'));
  const pkg = JSON.parse(read(h + 'package.json'));
  assert.equal(pkg.scripts['sync:web'], 'node scripts/sync-web-assets.mjs');
  const script = read(h + 'scripts/sync-web-assets.mjs');
  assert.ok(script.includes("repositoryRoot, 'assets'"));
  assert.ok(script.includes('resources/rawfile'));
  assert.ok(!exists(h + 'entry/src/main/resources/rawfile'));
});
