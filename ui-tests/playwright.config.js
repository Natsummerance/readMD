const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: '.',
  testMatch: '*.spec.js',
  timeout: 30000,
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:28473',
    viewport: { width: 720, height: 600 },
    trace: 'retain-on-failure',
    locale: 'zh-CN',
  },
  webServer: {
    command: 'python ../tools/ui_server.py',
    port: 28473,
    reuseExistingServer: true,
    cwd: __dirname
  }
});
