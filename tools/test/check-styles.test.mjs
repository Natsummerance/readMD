import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { contrast, count, contrastProblems, themes } from '../check-styles.mjs';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')), '..', '..');

test('WCAG contrast matches known values', () => {
  assert.equal(contrast('#000', '#fff').toFixed(2), '21.00');
  assert.equal(contrast('#767676', '#fff').toFixed(2), '4.54');
  assert.equal(contrast('#2f5fe8', '#fcfcfb').toFixed(2), '5.21');
});

test('rules count violations and honour the allow list', () => {
  const css = `
    .a { color: #fff; font-size: 12px; }
    .b { color: var(--fg) !important; }
    .hidden { display: none !important; }
    @media print { .c { color: red !important; } }
    @media (max-width: 777px) { .d { color: var(--fg); } }
    @media (max-width: 640px) { .e { color: var(--fg); } }
    .theme-dark .f { color: var(--fg); }
    .g:focus { outline: none; }
    .h:focus:not(:focus-visible) { outline: none; }
    /* #abc in a comment is ignored */`;
  assert.deepEqual(count(css, 'x.css'), { S1: 1, S2: 1, S3: 1, S4: 1, S5: 1, S6: 1 });
  assert.equal(count('.a { color: #fff; }', 'assets/css/tokens.css').S1, 0);
});

test('every declared theme pair in tokens.css meets WCAG AA', () => {
  const tokens = fs.readFileSync(path.join(root, 'assets/css/tokens.css'), 'utf8');
  assert.deepEqual(Object.keys(themes(tokens)).sort(), ['dark', 'light', 'sepia']);
  assert.deepEqual(contrastProblems(tokens), []);
});

test('contrast gate catches a failing pair', () => {
  const css = ':root { --fg: #999999; --bg: #ffffff; }';
  assert.ok(contrastProblems(css).some(p => p.includes('--fg on --bg')));
});
