#!/usr/bin/env node
// Zero-dependency "no Python" gate over tracked (and not-ignored new) files.
//   R1  a tracked `.py` file outside assets/upstream/** and assets/skills/**
//       (vendored Skill data is never executed);
//   R2  a Python invocation in an executable context: shell commands
//       (workflows, package.json scripts, Docker, sh/ps1/bat/cmd), workflow
//       declarations, Node/TS process spawns, Rust `Command::new`, MCP/extension
//       configs, and references to the deleted Python host.
// Markdown and .kiro/** are not scanned: docs may describe running Python
// code blocks. Exemptions live in tools/no-python.allow.json; every entry must
// match at least once, otherwise it is reported as stale.
// Usage: node tools/check-no-python.mjs [--report]   (--report never fails)
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { stripRustTests } from './check-assets.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// A command word at line start or after a separator, followed by whitespace,
// a quote, a closing paren or end of line.
export const SHELL_CMD = /(^|[\s;&|("'`])(python3?|pip3?|pipx|py(?=\s+-))(\.exe)?(?=[\s"'`)]|$)/;
const PY_ARG = /(^|[\s"'`=])[\w./\\-]+\.py(?=[\s"'`)]|$)/;
const SPAWN = /\b(?:spawn|spawnSync|exec|execSync|execFile|execFileSync)\(\s*['"`](?:python3?|py|pip3?)\b/;
const CMD_KEY = /\bcommand:\s*['"](?:python3?|pip3?)['"]/;
const PY_STRING = /\.py['"`]/;
const RUST_CMD = /Command::new\(\s*"(?:python3?|py|pip3?)(?:\.exe)?"/;
const RUST_PY = /\.py\b/;
const JSON_CMD = /"command"\s*:\s*"(?:python3?|py)(?:\.exe)?"/;
const HOST_REF = /(?:\bfrom\s+src\.|\bimport\s+src\.|\breadmd\.py\b|tools\/ui_server)/;

const EXCLUDED = [/^assets\/upstream\//, /^assets\/skills\//, /(^|\/)vendor\//, /(^|\/)node_modules\//, /(^|\/)dist\//, /^assets\/readmd\.boot\.js$/, /^\.kiro\//];

export function contextOf(file) {
  if (EXCLUDED.some(re => re.test(file))) return null;
  if (/^\.github\/workflows\/[^/]+\.ya?ml$/.test(file)) return 'workflow';
  if (/(^|\/)package\.json$/.test(file)) return 'package';
  if (/(^|\/)Dockerfile[^/]*$/.test(file) || /(^|\/)docker-compose[^/]*\.ya?ml$/.test(file)) return 'shell';
  if (/\.(sh|ps1|bat|cmd)$/i.test(file)) return 'shell';
  if (/\.(js|mjs|cjs|ts)$/.test(file)) return 'node';
  if (/\.rs$/.test(file) || /(^|\/)Cargo\.toml$/.test(file)) return 'rust';
  if (/^packages\/.*\.json$/.test(file)) return 'config';
  return null;
}

// Lines that are shell commands inside a workflow: `run:` values and the
// indented block that follows `run: |`.
function workflowRunLines(lines) {
  const out = [];
  let blockIndent = -1;
  lines.forEach((line, i) => {
    const indent = line.search(/\S/);
    if (blockIndent >= 0) {
      if (line.trim() === '' || indent > blockIndent) { out.push(i); return; }
      blockIndent = -1;
    }
    const m = /^(\s*)(?:-\s+)?run:\s*(.*)$/.exec(line);
    if (m) {
      if (/^[|>][-+]?\s*$/.test(m[2])) blockIndent = m[1].length + (line.trimStart().startsWith('-') ? 0 : 0);
      else out.push(i);
      if (blockIndent >= 0) blockIndent = indent;
    }
  });
  return new Set(out);
}

function packageScriptLines(text, lines) {
  const set = new Set();
  let scripts;
  try { scripts = JSON.parse(text).scripts || {}; } catch { return set; }
  const values = Object.values(scripts).filter(v => typeof v === 'string');
  lines.forEach((line, i) => {
    const m = /^\s*"[^"]+"\s*:\s*"(.*)",?\s*$/.exec(line);
    if (m && values.includes(JSON.parse('"' + m[1] + '"'))) set.add(i);
  });
  return set;
}

// Blank out `//` and `/* */` comments, keeping line numbers. String literals
// are skipped over so a "//" inside a URL is not taken as a comment.
export function stripComments(lines) {
  let inBlock = false;
  return lines.map(line => {
    let out = '';
    let i = 0;
    while (i < line.length) {
      if (inBlock) {
        const end = line.indexOf('*/', i);
        if (end < 0) break;
        inBlock = false;
        i = end + 2;
        continue;
      }
      const c = line[i];
      if (c === '"' || c === '`' || (c === "'" && /[\s(=,:[]/.test(line[i - 1] || ' '))) {
        let j = i + 1;
        while (j < line.length && line[j] !== c) j += line[j] === '\\' ? 2 : 1;
        out += line.slice(i, j + 1);
        i = j + 1;
        continue;
      }
      if (c === '/' && line[i + 1] === '/') break;
      if (c === '/' && line[i + 1] === '*') { inBlock = true; i += 2; continue; }
      out += c;
      i++;
    }
    return out;
  });
}

/** Scan one file; returns hits `{path, line, rule, text}` (line is 1-based). */
export function scanFile(file, text) {
  const hits = [];
  if (/\.py$/.test(file) && !/^assets\/(upstream|skills)\//.test(file)) {
    hits.push({ path: file, line: 1, rule: 'R1/tracked-py', text: file });
  }
  const ctx = contextOf(file);
  if (!ctx || text == null) return hits;
  const lines = text.split(/\r?\n/);
  // Rust and JS/TS: only code counts. Provenance comments such as
  // "port of `readmd.py:2388`" document history, and Rust test modules quote
  // the Python authority in assertion messages; neither runs anything.
  const src = ctx === 'rust' && file.endsWith('.rs') ? stripRustTests(text).split(/\r?\n/) : lines;
  const code = ctx === 'rust' || ctx === 'node' ? stripComments(src) : lines;
  const cmdLines = ctx === 'workflow' ? workflowRunLines(lines) : ctx === 'package' ? packageScriptLines(text, lines) : null;
  const add = (i, rule) => hits.push({ path: file, line: i + 1, rule, text: lines[i].trim() });
  code.forEach((line, i) => {
    if (HOST_REF.test(line)) { add(i, `R2/${ctx}/host-ref`); return; }
    switch (ctx) {
      case 'workflow':
        if (/uses:\s*actions\/setup-python/.test(line) || /shell:\s*python/.test(line)) add(i, 'R2/workflow/declaration');
        else if (cmdLines.has(i) && (SHELL_CMD.test(line) || PY_ARG.test(line))) add(i, 'R2/workflow/shell');
        break;
      case 'package':
        if (cmdLines.has(i) && (SHELL_CMD.test(line) || PY_ARG.test(line))) add(i, 'R2/package/shell');
        break;
      case 'shell':
        if (/^\s*(#|rem\b|::)/i.test(line)) break;
        if (SHELL_CMD.test(line) || PY_ARG.test(line)) add(i, 'R2/shell');
        break;
      case 'node':
        if (SPAWN.test(line) || CMD_KEY.test(line) || PY_STRING.test(line)) add(i, 'R2/node');
        break;
      case 'rust':
        if (RUST_CMD.test(line)) add(i, 'R2/rust/command');
        else if (RUST_PY.test(line)) add(i, 'R2/rust/py-ref');
        break;
      case 'config':
        if (JSON_CMD.test(line)) add(i, 'R2/config');
        break;
    }
  });
  return hits;
}

/**
 * Scan a set of files against an allowlist.
 * `allow` entries are `{path, pattern?, reason, requirement}`; `path` may end
 * in `/**` to cover a directory. Returns `{hits, stale}` where `hits` are the
 * unexempted hits and `stale` lists allow entries that matched nothing.
 */
export function scan(files, allow = []) {
  const used = new Set();
  const hits = [];
  for (const [file, text] of files) {
    for (const hit of scanFile(file, text)) {
      const idx = allow.findIndex(a => matchPath(a.path, hit.path) && (!a.pattern || new RegExp(a.pattern).test(hit.text)));
      if (idx >= 0) used.add(idx);
      else hits.push(hit);
    }
  }
  const stale = allow.filter((_, i) => !used.has(i));
  return { hits, stale };
}

function matchPath(pattern, file) {
  if (pattern.endsWith('/**')) return file.startsWith(pattern.slice(0, -2));
  return pattern === file;
}

function trackedFiles() {
  const raw = execFileSync('git', ['ls-files', '-co', '--exclude-standard', '-z'], { cwd: root });
  return raw.toString('utf8').split('\0').filter(Boolean).filter(f => fs.existsSync(path.join(root, f)));
}

function main(argv) {
  const report = argv.includes('--report');
  const allow = JSON.parse(fs.readFileSync(path.join(root, 'tools', 'no-python.allow.json'), 'utf8'));
  const files = new Map();
  for (const f of trackedFiles()) {
    const needsText = contextOf(f) !== null;
    files.set(f, needsText ? fs.readFileSync(path.join(root, f), 'utf8') : null);
  }
  const { hits, stale } = scan(files, allow);
  for (const h of hits) console.log(`${h.path}:${h.line}: ${h.rule}: ${h.text}`);
  for (const s of stale) console.log(`stale allowlist entry: ${s.path}${s.pattern ? ` /${s.pattern}/` : ''}`);
  const bad = hits.length + stale.length;
  if (!bad) console.log(`no-python check ok (${files.size} tracked files, ${allow.length} allowlist entries)`);
  return bad && !report ? 1 : 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
