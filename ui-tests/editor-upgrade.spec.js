// Editor lane: slash menu, smart editing, selection toolbar, AI hook, stats.
const { test, expect } = require('@playwright/test');

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200, contentType: 'application/json', body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'en'));
});

async function openEditor(page, markdown = '# Draft\n\nHello world\n') {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function' && typeof toggleEdit === 'function');
  await page.evaluate(async md => {
    await renderVirtual('clipboard', 'editor.md', '', md, []);
    state.pvLayout = 'none';
    await toggleEdit();
  }, markdown);
  await expect(page.locator('#edit-bar')).toBeVisible();
  await page.waitForFunction(() => window.cmView && document.querySelector('#edit-cm .cm-content'));
}

/** Replace the document and put the caret at `pos` (default: end). */
async function setDoc(page, doc, pos) {
  await page.evaluate(([d, p]) => {
    cmView.dispatch({ changes: { from: 0, to: cmView.state.doc.length, insert: d }, selection: { anchor: p == null ? d.length : p } });
    cmView.focus();
  }, [doc, pos == null ? null : pos]);
}

const docText = page => page.evaluate(() => cmView.state.doc.toString());

test('slash menu is a filterable listbox that inserts a table with a size picker', async ({ page }) => {
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  await openEditor(page);
  await setDoc(page, 'Intro\n\n');
  await page.keyboard.type('/');
  const menu = page.locator('#cm-slash-menu');
  await expect(menu).toBeVisible();
  await expect(menu).toHaveAttribute('role', 'listbox');
  expect(await menu.locator('[role="option"]').count()).toBeGreaterThanOrEqual(20);
  await expect(page.locator('#edit-cm .cm-content')).toHaveAttribute('aria-activedescendant', /cm-slash-opt-/);

  // Fuzzy filter ("tbl" -> Table) and keyboard selection.
  await page.keyboard.type('tbl');
  await expect(menu.locator('[aria-selected="true"] .cm-slash-label')).toHaveText('Table');
  await page.keyboard.press('Enter');
  await expect(menu).toHaveClass(/is-table/);
  await page.keyboard.press('ArrowRight');   // 4 columns
  await page.keyboard.press('ArrowDown');    // 4 rows (header + 3)
  await expect(menu.locator('.cm-slash-size')).toHaveText('4 × 4');
  await page.keyboard.press('Enter');
  await expect(menu).toBeHidden();

  const doc = await docText(page);
  const table = doc.split('\n').slice(2);
  expect(table[0]).toMatch(/^\| Header 1 +\| Header 2 +\| Header 3 +\| Header 4 +\|$/);
  expect(table[1]).toMatch(/^\|( -+ \|){4}$/);
  expect(table).toHaveLength(5);
  expect(doc).not.toContain('/tbl');
  // Header cell is selected so typing renames it.
  expect(await page.evaluate(() => cmView.state.sliceDoc(cmView.state.selection.main.from, cmView.state.selection.main.to))).toBe('Header 1');

  // One undo step removes the whole insertion and restores the typed query.
  await page.keyboard.press('Control+z');
  expect(await docText(page)).toBe('Intro\n\n/tbl');
  expect(errors).toEqual([]);
});

test('slash menu inserts a callout, a code block with a language, and closes on Escape', async ({ page }) => {
  await openEditor(page);
  await setDoc(page, '');
  await page.keyboard.type('/warn');
  await expect(page.locator('#cm-slash-menu [aria-selected="true"] .cm-slash-label')).toHaveText('Warning');
  await page.keyboard.press('Enter');
  await page.keyboard.type('Back up first.');
  expect(await docText(page)).toBe('> [!WARNING]\n> Back up first.');
  await expect(page.locator('#edit-cm .cm-md-callout-warning').first()).toBeVisible();

  await setDoc(page, '');
  await page.keyboard.type('/code');
  await page.keyboard.press('Enter');
  await expect(page.locator('#cm-slash-menu')).toHaveAttribute('aria-label', 'Code language');
  await page.keyboard.type('py');
  await page.keyboard.press('Enter');
  await page.keyboard.type('print(1)');
  expect(await docText(page)).toBe('```python\nprint(1)\n```');

  await setDoc(page, 'text ');
  await page.keyboard.type('/');
  await expect(page.locator('#cm-slash-menu')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('#cm-slash-menu')).toBeHidden();
  // Escape closed the menu only: still editing, the slash stays as text.
  await expect(page.locator('#edit-bar')).toBeVisible();
  expect(await docText(page)).toBe('text /');

  // A slash inside a word is plain text.
  await setDoc(page, 'and');
  await page.keyboard.type('/or');
  await expect(page.locator('#cm-slash-menu')).toBeHidden();
});

test('lists continue on Enter, exit on an empty item and indent with Tab', async ({ page }) => {
  await openEditor(page);
  await setDoc(page, '');
  await page.keyboard.type('- one');
  await page.keyboard.press('Enter');
  await page.keyboard.type('two');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Tab');
  await page.keyboard.type('nested');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.type('three');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Enter');
  await page.keyboard.type('After the list');
  expect(await docText(page)).toBe('- one\n- two\n  - nested\n- three\n\nAfter the list');

  await setDoc(page, '');
  await page.keyboard.type('1. first');
  await page.keyboard.press('Enter');
  await page.keyboard.type('second');
  expect(await docText(page)).toBe('1. first\n2. second');

  await setDoc(page, '');
  await page.keyboard.type('- [x] done');
  await page.keyboard.press('Enter');
  await page.keyboard.type('next');
  expect(await docText(page)).toBe('- [x] done\n- [ ] next');

  await setDoc(page, '');
  await page.keyboard.type('> quoted');
  await page.keyboard.press('Enter');
  await page.keyboard.type('more');
  expect(await docText(page)).toBe('> quoted\n> more');
});

test('brackets, quotes, backticks and emphasis marks auto-pair', async ({ page }) => {
  await openEditor(page);
  const cases = [
    ['call (x', 'call (x)'],
    ['a [link', 'a [link]'],
    ['say "hi', 'say "hi"'],
    ['use `code', 'use `code`'],
    ['very **bold', 'very **bold**'],
    ['old ~~gone', 'old ~~gone~~'],
    ["it's fine", "it's fine"],
  ];
  for (const [typed, expected] of cases) {
    await setDoc(page, '');
    await page.keyboard.type(typed);
    expect(await docText(page), typed).toBe(expected);
  }
  // Typing the closing mark steps over it instead of doubling it.
  await setDoc(page, '');
  await page.keyboard.type('(a)b');
  expect(await docText(page)).toBe('(a)b');
  await setDoc(page, '');
  await page.keyboard.type('x **b**c');
  expect(await docText(page)).toBe('x **b**c');
  // A selection is wrapped.
  await setDoc(page, 'wrap me');
  await page.evaluate(() => cmView.dispatch({ selection: { anchor: 0, head: 4 } }));
  await page.keyboard.type('*');
  expect(await docText(page)).toBe('*wrap* me');
  // ``` + Enter closes the fence.
  await setDoc(page, '');
  await page.keyboard.type('```js');
  await page.keyboard.press('Enter');
  await page.keyboard.type('let a;');
  expect(await docText(page)).toBe('```js\nlet a;\n```');
});

test('Tab moves between table cells and realigns the pipes', async ({ page }) => {
  await openEditor(page);
  await setDoc(page, '| Name | Qty |\n|---|---|\n| apple | 3 |', 2);
  await page.keyboard.press('Tab');
  await page.keyboard.press('Tab');
  await page.keyboard.type('pear');
  // The next Tab realigns for the new content and selects the next cell.
  await page.keyboard.press('Tab');
  expect(await docText(page)).toBe('| Name | Qty |\n| ---- | --- |\n| pear | 3   |');
  // Tab past the last cell adds a row; typing replaces the empty cell's padding.
  await page.keyboard.press('Tab');
  await page.keyboard.type('kiwi');
  expect((await docText(page)).split('\n')[3]).toBe('| kiwi |     |');
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.type('9');
  expect(await docText(page)).toBe('| Name | Qty |\n| ---- | --- |\n| pear | 9   |\n| kiwi |     |');
});

test('shortcuts match the toolbar and each action is a single undo step', async ({ page }) => {
  await openEditor(page);
  const select = (a, b) => page.evaluate(([x, y]) => { cmView.dispatch({ selection: { anchor: x, head: y } }); cmView.focus(); }, [a, b]);
  const shortcuts = [
    ['Control+b', '**word** rest'],
    ['Control+i', '*word* rest'],
    ['Control+e', '`word` rest'],
    ['Control+Shift+X', '~~word~~ rest'],
    ['Control+k', '[word](url) rest'],
  ];
  for (const [key, expected] of shortcuts) {
    await setDoc(page, 'word rest');
    await select(0, 4);
    await page.keyboard.press(key);
    expect(await docText(page), key).toBe(expected);
    await page.keyboard.press('Control+z');
    expect(await docText(page), key + ' undo').toBe('word rest');
  }
  // Ctrl+E inside the editor must not leave edit mode.
  await expect(page.locator('#edit-bar')).toBeVisible();

  await setDoc(page, 'Title');
  await page.keyboard.press('Control+2');
  expect(await docText(page)).toBe('## Title');
  await page.locator('#edit-bar [data-md="bold"]').click();
  expect(await docText(page)).toContain('**');
  await page.keyboard.press('Control+z');
  expect(await docText(page)).toBe('## Title');

  // Multiple cursors: typing reaches every cursor.
  await setDoc(page, 'a\nb');
  await page.evaluate(() => {
    const { EditorSelection } = window.ReadMDCodeMirror;
    cmView.dispatch({ selection: EditorSelection.create([EditorSelection.cursor(1), EditorSelection.cursor(3)]) });
    cmView.focus();
  });
  await page.keyboard.type('!');
  expect(await docText(page)).toBe('a!\nb!');
});

test('pasting a URL over a selection creates a link', async ({ page }) => {
  await openEditor(page);
  await setDoc(page, 'Read the docs today');
  await page.evaluate(() => {
    cmView.dispatch({ selection: { anchor: 9, head: 13 } });
    const data = new DataTransfer();
    data.setData('text/plain', 'https://example.com/docs');
    cmView.contentDOM.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true }));
  });
  expect(await docText(page)).toBe('Read the [docs](https://example.com/docs) today');
});

test('selection toolbar appears, formats, and fires readmd:ai-inline', async ({ page }, testInfo) => {
  const source = 'Intro line.\n\nSecond line.\n\nPolish this sentence please.\n';
  await openEditor(page, source);
  const line = page.locator('#edit-cm .cm-line').nth(4);
  const box = await line.boundingBox();
  await page.mouse.move(box.x + 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 120, box.y + box.height / 2, { steps: 6 });
  await page.mouse.up();
  const toolbar = page.locator('#cm-selection-toolbar');
  await expect(toolbar).toBeVisible();
  await expect(toolbar).toHaveAttribute('role', 'toolbar');
  // Toolbar sits above the selection (below it on touch screens, where the
  // native selection menu takes the space above) and inside the window.
  const tb = await toolbar.boundingBox();
  if (testInfo.project.name === 'mobile') expect(tb.y).toBeGreaterThanOrEqual(box.y + box.height - 2);
  else expect(tb.y + tb.height).toBeLessThanOrEqual(box.y + 2);
  expect(tb.x).toBeGreaterThanOrEqual(0);
  expect(tb.x + tb.width).toBeLessThanOrEqual(page.viewportSize().width);

  const range = await page.evaluate(() => {
    const s = cmView.state.selection.main;
    return { from: s.from, to: s.to, text: cmView.state.sliceDoc(s.from, s.to) };
  });
  expect(range.text.length).toBeGreaterThan(0);

  await page.evaluate(() => {
    window.__aiInline = null;
    document.addEventListener('readmd:ai-inline', e => { window.__aiInline = e.detail; e.preventDefault(); }, { once: true });
  });
  await page.locator('#cm-sel-ai').click();
  const detail = await page.evaluate(() => window.__aiInline);
  expect(detail).toMatchObject({ selection: range.text, from: range.from, to: range.to });
  // A claimed event does not open the built-in AI bar.
  await expect(page.locator('#edit-ai-bar')).toBeHidden();

  // Format from the toolbar keeps the selection and marks the button pressed.
  await page.evaluate(([a, b]) => { cmView.dispatch({ selection: { anchor: a, head: b } }); cmView.focus(); updateCmSelectionToolbar(); }, [range.from, range.to]);
  await expect(toolbar).toBeVisible();
  await page.locator('#cm-sel-bold').click();
  const trimmed = range.text.replace(/\s+$/, '');
  const from = range.from + (range.text.length - range.text.replace(/^\s+/, '').length);
  expect(await docText(page)).toBe(source.slice(0, from) + '**' + trimmed.trim() + '**' + source.slice(from + trimmed.trim().length));
  await expect(page.locator('#cm-sel-bold')).toHaveAttribute('aria-pressed', 'true');
});

test('edit bar shows live word, character and reading-time stats', async ({ page }) => {
  await openEditor(page);
  await setDoc(page, 'One two three four five\n\n你好世界');
  await expect(page.locator('#edit-doc-stats')).toHaveText('9 words · 23 chars · 1 min read');
  await page.evaluate(() => cmView.dispatch({ selection: { anchor: 0, head: 7 } }));
  await expect(page.locator('#edit-doc-stats')).toHaveText('2 of 9 words selected');
});

test('view menu toggles focus mode, typewriter scrolling and line numbers', async ({ page }) => {
  await openEditor(page, Array.from({ length: 60 }, (_, i) => `Paragraph ${i + 1}\n`).join('\n'));
  await page.locator('#edit-view-trigger').click();
  await expect(page.locator('#edit-view-menu')).toBeVisible();
  await page.locator('#edit-view-focus').click();
  await expect(page.locator('#edit-view-focus')).toHaveAttribute('aria-checked', 'true');
  expect(await page.locator('#edit-cm .cm-focus-dim').count()).toBeGreaterThan(3);
  await page.locator('#edit-view-lines').click();
  await expect(page.locator('#edit-cm .cm-lineNumbers')).toBeVisible();
  await page.locator('#edit-view-typewriter').click();
  await expect(page.locator('#edit-wrap')).toHaveClass(/is-typewriter/);
  // Preferences persist across editor sessions.
  const prefs = await page.evaluate(() => JSON.parse(localStorage.getItem('readmd-editor-prefs')));
  expect(prefs).toMatchObject({ focus: true, lineNumbers: true, typewriter: true });
  for (const id of ['edit-view-focus', 'edit-view-lines', 'edit-view-typewriter']) await page.locator('#' + id).click();
  await expect(page.locator('#edit-cm .cm-focus-dim')).toHaveCount(0);
  await expect(page.locator('#edit-cm .cm-gutters')).toHaveCount(0);
});

test('split preview updates quickly and scroll stays in sync both ways', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name === 'mobile', 'Split preview needs a wide window.');
  const md = Array.from({ length: 80 }, (_, i) => `## Section ${i + 1}\n\nBody text for section ${i + 1}.\n`).join('\n');
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.evaluate(async src => {
    await renderVirtual('clipboard', 'long.md', '', src, []);
    state.pvLayout = 'right';
    state.pvSync = true;
    await toggleEdit();
  }, md);
  await expect(page.locator('#preview-wrap')).toBeVisible();
  await expect(page.locator('#preview-pane h2').first()).toHaveText('Section 1');

  // Typing reaches the preview well under a second.
  await page.evaluate(() => { cmView.dispatch({ changes: { from: 3, to: 10, insert: 'Opening' } }); });
  await expect(page.locator('#preview-pane h2').first()).toHaveText('Opening 1', { timeout: 600 });

  // Editor -> preview
  await page.evaluate(() => {
    const line = cmView.state.doc.line(160);
    cmView.dispatch({ effects: window.ReadMDCodeMirror.EditorView.scrollIntoView(line.from, { y: 'start' }) });
  });
  await expect.poll(() => page.evaluate(() => document.getElementById('preview-wrap').scrollTop)).toBeGreaterThan(500);
  const visibleHeading = () => page.evaluate(() => {
    const wrap = document.getElementById('preview-wrap');
    const top = wrap.getBoundingClientRect().top;
    const hs = [...document.querySelectorAll('#preview-pane h2')];
    const h = hs.find(el => el.getBoundingClientRect().bottom > top + 4);
    return h ? parseInt(h.textContent.replace(/\D+/g, ''), 10) : 0;
  });
  const editorHeading = () => page.evaluate(() => {
    const block = cmView.lineBlockAtHeight(cmView.scrollDOM.scrollTop - (cmView.documentTop - cmView.scrollDOM.getBoundingClientRect().top + cmView.scrollDOM.scrollTop) + 4);
    const n = cmView.state.doc.lineAt(block.from).number;
    for (let i = n; i >= 1; i--) { const m = /^## Section (\d+)/.exec(cmView.state.doc.line(i).text); if (m) return +m[1]; }
    return 0;
  });
  const fromEditor = await editorHeading();
  await expect.poll(visibleHeading).toBeGreaterThanOrEqual(fromEditor - 1);
  expect(await visibleHeading()).toBeLessThanOrEqual(fromEditor + 1);

  // Preview -> editor
  await page.waitForTimeout(250);
  await page.locator('#preview-wrap').evaluate(el => { el.scrollTop = el.scrollHeight * 0.25; });
  await page.waitForTimeout(300);
  const previewAt = await visibleHeading();
  await expect.poll(editorHeading).toBeGreaterThanOrEqual(previewAt - 1);
  expect(await editorHeading()).toBeLessThanOrEqual(previewAt + 1);
});
