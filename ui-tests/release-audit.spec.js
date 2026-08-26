const { test, expect } = require('@playwright/test');

test('QR code stays out of startup and loads once on demand', async ({ page }) => {
  let qrcodeRequests = 0;
  await page.route('**/assets/vendor/qrcode.min.js*', async route => {
    qrcodeRequests += 1;
    await route.continue();
  });

  await page.goto('/');
  await page.waitForFunction(() => typeof loadQrLibrary === 'function');
  expect(await page.evaluate(() => typeof qrcode)).toBe('undefined');

  await page.evaluate(() => loadQrLibrary());
  await page.waitForFunction(() => typeof qrcode === 'function');
  const generated = await page.evaluate(() => {
    const qr = qrcode(0, 'M');
    qr.addData('readmd:lazy-qr');
    qr.make();
    return qr.getModuleCount();
  });

  expect(qrcodeRequests).toBe(1);
  expect(generated).toBeGreaterThan(20);
});

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => {
    localStorage.setItem('readmd_language', 'zh-CN');
  });
});

test('presentation renders offline with initialized Reveal assets', async ({ page }) => {
  const errors = [];
  const failedResponses = [];
  const externalRequests = [];
  page.on('pageerror', error => errors.push(String(error)));
  page.on('request', request => {
    const { protocol, hostname } = new URL(request.url());
    if ((protocol === 'http:' || protocol === 'https:') && hostname !== '127.0.0.1') {
      externalRequests.push(request.url());
    }
  });
  page.on('response', response => {
    if (response.status() >= 400) failedResponses.push(`${response.status()} ${response.url()}`);
  });

  await page.goto('/');
  await page.waitForFunction(() => typeof launchPresentationMode === 'function');
  await page.locator('#btn-zen').focus();
  await page.evaluate(async () => {
    state.original = [
      '# First slide',
      '',
      '$E=mc^2$',
      '',
      '<!-- slide -->',
      '',
      '# Second slide',
      '',
      '| A | B |',
      '|---|---|',
      '| 1 | 2 |',
    ].join('\n');
    await launchPresentationMode();
  });

  const frame = page.frameLocator('.presentation-iframe');
  await expect(frame.locator('.reveal.ready')).toBeVisible();
  const modal = page.locator('#presentation-modal');
  await expect(modal).toHaveAttribute('role', 'dialog');
  await expect(modal).toHaveAttribute('aria-modal', 'true');
  await expect(modal).toHaveAttribute('aria-label', /演讲演示|Presentation/);
  await expect(page.locator('.presentation-iframe')).toHaveAttribute('title', /演讲演示|Presentation/);
  await expect(page.locator('#presentation-theme-select')).toBeFocused();
  await expect(frame.locator('.slides > section')).toHaveCount(2);
  await expect(frame.locator('.katex')).toBeVisible();
  await expect.poll(() => page.evaluate(() => {
    const iframe = document.querySelector('.presentation-iframe');
    return iframe.contentWindow.window.deck?.isReady?.() === true;
  })).toBe(true);

  expect(externalRequests.filter(url => !url.startsWith(page.url().origin))).toEqual([]);
  expect(failedResponses).toEqual([]);
  expect(errors).toEqual([]);

  for (const selector of ['#presentation-theme-select', '#presentation-transition-select',
    '#presentation-font-dec', '#presentation-font-norm', '#presentation-font-inc',
    '#presentation-overview-btn', '#presentation-fullscreen-btn', '#presentation-close-btn']) {
    const control = page.locator(selector);
    await expect(control).toBeVisible();
    const box = await control.boundingBox();
    expect(box.width).toBeGreaterThanOrEqual(22);
    expect(box.height).toBeGreaterThanOrEqual(20);
  }

  await page.keyboard.press('Escape');
  await expect(modal).toBeHidden();
  await expect(page.locator('#btn-zen')).toBeFocused();
});

test('F11 enters immersive Zen once without layout jitter', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleEdit === 'function');
  await page.evaluate(() => {
    state.original = '# Zen target\n\nImmersive writing body.';
    state.fixed = state.original;
    state.mode = 'virtual';
    return toggleEdit();
  });
  await expect(page.locator('#edit-bar')).toBeVisible();

  const samples = await page.evaluate(async () => {
    const targets = ['#main', '#edit-wrap', '#edit-area']
      .map(selector => document.querySelector(selector))
      .filter(Boolean);
    const boxes = [];
    const record = () => boxes.push(targets.map(target => {
      const rect = target.getBoundingClientRect();
      return [rect.x, rect.y, rect.width, rect.height];
    }));
    record();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'F11', bubbles: true, cancelable: true }));
    await new Promise(resolve => {
      let frames = 0;
      const tick = () => {
        record();
        if (++frames === 10) resolve();
        else requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
    return boxes;
  });

  await expect(page.locator('body')).toHaveClass(/zen-mode/);
  for (const selector of ['#header', '#statusbar', '#doc-tabs-container', '#doc-tabs-secondary-bar']) {
    await expect(page.locator(selector)).toBeHidden();
  }
  await expect(page.locator('#toolbar')).toHaveCSS('transform', /matrix\(1, 0, 0, 1, 0, -\d+/);
  expect(await page.evaluate(() => document.activeElement)).toBeTruthy();

  const settledTail = samples.slice(-5);
  const settledTarget = samples[1];
  expect(settledTail.every(frame => frame.every(
    ([x, y, width, height], index) => Math.max(
      Math.abs(x - settledTarget[index][0]),
      Math.abs(y - settledTarget[index][1]),
      Math.abs(width - settledTarget[index][2]),
      Math.abs(height - settledTarget[index][3]),
    ) <= 1
  ))).toBe(true);
  expect(errors).toEqual([]);

  await page.keyboard.press('Escape');
  await expect(page.locator('body')).not.toHaveClass(/zen-mode/);
});

test('clicked Zen remains immersive until the pointer moves away', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof toggleZenMode === 'function' && typeof renderContent === 'function');
  await page.evaluate(() => renderContent('# Clicked Zen\n\nImmersive reading body.', 'clicked-zen.md'));
  await page.click('#btn-zen');
  await expect(page.locator('body')).toHaveClass(/zen-mode/);
  await expect(page.locator('body')).not.toHaveClass(/zen-entering/);

  const suppressed = await page.evaluate(() => ({
    className: document.body.className,
    transform: getComputedStyle(document.getElementById('toolbar')).transform,
    bottom: document.getElementById('toolbar').getBoundingClientRect().bottom,
  }));
  expect(suppressed.className).toContain('zen-toolbar-suppressed');
  expect(suppressed.transform).toMatch(/matrix\(1, 0, 0, 1, 0, -\d+\)/);
  expect(suppressed.bottom).toBeLessThanOrEqual(0);

  await page.mouse.move(195, 300);
  await expect(page.locator('body')).not.toHaveClass(/zen-toolbar-suppressed/);
  await page.mouse.move(195, 4);
  await expect.poll(async () => page.evaluate(() =>
    getComputedStyle(document.getElementById('toolbar')).transform
  )).toContain('matrix(1, 0, 0, 1, 0, 0)');

  await page.mouse.move(195, 300);
  await expect.poll(async () => page.evaluate(() =>
    getComputedStyle(document.getElementById('toolbar')).transform
  )).toMatch(/matrix\(1, 0, 0, 1, 0, -\d+\)/);
  await page.keyboard.press('Escape');
  await expect(page.locator('body')).not.toHaveClass(/zen-mode/);
});

test('rapid tab switching stays aligned and reveals the active tab', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderTabsBar === 'function');
  await page.setViewportSize({ width: 720, height: 600 });

  const result = await page.evaluate(async () => {
    state.tabs = Array.from({ length: 18 }, (_, index) => ({
      id: `tab-${index}`,
      mode: 'file',
      title: `document-${index}.md`,
      name: `document-${index}.md`,
      content: `# Document ${index}`,
    }));
    state.activeTabId = 'tab-0';
    renderTabsBar();

    const started = performance.now();
    await Promise.all(state.tabs.map(tab => switchTab(tab.id)));
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return { elapsed: performance.now() - started };
  });

  await expect(page.locator('.tab-item[data-tab-id="tab-17"]')).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('#content .markdown-body h1')).toContainText('Document 17');
  expect(result.elapsed).toBeLessThan(500);

  const alignment = await page.evaluate(() => {
    const bar = document.getElementById('doc-tabs-bar');
    const active = bar.querySelector('.tab-item.active');
    const tabs = [...bar.querySelectorAll('.tab-item')];
    const activeRect = active.getBoundingClientRect();
    const barRect = bar.getBoundingClientRect();
    return {
      scrollLeft: bar.scrollLeft,
      visible: activeRect.left >= barRect.left - 1 && activeRect.right <= barRect.right + 1,
      heights: [...new Set(tabs.map(tab => Math.round(tab.getBoundingClientRect().height)))],
      selectedCount: tabs.filter(tab => tab.getAttribute('aria-selected') === 'true').length,
    };
  });
  expect(alignment.scrollLeft).toBeGreaterThan(0);
  expect(alignment.visible).toBe(true);
  expect(alignment.heights).toHaveLength(1);
  expect(alignment.selectedCount).toBe(1);
});

test('superseded incremental renders stop without stale content', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderContent === 'function');

  const result = await page.evaluate(async () => {
    const originalSplit = splitMdBlocks;
    const started = performance.now();
    try {
      splitMdBlocks = () => Array.from(
        { length: 80 },
        (_, index) => `# Stale ${index}\n\n${'x'.repeat(1200)}`
      );
      void renderContent('x'.repeat(301 * 1024), 'long.md');
      await renderContent('# Final short', 'short.md');

      let frames = 0;
      await new Promise(resolve => {
        const tick = () => {
          if (++frames === 20) resolve();
          else requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });

      return {
        elapsed: performance.now() - started,
        finalVisible: document.getElementById('content')?.textContent.includes('Final short'),
        staleVisible: [...document.querySelectorAll('#content h1')]
          .some(heading => heading.textContent.startsWith('Stale ')),
        progressVisible: Boolean(document.getElementById('render-progress')),
      };
    } finally {
      splitMdBlocks = originalSplit;
    }
  });

  expect(result.finalVisible).toBe(true);
  expect(result.staleVisible).toBe(false);
  expect(result.progressVisible).toBe(false);
  expect(result.elapsed).toBeLessThan(750);
});

test('a completed stale tab switch cannot restore the abandoned document', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof switchTab === 'function');

  const result = await page.evaluate(async () => {
    const originalSplit = splitMdBlocks;
    try {
      splitMdBlocks = () => Array.from(
        { length: 60 },
        (_, index) => `# Abandoned ${index}\n\n${'x'.repeat(1200)}`
      );
      state.tabs = [
        { id: 'long', title: 'long.md', name: 'long.md', content: 'x'.repeat(301 * 1024) },
        {
          id: 'short',
          title: 'short.md',
          name: 'short.md',
          content: ['# Short winner'].concat(
            Array.from({ length: 80 }, (_, index) => `Winner paragraph ${index} provides scroll room.`)
          ).join('\n\n'),
        },
      ];
      state.activeTabId = 'short';
      syncStateFromActiveTab();
      renderTabsBar();
      await renderActiveTab();
      document.getElementById('content').scrollTop = 48;

      void switchTab('long');
      await switchTab('short');
      let frames = 0;
      await new Promise(resolve => {
        const tick = () => {
          if (++frames === 15) resolve();
          else requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });

      return {
        activeId: state.activeTabId,
        winnerVisible: document.getElementById('content').textContent.includes('Short winner'),
        staleVisible: [...document.querySelectorAll('#content h1')]
          .some(heading => heading.textContent.startsWith('Abandoned ')),
        progressVisible: Boolean(document.getElementById('render-progress')),
        scroll: document.getElementById('content').scrollTop,
      };
    } finally {
      splitMdBlocks = originalSplit;
    }
  });

  expect(result.activeId).toBe('short');
  expect(result.winnerVisible).toBe(true);
  expect(result.staleVisible).toBe(false);
  expect(result.progressVisible).toBe(false);
  expect(Math.abs(result.scroll - 48)).toBeLessThanOrEqual(2);
});
