const { defineConfig } = require('../ui-tests/node_modules/@playwright/test');
const { loadCaptureConfig } = require('./capture.config.cjs');

const config = loadCaptureConfig();

module.exports = defineConfig({
  testDir: '.',
  testMatch: 'showcase.spec.js',
  timeout: 30000,
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:28473',
    viewport: config.viewport,
    deviceScaleFactor: config.scale,
    trace: 'off',
    locale: config.locale,
  },
  webServer: {
    command: 'python ../tools/ui_server.py',
    port: 28473,
    reuseExistingServer: true,
    cwd: __dirname
  }
});
