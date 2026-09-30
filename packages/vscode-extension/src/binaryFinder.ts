import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';

/**
 * Locate the ReadMD executable that serves `readmd --mcp`.
 * Order:
 * 1. `readmd.executablePath` from settings.json
 * 2. A binary staged inside the VSIX (`core/bin/`)
 * 3. `readmd` / `ReadMD` on PATH
 * 4. Default install locations of the desktop app
 * Every candidate is probed with `--version` (3 s timeout).
 */
export function candidatePaths(extensionPath: string, env: NodeJS.ProcessEnv = process.env,
    platform: NodeJS.Platform = process.platform): string[] {
  const exe = platform === 'win32' ? 'readmd.exe' : 'readmd';
  const list: string[] = [path.join(extensionPath, 'core', 'bin', exe)];
  list.push(...(platform === 'win32' ? ['readmd', 'ReadMD'] : ['readmd']));
  if (platform === 'win32') {
    const roots = [
      env.LOCALAPPDATA ? path.join(env.LOCALAPPDATA, 'Programs') : '',
      env.ProgramFiles || '',
      env['ProgramFiles(x86)'] || '',
    ].filter(Boolean);
    for (const root of roots) list.push(path.join(root, 'ReadMD', 'ReadMD.exe'));
  } else if (platform === 'darwin') {
    list.push('/Applications/ReadMD.app/Contents/MacOS/ReadMD', '/Applications/ReadMD.app/Contents/MacOS/readmd');
    if (env.HOME) list.push(path.join(env.HOME, 'Applications', 'ReadMD.app', 'Contents', 'MacOS', 'ReadMD'));
  } else {
    list.push('/usr/bin/readmd', '/usr/local/bin/readmd', '/opt/readmd/ReadMD');
    if (env.HOME) list.push(path.join(env.HOME, '.local', 'bin', 'readmd'));
  }
  return list;
}

/** `readmd --version` exits 0 and prints `readmd-rust <version>`. */
export function probeBinary(candidate: string): boolean {
  if (path.isAbsolute(candidate) && !fs.existsSync(candidate)) return false;
  try {
    const res = cp.spawnSync(candidate, ['--version'], { timeout: 3000, windowsHide: true, encoding: 'utf8' });
    return res.status === 0 && /readmd/i.test(String(res.stdout || ''));
  } catch {
    return false;
  }
}

export async function findReadmdBinary(extensionPath: string): Promise<string> {
  const configured = vscode.workspace.getConfiguration('readmd').get<string>('executablePath', '').trim();
  if (configured) {
    if (probeBinary(configured)) return configured;
    throw new Error('readmd_binary_invalid');
  }
  for (const candidate of candidatePaths(extensionPath)) {
    if (probeBinary(candidate)) return candidate;
  }
  throw new Error('readmd_binary_not_found');
}
