// Mirrors the upstream pause surface without replacing its PetSprite loop.
export function createRendererLoopPauseController(onChange: () => void, { pauseWhenUnfocused = true } = {}) {
  let focused = document.hasFocus()
  const refresh = () => { focused = document.hasFocus(); onChange() }
  document.addEventListener('visibilitychange', refresh)
  window.addEventListener('blur', refresh)
  window.addEventListener('focus', refresh)
  return {
    dispose: () => {
      document.removeEventListener('visibilitychange', refresh)
      window.removeEventListener('blur', refresh)
      window.removeEventListener('focus', refresh)
    },
    isPaused: () => document.visibilityState === 'hidden' || (pauseWhenUnfocused && !focused)
  }
}
