/**
 * ReadMD authentic product capture layer.
 * Every image must show a UI state asserted immediately before capture.
 */
const { test, expect } = require('../ui-tests/node_modules/@playwright/test');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const { loadCaptureConfig, loadShotLibrary } = require('./capture.config.cjs');

const config = loadCaptureConfig();
const library = loadShotLibrary();
const RAW_DIR = path.resolve(__dirname, config.outputDir);
if (!RAW_DIR.startsWith(path.resolve(__dirname))) throw new Error('SHOWCASE_OUTPUT_DIR must remain inside showcase/');
fs.mkdirSync(RAW_DIR, { recursive: true });
for (const entry of fs.readdirSync(RAW_DIR)) {
  if (/\.png$/i.test(entry) || entry === 'capture.json' || entry.endsWith('.metadata.json')) fs.unlinkSync(path.join(RAW_DIR, entry));
}

const DEMO_MD = fs.readFileSync(path.join(__dirname, 'fixtures/readmd-showcase.md'), 'utf-8');

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ locale, theme }) => {
    try {
      localStorage.setItem('readmd_language', locale);
      localStorage.setItem('readmd-settings', JSON.stringify({ theme }));
    } catch (error) {}
  }, { locale: config.locale, theme: config.theme });
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
});

async function openDemo(page) {
  await page.evaluate((content) => {
    // tabs.js currently reads the i18n helper without declaring its local fallback.
    // Install the same semantic fallback before invoking the real renderer.
    if (typeof window._t !== 'function') {
      window._t = (key, params) => (window.i18n ? window.i18n.t(key, params) : key);
    }
    return renderVirtual('virtual', 'ReadMD 研究笔记.md', '', content);
  }, DEMO_MD);
  await expect(page.locator('#content h1')).toBeVisible();
  await expect(page.locator('#content')).toContainText('高斯');
  await expect(page.locator('#content mjx-container').first()).toBeVisible();
  await page.waitForTimeout(900);
}

async function shoot(page, shotId) {
  const shot = library.shots[shotId];
  await page.screenshot({ path: path.join(RAW_DIR, shot.output), type: 'png' });
  const bytes = fs.readFileSync(path.join(RAW_DIR, shot.output));
  const record = {
    shot_id: shotId,
    file: `raw/${shot.output}`,
    feature: shot.name,
    description: shot.description,
    release: config.release,
    authentic: true,
    role: shot.role,
    visuality: shot.visuality,
    capture: {
      viewport: `${config.viewport.width}x${config.viewport.height}`,
      scale: config.scale,
      locale: config.locale,
      theme: config.theme,
    },
    bytes: bytes.length,
    sha256: crypto.createHash('sha256').update(bytes).digest('hex'),
    evidence: shot.evidence,
  };
  // Playwright may isolate spec modules by test; durable sidecars allow the final aggregate gate.
  fs.writeFileSync(path.join(RAW_DIR, `${shot.output}.metadata.json`), JSON.stringify(record, null, 2));
}

async function assertVisible(page, selectors) {
  for (const selector of selectors) await expect(page.locator(selector).first()).toBeVisible();
}

test('overview.reader captures the complete reading interface', async ({ page }) => {
  await openDemo(page);
  await page.evaluate(() => toggleSide('toc'));
  await assertVisible(page, library.shots['overview.reader'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'overview.reader');
});

test('overview.editor captures split editor preview', async ({ page }) => {
  await openDemo(page);
  await page.evaluate(async () => {
    await toggleEdit();
    setPvLayout('left');
  });
  await assertVisible(page, library.shots['overview.editor'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'overview.editor');
});

test('presentation.reveal captures the real presentation iframe', async ({ page }) => {
  await openDemo(page);
  await page.evaluate(() => launchPresentationMode());
  await assertVisible(page, library.shots['presentation.reveal'].assertions);
  const frame = page.frameLocator('.presentation-iframe');
  await frame.locator('.reveal').first().waitFor({ state: 'visible', timeout: 15000 });
  await page.waitForTimeout(1200);
  await shoot(page, 'presentation.reveal');
});

test('editor.diagram-picker captures the diagram modal', async ({ page }) => {
  await openDemo(page);
  await page.evaluate(async () => {
    await toggleEdit();
    openDiagramModal();
  });
  await assertVisible(page, library.shots['editor.diagram-picker'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'editor.diagram-picker');
});

test('academic.latex-bib captures rendered formulas', async ({ page }) => {
  await openDemo(page);
  await page.evaluate(() => {
    document.querySelector('#content .katex-display')?.scrollIntoView({ behavior: 'instant', block: 'center' });
  });
  await assertVisible(page, library.shots['academic.latex-bib'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'academic.latex-bib');
});

test('editor.code-chunk captures runnable code card', async ({ page }) => {
  await openDemo(page);
  await page.evaluate(() => {
    document.querySelector('#content .code-chunk-card')?.scrollIntoView({ behavior: 'instant', block: 'center' });
  });
  await assertVisible(page, library.shots['editor.code-chunk'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'editor.code-chunk');
});

test('convert.home captures the welcome workflow entries', async ({ page }) => {
  await assertVisible(page, library.shots['convert.home'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'convert.home');
});

test('sharing.export captures the mobile sharing panel', async ({ page }) => {
  await page.evaluate(() => openShareModal());
  await assertVisible(page, library.shots['sharing.export'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'sharing.export');
});

test.afterAll(async () => {
  const expectedIds = Object.keys(library.shots);
  const records = fs.readdirSync(RAW_DIR)
    .filter((entry) => entry.endsWith('.metadata.json'))
    .map((entry) => JSON.parse(fs.readFileSync(path.join(RAW_DIR, entry), 'utf8')));
  const order = new Map(expectedIds.map((id, index) => [id, index]));
  records.sort((left, right) => order.get(left.shot_id) - order.get(right.shot_id));
  const actualIds = records.map((shot) => shot.shot_id);
  if (actualIds.length !== expectedIds.length) throw new Error(`Expected ${expectedIds.length} shots, got ${actualIds.length}`);
  const seenFiles = new Set();
  const seenHashes = new Map();
  for (const shot of records) {
    if (seenHashes.has(shot.sha256)) throw new Error(`Duplicate screenshot: ${shot.shot_id} equals ${seenHashes.get(shot.sha256)}`);
    seenHashes.set(shot.sha256, shot.shot_id);
    if (seenFiles.has(shot.file)) throw new Error(`Duplicate output path: ${shot.file}`);
    seenFiles.add(shot.file);
  }
  fs.writeFileSync(
    path.join(RAW_DIR, 'capture.json'),
    JSON.stringify({ schema_version: 1, release: config.release, captured_at: new Date().toISOString(), config, shots: records }, null, 2),
  );
});
