const { test, expect } = require('@playwright/test');

test.describe('frontend release audit', () => {
  test('interactive state classes resolve to stylesheet rules', async ({ page }) => {
    await page.goto('/');
    const states = await page.evaluate(() => {
      const probe = document.createElement('span');
      probe.style.display = 'none';
      document.body.appendChild(probe);
      const read = className => {
        probe.className = className;
        const style = getComputedStyle(probe);
        return {
          color: style.color,
          backgroundColor: style.backgroundColor,
          paddingTop: style.paddingTop,
          fontWeight: style.fontWeight,
        };
      };
      return {
        danger: read('tb-btn danger').color,
        external: read('tab-dirty tab-external-changed').backgroundColor,
        empty: read('ai-history-empty').paddingTop,
        remoteLink: read('remote-image-link').color,
        tocCurrent: read('toc-cur-page').backgroundColor,
        previewTitle: read('export-preview-title').fontWeight,
      };
    });

    expect(states.danger).not.toBe('rgb(92, 100, 112)');
    expect(states.external).toBe('rgb(179, 38, 30)');
    expect(Number.parseFloat(states.empty)).toBeGreaterThan(0);
    expect(states.remoteLink).not.toBe('rgb(28, 31, 38)');
    expect(states.tocCurrent).not.toBe('rgba(0, 0, 0, 0)');
    expect(states.previewTitle).toBe('700');
  });

});

test.describe('presentation security audit', () => {
  test.use({ bypassCSP: false });

  test('presentation renders Reveal under the application CSP', async ({ page }) => {
    await page.addInitScript(() => {
      window.__cspViolations = [];
      window.addEventListener('securitypolicyviolation', event => {
        window.__cspViolations.push({
          directive: event.violatedDirective,
          blocked: event.blockedURL,
          policy: event.originalPolicy,
        });
      });
    });

    const consoleBlocks = [];
    page.on('console', message => {
      if (/Refused to load|Content Security Policy/i.test(message.text())) {
        consoleBlocks.push(message.text());
      }
    });

    await page.goto('/');
    const indexHeaders = await page.request.get('/');
    expect(indexHeaders.headers()['x-frame-options']).toBe('DENY');
    await page.waitForFunction(() => typeof launchPresentationMode === 'function');
    await page.evaluate(() => {
      const markdown = '# CSP Slide One\n\nDirect Reveal rendering.\n\n<!-- slide -->\n\n## CSP Slide Two\n\n- stable\n- offline';
      state.mode = 'file';
      state.file = 'csp.md';
      state.dir = '';
      state.original = markdown;
      state.tabs = [{ id: 'p1', name: 'csp.md', title: 'csp.md', original: markdown }];
      state.activeTabId = 'p1';
      renderTabsBar();
      updateStatus();
      void launchPresentationMode();
    });

    await expect(page.locator('#presentation-modal')).toBeVisible();
    let frame;
    await expect.poll(async () => {
      frame = page.frames().find(candidate =>
        candidate !== page.mainFrame() && candidate.parentFrame() === page.mainFrame()
      );
      return Boolean(frame);
    }, { timeout: 5000 }).toBeTruthy();
    expect(frame).toBeTruthy();
    await frame.waitForSelector('.reveal .slides section');
    await frame.waitForFunction(() => Boolean(window.Reveal && window.deck));
    await expect(frame.locator('.reveal .slides section')).toHaveCount(2);
    await expect(frame.locator('.reveal .slides').first()).toContainText('CSP Slide One');

    for (const current of page.frames()) {
      const violations = await current.evaluate(() => window.__cspViolations || []).catch(() => []);
      expect(violations).toEqual([]);
    }
    expect(consoleBlocks).toEqual([]);
  });

});

test.describe('zen and tab stability', () => {
  test('zen mode enters without moving the reader or showing a toast', async ({ page }) => {
    await page.goto('/');
    await page.waitForFunction(() => typeof toggleZenMode === 'function' && typeof renderContent === 'function');
    await page.evaluate(async () => {
      await renderContent('# Stable Zen\n\nImmersive reading body.', 'zen.md');
      $('content').scrollTop = 24;
      document.getElementById('toast').classList.add('hidden');
    });

    const before = await page.evaluate(() => ({
      rect: document.querySelector('#content .markdown-body').getBoundingClientRect().toJSON(),
      scroll: document.getElementById('content').scrollTop,
    }));
    const entering = await page.evaluate(() => {
      const hasEnteringRule = Array.from(document.styleSheets).some(sheet => {
        try {
          return Array.from(sheet.cssRules).some(rule =>
            rule.selectorText?.includes('.zen-mode.zen-entering')
          );
        } catch (_) {
          return false;
        }
      });
      toggleZenMode(true);
      return {
        zen: document.body.classList.contains('zen-mode'),
        enteringRule: hasEnteringRule,
        toastHidden: document.getElementById('toast').classList.contains('hidden'),
      };
    });

    expect(entering.zen).toBe(true);
    expect(entering.enteringRule).toBe(true);
    expect(entering.toastHidden).toBe(true);

    await page.waitForFunction(() => document.activeElement?.id === 'content');
    await page.waitForFunction(() => !document.body.classList.contains('zen-entering'));
    const after = await page.evaluate(() => ({
      rect: document.querySelector('#content .markdown-body').getBoundingClientRect().toJSON(),
      scroll: document.getElementById('content').scrollTop,
      transform: getComputedStyle(document.getElementById('toolbar')).transform,
    }));
    expect(Math.abs(after.rect.left - before.rect.left)).toBeLessThanOrEqual(1);
    expect(Math.abs(after.rect.top - before.rect.top)).toBeLessThanOrEqual(1);
    expect(Math.abs(after.scroll - before.scroll)).toBeLessThanOrEqual(1);
    expect(after.transform).toMatch(/matrix\(1, 0, 0, 1, 0, -\d+\)/);
  });

  test('tab selection is immediate and continuous scroll survives switches', async ({ page }) => {
    await page.goto('/');
    await page.waitForFunction(() => typeof renderTabsBar === 'function');
    const paragraphs = Array.from({ length: 160 }, (_, index) => `Paragraph ${index} keeps continuous reading measurable.`).join('\n\n');
    await page.evaluate(({ paragraphs }) => {
      state.tabs = [
        { id: 'long', title: 'long.md', name: 'long.md', content: `# Long\n\n${paragraphs}` },
        { id: 'short', title: 'short.md', name: 'short.md', content: '# Short' },
      ];
      state.activeTabId = 'long';
      syncStateFromActiveTab();
      renderTabsBar();
      void renderActiveTab({ restoreScroll: true });
    }, { paragraphs });

    await page.waitForFunction(() => document.getElementById('content')?.textContent.includes('Paragraph 159'));
    await page.evaluate(() => { document.getElementById('content').scrollTop = 320; });
    await page.waitForFunction(() => getActiveTab()?.scrollPos === 320);

    const selectionTiming = await page.evaluate(() => {
      const bar = document.getElementById('doc-tabs-bar');
      void switchTab('short');
      return {
        activeId: state.activeTabId,
        selectedNow: bar.querySelector('[aria-selected="true"]')?.dataset.tabId,
      };
    });
    expect(selectionTiming.activeId).toBe('short');
    expect(selectionTiming.selectedNow).toBe('short');

    await page.waitForFunction(() => document.getElementById('content')?.textContent.includes('Short'));
    await page.evaluate(() => { void switchTab('long'); });
    await page.waitForFunction(() => document.getElementById('content')?.textContent.includes('Paragraph 159'));
    await page.waitForFunction(() => Math.abs(document.getElementById('content').scrollTop - 320) <= 2);
  });
});
