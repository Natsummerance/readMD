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
  await expect(page.locator('#ai-connection-label')).toContainText('已就绪');
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

test('chat import previews clipboard, atomic file, and public link sources', async ({ page }) => {
  const imports = [];
  const result = { ok: true, title: '导入测试', source: 'ChatGPT', message_count: 2, warnings: ['已忽略系统消息'], content: '# 导入测试\n\n> 来源：ChatGPT\n\n## 用户\n\n你好\n\n## AI 助手\n\n你好！\n' };
  await page.addInitScript(() => {
    window.pywebview = { api: {
      authorize_clipboard_read: async () => ({ ok: true, token: 'one-time' }),
      read_clipboard: async token => token === 'one-time' ? { text: 'clipboard chat' } : { error: 'bad token' },
      choose_chat_file: async () => ({ ok: true, title: '文件对话', source: '文件', message_count: 2, content: '# 文件对话\n\n## 用户\n\n文件问题\n\n## AI 助手\n\n文件回答\n' }),
      get_settings: async () => ({}), get_recent: async () => [], start_modules: async () => true,
      get_modules_status: async () => ({ modules: {}, errors: {} }),
    } };
  });
  await page.route('**/api/chat/import', route => { imports.push(route.request().postDataJSON()); return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(result) }); });
  await page.goto('/');
  await page.locator('#btn-more').click();
  await page.locator('#btn-chat-import').click();
  await page.locator('#chat-import-clipboard').click();
  await expect(page.locator('#chat-import-preview')).toBeVisible();
  expect(imports[0]).toEqual({ text: 'clipboard chat' });
  await page.locator('#chat-import-file').click();
  await expect(page.locator('#chat-import-preview-title')).toHaveText('文件对话');
  await page.locator('#chat-import-url').fill('https://share.example/chat');
  await page.locator('#chat-import-url-go').click();
  expect(imports[1]).toEqual({ url: 'https://share.example/chat' });
  await page.locator('#chat-import-load').click();
  await expect(page.locator('#ai-output')).toContainText('你好！');
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
  await expect(page.locator('#url-mode')).toHaveValue('smart');
  await expect(page.locator('#url-images')).not.toBeChecked();
  await expect(page.locator('#url-pages')).toHaveValue('10');
  await expect(page.locator('#url-pages')).toHaveAttribute('max', '30');
  await expect(page.locator('#url-private')).not.toBeChecked();
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
  await page.locator('#url-crawl').check();
  await page.locator('#url-go').click();
  await expect.poll(() => seenUrls.length).toBe(2);
  await page.locator('#url-cancel').click();
  await page.waitForFunction(() => !webRun.running);
  await expect(page.locator('#content')).toContainText('One');
  await expect(page.locator('#content')).toContainText('抓取统计');
  await expect(page.locator('#content')).toContainText('跳过');
  await expect(page.locator('#url-status')).toContainText('已保留成功抓取');
});
