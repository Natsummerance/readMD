// Shell upgrade: command palette, shortcut cheat-sheet, tab indicator,
// welcome screen and narrow-screen layout.
const { test, expect } = require('@playwright/test');

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200, contentType: 'application/json', body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => {
    localStorage.setItem('readmd_language', 'zh-CN');
    localStorage.removeItem('readmd.palette.mru');
  });
});

async function boot(page) {
  await page.goto('/');
  await page.waitForFunction(() => window.__readmdAppReady === true && window.ReadMDPalette && window.ReadMDModal);
}

async function openDoc(page, name = 'shell.md', body = '# Shell\n\n## Section\n\nBody text.\n') {
  await page.evaluate(async ([n, md]) => { await renderVirtual('clipboard', n, '', md, []); }, [name, body]);
}

test('Ctrl+K opens the palette as a managed modal layer with a listbox', async ({ page }) => {
  await boot(page);
  await page.locator('#btn-search').focus().catch(() => {});
  await page.keyboard.press('Control+k');
  const modal = page.locator('#command-palette-modal');
  await expect(modal).toBeVisible();
  await expect(modal).toHaveAttribute('role', 'dialog');
  await expect(modal).toHaveAttribute('aria-modal', 'true');
  const input = page.locator('#cmdp-input');
  await expect(input).toBeFocused();
  await expect(input).toHaveAttribute('role', 'combobox');
  await expect(page.locator('#cmdp-list')).toHaveAttribute('role', 'listbox');
  expect(await page.evaluate(() => window.ReadMDModal.top() && window.ReadMDModal.top().id)).toBe('command-palette-modal');
  // Grouped sections with labelled groups and options.
  expect(await page.locator('.rm-palette-group[role="group"]').count()).toBeGreaterThan(3);
  expect(await page.locator('.rm-palette-item[role="option"]').count()).toBeGreaterThan(10);
  // Active descendant tracks keyboard navigation.
  const first = await input.getAttribute('aria-activedescendant');
  await page.keyboard.press('ArrowDown');
  const second = await input.getAttribute('aria-activedescendant');
  expect(second).not.toBe(first);
  await expect(page.locator('#' + second)).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('ArrowUp');
  await expect(input).toHaveAttribute('aria-activedescendant', first);
  // Focus is trapped inside the layer.
  for (let i = 0; i < 6; i++) {
    await page.keyboard.press('Tab');
    expect(await page.evaluate(() => document.getElementById('command-palette-modal').contains(document.activeElement))).toBe(true);
  }
  // Background is inert while open.
  expect(await page.locator('[data-rm-inert]').count()).toBeGreaterThan(0);
  await page.keyboard.press('Escape');
  await expect(modal).toBeHidden();
  expect(await page.locator('[data-rm-inert]').count()).toBe(0);
});

test('palette fuzzy search ranks the best match first and shows shortcut hints', async ({ page }) => {
  await boot(page);
  await openDoc(page);
  await page.keyboard.press('Control+Shift+P');
  await expect(page.locator('#command-palette-modal')).toBeVisible();
  await page.locator('#cmdp-input').fill('导出为 PDF');
  const top = page.locator('.rm-palette-item').first();
  await expect(top).toContainText('PDF');
  await expect(top).toHaveAttribute('aria-selected', 'true');
  // Keyword matching: "zen" finds Zen mode in any UI language.
  await page.locator('#cmdp-input').fill('zen');
  await expect(page.locator('.rm-palette-item').first()).toContainText(/禅|Zen/);
  await expect(page.locator('.rm-palette-item').first().locator('.rm-palette-keys kbd')).toHaveText('F11');
  // Empty state.
  await page.locator('#cmdp-input').fill('qqqzzzxxx');
  await expect(page.locator('.rm-palette-empty')).toBeVisible();
  await expect(page.locator('.rm-palette-item')).toHaveCount(0);
  await expect(page.locator('#cmdp-input')).not.toHaveAttribute('aria-activedescendant', /.+/);
  await page.keyboard.press('Escape');
  await expect(page.locator('#command-palette-modal')).toBeHidden();
});

test('palette executes the existing feature and ranks it first next time', async ({ page }) => {
  await boot(page);
  await openDoc(page);
  await page.keyboard.press('Control+k');
  await page.locator('#cmdp-input').fill('sepia');
  await page.keyboard.press('Enter');
  await expect(page.locator('#command-palette-modal')).toBeHidden();
  await expect.poll(() => page.evaluate(() => [state.theme, document.body.dataset.theme].join('/'))).toBe('sepia/sepia');

  // Opening a dialog from the palette: focus returns to the original opener.
  await page.locator('#btn-search').focus();
  await page.keyboard.press('Control+k');
  await page.locator('#cmdp-input').fill('导出文档');
  await page.keyboard.press('Enter');
  await expect(page.locator('#export-modal')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('#export-modal')).toBeHidden();
  await expect(page.locator('#btn-search')).toBeFocused();

  // Recently used actions are listed first on an empty query.
  await page.keyboard.press('Control+k');
  await expect(page.locator('.rm-palette-group-label').first()).toHaveText(/最近使用|Recently used/);
  await expect(page.locator('.rm-palette-item').first()).toContainText(/导出文档|Export Document/);
  // Clicking an option runs it too.
  await page.locator('#cmdp-input').fill('目录');
  await page.locator('.rm-palette-item').first().click();
  await expect(page.locator('#command-palette-modal')).toBeHidden();
  await expect(page.locator('#side')).toBeVisible();
});

test('toolbar palette button and welcome search open the palette', async ({ page }) => {
  await boot(page);
  await page.locator('.welcome-search').click();
  await expect(page.locator('#command-palette-modal')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('.welcome-search')).toBeFocused();
  await page.locator('#btn-palette').click();
  await expect(page.locator('#command-palette-modal')).toBeVisible();
  await page.keyboard.press('Control+k'); // same chord toggles it closed
  await expect(page.locator('#command-palette-modal')).toBeHidden();
});

test('shortcut cheat-sheet opens with ? and Ctrl+/, is generated from the registry and filters', async ({ page }) => {
  await boot(page);
  await page.locator('body').click({ position: { x: 5, y: 300 } }).catch(() => {});
  await page.keyboard.press('Shift+Slash');
  const sheet = page.locator('#shortcuts-modal');
  await expect(sheet).toBeVisible();
  await expect(sheet).toHaveAttribute('role', 'dialog');
  await expect(page.locator('#shortcuts-filter')).toBeFocused();
  // Every registry action with a shortcut is listed.
  const expected = await page.evaluate(() => window.ReadMDActions.list().filter(a => a.shortcut && !a.hidden && a.id !== 'help.shortcuts').length);
  expect(await page.locator('.rm-keys-row').count()).toBeGreaterThanOrEqual(expected);
  await expect(sheet).toContainText('Ctrl');
  await page.locator('#shortcuts-filter').fill('导出');
  await expect(page.locator('.rm-keys-row').first()).toContainText(/导出/);
  await page.keyboard.press('Escape');
  await expect(sheet).toBeHidden();

  await page.keyboard.press('Control+/');
  await expect(sheet).toBeVisible();
  await page.locator('#shortcuts-close').click();
  await expect(sheet).toBeHidden();

  // The palette can open the sheet as well.
  await page.keyboard.press('Control+k');
  await page.locator('#cmdp-input').fill('快捷键');
  await page.keyboard.press('Enter');
  await expect(sheet).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(sheet).toBeHidden();
  expect(await page.evaluate(() => window.ReadMDModal.depth())).toBe(0);
});

test('? typed into a text field does not open the cheat-sheet', async ({ page }) => {
  await boot(page);
  await openDoc(page);
  await page.locator('#btn-search').click();
  await page.locator('#search-input').focus();
  await page.keyboard.type('?');
  await expect(page.locator('#search-input')).toHaveValue('?');
  await expect(page.locator('#shortcuts-modal')).toHaveCount(0);
});

test('active tab indicator follows the selected tab', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === 'mobile', 'The narrow layout uses the secondary tab bar.');
  await boot(page);
  await openDoc(page, 'one.md');
  await openDoc(page, 'two-longer-name.md');
  const indicator = page.locator('#doc-tabs-bar .rm-tab-indicator');
  const matches = async () => page.evaluate(() => {
    const ind = document.querySelector('#doc-tabs-bar .rm-tab-indicator');
    const active = document.querySelector('#doc-tabs-bar .tab-item.active');
    ind.getAnimations().forEach(a => a.finish());
    return Math.abs(ind.getBoundingClientRect().width - active.getBoundingClientRect().width) < 1.5;
  });
  await expect(indicator).toHaveCount(1);
  await expect.poll(matches).toBe(true);
  await page.locator('#doc-tabs-bar .tab-item').first().click();
  await expect(page.locator('#doc-tabs-bar .tab-item').first()).toHaveAttribute('aria-selected', 'true');
  await expect.poll(async () => {
    await page.waitForTimeout(320);
    return matches();
  }).toBe(true);
});

test('welcome screen offers primary actions, drop hint and no overflow at 360px', async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 740 });
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  await boot(page);
  for (const id of ['w-open', 'w-folder', 'w-convert', 'w-web', 'w-ocr', 'w-ai']) await expect(page.locator('#' + id)).toBeVisible();
  await expect(page.locator('.welcome-search')).toBeVisible();
  const overflow = () => page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(await overflow()).toBeLessThanOrEqual(0);
  // Toolbar fits in at most two rows.
  expect((await page.locator('#toolbar').boundingBox()).height).toBeLessThanOrEqual(104);

  await openDoc(page, 'narrow.md', '# Narrow\n\n| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |\n');
  expect(await overflow()).toBeLessThanOrEqual(0);
  await page.keyboard.press('Control+k');
  await expect(page.locator('#command-palette-modal')).toBeVisible();
  expect(await overflow()).toBeLessThanOrEqual(0);
  const box = await page.locator('.rm-palette').boundingBox();
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(360);
  await page.keyboard.press('Escape');
  await page.evaluate(() => window.ReadMDShortcuts.open());
  await expect(page.locator('#shortcuts-modal')).toBeVisible();
  expect(await overflow()).toBeLessThanOrEqual(0);
  const sheetBox = await page.locator('.rm-keys').boundingBox();
  expect(sheetBox.x + sheetBox.width).toBeLessThanOrEqual(360);
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('theme cycling keeps working through the cross-fade wrapper', async ({ page }) => {
  await boot(page);
  const before = await page.evaluate(() => state.theme);
  await page.locator('#btn-theme').click();
  await expect.poll(() => page.evaluate(() => state.theme)).not.toBe(before);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.keyboard.press('Control+d');
  await expect.poll(() => page.evaluate(() => document.documentElement.classList.contains('rm-theme-vt'))).toBe(false);
});
