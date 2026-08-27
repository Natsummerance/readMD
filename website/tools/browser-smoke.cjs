'use strict';

const { chromium } = require('../../ui-tests/node_modules/playwright');

const baseUrl = process.argv[2] || 'http://127.0.0.1:4173';
const routes = ['/', '/download/', '/workflows/', '/large-markdown-files/', '/markdown-to-slides/', '/convert-to-markdown/', '/pdf-to-markdown/', '/markdown-tables/', '/release-notes/', '/scan-to-markdown/', '/bibtex-citations/', '/zh-cn/', '/zh-cn/download/', '/zh-cn/workflows/', '/zh-cn/large-markdown-files/', '/zh-cn/markdown-to-slides/', '/zh-cn/convert-to-markdown/', '/zh-cn/pdf-to-markdown/', '/zh-cn/markdown-tables/', '/zh-cn/release-notes/', '/zh-cn/scan-to-markdown/', '/zh-cn/bibtex-citations/', '/zh-tw/', '/zh-tw/download/', '/zh-tw/workflows/', '/zh-tw/large-markdown-files/', '/zh-tw/markdown-to-slides/', '/zh-tw/convert-to-markdown/', '/zh-tw/pdf-to-markdown/', '/zh-tw/markdown-tables/', '/zh-tw/release-notes/', '/zh-tw/scan-to-markdown/', '/zh-tw/bibtex-citations/', '/ja/', '/ja/download/', '/ja/workflows/', '/ja/large-markdown-files/', '/ja/markdown-to-slides/', '/ja/convert-to-markdown/', '/ja/pdf-to-markdown/', '/ja/markdown-tables/', '/ja/release-notes/', '/ja/scan-to-markdown/', '/ja/bibtex-citations/'];
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
  page.on('requestfailed', request => {
    if (request.url().includes('/cdn-cgi/rum')) return;
    failedRequests.push(request.url());
  });

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
        ogImage: document.querySelector('meta[property="og:image"]')?.content || '',
        twitterImage: document.querySelector('meta[name="twitter:image"]')?.content || '',
        robots: document.querySelector('meta[name="robots"]')?.content || '',
        stylesheetLoaded: [...document.styleSheets].some(sheet => (sheet.href || '').includes('/assets/site.css')),
        brokenImages: [...document.images].filter(image => !image.complete || image.naturalWidth === 0).length,
        pictureCount: document.querySelectorAll('picture').length,
        webpImages: [...document.querySelectorAll('picture img')].filter(image => image.currentSrc.endsWith('.webp')).length,
        heroPreloaded: [...document.querySelectorAll('link[rel="preload"]')].some(link => link.href.endsWith('/media/overview-reader.webp')),
        shareLinks: document.querySelectorAll('#share a[href*="twitter.com"], #share a[href*="t.me"], #share a[href*="linkedin.com"]').length,
        breadcrumb: !!document.querySelector('nav[aria-label="Breadcrumb"]'),
        breadcrumbPaths: [...document.querySelectorAll('nav[aria-label="Breadcrumb"] a')].map(link => new URL(link.href).pathname),
        faviconLinked: !!document.querySelector('link[rel="icon"][href="/assets/icon-256.png"]'),
        manifestLinked: !!document.querySelector('link[rel="manifest"][href="/site.webmanifest"]'),
        releaseFeedLinked: !!document.querySelector('link[type="application/atom+xml"][href$="releases.atom"]'),
        siteScriptLoaded: [...document.scripts].some(script => (script.src || '').endsWith('/assets/site.js')),
        particleField: !!document.querySelector('.particle-field'),
        capabilityCanvas: !!document.querySelector('.capability-canvas'),
        capabilityProgress: document.querySelector('.capability-canvas')?.dataset.syncProgress || '',
        capabilityTrack: getComputedStyle(document.querySelector('[data-capability-track]') || document.body).transform,
        capabilityPanels: document.querySelectorAll('.capability-panel').length,
        journeyCanvas: !!document.querySelector('#journey-film'),
      }))),
    });
  }

  await page.goto(`${baseUrl}/#journey`, { waitUntil: 'load' });
  const journey = await page.evaluate(() => Boolean(document.querySelector('[data-journey]')));
  let motionEvidence = { scrolled: false, framesReady: false, frameIndex: 0, activeCaptions: 0, progress: 0, centerPixel: '' };
  if (journey) {
    await page.goto(`${baseUrl}/#journey`, { waitUntil: 'load' });
    for (let step = 0; step <= 24; step += 1) {
      await page.evaluate(ratio => {
        const section = document.querySelector('[data-journey]');
        const range = section.getBoundingClientRect().height - window.innerHeight;
        window.scrollTo(0, section.offsetTop + range * ratio);
      }, step / 24);
      await page.waitForTimeout(45);
    }
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);
    motionEvidence = await page.evaluate(() => {
      const bar = document.querySelector('.journey-progress span');
      const canvas = document.querySelector('#journey-film');
      const context = canvas?.getContext('2d');
      const pixel = context ? Array.from(context.getImageData(640, 360, 1, 1).data).join(',') : '';
      const matrix = bar ? new DOMMatrixReadOnly(getComputedStyle(bar).transform) : null;
      return {
        scrolled: true,
        framesReady: canvas?.dataset.framesReady === 'true',
        frameIndex: Number(canvas?.dataset.frameIndex || -1),
        centerPixel: pixel,
        motionProgress: document.querySelector('[data-journey]')?.dataset.motionProgress,
        activeCaptions: document.querySelectorAll('.journey-caption.is-active').length,
        progress: matrix ? matrix.a : 0,
      };
    });
    await page.goto(`${baseUrl}/`, { waitUntil: 'load' });
  }

  for (const file of aiFiles) {
    const response = await page.request.get(baseUrl + file);
    if (!response.ok()) errors.push(`${file} returned HTTP ${response.status()}`);
  }
  for (const file of ['/assets/icon-256.png', '/site.webmanifest']) {
    const response = await page.request.get(baseUrl + file);
    if (!response.ok()) errors.push(`${file} returned HTTP ${response.status()}`);
  }
  for (const imageUrl of [...new Set(pages.map(item => item.ogImage))]) {
    const assetUrl = new URL(new URL(imageUrl).pathname, baseUrl);
    const response = await page.request.head(assetUrl.href);
    if (!response.ok()) errors.push(`${imageUrl} returned HTTP ${response.status()}`);
  }

  const rootResponse = await page.request.get(baseUrl + '/');
  const security = {
    csp: rootResponse.headers()['content-security-policy'] || '',
    hsts: rootResponse.headers()['strict-transport-security'] || '',
  };

  const failures = [];
  for (const item of pages) {
    const isHomepage = ['/', '/zh-cn/', '/zh-tw/', '/ja/'].includes(item.route);
    if (!item.title.includes('ReadMD')) failures.push(`${item.route}: missing ReadMD title`);
    if (item.h1Count !== 1) failures.push(`${item.route}: expected one h1`);
    if (item.canonical !== item.ogUrl) failures.push(`${item.route}: canonical and og:url differ`);
    if (!item.ogImage.endsWith('.png')) failures.push(`${item.route}: og:image must use compatible PNG fallback`);
    if (item.ogImage !== item.twitterImage) failures.push(`${item.route}: twitter:image must match og:image`);
    if (item.robots !== 'index,follow,max-image-preview:large,max-snippet:-1,max-video-preview:-1') failures.push(`${item.route}: bad robots directive`);
    if (!item.stylesheetLoaded) failures.push(`${item.route}: production stylesheet did not load`);
    if (item.brokenImages) failures.push(`${item.route}: ${item.brokenImages} broken images`);
    if (item.pictureCount !== item.webpImages) failures.push(`${item.route}: expected every picture to select WebP`);
    if (isHomepage) {
      if (!item.heroPreloaded) failures.push(`${item.route}: hero WebP is not preloaded`);
      if (item.shareLinks < 3) failures.push(`${item.route}: share links are incomplete`);
    }
    if (!isHomepage) {
      if (!item.breadcrumb) failures.push(`${item.route}: breadcrumb is missing`);
    }
    if (item.route.includes('/download/') || item.route.includes('/workflows/')) {
      const expectedRelated = item.route.includes('/download/')
        ? item.route.replace('/download/', '/workflows/')
        : item.route.replace('/workflows/', '/download/');
      if (!item.breadcrumbPaths?.includes(expectedRelated)) failures.push(`${item.route}: sibling internal link is missing`);
    }
    if (!item.faviconLinked) failures.push(`${item.route}: favicon is missing`);
    if (!item.manifestLinked) failures.push(`${item.route}: web manifest is missing`);
    if (!item.releaseFeedLinked) failures.push(`${item.route}: release feed link is missing`);
    if (item.route === '/') {
    if (!item.siteScriptLoaded) failures.push(`${item.route}: motion stylesheet script did not load`);
    if (!item.particleField) failures.push(`${item.route}: particle field is missing`);
    if (!item.capabilityCanvas) failures.push(`${item.route}: capability cinema canvas is missing`);
    if (item.capabilityPanels !== 9) failures.push(`${item.route}: expected nine capability panels`);
    if (Number(item.capabilityProgress || 0) < 0.85) failures.push(`${item.route}: capability cinema progress was ${item.capabilityProgress}`);
    if (item.capabilityTrack === 'none') failures.push(`${item.route}: capability rail did not move`);
      if (!item.journeyCanvas) failures.push(`${item.route}: journey canvas is missing`);
      if (!motionEvidence.scrolled) failures.push(`${item.route}: journey was not scroll-tested`);
      if (!motionEvidence.framesReady) failures.push(`${item.route}: film frames are not ready`);
      if (motionEvidence.frameIndex < 45) failures.push(`${item.route}: scroll drove only frame ${motionEvidence.frameIndex}`);
      if (!motionEvidence.centerPixel || motionEvidence.centerPixel === '0,0,0,0') failures.push(`${item.route}: film canvas is blank`);
      if (motionEvidence.activeCaptions !== 1) failures.push(`${item.route}: expected one active journey caption`);
      if (motionEvidence.progress <= 0) failures.push(`${item.route}: journey progress did not advance`);
    }
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
