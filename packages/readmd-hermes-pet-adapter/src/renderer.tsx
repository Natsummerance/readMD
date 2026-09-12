// Overlay entry.  The pinned Hermes overlay remains the default renderer; the
// host selects ReadMD's Live2D stage with a `?renderer=live2d` query so a
// preference change swaps the page without restarting the plugin process.
// Both renderers get the ReadMD companion layer (speech bubbles and the idle
// companionship state machine) on top of their own mount.
const requested = new URLSearchParams(window.location.search).get('renderer')

async function mountOverlay(): Promise<void> {
  if (requested === 'live2d') {
    const stage = await import('./live2d/stage')
    const live2d = await stage.mountLive2dStage()
    const { mountPetLife } = await import('./pet-life')
    mountPetLife({ live2d })
    return
  }
  const root = await import('../.generated/overlay-root')
  await root.mountPetOverlay()
  const { mountPetLife } = await import('./pet-life')
  mountPetLife()
}

const mount = mountOverlay()

mount.catch(error => {
  console.error('pet overlay failed to mount', error)
  // Surface the failure instead of leaving a silently empty transparent
  // window: host diagnostics and tests read this flag.
  document.body.dataset.overlayMountState = 'failed'
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
