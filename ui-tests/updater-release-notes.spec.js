const { test, expect } = require('@playwright/test');

test('update release notes are sanitized before insertion', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => (
    typeof startUpdateDownload === 'function' &&
    typeof window.renderSafeMarkdown === 'function'
  ));

  await page.evaluate(() => {
    updateInfo = {
      flavor: 'win_installer',
      latest_version: 'v9.0.0',
      published_at: new Date().toISOString(),
      release_notes: [
        '**Safe release note**',
        '<script>window.__updaterXss = true;</script>',
        '<img src="missing.gif" onerror="window.__updaterXss = true">',
        '<div id="update-notes-content" class="modal" role="dialog" aria-hidden="false" aria-live="assertive">Override</div>',
      ].join('\n'),
      asset: null,
    };
    openUpdateModal();
  });

  const notes = page.locator('#update-notes-content');
  await expect(notes.locator('strong')).toHaveText('Safe release note');
  await expect(notes.locator('script')).toHaveCount(0);
  await expect(notes.locator('[onerror]')).toHaveCount(0);
  await expect(notes.locator('[role], [aria-hidden], [aria-live]')).toHaveCount(0);
  await expect(notes.locator('.modal')).toHaveCount(0);
  expect(await page.evaluate(() => window.__updaterXss)).toBeFalsy();
});
