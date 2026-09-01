// Overlay entry.  The pinned Hermes overlay remains the default renderer; the
// host selects ReadMD's Live2D stage with a `?renderer=live2d` query so a
// preference change swaps the page without restarting the plugin process.
const requested = new URLSearchParams(window.location.search).get('renderer')

const mount = requested === 'live2d'
  ? import('./live2d/stage').then(stage => stage.mountLive2dStage())
  : import('../.generated/overlay-root').then(root => root.mountPetOverlay())

mount.catch(error => console.error('pet overlay failed to mount', error))

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
