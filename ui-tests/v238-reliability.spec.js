const { test, expect } = require('@playwright/test');

async function setEditorContent(page, content) {
  await page.waitForFunction(() => (
    document.querySelector('#edit-cm .cm-content') ||
    !document.getElementById('edit-area')?.classList.contains('hidden')
  ));
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
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'zh-CN'));
});

test('saving in place keeps editing and clears the dirty indicator', async ({ page }) => {
  await page.route('**/api/save', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: true, mtime: 42 }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof saveEdit === 'function');
  await page.evaluate(async () => {
    const tab = {
      id: 'doc', mode: 'file', source: 'file', path: 'C:/doc.md', dir: 'C:/',
      name: 'doc.md', title: 'doc.md', content: '# old', original: '# old',
      fixed: '# old', encoding: 'utf-8', mtime: 1, isDirty: false,
    };
    state.tabs = [tab];
    state.activeTabId = tab.id;
    syncStateFromActiveTab();
    renderTabsBar();
    await toggleEdit();
  });
  await setEditorContent(page, '# changed');
  await page.waitForFunction(() => getActiveTab()?.isDirty === true);

  await page.keyboard.press('Control+S');

  await page.waitForFunction(() => (
    state.original === '# changed' && state.mtime === 42 &&
    getActiveTab()?.original === '# changed' && getActiveTab()?.isDirty === false
  ));
  expect(await page.evaluate(() => state.editing)).toBe(true);
  await expect(page.locator('#edit-bar')).toBeVisible();
  await expect(page.locator('[data-tab-id="doc"] .tab-dirty')).toHaveCount(0);
});

test('opening the same clean path again reloads current disk content', async ({ page }) => {
  let revision = 1;
  let requests = 0;
  await page.route('**/api/file?p=**', route => {
    requests += 1;
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ok: true, path: 'C:/direct.md', dir: 'C:/', name: 'direct.md',
        content: `# revision ${revision}`, original: `# revision ${revision}`,
        encoding: 'utf-8', mtime: revision, fixes: [], stats: {},
      }),
    });
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof loadFile === 'function');
  await page.evaluate(() => loadFile('C:/direct.md'));
  await page.waitForFunction(() => state.original === '# revision 1');
  await page.evaluate(() => {
    state.autoReload = false;
    if (typeof stopAutoReload === 'function') stopAutoReload();
  });

  requests = 0;
  revision = 2;
  await page.evaluate(() => loadFile('C:/direct.md'));

  await page.waitForFunction(() => state.original === '# revision 2' && state.mtime === 2);
  expect(requests).toBe(1);
  expect(await page.evaluate(() => getActiveTab()?.isDirty)).toBe(false);
});

test('paged long documents keep a usable editor when split preview is enabled', async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function');
  await page.evaluate(async () => {
    state.original = '# Long document\n\n' + 'content '.repeat(15000);
    state.fixed = state.original;
    state.file = 'C:/long.md';
    state.sourceName = 'long.md';
    state.pagination.enabled = true;
    state.pagination.mode = 'paged';
    document.getElementById('pagination-bar').classList.remove('hidden');
    state.pvLayout = 'right';
    await toggleEdit();
  });

  await expect(page.locator('#pagination-bar')).toBeHidden();
  const widths = await page.evaluate(() => ({
    main: document.getElementById('main-col').getBoundingClientRect().width,
    editor: document.getElementById('edit-wrap').getBoundingClientRect().width,
    preview: document.getElementById('preview-wrap').getBoundingClientRect().width,
  }));
  expect(widths.editor).toBeGreaterThanOrEqual(420);
  expect(widths.editor + widths.preview).toBeLessThanOrEqual(widths.main + 8);
});

test('narrow editor hides split preview and restores it when space returns', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 480 });
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function');
  await page.evaluate(async () => {
    state.original = '# Narrow';
    state.fixed = state.original;
    state.file = 'C:/narrow.md';
    state.sourceName = 'narrow.md';
    state.pvLayout = 'right';
    await toggleEdit();
  });
  await expect(page.locator('#preview-wrap')).toBeHidden();
  await expect(page.locator('#edit-wrap')).toBeVisible();

  await page.setViewportSize({ width: 1200, height: 800 });
  await page.waitForFunction(() => !document.getElementById('preview-wrap').classList.contains('hidden'));
  await expect(page.locator('#preview-wrap')).toBeVisible();
});

test('checkboxes are compact while their labels remain touch friendly', async ({ page }) => {
  await page.goto('/');
  const sizes = await page.evaluate(() => {
    const label = document.createElement('label');
    label.className = 'ai-sel';
    const input = document.createElement('input');
    input.type = 'checkbox';
    label.append(input, document.createElement('span'));
    document.body.append(label);
    const inputRect = input.getBoundingClientRect();
    const labelRect = label.getBoundingClientRect();
    const result = { inputWidth: inputRect.width, inputHeight: inputRect.height, labelHeight: labelRect.height };
    label.remove();
    return result;
  });
  expect(sizes.inputWidth).toBe(16);
  expect(sizes.inputHeight).toBe(16);
  expect(sizes.labelHeight).toBeGreaterThanOrEqual(44);
});

test('editor selection uses a visible theme-specific highlight', async ({ page }) => {
  await page.goto('/');
  const backgrounds = await page.evaluate(() => {
    const style = getComputedStyle(document.documentElement);
    return {
      defaultSelection: style.getPropertyValue('--editor-selection').trim(),
      checkbox: getComputedStyle(document.getElementById('pv-sync')).width,
    };
  });
  expect(backgrounds.defaultSelection).toMatch(/rgba?\(/);
  expect(backgrounds.defaultSelection).not.toContain(', 0.25)');
  expect(backgrounds.checkbox).toBe('16px');
});

test('mouse drag creates a visibly painted CodeMirror selection', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function');
  await page.evaluate(async () => {
    state.original = 'Select this text with the mouse.\nSecond line.';
    state.fixed = state.original;
    state.file = 'C:/selection.md';
    state.sourceName = 'selection.md';
    state.pvLayout = 'none';
    await toggleEdit();
  });
  const line = page.locator('#edit-cm .cm-line').first();
  const box = await line.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move(box.x + 8, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + Math.min(box.width - 8, 170), box.y + box.height / 2, { steps: 8 });
  await page.mouse.up();
  await page.waitForFunction(() => cmView && !cmView.state.selection.main.empty);
  await expect(page.locator('#edit-cm .cm-selectionBackground').first()).toBeVisible();
  const color = await page.locator('#edit-cm .cm-selectionBackground').first().evaluate(
    element => getComputedStyle(element).backgroundColor
  );
  expect(color).not.toBe('rgba(0, 0, 0, 0)');
});
