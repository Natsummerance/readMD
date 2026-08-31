// Hermes's source stores only overlay preferences here.  Keep them isolated in
// the plugin profile, never in ReadMD documents or the model directory.
const prefix = 'readmd.hermes-pet.'

export function storedString(key: string): string | null {
  try { return window.localStorage.getItem(prefix + key) } catch { return null }
}
export function persistString(key: string, value: string): void {
  try { window.localStorage.setItem(prefix + key, value) } catch { /* unavailable profile */ }
}
export function storedBoolean(key: string, fallback = false): boolean {
  const value = storedString(key)
  return value === null ? fallback : value === 'true'
}
export function persistBoolean(key: string, value: boolean): void {
  persistString(key, String(Boolean(value)))
}
