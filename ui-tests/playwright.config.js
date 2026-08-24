const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: '.',
  testMatch: '*.spec.js',
  timeout: 45000,
  workers: 1,
  retries: 1,
  use: {
    baseURL: 'http://127.0.0.1:28473',
    viewport: { width: 720, height: 600 },
    bypassCSP: true,
    trace: 'retain-on-failure',
    locale: 'zh-CN',
  },
  webServer: {
    command: 'python ../tools/ui_server.py',
    port: 28473,
    reuseExistingServer: process.env.READMD_REUSE_UI_SERVER === '1',
    cwd: __dirname
  },
  projects: [
    { name: 'desktop', use: { viewport: { width: 1440, height: 900 } } },
    { name: 'mobile', use: { viewport: { width: 390, height: 844 }, hasTouch: true } },
  ],
});
