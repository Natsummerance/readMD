const { defineConfig, devices } = require('@playwright/test');
const uiPort = Number(process.env.READMD_UI_PORT || 28473);

module.exports = defineConfig({
  testDir: '.',
  testMatch: '*.spec.js',
  timeout: 45000,
  workers: 1,
  retries: 1,
  use: {
    baseURL: `http://127.0.0.1:${uiPort}`,
    viewport: { width: 720, height: 600 },
    bypassCSP: true,
    trace: 'retain-on-failure',
    locale: 'zh-CN',
  },
  webServer: {
    command: `python ../tools/ui_server.py ${uiPort}`,
    port: uiPort,
    reuseExistingServer: process.env.READMD_REUSE_UI_SERVER === '1',
    env: {
      READMD_UI_PORT: String(uiPort),
    },
    cwd: __dirname
  },
  projects: [
    { name: 'desktop', use: { viewport: { width: 1440, height: 900 } } },
    { name: 'firefox', use: { ...devices['Desktop Firefox'], viewport: { width: 1440, height: 900 } } },
    { name: 'webkit', use: { ...devices['Desktop Safari'], viewport: { width: 1440, height: 900 } } },
    { name: 'mobile', use: { viewport: { width: 390, height: 844 }, hasTouch: true } },
  ],
});
