import { persistString, storedString } from '@/lib/storage'

/** The same bounds used by Hermes' zoom gesture and the ReadMD settings UI. */
export const PET_SCALE_DEFAULT = 0.33
export const PET_SCALE_MIN = 0.18
export const PET_SCALE_MAX = 0.72
const PET_SCALE_KEY = 'scale'

export type PetScaleRequester = ((method: string, params?: unknown) => Promise<unknown>) | null | undefined

function clampScale(value: number): number {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return PET_SCALE_DEFAULT
  return Math.max(PET_SCALE_MIN, Math.min(PET_SCALE_MAX, Math.round(numeric * 100) / 100))
}

export function readPetScale(): number {
  const raw = storedString(PET_SCALE_KEY)
  return raw === null ? PET_SCALE_DEFAULT : clampScale(Number(raw))
}

/**
 * Apply a scale change without inventing a second persistence protocol.
 *
 * Hermes' overlay bridge is the authoritative runtime channel: when present,
 * the Electron main process forwards the `{type: 'scale'}` command to ReadMD,
 * which persists it through the existing settings API.  The local value is
 * also kept as a restart-safe fallback for the standalone renderer.  A
 * requestGateway argument is accepted for source compatibility with Hermes;
 * it is intentionally not called with an undocumented RPC method.
 */
export function setPetScale(_requestGateway: PetScaleRequester, value: number): number {
  const next = clampScale(value)
  persistString(PET_SCALE_KEY, String(next))

  const desktop = (globalThis as {
    hermesDesktop?: { petOverlay?: { control?: (payload: { type: string; scale: number }) => void } }
  }).hermesDesktop
  desktop?.petOverlay?.control?.({ type: 'scale', scale: next })

  return next
}

/** Clear profile-scoped gallery state while keeping the window position. */
export function resetPetGallery(): void {
  try {
    const storage = (globalThis as { localStorage?: Storage }).localStorage
    storage?.removeItem('readmd.hermes-pet.' + PET_SCALE_KEY)
  } catch {
    // Storage can be unavailable in a restricted/teardown renderer.
  }
}

export function nextScaleFromWheel(current: number, deltaY: number): number {
  const delta = deltaY < 0 ? 0.03 : -0.03
  return clampScale(clampScale(current) + delta)
}
