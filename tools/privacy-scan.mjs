#!/usr/bin/env node
// Zero-dependency privacy gate: fail when retired private AI seeds, likely
// plaintext API keys, hard-coded developer paths or real personal documents
// appear in the repository (tracked + non-ignored files), or in the release
// artifacts passed as arguments.
// Usage: node tools/privacy-scan.mjs [artifact-file-or-dir ...]
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// Split so this file does not match itself.
export const RETIRED = ['cc' + '-switch', 'xem' + '8k5', 'hot' + 'api', 'penguins' + 'aichat'];
export const KEY_PATTERNS = [/\bsk-[A-Za-z0-9_-]{20,}\b/, /\bAIza[0-9A-Za-z_-]{30,}\b/];
export const LOCAL_PATH_PATTERNS = [
  /[a-zA-Z]:[/\\](?:users|programming|project|workspace|home|desktop|downloads|natsumer|[a-zA-Z0-9_.-]+[/\\]\.(?:codex|gemini|antigravity))/i,
  /[a-zA-Z]:[/\\][a-zA-Z0-9_.-]+[/\\](?:skills|plugins|creator)[/\\]/i,
  /\/(?:Users|home)\/[a-zA-Z0-9_.-]+\/(?:\.codex|\.gemini|Programming|Projects)/i,
];
// Real personal documents must never enter the repo; a filename hit fails
// before any other early return.
export const SENSITIVE_NAME_PATTERNS = [/北京交通大学软件学院毕业实习文档/];

// Test fixtures that spell out obviously fake key-shaped strings.
const FAKE_KEY_FILES = new Set(['rust/readmd-kernel/src/ai.rs', 'rust/readmd-kernel/src/crypto.rs']);
// Test fixtures that use a synthetic install root (`C:/app/plugins/...`).
const FAKE_PATH_FILES = new Set(['rust/readmd-kernel/src/pet_host.rs']);
// The scanner's own pattern source and its unit-test fixtures.
const SELF = new Set(['tools/privacy-scan.mjs', 'tools/test/privacy-scan.test.mjs']);
// The public provider-catalog attribution is intentional and audited.
const CC_ATTRIBUTION = [
  'assets/providers/provider-catalog.json', 'provider-catalog.json', 'tools/check-assets.mjs',
  'tools/privacy-scan.mjs', 'source-snapshot-manifest.json', 'THIRD_PARTY_LICENSES.md',
  'candidate.json', 'SHA256SUMS.txt',
];
const SKIP_EXT = [
  '.png', '.ico', '.icns', '.lock', '.svg', '.woff', '.woff2', '.ttf', '.eot',
  '.exe', '.dll', '.pyd', '.pyc', '.dylib', '.so', '.zip', '.gz', '.bin', '.dat', '.obj', '.o', '.a',
  '.node', '.vsix', '.hap', '.deb', '.appimage', '.tar', '.xz', '.bz2', '.7z', '.pak', '.dmg',
  '.strings', '.nib', '.storyboardc', '.db', '.sqlite', '.sqlite3', '.jpg', '.jpeg', '.webp', '.gif',
  '.mp4', '.webm', '.pdf', '.docx', '.xlsx', '.pptx', '.epub',
];

const under = (label, dir) => label.startsWith(dir + '/') || label.includes('/' + dir + '/');

/** Scan one file's bytes. Returns a list of failure strings. */
export function scanBuffer(label, data, { sourceTree = true } = {}) {
  label = label.replace(/\\/g, '/');
  const failures = [];
  if (SENSITIVE_NAME_PATTERNS.some(re => re.test(label))) return [`sensitive real-document name in ${label}`];
  if (['assets/vendor', 'assets/upstream', 'tests', 'ui-tests', 'showcase'].some(d => under(label, d))) return failures;
  if (label.includes('verify-macos') || under(label, 'Contents') || label.startsWith('Contents/') || under(label, 'Frameworks')) return failures;
  const lower = label.toLowerCase();
  if (SKIP_EXT.some(ext => lower.endsWith(ext)) || label.endsWith('ReadMD')) return failures;
  const text = data.toString('latin1'); // byte-exact for ASCII patterns
  const low = text.toLowerCase();
  const attributed = CC_ATTRIBUTION.some(n => label === n || label.endsWith('/' + n));
  for (const token of RETIRED) {
    if (token === 'cc' + '-switch' && attributed) continue;
    if (low.includes(token)) failures.push(`retired provider marker in ${label}`);
  }
  if (!FAKE_KEY_FILES.has(label) && KEY_PATTERNS.some(re => re.test(text))) failures.push(`possible plaintext API key in ${label}`);
  if (sourceTree && !FAKE_PATH_FILES.has(label) && !SELF.has(label) &&!under(label, 'dist') && !label.startsWith('dist/')) {
    if (LOCAL_PATH_PATTERNS.some(re => re.test(text))) failures.push(`hardcoded local absolute path in ${label}`);
  }
  return failures;
}

function candidateFiles() {
  const raw = execFileSync('git', ['ls-files', '-co', '--exclude-standard', '-z'], { cwd: root });
  return raw.toString('utf8').split('\0').filter(Boolean);
}

function* walkExternal(target) {
  const abs = path.resolve(target);
  if (!fs.existsSync(abs)) return;
  if (fs.statSync(abs).isFile()) { yield [abs, path.basename(abs)]; return; }
  const stack = [abs];
  while (stack.length) {
    const dir = stack.pop();
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      if (['node_modules', '.git', '__pycache__', '_internal'].includes(e.name)) continue;
      const full = path.join(dir, e.name);
      if (e.isDirectory()) stack.push(full);
      else if (e.isFile()) yield [full, path.relative(abs, full)];
    }
  }
}

function main(args) {
  const failures = new Set();
  let count = 0;
  const entries = args.length
    ? args.flatMap(a => [...walkExternal(a)]).map(([p, l]) => [p, l, false])
    : candidateFiles().map(r => [path.join(root, r), r, true]);
  for (const [file, label, sourceTree] of entries) {
    let data;
    try { data = fs.readFileSync(file); } catch { continue; }
    count++;
    for (const f of scanBuffer(label, data, { sourceTree })) failures.add(f);
  }
  if (failures.size) {
    console.error('PRIVACY SCAN FAILED');
    for (const f of [...failures].sort()) console.error(f);
    return 1;
  }
  console.log(`privacy scan PASSED (${count} files)`);
  return 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
