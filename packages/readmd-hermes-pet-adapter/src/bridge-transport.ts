// ReadMD-owned transport. Atomic snapshots and durable, bounded FIFO commands.
import fs from 'node:fs'
import { randomUUID } from 'node:crypto'

const MAX_BYTES = 32 * 1024 * 1024
let sequence = 0

export function publishCommand(bridge: string, command: unknown): void {
  const directory = `${bridge}.commands`
  fs.mkdirSync(directory, { recursive: true })
  const queued = fs.readdirSync(directory).filter(name => name.endsWith('.json'))
  if (queued.length >= 128) throw new Error('pet_command_queue_full')
  const body = JSON.stringify({ command, created_at: Date.now() })
  const bytes = Buffer.byteLength(body)
  if (bytes > MAX_BYTES) throw new Error('pet_command_too_large')
  const pendingBytes = queued.reduce((size, name) => {
    try { return size + fs.statSync(`${directory}/${name}`).size } catch { return size }
  }, 0)
  if (pendingBytes + bytes > MAX_BYTES * 2) throw new Error('pet_command_queue_full')
  const id = `${Date.now().toString().padStart(16, '0')}-${(++sequence).toString().padStart(8, '0')}-${randomUUID()}`
  const target = `${directory}/${id}.json`
  const temp = `${target}.tmp`
  try {
    fs.writeFileSync(temp, body, { encoding: 'utf8', flag: 'wx' })
    fs.renameSync(temp, target)
  } finally {
    try { fs.unlinkSync(temp) } catch { /* already renamed */ }
  }
}

export class SnapshotReader {
  private signature = ''
  private reading = false
  constructor(private file: string) {}

  async read(): Promise<Record<string, unknown> | undefined> {
    if (this.reading) return
    this.reading = true
    try {
      const stat = await fs.promises.stat(this.file, { bigint: true })
      if (!stat.isFile() || stat.size > BigInt(MAX_BYTES)) throw new Error('invalid_pet_snapshot')
      const signature = `${stat.ino}:${stat.mtimeNs}:${stat.ctimeNs}:${stat.size}`
      if (signature === this.signature) return
      const value = JSON.parse(await fs.promises.readFile(this.file, 'utf8'))
      if (!value || typeof value !== 'object' || Array.isArray(value) || value.format_version !== 1) {
        throw new Error('unsupported_pet_snapshot')
      }
      // Parse failures never poison the change detector; a repaired file retries.
      this.signature = signature
      return value
    } finally { this.reading = false }
  }
}
