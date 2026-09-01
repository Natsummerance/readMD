const test = require('node:test');
const assert = require('node:assert');
const path = require('node:path');
const Module = require('node:module');

const vscodeStub = {
  TreeItemCollapsibleState: { None: 0, Collapsed: 1, Expanded: 2 },
  TreeItem: class TreeItem {
    constructor(label, collapsibleState) { this.label = label; this.collapsibleState = collapsibleState; }
  },
  ThemeIcon: class ThemeIcon { constructor(id) { this.id = id; } },
  EventEmitter: class EventEmitter {
    constructor() { this.listeners = []; }
    get event() {
      const self = this;
      return (listener) => {
        self.listeners.push(listener);
        return { dispose: () => { const i = self.listeners.indexOf(listener); if (i >= 0) self.listeners.splice(i, 1); } };
      };
    }
    fire(payload) { for (const listener of [...this.listeners]) listener(payload); }
    dispose() { this.listeners = []; }
  },
};

const originalLoad = Module._load;
Module._load = function patchedLoad(request, parent, isMain) {
  const parentFile = (parent && parent.filename ? parent.filename : '').replace(/\\/g, '/');
  if (parentFile.endsWith('/out/sidebarProvider.js') && request === 'vscode') return vscodeStub;
  return originalLoad.call(this, request, parent, isMain);
};

const { buildSkillItems, ReadMDToolboxProvider } = require(path.join(__dirname, '..', 'out', 'sidebarProvider.js'));

test('buildSkillItems maps skills to open commands', () => {
  const items = buildSkillItems([
    { name: 'Fix Format', uri: 'skill://fix-format', description: '自愈修复' },
    { uri: 'skill://unnamed' },
  ]);
  assert.equal(items.length, 2);
  assert.equal(items[0].label, 'Fix Format');
  assert.equal(items[0].description, '自愈修复');
  assert.equal(items[0].contextValue, 'skill');
  assert.equal(items[0].command.command, 'readmd.openSkillByUri');
  assert.deepEqual(items[0].command.arguments, ['skill://fix-format']);
  assert.equal(items[1].label, 'skill://unnamed');
  assert.deepEqual(items[1].command.arguments, ['skill://unnamed']);
});

test('buildSkillItems shows a placeholder when no skills exist', () => {
  const items = buildSkillItems([]);
  assert.equal(items.length, 1);
  assert.match(items[0].label, /未发现/);
  assert.equal(items[0].command, undefined);
});

test('toolbox provider pulls the skills group through the injected bridge API', async () => {
  let calls = 0;
  const provider = new ReadMDToolboxProvider(async () => {
    calls += 1;
    return [{ name: 'S1', uri: 'skill://s1' }];
  });
  const root = await provider.getChildren();
  const skillsGroup = root.find(item => item.contextValue === 'group_skills');
  assert.ok(skillsGroup, 'skills group should be part of the root tree');
  const children = await provider.getChildren(skillsGroup);
  assert.equal(calls, 1);
  assert.equal(children[0].label, 'S1');
  const again = await provider.getChildren(skillsGroup);
  assert.equal(calls, 2);
  assert.equal(again[0].label, 'S1');
});

test('toolbox provider shows a readable error item when listing fails', async () => {
  const provider = new ReadMDToolboxProvider(async () => { throw new Error('core down'); });
  const root = await provider.getChildren();
  const skillsGroup = root.find(item => item.contextValue === 'group_skills');
  const children = await provider.getChildren(skillsGroup);
  assert.equal(children.length, 1);
  assert.match(children[0].label, /读取 Skill 失败/);
  assert.equal(children[0].contextValue, 'skill_error');
});
