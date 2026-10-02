// Offline renderer rebuild from tracked TS/JS, reusing the existing packaged
// PIXI/Cubism library chunks. Node 22.13+ strips TypeScript syntax.
// No package install, network request, generated-source patch or directory delete.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const adapter = path.join(root, 'packages/readmd-hermes-pet-adapter');
const argument = name => {
  const at = process.argv.indexOf(name);
  if (at < 0 || !process.argv[at + 1]) throw new Error(`${name} is required`);
  return path.resolve(process.argv[at + 1]);
};
const cache = argument('--renderer-cache');
const output = argument('--output');
if (cache === output) throw new Error('Renderer cache and output must differ');
const libraries = fs.readdirSync(path.join(cache, 'assets')).filter(name =>
  /^(pixi-|cubism4\.es-|display-|rolldown-runtime-).+\.js$/.test(name));
const pixi = libraries.find(name => name.startsWith('pixi-'));
const cubism = libraries.find(name => name.startsWith('cubism4.es-'));
if (!pixi || !cubism || libraries.length !== 4) throw new Error('Complete existing PIXI/Cubism cache is required');
fs.mkdirSync(path.join(output, 'assets'), { recursive: true });
for (const name of libraries) fs.copyFileSync(path.join(cache, 'assets', name), path.join(output, 'assets', name));
for (const name of fs.readdirSync(path.join(cache, 'assets')).filter(name => /-sprite\.(png|webp)$/.test(name))) {
  fs.copyFileSync(path.join(cache, 'assets', name), path.join(output, 'assets', name));
}
function compile(name) {
  return stripTypeScriptTypes(fs.readFileSync(path.join(adapter, 'src', name), 'utf8'), { mode: 'strip' });
}
let entry = compile('renderer.tsx');
entry = entry.replaceAll("import('./live2d/stage')", "import('./assets/stage.js')")
  .replaceAll("import('./live2d/bongo-classic')", "import('./assets/bongo-classic.js')")
  .replaceAll("import('./pet-life')", "import('./assets/pet-life.js')")
  .replaceAll("import('../../readmd-pet-rust/renderer/bongocat.js')", "import('./assets/bongocat.js')");
let stage = compile('live2d/stage.ts');
stage = stage.replace("import('pixi.js')", `import('./${pixi}')`)
  .replace("import('pixi-live2d-display/cubism4')", `import('./${cubism}')`);
fs.writeFileSync(path.join(output, 'entry.js'), entry);
fs.writeFileSync(path.join(output, 'assets/stage.js'), stage);
let classic = compile('live2d/bongo-classic.ts');
classic = classic.replace("from './stage'", "from './stage.js'")
  .replace("import('pixi.js')", `import('./${pixi}')`)
  .replace("import('pixi-live2d-display/cubism4')", `import('./${cubism}')`);
fs.writeFileSync(path.join(output, 'assets/bongo-classic.js'), classic);
fs.cpSync(path.join(root, 'packages/readmd-pet-rust/models/bongocat-standard'), path.join(output, '../models/bongocat-standard'), { recursive: true });
fs.writeFileSync(path.join(output, 'assets/pet-life.js'), compile('pet-life.ts'));
fs.copyFileSync(path.join(root, 'packages/readmd-pet-rust/renderer/bongocat.js'), path.join(output, 'assets/bongocat.js'));
fs.copyFileSync(path.join(root, 'packages/readmd-pet-rust/renderer/index.html'), path.join(output, 'index.html'));
console.log(`Offline pet renderer: ${output}`);
