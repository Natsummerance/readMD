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
let lastWebviewOptions = null;
let lastPostedMessages = [];

function makeEditor(text = '# 测试文档', version = 1) {
  return {
    document: {
      version,
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

const zhBundle = JSON.parse(fs.readFileSync(path.join(extDir, 'l10n', 'bundle.l10n.zh-cn.json'), 'utf-8'));
const enBundle = JSON.parse(fs.readFileSync(path.join(extDir, 'l10n', 'bundle.l10n.json'), 'utf-8'));

const vscodeStub = {
  l10n: {
    t: (opts, ...args) => {
      const key = opts && opts.key;
      const isZh = vscodeStub.env && typeof vscodeStub.env.language === 'string' && vscodeStub.env.language.startsWith('zh');
      const bundle = isZh ? zhBundle : enBundle;
      if (key && bundle[key]) {
        let msg = bundle[key];
        if (opts.args) {
          for (const [k, v] of Object.entries(opts.args)) {
            msg = msg.replace(new RegExp(`\\{${k}\\}`, 'g'), String(v));
          }
        }
        return msg;
      }
      if (typeof opts === 'string') return opts;
      if (opts && opts.message) return opts.message;
      return '';
    },
  },
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
    createWebviewPanel: (viewType, title, col, options) => {
      lastWebviewOptions = options;
      return {
        webview: {
          html: '',
          asWebviewUri: uri => ({ toString: () => `vscode-webview://${uri.fsPath}` }),
          postMessage: msg => lastPostedMessages.push(msg),
          onDidReceiveMessage: () => ({ dispose() {} }),
        },
        onDidDispose: () => ({ dispose() {} }),
      };
    },
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

const { activate, resolveMarkdownImages, getEnhancedWebviewContent, parseJsoncSafely } = require(path.join(outDir, 'extension.js'));

function freshState() {
  registered = {};
  quickPickQueue = [];
  messages = [];
  errors = [];
  openedDocs = [];
  bridgeCalls = {};
  lastWebviewOptions = null;
  lastPostedMessages = [];
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

test('openAiWorkbench aborts replacing selection if document version changed during generation', async () => {
  freshState();
  let editCalled = false;
  vscodeStub.window.activeTextEditor = {
    document: {
      version: 1,
      getText: () => '# 初始文档',
      fileName: '/tmp/doc.md',
      languageId: 'markdown',
      uri: { toString: () => 'file:///tmp/doc.md' },
      positionAt: offset => ({ line: 0, character: offset }),
      offsetAt: () => 0,
    },
    selection: { active: { line: 0, character: 0 } },
    edit: async cb => {
      editCalled = true;
      cb({ replace: () => {}, insert: () => {} });
      return true;
    },
  };
  const origAiChat = fakeBridgeInstance.aiChatStreaming;
  fakeBridgeInstance.aiChatStreaming = async (args, onChunk) => {
    // 模拟生成期间用户修改了文档
    vscodeStub.window.activeTextEditor.document.version = 2;
    if (onChunk) onChunk('AI生成内容');
    return { ok: true, content: 'AI生成内容' };
  };
  const origShowInfo = vscodeStub.window.showInformationMessage;
  vscodeStub.window.showInformationMessage = async (...args) => {
    messages.push(args[0]);
    return '替换选区'; // 用户点击替换选区
  };
  let warningShown = false;
  const origShowWarn = vscodeStub.window.showWarningMessage;
  vscodeStub.window.showWarningMessage = async (...args) => {
    warningShown = true;
    messages.push(args[0]);
    return undefined;
  };

  try {
    activateExtension();
    const workflow = { label: 'readmd-summary', description: '总结', skillId: 'readmd-summary' };
    const provider = { label: 'Test Provider', description: '已配置凭据', value: { id: 'custom:test', credential_id: 'cred:abc12345', models: ['mock-a'] } };
    quickPickQueue.push(workflow, provider);
    await registered['readmd.openAiWorkbench']();

    assert.strictEqual(editCalled, false, 'editor.edit must not be called when document version changed');
    assert.strictEqual(warningShown, true, 'Warning message must be shown');
    assert.ok(messages.some(m => m && m.includes('已变更')), 'Must notify user that document changed');
  } finally {
    fakeBridgeInstance.aiChatStreaming = origAiChat;
    vscodeStub.window.showInformationMessage = origShowInfo;
    vscodeStub.window.showWarningMessage = origShowWarn;
  }
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
    servers: {
      readmd: {
        command: 'python',
        args: ['/fake/readmd_mcp_server.py'],
        env: { PYTHONIOENCODING: 'utf-8' },
      },
    },
  });
});

test('setupMcpServer merges and preserves existing MCP server configurations', async () => {
  freshState();
  const ws = tempWorkspace();
  vscodeStub.workspace.workspaceFolders = [{ uri: { fsPath: ws } }];
  const targetDir = path.join(ws, '.vscode');
  fs.mkdirSync(targetDir, { recursive: true });
  fs.writeFileSync(path.join(targetDir, 'mcp.json'), JSON.stringify({
    servers: {
      existingServer: {
        command: 'node',
        args: ['/path/to/existing.js'],
      },
    },
    customSetting: 'preserved',
  }, null, 2), 'utf-8');

  activateExtension();
  quickPickQueue.push({ label: 'vscode', value: 'vscode' });
  await registered['readmd.setupMcpServer']();
  const written = JSON.parse(fs.readFileSync(path.join(targetDir, 'mcp.json'), 'utf-8'));
  assert.strictEqual(written.customSetting, 'preserved');
  assert.deepStrictEqual(written.servers.existingServer, {
    command: 'node',
    args: ['/path/to/existing.js'],
  });
  assert.deepStrictEqual(written.servers.readmd, {
    command: 'python',
    args: ['/fake/readmd_mcp_server.py'],
    env: { PYTHONIOENCODING: 'utf-8' },
  });
});

test('setupMcpServer safely parses JSONC with comments and trailing commas without wiping config', async () => {
  freshState();
  const ws = tempWorkspace();
  const cursorDir = path.join(ws, '.cursor');
  fs.mkdirSync(cursorDir, { recursive: true });
  const jsoncContent = `{\n  // Single line comment\n  "customProp": "http://example.com/test", // inline comment\n  /* Block\n     Comment */\n  "mcpServers": {\n    "existingTool": {\n      "command": "node",\n      "args": ["server.js", ], // trailing comma\n    },\n  },\n}`;
  fs.writeFileSync(path.join(cursorDir, 'mcp.json'), jsoncContent, 'utf-8');

  vscodeStub.workspace.workspaceFolders = [{ uri: { fsPath: ws } }];
  activateExtension();
  quickPickQueue.push({ label: 'cursor', value: 'cursor' });
  await registered['readmd.setupMcpServer']();

  const written = JSON.parse(fs.readFileSync(path.join(cursorDir, 'mcp.json'), 'utf-8'));
  assert.strictEqual(written.customProp, 'http://example.com/test');
  assert.deepStrictEqual(written.mcpServers.existingTool, {
    command: 'node',
    args: ['server.js'],
  });
  assert.ok(written.mcpServers.readmd, 'readmd config must be added');
});

test('setupMcpServer aborts and preserves file if JSON cannot be repaired', async () => {
  freshState();
  const ws = tempWorkspace();
  const cursorDir = path.join(ws, '.cursor');
  fs.mkdirSync(cursorDir, { recursive: true });
  const corrupted = 'NOT VALID JSON {{{ [[[ ';
  fs.writeFileSync(path.join(cursorDir, 'mcp.json'), corrupted, 'utf-8');

  vscodeStub.workspace.workspaceFolders = [{ uri: { fsPath: ws } }];
  activateExtension();
  quickPickQueue.push({ label: 'cursor', value: 'cursor' });
  await registered['readmd.setupMcpServer']();

  // File must NOT be replaced with empty or overwritten with only readmd
  assert.strictEqual(fs.readFileSync(path.join(cursorDir, 'mcp.json'), 'utf-8'), corrupted);
  assert.ok(errors.length > 0, 'Error must be surfaced to user');
  assert.ok(errors.some(e => e && e.includes('无法解析') || e.includes('Failed to parse')), 'Error message must explain parsing abort');
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

test('preview builds a webview and tracks document changes with localResourceRoots', () => {
  freshState();
  activateExtension();
  registered['readmd.preview']();
  assert.ok(lastWebviewOptions, 'Webview options must be passed');
  assert.ok(Array.isArray(lastWebviewOptions.localResourceRoots), 'localResourceRoots must be an array');
  assert.strictEqual(lastWebviewOptions.enableScripts, true);
  assert.strictEqual(lastWebviewOptions.retainContextWhenHidden, true);
});

test('getEnhancedWebviewContent escapes </script> inside markdown safely', () => {
  const malicious = '# Title\n\n```html\n<script>alert("hack")</script>\n```\n</script><script>window.pwned=true;</script>';
  const html = getEnhancedWebviewContent(malicious, 'Test Doc');
  // Must not contain unescaped raw </script> inside the script tag
  assert.ok(!html.includes('</script><script>window.pwned=true;</script>'), 'Unescaped script closing tags must not exist');
  assert.ok(html.includes('<\\/script>'), 'Script closing tags inside JSON must be escaped');
  assert.ok(html.includes('window.addEventListener(\'message\''), 'Webview must contain message event listener');
});

test('resolveMarkdownImages converts relative paths to asWebviewUri and preserves remote URLs', () => {
  const fakeWebview = {
    asWebviewUri: uri => ({ toString: () => `vscode-webview://assets/${path.basename(uri.fsPath)}` }),
  };
  const docDir = '/workspace/docs';
  const md = [
    '![local](./images/diagram.png)',
    '![remote](https://example.com/logo.png)',
    '![data](data:image/png;base64,abc123==)',
    '<img src="./images/graph.svg" alt="graph">',
    '<img src="https://cdn.example.com/img.jpg">',
  ].join('\n');

  const resolved = resolveMarkdownImages(md, docDir, fakeWebview);
  assert.ok(resolved.includes('![local](vscode-webview://assets/diagram.png)'));
  assert.ok(resolved.includes('![remote](https://example.com/logo.png)'));
  assert.ok(resolved.includes('![data](data:image/png;base64,abc123==)'));
  assert.ok(resolved.includes('<img src="vscode-webview://assets/graph.svg" alt="graph">'));
  assert.ok(resolved.includes('<img src="https://cdn.example.com/img.jpg">'));
});

test('parseJsoncSafely parses complex comments and strings with slashes', () => {
  const input = `
  {
    // Single line comment
    "url": "http://example.com/api?foo=//bar/*baz*/",
    /* Multi-line
       Comment */
    "items": [
      1,
      2, // inline comment
    ],
    "nested": {
      "key": "val",
    },
  }
  `;
  const result = parseJsoncSafely(input);
  assert.strictEqual(result.url, 'http://example.com/api?foo=//bar/*baz*/');
  assert.deepStrictEqual(result.items, [1, 2]);
  assert.deepStrictEqual(result.nested, { key: 'val' });
});

test('fixCurrentDocument applies repaired content to the editor', async () => {
  freshState();
  activateExtension();
  await registered['readmd.fixCurrentDocument']();
  assert.strictEqual(errors.length, 0);
  assert.ok(messages.some(m => m && m.includes('已成功自愈')));
});

test('fixCurrentDocument aborts replacement if document version changed during repair', async () => {
  freshState();
  let editCalled = false;
  vscodeStub.window.activeTextEditor = {
    document: {
      version: 1,
      getText: () => '# 原文',
      fileName: path.join(os.tmpdir(), 'doc.md'),
      languageId: 'markdown',
      uri: { toString: () => 'file:///tmp/doc.md' },
      positionAt: offset => ({ line: 0, character: offset }),
      offsetAt: () => 0,
    },
    selection: { active: { line: 0, character: 0 } },
    edit: async () => { editCalled = true; return true; },
    insertSnippet: async () => true,
  };
  const originalFix = fakeBridgeInstance.fixMarkdown;
  fakeBridgeInstance.fixMarkdown = async () => {
    vscodeStub.window.activeTextEditor.document.version = 2;
    return { ok: true, repaired_content: '# 修复文', fixes_count: 1 };
  };
  try {
    activateExtension();
    await registered['readmd.fixCurrentDocument']();
    assert.strictEqual(editCalled, false, 'editor.edit must not be called when document version changed');
    assert.ok(messages.some(m => m && m.includes('已变更')));
  } finally {
    fakeBridgeInstance.fixMarkdown = originalFix;
  }
});

test('exportPresentation opens file externally when user clicks the localized open button', async () => {
  freshState();
  let openedUri = null;
  vscodeStub.env.openExternal = async uri => { openedUri = uri; return true; };
  const origShowInfo = vscodeStub.window.showInformationMessage;
  vscodeStub.window.showInformationMessage = async (...args) => {
    messages.push(args[0]);
    return args[1]; // Simulate user clicking the button passed as args[1]
  };
  vscodeStub.window.showSaveDialog = async () => ({ fsPath: path.join(os.tmpdir(), 'pres.slides.html') });
  try {
    activateExtension();
    await registered['readmd.exportPresentation']();
    assert.ok(openedUri, 'openExternal must be called when user clicks the open button');
    assert.strictEqual(openedUri.fsPath, path.join(os.tmpdir(), 'pres.slides.html'));
  } finally {
    vscodeStub.window.showInformationMessage = origShowInfo;
  }
});

test('runCodeChunk reports successful execution output', async () => {
  freshState();
  activateExtension();
  await registered['readmd.runCodeChunk']();
  assert.strictEqual(errors.length, 0);
  assert.ok(messages.some(m => m && m.includes('SUM=30')));
});

test('runCodeChunk detects javascript fence and passes language to bridge', async () => {
  freshState();
  let passedCode = null;
  let passedLang = null;
  fakeBridgeInstance.runCodeChunk = async (code, lang) => {
    passedCode = code;
    passedLang = lang;
    return { ok: true, stdout: 'JS_OUT:42', images: [] };
  };
  const jsMd = '```javascript\nconsole.log(42);\n```';
  vscodeStub.window.activeTextEditor = {
    document: {
      getText: () => jsMd,
      fileName: '/tmp/test.md',
      languageId: 'markdown',
      uri: { toString: () => 'file:///tmp/test.md' },
      positionAt: () => ({ line: 1, character: 2 }),
      offsetAt: () => 18,
    },
    selection: { active: { line: 1, character: 2 } },
  };
  activateExtension();
  await registered['readmd.runCodeChunk']();
  assert.strictEqual(passedLang, 'javascript');
  assert.ok(passedCode && passedCode.includes('console.log(42)'));
  assert.ok(messages.some(m => m && m.includes('JS_OUT:42')));
});

test('runCodeChunk normalizes sh and py aliases', async () => {
  freshState();
  let passedLang = null;
  fakeBridgeInstance.runCodeChunk = async (code, lang) => {
    passedLang = lang;
    return { ok: true, stdout: 'OK' };
  };
  const shMd = '```sh\necho hello\n```';
  vscodeStub.window.activeTextEditor = {
    document: {
      getText: () => shMd,
      fileName: '/tmp/test.md',
      languageId: 'markdown',
      uri: { toString: () => 'file:///tmp/test.md' },
      positionAt: () => ({ line: 1, character: 2 }),
      offsetAt: () => 10,
    },
    selection: { active: { line: 1, character: 2 } },
  };
  activateExtension();
  await registered['readmd.runCodeChunk']();
  assert.strictEqual(passedLang, 'bash');
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

test('l10n bundle resolves English messages when vscode environment language is English', async () => {
  freshState();
  fakeBridgeInstance.listProviders = async () => [{ id: 'custom:test', name: 'Test Provider', credential_id: 'cred:abc12345', has_key: true, models: ['mock-a'] }];
  const prevLang = vscodeStub.env.language;
  vscodeStub.env.language = 'en';
  try {
    bridgeCalls.aiChatStreamingResult = 'cancelled';
    activateExtension();
    const workflow = { label: 'readmd-summary', description: 'summary', skillId: 'readmd-summary' };
    const provider = { label: 'Test Provider', description: 'Configured credentials', value: { id: 'custom:test', credential_id: 'cred:abc12345', models: ['mock-a'] } };
    quickPickQueue.push(workflow, provider);
    await registered['readmd.openAiWorkbench']();
    assert.ok(messages.some(m => m === 'ReadMD AI generation was cancelled'),
      `expected English cancellation message, got: ${messages.join(' | ')}`);
  } finally {
    vscodeStub.env.language = prevLang;
  }
});
