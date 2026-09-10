const { test, expect } = require('@playwright/test');

async function waitForApp(page) {
  await page.goto('/');
  await page.waitForFunction(() => typeof openConvertModal === 'function');
}

test.beforeEach(async ({ page }) => {
  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));
  await page.route('**/api/plugins/list', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      ok: true,
      ffmpeg: true,
      sandbox_dir: 'C:/Users/TestUser/AppData/Roaming/ReadMD/plugins',
      plugins: {
        docling: {
          id: 'docling',
          name_key: 'plugin.docling.name',
          desc_key: 'plugin.docling.desc',
          category: 'document',
          weight: 'heavy',
          approx_size: '~500MB',
          installed: false,
          enabled: false,
          installing: false,
          install_error: '',
          last_log: '',
        },
        easyocr: {
          id: 'easyocr',
          name_key: 'plugin.easyocr.name',
          desc_key: 'plugin.easyocr.desc',
          category: 'ocr',
          weight: 'heavy',
          approx_size: '~150MB',
          installed: true,
          enabled: true,
          installing: false,
          install_error: '',
          last_log: '',
        },
        pylatexenc: {
          id: 'pylatexenc',
          name_key: 'plugin.pylatexenc.name',
          desc_key: 'plugin.pylatexenc.desc',
          category: 'latex',
          weight: 'light',
          approx_size: '~1MB',
          installed: true,
          enabled: false,
          installing: false,
          install_error: '',
          last_log: '',
        },
        whisper: {
          id: 'whisper',
          name_key: 'plugin.whisper.name',
          desc_key: 'plugin.whisper.desc',
          category: 'audio',
          weight: 'heavy',
          approx_size: '~150MB',
          installed: false,
          enabled: false,
          installing: false,
          install_error: '',
          last_log: '',
        },
      }
    }),
  }));
  await page.addInitScript(() => localStorage.setItem('readmd_language', 'zh-CN'));
});

test('Plugin Center entry point and modal cards render correctly', async ({ page }) => {
  await waitForApp(page);

  // 1. 打开更多菜单并打开万物转 MD 模态框
  await page.locator('#btn-more').click();
  await expect(page.locator('#more-menu')).toHaveClass(/open/);
  await page.locator('.more-group:has(#btn-convert) .more-group-header').click();
  await page.locator('#btn-convert').click();
  await expect(page.locator('#convert-modal')).toBeVisible();

  // 2. 检查扩展插件按钮并点击
  const btnPlugins = page.locator('#btn-open-plugins');
  await expect(btnPlugins).toBeVisible();
  await btnPlugins.click();

  // 3. 检查插件管理中心弹窗
  const pluginModal = page.locator('#plugin-modal');
  await expect(pluginModal).toBeVisible();
  await expect(page.locator('#plugin-title')).toContainText('扩展插件管理中心');

  // 4. 检查 FFmpeg 状态徽章与沙箱路径
  const ffmpegBadge = page.locator('#plugin-ffmpeg-badge');
  await expect(ffmpegBadge).toBeVisible();
  await expect(ffmpegBadge).toContainText('已就绪');

  const sandboxPath = page.locator('#plugin-sandbox-path');
  await expect(sandboxPath).toContainText('plugins');

  // 5. 检查插件卡片渲染
  const cards = page.locator('#plugin-cards-grid .plugin-card');
  await expect(cards).toHaveCount(4);

  // 验证 docling 卡片（未安装状态）
  const doclingCard = page.locator('.plugin-card[data-plugin-id="docling"]');
  await expect(doclingCard.locator('button[data-action="install"]')).toBeVisible();

  // 验证 easyocr 卡片（已安装已启用状态）
  const easyocrCard = page.locator('.plugin-card[data-plugin-id="easyocr"]');
  await expect(easyocrCard).toBeVisible();
  await expect(easyocrCard.locator('input[data-action="toggle"]')).toBeChecked();
  await expect(easyocrCard.locator('button[data-action="uninstall"]')).toBeVisible();

  // 6. 验证卡片激活态样式与开关结构
  await expect(easyocrCard).toHaveClass(/is-enabled/);
  await expect(easyocrCard.locator('.plugin-switch-track')).toBeVisible();
  await expect(easyocrCard.locator('.plugin-switch-thumb')).toBeVisible();

  // 7. 按 Esc 键关闭弹窗
  await page.keyboard.press('Escape');
  await expect(pluginModal).toBeHidden();
});

test('Plugin install and uninstall invoke in-app confirmation modal', async ({ page }) => {
  await waitForApp(page);

  // 打开插件管理中心
  await page.locator('#btn-more').click();
  await page.locator('.more-group:has(#btn-convert) .more-group-header').click();
  await page.locator('#btn-convert').click();
  await page.locator('#btn-open-plugins').click();
  const pluginModal = page.locator('#plugin-modal');
  await expect(pluginModal).toBeVisible();

  // 1. 测试安装确认弹窗
  const doclingCard = page.locator('.plugin-card[data-plugin-id="docling"]');
  await doclingCard.locator('button[data-action="install"]').click();

  const confirmModal = page.locator('#confirm-modal');
  await expect(confirmModal).toBeVisible();
  await expect(page.locator('#confirm-title')).toContainText('安装');
  await expect(page.locator('#confirm-cancel')).toBeFocused();
  await page.locator('#confirm-cancel').click();
  await expect(confirmModal).toBeHidden();

  // 2. 测试卸载确认弹窗 (danger 态)
  const easyocrCard = page.locator('.plugin-card[data-plugin-id="easyocr"]');
  await easyocrCard.locator('button[data-action="uninstall"]').click();
  await expect(confirmModal).toBeVisible();
  await expect(page.locator('#confirm-title')).toContainText('卸载');
  await expect(page.locator('#confirm-action')).toHaveClass(/danger/);
  await page.locator('#confirm-cancel').click();
  await expect(confirmModal).toBeHidden();
});

