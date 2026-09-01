import { atom } from 'nanostores'

// Host state only.  These atoms satisfy the upstream pet module's narrow
// overlay contract; ReadMD's Python core remains the source of truth.
export const $busy = atom(false)
export const $awaitingResponse = atom(false)
export const setBusy = (value: boolean) => $busy.set(Boolean(value))
export const setAwaitingResponse = (value: boolean) => $awaitingResponse.set(Boolean(value))
