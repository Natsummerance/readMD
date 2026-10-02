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
