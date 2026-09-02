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

const waitFor = async (predicate, ms = 2000) => {
  const deadline = Date.now() + ms;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error('waitFor timeout');
    await sleep(5);
  }
};

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
  await assert.rejects(pending, /core_process_exit/);
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
  await assert.rejects(pending, /core_process_exit/);
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
  await assert.rejects(pending, /core_closed/);
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

test('streaming tool call forwards progress notifications to onChunk and unwraps the final payload', async () => {
  const proc = makeFakeProc();
  fakeCp.spawn = () => proc;
  const bridge = new ReadMDBridge({ extensionPath: '/fake-ext' });
  const chunks = ['第一段。', '第二段。', '第三段。'];
  const streamed = [];

  proc.stdin.write = (payload) => {
    const request = JSON.parse(payload);
    const token = request.params && request.params._meta && request.params._meta.progressToken;
    assert.ok(token, 'streaming request must carry _meta.progressToken');
    assert.equal(request.method, 'tools/call');
    assert.equal(request.params.name, 'readmd_ai_chat');
    let emitted = 0;
    const emitNext = () => {
      if (emitted < chunks.length) {
        proc.stdout.emit('data', Buffer.from(JSON.stringify({
          jsonrpc: '2.0',
          method: 'notifications/progress',
          params: { progressToken: token, progress: emitted + 1, message: chunks[emitted] },
        }) + '\n'));
        emitted += 1;
        setTimeout(emitNext, 5);
      } else {
        proc.stdout.emit('data', Buffer.from(JSON.stringify({
          jsonrpc: '2.0',
          id: request.id,
          result: { content: [{ type: 'text', text: JSON.stringify({ ok: true, content: chunks.join(''), usage: { total_tokens: 9 } }) }] },
        }) + '\n'));
      }
    };
    setTimeout(emitNext, 5);
    return true;
  };

  const pending = bridge.aiChatStreaming(
    { provider: 'custom:test', credential_id: 'cred:abc12345', model: 'mock', skill_id: 'readmd-summary', markdown_content: '# 文档', stream: true },
    (text) => { streamed.push(text); });
  await sleep();
  proc.emit('spawn');
  const result = await pending;

  assert.deepEqual(streamed, chunks);
  assert.equal(result.ok, true);
  assert.equal(result.content, chunks.join(''));
  assert.deepEqual(result.usage, { total_tokens: 9 });
  bridge.dispose();
});

test('cancellation rejects with ai_cancelled, notifies the core, and ignores the late response', async () => {
  const proc = makeFakeProc();
  fakeCp.spawn = () => proc;
  const bridge = new ReadMDBridge({ extensionPath: '/fake-ext' });
  let cancellationCb = null;
  const token = {
    onCancellationRequested: (cb) => {
      cancellationCb = cb;
      return { dispose: () => { cancellationCb = null; } };
    },
  };
  const writes = [];
  let respondLater = null;

  proc.stdin.write = (payload) => {
    const msg = JSON.parse(payload);
    writes.push(msg);
    if (msg.method === 'notifications/cancelled') return true;
    let emitted = 0;
    const emitNext = () => {
      if (emitted < 3) {
        proc.stdout.emit('data', Buffer.from(JSON.stringify({
          jsonrpc: '2.0',
          method: 'notifications/progress',
          params: { progressToken: msg.params._meta.progressToken, progress: emitted + 1, message: 'chunk-%03d'.replace('%03d', String(emitted).padStart(3, '0')) },
        }) + '\n'));
        emitted += 1;
        setTimeout(emitNext, 5);
      } else {
        respondLater = () => proc.stdout.emit('data', Buffer.from(JSON.stringify({
          jsonrpc: '2.0',
          id: msg.id,
          result: { content: [{ type: 'text', text: JSON.stringify({ ok: true, content: 'full' }) }] },
        }) + '\n'));
      }
    };
    setTimeout(emitNext, 5);
    return true;
  };

  const streamed = [];
  const pending = bridge.aiChatStreaming(
    { provider: 'custom:test', credential_id: 'cred:abc12345', model: 'mock', skill_id: 'readmd-summary', markdown_content: '# 文档', stream: true },
    (text) => { streamed.push(text); }, token);
  await sleep();
  proc.emit('spawn');
  await waitFor(() => cancellationCb && streamed.length >= 3 && respondLater);

  cancellationCb();
  await assert.rejects(pending, /ai_cancelled/);

  const cancelledNote = writes.find(w => w.method === 'notifications/cancelled');
  assert.ok(cancelledNote, 'bridge must send notifications/cancelled to the core');
  assert.equal(cancelledNote.params.requestId, writes[0].id);

  respondLater();
  await sleep();

  proc.stdin.write = (payload) => {
    const request = JSON.parse(payload);
    proc.stdout.emit('data', Buffer.from(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { ok: true } }) + '\n'));
    return true;
  };
  assert.deepEqual(await bridge.callMcpMethod('tools/list'), { ok: true });
  bridge.dispose();
});
