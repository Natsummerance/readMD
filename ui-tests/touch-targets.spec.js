const { test, expect } = require('@playwright/test');

test('mobile reader controls meet touch target budgets', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'mobile', 'Target-size rules are checked in the touch project.');
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'zh-CN'));
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');

  await page.evaluate(async () => {
    await renderVirtual('clipboard', 'touch.md', '', '# Touch target\n\nReadable body.', []);
    state.tabs = [{
      id: 'touch',
      mode: 'virtual',
      title: 'touch.md',
      name: 'touch.md',
      content: '# Touch target\n\nReadable body.',
    }];
    state.activeTabId = 'touch';
    renderTabsBar();
  });

  const sizes = await page.evaluate(() => {
    const selectors = [
      '#toolbar .tb-btn',
      '#doc-tabs-secondary-bar .tab-item',
      '#search-input',
      '#search-bar button',
      '.pagination-bar button',
      '.pagination-bar select',
    ];
    return selectors.flatMap(selector => [...document.querySelectorAll(selector)])
      .filter(target => target.offsetParent !== null)
      .map(target => ({
        label: `${target.id || target.className}`,
        minimum: Math.min(target.getBoundingClientRect().width, target.getBoundingClientRect().height),
      }));
  });

  expect(sizes.length).toBeGreaterThan(8);
  const undersized = sizes.filter(size => size.minimum < 43.5);
  expect(undersized).toEqual([]);

  await page.locator('#btn-more').click();
  const moreItems = await page.evaluate(() => [...document.querySelectorAll('.more-item')]
    .filter(target => target.offsetParent !== null)
    .map(target => Math.min(
      target.getBoundingClientRect().width,
      target.getBoundingClientRect().height,
    )));
  expect(moreItems.length).toBeGreaterThan(4);
  const undersizedMore = moreItems.filter(minimum => minimum < 43.5);
  expect(undersizedMore).toEqual([]);
});
