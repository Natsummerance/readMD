const { test, expect } = require('@playwright/test');

// 分页条窄宽自适应：AI 面板挤压 #main-col 时分页条不得溢出，
// 中间下拉随宽收缩，次要控件按容器查询分级隐藏。
test('pagination bar adapts when main column is squeezed', async ({ page }) => {
  const section = '## 第 1 章\n\n' + Array.from({ length: 700 }, (_, i) => `段落内容 ${i}，用于触发智能分页。`).join('\n\n');
  const longDoc = Array.from({ length: 12 }, (_, s) => section.replace('第 1 章', `第 ${s + 1} 章`)).join('\n\n');
  await page.route('**/api/file?p=**', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      ok: true, path: 'C:/long.md', dir: 'C:/', name: 'long.md',
      content: longDoc, original: longDoc, encoding: 'utf-8', fixes: [], stats: {},
    }),
  }));
  await page.setViewportSize({ width: 1280, height: 860 });
  await page.goto('/');
  await page.waitForFunction(() => typeof loadFile === 'function');
  await page.evaluate(() => loadFile('C:/long.md'));
  await page.waitForFunction(() => state.pagination.enabled && state.pagination.totalPages > 1);
  await expect(page.locator('#pagination-bar')).toBeVisible();

  const measure = () => page.evaluate(() => {
    const col = document.querySelector('#main-col');
    const bar = document.querySelector('#pagination-bar');
    const select = document.querySelector('#pg-page-select');
    return {
      colW: col.clientWidth,
      barW: bar.getBoundingClientRect().width,
      barRight: Math.round(bar.getBoundingClientRect().right),
      colRight: Math.round(col.getBoundingClientRect().right),
      selectW: Math.round(select.getBoundingClientRect().width),
      chapterShown: getComputedStyle(document.querySelector('.pg-chapter-label')).display !== 'none',
      totalShown: getComputedStyle(document.querySelector('#pg-total-label')).display !== 'none',
      firstShown: getComputedStyle(document.querySelector('#pg-first-btn')).display !== 'none',
      badgeShown: getComputedStyle(document.querySelector('.pg-mode-badge')).display !== 'none',
    };
  });

  // 1) 宽敞状态：完整布局
  const wide = await measure();
  expect(wide.chapterShown, '宽布局应显示章节标签').toBeTruthy();
  expect(wide.firstShown, '宽布局应显示首末页按钮').toBeTruthy();

  // 2) 中度挤压（模拟打开 AI 面板：main-col 失去剩余空间）
  await page.evaluate(() => {
    const main = document.querySelector('#main-col');
    main.style.width = '560px';
    main.style.flexGrow = '0';
  });
  await page.waitForTimeout(150);
  const mid = await measure();
  expect(mid.chapterShown, '620px 以下章节标签隐藏').toBeFalsy();
  expect(mid.totalShown, '620px 以下总页数标签隐藏').toBeFalsy();
  expect(mid.barRight, `分页条不得溢出 main-col (bar=${mid.barRight} col=${mid.colRight})`).toBeLessThanOrEqual(mid.colRight + 1);

  // 3) 极限挤压
  await page.evaluate(() => { document.querySelector('#main-col').style.width = '360px'; });  await page.waitForTimeout(150);
  const narrow = await measure();
  expect(narrow.firstShown, '极窄时首末页按钮隐藏').toBeFalsy();
  expect(narrow.badgeShown, '极窄时模式角标隐藏').toBeFalsy();
  expect(narrow.barW, '极窄时分页条宽度跟随列宽').toBeLessThanOrEqual(360);
  expect(narrow.barRight).toBeLessThanOrEqual(narrow.colRight + 1);
  expect(narrow.selectW, '下拉栏应收缩到 150px 上限内').toBeLessThanOrEqual(150);

  await page.screenshot({ path: 'test-results-pagination/pagination-adaptive.png', clip: { x: 0, y: 660, width: 640, height: 200 } });
});
