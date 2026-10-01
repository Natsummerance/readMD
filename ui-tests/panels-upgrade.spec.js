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
