import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const extensionRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(extensionRoot, '..', '..');
const target = path.join(extensionRoot, 'core');

// The extension talks to `readmd --mcp`.  By default the VSIX stays tiny and
// uses the installed ReadMD desktop app (or `readmd` on PATH).  Setting
// READMD_BIN=<path to a release readmd binary> produces a self-contained,
// platform-specific VSIX: the binary goes to core/bin/ with the assets the
// MCP tools read (Skills, provider catalog, export vendor files) beside it.
fs.rmSync(target, { recursive: true, force: true });
const bin = (process.env.READMD_BIN || '').trim();
if (!bin) {
  console.log('No READMD_BIN set: VSIX will use the installed ReadMD (`readmd --mcp`)');
  process.exit(0);
}
if (!fs.statSync(bin, { throwIfNoEntry: false })?.isFile()) {
  console.error(`READMD_BIN does not point to a file: ${bin}`);
  process.exit(1);
}
const binDir = path.join(target, 'bin');
fs.mkdirSync(binDir, { recursive: true });
const exe = process.platform === 'win32' || bin.toLowerCase().endsWith('.exe') ? 'readmd.exe' : 'readmd';
fs.copyFileSync(bin, path.join(binDir, exe));
fs.chmodSync(path.join(binDir, exe), 0o755);
for (const dir of ['skills', 'providers', 'upstream', 'vendor']) {
  const from = path.join(repoRoot, 'assets', dir);
  if (fs.existsSync(from)) fs.cpSync(from, path.join(binDir, 'assets', dir), { recursive: true });
}
console.log(`Staged ${exe} + MCP assets into core/bin for a self-contained VSIX`);
