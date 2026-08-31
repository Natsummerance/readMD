// Generated at build time from the immutable Hermes overlay sources. The
// generation applies exactly one documented host adaptation: a single click
// asks ReadMD to show its existing quick menu rather than opening a composer.
import { mountPetOverlay } from '../.generated/overlay-root'

mountPetOverlay()

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
