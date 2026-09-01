// ReadMD-owned process/lifecycle adapter.  The overlay renderer and its IPC
// semantics stay in the immutable Hermes source snapshot.
import fs from 'node:fs'
import path from 'node:path'
import { app, BrowserWindow, clipboard, ipcMain } from 'electron'
import { registerPetOverlayIpc } from '../../../third_party/hermes-agent-pet/apps/desktop/electron/pet-overlay-ipc'

type Bounds = { x?: number; y?: number; width?: number; height?: number }
type RuntimeState = { info?: Record<string, unknown>; activity?: Record<string, unknown>; busy?: boolean; awaiting?: boolean; unread?: boolean }

// Electron consumes command-line switches before exposing `process.argv`; use
// its command-line API first, with argv only for non-Electron test runners.
const bridgeArg = process.argv.find(arg => arg.startsWith('--bridge-file='))
const bridgeFile = process.env.READMD_PET_BRIDGE_FILE || app.commandLine.getSwitchValue('bridge-file') || (bridgeArg ? bridgeArg.slice('--bridge-file='.length) : '')
let overlay: BrowserWindow | null = null
let latest: RuntimeState = {}
let bridgeTimer: NodeJS.Timeout | undefined
let lastBridgeContents = ''
let fallbackSpriteInfo: Record<string, unknown> | undefined

function getFallbackSpriteInfo(): Record<string, unknown> {
  if (fallbackSpriteInfo) return fallbackSpriteInfo
  try {
    // The fallback image is copied from Hermes and remains inside the optional
    // plugin package. It is never loaded by the lightweight ReadMD reader.
    fallbackSpriteInfo = {
      displayName: 'ReadMD',
      enabled: true,
      // `hermes-sprite.png` is a 1536x1024 source sheet laid out as four
      // 384x512 cells on each of two rows.  Supplying the sheet dimensions as
      // a single frame made Hermes' own overlay-size calculation treat one
      // pose as a 1536px-wide mascot, so the first state sync looked like a
      // drag had enlarged it.  Keep the original asset and renderer, but give
      // the copied renderer the sheet's real cell geometry.
      frameH: 512,
      frameW: 384,
      framesPerState: 4,
      mime: 'image/png',
      // This is deliberately smaller than the Hermes Petdex default: the
      // source sheet has larger illustration cells than a Petdex frame.
      // Users can still use Hermes' Alt+wheel gesture within the validated
      // 0.18–0.72 range.
      scale: 0.33,
      spritesheetBase64: fs.readFileSync(path.join(__dirname, 'assets', 'hermes-sprite.png')).toString('base64'),
      spritesheetRevision: 'hermes-fallback-e328d387a2fca8c0',
      stateRows: ['idle', 'wave']
    }
  } catch {
    fallbackSpriteInfo = { enabled: false }
  }
  return fallbackSpriteInfo
}

function normalizeState(payload: RuntimeState = {}): RuntimeState {
  const supplied = payload.info && typeof payload.info === 'object' ? payload.info : {}
  return { ...payload, info: { ...getFallbackSpriteInfo(), ...supplied, enabled: supplied.enabled !== false } }
}

function clampBounds(input: Bounds = {}): Required<Bounds> {
  const width = Math.max(240, Math.min(640, Math.round(Number(input.width) || 300)))
  const height = Math.max(300, Math.min(720, Math.round(Number(input.height) || 420)))
  return { width, height, x: Math.round(Number(input.x) || 72), y: Math.round(Number(input.y) || 72) }
}

function currentOpacity(): number {
  const raw = Number(latest.info?.opacity)
  return Number.isFinite(raw) ? Math.max(0.35, Math.min(1, raw)) : 1
}

function applyOverlayOpacity(): void {
  if (overlay && !overlay.isDestroyed()) overlay.setOpacity(currentOpacity())
}

function openPetOverlay(bounds: unknown): void {
  const next = clampBounds((bounds || {}) as Bounds)
  if (overlay && !overlay.isDestroyed()) {
    overlay.setBounds(next)
    overlay.showInactive()
    return
  }
  overlay = new BrowserWindow({
    ...next,
    alwaysOnTop: true,
    backgroundColor: '#00000000',
    focusable: false,
    frame: false,
    hasShadow: false,
    resizable: false,
    skipTaskbar: true,
    transparent: true,
    webPreferences: { contextIsolation: true, nodeIntegration: false, preload: path.join(app.getAppPath(), 'preload.cjs') }
  })
  applyOverlayOpacity()
  overlay.setAlwaysOnTop(true, 'screen-saver')
  overlay.loadFile(path.join(app.getAppPath(), 'renderer', 'index.html'))
  overlay.webContents.once('did-finish-load', () => pushState())
  overlay.on('closed', () => { overlay = null })
}

function closePetOverlay(): void {
  overlay?.close()
  overlay = null
}

function pushState(): void {
  if (overlay && !overlay.isDestroyed()) {
    applyOverlayOpacity()
    overlay.webContents.send('hermes:pet-overlay:state', latest)
  }
}

function writeCommand(command: unknown): void {
  if (!bridgeFile) return
  const commandFile = `${bridgeFile}.command`
  try { fs.writeFileSync(commandFile, JSON.stringify({ command, created_at: Date.now() }), 'utf8') } catch { /* host may be closing */ }
}

function clipboardCommand(): Record<string, unknown> {
  const text = clipboard.readText().slice(0, 4 * 1024 * 1024)
  const image = clipboard.readImage()
  const imagePng = image.isEmpty() ? '' : image.toPNG().toString('base64')
  // Windows exposes copied files in CF_HDROP / FileNameW.  This is a best-effort
  // path list; malformed values are rejected again by the Python bridge.
  let paths: string[] = []
  try {
    paths = clipboard.readBuffer('FileNameW').toString('utf16le').split('\0').filter(Boolean).slice(0, 128)
  } catch { /* clipboard has no file-list representation */ }
  return { type: 'clipboard', text, image_png: imagePng.slice(0, 24 * 1024 * 1024), paths }
}

function pollBridge(): void {
  if (!bridgeFile) return
  try {
    const contents = fs.readFileSync(bridgeFile, 'utf8')
    // Do not continually reapply the last host position.  Hermes owns a live
    // drag through `setBounds`; the host only publishes a changed snapshot.
    if (contents === lastBridgeContents) return
    lastBridgeContents = contents
    const next = JSON.parse(contents) as RuntimeState & { bounds?: Bounds; visible?: boolean }
    latest = normalizeState(next)
    if (next.visible === false) { closePetOverlay(); return }
    if (!overlay || overlay.isDestroyed()) openPetOverlay(next.bounds)
    pushState()
  } catch { /* bridge has not been published yet */ }
}

app.whenReady().then(() => {
  // Original Hermes IPC module; this is the sole implementation of drag,
  // focus, click-through and state forwarding window behavior.
  registerPetOverlayIpc({
    closePetOverlay,
    // There is no Hermes main window in the external-plugin topology.  The
    // copied overlay still emits its original control events; ReadMD maps them
    // below instead of accidentally minimizing the pet itself.
    getMainWindow: () => null,
    getPetOverlayWindow: () => overlay,
    openPetOverlay
  })
  ipcMain.on('hermes:pet-overlay:control', (_event, payload) => {
    // The unmodified Hermes overlay announces that its `onState` listener is
    // mounted.  Reply then, rather than relying on a load-time race.
    const type = (payload as { type?: string } | null)?.type
    if (type === 'ready') pushState()
    else if (type === 'toggle-app') writeCommand(clipboardCommand())
    else if (type === 'open-menu') writeCommand({ type: 'open-menu' })
    else writeCommand(payload)
  })
  ipcMain.on('readmd:pet:drop', (_event, paths) => {
    if (!Array.isArray(paths)) return
    const safePaths = paths.filter(path => typeof path === 'string' && path.length > 0 && path.length <= 32768).slice(0, 128)
    if (safePaths.length) writeCommand({ type: 'drop', paths: safePaths })
  })
  pollBridge()
  bridgeTimer = setInterval(pollBridge, 250)
})

app.on('window-all-closed', () => { /* overlay lifecycle follows bridge state */ })
app.on('before-quit', () => { if (bridgeTimer) clearInterval(bridgeTimer) })
