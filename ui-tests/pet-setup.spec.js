const { test, expect } = require('@playwright/test');

test('Live2D enable installs its runtime without opening a file picker', async ({ page }) => {
  const status = { installed: true, enabled: false, in_app: true, adapter: { available: false }, preferences: { renderer: 'hermes-sprite', scale: .33, opacity: 1 } };
  let installs = 0;
  await page.route('**/api/pets/status', route => route.fulfill({ json: { ok: true, status } }));
  await page.route('**/api/pets/runtime/install', async route => {
    installs++;
    status.adapter.available = true;
    await route.fulfill({ json: { ok: true, installed: true } });
  });
  await page.route('**/api/pets/configure', async route => {
    const body = route.request().postDataJSON();
    if (body.enabled && body.renderer === 'live2d') expect(installs).toBe(1);
    Object.assign(status, { enabled: body.enabled, in_app: body.in_app });
    Object.assign(status.preferences, { renderer: body.renderer });
    await route.fulfill({ json: { ok: true } });
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof window.openPetSettings === 'function');
  await page.evaluate(() => window.openPetSettings());
  await page.locator('#pet-renderer').selectOption('live2d');
  await expect(page.locator('#pet-runtime')).toHaveValue('desktop');
  page.on('filechooser', () => { throw new Error('Unexpected file picker'); });
  await page.locator('label:has(#pet-enabled)').click();
  await expect.poll(() => installs).toBe(1);
  await expect(page.locator('#pet-enabled')).toBeChecked();
  await expect(page.locator('#pet-renderer')).toHaveValue('live2d');
  await page.locator('#pet-install-runtime').click();
  await expect.poll(() => installs).toBe(2);
});
