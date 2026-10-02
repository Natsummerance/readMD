// Overlay entry. The Rust host selects the input-reactive sprite presentation
// or the Live2D stage; both are built from tracked source in the same bundle.
// Both renderers get the ReadMD companion layer (speech bubbles and the idle
// companionship state machine) on top of their own mount.
const requested = new URLSearchParams(window.location.search).get('renderer')

async function mountOverlay(): Promise<void> {
  const root = document.getElementById('root')!
  root.style.cssText = 'position:relative;width:100%;height:100%;overflow:hidden;touch-action:none;user-select:none'
  const instruments = document.createElement('canvas')
  instruments.id = 'pet-instruments-canvas'
  instruments.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;pointer-events:none'
  const bongoRoot = document.createElement('div')
  bongoRoot.id = 'bongo-classic-stage'
  bongoRoot.style.cssText = 'position:absolute;inset:0;display:none;pointer-events:none'
  const live2dRoot = document.createElement('div')
  live2dRoot.id = 'live2d-stage'
  live2dRoot.style.cssText = 'position:absolute;inset:0;display:none;pointer-events:none'
  const canvas = document.createElement('canvas')
  canvas.id = 'bongocat-canvas'
  canvas.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;cursor:grab;touch-action:none'
  root.replaceChildren(instruments, bongoRoot, live2dRoot, canvas)
  ;(window as unknown as { readmdMountBongoClassic: unknown }).readmdMountBongoClassic = async (container: HTMLElement, readState: unknown) => {
    const stage = await import('./live2d/bongo-classic')
    return stage.mountBongoClassic(container, readState as Parameters<typeof stage.mountBongoClassic>[1])
  }
  ;(window as unknown as { readmdMountLive2d: (container: HTMLElement) => Promise<unknown> }).readmdMountLive2d = async container => {
    const stage = await import('./live2d/stage')
    return stage.mountLive2dStage(container)
  }
  await import('../../readmd-pet-rust/renderer/bongocat.js')
  await (window as unknown as { readmdClassicReady?: Promise<unknown> }).readmdClassicReady
  if (requested === 'live2d') await (window as unknown as { readmdLive2dReady?: Promise<unknown> }).readmdLive2dReady
  const { mountPetLife } = await import('./pet-life')
  // A late character switch still uses the same companion controller.
  const live2d = (window as unknown as { readmdLive2dLife?: import('./live2d/stage').Live2dLifeController }).readmdLive2dLife
  mountPetLife({ live2d })
  // The presentation reports its own visible silhouette, rather than treating
  // the entire transparent canvas as an interactive surface.
  window.hermesDesktop?.petOverlay?.control({ type: 'renderer-ready', renderer: requested === 'live2d' ? 'live2d' : 'hermes-sprite' })
}

const mount = mountOverlay()

mount.catch(error => {
  console.error('pet overlay failed to mount', error)
  // Surface the failure instead of leaving a silently empty transparent
  // window: host diagnostics and tests read this flag.
  document.body.dataset.overlayMountState = 'failed'
  window.hermesDesktop?.petOverlay?.control({ type: 'renderer-failed', renderer: requested || 'hermes-sprite', code: 'pet_model_load_failed' })
})

// A host-side listener adds ReadMD file intake without altering the copied
// Hermes React tree or its pointer/drag implementation.
document.addEventListener('dragover', event => {
  if (event.dataTransfer?.types.includes('Files')) event.preventDefault()
})
document.addEventListener('drop', event => {
  if (!event.dataTransfer?.files?.length) return
  event.preventDefault()
  window.readmdPet?.dropFiles(Array.from(event.dataTransfer.files))
})
