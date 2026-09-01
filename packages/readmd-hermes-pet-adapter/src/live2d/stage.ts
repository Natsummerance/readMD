// ReadMD-owned Live2D stage for the optional desktop pet plugin.  It mounts
// the CC0 Arch Chan model through the exact preload IPC contract that the
// copied Hermes overlay uses, so drag, click-through, the quick menu and the
// clipboard double-click behave identically without touching the immutable
// Hermes snapshot.  The model ships no motion files: idle life comes from its
// physics and the Mouse expression, and nothing pretends otherwise.

const CORE_SRC = '../vendor/live2dcubismcore.min.js'
const MANIFEST_URL = '../models/arch-chan/readmd.live2d.json'
const MIN_SCALE = 0.18
const MAX_SCALE = 0.72
const CLICK_WINDOW_MS = 320
const DRAG_THRESHOLD_PX = 4

type PetOverlayApi = {
  setBounds: (bounds: { x: number; y: number; width: number; height: number }) => void
  setIgnoreMouse: (ignore: boolean) => void
  control: (command: Record<string, unknown>) => void
  onState: (listener: (state: unknown) => void) => void
}

type OverlayState = {
  bounds?: { x: number; y: number; width: number; height: number }
  info?: { scale?: number }
  activity?: { busy?: boolean; error?: boolean; justCompleted?: boolean }
}

type Live2dModel = {
  scale: { x: number; set: (value: number) => void }
  x: number
  y: number
  width: number
  height: number
  hitTest: (x: number, y: number) => boolean
  expression: (name?: string) => unknown
}

function petOverlayApi(): PetOverlayApi | undefined {
  return (window as unknown as { hermesDesktop?: { petOverlay: PetOverlayApi } }).hermesDesktop?.petOverlay
}

// Chromium blocks fetch() for file: URLs, so the manifest is read with XHR,
// which Electron allows from the packaged file:// page.
function readManifest(): Promise<{ entry?: string }> {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest()
    request.open('GET', MANIFEST_URL)
    request.onload = () => {
      try { resolve(JSON.parse(request.responseText)) } catch (error) { reject(error) }
    }
    request.onerror = () => reject(new Error('live2d manifest unavailable'))
    request.send()
  })
}

function loadCubismCore(): Promise<void> {
  const host = window as unknown as { Live2DCubismCore?: unknown }
  if (host.Live2DCubismCore) return Promise.resolve()
  return new Promise((resolve, reject) => {
    const script = document.createElement('script')
    script.src = CORE_SRC
    script.onload = () => resolve()
    script.onerror = () => reject(new Error('cubism core failed to load'))
    document.head.appendChild(script)
  })
}

async function mountLive2dStage(): Promise<void> {
  const api = petOverlayApi()
  // Announce the mounted onState listener first: the host replies with the
  // current state instead of relying on a load-time race.
  api?.control({ type: 'ready' })

  const PIXI = await import('pixi.js')
  await loadCubismCore()
  const { Live2DModel } = await import('pixi-live2d-display/cubism4')
  Live2DModel.registerTicker(PIXI.Ticker)

  const app = new PIXI.Application({ backgroundAlpha: 0, autoDensity: true, resolution: 1, resizeTo: window })
  document.body.appendChild(app.view)
  document.body.style.margin = '0'
  document.body.style.overflow = 'hidden'

  const manifest = await readManifest()
  if (!manifest.entry) throw new Error('live2d manifest has no entry')
  const modelUrl = new URL(`../models/arch-chan/${manifest.entry}`, new URL(MANIFEST_URL, window.location.href)).toString()
  const model = await Live2DModel.from(modelUrl, { autoInteract: false }) as unknown as Live2dModel
  const naturalWidth = model.width / (model.scale.x || 1)
  app.stage.addChild(model)

  let state: OverlayState = {}
  let bounds = { x: 0, y: 0, width: 300, height: 420 }
  let dragging: { startX: number; startY: number; bounds: typeof bounds } | undefined
  let clickTimer: number | undefined
  let ignoringMouse = true

  function layout(): void {
    const raw = Number(state.info && state.info.scale) || 0.33
    const scale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, raw))
    model.scale.set((app.screen.width * scale) / naturalWidth)
    model.x = (app.screen.width - model.width) / 2
    model.y = app.screen.height - model.height
  }

  function applyState(next: unknown): void {
    state = (next || {}) as OverlayState
    if (state.bounds) bounds = state.bounds
    layout()
    if (state.activity?.error) model.expression('Mouse')
  }

  function setIgnoringMouse(ignore: boolean): void {
    if (ignore === ignoringMouse) return
    ignoringMouse = ignore
    api?.setIgnoreMouse(ignore)
  }

  function handleTap(): void {
    if (clickTimer !== undefined) {
      window.clearTimeout(clickTimer)
      clickTimer = undefined
      api?.control({ type: 'toggle-app' })
      return
    }
    clickTimer = window.setTimeout(() => {
      clickTimer = undefined
      model.expression('Mouse')
      api?.control({ type: 'open-menu' })
    }, CLICK_WINDOW_MS)
  }

  api?.onState(applyState)
  window.addEventListener('resize', layout)
  window.addEventListener('pointermove', event => {
    if (dragging) {
      api?.setBounds({
        x: Math.round(dragging.bounds.x + event.screenX - dragging.startX),
        y: Math.round(dragging.bounds.y + event.screenY - dragging.startY),
        width: dragging.bounds.width,
        height: dragging.bounds.height
      })
      return
    }
    setIgnoringMouse(!model.hitTest(event.clientX, event.clientY))
  })
  window.addEventListener('pointerdown', event => {
    if (!model.hitTest(event.clientX, event.clientY)) return
    dragging = { startX: event.screenX, startY: event.screenY, bounds: { ...bounds } }
  })
  window.addEventListener('pointerup', event => {
    if (!dragging) return
    const moved = Math.hypot(event.screenX - dragging.startX, event.screenY - dragging.startY)
    dragging = undefined
    if (moved > DRAG_THRESHOLD_PX) return
    handleTap()
  })
  window.addEventListener('pointercancel', () => { dragging = undefined })

  layout()
}

export { mountLive2dStage }
