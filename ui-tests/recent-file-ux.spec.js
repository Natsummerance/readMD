const { test, expect } = require('@playwright/test');

test.describe('Recent files status and removal UX', () => {
  test('renders deleted files with strikethrough, enables single-item removal', async ({ page }) => {
    // Mock the backend APIs
    await page.route('**/api/recent/status', async route => {
      const body = {
        ok: true,
        items: [
          {
            path: 'C:/docs/normal.md',
            status: 'exists',
            resolved_path: 'C:/docs/normal.md',
            name: 'normal.md',
            dir: 'C:/docs'
          },
          {
            path: 'C:/docs/moved_old.md',
            status: 'moved',
            resolved_path: 'C:/docs/sub/moved_old.md',
            name: 'moved_old.md',
            dir: 'C:/docs/sub'
          },
          {
            path: 'C:/docs/deleted.md',
            status: 'deleted',
            resolved_path: 'C:/docs/deleted.md',
            name: 'deleted.md',
            dir: 'C:/docs'
          }
        ]
      };
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(body)
      });
    });

    let removedPath = null;
    await page.route('**/api/recent/remove', async route => {
      const data = JSON.parse(route.request().postData() || '{}');
      removedPath = data.path;
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ ok: true })
      });
    });

    await page.goto('/');

    // Wait for the app boot scripts to settle
    await page.waitForFunction(() => typeof refreshRecent === 'function');

    // Trigger renderRecentList
    await page.evaluate(async () => {
      window.hasPy = false;
      const box = document.getElementById('recent-box');
      if (box) box.classList.remove('hidden');
      await refreshRecent();
    });

    // Wait for recent cards to appear
    await page.waitForSelector('.recent-card');
    const cards = await page.locator('.recent-card').all();
    expect(cards.length).toBe(3);

    // Wait for async status resolution (is-deleted applied)
    await page.waitForSelector('.recent-card.is-deleted');
    const deletedCard = page.locator('.recent-card.is-deleted');
    await expect(deletedCard).toBeVisible();
    const deletedName = deletedCard.locator('.recent-name.is-deleted');
    await expect(deletedName).toBeVisible();

    // Verify text-decoration contains line-through
    const textDecoration = await deletedName.evaluate(el => window.getComputedStyle(el).textDecorationLine);
    expect(textDecoration).toBe('line-through');

    // Verify remove button exists on each card
    const removeBtns = await page.locator('.recent-remove').all();
    expect(removeBtns.length).toBe(3);

    // Click remove on the deleted card
    await deletedCard.locator('.recent-remove').click();
    await page.waitForTimeout(100);
    expect(removedPath).toBe('C:/docs/deleted.md');
  });
});
