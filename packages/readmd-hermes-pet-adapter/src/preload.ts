// Exact IPC contract copied from Hermes's `electron/preload.ts` petOverlay
// block.  The surrounding Hermes-only APIs are intentionally omitted; this
// adapter exposes no broader desktop privileges to the overlay renderer.
import { contextBridge, ipcRenderer, webUtils } from 'electron'

contextBridge.exposeInMainWorld('hermesDesktop', {
  petOverlay: {
    open: request => ipcRenderer.invoke('hermes:pet-overlay:open', request),
    close: () => ipcRenderer.invoke('hermes:pet-overlay:close'),
    setBounds: bounds => ipcRenderer.send('hermes:pet-overlay:set-bounds', bounds),
    setIgnoreMouse: ignore => ipcRenderer.send('hermes:pet-overlay:ignore-mouse', ignore),
    setFocusable: focusable => ipcRenderer.send('hermes:pet-overlay:set-focusable', focusable),
    pushState: payload => ipcRenderer.send('hermes:pet-overlay:state', payload),
    control: payload => ipcRenderer.send('hermes:pet-overlay:control', payload),
    onState: callback => {
      const listener = (_event: unknown, payload: unknown) => callback(payload)
      ipcRenderer.on('hermes:pet-overlay:state', listener)
      return () => ipcRenderer.removeListener('hermes:pet-overlay:state', listener)
    },
    onControl: callback => {
      const listener = (_event: unknown, payload: unknown) => callback(payload)
      ipcRenderer.on('hermes:pet-overlay:control', listener)
      return () => ipcRenderer.removeListener('hermes:pet-overlay:control', listener)
    }
  }
})

// ReadMD-only bridge: a least-privilege transfer of file paths from an actual
// desktop drag.  Renderer JavaScript cannot access Node or arbitrary paths.
contextBridge.exposeInMainWorld('readmdPet', {
  dropFiles: (files: File[]) => {
    const paths = files.slice(0, 128).map(file => webUtils.getPathForFile(file)).filter(Boolean)
    ipcRenderer.send('readmd:pet:drop', paths)
  }
})
