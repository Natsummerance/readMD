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
