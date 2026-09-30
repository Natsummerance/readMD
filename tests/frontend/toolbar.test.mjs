// Toolbar transform invariants (T-A4). Run: node --test tests/frontend/
import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { computeSyntaxEdit, applySyntaxEdit } = require('../../assets/js/editor/md-transforms.js');

function splitmix64(seed) {
  let s = BigInt(seed);
  return () => {
    s = (s + 0x9e3779b97f4a7c15n) & 0xffffffffffffffffn;
    let z = s;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & 0xffffffffffffffffn;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & 0xffffffffffffffffn;
    return Number((z ^ (z >> 31n)) & 0xffffffffn) / 0x100000000;
  };
}

const WORDS = ['alpha', 'beta', '中文', 'x', 'foo bar', '**b**', '`c`', '  indented', '- item', '1. one', '> q', '## h', ''];
function randomDoc(rnd) {
  const n = 1 + Math.floor(rnd() * 6);
  return Array.from({ length: n }, () => WORDS[Math.floor(rnd() * WORDS.length)]).join('\n');
}
function randomSel(rnd, doc) {
  const a = Math.floor(rnd() * (doc.length + 1));
  const b = rnd() < 0.3 ? a : Math.floor(rnd() * (doc.length + 1));
  return [Math.min(a, b), Math.max(a, b)];
}
const run = (doc, from, to, kind) => {
  const e = computeSyntaxEdit(doc, from, to, kind);
  return { e, doc: applySyntaxEdit(doc, e) };
};
function linesIn(doc, from, to) {
  const start = doc.lastIndexOf('\n', from - 1) + 1;
  let end = doc.indexOf('\n', to > from && doc[to - 1] === '\n' ? to - 1 : to);
  if (end < 0) end = doc.length;
  return doc.slice(start, end).split('\n');
}

test('block toggles apply to every selected line and undo on second press', () => {
  const rnd = splitmix64(0xC0FFEE);
  const prefix = { h2: /^\s*## /, quote: /^\s*>/, list: /^\s*- (?!\[)/, ordered: /^\s*\d+\. /, task: /^\s*- \[ \] / };
  for (let i = 0; i < 400; i++) {
    const doc = randomDoc(rnd);
    if (!doc.trim()) continue;
    const [from, to] = randomSel(rnd, doc);
    for (const kind of Object.keys(prefix)) {
      const once = run(doc, from, to, kind);
      const { anchor, head } = once.e.selection;
      const touched = anchor === head ? linesIn(once.doc, anchor, anchor) : linesIn(once.doc, Math.min(anchor, head), Math.max(anchor, head));
      const content = touched.filter(l => l.trim() && l.trim() !== '>');
      const allHad = linesIn(doc, from, to).filter(l => l.trim()).every(l => prefix[kind].test(l));
      if (!allHad) {
        for (const l of content) assert.match(l, prefix[kind], `${kind} on ${JSON.stringify(doc)} [${from},${to}] → ${JSON.stringify(once.doc)}`);
        const twice = run(once.doc, Math.min(anchor, head), Math.max(anchor, head), kind);
        const again = linesIn(twice.doc, 0, twice.doc.length).filter(l => l.trim());
        for (const l of again.slice(0)) {
          if (content.some(c => c.replace(prefix[kind], '') === l.replace(/^\s*/, ''))) {
            assert.doesNotMatch(l, kind === 'quote' ? /^\s*> / : prefix[kind]);
          }
        }
      }
    }
  }
});

test('hr never forms a setext heading and fences sit on their own lines', () => {
  const rnd = splitmix64(42);
  for (let i = 0; i < 400; i++) {
    const doc = randomDoc(rnd);
    const [from, to] = randomSel(rnd, doc);
    const hr = run(doc, from, to, 'hr').doc;
    const lines = hr.split('\n');
    lines.forEach((l, k) => {
      if (l === '---') {
        if (k > 0) assert.equal(lines[k - 1].trim(), '', `setext risk in ${JSON.stringify(hr)}`);
      }
    });
    assert.ok(lines.includes('---'));
    for (const kind of ['codeblock', 'mathblock']) {
      const e = computeSyntaxEdit(doc, from, to, kind);
      const out = applySyntaxEdit(doc, e);
      const ins = e.changes.insert.replace(/^\n+|\n+$/g, '');
      const fence = kind === 'mathblock' ? '$$' : ins.split('\n')[0];
      const start = out.indexOf(ins, e.changes.from);
      assert.ok(start === 0 || out[start - 1] === '\n', `${kind} fence not at line start in ${JSON.stringify(out)}`);
      const end = start + ins.length;
      assert.ok(end === out.length || out[end] === '\n');
      assert.ok(ins.startsWith(fence + '\n') && ins.endsWith('\n' + fence));
    }
  }
});

test('inline wrap toggles back to the original text', () => {
  const rnd = splitmix64(7);
  for (let i = 0; i < 400; i++) {
    const doc = randomDoc(rnd).replace(/[*`~$]/g, '');
    const [from, to] = randomSel(rnd, doc);
    const s = doc.slice(from, to);
    if (!s.trim() || s.includes('\n')) continue;
    for (const kind of ['bold', 'italic', 'strike', 'code']) {
      const once = run(doc, from, to, kind);
      const { anchor, head } = once.e.selection;
      const twice = run(once.doc, anchor, head, kind);
      assert.equal(twice.doc, doc, `${kind} on ${JSON.stringify(s)}`);
    }
  }
});

test('specific cases', () => {
  const cases = [
    ['a\nb\nc', 0, 5, 'ordered', '1. a\n2. b\n3. c'],
    ['- a\n- b', 0, 7, 'ordered', '1. a\n2. b'],
    ['1. a\n2. b', 0, 9, 'ordered', 'a\nb'],
    ['  a', 3, 3, 'list', '  - a'],
    ['## t', 2, 2, 'h2', 't'],
    ['# t', 2, 2, 'h2', '## t'],
    ['text', 4, 4, 'hr', 'text\n\n---'],
    ['a **b** c', 4, 5, 'bold', 'a b c'],
    ['a **b** c', 2, 7, 'bold', 'a b c'],
    ['a **b** c', 4, 5, 'italic', 'a ***b*** c'],
    ['x', 0, 1, 'code', '`x`'],
    ['a`b', 0, 3, 'code', '``a`b``'],
    ['x', 0, 1, 'codeblock', '```\nx\n```'],
    ['a\tb\nc\td', 0, 7, 'table', '| a | b |\n| --- | --- |\n| c | d |'],
    ['https://e.com', 0, 13, 'link', '[text](https://e.com)'],
    ['a\nb', 0, 3, 'quote', '> a\n> b'],
  ];
  for (const [doc, f, t, k, want] of cases) {
    assert.equal(run(doc, f, t, k).doc, want, `${k} ${JSON.stringify(doc)}`);
  }
  // one call = one change (single undo step)
  const e = computeSyntaxEdit('a\nb', 0, 3, 'list');
  assert.equal(typeof e.changes.insert, 'string');
});

// ---- Editor upgrade helpers (slash menu, tables, stats) ----
const T = require('../../assets/js/editor/md-transforms.js');

test('heading levels and callouts are single edits that toggle', () => {
  assert.equal(run('Title', 0, 0, 'h1').doc, '# Title');
  assert.equal(run('# Title', 3, 3, 'h1').doc, 'Title');
  assert.equal(run('## Title', 3, 3, 'h3').doc, '### Title');
  assert.equal(run('### a\n## b', 0, 10, 'para').doc, 'a\nb');
  assert.equal(run('', 0, 0, 'callout').doc, '> [!NOTE]\n> text');
  assert.equal(run('one\ntwo', 0, 7, 'callout').doc, '> [!NOTE]\n> one\n> two');
});

test('slash inserts replace the typed query in one change', () => {
  const apply = (doc, e) => T.applySyntaxEdit(doc, e);
  assert.equal(apply('/tab', T.blockInsertEdit('/tab', 0, 4, 'X')), 'X');
  assert.equal(apply('intro /tab', T.blockInsertEdit('intro /tab', 6, 10, 'X')), 'intro\n\nX');
  assert.equal(apply('a\n/x\nb', T.blockInsertEdit('a\n/x\nb', 2, 4, 'X')), 'a\n\nX\n\nb');
  assert.equal(apply('- foo /h2', T.linePrefixEdit('- foo /h2', 6, 9, '## ')), '## foo');
  assert.equal(apply('/h1', T.linePrefixEdit('/h1', 0, 3, '# ')), '# ');
  const fn = T.footnoteEdit('see /fn here', 4, 7);
  assert.equal(apply('see /fn here', fn), 'see [^1] here\n\n[^1]: ');
  const fn2 = T.footnoteEdit('a[^1]\n\n[^1]: x', 1, 1);
  assert.equal(apply('a[^1]\n\n[^1]: x', fn2), 'a[^2][^1]\n\n[^1]: x\n[^2]: ');
});

test('table of contents skips fenced code and nests by level', () => {
  const toc = T.tocMarkdown('# A\n## B c\n```\n# not\n```\n### D\n## B c');
  assert.equal(toc, '- [A](#a)\n  - [B c](#b-c)\n    - [D](#d)\n  - [B c](#b-c-2)');
  assert.equal(T.tocMarkdown('no headings'), '');
});

test('fuzzy score prefers prefixes and rejects non-matches', () => {
  assert.ok(T.fuzzyScore('tab', 'Table') > T.fuzzyScore('tab', 'Task list'));
  assert.ok(T.fuzzyScore('tbl', 'Table') > 0);
  assert.equal(T.fuzzyScore('zzz', 'Table'), 0);
  assert.equal(T.fuzzyScore('', 'Anything'), 1);
});

test('table navigation realigns, wraps rows and adds a row at the end', () => {
  const doc = '| a | bb |\n|---|:-:|\n| 1 | 2 |';
  const e1 = T.tableNavEdit(doc, 2, 1);
  const d1 = T.applySyntaxEdit(doc, e1);
  assert.equal(d1, '| a   | bb  |\n| --- | :-: |\n| 1   |  2  |');
  assert.equal(d1.slice(e1.selection.anchor, e1.selection.head), 'bb');
  const end = d1.length - 3;
  const e2 = T.tableNavEdit(d1, end, 1);
  const d2 = T.applySyntaxEdit(d1, e2);
  assert.equal(d2.split('\n').length, 4);
  const e3 = T.tableEnterEdit(d2, d2.length - 2);
  assert.equal(T.applySyntaxEdit(d2, e3), d1 + '\n');
  // CJK cells count as double width when aligning
  const cjk = T.formatTable(['| 名字 | x |', '|---|---|', '| a | b |']);
  assert.equal(cjk[0], '| 名字 | x   |');
  assert.equal(cjk[2], '| a    | b   |');
  assert.equal(T.tableNavEdit('not a table', 2, 1), null);
});

test('text stats count CJK characters as words', () => {
  assert.deepEqual(T.textStats(''), { words: 0, chars: 0, cjk: 0, minutes: 0 });
  const s = T.textStats('Hello world 你好世界 **bold** - ');
  assert.equal(s.words, 7);
  assert.equal(s.cjk, 4);
  assert.equal(s.minutes, 1);
});
