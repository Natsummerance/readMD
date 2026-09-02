const { test, expect } = require('@playwright/test');

async function openWorkbench(page, templates) {
  await page.goto('/');
  await page.waitForFunction(() => typeof openTplModal === 'function');
  await page.evaluate(items => {
    state.ai.templates = items;
    state.ai.templateId = items[0] ? items[0].id : '';
    openTplModal();
  }, templates);
}

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'zh-CN'));
});

test('small windows do not cause horizontal overflow', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof openTplModal === 'function');
  for (const width of [640, 700, 900]) {
    await page.setViewportSize({ width, height: 800 });
    const m = await page.evaluate(() => ({
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth: window.innerWidth,
      aiPanelMinWidth: document.getElementById('ai-panel')
        ? parseFloat(getComputedStyle(document.getElementById('ai-panel')).minWidth) || 0
        : null,
    }));
    expect(m.scrollWidth, `width=${width}`).toBeLessThanOrEqual(m.innerWidth + 2);
    if (m.aiPanelMinWidth !== null) {
      expect(m.aiPanelMinWidth, `width=${width}`).toBeLessThanOrEqual(m.innerWidth);
    }
  }
});

test('doc import modal offers a browse button and keeps the modal open in browser mode', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof openDocImportModal === 'function');
  await page.evaluate(() => { state.editing = true; openDocImportModal(); });
  await expect(page.locator('#doc-import-modal')).toBeVisible();
  await expect(page.locator('#doc-import-browse')).toBeVisible();
  await page.locator('#doc-import-browse').click();
  await expect(page.locator('#doc-import-modal')).toBeVisible();
});

test('provider search input filters provider cards', async ({ page }) => {
  await page.route('**/api/ai/config', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ schema_version: 3, presets: [], custom: [], upstream_catalog: [], current: {} }),
  }));
  await page.goto('/');
  await page.waitForFunction(() => typeof fillAiProviders === 'function');
  // Hydrate the same config path used by the settings modal before seeding;
  // this makes the fixture deterministic even when startup keeps AI lazy.
  await page.evaluate(() => loadAiConfig());
  await page.waitForFunction(() => window.state && state.ai && state.ai.config, null, { timeout: 15000 });
  await page.evaluate(() => openAiModal('ai-settings-modal', $('ai-settings-open')));
  await page.evaluate(() => {
    state.ai.providers = [
      { id: 'openai', name: 'OpenAI', note: 'GPT presets', custom: false },
      { id: 'deepseek', name: 'DeepSeek', note: 'deepseek-chat', custom: false },
    ];
    state.ai.upstreamCatalog = [];
    fillAiProviders(state.ai.providers, { provider_id: 'openai' });
  });
  await expect(page.locator('#ai-provider-cards .ai-provider-card')).toHaveCount(2);
  await page.locator('#ai-provider-search').fill('deepseek');
  await expect(page.locator('#ai-provider-cards .ai-provider-card')).toHaveCount(1);
  await expect(page.locator('#ai-provider-cards')).toContainText('DeepSeek');
  await page.locator('#ai-provider-search').fill('');
  await expect(page.locator('#ai-provider-cards .ai-provider-card')).toHaveCount(2);
});

test('two-step AI creation validates fields and routes unconfigured users to AI settings', async ({ page }) => {
  await page.route('**/api/ai/config', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ schema_version: 3, presets: [], custom: [], upstream_catalog: [], current: {} }),
  }));
  await openWorkbench(page, []);
  await page.locator('#tpl-ai-generate').click();

  await expect(page.locator('#skill-create-modal')).toBeVisible();
  await expect(page.locator('#skill-create-name')).toBeVisible();
  await expect(page.locator('#skill-create-purpose')).toBeVisible();

  await page.locator('#skill-create-go').click();
  await expect(page.locator('#skill-create-modal')).toBeVisible();

  await page.locator('#skill-create-name').fill('测试 Skill');
  await page.locator('#skill-create-purpose').fill('用于回归测试的 Skill 草稿');
  await page.locator('#skill-create-go').click();

  await expect(page.locator('#skill-create-modal')).toBeHidden();
  await expect(page.locator('#ai-settings-modal')).toBeVisible({ timeout: 15000 });
});

test('skill import menu stays inside the viewport at 600/900/1440 widths', async ({ page }) => {
  for (const width of [600, 900, 1440]) {
    await page.setViewportSize({ width, height: 900 });
    await openWorkbench(page, []);
    await page.locator('#tpl-import-btn').click();
    const menu = page.locator('#tpl-import-menu');
    await expect(menu).toBeVisible();
    const box = await menu.boundingBox();
    expect(box, `width=${width}`).not.toBeNull();
    expect(box.x, `width=${width}`).toBeGreaterThanOrEqual(-1);
    expect(box.x + box.width, `width=${width}`).toBeLessThanOrEqual(width + 1);
  }
});

test('long user message collapses into an expandable summary card', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof appendAiUserBubble === 'function');
  const info = await page.evaluate(() => {
    const out = document.createElement('div');
    out.id = 'bubble-test-out';
    document.body.appendChild(out);
    const longDoc = Array.from({ length: 60 }, (_, i) => 'line ' + i + ' ' + 'x'.repeat(20)).join('\n');
    appendAiUserBubble(out, '我 · 提问 1', longDoc + '\n\n问题：\n请总结', null);
    appendAiUserBubble(out, '我 · 提问 2', 'short plain message', null);
    const bubbles = out.querySelectorAll('.ai-msg');
    const card = bubbles[0].querySelector('.ai-user-card');
    const full = bubbles[0].querySelector('.ai-user-card-full');
    return {
      bubbles: bubbles.length,
      hasCard: !!card,
      fullHidden: full ? full.classList.contains('hidden') : null,
      hasToggle: !!bubbles[0].querySelector('.ai-user-card-toggle'),
      statsText: bubbles[0].querySelector('.ai-user-card-stats') ? bubbles[0].querySelector('.ai-user-card-stats').textContent : '',
      shortHasCard: !!bubbles[1].querySelector('.ai-user-card'),
    };
  });
  expect(info.bubbles).toBe(2);
  expect(info.hasCard).toBe(true);
  expect(info.fullHidden).toBe(true);
  expect(info.hasToggle).toBe(true);
  expect(info.statsText).toContain('60');
  expect(info.shortHasCard).toBe(false);

  await page.locator('#bubble-test-out .ai-user-card-toggle').click();
  await expect(page.locator('#bubble-test-out .ai-user-card-full')).toBeVisible();
  await page.locator('#bubble-test-out .ai-user-card-toggle').click();
  await expect(page.locator('#bubble-test-out .ai-user-card-full')).toBeHidden();
});

test('offline diagram dispatcher renders Mermaid, WaveDrom, Bitfield and TikZ without network', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderAllDiagrams === 'function');
  const result = await page.evaluate(async () => {
    const cases = [
      ['mermaid', 'graph TD; A[Start] --> B[Done]'],
      ['wavedrom', '{signal:[{name:"clk",wave:"p..."}]}'],
      ['bitfield', JSON.stringify([{bits: 8, name: 'FLAGS'}])],
      ['viz', 'digraph { A -> B; }'],
      ['vega-lite', JSON.stringify({
        data: { values: [{ label: 'A', value: 28 }, { label: 'B', value: 55 }] },
        mark: 'bar',
        encoding: {
          x: { field: 'label', type: 'nominal' },
          y: { field: 'value', type: 'quantitative' },
        },
      })],
      ['tikz', '\\begin{tikzpicture}\\draw (0,0) -- (1,1);\\end{tikzpicture}'],
    ];
    const outputs = [];
    for (const [engine, code] of cases) {
      const card = document.createElement('div');
      card.className = 'diagram-card';
      card.dataset.diagramEngine = engine;
      card.dataset.diagramCode = encodeURIComponent(code);
      card.innerHTML = '<div class="diagram-preview"></div><button class="diagram-reload-btn"></button>';
      document.body.appendChild(card);
      renderAllDiagrams(card);
      const deadline = Date.now() + (engine === 'tikz' ? 40000 : 12000);
      while (!card.querySelector('.diagram-preview svg') && !card.querySelector('.diagram-fallback') && Date.now() < deadline) {
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      outputs.push({ engine, svg: !!card.querySelector('.diagram-preview svg'), fallback: !!card.querySelector('.diagram-fallback'), text: card.querySelector('.diagram-preview')?.textContent || '' });
    }
    return outputs;
  });
  for (const item of result) expect(item.svg, `${item.engine} did not render: ${JSON.stringify(result)}`).toBeTruthy();
  for (const item of result) expect(item.fallback, `${item.engine} unexpectedly fell back`).toBeFalsy();
});

test('offline Chart.js fence renders a bounded canvas without network', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderAllDiagrams === 'function');
  const result = await page.evaluate(async () => {
    const card = document.createElement('div');
    card.className = 'diagram-card';
    card.dataset.diagramEngine = 'chart';
    card.dataset.diagramCode = encodeURIComponent(JSON.stringify({
      type: 'bar',
      data: { labels: ['A', 'B'], datasets: [{ label: 'ReadMD', data: [1, 2] }] },
      options: { animation: false, responsive: false },
    }));
    card.innerHTML = '<div class="diagram-preview"></div><button class="diagram-reload-btn"></button>';
    document.body.appendChild(card);
    renderAllDiagrams(card);
    const deadline = Date.now() + 12000;
    while (!card.querySelector('.diagram-chart-canvas, .diagram-fallback') && Date.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    const canvas = card.querySelector('.diagram-chart-canvas');
    const rect = canvas?.getBoundingClientRect();
    return {
      canvas: !!canvas,
      width: canvas?.width || 0,
      height: canvas?.height || 0,
      cssWidth: rect?.width || 0,
      cssHeight: rect?.height || 0,
      devicePixelRatio: window.devicePixelRatio || 1,
      fallback: !!card.querySelector('.diagram-fallback'),
    };
  });
  expect(result.canvas).toBeTruthy();
  expect(result.fallback).toBeFalsy();
  // Chart.js retina scaling may enlarge the backing store (WebKit uses DPR 2),
  // but the rendered CSS box must remain bounded for narrow panes and memory.
  expect(result.cssWidth).toBeGreaterThan(0);
  expect(result.cssWidth).toBeLessThanOrEqual(960);
  expect(result.cssHeight).toBeLessThanOrEqual(540);
  expect(result.width).toBeLessThanOrEqual(960 * result.devicePixelRatio);
  expect(result.height).toBeLessThanOrEqual(540 * result.devicePixelRatio);
});
