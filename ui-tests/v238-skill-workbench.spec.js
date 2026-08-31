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

test('Skill details are readable before any editing controls are exposed', async ({ page }) => {
  await openWorkbench(page, [{
    id: 'humanizer',
    skill_id: 'humanizer',
    name: 'Humanizer',
    description: 'Make writing sound natural without changing its meaning.',
    system: '# Humanizer\n\nInstructions from the complete SKILL.md.',
    variables: ['document', 'request'],
    builtin: true,
    scope: 'builtin',
    license: { spdx: 'MIT' },
    provenance: { source: 'upstream', revision: 'abc123' },
    source_files: [{ path: 'SKILL.md', sha256: 'f'.repeat(64) }],
    adaptation_notes: 'ReadMD variable mapping only.',
  }]);

  await expect(page.locator('#tpl-skill-overview')).toBeVisible();
  await expect(page.locator('#tpl-skill-overview')).toContainText('Make writing sound natural');
  await expect(page.locator('#tpl-skill-overview')).toContainText('MIT');
  await expect(page.locator('#tpl-skill-overview')).toContainText('abc123');
  await expect(page.locator('#tpl-skill-overview')).toContainText('document');
  await expect(page.locator('#tpl-editor-fields')).toBeHidden();

  await page.locator('#tpl-edit').click();
  await expect(page.locator('#tpl-editor-fields')).toBeVisible();
  await expect(page.locator('#tpl-system')).toHaveValue(/complete SKILL\.md/);
});

test('import chooser supports GitHub, folder and ZIP without showing credentials by default', async ({ page }) => {
  await openWorkbench(page, []);
  await page.locator('#tpl-import-btn').click();

  await expect(page.locator('#tpl-import-source-github')).toBeVisible();
  await expect(page.locator('#tpl-import-source-folder')).toBeVisible();
  await expect(page.locator('#tpl-import-source-zip')).toBeVisible();
  await expect(page.locator('#tpl-github-credential-wrap')).toBeHidden();

  await page.locator('#tpl-import-source-folder').click();
  await expect(page.locator('#tpl-folder-input')).toHaveAttribute('webkitdirectory', '');
  await page.locator('#tpl-import-source-zip').click();
  await expect(page.locator('#tpl-zip-input')).toHaveAttribute('accept', /\.zip/);
});

test('GitHub authentication failure reveals the credential field only when needed', async ({ page }) => {
  await page.route('**/api/skill-imports/preview', route => route.fulfill({
    status: 401,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false, error_code: 'github_auth_failed' }),
  }));
  await openWorkbench(page, []);
  await page.locator('#tpl-import-btn').click();
  await page.locator('#tpl-github-url').fill('https://github.com/example/private-skills');
  await page.locator('#tpl-github-preview-btn').click();

  await expect(page.locator('#tpl-github-credential-wrap')).toBeVisible();
  await expect(page.locator('#tpl-github-credential')).toBeFocused();
  await expect(page.locator('#tpl-github-credential')).toHaveAttribute('type', 'password');
});

test('GitHub token is submitted once and replaced by an opaque credential id', async ({ page }) => {
  let previewPayload;
  let applyPayload;
  await page.route('**/api/skill-imports/preview', async route => {
    previewPayload = route.request().postDataJSON();
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ok: true,
        credential_id: 'cred:github:opaque',
        preview: { source: { type: 'github' }, skills: [{ id: 'writer', name: 'writer', path: 'writer/SKILL.md', valid: true }] },
      }),
    });
  });
  await page.route('**/api/skill-imports/apply', async route => {
    applyPayload = route.request().postDataJSON();
    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, skills: [{ id: 'writer' }] }) });
  });
  await openWorkbench(page, []);
  await page.locator('#tpl-import-btn').click();
  await page.locator('#tpl-github-url').fill('https://github.com/example/private-skills');
  await page.evaluate(() => revealGithubCredential());
  await page.locator('#tpl-github-credential').fill('github_pat_secret');
  await page.locator('#tpl-github-preview-btn').click();
  await expect(page.locator('#tpl-github-credential')).toHaveValue('');
  await page.locator('#tpl-github-apply-btn').click();
  await page.locator('#confirm-action').click();
  await page.waitForFunction(() => document.getElementById('tpl-github-preview').children.length === 0);

  expect(previewPayload.github_token).toBe('github_pat_secret');
  expect(previewPayload.credential_id).toBeUndefined();
  expect(applyPayload.credential_id).toBe('cred:github:opaque');
  expect(JSON.stringify(applyPayload)).not.toContain('github_pat_secret');
});
