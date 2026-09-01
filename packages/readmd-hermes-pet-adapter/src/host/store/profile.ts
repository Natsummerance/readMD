import { atom } from 'nanostores'

export const $activeGatewayProfile = atom('readmd')
export const normalizeProfileKey = (value: string | undefined) => value || 'readmd'
