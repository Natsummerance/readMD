// Reader lane: callouts, code headers, outline scroll-spy, table wrappers,
// footnotes, reading preferences and the image lightbox.
const { test, expect } = require('@playwright/test');

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200, contentType: 'application/json', body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'en'));
});

async function openDoc(page, md, name = 'reader.md') {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function' && window.ReadMDReader);
  await page.evaluate(async ({ md, name }) => { await renderVirtual('clipboard', name, '', md, []); }, { md, name });
  await expect(page.locator('#content .rd-article')).toBeVisible();
}

const LONG_SECTIONS = Array.from({ length: 8 }, (_, i) => [
  `## Section ${i + 1}`,
  '',
  ...Array.from({ length: 14 }, (_, k) => `Paragraph ${k} of section ${i + 1}. The quick brown fox jumps over the lazy dog, again and again, to fill the page.`),
  '',
].join('\n\n')).join('\n');

test('GitHub alerts and Obsidian callouts render with icons and fold', async ({ page }) => {
  await openDoc(page, [
    '> [!NOTE]', '> Note body.', '',
    '> [!TIP]', '> Tip body.', '',
    '> [!IMPORTANT]', '> Important body.', '',
    '> [!WARNING]', '> Warning body.', '',
    '> [!CAUTION]', '> Caution body.', '',
    '> [!info]- Folded title', '> Hidden until opened.', '',
    '> Plain quote stays a blockquote.',
  ].join('\n'));
  for (const variant of ['note', 'tip', 'important', 'warning', 'caution']) {
    const box = page.locator(`#content .rd-callout--${variant}`).first();
    await expect(box).toBeVisible();
    await expect(box.locator('.rd-callout-icon')).toHaveCount(1);
  }
  await expect(page.locator('#content .rd-callout--note .rd-callout-label').first()).toHaveText('Note');
  await expect(page.locator('#content .rd-callout')).toHaveCount(6);
  await expect(page.locator('#content')).not.toContainText('[!NOTE]');

  const folded = page.locator('#content details.rd-callout');
  await expect(folded).toHaveCount(1);
  await expect(folded).not.toHaveAttribute('open', '');
  await expect(folded.locator('summary')).toContainText('Folded title');
  await expect(folded.locator('.rd-callout-body')).toBeHidden();
  await folded.locator('summary').click();
  await expect(folded.locator('.rd-callout-body')).toBeVisible();
  await expect(page.locator('#content blockquote')).toHaveCount(1);
});

test('code blocks get a language header, syntax colour and a working copy button', async ({ page, context, browserName }) => {
  if (browserName === 'chromium') await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await openDoc(page, ['```python', 'def greet(name):', '    return f"hi {name}"  # comment', '```', '', '```rust linenos', 'fn main() {}', '```'].join('\n'));
  const block = page.locator('#content .rd-code').first();
  await expect(block.locator('.rd-code-lang')).toHaveText('Python');
  await expect(block.locator('.tk-kw').first()).toHaveText('def');
  await expect(block.locator('.tk-com')).toContainText('# comment');
  await expect(page.locator('#content .rd-code').nth(1)).toHaveClass(/rd-code--lines/);

  const copy = block.locator('.rd-code-copy');
  await expect(copy).toHaveAttribute('aria-label', /Copy code/);
  await copy.click();
  await expect(copy).toHaveClass(/is-copied/);
  await expect(copy).toContainText('Copied');
  if (browserName === 'chromium') {
    // The OS clipboard may normalise line endings (CRLF on Windows).
    expect((await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/g, '\n')).toBe('def greet(name):\n    return f"hi {name}"  # comment');
  }
  await expect(copy).not.toHaveClass(/is-copied/, { timeout: 4000 });
});

test('outline scroll-spy follows the reading position', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await openDoc(page, '# Spy\n\n' + LONG_SECTIONS);
  await page.evaluate(() => showSide('toc'));
  const active = page.locator('#toc-list .toc-heading-active');
  await expect(active).toHaveAttribute('data-heading-id', 'spy');

  await page.evaluate(() => document.getElementById('section-5').scrollIntoView({ block: 'start' }));
  await expect(active).toHaveAttribute('data-heading-id', 'section-5');
  await expect(active).toHaveCount(1);

  await page.locator('#toc-list a[data-heading-id="section-2"]').click();
  await expect(active).toHaveAttribute('data-heading-id', 'section-2');
  await expect(page.locator('#rd-toc-progress')).toHaveText(/\d+%/);
  await expect(page.locator('#rd-toc-stats')).toContainText(/min read/);

  // Folding hides nested entries; expanding brings them back.
  await openDoc(page, '# Top\n\n## Child A\n\ntext\n\n## Child B\n\ntext');
  await page.evaluate(() => showSide('toc'));
  await page.locator('#rd-toc-fold').click();
  await expect(page.locator('#toc-list a[data-heading-id="child-a"]')).toBeHidden();
  await page.locator('#rd-toc-fold').click();
  await expect(page.locator('#toc-list a[data-heading-id="child-a"]')).toBeVisible();
});

test('wide tables scroll inside their wrapper at 360 px', async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 740 });
  const header = '| ' + Array.from({ length: 12 }, (_, i) => `Column ${i + 1}`).join(' | ') + ' |';
  const rule = '|' + Array.from({ length: 12 }, () => '---').join('|') + '|';
  const row = '| ' + Array.from({ length: 12 }, (_, i) => `${(i + 1) * 1234.5}`).join(' | ') + ' |';
  await openDoc(page, ['# Wide', '', header, rule, row, row].join('\n'));
  const wrap = page.locator('#content .rd-table-wrap');
  await expect(wrap).toHaveCount(1);
  const m = await wrap.evaluate(el => ({
    wrapScroll: el.scrollWidth > el.clientWidth,
    page: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    content: document.getElementById('content').scrollWidth - document.getElementById('content').clientWidth,
  }));
  expect(m.wrapScroll).toBe(true);
  expect(m.page).toBeLessThanOrEqual(0);
  expect(m.content).toBeLessThanOrEqual(0);
  await expect(wrap).toHaveClass(/is-scrollable/);
  await expect(wrap).toHaveAttribute('tabindex', '0');
  await expect(page.locator('#content td.rd-num').first()).toHaveCSS('text-align', 'right');
});

test('reading width, typeface and spacing persist across reloads', async ({ page }) => {
  // A desktop-width window so the narrow measure is not already capped by the viewport.
  await page.setViewportSize({ width: 1280, height: 800 });
  await openDoc(page, '# Width\n\n' + 'Measure matters. '.repeat(80));
  const measure = () => page.locator('#content .rd-article p').first().evaluate(el => el.getBoundingClientRect().width);
  const normal = await measure();
  await page.locator('#rd-prefs-btn').click();
  await expect(page.locator('#rd-prefs')).toBeVisible();
  await page.locator('#rd-prefs [data-pref-key="readingWidth"][data-pref-value="narrow"]').click();
  await page.locator('#rd-prefs [data-pref-key="readingFont"][data-pref-value="serif"]').click();
  await page.locator('#rd-prefs [data-pref-key="readingLeading"][data-pref-value="relaxed"]').click();
  await expect(page.locator('body')).toHaveAttribute('data-reading-width', 'narrow');
  expect(await measure()).toBeLessThan(normal);
  await page.keyboard.press('Escape');
  await expect(page.locator('#rd-prefs')).toBeHidden();
  await expect(page.locator('#rd-prefs-btn')).toBeFocused();

  await openDoc(page, '# Width\n\n' + 'Measure matters. '.repeat(80));
  await expect(page.locator('body')).toHaveAttribute('data-reading-width', 'narrow');
  await expect(page.locator('body')).toHaveAttribute('data-reading-font', 'serif');
  await expect(page.locator('body')).toHaveAttribute('data-reading-leading', 'relaxed');
  expect(await measure()).toBeLessThan(normal);
  await page.locator('#rd-prefs-btn').click();
  await expect(page.locator('#rd-prefs [data-pref-value="narrow"]')).toHaveAttribute('aria-checked', 'true');
});

test('footnotes, task lists, definition lists and figures upgrade safely', async ({ page }) => {
  await openDoc(page, [
    '# Extras', '', 'A claim[^a] and another[^b].', '',
    '- [x] done', '- [ ] todo', '',
    'Term', ': Definition text.', '',
    '![Alt caption](/assets/icon-256.png "Figure title")', '',
    '[^a]: First note.', '[^b]: Second <img src=x onerror="window.__rdpwn=1"> note.',
  ].join('\n'));
  await expect(page.locator('#content sup.rd-fnref a')).toHaveCount(2);
  await expect(page.locator('#content .rd-footnotes li')).toHaveCount(2);
  await expect(page.locator('#content .rd-footnotes')).toContainText('First note.');
  expect(await page.evaluate(() => window.__rdpwn)).toBeUndefined();
  await expect(page.locator('#content img[onerror]')).toHaveCount(0);
  await expect(page.locator('#content li.rd-task.is-done')).toHaveCount(1);
  await expect(page.locator('#content dl.rd-dl dt')).toHaveText('Term');
  await expect(page.locator('#content figure.rd-figure figcaption')).toHaveText('Figure title');

  await page.locator('#content sup.rd-fnref a').first().click();
  await expect(page.locator('#content .rd-footnotes li').first()).toBeInViewport();

  await page.locator('#content figure.rd-figure img').click();
  await expect(page.locator('#rd-lightbox-modal')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('#rd-lightbox-modal')).toBeHidden();
});

test('search skips reader chrome and keeps the count contract', async ({ page }) => {
  await openDoc(page, ['# Copy target', '', '```js', 'const copy = 1;', '```', '', 'copy here and copy there'].join('\n'));
  await page.locator('#btn-search').click();
  await page.locator('#search-input').fill('copy');
  await page.keyboard.press('Enter');
  // heading + code + two in the paragraph; the "Copy" button label is chrome.
  await expect(page.locator('#search-count')).toHaveText('1/4');
  await expect(page.locator('#content .rd-code-head mark.hl')).toHaveCount(0);
  await expect(page.locator('#content mark.hl.cur')).toHaveCount(1);
  await page.locator('#search-input').fill('zzzz-none');
  await page.keyboard.press('Enter');
  await expect(page.locator('#search-bar')).toHaveClass(/rd-search-empty/);
});
