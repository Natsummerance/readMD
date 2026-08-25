const { test, expect } = require('@playwright/test');

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
