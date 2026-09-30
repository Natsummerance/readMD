#!/usr/bin/env node
// Style gate for assets/**/*.css (vendor/upstream excluded).
//
//   node tools/check-styles.mjs                   # fail on any regression
//   node tools/check-styles.mjs --report          # print counts, exit 0
//   node tools/check-styles.mjs --update-baseline # accept the current counts
//
// Rules (per file):
//   S1 hex colour literal outside tokens.css
//   S2 px font-size (use the rem --text-* scale)
//   S3 !important outside the allow list (.hidden, print, reduced motion, third-party overrides)
//   S4 @media width breakpoint other than 640 / 1024 (and legacy ones in the baseline)
//   S5 dead theme selectors (.theme-dark, body.dark) — themes are body[data-theme]
//   S6 outline:none outside :focus:not(:focus-visible) — blocks keyboard focus
//   S9 WCAG AA contrast of every declared theme pair in tokens.css
//
// S1–S4 and S6 are ratcheted: a file may never exceed its baseline count, and
// a count below the baseline also fails until the baseline is lowered, so
// progress is recorded.  S5 and S9 always block.
import fs from 'node:fs';
import path from 'node:path';

const here = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const root = path.resolve(here, '..');
const BASELINE = path.join(root, 'tools', 'styles-baseline.json');

export function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, '');
}

export function count(css, file) {
  const s = stripComments(css);
  const isTokens = /tokens\.css$/.test(file);
  const out = { S1: 0, S2: 0, S3: 0, S4: 0, S5: 0, S6: 0 };
  if (!isTokens) out.S1 = (s.match(/#[0-9a-fA-F]{3,8}\b(?![-\w])/g) || []).length;
  out.S2 = (s.match(/font-size\s*:\s*[\d.]+px/g) || []).length;
  // !important is fine on .hidden utilities, inside print / reduced-motion blocks,
  // and on third-party widget overrides.
  const blocks = splitRules(s);
  for (const { selector, body, at } of blocks) {
    const n = (body.match(/!important/g) || []).length;
    if (!n) continue;
    const allowed = /(^|[\s,])\.hidden\b/.test(selector) || /print|prefers-reduced-motion/.test(at) ||
      /\.(cm-|katex|mermaid|hljs)/.test(selector);
    if (!allowed) out.S3 += n;
  }
  for (const { selector, body } of blocks) {
    if (/outline\s*:\s*(none|0)\b/.test(body) && !/:focus:not\(:focus-visible\)/.test(selector) && !/\binput\b|textarea|\.cm-/.test(selector)) out.S6++;
  }
  for (const m of s.matchAll(/@media[^{]*/g)) {
    for (const w of m[0].matchAll(/(?:max|min)-width\s*:\s*(\d+)px/g)) {
      if (!['639', '640', '1023', '1024'].includes(w[1])) out.S4++;
    }
  }
  out.S5 = (s.match(/\.theme-dark\b|body\.dark\b/g) || []).length;
  return out;
}

/** Flat list of { at, selector, body } for every rule (one nesting level of @-blocks). */
export function splitRules(css) {
  const out = [];
  const walk = (text, at) => {
    let j = 0;
    while (j < text.length) {
      const open = text.indexOf('{', j);
      if (open < 0) break;
      const head = text.slice(j, open).trim();
      let depth = 1, k = open + 1;
      while (k < text.length && depth) { if (text[k] === '{') depth++; else if (text[k] === '}') depth--; k++; }
      const inner = text.slice(open + 1, k - 1);
      // `head` may carry a preceding `@import ...;` or stray `;` — keep the last statement.
      const sel = head.split(';').pop().trim();
      if (sel.startsWith('@')) walk(inner, at + ' ' + sel);
      else out.push({ at, selector: sel, body: inner });
      j = k;
    }
  };
  walk(css, '');
  return out;
}

// ---- WCAG contrast ------------------------------------------------------
const hexRgb = h => { h = h.replace('#', ''); if (h.length === 3) h = [...h].map(c => c + c).join(''); return [0, 2, 4].map(i => parseInt(h.slice(i, i + 2), 16) / 255); };
const lum = rgb => { const [r, g, b] = rgb.map(c => c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
export function contrast(a, b) {
  const [x, y] = [lum(hexRgb(a)), lum(hexRgb(b))].sort((m, n) => n - m);
  return (x + 0.05) / (y + 0.05);
}

/** Theme variable maps from tokens.css: `:root` then body[data-theme=...] overrides. */
export function themes(tokensCss) {
  const s = stripComments(tokensCss);
  const vars = body => Object.fromEntries([...body.matchAll(/--([\w-]+)\s*:\s*([^;]+);/g)].map(m => [m[1], m[2].trim()]));
  const base = {};
  const out = {};
  for (const { selector, body, at } of splitRules(s)) {
    if (at) continue;
    if (/^:root\b/.test(selector) || selector === ':root, body') Object.assign(base, vars(body));
    const m = selector.match(/^body\[data-theme="(\w+)"\]$/);
    if (m) out[m[1]] = Object.assign(out[m[1]] || {}, vars(body));
  }
  const all = { light: { ...base } };
  for (const [k, v] of Object.entries(out)) all[k] = { ...base, ...v };
  return all;
}

export const PAIRS = [
  ['fg', 'bg'], ['fg', 'bg2'], ['fg2', 'bg2'], ['fg3', 'bg'], ['fg3', 'bg3'],
  ['accent', 'bg2'], ['accent-fg', 'accent'], ['danger', 'bg2'], ['warning', 'bg2'], ['fg', 'code-bg'],
  ['color-success', 'bg2'],
];

export function contrastProblems(tokensCss, min = 4.5) {
  const problems = [];
  for (const [name, t] of Object.entries(themes(tokensCss))) {
    for (const [fg, bg] of PAIRS) {
      const a = t[fg], b = t[bg];
      if (!/^#[0-9a-f]{3,6}$/i.test(a || '') || !/^#[0-9a-f]{3,6}$/i.test(b || '')) continue;
      const r = contrast(a, b);
      if (r < min) problems.push(`S9 ${name}: --${fg} on --${bg} = ${r.toFixed(2)} < ${min}`);
    }
  }
  return problems;
}

function cssFiles(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { if (!['vendor', 'upstream', 'pet'].includes(e.name)) cssFiles(p, out); }
    else if (e.name.endsWith('.css')) out.push(p);
  }
  return out;
}

function main() {
  const report = process.argv.includes('--report');
  const update = process.argv.includes('--update-baseline');
  const files = cssFiles(path.join(root, 'assets'));
  const counts = {};
  for (const f of files) counts[path.relative(root, f).replace(/\\/g, '/')] = count(fs.readFileSync(f, 'utf8'), f);
  const blocking = [];
  const tokens = files.find(f => /tokens\.css$/.test(f));
  if (tokens) blocking.push(...contrastProblems(fs.readFileSync(tokens, 'utf8')));
  else blocking.push('assets/css/tokens.css is missing');
  for (const [f, c] of Object.entries(counts)) if (c.S5) blocking.push(`S5 ${f}: ${c.S5} dead theme selector(s)`);

  if (update) {
    fs.writeFileSync(BASELINE, JSON.stringify(counts, null, 2) + '\n');
    console.log(`check-styles: baseline updated (${files.length} files)`);
  }
  const baseline = fs.existsSync(BASELINE) ? JSON.parse(fs.readFileSync(BASELINE, 'utf8')) : {};
  const ratchet = [];
  for (const [f, c] of Object.entries(counts)) {
    const b = baseline[f] || {};
    for (const rule of ['S1', 'S2', 'S3', 'S4', 'S6']) {
      const limit = b[rule] ?? 0;
      if (c[rule] > limit) ratchet.push(`${rule} ${f}: ${c[rule]} > baseline ${limit}`);
      else if (c[rule] < limit) ratchet.push(`${rule} ${f}: ${c[rule]} < baseline ${limit} (lower the baseline: --update-baseline)`);
    }
  }
  if (report) {
    for (const [f, c] of Object.entries(counts)) console.log(f, JSON.stringify(c));
  }
  for (const p of [...blocking, ...ratchet]) console.log(p);
  if ((blocking.length || ratchet.length) && !report && !update) {
    console.error(`check-styles: ${blocking.length + ratchet.length} problem(s)`);
    process.exit(1);
  }
  console.log('check-styles: ' + (blocking.length || ratchet.length ? 'problems reported' : 'ok'));
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'))) main();
