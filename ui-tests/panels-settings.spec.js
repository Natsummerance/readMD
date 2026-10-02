const { test, expect } = require('@playwright/test');
const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');

const demoProvider = {
  id: 'custom:panel-test', name: 'Panel provider', custom: true,
  base_url: 'https://api.example.test/v1', mode: 'chat', endpoint_mode: 'prefix',
  models: ['panel-model'], has_key: true, credential_id: 'cred:paneltest123',
  headers: { 'X-Organization': 'panel-test' },
};
const config = { ok: true, presets: [], custom: [demoProvider], current: { provider_id: demoProvider.id, model: 'panel-model' } };
const plugin = { name: 'Installed OCR', desc_key: 'Recognizes text in local images.', category: 'ocr', installed: true, enabled: true, alternatives: [] };
const json = (route, body, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', r => json(r, { ok: false }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'en'));
  await page.emulateMedia({ reducedMotion: 'reduce' });
});

async function openDoc(page) {
  await page.goto('/');
  await page.waitForFunction(() => typeof openExportModal === 'function' && typeof testAiConnection === 'function');
  await page.evaluate(() => renderVirtual('clipboard', 'panel.md', '', '# Panel title\n\nBody and [link](https://example.test).\n\n| A | B |\n|---|---|\n| One | Two |\n', []));
}

async function openAi(page) {
  await page.route('**/api/ai/config', r => json(r, config));
  await openDoc(page);
  await page.evaluate(async () => { await loadAiConfig(); $('ai-settings-modal').classList.remove('hidden'); });
}

test('connection testing keeps feedback visible, uses saved credentials and resets key visibility', async ({ page }) => {
  const requests = [];
  await page.route('**/api/ai/models', async r => {
    requests.push(r.request().postDataJSON());
    await new Promise(resolve => setTimeout(resolve, 180));
    await json(r, { ok: true, models: ['panel-model'] });
  });
  await openAi(page);
  await page.locator('#ai-key').fill('dummy-panel-input');
  await page.locator('#ai-key-toggle').click();
  await expect(page.locator('#ai-key')).toHaveAttribute('type', 'text');
  await page.locator('#ai-test-connection').click();
  await expect(page.locator('#ai-test-connection')).toBeDisabled();
  await expect(page.locator('#ai-conn-status')).toHaveAttribute('aria-busy', 'true');
  await expect(page.locator('#ai-conn-status')).toContainText(/connected|ready|available/i);
  await expect(page.locator('#ai-settings-modal')).toBeVisible();
  await expect(page.locator('#ai-key')).toHaveValue('');
  await expect(page.locator('#ai-key')).toHaveAttribute('type', 'password');
  await expect(page.locator('#ai-key-toggle')).toHaveAttribute('aria-pressed', 'false');
  await expect(page.locator('#ai-test-connection')).toBeEnabled();
  expect(requests).toHaveLength(1);
  expect(requests[0].credential_id).toBe(demoProvider.credential_id);
  expect(requests[0].headers).toEqual(demoProvider.headers);
  expect(JSON.stringify(requests)).not.toContain('dummy-panel-input');
});

test('connection failures, including HTTP 200 failures, report an inline error', async ({ page }) => {
  await page.route('**/api/ai/models', r => json(r, { ok: false, error: '401 authentication failed' }));
  await openAi(page);
  await page.locator('#ai-test-connection').click();
  await expect(page.locator('#ai-conn-status')).toContainText(/key|authentication/i);
  await expect(page.locator('#ai-settings-modal')).toBeVisible();
  await expect(page.locator('#ai-test-connection')).toBeEnabled();
});

test('failed settings saves and invalid headers stop connection testing', async ({ page }) => {
  let discoveries = 0;
  await page.route('**/api/ai/models', r => { discoveries++; return json(r, { ok: true }); });
  await openAi(page);
  await page.locator('.conn-advanced > summary').click();
  await page.locator('#ai-headers').fill('[]');
  await page.locator('#ai-test-connection').click();
  await expect(page.locator('#ai-conn-status')).toContainText('valid JSON object');
  await page.locator('#ai-headers').fill('{}');
  await page.route('**/api/ai/config', r => json(r, { ok: false, error: 'Storage unavailable' }, 500));
  await page.locator('#ai-test-connection').click();
  await expect(page.locator('#ai-conn-status')).toContainText('Storage unavailable');
  await expect(page.locator('#ai-settings-modal')).toBeVisible();
  expect(discoveries).toBe(0);
});

test('failed preset loading can be retried without caching an empty configuration', async ({ page }) => {
  let failed = true;
  await page.route('**/api/export/presets', r => failed ? json(r, { ok: false }, 500) : r.continue());
  await openDoc(page);
  await page.evaluate(() => openExportModal());
  await expect(page.locator('#toast')).toContainText(/export|load/i);
  expect(await page.evaluate(() => state.export.defaults)).toBeFalsy();
  failed = false;
  await page.evaluate(() => openExportModal());
  await expect(page.locator('#exp-preset-cards [data-preset="business"]')).toBeVisible();
});

test('preset selection and edits survive closing and changing formats', async ({ page }) => {
  await openDoc(page);
  await page.evaluate(() => openExportModal());
  await page.locator('#exp-preset-cards [data-preset="business"]').click();
  await page.locator('.export-customize > summary').click();
  const size = page.locator('#export-opts [data-k="typography.size"]');
  await size.evaluate(el => el.closest('.exp-sec').classList.add('open'));
  await size.fill('13');
  await size.dispatchEvent('change');
  await page.locator('#export-tab-epub').click();
  await page.locator('#export-tab-html').click();
  await expect(size).toHaveValue('13');
  const options = await page.evaluate(() => collectExportOptions());
  expect(options.link.color).toBe('#1f3864');
  expect(options.typography.size).toBe(13);
  await page.locator('#export-close').click();
  await page.evaluate(() => openExportModal());
  await expect(page.locator('#exp-preset-cards [data-preset="__custom__"]')).toHaveAttribute('aria-pressed', 'true');
  await page.locator('#exp-reset').click();
  await expect(page.locator('#exp-preset-cards [data-preset="__default__"]')).toHaveAttribute('aria-pressed', 'true');
  expect(await page.evaluate(() => collectExportOptions().link.color)).toBe('#2b6cb0');
});

test('custom presets save through the HTTP host and remain selected after reload', async ({ page }) => {
  await openDoc(page);
  await page.evaluate(() => openExportModal());
  await page.locator('#exp-preset-cards [data-preset="business"]').click();
  await page.locator('#export-tab-docx').click();
  await page.locator('#exp-save-preset').click();
  const name = 'Panel saved preset with a complete title';
  await page.locator('#exp-save-input').fill(name);
  await page.locator('#exp-save-ok').click();
  await expect(page.locator('#exp-save-name')).toBeHidden();
  await page.reload();
  await page.waitForFunction(() => typeof openExportModal === 'function');
  await page.evaluate(() => { renderVirtual('clipboard', 'again.md', '', '# Again', []); openExportModal(); });
  await expect(page.locator('#exp-preset-cards').getByRole('button', { name })).toHaveAttribute('aria-pressed', 'true');
  expect(await page.evaluate(() => [state.export.fmt, collectExportOptions().link.color])).toEqual(['docx', '#1f3864']);
  await expect(page.locator('#export-tab-docx')).toHaveAttribute('aria-selected', 'true');
});

test('failed custom preset saves keep the input and show a failure', async ({ page }) => {
  await openDoc(page);
  await page.evaluate(() => openExportModal());
  await page.route('**/api/export/presets', r => r.request().method() === 'POST' ? json(r, { ok: false }, 500) : r.continue());
  await page.locator('#exp-save-preset').click();
  await page.locator('#exp-save-input').fill('Failed preset');
  await page.locator('#exp-save-ok').click();
  await expect(page.locator('#export-result')).toContainText('Could not save');
  await expect(page.locator('#exp-save-name')).toBeVisible();
  expect(await page.evaluate(() => 'Failed preset' in state.export.custom)).toBe(false);
});

test('paper size, zero margins and disabled page decoration reach the preview', async ({ page }) => {
  await openDoc(page);
  await page.evaluate(() => openExportModal());
  await page.locator('#export-tab-pdf').click();
  await page.locator('#exp-reset').click();
  await page.locator('.export-customize > summary').click();
  await page.locator('[data-k="page.size"]').selectOption('A5');
  await page.locator('[data-k="page.marginTop"]').fill('0');
  await page.locator('[data-k="typography.spacing"]').evaluate(el => el.closest('.exp-sec').classList.add('open'));
  await page.locator('[data-k="typography.spacing"]').fill('0');
  await page.locator('[data-k="footer.pageNumbers"]').evaluate(el => el.closest('.exp-sec').classList.add('open'));
  await page.locator('[data-k="footer.pageNumbers"]').uncheck();
  await page.locator('#export-preview-card').click();
  const sheet = page.locator('.export-preview-page-sheet').first();
  const dimensions = await sheet.evaluate(el => ({ width: el.getBoundingClientRect().width, height: el.getBoundingClientRect().height, padding: getComputedStyle(el).paddingTop }));
  expect(dimensions.width).toBeCloseTo(148 * 96 / 25.4, 0);
  expect(dimensions.height).toBeCloseTo(210 * 96 / 25.4, 0);
  expect(dimensions.padding).toBe('0px');
  await expect(page.locator('.export-page-header, .export-page-footer')).toHaveCount(0);
  expect(await sheet.locator('p').first().evaluate(el => getComputedStyle(el).marginBottom)).toBe('0px');
});

// Read a DOCX part using only Node's ZIP and compression primitives.
function zipPart(bytes, wanted) {
  let at = bytes.indexOf(Buffer.from([0x50, 0x4b, 0x01, 0x02]));
  while (at >= 0 && bytes.readUInt32LE(at) === 0x02014b50) {
    const method = bytes.readUInt16LE(at + 10), size = bytes.readUInt32LE(at + 20);
    const nameSize = bytes.readUInt16LE(at + 28), extraSize = bytes.readUInt16LE(at + 30), commentSize = bytes.readUInt16LE(at + 32);
    const name = bytes.subarray(at + 46, at + 46 + nameSize).toString();
    if (name === wanted) {
      const local = bytes.readUInt32LE(at + 42);
      const start = local + 30 + bytes.readUInt16LE(local + 26) + bytes.readUInt16LE(local + 28);
      const data = bytes.subarray(start, start + size);
      return (method === 8 ? zlib.inflateRawSync(data) : data).toString();
    }
    at += 46 + nameSize + extraSize + commentSize;
  }
  throw new Error('DOCX part missing: ' + wanted);
}

test('HTML, PDF and DOCX export the selected preset and the previewed document', async ({ page }, testInfo) => {
  const payloads = [], outputs = [];
  await page.route('**/api/export', async r => {
    const payload = r.request().postDataJSON();
    payloads.push(payload);
    const outPath = testInfo.outputPath('business.' + payload.format);
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    const response = await r.fetch({ postData: JSON.stringify({ ...payload, out_path: outPath }) });
    outputs.push({ format: payload.format, outPath, result: await response.json() });
    await r.fulfill({ response });
  });
  await openDoc(page);
  await page.evaluate(() => openExportModal());
  await page.locator('#exp-preset-cards [data-preset="business"]').click();
  for (const format of ['html', 'pdf', 'docx']) {
    await page.locator('#export-tab-' + format).click();
    const previewOptions = await page.evaluate(() => collectExportOptions());
    await page.locator('#export-run').click();
    await expect(page.locator('#export-result')).toHaveClass(/ok/);
    expect(payloads.at(-1).options).toEqual(previewOptions);
    expect(payloads.at(-1).options.link.color).toBe('#1f3864');
    expect(outputs.at(-1).result.ok).toBe(true);
  }
  const html = fs.readFileSync(outputs[0].outPath, 'utf8');
  expect(html).toContain('#1f3864');
  expect(html).toContain('Panel title');
  await page.locator('#export-tab-html').click();
  await page.locator('#export-preview-card').click();
  const exported = await page.context().newPage();
  await exported.route('**/panel-export.html', r => r.fulfill({ contentType: 'text/html', body: html }));
  await exported.goto(new URL('/panel-export.html', page.url()).href);
  await expect(exported.locator('#content h1')).toHaveText('Panel title');
  const styles = async (target, root) => target.evaluate(root => ['h1', 'p', 'a', 'th'].map(selector => {
    const style = getComputedStyle(document.querySelector(root + ' ' + selector));
    return { color: style.color, fontSize: style.fontSize, weight: style.fontWeight, align: style.textAlign };
  }), root);
  expect(await styles(page, '#export-preview-full-page')).toEqual(await styles(exported, '#content'));
  await exported.close();
  const pdf = fs.readFileSync(outputs[1].outPath);
  expect(pdf.subarray(0, 5).toString()).toBe('%PDF-');
  expect(pdf.length).toBeGreaterThan(1000);
  const docx = zipPart(fs.readFileSync(outputs[2].outPath), 'word/document.xml');
  expect(docx).toContain('Panel title');
  expect(docx.toLowerCase()).toContain('1f3864');
});

test('plugin loading, empty categories and failed loads offer visible feedback and retry', async ({ page }) => {
  let release;
  const ready = new Promise(resolve => { release = resolve; });
  await page.route('**/api/plugins/list', async r => { await ready; await json(r, { ok: false }, 500); });
  await openDoc(page);
  await page.evaluate(() => { openPluginModal(); });
  await expect(page.locator('#plugin-list-status')).toContainText('Loading plugins');
  release();
  await expect(page.locator('#plugin-list-status')).toContainText('Could not load');
  await page.route('**/api/plugins/list', r => json(r, { ok: true, plugins: {} }));
  await page.locator('#plugin-list-status button').click();
  await expect(page.locator('#plugin-list-status')).toContainText('No plugins');
});

test('plugin toggles save once, survive reload and restore their state on failure', async ({ page }) => {
  let enabled = true, fail = false, writes = 0;
  await page.route('**/api/plugins/list', r => json(r, { ok: true, plugins: { easyocr: { ...plugin, enabled } } }));
  await page.route('**/api/plugins/toggle', async r => {
    writes++;
    await new Promise(resolve => setTimeout(resolve, 120));
    if (fail) return json(r, { ok: false }, 500);
    enabled = r.request().postDataJSON().enabled;
    await json(r, { ok: true, enabled });
  });
  await openDoc(page);
  await page.evaluate(() => openPluginModal());
  const toggle = page.locator('[data-plugin-id="easyocr"] input[data-action="toggle"]');
  await page.locator('[data-plugin-id="easyocr"] .plugin-switch').click();
  await expect(toggle).toBeDisabled();
  await expect(toggle).toBeEnabled();
  await expect(toggle).not.toBeChecked();
  await page.reload();
  await page.waitForFunction(() => typeof openPluginModal === 'function');
  await page.evaluate(() => openPluginModal());
  await expect(toggle).not.toBeChecked();
  fail = true;
  await page.locator('[data-plugin-id="easyocr"] .plugin-switch').click();
  await expect(page.locator('#plugin-list-status')).toContainText(/failed|could not/i);
  await expect(toggle).not.toBeChecked();
  expect(writes).toBe(2);
});

test('plugin switch persistence reaches the native manifest', async ({ page }) => {
  await openDoc(page);
  const list = await page.evaluate(async () => (await apiFetch('/api/plugins/list')).json());
  const sandbox = path.resolve(list.sandbox_dir);
  const temp = path.resolve(require('node:os').tmpdir()) + path.sep;
  expect(sandbox.toLowerCase().startsWith(temp.toLowerCase())).toBe(true);
  expect(sandbox).toContain('readmd-ui-test-');
  // The native detector only needs local package metadata; no installer runs.
  fs.mkdirSync(path.join(sandbox, 'site-packages', 'jieba'), { recursive: true });
  fs.mkdirSync(path.join(sandbox, 'site-packages', 'jieba-0.0.dist-info'), { recursive: true });
  await page.evaluate(() => openPluginModal());
  const toggle = page.locator('[data-plugin-id="jieba"] input[data-action="toggle"]');
  await expect(toggle).not.toBeChecked();
  await page.locator('[data-plugin-id="jieba"] .plugin-switch').click();
  await expect(toggle).toBeChecked();
  await expect(toggle).toBeEnabled();
  await page.reload();
  await page.waitForFunction(() => typeof openPluginModal === 'function');
  await page.evaluate(() => openPluginModal());
  await expect(toggle).toBeChecked();
  const saved = await page.evaluate(async () => (await apiFetch('/api/plugins/list')).json());
  expect(saved.plugins.jieba.enabled).toBe(true);
  await page.locator('[data-plugin-id="jieba"] .plugin-switch').click();
  await expect(toggle).toBeEnabled();
  expect((await page.evaluate(async () => (await apiFetch('/api/plugins/list')).json())).plugins.jieba.enabled).toBe(false);
});

async function auditPanel(page, selector) {
  const issues = await page.locator(selector).evaluate(async root => {
    const issues = [];
    const visible = el => {
      if (!el.getClientRects().length || getComputedStyle(el).visibility === 'hidden') return false;
      for (let parent = el.parentElement; parent && parent !== root; parent = parent.parentElement) {
        if (parent.tagName === 'DETAILS' && !parent.open && !parent.querySelector(':scope > summary')?.contains(el)) return false;
      }
      return true;
    };
    const elements = [...root.querySelectorAll('h3, h4, label, summary, button, .exp-preset-name, .plugin-card-submeta, .conn-status, .plugin-list-status')].filter(visible);
    for (const el of elements) {
      el.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'instant' });
      await new Promise(resolve => requestAnimationFrame(resolve));
      const rect = el.getBoundingClientRect();
      const hit = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
      if (!hit || (hit !== el && !el.contains(hit) && !hit.contains(el))) issues.push('covered: ' + (el.id || el.textContent.trim().slice(0, 55)));
      if (el.scrollWidth > el.clientWidth + 1 && !el.matches('.export-preview-card')) issues.push('clipped: ' + (el.id || el.textContent.trim().slice(0, 55)));
    }
    if (root.scrollWidth > root.clientWidth + 1) issues.push('horizontal panel overflow');
    for (const el of [...root.querySelectorAll('button, select, input:not([type="checkbox"]), textarea, summary, .plugin-switch, .exp-check, .ai-sel')].filter(visible)) {
      const r = el.getBoundingClientRect();
      if (Math.min(r.width, r.height) < 43.5) issues.push('small target: ' + (el.id || el.className));
    }
    return issues;
  });
  expect(issues, selector).toEqual([]);
}

for (const locale of ['en', 'zh-CN', 'zh-TW']) {
  for (const size of [{ width: 1160, height: 820 }, { width: 1024, height: 680 }]) {
    test(`panels have no covered text and 44px controls at ${size.width}x${size.height} in ${locale}`, async ({ browser }, testInfo) => {
      const context = await browser.newContext({ viewport: size, deviceScaleFactor: 1.5, reducedMotion: 'reduce' });
      const page = await context.newPage();
      try {
        await page.addInitScript(language => localStorage.setItem('readmd_language', language), locale);
        await page.route('**/api/update/check', r => json(r, { ok: false }));
        await page.route('**/api/ai/config', r => json(r, config));
        await page.route('**/api/plugins/list', r => json(r, { ok: true, ffmpeg: true, plugins: {
          easyocr: { ...plugin, desc_key: 'A long local OCR description that should remain limited to two readable lines. '.repeat(3) },
          docling: { ...plugin, name: 'Document parser', category: 'document', installed: false, enabled: false },
        } }));
        await openDoc(page);
        await page.evaluate(async () => { await loadAiConfig(); $('ai-settings-modal').classList.remove('hidden'); });
        await expect(page.locator('#ai-model')).toBeVisible();
        await expect(page.locator('#ai-key')).toBeVisible();
        await page.locator('.conn-advanced > summary').click();
        await page.locator('.ai-provider-directory > summary').click();
        await auditPanel(page, '#ai-settings-box');
        await page.locator('#ai-settings-close').click();
        await page.evaluate(() => openExportModal());
        await expect(page.locator('#exp-preset-cards')).toBeVisible();
        await page.evaluate(() => {
          const name = 'Long preset name with complete readable text';
          state.export.custom[name] = exportPresetOptions('business');
          renderExportPresetSelect();
        });
        await page.locator('.export-customize > summary').click();
        await page.locator('.export-preset-list > summary').click();
        await page.evaluate(() => document.querySelectorAll('#export-opts .exp-sec').forEach(section => section.classList.add('open')));
        await auditPanel(page, '#export-box');
        await page.locator('#export-close').click();
        await page.evaluate(() => openPluginModal());
        await page.locator('.plugin-runtime-details > summary').click();
        await auditPanel(page, '#plugin-box');
        const lineClamp = await page.locator('.plugin-card-desc').first().evaluate(el => ({ height: el.clientHeight, line: parseFloat(getComputedStyle(el).lineHeight) }));
        expect(lineClamp.height).toBeLessThanOrEqual(lineClamp.line * 2 + 1);
        if (process.env.READMD_PANEL_SCREENSHOTS) {
          for (const panel of ['ai-settings', 'export', 'plugin']) {
            await page.evaluate(id => document.querySelectorAll('#ai-settings-modal, #export-modal, #plugin-modal').forEach(el => el.classList.toggle('hidden', el.id !== id + '-modal')), panel);
            await page.evaluate(() => document.querySelectorAll('.ai-settings-body, .export-side, .export-main-col, .plugin-grid').forEach(el => { el.scrollTop = 0; }));
            await page.screenshot({ path: path.join(process.env.READMD_PANEL_SCREENSHOTS, `${panel}-${locale}-${size.width}x${size.height}.png`) });
          }
        }
      } finally { await context.close(); }
    });
  }
}
