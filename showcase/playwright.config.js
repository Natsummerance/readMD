const { defineConfig } = require('../ui-tests/node_modules/@playwright/test');

module.exports = defineConfig({
  testDir: '.',
  testMatch: 'showcase.spec.js',
  timeout: 30000,
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:28473',
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 2,
    trace: 'off',
    locale: 'zh-CN',
  },
  webServer: {
    command: 'python ../tools/ui_server.py',
    port: 28473,
    reuseExistingServer: true,
    cwd: __dirname
  }
});
