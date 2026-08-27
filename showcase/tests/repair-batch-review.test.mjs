import assert from 'node:assert/strict';
import { after, before, describe, it } from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('../../ui-tests/node_modules/@playwright/test');

describe('repair batch review layout', () => {
  let browser;
  let context;
  let page;
  const url = new URL('../reports/v2.3.7-beta-repair-batch-review.html', import.meta.url).href;

  before(async () => {
    browser = await chromium.launch();
    context = await browser.newContext({ locale: 'zh-CN' });
    page = await context.newPage();
  });

  after(async () => {
    await context?.close();
    await browser?.close();
  });

  async function auditLayout() {
    return page.evaluate(() => {
      const documentElement = document.documentElement;
      const visibleText = [...document.querySelectorAll('h1,h2,h3,p,li,td,dd')]
        .filter(element => element.innerText?.trim());
      const clipped = visibleText.filter(element => {
        const style = getComputedStyle(element);
        return (
          style.overflowY === 'hidden'
          && element.scrollHeight > element.clientHeight + 2
        );
      }).map(element => element.tagName);
      return {
        horizontal_overflow: documentElement.scrollWidth > documentElement.clientWidth + 1,
        document_width: documentElement.clientWidth,
        content_width: documentElement.scrollWidth,
        heading_count: document.querySelectorAll('h1,h2,h3').length,
        package_count: document.querySelectorAll('article').length,
        script_count: document.querySelectorAll('script').length,
        image_count: document.querySelectorAll('img').length,
        clipped_tags: clipped,
        title: document.querySelector('h1')?.innerText.trim(),
        body_length: document.body.innerText.length,
      };
    });
  }

  async function waitForLayout() {
    let layout;
    for (let attempt = 0; attempt < 20; attempt += 1) {
      layout = await auditLayout();
      if (!layout.horizontal_overflow && !layout.clipped_tags.length) return layout;
      await page.waitForTimeout(100);
    }
    return layout;
  }

  it('keeps evidence readable without scripts or images on desktop', async () => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto(url);
    const initial = await waitForLayout();
    assert.equal(initial.horizontal_overflow, false);
    assert.equal(initial.package_count, 3);
    assert.equal(initial.script_count, 0);
    assert.equal(initial.image_count, 0);
    assert.deepEqual(initial.clipped_tags, []);
    const layout = await auditLayout();
    assert.equal(layout.title, '三版本修复批次审查');
    assert.ok(layout.heading_count >= 10);
    assert.ok(layout.body_length > 3000);
  });

  it('keeps the operator brief usable on mobile', async () => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(url);
    const layout = await waitForLayout();
    assert.equal(layout.horizontal_overflow, false);
    assert.equal(layout.package_count, 3);
    assert.equal(layout.script_count, 0);
    assert.equal(layout.image_count, 0);
    assert.deepEqual(layout.clipped_tags, []);
    const heading = page.locator('h1');
    const box = await heading.boundingBox();
    assert.ok(box);
    assert.ok(box.x >= -1);
    assert.ok(box.width <= 391);
    assert.match(await heading.innerText(), /三版本修复批次审查/);
  });
});
