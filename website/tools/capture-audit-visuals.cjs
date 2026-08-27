'use strict';

const http = require('http');
const path = require('path');
const fs = require('fs');

function getPlaywright() {
  const candidates = [
    'playwright',
    '../../ui-tests/node_modules/playwright',
    '../ui-tests/node_modules/playwright',
    'T:/Programming/Project/codex/creator/readmd-soe-share-png/ui-tests/node_modules/playwright',
  ];
  for (const c of candidates) {
    try {
      return require(c);
    } catch (e) {}
  }
  throw new Error('Playwright not found');
}

const { chromium } = getPlaywright();

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.webp': 'image/webp',
  '.svg': 'image/svg+xml',
  '.txt': 'text/plain; charset=utf-8',
  '.xml': 'application/xml; charset=utf-8',
};

function createStaticServer(distDir) {
  return http.createServer((req, res) => {
    let reqPath = req.url.split('?')[0];
    if (reqPath.endsWith('/')) reqPath += 'index.html';
    let filePath = path.join(distDir, reqPath);
    if (!fs.existsSync(filePath) && fs.existsSync(filePath + '.html')) {
      filePath += '.html';
    }
    if (!fs.existsSync(filePath)) {
      res.statusCode = 404;
      res.end('Not Found');
      return;
    }
    const ext = path.extname(filePath).toLowerCase();
    res.setHeader('Content-Type', MIME_TYPES[ext] || 'application/octet-stream');
    fs.createReadStream(filePath).pipe(res);
  });
}

(async () => {
  const distDir = path.resolve(__dirname, '../dist');
  const outDir = path.resolve(__dirname, '../../showcase/audit-captures');
  fs.mkdirSync(outDir, { recursive: true });

  const server = createStaticServer(distDir);
  await new Promise((resolve) => server.listen(4173, resolve));
  console.log('Static preview server listening on http://127.0.0.1:4173');

  const browser = await chromium.launch();

  const captures = [
    { name: '01-desktop-home-hero.png', url: '/', width: 1920, height: 1080, scroll: 0 },
    { name: '02-desktop-home-journey.png', url: '/', width: 1920, height: 1080, scroll: 1800 },
    { name: '03-desktop-home-cinema.png', url: '/#capability-cinema', width: 1920, height: 1080, scroll: 6200 },
    { name: '04-desktop-download-en.png', url: '/download/', width: 1920, height: 1080, scroll: 0 },
    { name: '05-desktop-zh-cn-home.png', url: '/zh-cn/', width: 1920, height: 1080, scroll: 0 },
    { name: '06-desktop-zh-cn-download.png', url: '/zh-cn/download/', width: 1920, height: 1080, scroll: 0 },
    { name: '07-laptop-home-hero.png', url: '/', width: 1440, height: 900, scroll: 0 },
    { name: '08-laptop-download-en.png', url: '/download/', width: 1440, height: 900, scroll: 0 },
    { name: '09-mobile-home-hero.png', url: '/', width: 390, height: 844, scroll: 0 },
    { name: '10-mobile-download-en.png', url: '/download/', width: 390, height: 844, scroll: 0 },
    { name: '11-mobile-zh-cn-download.png', url: '/zh-cn/download/', width: 390, height: 844, scroll: 0 },
  ];

  for (const item of captures) {
    const page = await browser.newPage({
      viewport: { width: item.width, height: item.height },
      deviceScaleFactor: 2,
    });
    await page.goto('http://127.0.0.1:4173' + item.url, { waitUntil: 'networkidle' });
    if (item.scroll > 0) {
      await page.evaluate((s) => window.scrollTo(0, s), item.scroll);
      await page.waitForTimeout(400);
    }
    const savePath = path.join(outDir, item.name);
    await page.screenshot({ path: savePath, fullPage: false });
    console.log('Captured:', item.name);
    await page.close();
  }

  await browser.close();
  server.close();
  console.log('Visual audit capture complete!');
})();
