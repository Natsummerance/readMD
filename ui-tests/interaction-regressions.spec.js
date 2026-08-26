const { test, expect } = require('@playwright/test');

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'zh-CN'));
});

test('search Enter cycles matches without leaving the field', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.evaluate(async () => {
    await renderVirtual(
      'clipboard',
      'search.md',
      '',
      '# Search target\n\nalpha target beta target gamma',
      [],
    );
  });
  await page.locator('#btn-search').click();
  await page.locator('#search-input').fill('target');

  await page.keyboard.press('Enter');
  await expect(page.locator('#search-count')).toHaveText('1/3');
  await page.keyboard.press('Enter');
  await expect(page.locator('#search-count')).toHaveText('1/3');
  await page.keyboard.press('Enter');
  await expect(page.locator('#search-count')).toHaveText('2/3');
  await page.keyboard.press('Shift+Enter');
  await expect(page.locator('#search-count')).toHaveText('1/3');
  await expect(page.locator('#search-input')).toBeFocused();
});

test('welcome Ctrl+P respects the document guard', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => window.__readmdAppReady === true);
  await page.keyboard.press('Control+P');
  await expect(page.locator('#export-modal')).toBeHidden();
  await expect(page.locator('#toast')).toContainText(/请先打开文档|Open a document/);
});

test('reading mode supports F11 Zen and Escape exit', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.evaluate(async () => {
    await renderVirtual('clipboard', 'zen.md', '', '# Reading\n\nFocus body.', []);
  });
  await page.keyboard.press('F11');
  await expect(page.locator('body')).toHaveClass(/zen-mode/);
  await page.keyboard.press('Escape');
  await expect(page.locator('body')).not.toHaveClass(/zen-mode/);
});

test('continuous confirmation escapes back to paged reading', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof togglePaginationMode === 'function');
  await page.evaluate(() => {
    Object.assign(state.pagination, {
      enabled: true,
      mode: 'paged',
      pages: Array.from({ length: 30 }, (_, index) => ({ index })),
      rawContent: '# Continuous target',
    });
    togglePaginationMode();
  });
  const modal = page.locator('#continuous-modal');
  await expect(modal).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(modal).toBeHidden();
  await page.waitForFunction(() => state.pagination.mode === 'paged');
});
