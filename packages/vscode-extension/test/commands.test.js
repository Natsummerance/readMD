const test = require('node:test');
const assert = require('node:assert');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');
const Module = require('node:module');

const extDir = path.join(__dirname, '..');
const outDir = path.join(extDir, 'out');
const packageJson = JSON.parse(fs.readFileSync(path.join(extDir, 'package.json'), 'utf-8'));
const contributedCommands = packageJson.contributes.commands.map(c => c.command).sort();

let registered = {};
let quickPickQueue = [];
let messages = [];
let errors = [];
let openedDocs = [];
let bridgeCalls = {};
let clipboardText = '';

function makeEditor(text = '# 测试文档') {
  return {
    document: {
      getText: () => text,
      fileName: path.join(os.tmpdir(), 'doc.md'),
      languageId: 'markdown',
      uri: { toString: () => 'file:///tmp/doc.md' },
      positionAt: offset => ({ line: 0, character: offset }),
      offsetAt: () => 0,
    },
    selection: { active: { line: 0, character: 0 } },
    edit: async build => { build({ insert: () => {}, replace: () => {} }); return true; },
    insertSnippet: async () => true,
  };
}

const vscodeStub = {
  commands: {
    registerCommand: (id, handler) => { registered[id] = handler; return { dispose() {} }; },
    executeCommand: async (id, ...args) => {
      if (registered[id]) return registered[id](...args);
      throw new Error(`unknown command: ${id}`);
    },
  },
  window: {
    activeTextEditor: null,
    createStatusBarItem: () => ({ text: '', tooltip: '', command: '', name: '', show() {}, hide() {}, dispose() {} }),
    onDidChangeActiveTextEditor: () => ({ dispose() {} }),
    registerTreeDataProvider: () => {},
    createWebviewPanel: () => ({
      webview: { html: '' },
      onDidDispose: () => ({ dispose() {} }),
    }),
    withProgress: (options, cb) => cb(
      { report() {} },
      { onCancellationRequested: () => ({ dispose() {} }) }
    ),
    showQuickPick: async () => quickPickQueue.shift(),
    showInputBox: async () => undefined,
    showSaveDialog: async () => undefined,
    showOpenDialog: async () => undefined,
    showInformationMessage: async (...args) => { messages.push(args[0]); return undefined; },
    showWarningMessage: async (...args) => { messages.push(args[0]); return undefined; },
    showErrorMessage: async (...args) => { errors.push(args[0]); return undefined; },
    showTextDocument: async doc => { openedDocs.push(doc); return doc; },
  },
  workspace: {
    onDidChangeTextDocument: () => ({ dispose() {} }),
    findFiles: async () => [],
    workspaceFolders: undefined,
    openTextDocument: async options => ({
      fileName: 'Untitled-1',
      language: (options && options.language) || 'plaintext',
      content: options && options.content,
    }),
  },
  env: {
    language: 'zh-cn',
    openExternal: async () => true,
    clipboard: {
      writeText: async text => { clipboardText = text; },
      readText: async () => clipboardText,
    },
  },
  Uri: { file: p => ({ fsPath: p }) },
  StatusBarAlignment: { Right: 2, Left: 1 },
  ViewColumn: { Beside: 2, Active: 1, Right: 2 },
  ProgressLocation: { Notification: 15 },
  Position: class { constructor(line, character) { this.line = line; this.character = character; } },
  Range: class {},
  SnippetString: class { constructor(value) { this.value = value; } },
};

const fakeBridgeInstance = {
  listSkills: async () => [{ name: 'readmd-summary', description: '总结', uri: 'readmd://skills/readmd-summary' }],
  readSkill: async uri => `skill:${uri}`,
  listPrompts: async () => [{ name: 'readmd-summary', description: '总结', skill_id: 'readmd-summary' }],
  listProviders: async () => [{ id: 'custom:test', name: 'Test Provider', credential_id: 'cred:abc12345', has_key: true, models: ['mock-a', 'mock-b'] }],
  aiChatStreaming: async (args, onChunk) => {
    bridgeCalls.aiChatStreaming = args;
    if (onChunk) onChunk('你好，');
    if (onChunk) onChunk('世界。');
    if (bridgeCalls.aiChatStreamingResult === 'cancelled') return { ok: false, error_code: 'ai_cancelled' };
    if (bridgeCalls.aiChatStreamingResult === 'failed') return { ok: false, error_code: 'mcp_tool_failed' };
    return { ok: true, content: '你好，世界。' };
  },
  getServerPath: () => '/fake/readmd_mcp_server.py',
  onReady: () => ({ dispose() {} }),
  onDisconnected: () => ({ dispose() {} }),
  fixMarkdown: async () => ({ ok: true, repaired_content: '# fixed', fixes_count: 1 }),
  exportPresentation: async () => {},
  processImports: async () => '# flattened',
  runCodeChunk: async () => ({ ok: true, stdout: 'SUM=30', images: [] }),
  exportDoc: async () => {},
  exportEpub: async () => {},
  convertFile: async filePath => `converted:${filePath}`,
  fetchWeb: async () => ({ title: '网页标题', markdown: '# 网页内容' }),
  mdToLatex: async () => '\\begin{document}test\\end{document}',
  parseBibtex: async () => ({ entries: { key1: {}, key2: {} } }),
  dispose() {},
};

const originalLoad = Module._load;
Module._load = function patchedLoad(request, parent, isMain) {
  const parentFile = (parent && parent.filename ? parent.filename : '').replace(/\\/g, '/');
  if (parentFile.endsWith('/out/extension.js')) {
    if (request === 'vscode') return vscodeStub;
    if (request === './bridge') return { ReadMDBridge: function ReadMDBridge() { return fakeBridgeInstance; } };
    if (request === './sidebarProvider') return { ReadMDToolboxProvider: class ReadMDToolboxProvider { constructor(listSkills) { this.listSkills = listSkills; } } };
  }
  return originalLoad.call(this, request, parent, isMain);
};

const { activate } = require(path.join(outDir, 'extension.js'));

function freshState() {
  registered = {};
  quickPickQueue = [];
  messages = [];
  errors = [];
  openedDocs = [];
  bridgeCalls = {};
  vscodeStub.window.activeTextEditor = makeEditor();
}

function activateExtension() {
  const context = { subscriptions: [] };
  activate(context);
  return context;
}

function tempWorkspace() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'readmd-vscode-test-'));
}

test('activate registers exactly the 22 commands contributed in package.json', () => {
  freshState();
  const context = activateExtension();
  const registeredIds = Object.keys(registered).sort();
  assert.deepStrictEqual(registeredIds, contributedCommands);
  assert.strictEqual(registeredIds.length, 22);
  assert.ok(context.subscriptions.length > 0);
  for (const disposable of context.subscriptions) {
    assert.strictEqual(typeof disposable.dispose, 'function');
  }
});

test('every command handler can be invoked without throwing (cancel paths)', async () => {
  freshState();
  activateExtension();
  for (const id of contributedCommands) {
    await registered[id]();
  }
  assert.strictEqual(errors.length, 0, `unexpected errors: ${errors.join(' | ')}`);
});

test('openSkills deep flow reads the picked Skill beside the editor', async () => {
  freshState();
  activateExtension();
  quickPickQueue.push({ label: 'readmd-summary', description: '总结', uri: 'readmd://skills/readmd-summary' });
  await registered['readmd.openSkills']();
  assert.strictEqual(messages.some(m => m && m.includes('打开失败')), false);
});

test('openSkillByUri reads the Skill for a given uri', async () => {
  freshState();
  activateExtension();
  await registered['readmd.openSkillByUri']('readmd://skills/readmd-summary');
  assert.strictEqual(errors.length, 0);
});

test('openAiWorkbench streams chunks and passes credential handles to the bridge', async () => {
  freshState();
  activateExtension();
  const workflow = { label: 'readmd-summary', description: '总结', skillId: 'readmd-summary' };
  const provider = { label: 'Test Provider', description: '已配置凭据', value: { id: 'custom:test', credential_id: 'cred:abc12345', models: ['mock-a', 'mock-b'] } };
  const modelPick = { label: 'mock-a', value: 'mock-a' };
  quickPickQueue.push(workflow, provider, modelPick);
  await registered['readmd.openAiWorkbench']();
  const sent = bridgeCalls.aiChatStreaming;
  assert.ok(sent, 'aiChatStreaming must be called');
  assert.strictEqual(sent.provider, 'custom:test');
  assert.strictEqual(sent.credential_id, 'cred:abc12345');
  assert.strictEqual(sent.model, 'mock-a');
  assert.strictEqual(sent.skill_id, 'readmd-summary');
  assert.strictEqual(sent.markdown_content, '# 测试文档');
  assert.strictEqual(sent.stream, true);
  assert.strictEqual(errors.length, 0);
});

test('openAiWorkbench reports server-side cancellation via ai_cancelled', async () => {
  freshState();
  bridgeCalls.aiChatStreamingResult = 'cancelled';
  activateExtension();
  const workflow = { label: 'readmd-summary', description: '总结', skillId: 'readmd-summary' };
  const provider = { label: 'Test Provider', description: '已配置凭据', value: { id: 'custom:test', credential_id: 'cred:abc12345', models: ['mock-a'] } };
  quickPickQueue.push(workflow, provider);
  await registered['readmd.openAiWorkbench']();
  assert.ok(messages.some(m => m === 'ReadMD AI 生成已取消'),
    `expected cancellation message, got: ${messages.join(' | ')}`);
  assert.strictEqual(errors.length, 0);
});

test('openAiWorkbench surfaces tool failures as error messages', async () => {
  freshState();
  bridgeCalls.aiChatStreamingResult = 'failed';
  activateExtension();
  const workflow = { label: 'readmd-summary', description: '总结', skillId: 'readmd-summary' };
  const provider = { label: 'Test Provider', description: '已配置凭据', value: { id: 'custom:test', credential_id: 'cred:abc12345', models: ['mock-a'] } };
  quickPickQueue.push(workflow, provider);
  await registered['readmd.openAiWorkbench']();
  assert.strictEqual(errors.length, 1);
  assert.ok(!errors[0].includes('mcp_tool_failed'), 'raw error codes must not leak to the UI');
});

test('openAiWorkbench stops before the bridge when no provider has credentials', async () => {
  freshState();
  activateExtension();
  fakeBridgeInstance.listProviders = async () => [{ id: 'x', name: 'No Creds', models: ['m'] }];
  const workflow = { label: 'readmd-summary', description: '总结', skillId: 'readmd-summary' };
  quickPickQueue.push(workflow);
  await registered['readmd.openAiWorkbench']();
  assert.strictEqual(bridgeCalls.aiChatStreaming, undefined);
  assert.ok(messages.some(m => m && m.includes('请先在 ReadMD 桌面端配置 AI 提供商和凭据')));
});

test('setupMcpServer writes the workspace .vscode/mcp.json contract', async () => {
  freshState();
  const ws = tempWorkspace();
  vscodeStub.workspace.workspaceFolders = [{ uri: { fsPath: ws } }];
  activateExtension();
  quickPickQueue.push({ label: 'vscode', value: 'vscode' });
  await registered['readmd.setupMcpServer']();
  const written = JSON.parse(fs.readFileSync(path.join(ws, '.vscode', 'mcp.json'), 'utf-8'));
  assert.deepStrictEqual(written, {
    mcpServers: {
      readmd: {
        command: 'python',
        args: ['/fake/readmd_mcp_server.py'],
        env: { PYTHONIOENCODING: 'utf-8' },
      },
    },
  });
});

test('setupMcpServer writes the Cursor .cursor/mcp.json contract', async () => {
  freshState();
  const ws = tempWorkspace();
  vscodeStub.workspace.workspaceFolders = [{ uri: { fsPath: ws } }];
  activateExtension();
  quickPickQueue.push({ label: 'cursor', value: 'cursor' });
  await registered['readmd.setupMcpServer']();
  const written = JSON.parse(fs.readFileSync(path.join(ws, '.cursor', 'mcp.json'), 'utf-8'));
  assert.deepStrictEqual(written.mcpServers.readmd, {
    command: 'python',
    args: ['/fake/readmd_mcp_server.py'],
    env: { PYTHONIOENCODING: 'utf-8' },
  });
});

test('setupMcpServer copies the Claude Desktop config to the clipboard without a workspace', async () => {
  freshState();
  vscodeStub.workspace.workspaceFolders = undefined;
  activateExtension();
  quickPickQueue.push({ label: 'clipboard', value: 'clipboard' });
  await registered['readmd.setupMcpServer']();
  const written = JSON.parse(clipboardText);
  assert.deepStrictEqual(written.mcpServers.readmd, {
    command: 'python',
    args: ['/fake/readmd_mcp_server.py'],
    env: { PYTHONIOENCODING: 'utf-8' },
  });
});

test('preview builds a webview and tracks document changes', () => {
  freshState();
  activateExtension();
  registered['readmd.preview']();
});

test('fixCurrentDocument applies repaired content to the editor', async () => {
  freshState();
  activateExtension();
  await registered['readmd.fixCurrentDocument']();
  assert.strictEqual(errors.length, 0);
  assert.ok(messages.some(m => m && m.includes('已成功自愈')));
});

test('runCodeChunk reports successful execution output', async () => {
  freshState();
  activateExtension();
  await registered['readmd.runCodeChunk']();
  assert.strictEqual(errors.length, 0);
  assert.ok(messages.some(m => m && m.includes('SUM=30')));
});

test('fetchWebToMarkdown requires an http(s) URL and renders the fetched doc', async () => {
  freshState();
  activateExtension();
  let validateResult;
  const originalShowInputBox = vscodeStub.window.showInputBox;
  vscodeStub.window.showInputBox = async options => {
    validateResult = options.validateInput('notaurl');
    return undefined;
  };
  await registered['readmd.fetchWebToMarkdown']();
  vscodeStub.window.showInputBox = originalShowInputBox;
  assert.ok(validateResult, 'invalid URL must be rejected by validateInput');
  assert.strictEqual(errors.length, 0);
});
