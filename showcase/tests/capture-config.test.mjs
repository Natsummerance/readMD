import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const require = createRequire(import.meta.url);
const { loadCaptureConfig, loadShotLibrary } = require('../capture.config.cjs');

test('capture config uses safe defaults and accepts environment overrides', () => {
  const config = loadCaptureConfig({});
  assert.equal(config.release, 'v2.3.7');
  assert.equal(config.locale, 'zh-CN');
  assert.equal(config.theme, 'dark');
  assert.deepEqual(config.viewport, { width: 1440, height: 810 });
  assert.equal(config.scale, 2);
  assert.equal(config.outputDir, 'raw');

  const overridden = loadCaptureConfig({
    SHOWCASE_RELEASE: 'v9.9.9',
    SHOWCASE_LOCALE: 'en-US',
    SHOWCASE_THEME: 'light',
    SHOWCASE_VIEWPORT: '1440x900',
    SHOWSCALE: 'ignored',
    SHOWCASE_SCALE: '1',
    SHOWCASE_OUTPUT_DIR: 'dist/raw',
  });
  assert.equal(overridden.release, 'v9.9.9');
  assert.equal(overridden.locale, 'en-US');
  assert.equal(overridden.theme, 'light');
  assert.deepEqual(overridden.viewport, { width: 1440, height: 900 });
  assert.equal(overridden.scale, 1);
  assert.equal(overridden.outputDir, 'dist/raw');
});

test('shot library defines the thirteen stable authentic shots', () => {
  const library = loadShotLibrary();
  const ids = Object.keys(library.shots);
  assert.deepEqual(ids, [
    'overview.reader',
    'overview.editor',
    'presentation.reveal',
    'editor.diagram-picker',
    'academic.latex-bib',
    'editor.code-chunk',
    'convert.home',
    'sharing.export',
    'ai.panel',
    'skills.workbench',
    'providers.catalog',
    'skills.github-import',
    'i18n.library',
  ]);
  for (const [id, shot] of Object.entries(library.shots)) {
    assert.equal(shot.id, id);
    assert.ok(shot.name.length > 0);
    assert.ok(shot.description.length > 0);
    assert.match(shot.output, /^[\w.-]+\.png$/);
    assert.ok(Array.isArray(shot.evidence) && shot.evidence.length > 0);
    assert.ok(Array.isArray(shot.assertions) && shot.assertions.length > 0);
    assert.ok(shot.visuality >= 0 && shot.visuality <= 1);
  }
});

test('shot overlay adjusts assertions without changing evidence identity', async () => {
  const root = await mkdtemp(join(tmpdir(), 'readmd-shot-overlay-'));
  const overlayPath = join(root, 'overlay.json');
  try {
    await writeFile(overlayPath, JSON.stringify({
      schema_version: 1,
      shots: {
        'presentation.reveal': { assertions: ['#presentation-modal'], visuality: 0.9 },
      },
    }));
    const library = loadShotLibrary(undefined, overlayPath);
    assert.deepEqual(library.shots['presentation.reveal'].assertions, ['#presentation-modal']);
    assert.equal(library.shots['presentation.reveal'].visuality, 0.9);
    assert.equal(library.shots['presentation.reveal'].output, 'presentation-reveal.png');
    assert.equal(library.shots['presentation.reveal'].evidence.length > 0, true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
