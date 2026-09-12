// ReadMD-owned Live2D stage for the optional desktop pet plugin.  It mounts
// the CC0 Arch Chan model through the exact preload IPC contract that the
// copied Hermes overlay uses, so drag, click-through, the quick menu and the
// clipboard double-click behave identically without touching the immutable
// Hermes snapshot.  Interactive life includes mouse gaze tracking, breathing,
// natural blinking, physics drag inertia, and expressive tap responses.

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
  info?: {
    scale?: number
    animation?: { enabled?: boolean; fpsCap?: number }
  }
  activity?: { busy?: boolean; error?: boolean; justCompleted?: boolean }
}

type Live2dModel = {
  scale: { x: number; set: (value: number) => void }
  x: number
  y: number
  width: number
  height: number
  getBounds?: () => { contains: (x: number, y: number) => boolean }
  hitTest: (x: number, y: number) => string[]
  expression: (name?: string) => unknown
  focus?: (x: number, y: number, instant?: boolean) => void
  internalModel?: {
    coreModel?: CoreModelLike
    focusController?: {
      focus: (x: number, y: number, instant?: boolean) => void
      targetX: number
      targetY: number
      x: number
      y: number
    }
    on?: (event: string, listener: () => void) => void
  }
}

type CoreModelLike = {
  setParameterValueById?: (id: string, value: number, weight?: number) => void
  addParameterValueById?: (id: string, value: number, weight?: number) => void
  getParameterValueById?: (id: string) => number
}

// Control surface for the companion layer (speech bubbles, moods, talking).
export type Live2dLifeController = {
  setMood: (mood: 'normal' | 'drowsy' | 'sleeping') => void
  setTalking: (talking: boolean) => void
  celebrate: () => void
  getCharacterTop?: () => number
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

async function mountLive2dStage(): Promise<Live2dLifeController> {
  const api = petOverlayApi()
  // Announce the mounted onState listener first: the host replies with the
  // current state instead of relying on a load-time race.

  const PIXI = await import('pixi.js')
  await loadCubismCore()
  const { Live2DModel } = await import('pixi-live2d-display/cubism4')
  Live2DModel.registerTicker(PIXI.Ticker)

  const dpr = Math.max(1, window.devicePixelRatio || 1)
  const app = new PIXI.Application({ backgroundAlpha: 0, autoDensity: true, resolution: dpr, resizeTo: window })
  // The shared overlay shell reserves a full-height #root. Appending after
  // it places the canvas below the viewport, where overflow:hidden clips it.
  const stageRoot = document.getElementById('root') || document.body
  stageRoot.replaceChildren(app.view)
  document.body.style.margin = '0'
  document.body.style.overflow = 'hidden'

  const manifest = await readManifest()
  if (!manifest.entry) throw new Error('live2d manifest has no entry')
  const modelUrl = new URL(manifest.entry, new URL(MANIFEST_URL, window.location.href)).toString()
  const model = await Live2DModel.from(modelUrl, { autoInteract: true }) as unknown as Live2dModel
  const naturalWidth = model.width / (model.scale.x || 1)
  app.stage.addChild(model)

  const hitModel = (x: number, y: number) => {
    const areas = typeof model.hitTest === 'function' ? model.hitTest(x, y) : []
    return (Array.isArray(areas) && areas.length > 0) || Boolean(model.getBounds?.().contains(x, y))
  }

  let state: OverlayState = {}
  let bounds = { x: 0, y: 0, width: 300, height: 420 }
  let dragging: { startX: number; startY: number; pointerId: number; target?: Element; bounds: typeof bounds } | undefined
  let clickTimer: number | undefined
  let ignoringMouse: boolean | undefined = undefined

  const getTime = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now())

  // Lifelike interaction state variables
  let tapReactionWeight = 0
  let lastBlinkTime = getTime()
  let nextBlinkInterval = 3200 + Math.random() * 2000
  let blinkProgress = 0
  let isBlinking = false
  let lastDragX = 0
  let lastDragY = 0
  let lastDragTime = getTime()
  let dragVelocityX = 0
  let dragVelocityY = 0
  let currentTiltZ = 0
  let currentTiltBodyX = 0

  const mood: { state: 'normal' | 'drowsy' | 'sleeping'; talking: boolean } = { state: 'normal', talking: false }
  const lifeController: Live2dLifeController = {
    setMood: next => { mood.state = next },
    setTalking: talking => { mood.talking = talking },
    celebrate: () => { tapReactionWeight = 1.0 },
    getCharacterTop: () => model.y
  }

  const probeActive = new URLSearchParams(window.location.search).has('live2dProbe')
  const probeHost = window as unknown as { __live2dProbe?: { frame: number; params: Record<string, number>; fps?: number } }
  if (probeActive) probeHost.__live2dProbe = { frame: 0, params: {}, fps: 0 }

  function layout(): void {
    const raw = Number(state.info && state.info.scale) || 0.33
    const scale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, raw))
    model.scale.set((app.screen.width * scale) / naturalWidth)
    model.x = (app.screen.width - model.width) / 2
    model.y = app.screen.height - model.height
  }

  function updateAnimationState(): void {
    const animation = state.info && state.info.animation
    const disabled = Boolean(animation && animation.enabled === false)
    const hidden = document.visibilityState === 'hidden' || Boolean((state as OverlayState & { fullscreen?: boolean }).fullscreen)
    if (disabled || hidden) {
      if (app.ticker.started) app.ticker.stop()
      return
    }
    const cap = Number(animation && animation.fpsCap)
    // The host budget (6 idle / 30 active / 0 off) was tuned for the sprite
    // flipbook; a skeletal Live2D model needs a smoothness floor, so an idle
    // cap of 6 still renders at 24fps while active work keeps 30.  A missing
    // budget (older hosts) keeps the previous unlimited rendering.
    app.ticker.maxFPS = Number.isFinite(cap) && cap > 0 ? Math.max(24, Math.min(cap, 60)) : 0
    if (!app.ticker.started) app.ticker.start()
  }

  function applyState(next: unknown): void {
    state = (next || {}) as OverlayState
    if (state.bounds) bounds = { ...state.bounds }
    layout()
    updateAnimationState()
    if (state.activity?.error) {
      model.expression('Mouse.exp3.json')
      tapReactionWeight = 0.6
    } else if (state.activity?.justCompleted) {
      tapReactionWeight = 1.0
    }
  }

  function setIgnoringMouse(ignore: boolean): void {
    if (ignore === ignoringMouse) return
    ignoringMouse = ignore
    api?.setIgnoreMouse(ignore)
  }

  function handleTap(): void {
    tapReactionWeight = 1.0
    if (clickTimer !== undefined) {
      window.clearTimeout(clickTimer)
      clickTimer = undefined
      api?.control({ type: 'toggle-app' })
      return
    }
    clickTimer = window.setTimeout(() => {
      clickTimer = undefined
      model.expression('Mouse.exp3.json')
      api?.control({ type: 'open-menu' })
    }, CLICK_WINDOW_MS)
  }

  // Lifelike animation must run inside the Live2D runtime's own per-frame
  // bracket.  The bundled runtime snapshots the parameters after motions
  // (`saveParameters()`), layers blink/breath/focus/physics on top, emits
  // `beforeModelUpdate`, calls `update()`, then restores the snapshot
  // (`loadParameters()`).  A write issued for `beforeModelUpdate` therefore
  // shapes the rendered frame and is discarded afterwards, so additive sway
  // can never accumulate.  Writes from the plain app ticker instead get baked
  // into the saved baseline every frame and pin the head against its
  // parameter limits within a second — the frozen-model defect this replaced.
  let lastLifeTime = getTime()
  const additiveState: { compensate: boolean; last: Record<string, number> } = { compensate: false, last: {} }

  // Occasional scripted gestures keep the idle loop from feeling like a
  // single repeating sway: every few seconds the model performs one small
  // head performance (tilt / nod / glance) with eased in-out envelopes.
  const gestures = ['tilt', 'nod', 'glance'] as const
  let nextGestureAt = getTime() + 2200
  let gesture: { kind: (typeof gestures)[number]; start: number; duration: number; side: number } | undefined

  function gestureOffsets(now: number): { x: number; y: number; z: number } {
    if (!gesture && now >= nextGestureAt && !dragging && tapReactionWeight <= 0.05 && mood.state === 'normal') {
      const kind = gestures[Math.floor(Math.random() * gestures.length)]
      gesture = { kind, start: now, duration: 1500 + Math.random() * 500, side: Math.random() < 0.5 ? -1 : 1 }
    }
    if (!gesture) return { x: 0, y: 0, z: 0 }
    const progress = (now - gesture.start) / gesture.duration
    if (progress >= 1) {
      gesture = undefined
      nextGestureAt = now + 4200 + Math.random() * 4200
      return { x: 0, y: 0, z: 0 }
    }
    // Smooth rise-and-fall envelope over the gesture window.
    const envelope = Math.sin(progress * Math.PI)
    if (gesture.kind === 'tilt') return { x: 0, y: 0, z: gesture.side * 9 * envelope }
    if (gesture.kind === 'nod') return { x: 0, y: -7 * Math.sin(progress * Math.PI * 2), z: 0 }
    return { x: gesture.side * 11 * envelope, y: 0, z: 0 }
  }

  function applyLife(): void {
    const now = getTime()
    lastLifeTime = now
    const core = model.internalModel?.coreModel
    if (!core || typeof core.setParameterValueById !== 'function') return
    const setParam = (id: string, value: number) => { core.setParameterValueById?.(id, value) }
    const addParam = (id: string, value: number) => {
      if (typeof core.addParameterValueById !== 'function') return
      // Only the ticker fallback below needs explicit compensation: there the
      // runtime's frame-end restore does not wipe additive writes, so each
      // frame cancels the previous frame's contribution before adding its own.
      if (additiveState.compensate) {
        const previous = additiveState.last[id]
        if (previous) core.addParameterValueById(id, -previous)
        additiveState.last[id] = value
      }
      core.addParameterValueById(id, value)
    }
    const sleeping = mood.state === 'sleeping'
    const drowsy = mood.state === 'drowsy'

    // 1. Natural periodic blinking (heavy-lidded when drowsy, shut when asleep)
    if (!isBlinking && now - lastBlinkTime > (drowsy ? 1800 : nextBlinkInterval)) {
      isBlinking = true
      blinkProgress = 0
    }
    let eyeOpen = 1
    if (isBlinking) {
      blinkProgress += drowsy ? 0.09 : 0.12
      if (blinkProgress <= 0.5) {
        eyeOpen = Math.max(0, 1 - blinkProgress * 2)
      } else if (blinkProgress <= 1.0) {
        eyeOpen = Math.min(1, (blinkProgress - 0.5) * 2)
      } else {
        isBlinking = false
        lastBlinkTime = now
        nextBlinkInterval = 2800 + Math.random() * 2500
      }
    }
    const lid = sleeping ? 0 : drowsy ? 0.55 : 1
    setParam('ParamEyeLOpen', eyeOpen * lid)
    setParam('ParamEyeROpen', eyeOpen * lid)

    // 2. Idle subtle micro-movements plus occasional scripted gestures
    if (!dragging && tapReactionWeight <= 0.05 && !sleeping) {
      const sway = drowsy ? 0.5 : 1
      const perform = gestureOffsets(now)
      addParam('ParamAngleZ', (Math.sin(now * 0.0011) * 2.2 + perform.z) * sway)
      addParam('ParamAngleX', (Math.sin(now * 0.0008) * 3.0 + perform.x) * sway)
      addParam('ParamAngleY', (Math.cos(now * 0.0013) * 2.2 + perform.y) * sway)
    }

    // 3. Dynamic drag inertia & hair physics response
    if (dragging) {
      const targetTiltZ = Math.max(-18, Math.min(18, -dragVelocityX * 22))
      const targetTiltBody = Math.max(-12, Math.min(12, dragVelocityX * 15))
      currentTiltZ += (targetTiltZ - currentTiltZ) * 0.25
      currentTiltBodyX += (targetTiltBody - currentTiltBodyX) * 0.25
      dragVelocityX *= 0.85
      dragVelocityY *= 0.85
    } else {
      currentTiltZ *= 0.88
      currentTiltBodyX *= 0.88
    }
    if (Math.abs(currentTiltZ) > 0.1 || Math.abs(currentTiltBodyX) > 0.1) {
      addParam('ParamAngleZ', currentTiltZ)
      addParam('ParamBodyAngleX', currentTiltBodyX)
    }

    // 4. Expressive tap / click reaction (blush, smile, nod, spring decay)
    if (tapReactionWeight > 0.005) {
      tapReactionWeight *= 0.94
      setParam('ParamEyeLSmile', tapReactionWeight)
      setParam('ParamEyeRSmile', tapReactionWeight)
      setParam('ParamCheek', tapReactionWeight * 0.85)
      addParam('ParamAngleZ', Math.sin(tapReactionWeight * Math.PI) * 10)
      addParam('ParamAngleY', -Math.sin(tapReactionWeight * Math.PI) * 6)
      addParam('ParamMouthOpenY', tapReactionWeight * 0.4)
      addParam('ParamMouthForm', tapReactionWeight)
    }

    // 5. Talking mouth while a speech bubble is on screen
    if (mood.talking && !sleeping) {
      setParam('ParamMouthOpenY', 0.1 + 0.16 * Math.abs(Math.sin(now * 0.009)))
      setParam('ParamMouthForm', 0.6)
    }

    if (probeActive && probeHost.__live2dProbe) {
      const probe = probeHost.__live2dProbe
      probe.frame += 1
      const read = (id: string): number =>
        typeof core.getParameterValueById === 'function' ? core.getParameterValueById(id) : Number.NaN
      probe.params = {
        AngleX: read('ParamAngleX'),
        AngleY: read('ParamAngleY'),
        AngleZ: read('ParamAngleZ'),
        Breath: read('ParamBreath'),
        EyeLOpen: read('ParamEyeLOpen'),
        MouthOpenY: read('ParamMouthOpenY')
      }
      probe.fps = app.ticker.maxFPS
    }
  }

  const internalModel = model.internalModel as { on?: (event: string, listener: () => void) => void } | undefined
  if (typeof internalModel?.on === 'function') {
    internalModel.on('beforeModelUpdate', applyLife)
  } else {
    // Defensive fallback for a runtime without the per-frame restore bracket:
    // keep animating from the app ticker but enable explicit compensation so
    // additive writes still cancel themselves across frames.
    additiveState.compensate = true
    app.ticker.add(() => {
      if (getTime() - lastLifeTime < 4) return
      applyLife()
    })
  }

  api?.onState(applyState)
  api?.control({ type: 'ready' })
  window.addEventListener('resize', layout)
  document.addEventListener('visibilitychange', updateAnimationState)
  window.addEventListener('pointermove', event => {
    if (dragging) {
      const nextX = Math.round(dragging.bounds.x + event.screenX - dragging.startX)
      const nextY = Math.round(dragging.bounds.y + event.screenY - dragging.startY)
      bounds.x = nextX
      bounds.y = nextY
      const dt = Math.max(16, getTime() - lastDragTime)
      dragVelocityX = (event.screenX - lastDragX) / dt
      dragVelocityY = (event.screenY - lastDragY) / dt
      lastDragX = event.screenX
      lastDragY = event.screenY
      lastDragTime = getTime()
      api?.setBounds({
        x: nextX,
        y: nextY,
        width: dragging.bounds.width,
        height: dragging.bounds.height
      })
      return
    }
    setIgnoringMouse(!hitModel(event.clientX, event.clientY))
    if (typeof model.focus === 'function') {
      model.focus(event.clientX, event.clientY)
    }
  })
  window.addEventListener('pointerdown', event => {
    if (event.button !== 0 || !hitModel(event.clientX, event.clientY)) return
    setIgnoringMouse(false)
    lastDragX = event.screenX
    lastDragY = event.screenY
    lastDragTime = getTime()
    dragVelocityX = 0
    dragVelocityY = 0
    const target = event.target as Element
    target?.setPointerCapture?.(event.pointerId)
    dragging = {
      startX: event.screenX,
      startY: event.screenY,
      pointerId: event.pointerId,
      target,
      bounds: { ...bounds }
    }
  })
  window.addEventListener('pointerup', event => {
    if (!dragging) return
    const drag = dragging
    dragging = undefined
    try { drag.target?.releasePointerCapture?.(drag.pointerId) } catch { /* ignore */ }
    const moved = Math.hypot(event.screenX - drag.startX, event.screenY - drag.startY)
    bounds.x = Math.round(drag.bounds.x + event.screenX - drag.startX)
    bounds.y = Math.round(drag.bounds.y + event.screenY - drag.startY)
    if (moved > DRAG_THRESHOLD_PX) {
      api?.control({
        type: 'bounds',
        bounds: {
          x: bounds.x,
          y: bounds.y,
          width: drag.bounds.width,
          height: drag.bounds.height
        }
      })
      setIgnoringMouse(!hitModel(event.clientX, event.clientY))
      return
    }
    setIgnoringMouse(!hitModel(event.clientX, event.clientY))
    handleTap()
  })
  window.addEventListener('pointercancel', () => {
    if (dragging) {
      try { dragging.target?.releasePointerCapture?.(dragging.pointerId) } catch { /* ignore */ }
      dragging = undefined
      setIgnoringMouse(true)
    }
  })

  layout()
  setIgnoringMouse(true)
  document.body.dataset.live2dReady = 'true'
  return lifeController
}

export { mountLive2dStage }
