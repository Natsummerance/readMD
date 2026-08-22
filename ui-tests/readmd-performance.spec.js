const { test, expect } = require('@playwright/test');
const fs = require('fs/promises');
const os = require('os');
const path = require('path');

const CORPUS_SIZES = [1000, 10000, 50000];
let corpusDir;

function corpusDocument(lineCount) {
  const targetSection = Math.max(1, Math.floor((((lineCount - 1) / 2) * 0.75)));
  const lines = ['# Long document performance'];
  for (let section = 1; lines.length + 2 <= lineCount; section += 1) {
    lines.push(`## Section ${section}`);
    lines.push(section === targetSection
      ? 'This paragraph contains READMD_PERF_UNIQUE_ANCHOR_7351.'
      : `Stable paragraph ${section} keeps pagination work bounded.`);
  }
  while (lines.length < lineCount) lines.push('');
  return `${lines.join('\n')}\n`;
}

async function openCorpus(page, size) {
  const file = path.join(corpusDir, `readmd-${size}.md`);
  const started = Date.now();
  await page.goto(`/?file=${encodeURIComponent(file)}`);
  await page.waitForFunction(expected => state.file === expected && state.original.length > 0, file);
  await expect(page.locator('#content .markdown-body h1')).toContainText('Long document performance');
  if (size > 8000) {
    await page.waitForFunction(() => state.pagination.enabled && state.pagination.totalPages > 1);
    await expect(page.locator('#pg-total-label')).toHaveText(/\/ \d+/);
    await expect(page.locator('#pagination-bar')).toBeVisible();
  }
  return { firstReadableMs: Date.now() - started, file };
}

test.beforeAll(async () => {
  corpusDir = await fs.mkdtemp(path.join(os.tmpdir(), 'readmd-perf-'));
  await Promise.all(CORPUS_SIZES.map(async size => {
    await fs.writeFile(path.join(corpusDir, `readmd-${size}.md`), corpusDocument(size), 'utf8');
  }));
});

test.afterAll(async () => {
  if (corpusDir) await fs.rm(corpusDir, { recursive: true, force: true });
});

test('long-document interaction stays bounded from 1k through 50k lines', async ({ page }) => {
  test.setTimeout(90_000);
  const results = {};

  for (const size of CORPUS_SIZES) {
    results[size] = await openCorpus(page, size);
    if (size === 1000) continue;

    const targetSection = Math.ceil(size * 0.375);
    const expectedPage = Math.floor((targetSection - 1) / 300);

    let started = Date.now();
    await page.locator('#pg-next-btn').click();
    await page.waitForFunction(() => state.pagination.currentPage === 1);
    results[size].pageTurnMs = Date.now() - started;

    started = Date.now();
    await page.locator('#btn-search').click();
    await expect(page.locator('#search-input')).toBeVisible();
    await page.locator('#search-input').fill('READMD_PERF_UNIQUE_ANCHOR_7351');
    await page.locator('#search-input').press('Enter');
    await expect(page.locator('#search-count')).toHaveText('1/1 (P.' + (expectedPage + 1) + ')');
    await page.waitForFunction(expected => state.pagination.currentPage === expected, expectedPage);
    results[size].searchJumpMs = Date.now() - started;

    const tocSection = Math.max(1, targetSection - 300);
    const expectedTocPage = Math.floor((tocSection - 1) / 300);
    started = Date.now();
    await page.locator('#btn-toc').click();
    await expect(page.locator('#toc-list')).toBeVisible();
    await page.locator(`#toc-list details[data-page-idx="${expectedTocPage}"] > summary`).click();
    await page.locator(`#toc-list [data-heading-id="section-${tocSection}"]`).click();
    await page.waitForFunction(expected => state.pagination.currentPage === expected, expectedTocPage);
    await expect(page.locator(`#content [id="section-${tocSection}"]`)).toBeFocused();
    await expect(page.locator(`#toc-list [data-heading-id="section-${tocSection}"]`)).toHaveClass(/toc-heading-active/);
    results[size].tocJumpMs = Date.now() - started;

    started = Date.now();
    await page.locator('#search-input').fill('');
    await expect(page.locator('#search-count')).toHaveText('');
    await page.locator('#search-input').fill('READMD_PERF_UNIQUE_ANCHOR_7351');
    await page.locator('#search-input').press('Enter');
    await expect(page.locator('#search-count')).toHaveText('1/1 (P.' + (expectedPage + 1) + ')');
    results[size].repeatSearchMs = Date.now() - started;

    results[size].activeDomNodes = await page.evaluate(() => document.querySelectorAll('*').length);

    results[size].anchorSurvivesEdit = await page.evaluate(() => {
      const source = state.pagination.rawContent;
      const before = splitMdIntoPages(source).find(page => page.content.includes('## Section 100'));
      const edited = source.replace(
        'Stable paragraph 100 keeps pagination work bounded.',
        'Edited paragraph 100 preserves heading anchors.'
      );
      const after = splitMdIntoPages(edited).find(page => page.content.includes('## Section 100'));
      return Boolean(before && after && before.pageIndex === after.pageIndex);
    });
  }

  for (const size of CORPUS_SIZES) {
    expect(results[size].firstReadableMs, `${size}-line first readable content`).toBeLessThan(2500);
  }
  expect(results[10000].pageTurnMs).toBeLessThan(250);
  expect(results[50000].pageTurnMs).toBeLessThan(250);
  expect(results[10000].searchJumpMs).toBeLessThan(1250);
  expect(results[50000].searchJumpMs).toBeLessThan(1250);
  expect(results[10000].tocJumpMs).toBeLessThan(750);
  expect(results[50000].tocJumpMs).toBeLessThan(750);
  expect(results[10000].repeatSearchMs).toBeLessThan(750);
  expect(results[50000].repeatSearchMs).toBeLessThan(750);
  expect(results[50000].activeDomNodes / results[10000].activeDomNodes).toBeLessThan(2);
  expect(results[50000].anchorSurvivesEdit).toBe(true);
});
