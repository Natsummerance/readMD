// ReadMD-owned process/lifecycle adapter.  The overlay renderer and its IPC
// semantics stay in the immutable Hermes source snapshot.
import fs from 'node:fs'
import path from 'node:path'
import { app, BrowserWindow, clipboard, ipcMain, screen, Menu } from 'electron'
import { registerPetOverlayIpc } from '../../../third_party/hermes-agent-pet/apps/desktop/electron/pet-overlay-ipc'
import { publishCommand, SnapshotReader } from './bridge-transport'

// Enforce single instance: prevent launching multiple desktop pets simultaneously
const gotTheLock = app.requestSingleInstanceLock()
if (!gotTheLock) {
  app.exit(0)
}

app.on('second-instance', () => {
  if (overlay && !overlay.isDestroyed()) {
    overlay.showInactive()
    pushState()
  }
})

type Bounds = { x?: number; y?: number; width?: number; height?: number }
type RuntimeState = { info?: Record<string, unknown>; activity?: Record<string, unknown>; busy?: boolean; awaiting?: boolean; unread?: boolean }

// Electron consumes command-line switches before exposing `process.argv`; use
// its command-line API first, with argv only for non-Electron test runners.
const bridgeArg = process.argv.find(arg => arg.startsWith('--bridge-file='))
const bridgeFile = process.env.READMD_PET_BRIDGE_FILE || app.commandLine.getSwitchValue('bridge-file') || (bridgeArg ? bridgeArg.slice('--bridge-file='.length) : '')
const parentPidStr = process.env.READMD_PARENT_PID || app.commandLine.getSwitchValue('parent-pid')
const parentPid = parentPidStr ? Number.parseInt(parentPidStr, 10) : undefined

function isHostProcessAlive(): boolean {
  if (!parentPid || Number.isNaN(parentPid) || parentPid <= 0) return true
  try {
    process.kill(parentPid, 0)
    return true
  } catch (err: unknown) {
    return (err as NodeJS.ErrnoException).code !== 'ESRCH'
  }
}
let overlay: BrowserWindow | null = null
let latest: RuntimeState = {}
let bridgeTimer: NodeJS.Timeout | undefined
const snapshotReader = new SnapshotReader(bridgeFile)
let bridgeWatcher: fs.FSWatcher | undefined
let bridgeDebounce: NodeJS.Timeout | undefined
let shuttingDown = false
let recoveries: number[] = []
let fallbackSpriteInfo: Record<string, unknown> | undefined
let lastRenderer: string | undefined
let lastHostBounds: Bounds | undefined

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
      spritesheetRevision: 'hermes-fallback-a5661b457de00b9a',
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
  let x = Number.isFinite(input.x) ? Math.round(Number(input.x)) : 72
  let y = Number.isFinite(input.y) ? Math.round(Number(input.y)) : 72

  if (app.isReady()) {
    try {
      const displays = screen.getAllDisplays()
      const isVisible = displays.some(display => {
        const { x: dx, y: dy, width: dw, height: dh } = display.workArea
        return (
          x + width >= dx + 40 &&
          x <= dx + dw - 40 &&
          y + height >= dy + 40 &&
          y <= dy + dh - 40
        )
      })
      if (!isVisible && displays.length > 0) {
        const primary = screen.getPrimaryDisplay().workArea
        x = primary.x + Math.max(12, primary.width - width - 24)
        y = primary.y + Math.max(12, primary.height - height - 24)
      }
    } catch { /* screen API unavailable */ }
  }
  return { width, height, x, y }
}

function currentOpacity(): number {
  const raw = Number(latest.info?.opacity)
  return Number.isFinite(raw) ? Math.max(0.35, Math.min(1, raw)) : 1
}

function applyOverlayOpacity(): void {
  if (overlay && !overlay.isDestroyed()) overlay.setOpacity(currentOpacity())
}

// A renderer-preference change swaps the page inside the surviving window, so
// the Electron process and its registered IPC handlers are never restarted.
function loadOverlayPage(renderer?: string): void {
  if (!overlay || overlay.isDestroyed()) return
  const query: Record<string, string> = {}
  if (renderer) query.renderer = renderer
  if (process.env.READMD_PET_LIVE2D_PROBE === '1') query.live2dProbe = '1'
  const loadOptions = Object.keys(query).length ? { query } : undefined
  reportHealth('loading', renderer)
  void overlay.loadFile(path.join(app.getAppPath(), 'renderer', 'index.html'), loadOptions).catch(() => reportHealth('failed', renderer, 'pet_page_load_failed'))
  overlay.webContents.once('did-finish-load', () => pushState())
}

function openPetOverlay(bounds: unknown, renderer?: string): void {
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
    show: false,
    skipTaskbar: true,
    transparent: true,
    webPreferences: { contextIsolation: true, nodeIntegration: false, preload: path.join(app.getAppPath(), 'preload.cjs') }
  })
  applyOverlayOpacity()
  overlay.setAlwaysOnTop(true, 'screen-saver')
  overlay.on('closed', () => { overlay = null })
  overlay.webContents.on('context-menu', () => showCompanionMenu())
  overlay.webContents.on('render-process-gone', (_event, details) => {
    if (shuttingDown || details.reason === 'clean-exit') return
    reportHealth('failed', lastRenderer, 'pet_renderer_crashed')
    recoveries = recoveries.filter(moment => Date.now() - moment < 60_000)
    if (recoveries.length >= 3) return
    recoveries.push(Date.now())
    setTimeout(() => { if (!shuttingDown) loadOverlayPage(lastRenderer) }, 500 * recoveries.length)
  })
  loadOverlayPage(renderer)
}

function closePetOverlay(): void {
  overlay?.close()
  overlay = null
}

function pushState(): void {
  if (overlay && !overlay.isDestroyed()) {
    applyOverlayOpacity()
    const stateToSend = overlay ? { ...latest, bounds: overlay.getBounds() } : latest
    overlay.webContents.send('hermes:pet-overlay:state', stateToSend)
  }
}

function writeCommand(command: unknown): void {
  if (!bridgeFile) return
  try { publishCommand(bridgeFile, command) }
  catch (error) { reportHealth('degraded', lastRenderer, (error as Error).message) }
}

function reportHealth(state: string, renderer?: string, code?: string): void {
  if (!bridgeFile) return
  const target = `${bridgeFile}.health.json`, temp = `${target}.tmp`
  try {
    fs.writeFileSync(temp, JSON.stringify({ state, renderer, code, pid: process.pid, updated_at: Date.now() }), 'utf8')
    fs.renameSync(temp, target)
  } catch { /* host may be closing */ }
}

function showCompanionMenu(): void {
  if (!overlay || overlay.isDestroyed()) return
  const lines = (latest.info?.lines || {}) as Record<string, string>
  const life = (latest.info?.companion || {}) as { character?: string; level?: number; energy?: number; mood?: number; resting?: boolean; cooldowns?: Record<string, number> }
  const label = (key: string, fallback: string) => lines[key] || fallback
  const characters = Array.isArray(latest.info?.characters) ? latest.info.characters as { slug: string; name: string; renderer: string }[] : []
  const actions = ['pet', 'feed', 'play', life.resting ? 'wake' : 'rest']
  const fallback: Record<string, string> = { pet: 'Pet', feed: 'Feed', play: 'Play', rest: 'Rest', wake: 'Wake up' }
  Menu.buildFromTemplate([
    { label: `${label('pet.life.level', 'Level')} ${life.level || 1} · ${label('pet.life.energy', 'Energy')} ${life.energy ?? 80} · ${label('pet.life.mood', 'Mood')} ${life.mood ?? 75}`, enabled: false },
    { type: 'separator' },
    ...actions.map(action => ({ label: label(`pet.action.${action}`, fallback[action]), enabled: !(life.cooldowns?.[action]) && (action !== 'play' || (life.energy ?? 80) >= 10), click: () => writeCommand({ type: 'interact', action }) })),
    { type: 'separator' },
    { label: label('pet.life.characters', 'Characters'), submenu: characters.slice(0, 128).map(character => ({
      label: character.name, type: 'radio' as const,
      checked: life.character === (character.renderer === 'live2d' ? 'live2d:arch-chan' : character.slug || 'hermes'),
      click: () => writeCommand({ type: 'character', slug: character.slug, renderer: character.renderer })
    })) },
    { label: label('pet.life.reader', 'Open reader'), click: () => writeCommand({ type: 'open-menu' }) }
  ]).popup({ window: overlay })
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

async function pollBridge(): Promise<void> {
  if (shuttingDown) return
  if (!isHostProcessAlive()) {
    closePetOverlay()
    app.exit(0)
    return
  }
  if (!bridgeFile) return
  try {
    const next = await snapshotReader.read() as (RuntimeState & { bounds?: Bounds; visible?: boolean; renderer?: string; fullscreen?: boolean }) | undefined
    // Do not continually reapply the last host position.  Hermes owns a live
    // drag through `setBounds`; the host only publishes a changed snapshot.
    if (!next || shuttingDown) return
    latest = normalizeState(next)
    if (next.visible === false && !next.fullscreen) { closePetOverlay(); return }
    const renderer = typeof next.renderer === 'string' ? next.renderer : undefined
    if (!overlay || overlay.isDestroyed()) {
      lastRenderer = renderer
      lastHostBounds = next.bounds ? { ...next.bounds } : undefined
      openPetOverlay(next.bounds, renderer)
    } else if (next.bounds && JSON.stringify(next.bounds) !== JSON.stringify(lastHostBounds)) {
      lastHostBounds = { ...next.bounds }
      overlay.setBounds(clampBounds({ ...overlay.getBounds(), ...next.bounds }))
    }
    if (next.fullscreen === true) overlay.hide()
    else overlay.showInactive()
    if (renderer !== lastRenderer) {
      lastRenderer = renderer
      loadOverlayPage(renderer)
      return
    }
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
    if (type === 'renderer-ready' || type === 'renderer-failed') {
      const data = payload as { renderer?: string; code?: string }
      reportHealth(type === 'renderer-ready' ? 'ready' : 'failed', data.renderer, data.code)
    }
    else if (type === 'ready') pushState()
    else if (type === 'toggle-app') writeCommand(clipboardCommand())
    else if (type === 'open-menu') writeCommand({ type: 'open-menu' })
    else writeCommand(payload)
  })
  ipcMain.on('readmd:pet:drop', (_event, paths) => {
    if (!Array.isArray(paths)) return
    const safePaths = paths.filter(path => typeof path === 'string' && path.length > 0 && path.length <= 32768).slice(0, 128)
    if (safePaths.length) writeCommand({ type: 'drop', paths: safePaths })
  })
  void pollBridge()
  // Directory watch survives Python's atomic rename. The slow poll recovers
  // lost OS notifications and checks parent liveness without reading images.
  try {
    bridgeWatcher = fs.watch(path.dirname(bridgeFile), (_event, name) => {
      if (name && name.toString() !== path.basename(bridgeFile)) return
      if (bridgeDebounce) clearTimeout(bridgeDebounce)
      bridgeDebounce = setTimeout(() => { void pollBridge() }, 20)
    })
    bridgeWatcher.on('error', () => { bridgeWatcher?.close(); bridgeWatcher = undefined })
  } catch { /* poll remains available */ }
  bridgeTimer = setInterval(() => { void pollBridge() }, 2000)
})

app.on('window-all-closed', () => { /* overlay lifecycle follows bridge state */ })
app.on('before-quit', () => {
  shuttingDown = true
  if (bridgeTimer) clearInterval(bridgeTimer)
  if (bridgeDebounce) clearTimeout(bridgeDebounce)
  bridgeWatcher?.close()
})
