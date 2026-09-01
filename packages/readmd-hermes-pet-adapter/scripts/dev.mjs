// Uses explicitly supplied tooling for local development.  Production builds
// install this package's declared dependencies; no path is hard-coded.
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const electron = process.env.READMD_PET_ELECTRON
if (!electron) throw new Error('READMD_PET_ELECTRON must point to Electron for a local preview')
const result = spawnSync(electron, [path.join(root, 'dist')], { stdio: 'inherit' })
process.exit(result.status ?? 1)
