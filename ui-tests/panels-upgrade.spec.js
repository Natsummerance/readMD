// Panels upgrade: welcome → editor, empty docs, export preset gallery, AI config view.
const { test, expect } = require('@playwright/test');

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200, contentType: 'application/json', body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'en'));
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function' && typeof openExportModal === 'function');
});

test('edit from the welcome screen opens a blank document in the editor', async ({ page }) => {
  await expect(page.locator('#btn-edit')).toBeEnabled();
  await page.evaluate(() => toggleEdit());
  await expect(page.locator('#edit-bar')).toBeVisible();
  await page.waitForFunction(() => state.editing === true && state.mode === 'virtual');
});

test('an empty document can be edited', async ({ page }) => {
  await page.evaluate(() => renderVirtual('clipboard', 'empty.md', '', '', []));
  await expect(page.locator('#btn-edit')).toBeEnabled();
  await page.evaluate(() => toggleEdit());
  await page.waitForFunction(() => state.editing === true);
});

test('export preset gallery applies a preset and marks it pressed', async ({ page }) => {
  await page.evaluate(() => renderVirtual('clipboard', 'doc.md', '', '# Title\n\nBody\n', []));
  await page.evaluate(() => openExportModal());
  const cards = page.locator('#exp-preset-cards .exp-preset-card');
  await expect(cards.first()).toBeVisible();
  expect(await cards.count()).toBeGreaterThanOrEqual(4);
  const business = page.locator('#exp-preset-cards [data-preset="business"]');
  await business.click();
  await expect(page.locator('#exp-preset-cards [data-preset="business"]')).toHaveAttribute('aria-pressed', 'true');
  expect(await page.evaluate(() => state.export.options.link.color)).toBe('#1f3864');
  expect(await page.evaluate(() => $('exp-preset').value)).toBe('business');
});

test('AI config answers annotated providers without secrets', async ({ page }) => {
  const cfg = await page.evaluate(async () => (await apiFetch('/api/ai/config')).json());
  expect(cfg.ok).toBe(true);
  expect(Array.isArray(cfg.presets) && cfg.presets.length > 0).toBe(true);
  expect(Array.isArray(cfg.custom)).toBe(true);
  expect(cfg.presets.every(p => typeof p.has_key === 'boolean' && !('api_key' in p))).toBe(true);
});

test('editor opens with live preview and scroll sync on, and remembers the caret', async ({ page }) => {
  await page.evaluate(() => renderVirtual('clipboard', 'mem.md', '', '# A\n\nline two\n\nline three\n', []));
  await page.evaluate(() => toggleEdit());
  await page.waitForFunction(() => window.cmView);
  expect(await page.evaluate(() => [state.pvLayout, state.pvSync])).toEqual(['right', true]);
  await expect(page.locator('#preview-wrap')).toBeVisible();
  await page.evaluate(() => { cmView.dispatch({ selection: { anchor: 9 } }); });
  await page.evaluate(() => toggleEdit());
  await page.waitForFunction(() => !state.editing);
  await page.evaluate(() => toggleEdit());
  await page.waitForFunction(() => window.cmView);
  expect(await page.evaluate(() => cmView.state.selection.main.head)).toBe(9);
});

test('AI provider browser renders a bounded card list', async ({ page }) => {
  await page.evaluate(() => loadAiConfig());
  const n = await page.evaluate(() => document.querySelectorAll('#ai-provider-cards > *').length);
  expect(n).toBeGreaterThan(0);
  expect(n).toBeLessThanOrEqual(80);
});

test('welcome offers New document, and Ctrl+N opens a blank editor', async ({ page }) => {
  await expect(page.locator('#w-new')).toBeVisible();
  await page.locator('#w-new').click();
  await page.waitForFunction(() => state.editing === true && state.mode === 'virtual');
  await page.evaluate(() => { state.editing = false; exitEdit(); });
  await page.keyboard.press('Control+n');
  await page.waitForFunction(() => state.editing === true);
});

test('AI empty state shows six starters and a connect card when no key is set', async ({ page }) => {
  await page.evaluate(() => renderVirtual('clipboard', 'doc.md', '', '# Doc\n\nText.\n', []));
  await page.evaluate(() => toggleAiPanel());
  await expect(page.locator('#ai-output .ai-starter-grid button')).toHaveCount(6);
  await expect(page.locator('#ai-output [data-ai-connect]')).toBeVisible();
  await page.locator('#ai-output [data-starter-id="outline"]').click();
  expect(await page.locator('#ai-prompt').inputValue()).not.toBe('');
});

test('clicking the dim backdrop closes a modal, but not a static one', async ({ page }) => {
  await page.evaluate(() => renderVirtual('clipboard', 'doc.md', '', '# Doc\n', []));
  await page.evaluate(() => openExportModal());
  await expect(page.locator('#export-modal')).toBeVisible();
  await page.mouse.click(5, 5);
  await expect(page.locator('#export-modal')).toBeHidden();
  await page.evaluate(() => $('btn-style-custom').click());
  await expect(page.locator('#style-custom-modal')).toBeVisible();
  await page.mouse.click(5, 5);
  await expect(page.locator('#style-custom-modal')).toBeVisible();
});

test('AI conversation renders a typing state, rich answer, code copy and regenerate', async ({ page }) => {
  let calls = 0;
  await page.route('**/api/ai/config', r => r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({
    ok: true, schema_version: 3, presets: [], upstream_catalog: [],
    custom: [{ id: 'custom:demo', name: 'Demo', custom: true, base_url: 'https://api.example.test/v1', mode: 'chat', endpoint_mode: 'prefix', models: ['demo-model'], has_key: true, key_source: 'configured', credential_id: 'cred:demo1234567' }],
    current: { provider_id: 'custom:demo', model: 'demo-model' } }) }));
  await page.route('**/api/ai/history**', r => r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, session: { id: 's1' }, sessions: [] }) }));
  await page.route('**/api/ai/chat', async r => {
    calls++;
    await new Promise(res => setTimeout(res, 400));
    await r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, content: '## Answer ' + calls + '\n\n```js\nlet a = 1;\n```\n' }) });
  });
  await page.reload();
  await page.waitForFunction(() => typeof toggleAiPanel === 'function');
  await page.evaluate(() => renderVirtual('clipboard', 'doc.md', '', '# Doc\n\nText.\n', []));
  await page.evaluate(() => toggleAiPanel());
  await page.evaluate(() => { $('ai-stream').checked = false; });
  await page.fill('#ai-prompt', 'hello');
  await page.click('#ai-run');
  await expect(page.locator('#ai-output .ai-typing')).toBeVisible();
  await page.waitForFunction(() => !state.ai.busy);
  const answer = page.locator('#ai-output .ai-msg.ai').last();
  await expect(answer.locator('h2')).toHaveText('Answer 1');
  await expect(answer.locator('pre .ai-code-copy')).toHaveCount(1);
  await expect(answer.locator('.ai-tag-name')).not.toHaveText('');
  await answer.locator('.ai-regen-btn').click();
  await page.waitForFunction(() => !state.ai.busy && document.querySelector('#ai-output .ai-msg.ai:last-child h2')?.textContent === 'Answer 2');
  await expect(page.locator('#ai-output .ai-msg.ai')).toHaveCount(1);
  await expect(page.locator('#ai-output .ai-msg.user')).toHaveCount(1);
  expect(await page.evaluate(() => state.ai.messages.length)).toBe(2);
});
