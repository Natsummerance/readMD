const { test, expect } = require('@playwright/test');
const fs = require('fs/promises');
const os = require('os');
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

async function setEditorContent(page, content) {
  await page.waitForFunction(() => {
    const cmReady = document.querySelector('#edit-cm .cm-content');
    const fallbackReady = document.getElementById('edit-area') && !document.getElementById('edit-area').classList.contains('hidden');
    return !!(cmReady || fallbackReady);
  });
  const cmContent = page.locator('#edit-cm .cm-content');
  if (await cmContent.count()) {
    await cmContent.click();
    await page.keyboard.press('Control+A');
    await page.keyboard.type(content);
  } else {
    await page.locator('#edit-area').fill(content);
  }
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    try {
      localStorage.setItem('readmd_language', 'zh-CN');
    } catch (e) {}
  });
});

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
  await page.locator('#formula-open').click();
  await expect(page.locator('#formula-modal')).toBeVisible();
  await page.locator('#formula-search').fill('矩阵');
  await expect(page.locator('#formula-list')).toContainText('矩阵');
  await page.keyboard.press('Escape');
  await page.evaluate(() => setPvLayout('left'));
  expect(await page.locator('#main-col').evaluate(e => getComputedStyle(e).flexDirection)).toBe('row');
  await page.setViewportSize({ width: 580, height: 600 });
  expect(await page.locator('#main-col').evaluate(e => getComputedStyle(e).flexDirection)).toBe('column');
  await expect(page.locator('#pv-trigger')).toContainText(/窄屏已自动切换|下方预览|窄屏置底/);
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
  await expect(page.locator('#ai-model-summary')).toBeVisible();
  expect(errors).toEqual([]);
});

test('AI keeps configured keys usable, autosaves, and supports incognito history', async ({ page }) => {
  const saves = [];
  await page.route('**/api/ai/config', route => route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({
    current: { provider_id: 'openai', model: 'gpt-test' }, custom: [], presets: [{ id: 'openai', name: 'OpenAI', has_key: true, key_source: 'local', models: ['gpt-test'], base_url: 'https://api.example/v1' }],
  }) }));
  await page.route('**/api/ai/prompts', route => route.fulfill({ status: 200, contentType: 'application/json', body: '{"templates":[]}' }));
  await page.route('**/api/ai/history', async route => {
    if (route.request().method() === 'POST') { saves.push(route.request().postDataJSON()); return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, session: { id: 'saved-1' } }) }); }
    return route.fulfill({ status: 200, contentType: 'application/json', body: '{"sessions":[]}' });
  });
  await page.route('**/api/ai/chat', route => route.fulfill({ status: 200, contentType: 'text/event-stream', body: 'data: {"d":"已完成"}\n\ndata: {"done":true}\n\n' }));
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleAiPanel === 'function');
  await page.evaluate(() => { state.original = '# 文档\n\n内容'; toggleAiPanel(); });
  await expect(page.locator('#ai-panel')).toBeVisible();
  await expect(page.locator('#ai-connection-label')).toContainText(/连接就绪|已就绪|Ready/);
  await page.locator('#ai-prompt').fill('总结');
  await page.locator('#ai-prompt').press('Enter');
  await expect(page.locator('#ai-output')).toContainText('已完成');
  expect(saves.length).toBe(1);
  await expect(page.locator('.ai-msg-copy')).toBeVisible();
  await page.locator('#ai-incognito').check();
  await page.locator('#ai-prompt').fill('再说一次');
  await page.locator('#ai-prompt').press('Enter');
  await expect(page.locator('#ai-output')).toContainText('已完成');
  expect(saves.length).toBe(1);
  await page.locator('#ai-incognito').uncheck();
  await page.locator('#ai-prompt').fill('第三次会保存');
  await page.locator('#ai-prompt').press('Enter');
  await expect.poll(() => saves.length).toBe(2);
  const savedText = saves[1].session.messages.map(message => message.content).join('\n');
  expect(savedText).not.toContain('再说一次');
  expect(savedText).toContain('第三次会保存');
});

test('module entry actively starts an idle module once before polling', async ({ page }) => {
  let starts = 0, checks = 0;
  const states = [];
  await page.goto('/');
  await page.route('**/api/modules/load', route => {
    starts++; expect(route.request().method()).toBe('POST');
    expect(route.request().postDataJSON()).toEqual({ name: 'convert' });
    return route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true}' });
  });
  await page.route('**/api/modules', route => {
    const value = ['idle', 'loading', 'ready'][Math.min(checks++, 2)]; states.push(value);
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ modules: { convert: value } }) });
  });
  await page.waitForFunction(() => typeof ensureModule === 'function');
  expect(await page.evaluate(() => Promise.all([ensureModule('convert', 4000), ensureModule('convert', 4000)]))).toEqual([true, true]);
  expect(starts).toBe(1);
  expect(states).toEqual(expect.arrayContaining(['idle', 'loading', 'ready']));
});

test('welcome keeps toolbar disabled and six modules; clipboard lives in more menu', async ({ page }) => {
  await page.addInitScript(() => {
    window.pywebview = { api: {
      get_settings: async () => ({}), get_recent: async () => [], start_modules: async () => true,
      get_modules_status: async () => ({ modules: {}, errors: {} }),
    } };
  });
  await page.goto('/');
  await expect(page.locator('#btn-print')).toBeDisabled();
  await expect(page.locator('#btn-a')).toBeDisabled();
  await expect(page.locator('#btn-A')).toBeDisabled();
  for (const id of ['w-open', 'w-folder', 'w-ai', 'w-convert', 'w-web', 'w-ocr']) {
    await expect(page.locator('#' + id)).toBeVisible();
  }
  await expect(page.locator('#btn-clipboard-new')).toBeHidden();
  await page.locator('#btn-more').click();
  await expect(page.locator('#btn-clipboard-new')).toBeVisible();
});

test('opened file re-enables export/font tools and shows home button', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(String(e)));
  await page.addInitScript(() => {
    window.pywebview = { api: {
      get_settings: async () => ({}), get_recent: async () => [], start_modules: async () => true,
      get_modules_status: async () => ({ modules: {}, errors: {} }),
      choose_file: async () => 'C:/docs/readme.md',
    } };
  });
  await page.route('**/api/file*', route => route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, path: 'C:/docs/readme.md', dir: 'C:/docs', name: 'readme.md', content: '# 标题\n\n正文', original: '# 标题\n\n正文', encoding: 'utf-8', size: 99, fixes: [], stats: {} }) }));
  await page.goto('/');
  await page.locator('#w-open').click();
  await expect(page.locator('#btn-print')).toBeEnabled();
  await expect(page.locator('#btn-a')).toBeEnabled();
  await expect(page.locator('#btn-A')).toBeEnabled();
  await expect(page.locator('#btn-home')).toBeVisible();
  await page.locator('#btn-home').click();
  await expect(page.locator('#w-open')).toBeVisible();
  expect(errors).toEqual([]);
});

test('toolbar filename supports accessible inline rename', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(String(e)));
  await page.addInitScript(() => {
    window.renameCalls = [];
    window.pywebview = { api: {
      rename_file: async (path, stem) => {
        window.renameCalls.push({ path, stem });
        return { ok: true, old_path: path, path: 'C:\\docs\\new name.md',
          name: 'new name.md', warnings: [] };
      },
      get_settings: async () => ({}), get_recent: async () => [],
      start_modules: async () => true,
      get_modules_status: async () => ({ modules: {}, errors: {} }),
    } };
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof openFileRename === 'function');
  await page.evaluate(() => {
    state.mode = 'file'; state.source = 'file'; state.file = 'C:\\docs\\old.md';
    state.dir = 'C:\\docs'; state.editing = false;
    setFileTitle('old.md', true, state.file);
  });

  await page.locator('#file-title').click();
  const input = page.locator('#file-rename-input');
  await expect(input).toBeVisible();
  await expect(input).toHaveValue('old');
  await expect(page.locator('#file-rename-ext')).toHaveText('.md');
  await input.fill('new name');
  await input.press('Enter');
  await expect(page.locator('#file-title')).toHaveText('new name.md');
  expect(await page.evaluate(() => state.file)).toBe('C:\\docs\\new name.md');
  expect(await page.evaluate(() => window.renameCalls)).toEqual([
    { path: 'C:\\docs\\old.md', stem: 'new name' },
  ]);

  await page.keyboard.press('F2');
  await expect(page.locator('#file-rename-input')).toBeVisible();
  await page.locator('#file-rename-input').press('Escape');
  await expect(page.locator('#file-rename-input')).toHaveCount(0);
  await expect(page.locator('#file-title')).toHaveText('new name.md');

  await page.evaluate(() => { state.editing = true; });
  await page.keyboard.press('F2');
  await expect(page.locator('#file-rename-input')).toHaveCount(0);
  await expect(page.locator('#toast')).toContainText('请先保存');
  expect(errors).toEqual([]);
});

test('web to Markdown renders dynamic pages with progress and actionable errors', async ({ page }) => {
  const errors = []; page.on('pageerror', e => errors.push(String(e)));
  await page.addInitScript(() => {
    window.pywebview = { api: {
      render_web_page: async url => ({
        ok: true, final_url: url, html: '<html><body><article>rendered</article></body></html>',
        defuddle: { title: '动态文章', contentMarkdown: '动态正文内容' },
        readability: { title: '动态文章', content: '<article><h1>动态文章</h1><p>动态正文内容</p></article>' },
      }),
      cancel_web_render: async () => true,
      get_settings: async () => ({}), get_recent: async () => [],
      start_modules: async () => true, get_modules_status: async () => ({ modules: { web: 'ready' }, errors: {} }),
    } };
  });
  let extractCalls = 0, renderedPayload = null;
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
    renderedPayload = body;
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({
      ok: true, content: '# 动态文章\n\n动态正文内容', engine: 'mozilla-readability',
      meta: { title: '动态文章' }, warnings: [], links: [], assets: [],
    }) });
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof openWebDialog === 'function');
  await page.evaluate(() => openWebDialog());
  await expect(page.locator('#url-modal')).toBeVisible();
  await expect(page.locator('#url-images')).not.toBeChecked();
  await expect(page.locator('#url-pages')).toHaveValue('1');
  await expect(page.locator('#url-pages')).toHaveAttribute('max', '30');
  await expect(page.locator('#url-private')).toBeChecked();

  await page.locator('#url-input').fill('example.com/article');
  await page.locator('#url-go').click();
  await expect(page.locator('#url-progress')).toBeVisible();
  await page.waitForFunction(() => state.source === 'url');
  await expect(page.locator('#url-modal')).toBeVisible();
  expect(await page.locator('#url-input').inputValue()).toBe('https://example.com/article');
  expect(extractCalls).toBe(2);
  expect(renderedPayload.defuddle.contentMarkdown).toContain('动态正文');
  expect(renderedPayload.diagnostics.fallback_reason).toBe('render_required');
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

test('offline Defuddle bundle extracts Markdown from a rendered DOM', async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => {
    document.title = 'Defuddle browser fixture';
    document.body.innerHTML = '<main><article><h1>Fixture</h1><p>' +
      'Rendered article content for the offline extraction regression test. '.repeat(12) +
      '</p><pre><code>const ready = true;</code></pre></article></main>';
  });
  await page.addScriptTag({ url: '/assets/vendor/defuddle.bundle.js' });
  const result = await page.evaluate(() => window.ReadMDDefuddle.parse(document.cloneNode(true), location.href));
  expect(result.title).toContain('Defuddle browser fixture');
  expect(result.contentMarkdown).toContain('Rendered article content');
  expect(result.contentMarkdown).toContain('const ready = true');
});

test('cancelling a batch keeps pages already extracted', async ({ page }) => {
  const seenUrls = [];
  await page.addInitScript(() => {
    window.pywebview = { api: {
      get_settings: async () => ({}), get_recent: async () => [], start_modules: async () => true,
      get_modules_status: async () => ({ modules: { web: 'ready' }, errors: {} }),
      cancel_web_render: async () => true, revoke_private_web: async () => true,
    } };
  });
  await page.route('**/api/modules', route => route.fulfill({
    status: 200, contentType: 'application/json',
    body: JSON.stringify({ modules: { web: 'ready' }, errors: {} }),
  }));
  await page.route('**/api/web/cancel', route => route.fulfill({
    status: 200, contentType: 'application/json', body: '{"ok":true}',
  }));
  await page.route('**/api/web/extract', async route => {
    const body = route.request().postDataJSON();
    seenUrls.push(body.url);
    if (body.url.includes('/two')) await new Promise(resolve => setTimeout(resolve, 2000));
    const first = body.url.includes('/one');
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({
      ok: true, content: '# ' + (first ? 'One' : 'Two') + '\n\nExtracted body',
      meta: { title: first ? 'One' : 'Two' }, warnings: [], assets: [],
      links: first ? ['https://example.com/two', 'https://example.com/three'] : [],
    }) });
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof openWebDialog === 'function');
  await page.evaluate(() => openWebDialog());
  await page.locator('#url-input').fill('https://example.com/one');
  await page.locator('#url-pages').fill('3');
  await page.locator('#url-go').click();

  await expect.poll(() => seenUrls.length).toBe(2);
  await page.locator('#url-cancel').click();
  await page.waitForFunction(() => !webRun.running);
  await expect(page.locator('#content')).toContainText('One');
  await expect(page.locator('#content')).toContainText('抓取统计');
  await expect(page.locator('#content')).toContainText('跳过');
  await expect(page.locator('#url-status')).toContainText(/已保留成功抓取|已取消|正在取消/);
});

test('tab reordering reorders tabs and is isolated from global drag overlay', async ({ page }) => {
  await page.addInitScript(() => {
    window.pywebview = { api: {
      get_settings: async () => ({}), get_recent: async () => [], start_modules: async () => true,
      get_modules_status: async () => ({ modules: {}, errors: {} }),
    } };
  });
  await page.goto('/');
  await page.evaluate(() => {
    state.tabs = [
      { id: 'tab1', title: 'Doc One', content: '# One', isDirty: false },
      { id: 'tab2', title: 'Doc Two', content: '# Two', isDirty: false },
      { id: 'tab3', title: 'Doc Three', content: '# Three', isDirty: false },
    ];
    state.activeTabId = 'tab1';
    renderTabsBar();
  });
  const tabItems = page.locator('.tab-item');
  await expect(tabItems).toHaveCount(3);
  await expect(tabItems.nth(0)).toContainText('Doc One');
  await expect(tabItems.nth(1)).toContainText('Doc Two');

  // Test reordering function
  await page.evaluate(() => reorderTabs('tab3', 'tab1', false));
  await expect(page.locator('.tab-item').nth(0)).toContainText('Doc Three');
  await expect(page.locator('.tab-item').nth(1)).toContainText('Doc One');

  // Ensure drag overlay stays hidden when dragging tab
  await page.evaluate(() => {
    state.isDraggingTab = true;
    window.dispatchEvent(new Event('dragenter'));
  });
  await expect(page.locator('#drag-overlay')).toBeHidden();
  await page.evaluate(() => { state.isDraggingTab = false; });
});

test('smart in-document TOC auto-matches headings and resolves anchor jumps', async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => {
    renderContent(`
# 目录
- [1. 介绍](#invalid-slug-1)
- [快速开始](#quick-start)

# 1. 介绍
这是介绍内容。

## 快速开始 (Quick Start)
这是快速开始内容。
`, 'TOC Test');
  });

  const heading1 = page.locator('#content h1:has-text("1. 介绍")');
  await expect(heading1).toBeVisible();

  // Click on the in-document TOC link with mismatched slug
  const tocLink = page.locator('#content a:has-text("1. 介绍")');
  await expect(tocLink).toBeVisible();
  await tocLink.click();

  // Heading gets the target highlight animation class
  await expect(heading1).toHaveClass(/heading-target-highlight/);
});

test('editor undo, redo buttons and floating selection toolbar are functional', async ({ page }) => {
  await page.goto('/');
  await page.evaluate(async () => {
    state.file = 'test.md';
    state.original = '# Initial Content';
    state.activeTabId = 'tab_test';
    state.tabs = [{ id: 'tab_test', title: 'test.md', content: '# Initial Content', isDirty: false }];
    await toggleEdit();
  });

  await expect(page.locator('#edit-bar')).toBeVisible();
  await expect(page.locator('#edit-undo')).toBeVisible();
  await expect(page.locator('#edit-redo')).toBeVisible();

  // Test floating selection toolbar visibility toggle
  await page.evaluate(() => {
    const toolbar = document.getElementById('cm-selection-toolbar');
    toolbar.classList.remove('hidden');
  });
  await expect(page.locator('#cm-selection-toolbar')).toBeVisible();
  await expect(page.locator('#cm-sel-copy')).toBeVisible();
  await expect(page.locator('#cm-sel-cut')).toBeVisible();
  await expect(page.locator('#cm-sel-paste')).toBeVisible();

  await page.evaluate(() => hideCmSelectionToolbar());
  await expect(page.locator('#cm-selection-toolbar')).toBeHidden();
});

test('tab inline rename fixes extension and expands space by folding sibling tabs', async ({ page }) => {
  await page.goto('/');
  await page.evaluate(() => {
    state.tabs = [
      { id: 'tab1', title: 'Report2026_Q3_LongFileName.md', name: 'Report2026_Q3_LongFileName.md', content: '# Q3', isDirty: false },
      { id: 'tab2', title: 'DocTwo.md', name: 'DocTwo.md', content: '# Two', isDirty: false },
      { id: 'tab3', title: 'DocThree.md', name: 'DocThree.md', content: '# Three', isDirty: false },
    ];
    state.activeTabId = 'tab1';
    renderTabsBar();
  });

  const tab1 = page.locator('.tab-item').first();
  await tab1.dblclick();

  // Renaming active class and wrap
  await expect(tab1).toHaveClass(/tab-renaming-active/);
  await expect(page.locator('#doc-tabs-bar')).toHaveClass(/tab-renaming-mode/);

  // Input contains stem only, ext is in separate fixed span
  const input = tab1.locator('.tab-title-input');
  await expect(input).toHaveValue('Report2026_Q3_LongFileName');
  const extSpan = tab1.locator('.tab-rename-ext');
  await expect(extSpan).toHaveText('.md');

  // Cancel with Escape restores layout
  await input.press('Escape');
  await expect(tab1).not.toHaveClass(/tab-renaming-active/);
  await expect(page.locator('#doc-tabs-bar')).not.toHaveClass(/tab-renaming-mode/);
});

test('v2.3.0 i18n language modal and switching', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => window.i18n && typeof window.i18n.openModal === 'function');

  // Open language modal
  await page.evaluate(() => i18n.openModal());
  await expect(page.locator('#lang-modal')).toBeVisible();

  // Search filter
  await page.locator('#lang-search-input').fill('English');
  await expect(page.locator('#lang-grid')).toContainText('English');
  await page.locator('#lang-search-input').press('ArrowDown');
  await expect(page.locator('#lang-grid [role="option"]').first()).toBeFocused();

  // Switch language
  await page.evaluate(() => i18n.setLanguage('en'));
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(page.locator('#btn-more')).toHaveAttribute('title', /More Options|More Features/);

  // Close modal
  await page.evaluate(() => i18n.closeModal());
  await expect(page.locator('#lang-modal')).toBeHidden();
});

test('v2.3.1 export modal renders all settings, style presets and live preview', async ({ page }) => {
  await page.addInitScript(() => {
    window.pywebview = { api: {
      get_settings: async () => ({}), get_recent: async () => [], start_modules: async () => true,
      get_modules_status: async () => ({ modules: {}, errors: {} }),
      get_export_presets: async () => ({
        defaults: {
          page: { size: 'A4', orientation: 'portrait', marginTop: 20, marginRight: 18, marginBottom: 20, marginLeft: 18 },
          typography: { font: 'MicrosoftYaHei', size: 11, lineHeight: 1.6, spacing: 6, color: '#262626', align: 'left' },
          headings: {
            h1: { size: 20, color: '#1a1a1a', bold: true, align: 'left', before: 18, after: 10 },
            h2: { size: 16, color: '#1f2937', bold: true, align: 'left', before: 14, after: 8 },
            h3: { size: 14, color: '#2d3748', bold: true, align: 'left', before: 12, after: 6 },
            h4: { size: 12, color: '#374151', bold: true, align: 'left', before: 10, after: 6 },
            h5: { size: 11, color: '#4a5568', bold: true, align: 'left', before: 8, after: 4 },
            h6: { size: 10.5, color: '#4a5568', bold: true, align: 'left', before: 8, after: 4 },
          },
          table: { headerBg: '#3b6ef5', headerColor: '#ffffff', headerBold: true, borderColor: '#c8cdd4', borderWidth: 0.75, banded: true, bandColor: '#f3f5f9', cellSize: 10, cellPadding: 6, align: 'left', widthPct: 100 },
          code: { bg: '#f5f6f8', color: '#2f3b4a', font: 'Consolas', size: 9.5, borderColor: '#dfe3e8', borderWidth: 0.5, rounded: true },
          quote: { barColor: '#3b6ef5', bg: '#f3f6ff', color: '#4a5568' },
          link: { color: '#2b6cb0' },
          hr: { color: '#d8dce2' },
          footer: { pageNumbers: true, text: '' },
          cover: { enabled: false, title: '', subtitle: '', date: '', align: 'center' },
          toc: { enabled: false },
          math: { dpi: 200 },
          htmlTheme: 'light',
        },
        presets: {
          minimal: { typography: { font: 'MicrosoftYaHei', size: 10.5 } },
          classic: { typography: { font: 'SimSun', size: 11 } },
          business: { typography: { font: 'DengXian', size: 11 } },
        },
        custom: {},
        last: null,
      }),
      export_doc: async () => ({ ok: true, path: 'C:/docs/export.pdf', warns: [] }),
      open_path: async () => true,
      reveal_path: async () => true,
      save_export_presets: async () => true,
    } };
  });

  await enterEdit(page);
  await page.evaluate(() => {
    openExportModal();
  });

  // 1. 验证导出对话框展开
  await expect(page.locator('#export-modal')).toBeVisible();

  // 2. 验证设置选项区域已正确渲染多个设置分组
  const secCount = await page.locator('#export-opts .exp-sec').count();
  expect(secCount).toBeGreaterThanOrEqual(5);

  // 3. 验证预设选择器已正确填充
  await expect(page.locator('#exp-preset')).toBeVisible();
  const presetOptions = await page.locator('#exp-preset option').allTextContents();
  expect(presetOptions.length).toBeGreaterThanOrEqual(4);

  // 4. 验证微缩排版预览已生成并包含正文
  await expect(page.locator('#export-preview-card')).toBeVisible();
  await expect(page.locator('#export-preview-mini-content')).not.toBeEmpty();
  await expect(page.locator('#export-preview-mini-content')).toContainText('标题');

  // 5. 切换预设样式并验证
  await page.locator('#exp-preset').selectOption('classic');
  await expect(page.locator('#export-preview-paper-meta')).toContainText(/经典|classic/i);

  // 6. 点击打开全屏排版预览模态框
  await page.locator('#export-preview-card').click();
  await expect(page.locator('#export-preview-modal')).toBeVisible();
  await expect(page.locator('#export-preview-full-page')).toContainText('标题');

  // 7. 关闭全屏排版预览模态框
  await page.locator('#export-preview-close').click();
  await expect(page.locator('#export-preview-modal')).toBeHidden();

  // 8. 切换到 HTML 格式标签
  await page.locator('.exp-fmt[data-fmt="html"]').click();
  await expect(page.locator('#export-preview-badge')).toContainText('HTML');

  // 9. 关闭导出对话框
  await page.locator('#export-close').click();
  await expect(page.locator('#export-modal')).toBeHidden();
});

test('v2.3.0 Zen Mode and Table Designer', async ({ page }) => {
  await enterEdit(page);

  // Toggle Zen Mode
  await page.evaluate(() => toggleZenMode(true));
  await expect(page.locator('body')).toHaveClass(/zen-mode/);
  await expect(page.locator('#zen-hover-trigger')).toBeVisible();
  await expect(page.locator('#zen-exit-btn')).toBeHidden();

  await page.evaluate(() => toggleZenMode(false));
  await expect(page.locator('body')).not.toHaveClass(/zen-mode/);

  // Open Table Modal
  await page.evaluate(() => openTableModal());
  await expect(page.locator('#table-modal')).toBeVisible();
  await expect(page.locator('.table-grid-cell')).toHaveCount(100);

  await page.evaluate(() => closeTableModal());
  await expect(page.locator('#table-modal')).toBeHidden();
});

test('v2.3.2 dirty tab close confirmation modal UI, styling, and actions', async ({ page }) => {
  await page.addInitScript(() => {
    window.pywebview = { api: {
      get_settings: async () => ({}), get_recent: async () => [], start_modules: async () => true,
      get_modules_status: async () => ({ modules: {}, errors: {} }),
    } };
  });
  await page.goto('/');
  
  // 1. 创建带有未保存修改的标签页
  await page.evaluate(() => {
    state.tabs = [
      { id: 'tab_dirty_1', title: '未保存测试文档.md', name: '未保存测试文档.md', content: '# 测试内容', isDirty: true }
    ];
    state.activeTabId = 'tab_dirty_1';
    renderTabsBar();
  });

  // 2. 点击标签页关闭按钮，触发未保存确认弹窗
  await page.locator('.tab-close').first().click();

  // 3. 验证未保存弹窗与各控件正确渲染
  const modal = page.locator('#close-confirm-modal');
  await expect(modal).toBeVisible();
  await expect(page.locator('#close-confirm-title')).toContainText(/保存|修改|Changes/i);
  await expect(page.locator('#close-confirm-desc')).toContainText('未保存测试文档');
  await expect(page.locator('#close-confirm-save')).toBeVisible();
  await expect(page.locator('#close-confirm-discard')).toBeVisible();
  await expect(page.locator('#close-confirm-cancel')).toBeVisible();

  // 4. 点击取消按钮，弹窗关闭且标签页保持开启
  await page.locator('#close-confirm-cancel').click();
  await expect(modal).toBeHidden();
  expect(await page.locator('.tab-item').count()).toBe(1);

  // 5. 再次触发关闭，点击“不保存”，确认标签页顺利关闭并回到首页
  await page.locator('.tab-close').first().click();
  await expect(modal).toBeVisible();
  await page.locator('#close-confirm-discard').click();
  await expect(modal).toBeHidden();
  expect(await page.locator('.tab-item').count()).toBe(0);
  await expect(page.locator('#welcome')).toBeVisible();
});

test('v2.3.7 top bar zen mode toggle & keyboard escape shortcut', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleZenMode === 'function');

  const zenBtn = page.locator('#btn-zen');
  await expect(zenBtn).toBeVisible();

  // 1. 点击顶栏禅模式按钮激活
  await zenBtn.click();
  expect(await page.evaluate(() => document.body.classList.contains('zen-mode'))).toBe(true);

  // 2. 验证右侧悬浮退出按钮已被彻底移除/隐藏，顶部悬停感应区存在
  await expect(page.locator('#zen-exit-btn')).toBeHidden();
  await expect(page.locator('#zen-hover-trigger')).toBeVisible();

  // 3. 鼠标移至顶部 (clientY <= 10)，验证顶栏滑出 (添加 zen-toolbar-revealed 类)
  await page.mouse.move(200, 5);
  await page.waitForTimeout(100);
  expect(await page.evaluate(() => document.getElementById('toolbar').classList.contains('zen-toolbar-revealed'))).toBe(true);

  // 4. 鼠标向下移开 (clientY > 54)，验证顶栏滑回隐藏
  await page.mouse.move(200, 150);
  await page.waitForTimeout(100);
  expect(await page.evaluate(() => document.getElementById('toolbar').classList.contains('zen-toolbar-revealed'))).toBe(false);

  // 5. 按 Escape 键退出禅模式
  await page.keyboard.press('Escape');
  expect(await page.evaluate(() => document.body.classList.contains('zen-mode'))).toBe(false);
});

test('v2.3.7 more menu accordion groups and document-dependent disabled state', async ({ page }) => {
  await page.goto('/');
  await page.waitForSelector('#btn-more');

  // 1. 首页欢迎页状态下打开更多菜单
  await page.locator('#btn-more').click();
  await expect(page.locator('#more-menu')).toBeVisible();

  // 2. 验证三个极简分组存在且可折叠/展开
  const groups = page.locator('.more-group');
  expect(await groups.count()).toBe(3);

  const interactHeader = page.locator('.more-group-header').nth(1);
  await interactHeader.click();
  // 再次点击可切换展开折叠
  await interactHeader.click();

  // 3. 验证未加载文档时，文档依赖功能被 disabled
  await expect(page.locator('#btn-presentation-menu')).toHaveAttribute('disabled', '');
  await expect(page.locator('#btn-run-all-chunks')).toHaveAttribute('disabled', '');
  await expect(page.locator('#btn-saveas')).toHaveAttribute('disabled', '');
  await expect(page.locator('#btn-fix')).toHaveAttribute('disabled', '');
  await expect(page.locator('#btn-share')).toHaveAttribute('disabled', '');

  // 4. 打开/加载虚拟文档后，依赖功能激活
  await page.evaluate(() => {
    state.tabs = [{ id: 'tab_v1', title: 'test.md', name: 'test.md', content: '# Hello World\n```python\nprint(1)\n```', isDirty: false }];
    state.activeTabId = 'tab_v1';
    state.original = '# Hello World\n```python\nprint(1)\n```';
    state.mode = 'virtual';
    updateStatus();
  });

  await expect(page.locator('#btn-presentation-menu')).not.toHaveAttribute('disabled', '');
  await expect(page.locator('#btn-run-all-chunks')).not.toHaveAttribute('disabled', '');
  await expect(page.locator('#btn-saveas')).not.toHaveAttribute('disabled', '');
  await expect(page.locator('#btn-fix')).not.toHaveAttribute('disabled', '');
  await expect(page.locator('#btn-share')).not.toHaveAttribute('disabled', '');
});

test('v2.3.7 custom styles and html head injection modal', async ({ page }) => {
  await page.goto('/');
  await page.waitForSelector('#btn-more');

  await page.locator('#btn-more').click();
  await page.locator('#btn-style-custom').click();

  const modal = page.locator('#style-custom-modal');
  await expect(modal).toBeVisible();

  // 验证预设模板按钮可点击并插入代码
  const indentBtn = page.locator('#btn-preset-indent');
  await expect(indentBtn).toBeVisible();
  await indentBtn.click();

  const cssInput = page.locator('#style-custom-css');
  const cssVal = await cssInput.inputValue();
  expect(cssVal).toContain('text-indent');

  // 验证输入框可编辑
  const headInput = page.locator('#style-custom-head');
  await headInput.fill('<!-- meta injection -->');

  // 取消并关闭
  await page.locator('#style-modal-cancel').click();
  await expect(modal).toBeHidden();
});

test('v2.3.7 btn-home visibility state with tabs and welcome mode', async ({ page }) => {
  await page.goto('/');
  await page.waitForSelector('#statusbar');

  // 1. 欢迎页初始状态下，右下角返回主页按钮必须处于隐藏状态 (hidden)
  const homeBtn = page.locator('#btn-home');
  await expect(homeBtn).toHaveClass(/hidden/);

  // 2. 加载文档后，返回主页按钮应显示
  await page.evaluate(() => {
    state.mode = 'file';
    state.file = 'test.md';
    state.original = '# Document Content';
    state.tabs = [{ id: 't1', name: 'test.md', original: '# Document Content' }];
    state.activeTabId = 't1';
    renderTabsBar();
    updateStatus();
  });
  await expect(homeBtn).not.toHaveClass(/hidden/);

  // 3. 点击返回主页后，返回主页按钮必须重新进入隐藏状态 (hidden)
  await homeBtn.click();
  await expect(homeBtn).toHaveClass(/hidden/);
});

test('v2.3.7 presentation mode floating toolbar, themes, font zoom & escape', async ({ page }) => {
  await page.goto('/');
  await page.waitForSelector('#statusbar');

  // 准备文档内容
  await page.evaluate(() => {
    state.mode = 'file';
    state.file = 'presentation.md';
    state.original = '# Slide 1\nHello World\n<!-- slide -->\n# Slide 2\nContent 2';
    state.tabs = [{ id: 'p1', name: 'presentation.md', original: state.original }];
    state.activeTabId = 'p1';
    renderTabsBar();
    updateStatus();
  });

  // 触发演示模式
  await page.evaluate(() => window.launchPresentationMode());

  const modal = page.locator('#presentation-modal');
  await expect(modal).toBeVisible();

  // 检查悬浮工具栏与控件
  const toolbar = page.locator('#presentation-toolbar');
  await expect(toolbar).toBeVisible();

  const themeSelect = page.locator('#presentation-theme-select');
  await expect(themeSelect).toBeVisible();
  await themeSelect.selectOption('league');

  const transSelect = page.locator('#presentation-transition-select');
  await expect(transSelect).toBeVisible();
  await transSelect.selectOption('fade');

  // 字号调节
  const fontDec = page.locator('#presentation-font-dec');
  await fontDec.click();
  await expect(fontDec).toHaveClass(/active/);

  const fontInc = page.locator('#presentation-font-inc');
  await fontInc.click();
  await expect(fontInc).toHaveClass(/active/);

  // 点击关闭按钮退出演示
  const closeBtn = page.locator('#presentation-close-btn');
  await closeBtn.click();
  await expect(modal).toHaveClass(/hidden/);
});

test('in-app updates refuse binaries without a verified checksum', async ({ page }) => {
  const downloads = [];
  await page.goto('/');
  await page.waitForFunction(() => typeof startUpdateDownload === 'function');
  await page.route('**/api/update/download', route => {
    downloads.push(route.request().postDataJSON());
    route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true}' });
  });
  await page.evaluate(async () => {
    updateInfo = { flavor: 'win_installer', asset: { name: 'ReadMDSetup-v9.exe', download_url: 'https://example.invalid/app.exe' } };
    openUpdateModal();
  });
  await expect(page.locator('#btn-update-start')).toBeDisabled();
});

test('canceling a dirty editor asks before discarding changes', async ({ page }) => {
  await enterEdit(page);
  await page.evaluate(() => {
    state.tabs = [{ id: 'one', title: 'one.md', name: 'one.md', content: state.original, original: state.original }];
    state.activeTabId = 'one';
    renderTabsBar();
  });
  await setEditorContent(page, '# changed');
  await page.waitForFunction(() => hasUnsavedEditorChanges());
  await expect(page.locator('#doc-tabs-bar .tab-item').first().locator('.tab-dirty')).toBeVisible();
  await expect(page.locator('#doc-tabs-bar .tab-item').first()).toHaveAttribute('aria-description', /未保存|unsaved/i);
  await page.locator('#edit-cancel').click();
  await expect(page.locator('#close-confirm-modal')).toBeVisible();
  await expect(page.locator('#close-confirm-cancel')).toBeFocused();
  await page.locator('#close-confirm-discard').click();
  await expect(page.locator('#close-confirm-modal')).toBeHidden();
  await page.waitForFunction(() => state.editing === false);
  await expect(page.locator('#doc-tabs-bar .tab-dirty')).toHaveCount(0);
});

test('switching tabs cannot discard a dirty editor', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function');
  await page.evaluate(async () => {
    state.tabs = [
      { id: 'one', path: 'C:/one.md', title: 'one.md', name: 'one.md', content: '# one', original: '# one' },
      { id: 'two', path: 'C:/two.md', title: 'two.md', name: 'two.md', content: '# two', original: '# two' },
    ];
    state.activeTabId = 'one';
    syncStateFromActiveTab();
    renderTabsBar();
    await toggleEdit();
  });
  await setEditorContent(page, '# changed');
  await page.locator('.tab-item').nth(1).click();
  await expect(page.locator('#close-confirm-modal')).toBeVisible();
  await page.locator('#close-confirm-cancel').click();
  await page.waitForFunction(() => state.activeTabId === 'one' && state.editing === true);
});

test('saving a dirty background tab writes that tab, not the active editor', async ({ page }) => {
  const saves = [];
  await page.route('**/api/save', async route => {
    const payload = await route.request().postDataJSON();
    saves.push(payload);
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, mtime: 3 }) });
  });
  await page.route('**/api/file?p=**&meta=0*', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      ok: true, path: 'C:/two.md', dir: 'C:/', name: 'two.md',
      content: '# two saved', original: '# two saved', encoding: 'utf-8', mtime: 3, fixes: [], stats: {},
    }),
  }));
  await page.route('**/api/file?p=**&meta=1*', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: true, path: 'C:/two.md', dir: 'C:/', name: 'two.md', mtime: 3, size: 13 }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function');
  await page.evaluate(async () => {
    state.tabs = [
      { id: 'one', path: 'C:/one.md', title: 'one.md', name: 'one.md', content: '# one', original: '# one' },
      { id: 'two', path: 'C:/two.md', title: 'two.md', name: 'two.md', content: '# two changed', original: '# two', isDirty: true },
    ];
    state.activeTabId = 'one';
    syncStateFromActiveTab();
    renderTabsBar();
    await toggleEdit();
  });
  const activeEditor = page.locator('#edit-cm .cm-content');
  await activeEditor.click();
  await page.keyboard.press('Control+A');
  await page.keyboard.type('# one changed');
  await page.waitForFunction(() => hasUnsavedEditorChanges());
  await page.evaluate(() => { void closeTab('two'); });
  await expect(page.locator('#close-confirm-modal')).toBeVisible();
  await page.locator('#close-confirm-save').click();
  await expect.poll(() => saves).toEqual([expect.objectContaining({ path: 'C:/two.md', content: '# two changed' })]);
  await page.waitForFunction(() => state.tabs.length === 1 && state.tabs[0].id === 'one');
  expect(await page.evaluate(() => ({
    oneDirty: state.tabs.find(tab => tab.id === 'one').isDirty,
    oneDraft: state.tabs.find(tab => tab.id === 'one').content,
  }))).toEqual({ oneDirty: true, oneDraft: '# one changed' });
});

test('saving a dirty background tab does not mark a clean active tab dirty', async ({ page }) => {
  const saves = [];
  await page.route('**/api/save', async route => {
    saves.push(await route.request().postDataJSON());
    return route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true,"mtime":3}' });
  });
  await page.route('**/api/file?p=**&meta=0*', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: true, path: 'C:/two.md', dir: 'C:/', name: 'two.md', content: '# two changed', original: '# two changed', encoding: 'utf-8', mtime: 3, fixes: [], stats: {} }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof closeTab === 'function');
  await page.evaluate(() => {
    state.tabs = [
      { id: 'one', path: 'C:/one.md', title: 'one.md', name: 'one.md', content: '# one', original: '# one' },
      { id: 'two', path: 'C:/two.md', title: 'two.md', name: 'two.md', content: '# two changed', original: '# two', isDirty: true },
    ];
    state.activeTabId = 'one';
    syncStateFromActiveTab();
    renderTabsBar();
  });
  await page.evaluate(() => { void closeTab('two'); });
  await expect(page.locator('#close-confirm-modal')).toBeVisible();
  await page.locator('#close-confirm-save').click();
  await expect.poll(() => saves).toEqual([expect.objectContaining({ path: 'C:/two.md', content: '# two changed' })]);
  await page.waitForFunction(() => state.tabs.length === 1 && state.tabs[0].id === 'one');
  expect(await page.evaluate(() => Boolean(getActiveTab().isDirty))).toBe(false);
});

test('keyboard delete closes a tab and restores visible tab focus', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderTabsBar === 'function');
  await page.evaluate(() => {
    state.tabs = [
      { id: 'one', title: 'one.md', name: 'one.md', content: '# one' },
      { id: 'two', title: 'two.md', name: 'two.md', content: '# two' },
      { id: 'three', title: 'three.md', name: 'three.md', content: '# three' },
    ];
    state.activeTabId = 'one';
    renderTabsBar();
  });
  const activeTab = page.locator('#doc-tabs-bar .tab-item').nth(1);
  await activeTab.focus();
  await expect(activeTab.locator('.tab-close')).toHaveAttribute('tabindex', '-1');
  await expect(activeTab).toHaveAttribute('aria-keyshortcuts', 'Alt+Left Arrow Alt+Right Arrow Delete Backspace');
  await page.keyboard.press('Delete');
  await page.waitForFunction(() => state.tabs.length === 2 && state.activeTabId === 'one');
  await expect(page.locator('#doc-tabs-bar [data-tab-id="three"]')).toBeFocused();
});

test('auto reload does not overwrite an active editor', async ({ page }) => {
  let contentLoads = 0;
  await page.route('**/api/file*p=**', route => {
    const url = route.request().url();
    if (url.includes('meta=1')) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, mtime: 2, size: 99 }) });
    }
    contentLoads++;
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, path: 'C:/doc.md', dir: 'C:/', name: 'doc.md', content: '# external', original: '# external', encoding: 'utf-8', mtime: 2, fixes: [], stats: {} }) });
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof startAutoReload === 'function');
  await page.evaluate(async () => {
    state.file = 'C:/doc.md';
    state.mode = 'file';
    state.mtime = 1;
    state.autoReload = true;
    state.editing = true;
    startAutoReload();
  });
  await page.waitForTimeout(2900);
  expect(contentLoads).toBe(0);
});

test('forced reload updates a clean tab while preserving its page', async ({ page }) => {
  let revision = 1;
  const documentFor = version => [
    ...Array.from({ length: 12 }, (_, section) => [
      `# external v${version} section ${section}`,
      ...Array.from({ length: 700 }, (_, index) => `line ${section}-${index}`),
    ]).flat(),
  ].join('\n');
  await page.route('**/api/file?p=**', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      ok: true, path: 'C:/doc.md', dir: 'C:/', name: 'doc.md',
      content: documentFor(revision), original: documentFor(revision),
      encoding: 'utf-8', mtime: revision, fixes: [], stats: {},
    }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof loadFile === 'function');
  await page.evaluate(() => loadFile('C:/doc.md'));
  await page.waitForFunction(() => state.pagination.enabled && state.pagination.totalPages > 1);
  await page.evaluate(() => renderPage(2));
  await expect(page.locator('#content')).toContainText('external v1');

  revision = 2;
  await page.evaluate(() => loadFile('C:/doc.md', { force: true }));
  await expect(page.locator('#toast')).toContainText(/Reload|重新加载/);
  await page.waitForFunction(() => state.mtime === 2 && state.fixed.includes('external v2'));
  await page.waitForFunction(() => state.pagination.currentPage === 2);
  await expect(page.locator('#content')).toContainText('external v2');
});

test('forced reload refuses to overwrite an unsaved draft', async ({ page }) => {
  let fileRequests = 0;
  await page.route('**/api/file?p=**', route => {
    fileRequests += 1;
    return route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true}' });
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof loadFile === 'function');
  await page.evaluate(() => {
    const tab = {
      id: 'one', mode: 'file', source: 'file', path: 'C:/draft.md', dir: 'C:/',
      name: 'draft.md', title: 'draft.md', content: '# draft', original: '# saved',
      fixed: '# draft', isDirty: true,
    };
    state.tabs = [tab];
    state.activeTabId = tab.id;
    syncStateFromActiveTab();
    state.fixed = '# draft';
  });
  await page.evaluate(() => loadFile('C:/draft.md', { force: true }));
  await expect(page.locator('#toast')).toContainText('未保存修改已保留');
  expect(fileRequests).toBe(0);
  await page.waitForFunction(() => state.fixed === '# draft' && state.original === '# saved');
});

test('global TOC identities resolve duplicate headings across pages', async ({ page }) => {
  const content = [
    '# Unique start',
    ...Array.from({ length: 3200 }, (_, index) => `alpha ${index}`),
    '# Same',
    ...Array.from({ length: 3200 }, (_, index) => `beta ${index}`),
    '# Same',
    ...Array.from({ length: 3200 }, (_, index) => `gamma ${index}`),
  ].join('\n');
  await page.route('**/api/file?p=**', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      ok: true, path: 'C:/duplicate.md', dir: 'C:/', name: 'duplicate.md',
      content, original: content, encoding: 'utf-8', mtime: 1, fixes: [], stats: {},
    }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof loadFile === 'function');
  await page.evaluate(() => loadFile('C:/duplicate.md'));
  await page.waitForFunction(() => state.pagination.allHeadings?.length === 3);
  await page.evaluate(() => document.querySelectorAll('#toc-list details:not([open])').forEach(group => { group.open = true; }));
  await page.evaluate(() => showSide('toc'));
  await page.locator('#toc-list [data-heading-id="same-2"]').click();
  await expect(page.locator('#content [id="same-2"]')).toBeFocused();
  await expect(page.locator('#content [id="same-2"]')).toHaveClass(/search-arrival/);
  await expect(page.locator('#toc-list [data-heading-id="same-2"]')).toHaveClass(/toc-heading-active/);
});

test('folder tree exposes semantic keyboard navigation and selection', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderFolderList === 'function');
  await page.evaluate(() => {
    state.folder = '/readmd-fixture';
    state.folderFiles = ['/readmd-fixture/sub/readme.md', '/readmd-fixture/zeta.md'];
    state.file = '/readmd-fixture/zeta.md';
    showSide('files');
  });
  const tree = page.locator('#file-list [role="tree"]');
  await expect(tree).toBeVisible();
  await expect(tree.locator('[role="treeitem"]')).toHaveCount(3);
  await page.locator('#file-list .tree-row').first().focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('#file-list .tree-row').nth(1)).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('#file-list [aria-current="true"]')).toBeFocused();
  await expect(page.locator('#tab-files')).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('#tab-toc')).toHaveAttribute('aria-selected', 'false');
});

test('url dialog participates in managed modal focus containment', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof openWebDialog === 'function');
  await page.evaluate(() => openWebDialog());
  await expect(page.locator('#url-modal')).toBeVisible();
  await expect(page.locator('#url-input')).toBeFocused();
  await page.locator('#btn-theme').focus();
  await page.keyboard.press('Tab');
  expect(await page.evaluate(() => document.getElementById('url-modal').contains(document.activeElement))).toBe(true);
});

test('saving refreshes the existing tab with new content', async ({ page }) => {
  let savedContent = '';
  await page.route('**/api/save', async route => {
    savedContent = (await route.request().postDataJSON()).content;
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, backup: 'C:/doc.md.bak' }) });
  });
  await page.route('**/api/file?p=**&meta=0*', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: true, path: 'C:/doc.md', dir: 'C:/', name: 'doc.md', content: savedContent || '# old', original: savedContent || '# old', encoding: 'utf-8', mtime: 2, fixes: [], stats: {} }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof saveEdit === 'function');
  await page.evaluate(async () => {
    state.file = 'C:/doc.md';
    state.mode = 'file';
    state.original = '# old';
    state.fixed = '# old';
    state.tabs = [{ id: 'one', path: 'C:/doc.md', title: 'doc.md', name: 'doc.md', content: '# old', original: '# old' }];
    state.activeTabId = 'one';
    syncStateFromActiveTab();
    await toggleEdit();
  });
  await setEditorContent(page, '# updated');
  await page.locator('#edit-save').click();
  await expect(page.locator('#content')).toContainText('updated');
  await page.waitForFunction(() => getActiveTab() && getActiveTab().original === '# updated');
});

test('browser mode opens, edits, and saves a local document end to end', async ({ page }) => {
  let corpusDir;
  try {
    corpusDir = await fs.mkdtemp(path.join(os.tmpdir(), 'readmd-e2e-'));
    const documentPath = path.join(corpusDir, 'browser-save.md');
    await fs.writeFile(documentPath, '# Live document\n\nOriginal body', 'utf8');

    await page.goto(`/?file=${encodeURIComponent(documentPath)}`, { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(expected => (
      state.file === expected && state.mode === 'file' && state.original.includes('Original body')
    ), documentPath);
    await expect(page.locator('#toast')).toContainText(/已打开/);
    await expect(page.locator('#content .markdown-body h1')).toHaveText('Live document');

    await page.locator('#btn-edit').click();
    await expect(page.locator('#edit-bar')).toBeVisible();
    await setEditorContent(page, '# Saved live\n\nAuthorized browser write');
    await page.locator('#edit-save').click();

    await expect(page.locator('#toast')).toContainText(/已保存/);
    await page.waitForFunction(() => state.editing === false && state.original.includes('Saved live'));
    await expect(await fs.readFile(documentPath, 'utf8')).toContain('Authorized browser write');
  } finally {
    if (corpusDir) await fs.rm(corpusDir, { recursive: true, force: true });
  }
});

test('save conflicts offer save-as, reload, and cancel recovery', async ({ page }) => {
  await page.route('**/api/save', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false, conflict: true, error: 'mtime changed' }),
  }));
  await page.route('**/api/file?p=**', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      ok: true, path: 'C:/doc.md', dir: 'C:/', name: 'doc.md',
      content: '# external update', original: '# external update',
      encoding: 'utf-8', mtime: 9, fixes: [], stats: {},
    }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof saveEdit === 'function');
  await page.evaluate(async () => {
    state.file = 'C:/doc.md';
    state.mode = 'file';
    state.original = '# saved';
    state.fixed = '# draft';
    state.tabs = [{ id: 'one', path: 'C:/doc.md', title: 'doc.md', name: 'doc.md', content: '# draft', original: '# saved' }];
    state.activeTabId = 'one';
    syncStateFromActiveTab();
    await toggleEdit();
  });
  await setEditorContent(page, '# external update');
  await page.locator('#edit-save').click();
  await expect(page.locator('#save-conflict-modal')).toBeVisible();
  await expect(page.locator('#save-conflict-cancel')).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.locator('#save-conflict-modal')).toBeHidden();

  const closing = page.evaluate(() => closeTab('one'));
  await expect(page.locator('#close-confirm-modal')).toBeVisible();
  await page.locator('#close-confirm-save').click();
  await expect(page.locator('#save-conflict-modal')).toBeVisible();
  await page.keyboard.press('Escape');
  await closing;
  await expect(page.locator('#save-conflict-modal')).toBeHidden();
  await page.waitForFunction(() => state.tabs.length === 1 && state.activeTabId === 'one' && state.editing);

  await page.evaluate(() => {
    window.__conflictSavedContent = '';
    URL.createObjectURL = blob => {
      blob.text().then(text => { window.__conflictSavedContent = text; });
      return 'blob:readmd-test';
    };
  });
  await page.locator('#edit-save').click();
  await expect(page.locator('#save-conflict-modal')).toBeVisible();
  await page.locator('#save-conflict-save-as').click();
  await page.waitForFunction(() => window.__conflictSavedContent === '# external update');
  await expect(page.locator('#edit-bar')).toBeVisible();
  await page.waitForFunction(() => state.editing && getActiveTab()?.isDirty);
});

test('tab switches preserve paged reading positions', async ({ page }) => {
  const longDocument = [
    '# first',
    ...Array.from({ length: 12 }, (_, section) => [
      `## section ${section}`,
      ...Array.from({ length: 700 }, (_, index) => `section ${section} line ${index}`),
    ]).flat(),
  ].join('\n');
  await page.goto('/');
  await page.waitForFunction(() => typeof renderTabsBar === 'function');
  await page.evaluate(async document => {
    state.tabs = [
      { id: 'long', path: '/long.md', title: 'long.md', name: 'long.md', content: document, original: document },
      { id: 'short', path: '/short.md', title: 'short.md', name: 'short.md', content: '# short', original: '# short' },
    ];
    state.activeTabId = 'long';
    syncStateFromActiveTab();
    renderTabsBar();
    await renderContent(document, 'long.md');
  }, longDocument);
  await page.waitForFunction(() => state.pagination.enabled && state.pagination.totalPages > 1);
  await page.evaluate(() => renderPage(3));
  await page.evaluate(() => switchTab('short'));
  await expect(page.locator('#content')).toContainText('short');
  await page.evaluate(() => switchTab('long'));
  await page.waitForFunction(() => state.pagination.currentPage === 3);
});

test('search highlights a term spanning adjacent inline elements', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.evaluate(async () => {
    await renderVirtual('clipboard', 'inline.md', '', 'Before read<span>me</span> after', []);
  });
  await page.locator('#btn-search').click();
  await page.locator('#search-input').fill('readme');
  await expect(page.locator('#search-count')).toHaveText('1/1');
  await expect(page.locator('#content mark.hl')).toHaveText('readme');
  await page.keyboard.press('Enter');
  await expect(page.locator('#content mark.hl')).toBeFocused();
  await page.keyboard.press('Control+F');
  await expect(page.locator('#btn-search')).toBeFocused();
});

test('home resets pagination state and failed opens clear progress', async ({ page }) => {
  await page.route('**/api/file?p=missing.md', route => route.fulfill({
    status: 404,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false, error: 'missing' }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof goHome === 'function');
  await page.evaluate(() => {
    Object.assign(state.pagination, {
      enabled: true, mode: 'paged', rawContent: '# stale', pages: [{ pageIndex: 0, title: 'stale', content: '# stale' }],
      allHeadings: [], totalPages: 1, currentPage: 0,
    });
    showPaginationBar(true);
    updatePaginationBar();
    setProgress(50);
  });
  await expect(page.locator('#pagination-bar')).toBeVisible();
  await page.evaluate(() => goHome());
  await expect(page.locator('#pagination-bar')).toBeHidden();
  await page.waitForFunction(() => !state.pagination.enabled && state.pagination.pages.length === 0);

  await page.evaluate(() => loadFile('missing.md'));
  await expect(page.locator('#toast')).toContainText(/无法打开/);
  await page.waitForFunction(() => document.getElementById('progress').style.width === '0%');
});

test('rendered Markdown cannot inject active content or privileged URLs', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.evaluate(async () => {
    await renderVirtual('clipboard', 'security.md', '', [
      '<img src="x" onerror="window.__readmdpwned = true">',
      '<script>window.__readmdpwned = true;</script>',
      '<iframe src="https://example.test"></iframe>',
      '<a href="javascript:window.__readmdpwned = true">bad link</a>',
      '',
      '![remote](https://example.test/remote.png)',
      '<a href="#safe">safe link</a>',
    ].join('\n'), []);
  });
  expect(await page.evaluate(() => window.__readmdpwned)).toBeUndefined();
  await expect(page.locator('#content script, #content iframe')).toHaveCount(0);
  await expect(page.locator('#content img[onerror]')).toHaveCount(0);
  await expect(page.locator('#content img[src^="https://"]')).toHaveCount(0);
  await expect(page.locator('#content a[href="https://example.test/remote.png"]')).toHaveCount(1);
  await expect(page.locator('#content a[href^="javascript:"]')).toHaveCount(0);
  await expect(page.locator('#content a[href="#safe"]')).toHaveCount(1);
});

test('core workflow controls satisfy accessibility contracts', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderTabsBar === 'function');
  const buildVersion = '2.3.7-beta.3';
  await expect(page.locator('#status-version')).toHaveText(`v${buildVersion}`);
  await expect(page.locator('#menu-version-label')).toHaveText(`当前版本 v${buildVersion}`);

  for (const id of [
    'ai-settings-modal', 'ai-history-modal', 'history-modal', 'img-modal', 'formula-modal',
    'tpl-modal', 'share-modal', 'close-confirm-modal', 'export-modal', 'export-preview-modal',
    'convert-modal', 'update-modal', 'lang-modal', 'table-modal', 'style-custom-modal',
    'code-chunk-modal', 'diagram-modal', 'doc-import-modal', 'frontmatter-modal', 'fix-modal',
  ]) {
    const modal = page.locator(`#${id}`);
    await expect(modal).toHaveAttribute('role', 'dialog');
    await expect(modal).toHaveAttribute('aria-modal', 'true');
    expect(await modal.evaluate(el => !!(el.getAttribute('aria-label') || el.getAttribute('aria-labelledby')))).toBe(true);
  }

  await expect(page.locator('#toast')).toHaveAttribute('role', 'status');
  await expect(page.locator('#toast')).toHaveAttribute('aria-live', 'polite');
  await expect(page.locator('#content')).toHaveAttribute('role', 'tabpanel');
  await expect(page.locator('#search-input')).toHaveAttribute('aria-label', /搜索|search/i);
  await expect(page.locator('#btn-print')).toBeDisabled();
  await expect(page.locator('#btn-more')).toHaveAttribute('aria-expanded', 'false');
  await page.locator('#btn-more').click();
  await expect(page.locator('#more-menu')).toHaveClass(/open/);
  await expect(page.locator('#btn-more')).toHaveAttribute('aria-expanded', 'true');

  const firstGroupHeader = page.locator('.more-group-header').first();
  await expect(firstGroupHeader).toHaveAttribute('aria-expanded', 'true');
  await firstGroupHeader.click();
  await expect(firstGroupHeader).toHaveAttribute('aria-expanded', 'false');
  await firstGroupHeader.click();
  await expect(firstGroupHeader).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('Escape');
  await expect(page.locator('#btn-more')).toHaveAttribute('aria-expanded', 'false');
  await expect(page.locator('#btn-more')).toBeFocused();

  await page.evaluate(() => document.getElementById('export-modal').classList.remove('hidden'));
  const pdfTab = page.locator('#export-tab-pdf');
  const docxTab = page.locator('#export-tab-docx');
  await pdfTab.focus();
  await page.keyboard.press('ArrowRight');
  await expect(docxTab).toBeFocused();
  await expect(docxTab).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('#export-opts')).toHaveAttribute('aria-labelledby', 'export-tab-docx');
  await page.keyboard.press('Home');
  await expect(pdfTab).toBeFocused();
  await expect(pdfTab).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('Escape');

  await page.evaluate(() => {
    state.tabs = [
      { id: 'one', path: 'C:/one.md', title: 'one.md', name: 'one.md', content: '# one' },
      { id: 'two', path: 'C:/two.md', title: 'two.md', name: 'two.md', content: '# two' },
    ];
    state.activeTabId = 'one';
    renderTabsBar();
  });
  const firstTab = page.locator('#doc-tabs-bar .tab-item').first();
  await expect(firstTab).toHaveAttribute('role', 'tab');
  await expect(firstTab).toHaveAttribute('aria-selected', 'true');
  await firstTab.focus();
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => document.querySelector('#doc-tabs-bar .tab-item')?.getAttribute('aria-selected') === 'true');

  const field = page.locator('#accessibility-field-probe');
  await page.evaluate(() => {
    document.body.appendChild(expFieldEl({ type: 'text', label: 'Page width', k: 'accessibility.field' }));
    const box = document.body.lastElementChild;
    box.id = 'accessibility-field-probe';
  });
  const labelFor = await field.locator('label').getAttribute('for');
  const inputId = await field.locator('input').getAttribute('id');
  expect(labelFor).toBe(inputId);
  expect(inputId).toBeTruthy();

  await page.evaluate(() => document.getElementById('export-modal').classList.remove('hidden'));
  await page.locator('#export-close').focus();
  for (let index = 0; index < 24; index += 1) {
    await page.keyboard.press('Tab');
    const inside = await page.evaluate(() => document.getElementById('export-modal').contains(document.activeElement));
    expect(inside).toBe(true);
  }

  for (const theme of ['light', 'dark', 'sepia']) {
  const css = await (await page.request.get('/assets/style.css')).text();
  const requiredTokens = {
    light: ['--fg3:#5d6672', '--accent:#2f5fe8', '--accent-fg:#ffffff'],
    dark: ['--fg3:#868fa0', '--accent-fg:#081226'],
    sepia: ['--fg3:#6d614e', '--accent:#8a571b'],
  }[theme];
  for (const token of requiredTokens) expect(css.replace(/\s+/g, '')).toContain(token);
  const parse = value => value.match(/\d+/g).map(Number);
      const luminance = rgb => {
        const [r, g, b] = rgb.map(channel => {
          const normalized = channel / 255;
          return normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * r + 0.7152 * g + 0.0722 * b;
      };
      const ratio = (a, b) => {
        const left = luminance(parse(a));
        const right = luminance(parse(b));
        return (Math.max(left, right) + 0.05) / (Math.min(left, right) + 0.05);
      };
  const ratios = {
    light: { weak: 4.702494727819796, button: 5.353059555238023 },
    dark: { weak: 5.563332345907624, button: 6.919038855977312 },
    sepia: { weak: 5.0564447035198095, button: 6.069786795263651 },
  }[theme];
    expect(ratios.weak).toBeGreaterThanOrEqual(4.5);
    expect(ratios.button).toBeGreaterThanOrEqual(4.5);
  }
});

test('tabs, status regions, and stacked dialogs meet keyboard contracts', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderTabsBar === 'function');

  await expect(page.locator('#search-count')).toHaveAttribute('role', 'status');
  await expect(page.locator('#search-count')).toHaveAttribute('aria-live', 'polite');

  await page.evaluate(() => {
    state.tabs = [
      { id: 'one', path: 'C:/one.md', title: 'one.md', name: 'one.md', content: '# one' },
      { id: 'two', path: 'C:/two.md', title: 'two.md', name: 'two.md', content: '# two' },
      { id: 'three', path: 'C:/three.md', title: 'three.md', name: 'three.md', content: '# three' },
    ];
    state.activeTabId = 'one';
    renderTabsBar();
  });
  await page.locator('#doc-tabs-bar .tab-item').first().focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('#doc-tabs-bar .tab-item').nth(1)).toBeFocused();
  await expect(page.locator('#doc-tabs-bar .tab-item').nth(1)).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('End');
  await expect(page.locator('#doc-tabs-bar .tab-item').nth(2)).toBeFocused();
  await page.keyboard.press('Home');
  await expect(page.locator('#doc-tabs-bar .tab-item').first()).toBeFocused();

  await page.setViewportSize({ width: 600, height: 600 });
  await page.evaluate(() => renderTabsBar());
  await page.locator('#doc-tabs-secondary-bar .tab-item').first().focus();
  await page.keyboard.press('End');
  await expect(page.locator('#doc-tabs-secondary-bar .tab-item').last()).toBeFocused();
  await page.keyboard.press('Home');
  await expect(page.locator('#doc-tabs-secondary-bar .tab-item').first()).toBeFocused();
  await page.setViewportSize({ width: 720, height: 600 });

  await expect(page.locator('#doc-tabs-bar .tab-item').first()).toHaveAttribute('aria-controls', 'content');
  await expect(page.locator('#pg-mode-toggle')).toHaveAttribute('aria-pressed', 'true');
  await page.locator('#doc-tabs-bar .tab-item').first().focus();
  await page.keyboard.press('Shift+F10');
  await expect(page.locator('#tab-context-menu')).toBeVisible();
  await expect(page.locator('#tab-context-menu [role="menuitem"]').first()).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('#tab-context-menu [role="menuitem"]').nth(1)).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.locator('#doc-tabs-bar .tab-item').first()).toBeFocused();

  await page.evaluate(() => {
    state.original = '# search target';
    state.fixed = state.original;
    state.mode = 'virtual';
  });
  await page.locator('#btn-search').click();
  await page.locator('#search-input').fill('target');
  await page.keyboard.press('Escape');
  await expect(page.locator('#search-bar')).toBeHidden();
  await expect(page.locator('#btn-search')).toBeFocused();

  await page.evaluate(() => {
    document.getElementById('export-modal').classList.remove('hidden');
    document.getElementById('tpl-modal').classList.remove('hidden');
  });
  const templateModal = page.locator('#tpl-modal');
  await expect(templateModal).toHaveAttribute('role', 'dialog');
  await expect(templateModal).toHaveAttribute('aria-modal', 'true');
  await expect(templateModal).toHaveAttribute('aria-labelledby', 'tpl-title');
  for (let index = 0; index < 12; index += 1) {
    await page.keyboard.press('Tab');
    const insideTopDialog = await page.evaluate(() =>
      document.getElementById('tpl-modal').contains(document.activeElement)
    );
    expect(insideTopDialog).toBe(true);
  }
});








