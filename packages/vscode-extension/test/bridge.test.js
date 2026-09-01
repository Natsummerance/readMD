const test = require('node:test');
const assert = require('node:assert');
const path = require('node:path');
const Module = require('node:module');

const outDir = path.join(__dirname, '..', 'out');

const vscodeStub = {
  workspace: { getConfiguration: () => ({ get: () => '' }) },
};

function makeFakeProc() {
  const { EventEmitter } = require('node:events');
  const proc = new EventEmitter();
  proc.stdout = new EventEmitter();
  proc.stderr = new EventEmitter();
  proc.stdin = { write: () => true, writable: true };
  proc.killed = false;
  proc.exitCode = null;
  proc.signalCode = null;
  proc.kill = () => { proc.killed = true; return true; };
  return proc;
}

const fakeCp = { spawn: () => { throw new Error('spawn not configured'); } };

const originalLoad = Module._load;
Module._load = function patchedLoad(request, parent, isMain) {
  const parentFile = (parent && parent.filename ? parent.filename : '').replace(/\\/g, '/');
  if (parentFile.endsWith('/out/bridge.js')) {
    if (request === 'child_process') return fakeCp;
    if (request === './pythonFinder') return { findPythonPath: async () => 'python-stub' };
    if (request === 'vscode') return vscodeStub;
  }
  return originalLoad.call(this, request, parent, isMain);
};

const { ReadMDBridge } = require(path.join(outDir, 'bridge.js'));

const sleep = (ms = 10) => new Promise(resolve => setTimeout(resolve, ms));

test('onReady fires on spawn and onDisconnected fires when the core exits unexpectedly', async () => {
  const events = [];
  const proc = makeFakeProc();
  fakeCp.spawn = () => proc;
  const bridge = new ReadMDBridge({ extensionPath: '/fake-ext' });
  bridge.onDisconnected(() => events.push('disconnected'));
  bridge.onReady(() => events.push('ready'));

  const pending = bridge.callMcpMethod('tools/list');
  await sleep();
  proc.emit('spawn');
  await sleep();
  assert.deepEqual(events, ['ready']);

  proc.exitCode = 1;
  proc.emit('close', 1);
  await assert.rejects(pending, /进程异常退出/);
  await sleep();
  assert.deepEqual(events, ['ready', 'disconnected']);
  bridge.dispose();
});

test('close after a kill signal also fires onDisconnected', async () => {
  const events = [];
  const proc = makeFakeProc();
  fakeCp.spawn = () => proc;
  const bridge = new ReadMDBridge({ extensionPath: '/fake-ext' });
  bridge.onDisconnected(() => events.push('disconnected'));
  const pending = bridge.callMcpMethod('tools/list');
  await sleep();
  proc.emit('spawn');
  await sleep();

  proc.signalCode = 'SIGTERM';
  proc.emit('close', null, 'SIGTERM');
  await assert.rejects(pending, /进程异常退出/);
  await sleep();
  assert.deepEqual(events, ['disconnected']);
  bridge.dispose();
});

test('dispose does not report a disconnect', async () => {
  const events = [];
  const proc = makeFakeProc();
  fakeCp.spawn = () => proc;
  const bridge = new ReadMDBridge({ extensionPath: '/fake-ext' });
  bridge.onDisconnected(() => events.push('disconnected'));
  const pending = bridge.callMcpMethod('tools/list');
  await sleep();
  proc.emit('spawn');
  await sleep();

  bridge.dispose();
  await assert.rejects(pending, /ReadMD Core 已关闭/);
  proc.exitCode = 0;
  proc.emit('close', 0);
  await sleep();
  assert.deepEqual(events, []);
});

test('a failed spawn does not report a disconnect and the bridge can respawn', async () => {
  const events = [];
  const proc1 = makeFakeProc();
  const proc2 = makeFakeProc();
  const procs = [proc1, proc2];
  fakeCp.spawn = () => procs.shift();
  const bridge = new ReadMDBridge({ extensionPath: '/fake-ext' });
  bridge.onDisconnected(() => events.push('disconnected'));

  const first = bridge.callMcpMethod('tools/list');
  await sleep();
  proc1.emit('error', new Error('spawn python ENOENT'));
  await assert.rejects(first, /ENOENT/);
  await sleep();
  assert.deepEqual(events, []);

  proc2.stdin.write = (payload) => {
    const request = JSON.parse(payload);
    proc2.stdout.emit('data', Buffer.from(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { ok: true } }) + '\n'));
    return true;
  };
  const second = bridge.callMcpMethod('tools/list');
  await sleep();
  proc2.emit('spawn');
  assert.deepEqual(await second, { ok: true });
  assert.deepEqual(events, []);
  bridge.dispose();
});
