const { test, expect } = require('@playwright/test');

const DOC_A = 'C:/conv-test/a.docx';
const DOC_B = 'C:/conv-test/b.pdf';
const IMG_C = 'C:/conv-test/c.png';

async function waitForApp(page) {
  await page.goto('/');
  await page.waitForFunction(() => typeof openBatchModal === 'function');
}

async function mockModulesReady(page) {
  await page.route('**/api/modules', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ modules: { convert: 'ready', ocr: 'ready', web: 'ready', ai: 'ready' }, win7: false }),
  }));
  await page.route('**/api/modules/load', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: true }),
  }));
}

function batchRows(page) {
  return page.locator('#batch-list .batch-item');
}

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'zh-CN'));
});

test('batch workbench opens from the more menu with file and folder entry points', async ({ page }) => {
  await waitForApp(page);
  await page.locator('#btn-more').click();
  await expect(page.locator('#more-menu')).toHaveClass(/open/);
  await page.locator('#btn-batch').click();

  await expect(page.locator('#batch-modal')).toBeVisible();
  await expect(page.locator('#batch-title')).toContainText('批量工作台');
  await expect(page.locator('#batch-files')).toBeVisible();
  await expect(page.locator('#batch-folder')).toBeVisible();
  await expect(page.locator('#batch-list .batch-item')).toHaveCount(0);
});

test('mixed enqueue routes docs into one batch job and images into sequential OCR', async ({ page }) => {
  await mockModulesReady(page);
  let batchBody = null;
  let ocrPaths = [];
  await page.route('**/api/convert/batch', route => {
    batchBody = JSON.parse(route.request().postData() || '{}');
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ job: 'j-mix', total: 2 }),
    });
  });
  await page.route('**/api/convert/progress*', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      running: false, finished: true, done: 2, total: 2,
      items: [
        { src: DOC_A, status: 'ok', out: 'C:/conv-test/a.md', warns: [] },
        { src: DOC_B, status: 'ok', out: 'C:/conv-test/b.md', warns: [] },
      ],
    }),
  }));
  await page.route('**/api/ocr*', route => {
    ocrPaths.push(route.request().url());
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ name: 'c', dir: 'C:/conv-test', content: '# 图中文字', fixes: [] }),
    });
  });

  await waitForApp(page);
  await page.evaluate(paths => enqueueBatchFiles(paths, false), [DOC_A, DOC_B, IMG_C]);

  await expect(page.locator('#batch-modal')).toBeVisible();
  await expect(batchRows(page)).toHaveCount(3);
  await expect(batchRows(page).nth(0)).toContainText('a.docx');
  await expect(batchRows(page).nth(1)).toContainText('b.pdf');
  await expect(batchRows(page).nth(2)).toContainText('c.png');

  await expect.poll(() => batchBody, { timeout: 10000 }).not.toBeNull();
  expect(batchBody.paths).toEqual([DOC_A, DOC_B]);
  await expect.poll(() => ocrPaths.length, { timeout: 10000 }).toBe(1);
  expect(ocrPaths[0]).toContain(encodeURIComponent(IMG_C));

  await expect(batchRows(page).nth(0)).toContainText('成功', { timeout: 10000 });
  await expect(batchRows(page).nth(1)).toContainText('成功', { timeout: 10000 });
  await expect(batchRows(page).nth(2)).toContainText('成功', { timeout: 10000 });
  await expect(page.locator('#batch-status')).toContainText('完成');
});

test('cancel stops the docs job via the cancel endpoint and marks rows canceled', async ({ page }) => {
  await mockModulesReady(page);
  let cancelSeen = false;
  let cancelBody = null;
  await page.route('**/api/convert/cancel', route => {
    cancelSeen = true;
    cancelBody = JSON.parse(route.request().postData() || '{}');
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ ok: true, job: cancelBody.job }),
    });
  });
  await page.route('**/api/convert/batch', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ job: 'j-cancel', total: 2 }),
  }));
  await page.route('**/api/convert/progress*', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify(
      cancelSeen
        ? {
            running: false, finished: true, done: 2, total: 2,
            items: [
              { src: DOC_A, status: 'canceled' },
              { src: DOC_B, status: 'canceled' },
            ],
          }
        : {
            running: true, finished: false, done: 0, total: 2,
            items: [
              { src: DOC_A, status: 'queued' },
              { src: DOC_B, status: 'queued' },
            ],
          },
    ),
  }));

  await waitForApp(page);
  await page.evaluate(paths => enqueueBatchFiles(paths, false), [DOC_A, DOC_B]);
  await expect(page.locator('#batch-cancel')).toBeVisible({ timeout: 10000 });
  await page.locator('#batch-cancel').click();

  await expect.poll(() => cancelSeen, { timeout: 10000 }).toBe(true);
  expect(cancelBody.job).toBe('j-cancel');
  await expect(batchRows(page).nth(0)).toContainText('已取消', { timeout: 10000 });
  await expect(batchRows(page).nth(1)).toContainText('已取消', { timeout: 10000 });
});

test('cancel during the OCR lane stops before the remaining images', async ({ page }) => {
  await mockModulesReady(page);
  let ocrCalls = 0;
  let cancelClicked = false;
  await page.route('**/api/ocr*', route => {
    ocrCalls++;
    if (ocrCalls === 1) {
      const t0 = Date.now();
      const waitAndAnswer = () => {
        if (cancelClicked || Date.now() - t0 > 5000) {
          route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ name: 'c', dir: 'C:/conv-test', content: '# 图中文字', fixes: [] }),
          });
        } else {
          setTimeout(waitAndAnswer, 50);
        }
      };
      waitAndAnswer();
    } else {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ name: 'd', dir: 'C:/conv-test', content: '# 第二张', fixes: [] }),
      });
    }
  });

  await waitForApp(page);
  await page.evaluate(paths => enqueueBatchFiles(paths, false), [IMG_C, 'C:/conv-test/d.jpg']);
  await expect(page.locator('#batch-cancel')).toBeVisible({ timeout: 10000 });
  await page.locator('#batch-cancel').click();
  cancelClicked = true;

  await expect(batchRows(page).nth(0)).toContainText('成功', { timeout: 10000 });
  await expect(batchRows(page).nth(1)).toContainText('已取消', { timeout: 10000 });
  await expect.poll(() => ocrCalls).toBe(1);
});

test('failures do not stop the queue and stay traceable per row', async ({ page }) => {
  await mockModulesReady(page);
  await page.route('**/api/convert/batch', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ job: 'j-fail', total: 2 }),
  }));
  await page.route('**/api/convert/progress*', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      running: false, finished: true, done: 2, total: 2,
      items: [
        { src: DOC_A, status: 'error', error: '解析失败：损坏的 docx' },
        { src: DOC_B, status: 'ok', out: 'C:/conv-test/b.md', warns: [] },
      ],
    }),
  }));
  await page.route('**/api/ocr*', route => route.fulfill({
    status: 500,
    contentType: 'application/json',
    body: JSON.stringify({ error: 'OCR 引擎不可用' }),
  }));

  await waitForApp(page);
  await page.evaluate(paths => enqueueBatchFiles(paths, false), [DOC_A, DOC_B, IMG_C]);

  await expect(batchRows(page).nth(0)).toContainText('失败', { timeout: 10000 });
  await expect(batchRows(page).nth(1)).toContainText('成功', { timeout: 10000 });
  await expect(batchRows(page).nth(2)).toContainText('失败', { timeout: 10000 });
  await expect(page.locator('#batch-status')).toContainText('完成');
  await expect(page.locator('#batch-status')).toContainText('成功 1');
});
