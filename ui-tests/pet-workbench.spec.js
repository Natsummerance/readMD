const { test, expect } = require('@playwright/test');

async function setup(page, overrides = {}, failConfigure = false) {
  const status = { installed: true, enabled: true, in_app: true, adapter: { available: true, running: false }, preferences: { renderer: 'hermes-sprite', scale: .33, opacity: 1 }, ...overrides };
  const gallery = { ok: true, active: '', pets: [{ slug: 'custom-cat', display_name: 'My Cat' }] };
  const configs = [];
  await page.route('**/api/pets', r => r.fulfill({ json: gallery }));
  await page.route('**/api/pets/status', r => r.fulfill({ json: { ok: true, status } }));
  await page.route('**/api/pets/active', async r => {
    gallery.active = r.request().postDataJSON().slug;
    await r.fulfill({ json: { ok: true } });
  });
  await page.route('**/api/pets/configure', async r => {
    const config = r.request().postDataJSON(); configs.push(config);
    if (failConfigure) return r.fulfill({ json: { ok: false, code: 'test_runtime_failed' } });
    Object.assign(status, { enabled: config.enabled, in_app: config.in_app });
    status.adapter.running = config.enabled && !config.in_app;
    Object.assign(status.preferences, { renderer: config.renderer });
    await r.fulfill({ json: { ok: true } });
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof window.openPetSettings === 'function');
  await page.evaluate(() => window.openPetSettings());
  await expect(page.locator('#pet-renderer-row')).toBeVisible();
  return { status, gallery, configs };
}

test('character search and cards persist sprite and Live2D choices with top filter and favorites', async ({ page }) => {
  const { gallery, configs } = await setup(page);
  await expect(page.locator('label:has(#pet-enabled)')).toBeVisible();

  // Initially in sprite mode, 3 custom pets are at the front, followed by companion
  const buttons = page.locator('#pet-roster .pet-character-card');
  await expect(buttons.first()).toHaveAttribute('data-slug', 'mochi');
  await expect(buttons.nth(1)).toHaveAttribute('data-slug', 'moss');
  await expect(buttons.nth(2)).toHaveAttribute('data-slug', 'amber');
  await expect(buttons.nth(3)).toHaveAttribute('data-slug', '');

  // Favorite button exists on card and clicking it favorites the pet
  const amberCard = page.locator('#pet-roster .pet-character-card[data-slug="amber"]');
  const favBtn = amberCard.locator('.pet-fav-btn');
  await expect(favBtn).toBeAttached();
  await favBtn.click();
  await expect(favBtn).toHaveClass(/is-favorite/);
  // Now amber is favorited and moves to the top of the roster!
  await expect(page.locator('#pet-roster .pet-character-card').first()).toHaveAttribute('data-slug', 'amber');

  // Search filter
  await page.locator('#pet-roster-search').fill('My Cat');
  await expect(page.locator('#pet-roster .pet-character-card')).toHaveCount(1);
  await page.locator('#pet-roster .pet-character-card').click();
  await expect(page.locator('#pet-roster .pet-character-card')).toHaveAttribute('aria-pressed', 'true');
  expect(gallery.active).toBe('custom-cat');
  await expect(page.locator('#pet-active-slug')).toHaveText('My Cat');

  // Switch top renderer dropdown to Live2D
  await page.locator('#pet-roster-search').fill('');
  await page.locator('#pet-renderer').selectOption('live2d');
  // Under Live2D, only Live2D pets are shown
  await expect(page.locator('#pet-roster .pet-character-card')).toHaveCount(1);
  await expect(page.locator('#pet-roster .pet-character-card')).toHaveAttribute('data-slug', 'arch-chan');
  await page.locator('#pet-roster .pet-character-card').click();
  await expect.poll(() => configs.at(-1)?.renderer).toBe('live2d');
  expect(configs.at(-1).in_app).toBe(false);
  await expect(page.locator('#pet-roster .pet-character-card')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('#pet-status-dot')).toHaveClass(/is-running/);

  // Search non-existent character
  await page.locator('#pet-roster-search').fill('no-such-character');
  await expect(page.locator('#pet-roster .pet-character-card')).toHaveCount(0);
  await expect(page.locator('#pet-roster')).not.toBeEmpty();
});

test('failed character configuration restores the saved selection', async ({ page }) => {
  const { gallery } = await setup(page, {}, true);
  await page.locator('#pet-roster [data-slug="custom-cat"]').click();
  await expect(page.locator('#pet-choice-feedback')).toContainText('test_runtime_failed');
  expect(gallery.active).toBe('');
  await expect(page.locator('#pet-roster [data-slug=""]')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('#pet-roster [data-slug="custom-cat"]')).toBeEnabled();
});

test('quiet mode keeps deliberate greetings and keyboard interaction available', async ({ page }) => {
  await setup(page);
  await page.locator('[data-pet-section="companion"]').click();
  await page.locator('#pet-mode-quiet').click();
  await page.locator('#pet-say-hello').click();
  await expect(page.locator('#pet-interaction-preview')).not.toBeEmpty();
  await page.locator('#pet-settings-close').click();
  await page.locator('#pet-character-wrap').focus();
  await page.locator('#pet-character-wrap').press('Enter');
  await expect(page.locator('#pet-bubble')).toHaveClass(/is-visible/);
  await expect(page.locator('#pet-quick-quiet')).toHaveAttribute('aria-pressed', 'true');
  await page.locator('#pet-quick-quiet').click();
  await expect(page.locator('#pet-quick-quiet')).toHaveAttribute('aria-pressed', 'false');
  await page.locator('#pet-quick-chat').click();
  await expect(page.locator('#ai-panel')).toBeVisible();
});

test('desktop status does not report a stopped process as running', async ({ page }) => {
  await setup(page, { in_app: false });
  await expect(page.locator('#pet-status-dot')).not.toHaveClass(/is-running/);
  const hint = await page.evaluate(() => window.i18n.t('pet.runtime.stoppedHint'));
  await expect(page.locator('#pet-status-line')).toHaveText(hint);
});
