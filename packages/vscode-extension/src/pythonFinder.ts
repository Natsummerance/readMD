import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';

/**
 * 智能自适应 Python 解释器探测器。
 * 优先级：
 * 1. VSCode Python 官方扩展 (ms-python.python) 当前活动环境
 * 2. 用户在 settings.json 中配置的 readmd.pythonPath
 * 3. 系统环境变量 PATH 中的 python / python3
 * 4. 常见系统安装默认路径
 */
export async function findPythonPath(): Promise<string> {
  // 1. 用户自定义配置
  const config = vscode.workspace.getConfiguration('readmd');
  const customPath = config.get<string>('pythonPath', '').trim();
  if (customPath && isValidPython(customPath)) {
    return customPath;
  }

  // 2. VSCode Python 官方扩展 API
  try {
    const pythonExtension = vscode.extensions.getExtension('ms-python.python');
    if (pythonExtension) {
      if (!pythonExtension.isActive) {
        await pythonExtension.activate();
      }
      const api = pythonExtension.exports;
      if (api?.environments?.getActiveEnvironmentPath) {
        const activeEnv = await api.environments.getActiveEnvironmentPath();
        if (activeEnv?.path && isValidPython(activeEnv.path)) {
          return activeEnv.path;
        }
      }
    }
  } catch {
    // 忽略扩展 API 读取失败，继续向下降级
  }

  // 3. 系统 PATH 检测
  const candidates = process.platform === 'win32'
    ? ['python', 'py', 'python3']
    : ['python3', 'python'];

  for (const cmd of candidates) {
    if (checkCommand(cmd)) {
      return cmd;
    }
  }

  // 4. 常见默认路径兜底
  if (process.platform === 'win32') {
    const localApp = process.env.LOCALAPPDATA || '';
    const installRoots = [
      localApp ? path.join(localApp, 'Programs') : '',
      process.env.ProgramFiles || '',
      process.env['ProgramFiles(x86)'] || '',
      process.env.SystemDrive || '',
    ].filter(Boolean);
    const versions = ['Python313', 'Python312', 'Python311', 'Python310'];
    const winCandidates = installRoots.flatMap(root => versions.map(version =>
      path.join(root, 'Python', version, 'python.exe')))
      .concat(installRoots.flatMap(root => versions.map(version =>
        path.join(root, version, 'python.exe'))));
    for (const p of winCandidates) {
      if (fs.existsSync(p)) return p;
    }
  }

  return 'python'; // 默认回退
}

function checkCommand(cmd: string): boolean {
  try {
    const res = cp.spawnSync(cmd, ['--version'], { timeout: 1500 });
    return res.status === 0;
  } catch {
    return false;
  }
}

function isValidPython(p: string): boolean {
  if (fs.existsSync(p)) {
    return true;
  }
  return checkCommand(p);
}
