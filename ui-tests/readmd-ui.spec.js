const { test, expect } = require('@playwright/test');

async function enterEdit(page) {
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function');
  await page.evaluate(async () => {
    state.original = '# 标题\n\n正文 $x^2$'; state.fixed = state.original;
    state.mode = 'virtual'; await toggleEdit();
  });
  await expect(page.locator('#edit-bar')).toBeVisible();
}

test('single row commands, formula picker and responsive preview', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(String(e)));
  await enterEdit(page);
  expect((await page.locator('#edit-bar').boundingBox()).height).toBeLessThanOrEqual(50);
  await page.locator('#md-command-open').click();
  await expect(page.locator('#md-command-modal')).toBeVisible();
  await page.locator('#md-command-search').fill('表格');
  await expect(page.locator('#md-command-list')).toContainText('表格');
  await page.keyboard.press('Escape');
  await page.locator('#formula-open').click();
  await page.locator('#formula-search').fill('矩阵');
  await expect(page.locator('#formula-list')).toContainText('矩阵');
  await page.keyboard.press('Escape');
  await page.evaluate(() => setPvLayout('left'));
  expect(await page.locator('#main-col').evaluate(e => getComputedStyle(e).flexDirection)).toBe('row');
  await page.setViewportSize({ width: 580, height: 600 });
  expect(await page.locator('#main-col').evaluate(e => getComputedStyle(e).flexDirection)).toBe('column');
  await expect(page.locator('#pv-trigger')).toContainText('窄屏置底');
  expect(errors).toEqual([]);
});

test('image editor exposes professional lightweight controls', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(String(e)));
  await enterEdit(page);
  await page.setViewportSize({ width: 1100, height: 760 });
  await page.evaluate(() => {
    state.dir = 'C:/ReadMD-test'; openImgModal();
    loadImgSrc('data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="800" height="500"><rect width="800" height="500" fill="#3b6ef5"/></svg>'));
  });
  await page.waitForFunction(() => imgState.img && imgState.rawW === 800);
  await expect(page.locator('.crop-handle')).toHaveCount(8);
  await page.locator('#img-angle-number').fill('37.5'); await page.locator('#img-angle-number').press('Enter');
  await page.locator('#img-flip-x').click();
  await page.locator('#img-out-w').fill('640'); await page.locator('#img-out-w').press('Enter');
  expect(await page.evaluate(() => [imgState.angle, imgState.flipX, imgState.outW])).toEqual([37.5, true, 640]);
  expect(errors).toEqual([]);
});

test('export scroll, history and AI narrow layout remain usable', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(String(e)));
  await page.goto('/'); await page.waitForFunction(() => typeof openHistoryModal === 'function');
  await page.evaluate(() => {
    document.querySelector('#export-modal').classList.remove('hidden');
    document.querySelector('#export-opts').innerHTML = '<div style="height:1800px">all settings</div>';
  });
  const opts = page.locator('#export-opts');
  expect(await opts.evaluate(e => e.scrollHeight > e.clientHeight)).toBeTruthy();
  await expect(page.locator('.export-foot')).toBeVisible();
  await page.locator('#export-modal').evaluate(e => e.classList.add('hidden'));
  await page.locator('#btn-recent').click(); await expect(page.locator('#history-modal')).toBeVisible();
  await page.evaluate(() => document.querySelector('#ai-panel').classList.remove('hidden'));
  expect((await page.locator('#ai-panel').boundingBox()).width).toBeLessThanOrEqual(685);
  await expect(page.locator('#ai-model')).toBeVisible();
  expect(errors).toEqual([]);
});
