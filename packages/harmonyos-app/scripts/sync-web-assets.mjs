import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(projectRoot, '../..');
const sourceRoot = path.join(repositoryRoot, 'assets');
const targetRoot = path.join(
  projectRoot,
  'entry/src/main/resources/rawfile'
);

await fs.rm(targetRoot, { recursive: true, force: true });
await fs.cp(sourceRoot, targetRoot, { recursive: true });

// The desktop page uses server-root URLs. The bundled copy is relative.
const indexPath = path.join(targetRoot, 'index.html');
let html = await fs.readFile(indexPath, 'utf8');
html = html.replaceAll('"/assets/', '"assets/');
html = html.replaceAll("'/assets/", "'assets/");
await fs.writeFile(indexPath, html, 'utf8');

console.log(`Synced ReadMD web assets to ${path.relative(repositoryRoot, targetRoot)}`);
