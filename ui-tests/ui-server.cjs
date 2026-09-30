#!/usr/bin/env node
// Start the Rust kernel for UI tests and recordings (replaces tools/ui_server.py).
//
//   node ui-tests/ui-server.cjs [port]
//
// * READMD_BIN   — prebuilt readmd binary; otherwise `cargo run --release`.
// * READMD_DATA_DIR — data dir; otherwise a fresh temp dir (removed on exit),
//   so every run starts with no AI configured and no recent files.
// Prints "ReadMD UI test server ready on <port>" once the page answers.
'use strict';
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');

const ROOT = path.resolve(__dirname, '..');

function serverCommand(port, dataDir) {
  const args = ['--no-window', '--port', String(port), '--data-dir', dataDir, '--assets', path.join(ROOT, 'assets')];
  if (process.env.READMD_BIN) return { cmd: process.env.READMD_BIN, args };
  return {
    cmd: 'cargo',
    args: ['run', '--quiet', '--release', '--manifest-path', path.join(ROOT, 'rust', 'Cargo.toml'), '-p', 'readmd-kernel', '--', ...args],
  };
}

function waitReady(port, timeoutMs) {
  const until = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const tick = () => {
      const req = http.get({ host: '127.0.0.1', port, path: '/', timeout: 1000 }, res => {
        res.resume();
        if (res.statusCode === 200) resolve(); else retry();
      });
      req.on('error', retry);
      req.on('timeout', () => { req.destroy(); retry(); });
    };
    const retry = () => (Date.now() > until ? reject(new Error(`kernel did not answer on ${port}`)) : setTimeout(tick, 250));
    tick();
  });
}

/** Start the kernel; resolves to { child, port, dataDir, stop() }. */
async function startUiServer(port = Number(process.env.READMD_UI_PORT || 28473), { timeoutMs = 600000, stdio = 'inherit' } = {}) {
  const ownDir = !process.env.READMD_DATA_DIR;
  const dataDir = process.env.READMD_DATA_DIR || fs.mkdtempSync(path.join(os.tmpdir(), 'readmd-ui-test-'));
  const { cmd, args } = serverCommand(port, dataDir);
  const child = spawn(cmd, args, { cwd: ROOT, stdio: ['ignore', stdio, stdio], env: { ...process.env, READMD_DATA_DIR: dataDir } });
  let exited = false;
  child.on('exit', () => { exited = true; });
  const stop = () => {
    if (!exited) child.kill();
    if (ownDir) fs.rmSync(dataDir, { recursive: true, force: true });
  };
  try {
    await Promise.race([
      waitReady(port, timeoutMs),
      new Promise((_, reject) => child.on('exit', code => reject(new Error(`kernel exited early (${code})`)))),
    ]);
  } catch (e) {
    stop();
    throw e;
  }
  return { child, port, dataDir, stop };
}

module.exports = { startUiServer };

if (require.main === module) {
  const port = Number(process.argv[2] || process.env.READMD_UI_PORT || 28473);
  startUiServer(port).then(s => {
    console.log(`ReadMD UI test server ready on ${port}`);
    for (const sig of ['SIGINT', 'SIGTERM']) process.on(sig, () => { s.stop(); process.exit(0); });
    s.child.on('exit', code => { s.stop(); process.exit(code ?? 0); });
  }, e => {
    console.error(e.message);
    process.exit(1);
  });
}
