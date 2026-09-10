const { test, expect } = require('@playwright/test');

test('real pet API persists controls and gallery selection', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof window.openPetSettings === 'function');
  await page.evaluate(() => window.openPetSettings());
  await expect(page.locator('#pet-settings-modal')).toBeVisible();
  await page.locator('label:has(#pet-enabled)').click();
  await expect(page.locator('#readmd-pet-widget')).toBeVisible();
  const status = await page.evaluate(() => window.fetchPetRuntimeStatus());
  expect(status.enabled).toBe(true);
  expect(status.in_app).toBe(true);
  await page.locator('label:has(#pet-enabled)').click();
  await expect(page.locator('#readmd-pet-widget')).toBeHidden();
});

test('unreachable pet API reports failure', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof window.requestConfigurePet === 'function');
  await page.route('**/api/pets/configure', route => route.abort());
  const result = await page.evaluate(() => window.requestConfigurePet({ enabled: true }));
  expect(result.ok).toBe(false);
});
