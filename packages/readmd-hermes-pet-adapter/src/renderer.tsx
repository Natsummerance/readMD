// Overlay entry.  The pinned Hermes overlay remains the default renderer; the
// host selects ReadMD's Live2D stage with a `?renderer=live2d` query so a
// preference change swaps the page without restarting the plugin process.
// Both renderers get the ReadMD companion layer (speech bubbles and the idle
// companionship state machine) on top of their own mount.
const requested = new URLSearchParams(window.location.search).get('renderer')

type InteractionRect = { x: number; y: number; width: number; height: number }

/**
 * Keep the native host's hit-test region aligned with the actual renderer.
 * The window is intentionally larger than the mascot so drag bounds and the
 * Live2D canvas can be laid out without clipping.  Sending the visible DOM
 * surfaces lets the Rust backend keep the surrounding transparent area
 * click-through while still delivering pointer events to the renderer.
 */
function installSpriteInteractionBridge(): () => void {
  let frame = 0
  const schedule = () => {
    if (frame !== 0) return
    frame = window.requestAnimationFrame(() => {
      frame = 0
      const rects: InteractionRect[] = []
      const elements = document.querySelectorAll('canvas, input, button, [role="button"]')
      elements.forEach(element => {
        const rect = element.getBoundingClientRect()
        if (!Number.isFinite(rect.width) || !Number.isFinite(rect.height) || rect.width <= 0 || rect.height <= 0) return
        const x = Math.max(0, rect.left)
        const y = Math.max(0, rect.top)
        const right = Math.min(window.innerWidth, rect.right)
        const bottom = Math.min(window.innerHeight, rect.bottom)
        if (right > x && bottom > y) rects.push({ x, y, width: right - x, height: bottom - y })
      })
      window.hermesDesktop?.petOverlay?.control({ type: 'interaction-regions', rects })
    })
  }

  const observer = typeof MutationObserver === 'function' ? new MutationObserver(schedule) : undefined
  observer?.observe(document.body, { childList: true, subtree: true, attributes: true })
  const resizeObserver = typeof ResizeObserver === 'function' ? new ResizeObserver(schedule) : undefined
  resizeObserver?.observe(document.documentElement)
  resizeObserver?.observe(document.body)
  window.addEventListener('resize', schedule)
  schedule()
  return () => {
    if (frame !== 0) window.cancelAnimationFrame(frame)
    observer?.disconnect()
    resizeObserver?.disconnect()
    window.removeEventListener('resize', schedule)
  }
}

async function mountOverlay(): Promise<void> {
  if (requested === 'live2d') {
    const stage = await import('./live2d/stage')
    const live2d = await stage.mountLive2dStage()
    const { mountPetLife } = await import('./pet-life')
    mountPetLife({ live2d })
    window.hermesDesktop?.petOverlay?.control({ type: 'renderer-ready', renderer: 'live2d' })
    return
  }
  const root = await import('../.generated/overlay-root')
  await root.mountPetOverlay()
  const { mountPetLife } = await import('./pet-life')
  mountPetLife()
  installSpriteInteractionBridge()
  window.hermesDesktop?.petOverlay?.control({ type: 'renderer-ready', renderer: 'hermes-sprite' })
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
