export const PET_SCALE_DEFAULT = 0.33
const MIN_SCALE = 0.18
const MAX_SCALE = 0.72
export function nextScaleFromWheel(current: number, deltaY: number): number {
  const delta = deltaY < 0 ? 0.03 : -0.03
  return Math.max(MIN_SCALE, Math.min(MAX_SCALE, Math.round((current + delta) * 100) / 100))
}
