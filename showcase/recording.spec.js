/**
 * Authentic product recording.
 * Capture is delegated to Playwright's supported browser video recorder;
 * Recordly-inspired cursor and frame polish are applied during the scripted run
 * and by the website's scroll-driven composition layer.
 */
const { test, expect } = require('../ui-tests/node_modules/@playwright/test');
const fs = require('fs');
const path = require('path');

const RAW_DIR = path.resolve(__dirname, 'raw');
const OUTPUT = path.join(RAW_DIR, 'product-journey.webm');
const DEMO_MD = fs.readFileSync(path.join(__dirname, 'fixtures/readmd-showcase.md'), 'utf-8');

test.beforeAll(() => {
  fs.mkdirSync(RAW_DIR, { recursive: true });
  if (fs.existsSync(OUTPUT)) fs.unlinkSync(OUTPUT);
});

async function moveCursor(page, x, y) {
  await page.evaluate(({ left, top }) => {
    const cursor = document.getElementById('recordly-cursor');
    if (cursor) {
      cursor.style.transform = `translate3d(${left}px, ${top}px, 0)`;
    }
  }, { left: x, top: y });
  await page.mouse.move(x, y, { steps: 18 });
}

async function polishedClick(page, selector) {
  const locator = page.locator(selector).first();
  await locator.scrollIntoViewIfNeeded();
  const box = await locator.boundingBox();
  if (!box) throw new Error(`Missing bounding box for ${selector}`);
  await moveCursor(page, box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
}

async function openDemo(page) {
  await page.evaluate((content) => {
    if (typeof window._t !== 'function') {
      window._t = (key, params) => (window.i18n ? window.i18n.t(key, params) : key);
    }
    return renderVirtual('virtual', 'ReadMD 研究笔记.md', '', content);
  }, DEMO_MD);
  await expect(page.locator('#content h1')).toBeVisible();
  await expect(page.locator('#content mjx-container').first()).toBeVisible();
}

test('record authentic ReadMD product journey', async ({ browser }, testInfo) => {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 810 },
    deviceScaleFactor: 2,
    locale: 'zh-CN',
    recordVideo: {
      dir: testInfo.outputDir,
      size: { width: 1440, height: 810 },
      type: 'webm',
    },
  });
  const page = await context.newPage();
  await page.addInitScript(() => {
    try {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark' }));
    } catch (error) {}

  document.addEventListener('DOMContentLoaded', () => {
      const style = document.createElement('style');
      style.textContent = `
        html, body { background: #101116 !important; color: #f2f2f5 !important; }
        #recordly-cursor {
          position: fixed; z-index: 2147483000; left: 0; top: 0;
          width: 17px; height: 17px; border-radius: 50%;
          background: rgba(255, 255, 255, 0.94);
          border: 2px solid #0a72e8;
          box-shadow: 0 3px 14px rgba(0, 0, 0, 0.35);
          pointer-events: none; transition: transform 180ms cubic-bezier(.16,1,.3,1);
        }
        #recordly-cursor.clicking { transform-origin: center; scale: 0.82; }
      `;
      document.head.append(style);
      const cursor = document.createElement('div');
      cursor.id = 'recordly-cursor';
      document.body.append(cursor);
    });
  }, );

  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.waitForTimeout(700);

  await openDemo(page);
  await page.waitForTimeout(900);

  await page.evaluate(() => toggleSide('toc'));
  await expect(page.locator('#toc-list a:first-child')).toBeVisible();
  await page.waitForTimeout(850);

  await page.evaluate(() => {
    document.querySelector('#content .katex-display')?.scrollIntoView({ behavior: 'smooth', block: 'center' });
  });
  await page.waitForTimeout(1000);

  await page.evaluate(async () => {
    await toggleEdit();
    setPvLayout('left');
  });
  await expect(page.locator('#edit-bar')).toBeVisible();
  await page.waitForTimeout(1000);

  await page.evaluate(() => {
    document.querySelector('#content .code-chunk-card')?.scrollIntoView({ behavior: 'smooth', block: 'center' });
  });
  await page.waitForTimeout(950);

  await page.evaluate(() => openTableModal());
  await expect(page.locator('#table-modal')).toBeVisible();
  await expect(page.locator('.table-grid-cell')).toHaveCount(100);
  await page.waitForTimeout(1000);
  await page.evaluate(() => closeTableModal());
  await expect(page.locator('#table-modal')).toBeHidden();

  await page.evaluate(() => launchPresentationMode());
  const frame = page.frameLocator('.presentation-iframe');
  await frame.locator('.reveal').first().waitFor({ state: 'visible', timeout: 15_000 });
  await page.waitForTimeout(1600);
  await polishedClick(page, '#presentation-close-btn');
  await expect(page.locator('#presentation-modal')).toBeHidden();

  await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'smooth' }));
  await page.waitForTimeout(800);

  await page.close();
  await context.close();
  await page.video().saveAs(OUTPUT);
});
