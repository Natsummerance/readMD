import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '..')
const out = path.join(root, 'dist')
const generated = path.join(root, '.generated')
const upstreamOverlay = path.resolve(root, '../../third_party/hermes-agent-pet/apps/desktop/src/app/pet-overlay')
// A local install is used for packaging.  Maintainers can point this at an
// existing Hermes workspace for a zero-download, zero-copy runtime smoke test.
const tooling = process.env.READMD_PET_TOOLING_NODE_MODULES || path.join(root, 'node_modules')
const { build: viteBuild } = await import(pathToFileURL(path.join(tooling, 'vite', 'dist', 'node', 'index.js')).href)
const { build: esbuild } = await import(pathToFileURL(path.join(tooling, 'esbuild', 'lib', 'main.js')).href)

function generateHermesHostAdaptation() {
  // Keep the vendor snapshot immutable. This derivative is recreated on every
  // build, with one auditable change implementing the selected quick menu.
  const source = fs.readFileSync(path.join(upstreamOverlay, 'pet-overlay-app.tsx'), 'utf8')
  const original = '      setComposerOpen(open => !open)'
  const replacement = "      window.hermesDesktop?.petOverlay?.control({ type: 'open-menu' })"
  const occurrences = source.split(original).length - 1
  if (occurrences !== 1) throw new Error(`unexpected Hermes click-handler shape: ${occurrences}`)
  const header = [
    '// GENERATED FROM THE PINNED HERMES SOURCE SNAPSHOT. DO NOT EDIT.',
    '// ReadMD host adaptation: single click emits open-menu; double click remains toggle-app.',
    ''
  ].join('\n')
  fs.rmSync(generated, { force: true, recursive: true })
  fs.mkdirSync(generated, { recursive: true })
  fs.writeFileSync(path.join(generated, 'pet-overlay-app.tsx'), header + source.replace(original, replacement))
  fs.writeFileSync(path.join(generated, 'overlay-root.tsx'), header + fs.readFileSync(path.join(upstreamOverlay, 'overlay-root.tsx'), 'utf8'))
}

generateHermesHostAdaptation()
fs.rmSync(out, { force: true, recursive: true })
try {
  await viteBuild({ configFile: path.join(root, 'vite.config.mjs') })
  for (const [entry, outfile] of [['src/electron-main.ts', 'electron-main.cjs'], ['src/preload.ts', 'preload.cjs']]) {
    await esbuild({ bundle: true, entryPoints: [path.join(root, entry)], external: ['electron'], format: 'cjs', outfile: path.join(out, outfile), platform: 'node', target: 'node22' })
  }
  fs.mkdirSync(path.join(out, 'assets'), { recursive: true })
  fs.copyFileSync(path.join(root, 'assets', 'hermes-sprite.png'), path.join(out, 'assets', 'hermes-sprite.png'))
  fs.writeFileSync(path.join(out, 'package.json'), JSON.stringify({ main: 'electron-main.cjs', type: 'commonjs' }, null, 2))
} finally {
  fs.rmSync(generated, { force: true, recursive: true })
}
