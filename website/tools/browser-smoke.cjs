'use strict';

const { chromium } = require('../../ui-tests/node_modules/playwright');

const baseUrl = process.argv[2] || 'http://127.0.0.1:4173';
const routes = ['/', '/workflows/', '/zh-cn/', '/zh-cn/workflows/', '/zh-tw/', '/zh-tw/workflows/', '/ja/', '/ja/workflows/'];
const aiFiles = [
  '/llms.txt',
  '/llms-full.txt',
  '/zh-cn/llms.txt',
  '/zh-tw/llms.txt',
  '/ja/llms.txt',
  '/robots.txt',
  '/sitemap.xml',
  '/zh-cn/llms-full.txt',
  '/zh-tw/llms-full.txt',
  '/ja/llms-full.txt',
];

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  const failedRequests = [];
  page.on('pageerror', error => errors.push(String(error)));
  page.on('console', message => {
    if (message.type() === 'error') errors.push(message.text());
  });
  page.on('requestfailed', request => failedRequests.push(request.url()));

  const pages = [];
  for (const route of routes) {
    await page.goto(baseUrl + route, { waitUntil: 'load' });
    await page.evaluate(async () => {
      await new Promise(resolve => {
        let position = 0;
        const step = () => {
          position += 600;
          window.scrollTo(0, position);
          if (position >= document.body.scrollHeight) resolve();
          else setTimeout(step, 30);
        };
        step();
      });
    });
    await page.waitForLoadState('networkidle');
    pages.push({
      route,
      ...(await page.evaluate(() => ({
        title: document.title,
        h1Count: document.querySelectorAll('h1').length,
        canonical: document.querySelector('link[rel="canonical"]')?.href || '',
        ogUrl: document.querySelector('meta[property="og:url"]')?.content || '',
        robots: document.querySelector('meta[name="robots"]')?.content || '',
        stylesheetLoaded: [...document.styleSheets].some(sheet => (sheet.href || '').includes('/assets/site.css')),
        brokenImages: [...document.images].filter(image => !image.complete || image.naturalWidth === 0).length,
        pictureCount: document.querySelectorAll('picture').length,
        webpImages: [...document.querySelectorAll('picture img')].filter(image => image.currentSrc.endsWith('.webp')).length,
      }))),
    });
  }

  for (const file of aiFiles) {
    const response = await page.request.get(baseUrl + file);
    if (!response.ok()) errors.push(`${file} returned HTTP ${response.status()}`);
  }

  const rootResponse = await page.request.get(baseUrl + '/');
  const security = {
    csp: rootResponse.headers()['content-security-policy'] || '',
    hsts: rootResponse.headers()['strict-transport-security'] || '',
  };

  const failures = [];
  for (const item of pages) {
    if (!item.title.includes('ReadMD')) failures.push(`${item.route}: missing ReadMD title`);
    if (item.h1Count !== 1) failures.push(`${item.route}: expected one h1`);
    if (item.canonical !== item.ogUrl) failures.push(`${item.route}: canonical and og:url differ`);
    if (item.robots !== 'index,follow,max-image-preview:large') failures.push(`${item.route}: bad robots directive`);
    if (!item.stylesheetLoaded) failures.push(`${item.route}: production stylesheet did not load`);
    if (item.brokenImages) failures.push(`${item.route}: ${item.brokenImages} broken images`);
    if (item.pictureCount !== item.webpImages) failures.push(`${item.route}: expected every picture to select WebP`);
  }
  if (process.env.CHECK_HTTP_HEADERS === '1') {
    if (!security.csp.includes("script-src 'self'")) failures.push('missing script CSP');
    if (!security.hsts) failures.push('missing HSTS');
  }
  failures.push(...errors, ...failedRequests);

  console.log(JSON.stringify({ ok: failures.length === 0, pages, security, failures }, null, 2));
  await browser.close();
  process.exitCode = failures.length ? 1 : 0;
})().catch(error => {
  console.error(error);
  process.exit(1);
});
