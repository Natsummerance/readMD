// The original BongoCat standard model and instrument/key layers, driven by
// the Rust host's held controls. Asset provenance lives in third_party/bongocat.
import { loadCubismCore } from './stage'

const BASE = '../models/bongocat-standard/'
const keyNames: Record<number, string> = {
  0x28: 'Enter', 0x29: 'Escape', 0x2a: 'Backspace', 0x2b: 'Tab', 0x2c: 'Space',
  0x2d: 'Minus', 0x2e: 'Equal', 0x2f: 'BracketLeft', 0x30: 'BracketRight',
  0x31: 'Backslash', 0x33: 'Semicolon', 0x34: 'Quote', 0x35: 'BackQuote',
  0x36: 'Comma', 0x37: 'Period', 0x38: 'Slash', 0x39: 'CapsLock',
  0x4c: 'Delete', 0x4f: 'ArrowRight', 0x50: 'ArrowLeft', 0x51: 'ArrowDown', 0x52: 'ArrowUp',
  0xe0: 'ControlLeft', 0xe1: 'ShiftLeft', 0xe2: 'AltLeft', 0xe3: 'Meta',
  0xe4: 'ControlRight', 0xe5: 'ShiftRight', 0xe6: 'AltRight', 0xe7: 'Meta'
}
for (let i = 0; i < 26; i++) keyNames[4 + i] = `Key${String.fromCharCode(65 + i)}`
for (let i = 0; i < 10; i++) keyNames[0x1e + i] = `Num${(i + 1) % 10}`

type BongoState = {
  pointerX: number; pointerY: number; leftDown: boolean; rightDown: boolean;
  leftTapAt: number; rightTapAt: number; mouseButtons: number;
  pressedKeys: Set<number>; lastKey: number | null; pettingLevel: number;
}

export async function mountBongoClassic(container: HTMLElement, readState: () => BongoState) {
  const PIXI = await import('pixi.js')
  await loadCubismCore()
  const { Live2DModel } = await import('pixi-live2d-display/cubism4')
  Live2DModel.registerTicker(PIXI.Ticker)
  const app = new PIXI.Application({ backgroundAlpha: 0, autoDensity: true,
    resolution: Math.min(2, window.devicePixelRatio || 1), resizeTo: window })
  container.replaceChildren(app.view)
  const model = await Live2DModel.from(BASE + 'cat.model3.json', { autoInteract: false, autoUpdate: false })
  // PIXI reports the model canvas, not just the visible drawables. All original
  // background and pressed-key images use the same 612x354 canvas coordinates.
  const naturalWidth = model.internalModel.width, naturalHeight = model.internalModel.height
  const scene = new PIXI.Container()
  const background = PIXI.Sprite.from(BASE + 'resources/background.png')
  background.width = naturalWidth; background.height = naturalHeight
  scene.addChild(background, model)
  const available = await fetch(BASE + 'keys.json').then(response => {
    if (!response.ok) throw new Error('BongoCat key manifest unavailable')
    return response.json()
  }) as string[]
  const overlays = new Map<number, InstanceType<typeof PIXI.Sprite>>()
  // Create each original image once; keyboard operation never starts a fetch.
  await Promise.all(available.map(async name => {
    const id = Number(Object.keys(keyNames).find(id => keyNames[Number(id)] === name))
    if (!Number.isFinite(id)) return
    const texture = await PIXI.Texture.fromURL(BASE + `resources/left-keys/${name}.png`)
    const sprite = new PIXI.Sprite(texture)
    sprite.width = naturalWidth; sprite.height = naturalHeight; sprite.visible = false
    overlays.set(id, sprite); scene.addChild(sprite)
  }))
  app.stage.addChild(scene)
  const core = model.internalModel.coreModel
  const normalized = (id: string, value: number) => {
    const index = core.getParameterIndex(id)
    if (index < 0 || index >= core.getParameterCount()) return
    const minimum = core.getParameterMinimumValue(index), maximum = core.getParameterMaximumValue(index)
    const neutral = core.getParameterDefaultValue(index)
    const bounded = Math.max(-1, Math.min(1, value))
    core.setParameterValueByIndex(index, neutral + bounded * (bounded >= 0 ? maximum - neutral : neutral - minimum))
  }
  const parameters: Record<string, number> = {}
  model.internalModel.on('beforeModelUpdate', () => {
    const state = readState(), now = performance.now()
    parameters.CatParamLeftHandDown = Number(state.leftDown || now - state.leftTapAt < 55)
    parameters.ParamMouseLeftDown = Number(!!(state.mouseButtons & 1))
    parameters.ParamMouseRightDown = Number(!!(state.mouseButtons & 2))
    for (const [id, value] of Object.entries({ ...parameters,
      ParamMouseX: state.pointerX, ParamMouseY: state.pointerY,
      ParamAngleX: state.pointerX, ParamAngleY: state.pointerY,
      ParamEyeBallX: state.pointerX, ParamEyeBallY: state.pointerY,
      ParamEyeLSmile: Math.min(1, state.pettingLevel / 45),
      ParamEyeRSmile: Math.min(1, state.pettingLevel / 45) })) normalized(id, value)
    for (const [id, sprite] of overlays) sprite.visible = state.pressedKeys.has(id)
      || (id === 0xe3 && state.pressedKeys.has(0xe7))
      || ((state.lastKey === id || (id === 0xe3 && state.lastKey === 0xe7)) && now - state.leftTapAt < 55)
    const phase = (now % 4300) / 1000
    const open = phase < 0.16 ? 1 - Math.sin(phase / 0.16 * Math.PI) : 1
    core.setParameterValueById('ParamEyeLOpen', open)
    core.setParameterValueById('ParamEyeROpen', open)
  })
  const layout = () => {
    app.renderer.resize(window.innerWidth, window.innerHeight)
    const scale = Math.min((app.screen.width - 12) / naturalWidth, (app.screen.height - 12) / naturalHeight)
    scene.scale.set(scale)
    scene.x = (app.screen.width - naturalWidth * scale) / 2
    scene.y = app.screen.height - naturalHeight * scale - 6
    // CSS clips the original viewport without mixing PIXI's stencil masks with
    // Cubism's own masking pipeline. Inactive expressions sit outside it.
    container.style.clipPath = `inset(${scene.y}px ${scene.x}px 6px ${scene.x}px)`
    window.dispatchEvent(new Event('readmd-bongo-layout'))
  }
  let active = true
  app.ticker.maxFPS = 60
  app.ticker.add(() => model.update(Math.min(50, app.ticker.deltaMS)), undefined, PIXI.UPDATE_PRIORITY.HIGH)
  window.addEventListener('resize', layout)
  layout()
  // A compact observable probe verifies real Cubism parameters in UI tests.
  ;(window as unknown as { __bongoClassic: unknown }).__bongoClassic = { app, model, parameters, overlays, naturalWidth, naturalHeight }
  return { interactionRegions() {
    const rect = (x: number, y: number, width: number, height: number) => ({
      x: scene.x + x * scene.scale.x, y: scene.y + y * scene.scale.y,
      width: width * scene.scale.x, height: height * scene.scale.y
    })
    return { rects: [rect(100, 12, 415, 240), rect(0, 154, 612, 200)], head: rect(180, 65, 260, 140) }
  }, setActive(next: boolean) {
    container.style.display = next ? 'block' : 'none'
    if (next === active) return
    active = next
    if (next) app.ticker.start(); else app.ticker.stop()
  } }
}
