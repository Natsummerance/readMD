const { test, expect } = require('@playwright/test');

test('rendered code chunks preserve only sanctioned interactive buttons', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.evaluate(async () => {
    await renderVirtual('clipboard', 'interactive.md', '', [
      '```python cmd=true',
      "print('readme')",
      '```',
      '<button class="code-chunk-run-btn" formaction="/steal">fake</button>',
    ].join('\n'), []);
  });

  const runButton = page.locator('#content .code-chunk-run-btn');
  await expect(runButton).toHaveCount(1);
  await expect(runButton).toBeVisible();
  await expect(runButton).toHaveAttribute('type', 'button');
  await expect(page.locator('#content button')).toHaveCount(3);
  expect(await page.locator('#content button').evaluateAll(buttons => buttons.every(button =>
    button.type === 'button'
    && ['formaction', 'formmethod', 'onclick', 'onmouseover'].every(attribute => !button.hasAttribute(attribute))
  ))).toBe(true);
  await expect(page.locator('#content button', { hasText: 'fake' })).toHaveCount(0);
  await expect(page.locator('#content')).toContainText('fake');
});
