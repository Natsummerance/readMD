#!/usr/bin/env node
// Zero-dependency i18n gate:
//   1. every code in rust/readmd-kernel/src/api_codes.rs has a key in en + zh-CN
//      (`error.<code>`, `note.<code>`, `warn.<code>`);
//   2. all locale files under assets/i18n share the same key set;
//   3. no hard-coded Chinese literal is shown through showToast(...), title= or
//      aria-label in assets/js (a fallback after `_t(...) ||` / `i18n.t(...) :`
//      is allowed).
// Usage: node tools/check-i18n.mjs            (exit 1 on any problem)
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const i18nDir = path.join(root, 'assets', 'i18n');
const problems = [];

// ---- 1. api codes --------------------------------------------------------
const codesSrc = fs.readFileSync(path.join(root, 'rust/readmd-kernel/src/api_codes.rs'), 'utf8');
const consts = new Map();
for (const m of codesSrc.matchAll(/pub const ([A-Z0-9_]+): &str = "([a-z0-9_]+)";/g)) consts.set(m[1], m[2]);
consts.set('TRANSCRIBE_UNAVAILABLE', 'transcribe_unavailable');
function listOf(name) {
  const m = new RegExp(`pub const ${name}: &\\[&str\\] = &\\[([^\\]]*)\\]`, 's').exec(codesSrc);
  if (!m) { problems.push(`api_codes.rs: ${name} not found`); return []; }
  return m[1].split(',').map(s => s.trim()).filter(Boolean).map(id => {
    if (!consts.has(id)) problems.push(`api_codes.rs: ${name} refers to unknown const ${id}`);
    return consts.get(id);
  }).filter(Boolean);
}
const load = f => JSON.parse(fs.readFileSync(path.join(i18nDir, f), 'utf8'));
const en = load('en.json');
const zh = load('zh-CN.json');
for (const [list, prefix] of [['ERROR_CODES', 'error'], ['NOTE_CODES', 'note'], ['WARN_CODES', 'warn']]) {
  for (const code of listOf(list)) {
    const key = `${prefix}.${code}`;
    if (!en[key]) problems.push(`en.json: missing ${key}`);
    if (!zh[key]) problems.push(`zh-CN.json: missing ${key}`);
  }
}

// ---- 2. identical key sets ----------------------------------------------
// meta.json is the locale index, not a locale.
const files = fs.readdirSync(i18nDir).filter(f => f.endsWith('.json') && f !== 'meta.json').sort();
const enKeys = new Set(Object.keys(en));
for (const f of files) {
  if (f === 'en.json') continue;
  const keys = new Set(Object.keys(load(f)));
  const missing = [...enKeys].filter(k => !keys.has(k));
  const extra = [...keys].filter(k => !enKeys.has(k));
  if (missing.length) problems.push(`${f}: ${missing.length} key(s) missing, e.g. ${missing.slice(0, 5).join(', ')}`);
  if (extra.length) problems.push(`${f}: ${extra.length} extra key(s), e.g. ${extra.slice(0, 5).join(', ')}`);
}

// ---- 3. hard-coded Chinese in user-visible calls --------------------------
const HAN = /[㐀-鿿]/;
function* jsFiles(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { if (e.name !== 'vendor') yield* jsFiles(p); }
    else if (e.name.endsWith('.js')) yield p;
  }
}
// A string literal is a fallback when the text right before it ends with
// `|| ` (after a `_t(...)`/`t(...)` call) or `: ` (ternary after `i18n ?`).
function isFallback(before) {
  if (/(\|\||\?\?)\s*\(?\s*$/.test(before)) return true;             // _t(k) || '回退'
  if (/i18n\s*\?[^;]*:\s*\(?\s*$/.test(before)) return true;         // i18n ? i18n.t(k) : '回退'
  if (/\b_?t\(\s*(['"`])[\w.-]+\1\s*,\s*$/.test(before)) return true; // _t(k, '回退')
  // Inside a parenthesised fallback: `_t(k) || ('前缀' + n + '后缀')`.
  let depth = 0;
  for (let i = before.length - 1; i >= 0; i--) {
    const c = before[i];
    if (c === ')') depth++;
    else if (c === '(') {
      if (depth === 0) return /(\|\||\?\?|:)\s*$/.test(before.slice(0, i)) && /\bi18n\b|\b_?t\(/.test(before.slice(0, i));
      depth--;
    }
  }
  return false;
}
const LITERAL = /(['"`])((?:\\.|(?!\1)[^\\\n])*)\1/g;
const scanDirs = [path.join(root, 'assets', 'js'), path.join(root, 'assets', 'app.js')];
for (const target of scanDirs) {
  const list = fs.statSync(target).isDirectory() ? [...jsFiles(target)] : [target];
  for (const file of list) {
    const rel = path.relative(root, file).split(path.sep).join('/');
    const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
    lines.forEach((line, i) => {
      if (/^\s*(\/\/|\*|\/\*)/.test(line)) return;
      const sinks = [];
      for (const m of line.matchAll(/showToast\(|\btitle\s*=|aria-label/g)) sinks.push(m.index);
      if (!sinks.length) return;
      for (const m of line.matchAll(LITERAL)) {
        if (!HAN.test(m[2]) || m.index < sinks[0]) continue;
        const before = line.slice(0, m.index);
        if (isFallback(before)) continue;
        // Attribute re-translated at runtime by i18n.applyTo (data-i18n-title / -aria).
        if (/(title|aria-label)=$/.test(before) && /data-i18n-(title|aria)=/.test(line.slice(m.index))) continue;
        // `${_t(k) || '回退'}` inside an attribute template: judge the interpolation only.
        if (m[2].includes('${') && !HAN.test(m[2].replace(/\$\{[^}]*\}/g, ''))) continue;
        // A literal inside `${...}` of a template is judged by what precedes it inside.
        problems.push(`${rel}:${i + 1}: untranslated literal ${m[0].slice(0, 40)}`);
      }
    });
  }
}

if (problems.length) {
  console.error(problems.join('\n'));
  console.error(`\ncheck-i18n: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log(`check-i18n: ok (${files.length} locales, ${enKeys.size} keys)`);
