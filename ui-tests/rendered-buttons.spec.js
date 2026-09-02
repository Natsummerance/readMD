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

test('MPE-style fenced attributes select the command and presentation options', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.evaluate(async () => {
    await renderVirtual('clipboard', 'attributes.md', '', [
      '```text {cmd=python echo=false output=markdown}',
      "print('readme')",
      '```',
    ].join('\n'), []);
  });

  const card = page.locator('#content .code-chunk-card');
  await expect(card).toHaveCount(1);
  await expect(card).toHaveAttribute('data-lang', 'python');
  await expect(card).toHaveAttribute('data-hide', 'true');
  await expect(card).toHaveAttribute('data-echo', 'false');
  await expect(card).toHaveAttribute('data-output', 'true');
  await expect(card).toHaveAttribute('data-output-format', 'markdown');
  await expect(card.locator('.code-chunk-src')).toHaveClass(/hidden/);
});

test('cmd=false stays a normal fenced block and never enables execution', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function');
  await page.evaluate(async () => {
    await renderVirtual('clipboard', 'attributes-disabled.md', '', [
      '```python {cmd=false output=markdown}',
      "print('must stay inert')",
      '```',
    ].join('\n'), []);
  });

  await expect(page.locator('#content .code-chunk-card')).toHaveCount(0);
  await expect(page.locator('#content pre code.language-python')).toContainText('must stay inert');
});
