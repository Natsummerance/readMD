#!/usr/bin/env node
/**
 * Auto-publish VSCode extension to Open VSX registry using %VSX_TOKEN% environment variable.
 */
const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const token = process.env.VSX_TOKEN || process.env.OVSX_PAT;

if (!token) {
  console.error('\x1b[31m[Error] Missing VSX_TOKEN environment variable.\x1b[0m');
  console.error('Please set VSX_TOKEN in your system/user environment variables or terminal session:');
  console.error('  Windows PowerShell: $env:VSX_TOKEN="ovsxat_..."');
  console.error('  Linux / macOS:     export VSX_TOKEN="ovsxat_..."');
  process.exit(1);
}

const pkgPath = path.join(__dirname, 'package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
const vsixName = `readmd-vscode-${pkg.version}.vsix`;
const vsixPath = path.join(__dirname, vsixName);

if (!fs.existsSync(vsixPath)) {
  console.log(`[Info] ${vsixName} not found, building package first...`);
  execSync('npm run package', { stdio: 'inherit', cwd: __dirname });
}

if (!fs.existsSync(vsixPath)) {
  console.error(`\x1b[31m[Error] Package file ${vsixName} still not found after build.\x1b[0m`);
  process.exit(1);
}

console.log(`\x1b[36m[Open VSX] Publishing ${vsixName} to Open VSX Registry...\x1b[0m`);

try {
  // Execute ovsx publish without echoing the secret token
  execSync(`npx --yes ovsx publish "${vsixPath}" -p "${token}"`, {
    stdio: 'inherit',
    cwd: __dirname
  });
  console.log(`\x1b[32m[Success] ${vsixName} published to Open VSX successfully!\x1b[0m`);
  console.log(`\x1b[34m[View at] https://open-vsx.org/extension/${pkg.publisher}/${pkg.name}\x1b[0m`);
} catch (error) {
  console.error('\x1b[31m[Error] Failed to publish to Open VSX.\x1b[0m');
  process.exit(1);
}
