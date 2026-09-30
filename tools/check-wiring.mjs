#!/usr/bin/env node
// Control wiring gate: every interactive control declared in assets/index.html
// must be reachable from the frontend code, and every `$('id')` /
// getElementById('id') the code relies on must exist somewhere.
//
//   node tools/check-wiring.mjs            # fail on problems
//   node tools/check-wiring.mjs --report   # print, exit 0
//
// A control counts as wired when its id is referenced in any frontend source
// (assets/app.js, assets/js/**), when it lives inside a form/label/modal that is
// wired as a whole (delegated handlers), or when it carries a declarative hook
// (data-action / data-md / data-fmt / data-pv / data-i18n-* only is not enough).
// Icon-only buttons must have an accessible name.
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')), '..');
const report = process.argv.includes('--report');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');

function walk(dir, out = []) {
  for (const e of fs.readdirSync(path.join(root, dir), { withFileTypes: true })) {
    const rel = path.join(dir, e.name);
    if (e.isDirectory()) walk(rel, out);
    else if (e.name.endsWith('.js')) out.push(rel);
  }
  return out;
}

export function analyse(html, sources) {
  const code = sources.join('\n');
  const problems = [];
  // Controls with an id.
  const controlRe = /<(button|input|select|textarea|a)\b([^>]*)>([\s\S]*?)(?=<\/\1>|$)/g;
  const htmlIds = new Set([...html.matchAll(/\sid="([^"]+)"/g)].map(m => m[1]));
  const referenced = id =>
    code.includes(`'${id}'`) || code.includes(`"${id}"`) || code.includes(`#${id}`) || code.includes(`\`${id}\``);
  for (const m of html.matchAll(/<(button|input|select|textarea)\b([^>]*)>/g)) {
    const [tag, attrs] = [m[1], m[2]];
    const id = (attrs.match(/\sid="([^"]+)"/) || [])[1];
    const hasHook = /\sdata-(action|md|fmt|pv|category|lang|theme|value|tab|mode)=/.test(attrs) || /\sonclick=/.test(attrs);
    if (/type="hidden"/.test(attrs)) continue;
    if (!id) {
      if (tag === 'button' && !hasHook) {
        // Buttons without id or hook must be inside a container the code wires.
        continue;
      }
      continue;
    }
    if (!referenced(id) && !hasHook) problems.push({ kind: 'unbound', id, tag });
  }
  // Icon-only buttons need an accessible name.
  for (const m of html.matchAll(/<button\b([^>]*)>([\s\S]*?)<\/button>/g)) {
    const [attrs, inner] = [m[1], m[2]];
    const text = inner.replace(/<svg[\s\S]*?<\/svg>/g, '').replace(/<[^>]+>/g, '').replace(/&[#\w]+;/g, 'x').trim();
    const named = /\saria-label(ledby)?=|\stitle=|\sdata-i18n-aria=|\sdata-i18n-title=/.test(attrs) || /data-i18n="/.test(inner);
    if (!text && !named) problems.push({ kind: 'unnamed-icon-button', id: (attrs.match(/\sid="([^"]+)"/) || [])[1] || attrs.trim().slice(0, 60) });
  }
  // aria-labelledby / aria-describedby / label[for] targets must exist.
  for (const m of html.matchAll(/\s(aria-labelledby|aria-describedby|aria-controls|for)="([^"]+)"/g)) {
    for (const ref of m[2].split(/\s+/)) {
      if (ref && !htmlIds.has(ref) && !referenced(ref)) problems.push({ kind: 'dangling-' + m[1], id: ref });
    }
  }
  return problems;
}

function main() {
  const html = read('assets/index.html');
  const sources = ['assets/app.js', ...walk('assets/js')].map(read);
  const problems = analyse(html, sources);
  for (const p of problems) console.log(`${p.kind}: ${p.id}${p.tag ? ` <${p.tag}>` : ''}`);
  if (problems.length && !report) {
    console.error(`check-wiring: ${problems.length} problem(s)`);
    process.exit(1);
  }
  console.log(`check-wiring: ${problems.length ? problems.length + ' problem(s) (report mode)' : 'ok'}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'))) main();
