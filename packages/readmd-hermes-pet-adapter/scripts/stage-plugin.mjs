// Creates the optional, self-contained Windows plugin layout consumed by
// HermesPetLauncher.  This is deliberately not part of the main ReadMD build:
// Electron stays out of the lightweight reader unless the user installs pet
// support explicitly.
import fs from 'node:fs'
import path from 'node:path'
import crypto from 'node:crypto'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '..')
const output = process.argv[2] ? path.resolve(process.argv[2]) : path.join(root, 'stage')
const electron = process.env.READMD_PET_ELECTRON_DIR || path.join(root, 'node_modules', 'electron', 'dist')
const app = path.join(root, 'dist')
if (process.platform !== 'win32') throw new Error('Windows staging must run on Windows')
if (!fs.existsSync(path.join(electron, 'electron.exe'))) throw new Error('Electron runtime is not installed')
if (!fs.existsSync(path.join(app, 'package.json'))) throw new Error('run npm run build first')
const outputName = path.basename(output).toLowerCase()
if (output === path.parse(output).root || output === root || !outputName.startsWith('readmd-pet-')) {
  throw new Error('output directory must be a dedicated readmd-pet-* folder')
}

const verify = spawnSync(process.execPath, [path.join(root, 'scripts', 'verify-upstream.mjs')], { stdio: 'inherit' })
if (verify.status !== 0) process.exit(verify.status ?? 1)
fs.rmSync(output, { force: true, recursive: true })
fs.mkdirSync(output, { recursive: true })
fs.cpSync(electron, output, { recursive: true })
fs.cpSync(app, path.join(output, 'app'), { recursive: true })
fs.copyFileSync(path.join(root, 'assets', 'NOTICE.md'), path.join(output, 'NOTICE.md'))
const files = []
function collect(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const current = path.join(directory, entry.name)
    if (entry.isDirectory()) collect(current)
    else if (entry.isFile()) {
      files.push({
        path: path.relative(output, current).replaceAll('\\', '/'),
        sha256: crypto.createHash('sha256').update(fs.readFileSync(current)).digest('hex')
      })
    }
  }
}
collect(output)
files.sort((left, right) => left.path.localeCompare(right.path))
fs.writeFileSync(path.join(output, 'readmd-pet-plugin.json'), JSON.stringify({
  format_version: 1,
  id: 'readmd-hermes-pet',
  optional: true,
  upstream: { repository: 'https://github.com/NousResearch/hermes-agent', revision: 'fb27614addac115d55299bc6538ae112fd01f688', license: 'MIT' },
  files
}, null, 2))
console.log(JSON.stringify({ output, runtime: 'electron.exe', app: 'app', manifest: 'readmd-pet-plugin.json', optional: true }))
