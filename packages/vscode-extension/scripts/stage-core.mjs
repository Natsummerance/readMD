import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const extensionRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(extensionRoot, '..', '..');
const target = path.join(extensionRoot, 'core');

// Build-only staging: VSIX consumers get the same canonical Python core and
// Skills, while the repository keeps one source implementation.
fs.rmSync(target, { recursive: true, force: true });
fs.mkdirSync(path.join(target, 'mcp-server'), { recursive: true });
const sourceFilter = source => !source.includes(`${path.sep}__pycache__`) && !source.endsWith('.pyc');
fs.cpSync(path.join(repoRoot, 'src'), path.join(target, 'src'), { recursive: true, filter: sourceFilter });
fs.cpSync(path.join(repoRoot, 'assets', 'skills'), path.join(target, 'assets', 'skills'), { recursive: true, filter: sourceFilter });
fs.cpSync(path.join(repoRoot, 'assets', 'providers'), path.join(target, 'assets', 'providers'), { recursive: true, filter: sourceFilter });
fs.copyFileSync(path.join(repoRoot, 'packages', 'mcp-server', 'readmd_mcp_server.py'),
  path.join(target, 'mcp-server', 'readmd_mcp_server.py'));
console.log('Staged canonical ReadMD Core + Skills for VSIX');
