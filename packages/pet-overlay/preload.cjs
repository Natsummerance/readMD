const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('readmdPet', {
  close: () => ipcRenderer.send('readmd-pet:close'),
  beginDrag: point => ipcRenderer.invoke('readmd-pet:drag-start', point),
  moveDrag: point => ipcRenderer.send('readmd-pet:drag-move', point),
  endDrag: () => ipcRenderer.send('readmd-pet:drag-end')
})
