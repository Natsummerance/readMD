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

const FIXTURE_DIR = path.join(__dirname, 'fixtures');
const SKILL_ROOT = path.resolve(__dirname, '..', 'assets', 'skills');
const PROVIDER_CATALOG = JSON.parse(fs.readFileSync(path.resolve(__dirname, '..', 'assets', 'providers', 'provider-catalog.json'), 'utf8'));

function builtinSkillFixtures() {
  return fs.readdirSync(SKILL_ROOT, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => {
      const folder = path.join(SKILL_ROOT, entry.name);
      const instructions = fs.readFileSync(path.join(folder, 'SKILL.md'), 'utf8');
      const metadata = JSON.parse(fs.readFileSync(path.join(folder, 'readmd.skill.json'), 'utf8'));
      const name = (instructions.match(/^name:\s*(.+)$/m) || [])[1] || metadata.id;
      const description = (instructions.match(/^description:\s*(.+)$/m) || [])[1] || metadata.adaptation || '';
      return {
        id: metadata.id,
        name,
        description,
        instructions,
        scope: 'builtin',
        metadata,
        variables: [...instructions.matchAll(/{{([a-z_]+)}}/g)].map((match) => match[1]),
      };
    })
    .sort((left, right) => left.id.localeCompare(right.id));
}

const BUILTIN_SKILLS = builtinSkillFixtures();

function providerFixtures() {
  return (PROVIDER_CATALOG.providers || []).slice(0, 12).map((provider, index) => ({
    ...provider,
    id: `catalog:${index}:${String(provider.name || 'provider').replace(/[^a-z0-9]+/gi, '-').toLowerCase()}`,
    has_key: index === 0,
    key_source: index === 0 ? 'credential-store' : '',
    credential_id: index === 0 ? 'cred:showcase-fixture' : '',
    category: provider.category || 'preset',
    capabilities: provider.capabilities || { chat: true, models: true, stream: true },
  }));
}

async function seedAiServices(page) {
  const providers = providerFixtures();
  const upstreamCatalog = (PROVIDER_CATALOG.upstream_entries || []).slice(0, 12);
  const imported = BUILTIN_SKILLS.slice(0, 3).map((skill) => ({
    id: skill.id,
    name: skill.name,
    description: skill.description,
    path: `assets/skills/${skill.id}/SKILL.md`,
    directory: `assets/skills/${skill.id}`,
    valid: true,
    scripts_present: false,
  }));
  await page.route('**/api/modules/load', (route) => route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true}' }));
  await page.route('**/api/ai/config', (route) => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ current: { provider_id: providers[0].id, model: providers[0].models?.[0] || '' }, custom: [], presets: providers, upstream_catalog: upstreamCatalog }),
  }));
  await page.route('**/api/ai/prompts', (route) => route.fulfill({ status: 200, contentType: 'application/json', body: '{"templates":[]}' }));
  await page.route('**/api/ai/history', (route) => route.fulfill({ status: 200, contentType: 'application/json', body: '{"sessions":[]}' }));
  await page.route('**/api/skills', (route) => route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ skills: BUILTIN_SKILLS }) }));
  await page.route('**/api/skill-imports/preview', (route) => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      ok: true,
      preview: {
        source_id: 'gh-readmd-v237-showcase',
        source: { canonical_url: 'https://github.com/Natsummerance/readMD' },
        skills: imported,
      },
    }),
  }));
}

async function openReadyAi(page) {
  await seedAiServices(page);
  await page.evaluate(() => toggleAiPanel());
  await expect(page.locator('#ai-panel')).toBeVisible();
  await expect.poll(() => page.locator('#ai-template option[value]').count()).toBeGreaterThan(1);
  await expect(page.locator('#ai-model')).toHaveValue(/.+/);
}

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', (route) => route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true,"update":false}' }));
  await page.addInitScript(({ locale, theme }) => {
    try {
      localStorage.setItem('readmd_language', locale);
      localStorage.setItem('readmd-settings', JSON.stringify({ theme }));
    } catch (error) {}
  }, { locale: config.locale, theme: config.theme });
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
});

function shotViewport(shotId) {
  const shot = library.shots[shotId];
  return shot.viewport || config.viewport;
}

async function prepareShot(page, shotId) {
  await page.setViewportSize(shotViewport(shotId));
}

async function openDemo(page, shotId = 'overview.reader') {
  const fixture = library.shots[shotId].fixture || 'readmd-showcase.md';
  const content = fs.readFileSync(path.join(FIXTURE_DIR, fixture), 'utf-8');
  await page.evaluate(({ content, name }) => {
    // tabs.js currently reads the i18n helper without declaring its local fallback.
    // Install the same semantic fallback before invoking the real renderer.
    if (typeof window._t !== 'function') {
      window._t = (key, params) => (window.i18n ? window.i18n.t(key, params) : key);
    }
    return renderVirtual('virtual', name, '', content);
  }, { content, name: fixture.replace(/\.md$/i, '') + '.md' });
  await expect(page.locator('#content h1')).toBeVisible();
  await page.waitForTimeout(900);
}

async function saveShot(shotId, outputPath, bytes) {
  const shot = library.shots[shotId];
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
      viewport: `${shotViewport(shotId).width}x${shotViewport(shotId).height}`,
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

async function shoot(page, shotId) {
  const outputPath = path.join(RAW_DIR, library.shots[shotId].output);
  await page.screenshot({ path: outputPath, type: 'png' });
  await saveShot(shotId, outputPath, fs.readFileSync(outputPath));
}

async function assertVisible(page, selectors) {
  for (const selector of selectors) await expect(page.locator(selector).first()).toBeVisible();
}

test('overview.reader captures the complete reading interface', async ({ page }) => {
  await prepareShot(page, 'overview.reader');
  await openDemo(page, 'overview.reader');
  await page.evaluate(() => toggleSide('toc'));
  await assertVisible(page, library.shots['overview.reader'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'overview.reader');
});

test('overview.editor captures split editor preview', async ({ page }) => {
  await prepareShot(page, 'overview.editor');
  await openDemo(page, 'overview.editor');
  await page.evaluate(async () => {
    await toggleEdit();
    setPvLayout('left');
  });
  await assertVisible(page, library.shots['overview.editor'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'overview.editor');
});

test('presentation.reveal captures the real presentation iframe', async ({ page }) => {
  await prepareShot(page, 'presentation.reveal');
  await openDemo(page, 'presentation.reveal');
  await page.evaluate(() => launchPresentationMode());
  await assertVisible(page, library.shots['presentation.reveal'].assertions);
  const frame = page.frameLocator('.presentation-iframe');
  await frame.locator('.reveal').first().waitFor({ state: 'visible', timeout: 15000 });
  await page.waitForTimeout(3000);
  const presentationFrame = page.frames().find((candidate) => candidate !== page.mainFrame() && candidate.url() === 'about:srcdoc');
  await expect.poll(() => presentationFrame.evaluate(() => Boolean(window.deck || window.Reveal)), { timeout: 15000 }).toBe(true);
  await page.waitForTimeout(1200);
  const outputPath = path.join(RAW_DIR, library.shots['presentation.reveal'].output);
  await presentationFrame.locator('.reveal').screenshot({
    path: outputPath,
    type: 'png',
  });
  await saveShot('presentation.reveal', outputPath, fs.readFileSync(outputPath));
});

test('editor.diagram-picker captures the diagram modal', async ({ page }) => {
  await prepareShot(page, 'editor.diagram-picker');
  await openDemo(page, 'editor.diagram-picker');
  await page.evaluate(async () => {
    await toggleEdit();
    openDiagramModal();
  });
  await assertVisible(page, library.shots['editor.diagram-picker'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'editor.diagram-picker');
});

test('academic.latex-bib captures rendered formulas', async ({ page }) => {
  await prepareShot(page, 'academic.latex-bib');
  await openDemo(page, 'academic.latex-bib');
  await page.evaluate(() => {
    document.querySelector('#content .katex-display')?.scrollIntoView({ behavior: 'instant', block: 'center' });
  });
  await assertVisible(page, library.shots['academic.latex-bib'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'academic.latex-bib');
});

test('editor.code-chunk captures runnable code card', async ({ page }) => {
  await prepareShot(page, 'editor.code-chunk');
  await openDemo(page, 'editor.code-chunk');
  await page.evaluate(() => {
    document.querySelector('#content .code-chunk-card')?.scrollIntoView({ behavior: 'instant', block: 'center' });
  });
  await assertVisible(page, library.shots['editor.code-chunk'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'editor.code-chunk');
});

test('convert.home captures the welcome workflow entries', async ({ page }) => {
  await prepareShot(page, 'convert.home');
  await assertVisible(page, library.shots['convert.home'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'convert.home');
});

test('sharing.export captures the mobile sharing panel', async ({ page }) => {
  await prepareShot(page, 'sharing.export');
  await page.evaluate(() => openShareModal());
  await assertVisible(page, library.shots['sharing.export'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'sharing.export');
});

test('ai.panel captures the Skill-first AI conversation surface', async ({ page }) => {
  await prepareShot(page, 'ai.panel');
  await openDemo(page, 'ai.panel');
  await openReadyAi(page);
  await assertVisible(page, library.shots['ai.panel'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'ai.panel');
});

test('skills.workbench captures builtin Skills from the real registry', async ({ page }) => {
  await prepareShot(page, 'skills.workbench');
  await openDemo(page, 'skills.workbench');
  await openReadyAi(page);
  await page.evaluate(() => openTplModal());
  await assertVisible(page, library.shots['skills.workbench'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'skills.workbench');
});

test('providers.catalog captures provider v3 configuration from the offline catalogue', async ({ page }) => {
  await prepareShot(page, 'providers.catalog');
  await openReadyAi(page);
  await page.evaluate(() => document.getElementById('ai-settings-modal').classList.remove('hidden'));
  await assertVisible(page, library.shots['providers.catalog'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'providers.catalog');
});

test('skills.github-import captures a preview derived from the current builtin Skill source', async ({ page }) => {
  await prepareShot(page, 'skills.github-import');
  await openReadyAi(page);
  await page.evaluate(() => openTplModal());
  await page.locator('#tpl-github-import summary').click();
  await page.locator('#tpl-github-url').fill('https://github.com/Natsummerance/readMD/tree/v2.3.7/assets/skills');
  await page.locator('#tpl-github-preview-btn').click();
  await assertVisible(page, library.shots['skills.github-import'].assertions);
  await page.waitForTimeout(350);
  await shoot(page, 'skills.github-import');
});

test('i18n.library captures the full language chooser', async ({ page }) => {
  await prepareShot(page, 'i18n.library');
  await page.evaluate(() => i18n.openModal());
  await assertVisible(page, library.shots['i18n.library'].assertions);
  await expect(page.locator('#lang-grid [role=option]')).toHaveCount(46);
  await page.waitForTimeout(350);
  await shoot(page, 'i18n.library');
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
