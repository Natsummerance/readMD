const { test, expect } = require('@playwright/test');
const path = require('path');
const { pathToFileURL } = require('url');

async function enterEdit(page) {
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function');
  await page.evaluate(async () => {
    state.original = '# 标题\n\n正文 $x^2$'; state.fixed = state.original;
    state.mode = 'virtual'; await toggleEdit();
  });
  await expect(page.locator('#edit-bar')).toBeVisible();
}

test('installer keeps location-page actions inside compact viewports', async ({ page }) => {
  const installerUrl = pathToFileURL(path.resolve(__dirname, '../installer/setup.html')).href + '?demo=1';
  await page.emulateMedia({ reducedMotion: 'reduce' });
  for (const viewport of [{ width: 880, height: 640 }, { width: 784, height: 560 }]) {
    await page.setViewportSize(viewport);
    await page.goto(installerUrl);
    await page.locator('#btn-install').click();
    await expect(page.locator('#page-options')).toHaveClass(/active/);
    await page.locator('#page-options').evaluate(options => {
      options.getAnimations({ subtree: true }).forEach(animation => animation.finish());
    });
    const actions = await page.locator('#page-options .actions').boundingBox();
    expect(actions).not.toBeNull();
    expect(actions.y).toBeGreaterThanOrEqual(52);
    expect(actions.y + actions.height).toBeLessThanOrEqual(viewport.height - 8);
    await page.locator('#size-note').scrollIntoViewIfNeeded();
    const actionsAfterScroll = await page.locator('#page-options .actions').boundingBox();
    expect(actionsAfterScroll.y + actionsAfterScroll.height).toBeLessThanOrEqual(viewport.height - 8);
    await expect(page.locator('#btn-start-install')).toBeInViewport();
  }
});

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

test('web to Markdown renders dynamic pages with progress and actionable errors', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(String(e)));
  await page.addInitScript(() => {
    window.pywebview = { api: {
      render_web_page: async url => ({
        ok: true, final_url: url, html: '<html><body><article>rendered</article></body></html>',
        readability: { title: '动态文章', content: '<article><h1>动态文章</h1><p>动态正文内容</p></article>' },
      }),
      cancel_web_render: async () => true,
      get_settings: async () => ({}), get_recent: async () => [],
      start_modules: async () => true, get_modules_status: async () => ({ modules: { web: 'ready' }, errors: {} }),
    } };
  });
  let extractCalls = 0;
  await page.route('**/api/modules', route => route.fulfill({
    status: 200, contentType: 'application/json',
    body: JSON.stringify({ modules: { web: 'ready', convert: 'ready', ocr: 'ready', ai: 'ready' }, errors: {} }),
  }));
  await page.route('**/api/web/extract', async route => {
    extractCalls++;
    const body = route.request().postDataJSON();
    if (!body.html) {
      await new Promise(resolve => setTimeout(resolve, 80));
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({
        ok: false, render_required: true, code: 'render_required', error: '需要渲染',
      }) });
    }
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({
      ok: true, content: '# 动态文章\n\n动态正文内容', engine: 'mozilla-readability',
      meta: { title: '动态文章' }, warnings: [], links: [], assets: [],
    }) });
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof openWebDialog === 'function');
  await page.evaluate(() => openWebDialog());
  await expect(page.locator('#url-modal')).toBeVisible();
  await expect(page.locator('#url-mode')).toHaveValue('smart');
  await expect(page.locator('#url-images')).not.toBeChecked();
  await page.locator('#url-input').fill('example.com/article');
  await page.locator('#url-go').click();
  await expect(page.locator('#url-progress')).toBeVisible();
  await page.waitForFunction(() => state.source === 'url');
  await expect(page.locator('#url-modal')).toBeVisible();
  expect(await page.locator('#url-input').inputValue()).toBe('https://example.com/article');
  expect(extractCalls).toBe(2);
  await expect(page.locator('#content')).toContainText('动态正文内容');

  await page.unroute('**/api/web/extract');
  await page.route('**/api/web/extract', route => route.fulfill({
    status: 403, contentType: 'application/json',
    body: JSON.stringify({ ok: false, code: 'forbidden', error: '服务器拒绝访问该网页（403）' }),
  }));
  await page.evaluate(() => openWebDialog());
  await page.locator('#url-input').fill('https://blocked.example/article');
  await page.locator('#url-go').click();
  await expect(page.locator('#url-status')).toContainText('服务器拒绝访问');
  await expect(page.locator('#url-status')).toHaveClass(/error/);
  expect(errors).toEqual([]);
});
