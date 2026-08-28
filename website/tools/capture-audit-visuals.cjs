'use strict';

const http = require('http');
const path = require('path');
const fs = require('fs');

function getPlaywright() {
  const candidates = [
    'playwright',
    path.resolve(__dirname, '../../ui-tests/node_modules/playwright'),
    path.resolve(__dirname, '../ui-tests/node_modules/playwright'),
    path.resolve(__dirname, '../../../readmd-soe-share-png/ui-tests/node_modules/playwright'),
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
    { name: '01-desktop-home-hero.png', url: '/', width: 1920, height: 1080, scroll: 0, colorScheme: 'light' },
    { name: '02-desktop-home-journey-0.png', url: '/', width: 1920, height: 1080, scroll: 1200, colorScheme: 'light' },
    { name: '02-desktop-home-journey-50.png', url: '/', width: 1920, height: 1080, scroll: 2600, colorScheme: 'light' },
    { name: '03-desktop-home-cinema.png', url: '/#capability-cinema', width: 1920, height: 1080, scroll: 6200, colorScheme: 'light' },
    { name: '04-desktop-download-light.png', url: '/download/', width: 1920, height: 1080, scroll: 0, colorScheme: 'light' },
    { name: '04-desktop-download-dark.png', url: '/download/', width: 1920, height: 1080, scroll: 0, colorScheme: 'dark' },
    { name: '05-desktop-download-mcp-guide.png', url: '/zh-cn/download/', width: 1920, height: 1080, scroll: 650, colorScheme: 'dark' },
    { name: '05-desktop-download-mcp-terminal.png', url: '/zh-cn/download/', width: 1920, height: 1080, scroll: 1250, colorScheme: 'dark' },
    { name: '06-desktop-zh-cn-download-dark.png', url: '/zh-cn/download/', width: 1920, height: 1080, scroll: 0, colorScheme: 'dark' },
    { name: '07-laptop-home-hero.png', url: '/', width: 1440, height: 900, scroll: 0, colorScheme: 'light' },
    { name: '08-laptop-download-dark.png', url: '/download/', width: 1440, height: 900, scroll: 0, colorScheme: 'dark' },
    { name: '09-mobile-home-hero.png', url: '/', width: 390, height: 844, scroll: 0, colorScheme: 'light' },
    { name: '10-mobile-download-dark.png', url: '/download/', width: 390, height: 844, scroll: 0, colorScheme: 'dark' },
    { name: '11-mobile-mcp-guide.png', url: '/zh-cn/download/#mcp-guide', width: 390, height: 844, scroll: 1450, colorScheme: 'dark' },
  ];

  for (const item of captures) {
    const page = await browser.newPage({
      viewport: { width: item.width, height: item.height },
      deviceScaleFactor: 2,
      colorScheme: item.colorScheme || 'light',
    });

    page.on('console', (msg) => {
      if (msg.type() === 'error' || msg.type() === 'warning') {
        console.log(`[Browser Console ${msg.type()}]:`, msg.text());
      }
    });
    page.on('pageerror', (err) => console.log('[Browser Error]:', err.message));

    await page.goto('http://127.0.0.1:4173' + item.url, { waitUntil: 'networkidle' });
    if (item.scroll > 0) {
      await page.evaluate((s) => window.scrollTo(0, s), item.scroll);
      await page.waitForTimeout(600);
    } else {
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
