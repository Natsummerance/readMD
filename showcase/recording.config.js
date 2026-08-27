'use strict';

const { defineConfig } = require('../ui-tests/node_modules/@playwright/test');
const path = require('path');

module.exports = defineConfig({
  testDir: '.',
  testMatch: 'recording.spec.js',
  timeout: 90_000,
  workers: 1,
  retries: 0,
  reporter: [['line']],
  use: {
    baseURL: 'http://127.0.0.1:28473',
    viewport: { width: 1440, height: 810 },
    deviceScaleFactor: 2,
    locale: 'zh-CN',
    trace: 'off',
  },
  webServer: {
    command: 'python ../tools/ui_server.py',
    port: 28473,
    reuseExistingServer: true,
    cwd: path.resolve(__dirname),
    timeout: 30_000,
  },
});
