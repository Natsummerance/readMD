const { test, expect } = require('@playwright/test');

function median(values) {
  return [...values].sort((left, right) => left - right)[Math.floor(values.length / 2)];
}

test('welcome startup stays lightweight and interactive below one second', async ({ browser }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'Startup budget is measured once on the desktop project.');
  const samples = [];

  for (let index = 0; index < 3; index += 1) {
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    const page = await context.newPage();
    await page.route('**/api/update/check', route => route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ ok: false }),
    }));
    await page.addInitScript(() => localStorage.setItem('readmd_language', 'zh-CN'));

    let initialRequests = [];
    page.on('request', request => {
      if (request.url().startsWith('http://')) initialRequests.push(request.url());
    });

    await page.goto('/', { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => window.__readmdAppReady === true, null, { timeout: 5000 });
    const metrics = await page.evaluate(() => {
      const navigation = performance.getEntriesByType('navigation')[0];
      const paint = performance.getEntriesByType('paint').find(entry => entry.name === 'first-contentful-paint');
      const resources = performance.getEntriesByType('resource');
      return {
        ready: performance.now(),
        domContentLoaded: navigation.domContentLoadedEventEnd,
        load: navigation.loadEventEnd,
        firstContentfulPaint: paint?.startTime ?? Number.POSITIVE_INFINITY,
        transferredBytes: resources.reduce((total, entry) => total + (entry.decodedBodySize || entry.transferSize || 0), 0),
        requestCount: resources.length,
      };
    });
    metrics.initialRequests = initialRequests;
    samples.push(metrics);
    await context.close();
  }

  const ready = median(samples.map(sample => sample.ready));
  const firstContentfulPaint = median(samples.map(sample => sample.firstContentfulPaint));
  const transferredBytes = Math.max(...samples.map(sample => sample.transferredBytes));
  expect(ready).toBeLessThan(900);
  expect(firstContentfulPaint).toBeLessThan(400);
  expect(transferredBytes).toBeLessThan(950_000);
  expect(Math.max(...samples.map(sample => sample.requestCount))).toBeLessThanOrEqual(31);
  for (const sample of samples) {
    expect(sample.initialRequests.some(url => url.includes('/vendor/qrcode.min.js'))).toBe(false);
    expect(sample.initialRequests.some(url => url.includes('cdn.jsdelivr'))).toBe(false);
  }
});
