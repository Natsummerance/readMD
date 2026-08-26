const { test, expect } = require('@playwright/test');

test('AI history renders untrusted markdown through the shared sanitizer', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => (
    typeof renderAiHistory === 'function' && typeof window.renderSafeMarkdown === 'function'
  ));

  await page.evaluate(() => {
    state.ai.messages = [{
      role: 'assistant',
      content: [
        '**Safe AI response**',
        '<script>window.__aiXss = true;</script>',
        '<iframe src="https://example.invalid"></iframe>',
        '<div class="code-chunk-card" data-lang="python" data-code="window.__aiXss = true"><pre><code>print(1)</code></pre><button class="code-chunk-run-btn" type="button">Run</button></div>',
        '<div id="content" class="modal" role="dialog" aria-hidden="false" aria-live="assertive" onclick="window.__aiXss = true">Override</div>',
      ].join('\n'),
    }];
    renderAiHistory();
  });

  const output = page.locator('#ai-output');
  await expect(output.locator('strong')).toHaveText('Safe AI response');
  await expect(output.locator('script, iframe')).toHaveCount(0);
  await expect(output.locator('.code-chunk-card, .code-chunk-run-btn, [data-lang], [data-code]')).toHaveCount(0);
  await expect(output.locator('[onclick], [role], [aria-hidden], [aria-live]')).toHaveCount(0);
  await expect(output.locator('#content.modal')).toHaveCount(0);
  await expect(output.locator('.modal')).toHaveCount(0);
  expect(await page.evaluate(() => window.__aiXss)).toBeFalsy();
});
