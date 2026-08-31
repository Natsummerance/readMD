'use strict';

const fs = require('fs');
const http = require('http');
const path = require('path');

function getPlaywright() {
  for (const candidate of ['playwright', '../../ui-tests/node_modules/playwright', '../ui-tests/node_modules/playwright']) {
    try {
      return require(candidate);
    } catch (_error) {}
  }
  throw new Error('Playwright not found');
}

const { chromium } = getPlaywright();
let baseUrl = process.argv[2] || '';
const routes = ['/download/', '/zh-cn/download/', '/zh-tw/download/', '/ja/download/'];

async function startStaticServer() {
  const root = path.resolve(__dirname, '..', 'dist');
  const server = http.createServer((request, response) => {
    const pathname = decodeURIComponent(new URL(request.url, 'http://127.0.0.1').pathname);
    const relative = pathname.replace(/^\/+/, '');
    let target = path.resolve(root, relative);
    if (!target.startsWith(root + path.sep) && target !== root) {
      response.writeHead(403).end();
      return;
    }
    if (pathname.endsWith('/')) target = path.join(target, 'index.html');
    fs.readFile(target, (error, data) => {
      if (error) {
        response.writeHead(404).end();
        return;
      }
      const type = target.endsWith('.css') ? 'text/css' : target.endsWith('.js') ? 'text/javascript' : 'text/html';
      response.writeHead(200, { 'Content-Type': `${type}; charset=utf-8` });
      response.end(data);
    });
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  baseUrl = `http://127.0.0.1:${address.port}`;
  return server;
}

function parseColor(value) {
  const color = String(value || '').trim().toLowerCase();
  if (/^#[0-9a-f]{3}$/.test(color)) {
    return { rgb: [...color.slice(1)].map(channel => parseInt(channel + channel, 16) / 255), alpha: 1 };
  }
  if (/^#[0-9a-f]{6}$/.test(color)) {
    return { rgb: [1, 3, 5].map(index => parseInt(color.slice(index, index + 2), 16) / 255), alpha: 1 };
  }
  if (color.startsWith('color(srgb')) {
    const values = color.slice('color(srgb'.length).match(/[\d.]+/g)?.map(Number) || [];
    return values.length >= 3 ? { rgb: values.slice(0, 3), alpha: values[3] ?? 1 } : null;
  }
  const values = color.match(/[\d.]+/g)?.map(Number) || [];
  return values.length >= 3 ? { rgb: values.slice(0, 3).map(channel => channel / 255), alpha: values[3] ?? 1 } : null;
}

function luminance(value, backdropValue = null) {
  const parsed = parseColor(value);
  if (!parsed) return null;
  const backdrop = parseColor(backdropValue);
  const rgb = parsed.alpha < 1 && backdrop
    ? parsed.rgb.map((channel, index) => channel * parsed.alpha + backdrop.rgb[index] * (1 - parsed.alpha))
    : parsed.rgb;
  const linear = rgb.map(channel => channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4);
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrast(foreground, background) {
  const first = luminance(foreground);
  const second = luminance(background);
  if (first === null || second === null) return 0;
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

(async () => {
  const server = baseUrl ? null : await startStaticServer();
  const browser = await chromium.launch();
  const results = [];
  let failed = false;

  for (const route of routes) {
    for (const scheme of ['light', 'dark']) {
      const page = await browser.newPage({ colorScheme: scheme, viewport: { width: 1440, height: 900 } });
      await page.goto(baseUrl + route, { waitUntil: 'networkidle' });
      const evidence = await page.evaluate(() => {
        const guide = document.querySelector('#mcp-guide');
        const card = document.querySelector('.apple-platform-card');
        const terminal = document.querySelector('.macos-terminal');
        if (!guide || !card || !terminal) return null;
        const guideStyle = getComputedStyle(guide);
        return {
          panel: guideStyle.getPropertyValue('--mcp-panel').trim(),
          text: guideStyle.getPropertyValue('--mcp-text').trim(),
          card: getComputedStyle(card).backgroundColor,
          terminal: getComputedStyle(terminal).backgroundColor,
        };
      });

      const panelLight = luminance(evidence?.panel);
      const cardLight = luminance(evidence?.card, evidence?.panel);
      const terminalLight = luminance(evidence?.terminal, evidence?.panel);
      const textContrast = evidence ? contrast(evidence.text, evidence.panel) : 0;
      const expected = scheme === 'light'
        ? panelLight > 0.8 && cardLight > 0.8 && terminalLight > 0.8
        : panelLight < 0.05 && cardLight < 0.08 && terminalLight < 0.08;
      const passed = Boolean(evidence) && expected && textContrast >= 4.5;
      failed ||= !passed;
      results.push({ route, scheme, passed, panelLight, cardLight, terminalLight, textContrast, evidence });
      await page.close();
    }
  }

  await browser.close();
  if (server) await new Promise(resolve => server.close(resolve));
  process.stdout.write(`${JSON.stringify(results, null, 2)}\n`);
  if (failed) process.exitCode = 1;
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
