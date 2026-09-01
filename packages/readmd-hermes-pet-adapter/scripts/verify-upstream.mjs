import crypto from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const source = path.join(root, 'third_party/hermes-agent-pet')
const metadata = fs.readFileSync(path.join(source, 'UPSTREAM.md'), 'utf8')
if (!metadata.includes('Pinned revision: `fb27614addac115d55299bc6538ae112fd01f688`')) throw new Error('unexpected Hermes snapshot')
for (const relative of ['apps/desktop/electron/pet-overlay-ipc.ts', 'apps/desktop/src/app/pet-overlay/pet-overlay-app.tsx', 'apps/desktop/src/components/pet/pet-sprite.tsx']) {
  const file = path.join(source, relative)
  if (!fs.existsSync(file)) throw new Error(`missing upstream source: ${relative}`)
  process.stdout.write(`${relative} ${crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')}\n`)
}
