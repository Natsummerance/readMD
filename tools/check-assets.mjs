#!/usr/bin/env node
// Zero-dependency consistency checks for data files under assets/.
//   1. assets/upstream/manifest.json lists every vendored upstream file with
//      its size and SHA-256 (the read-only provenance allowlist);
//   2. assets/providers/provider-catalog.json is reproducible from the vendored
//      CC-SWITCH snapshot (source-only entries, no promotion fields);
//   3. runtime clients never embed an AI system prompt: those come from Skills.
// Usage: node tools/check-assets.mjs --check   (exit 1 on any problem)
//        node tools/check-assets.mjs --write   (regenerate 1 and 2)
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const UPSTREAM = path.join(root, 'assets', 'upstream');
const MANIFEST = path.join(UPSTREAM, 'manifest.json');
const CATALOG = path.join(root, 'assets', 'providers', 'provider-catalog.json');
const CC_COMMIT = '6243e20ad6f1835f9ac94ab39ea0eb62a6795bc0';
const CC_DIR = path.join(UPSTREAM, 'farion1231-cc-switch', CC_COMMIT);

const sha256 = buf => crypto.createHash('sha256').update(buf).digest('hex');
const rel = p => path.relative(root, p).split(path.sep).join('/');
const byCodepoint = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
// Path order of the checked-in manifest: part by part, case-insensitive.
function byPathParts(a, b) {
  const pa = a.toLowerCase().split('/');
  const pb = b.toLowerCase().split('/');
  for (let i = 0; i < Math.min(pa.length, pb.length); i++) {
    const c = byCodepoint(pa[i], pb[i]);
    if (c) return c;
  }
  return pa.length - pb.length;
}

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === '__pycache__' || entry.name === 'node_modules') continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.isFile()) out.push(full);
  }
  return out;
}

// ---- 1. upstream manifest ------------------------------------------------
export function buildManifest() {
  const files = walk(UPSTREAM)
    .filter(p => p !== MANIFEST)
    .map(p => ({ path: rel(p), bytes: fs.statSync(p).size, sha256: sha256(fs.readFileSync(p)) }))
    .sort((a, b) => byPathParts(a.path, b.path));
  const sources = new Map();
  for (const f of files) {
    const parts = f.path.split('/');
    if (parts.length < 4) continue;
    const id = parts.slice(2, 4).join('/');
    const item = sources.get(id) || { id, files: 0, bytes: 0 };
    item.files += 1;
    item.bytes += f.bytes;
    sources.set(id, item);
  }
  return {
    schema_version: 1,
    purpose: 'ReadMD offline upstream source allowlist; files are immutable snapshots',
    sources: [...sources.values()].sort((a, b) => byCodepoint(a.id, b.id)),
    files,
  };
}

export function diffManifest(expected, actual) {
  const problems = [];
  const want = new Map((expected.files || []).map(f => [f.path, f]));
  const have = new Map(actual.files.map(f => [f.path, f]));
  const missing = [...want.keys()].filter(k => !have.has(k));
  const added = [...have.keys()].filter(k => !want.has(k));
  if (missing.length || added.length) {
    problems.push(`upstream manifest file set mismatch; missing=${JSON.stringify(missing.slice(0, 5))} added=${JSON.stringify(added.slice(0, 5))}`);
  }
  for (const [p, item] of want) {
    const got = have.get(p);
    if (got && (got.bytes !== item.bytes || got.sha256 !== item.sha256)) problems.push(`upstream manifest hash mismatch: ${p}`);
  }
  return problems;
}

// ---- 2. provider catalog -------------------------------------------------
const STRING = `["']([^"']{1,300})["']`;
const NAME_RE = new RegExp(String.raw`\bname\s*:\s*` + STRING, 'g');
const FIELDS = {
  base_url: new RegExp(String.raw`\b(?:baseUrl|base_url|apiUrl|apiBaseUrl|websiteUrl)\s*:\s*` + STRING),
  format: new RegExp(String.raw`\b(?:format|protocol|providerType)\s*:\s*` + STRING, 'i'),
};

export function extractProviders(file, text) {
  // Python's universal-newline read: CRLF and lone CR become LF.
  text = text.replace(/\r\n?/g, '\n');
  const cps = Array.from(text); // index by code point, as the original did
  const hash = sha256(fs.readFileSync(file));
  const base = path.basename(file);
  const stem = base.replace(/\.[^.]*$/, '');
  const entries = [];
  let index = -1;
  for (const m of text.matchAll(NAME_RE)) {
    index += 1;
    const name = m[1].trim();
    if (!name || name === 'string' || name === 'name') continue;
    const startCp = Array.from(text.slice(0, m.index)).length;
    const endCp = startCp + Array.from(m[0]).length;
    const line = text.slice(0, m.index).split('\n').length;
    const nearby = cps.slice(endCp, endCp + 1800).join('');
    const fields = {};
    for (const [field, re] of Object.entries(FIELDS)) {
      const hit = re.exec(nearby);
      if (hit) fields[field] = hit[1].trim();
    }
    let fmt = (fields.format || 'openai').toLowerCase();
    if (fmt.includes('anthropic') || base.toLowerCase().includes('claude')) fmt = 'anthropic';
    else if (fmt.includes('gemini') || base.toLowerCase().includes('gemini')) fmt = 'gemini';
    else fmt = 'openai';
    let baseUrl = fields.base_url || '';
    if (!baseUrl.startsWith('http://') && !baseUrl.startsWith('https://')) baseUrl = '';
    entries.push({
      id: `cc-switch:${stem}:${String(index).padStart(4, '0')}`,
      name,
      base_url: baseUrl,
      format: fmt,
      models: [],
      category: 'upstream',
      source_only: true,
      source_ref: `assets/upstream/farion1231-cc-switch/${CC_COMMIT}/src/config/${base}`,
      source_sha256: hash,
      source_line: line,
      upstream_commit: CC_COMMIT,
      capabilities: { chat: true, stream: true, models: false },
      adaptation_notes: [
        'ReadMD stores this entry as an offline reference; promotion and affiliate fields are not runtime configuration.',
        'Complete endpoint and credentials are selected by the ReadMD Provider v3 editor.',
      ],
    });
  }
  return entries;
}

export function buildCatalog() {
  const configDir = path.join(CC_DIR, 'src', 'config');
  if (!fs.existsSync(configDir)) throw new Error(`vendored CC-SWITCH snapshot is missing: ${rel(CC_DIR)}`);
  const existing = fs.existsSync(CATALOG) ? JSON.parse(fs.readFileSync(CATALOG, 'utf8')) : {};
  const providers = (existing.providers || []).filter(p => p && typeof p === 'object' && p.name);
  const sources = fs.readdirSync(configDir).filter(n => n.endsWith('.ts')).sort(byCodepoint).map(n => path.join(configDir, n));
  return {
    schema_version: 2,
    source: 'https://github.com/farion1231/cc-switch',
    upstream_commit: CC_COMMIT,
    license: 'MIT',
    attribution: 'Provider catalog fields adapted from the offline CC-SWITCH snapshot; runtime excludes promotion and affiliate fields.',
    snapshot_manifest: 'assets/upstream/manifest.json',
    generated_by: 'tools/check-assets.mjs',
    providers,
    upstream_entries: sources.flatMap(f => extractProviders(f, fs.readFileSync(f, 'utf8'))),
  };
}

// ---- 3. prompt policy ----------------------------------------------------
const PROMPT_ROOTS = ['rust/readmd-kernel/src', 'packages', 'assets/js'];
const PROMPT_SKIP = new Set(['upstream', 'node_modules', 'dist', 'build', 'target', 'core']);
const PROMPT_SUFFIXES = new Set(['.rs', '.js', '.ts', '.tsx', '.ets', '.html']);
export const FORBIDDEN_PROMPTS = [
  /system_prompt\s*=\s*(['"]).{20,}/i,
  /['"]role['"]\s*:\s*['"]system['"]\s*,\s*['"]content['"]\s*:\s*(['"]).{20,}/i,
  /openAiPanelWithPrompt\s*\([^,]+,\s*`[^`]*(?:请将|请深度解析|You are|You must)/is,
];

// Blank out top-level `#[cfg(test)] mod … { … }` blocks (keeping line
// numbers): test fixtures legitimately spell out system messages. Sources are
// rustfmt-formatted, so a module ends at the next column-0 `}`.
export function stripRustTests(text) {
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    if (!/^#\[cfg\(test\)\]/.test(lines[i])) continue;
    let j = i + 1;
    while (j < lines.length && /^\s*(#\[|\/\/)/.test(lines[j])) j++;
    if (!/^(pub\s+)?mod\s+\w+\s*\{\s*$/.test(lines[j] || '')) continue;
    let k = j + 1;
    while (k < lines.length && !/^\}\s*$/.test(lines[k])) k++;
    for (let n = i; n <= k && n < lines.length; n++) lines[n] = '';
    i = k;
  }
  return lines.join('\n');
}

export function promptViolations(files) {
  const out = [];
  for (let [file, text] of files) {
    if (file.endsWith('.rs')) text = stripRustTests(text);
    for (const re of FORBIDDEN_PROMPTS) {
      const m = re.exec(text);
      if (m) out.push(`${file}:${text.slice(0, m.index).split('\n').length}: ${re.source}`);
    }
  }
  return out;
}

function promptFiles() {
  const files = new Map();
  for (const r of PROMPT_ROOTS) {
    for (const f of walk(path.join(root, r))) {
      const parts = rel(f).split('/');
      if (parts.some(p => PROMPT_SKIP.has(p))) continue;
      if (!PROMPT_SUFFIXES.has(path.extname(f).toLowerCase())) continue;
      files.set(rel(f), fs.readFileSync(f, 'utf8'));
    }
  }
  return files;
}

const encode = data => JSON.stringify(data, null, 2) + '\n';

function main(argv) {
  const write = argv.includes('--write');
  const problems = [];
  const manifest = buildManifest();
  const catalog = buildCatalog();
  if (write) {
    fs.writeFileSync(MANIFEST, encode(manifest));
    fs.writeFileSync(CATALOG, encode(catalog));
    console.log(`wrote ${rel(MANIFEST)} (${manifest.files.length} files)`);
    console.log(`wrote ${rel(CATALOG)} (${catalog.providers.length} presets, ${catalog.upstream_entries.length} upstream entries)`);
  } else {
    if (!fs.existsSync(MANIFEST)) problems.push('upstream manifest is missing');
    else problems.push(...diffManifest(JSON.parse(fs.readFileSync(MANIFEST, 'utf8')), manifest));
    const current = fs.existsSync(CATALOG) ? fs.readFileSync(CATALOG, 'utf8').replace(/\r\n/g, '\n') : '';
    if (current !== encode(catalog)) problems.push('provider catalog is stale; run node tools/check-assets.mjs --write');
  }
  problems.push(...promptViolations(promptFiles()));
  if (problems.length) {
    console.error('asset check failed:');
    for (const p of problems) console.error('  ' + p);
    return 1;
  }
  console.log(`asset check ok (${manifest.files.length} upstream files, ${catalog.upstream_entries.length} provider entries, prompts come from Skills)`);
  return 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
