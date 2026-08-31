const { test, expect } = require('@playwright/test');

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'zh-CN'));
});

test('all document AI entry points reuse the provider and opaque credential saved in settings', async ({ page }) => {
  const chatPayloads = [];
  await page.route('**/api/ai/config', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      schema_version: 3,
      presets: [],
      custom: [{
        id: 'custom:shared', name: 'Shared provider', custom: true,
        base_url: 'https://api.example.test/v1', mode: 'chat', endpoint_mode: 'prefix',
        models: ['shared-model'], has_key: true, key_source: 'configured',
        credential_id: 'cred:shared12345',
      }],
      current: { provider_id: 'custom:shared', model: 'shared-model' },
    }),
  }));
  await page.route('**/api/ai/chat', async route => {
    chatPayloads.push(route.request().postDataJSON());
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ ok: true, content: '{}' }),
    });
  });

  await page.goto('/');
  await page.waitForFunction(() => typeof loadAiConfig === 'function' && typeof generateExportStyleWithAi === 'function');
  await page.evaluate(async () => {
    await loadAiConfig();
    await generateExportStyleWithAi('clean reading layout');
    await runEditAiAction('polish', 'make this concise');
    state.fixed = '# document';
    await handleAiDocumentFix();
  });

  expect(chatPayloads.map(payload => payload.skill_id)).toEqual([
    'readmd-export-style', 'readmd-polish', 'readmd-format-fix',
  ]);
  for (const payload of chatPayloads) {
    expect(payload.provider).toBe('custom:shared');
    expect(payload.credential_id).toBe('cred:shared12345');
    expect(payload.model).toBe('shared-model');
    expect(payload.base_url).toBe('https://api.example.test/v1');
    expect(JSON.stringify(payload)).not.toContain('api_key');
  }
});

test('Save As in the More menu is localized by the active dictionary', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => window.i18n && window.i18n.currentLang === 'zh-CN');
  await page.locator('#btn-more').click();
  await expect(page.locator('#btn-saveas > span')).toHaveText('另存为');
  await expect(page.locator('#btn-saveas > em')).toHaveText('.md 文件');
});
