import assert from 'node:assert/strict';
import test from 'node:test';
import { contextOf, scan, scanFile, stripComments, SHELL_CMD } from '../check-no-python.mjs';

const files = entries => new Map(entries);

test('R1 flags tracked .py outside vendored Skill data', () => {
  assert.equal(scanFile('tools/x.py', null)[0].rule, 'R1/tracked-py');
  assert.deepEqual(scanFile('assets/upstream/a/b/scripts/x.py', null), []);
  assert.deepEqual(scanFile('assets/skills/demo/x.py', null), []);
});

test('SHELL_CMD matches command words, not substrings', () => {
  for (const ok of ['python tools/x.py', 'cd a && python3 -m y', 'pip install z', 'py -3 x', '"python.exe" run']) {
    assert.ok(SHELL_CMD.test(ok), ok);
  }
  for (const no of ['pythonic', 'mypy check', 'numpy', 'copy -r', 'py.typed']) {
    assert.ok(!SHELL_CMD.test(no), no);
  }
});

test('workflow: only run blocks and setup-python count', () => {
  const wf = [
    'jobs:',
    '  a:',
    '    steps:',
    '      - uses: actions/setup-python@v5',
    '      - name: python docs are fine here',
    '        run: python tools/sync.py',
    '      - run: |',
    '          echo ok',
    '          pip install x',
    '      - name: after',
  ].join('\n');
  const rules = scanFile('.github/workflows/ci.yml', wf).map(h => `${h.line}:${h.rule}`);
  assert.deepEqual(rules, ['4:R2/workflow/declaration', '6:R2/workflow/shell', '9:R2/workflow/shell']);
});

test('package.json: only scripts values count', () => {
  const pj = JSON.stringify({ description: 'python helper', scripts: { a: 'python x.py', b: 'node y.mjs' } }, null, 2);
  const hits = scanFile('web/package.json', pj);
  assert.equal(hits.length, 1);
  assert.equal(hits[0].rule, 'R2/package/shell');
});

test('node and rust: comments are ignored, code is not', () => {
  const js = "// spawned python once\nconst p = spawn('python3', ['x']);\n/* 'a.py' */ run('b.py');";
  assert.deepEqual(scanFile('src/a.ts', js).map(h => h.line), [2, 3]);
  const rs = '//! port of `readmd.py:10`\nlet c = Command::new("python");\n#[cfg(test)]\nmod tests {\n    const X: &str = "a.py";\n}\n';
  assert.deepEqual(scanFile('rust/k/src/a.rs', rs).map(h => `${h.line}:${h.rule}`), ['2:R2/rust/command']);
});

test('stripComments keeps // inside strings', () => {
  assert.deepEqual(stripComments(['let u = "http://x"; // c']), ['let u = "http://x"; ']);
  assert.deepEqual(stripComments(['a /* b', 'c */ d']), ['a ', ' d']);
});

test('config and host references', () => {
  assert.equal(scanFile('packages/x/mcp.json', '{"command": "python", "args": []}')[0].rule, 'R2/config');
  assert.equal(scanFile('scripts/run.sh', 'exec ./readmd.py "$@"')[0].rule, 'R2/shell/host-ref');
  assert.deepEqual(scanFile('scripts/run.sh', '# python used to live here'), []);
  assert.equal(contextOf('docs/guide.md'), null);
  assert.equal(contextOf('assets/vendor/x.js'), null);
});

test('allowlist exempts by path and pattern, and reports stale entries', () => {
  const input = files([['scripts/u.bat', 'reg delete "Applications\\readmd.py"\npython x']]);
  const allow = [
    { path: 'scripts/u.bat', pattern: 'Applications', reason: 'legacy key' },
    { path: 'scripts/never.bat', reason: 'stale' },
  ];
  const { hits, stale } = scan(input, allow);
  assert.deepEqual(hits.map(h => h.line), [2]);
  assert.deepEqual(stale.map(s => s.path), ['scripts/never.bat']);
});
