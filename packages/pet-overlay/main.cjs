/*
 * ReadMD desktop-pet overlay.
 *
 * Adapted directly from NousResearch/hermes-agent (MIT), revision
 * fb27614addac115d55299bc6538ae112fd01f688, particularly
 * apps/desktop/electron/pet-overlay-ipc.ts. See
 * docs/third-party-notices/hermes-agent-pet.md.
 */

const path = require('node:path')
const { app, BrowserWindow, ipcMain, screen } = require('electron')

let overlay = null
let dragOffset = null

function clampBounds(bounds) {
  const display = screen.getDisplayNearestPoint({ x: bounds.x, y: bounds.y })
  const area = display.workArea
  const width = Math.max(96, Math.round(bounds.width || 260))
  const height = Math.max(96, Math.round(bounds.height || 260))

  return {
    width,
    height,
    x: Math.min(Math.max(area.x, Math.round(bounds.x)), area.x + area.width - width),
    y: Math.min(Math.max(area.y, Math.round(bounds.y)), area.y + area.height - height)
  }
}

function createOverlay() {
  if (overlay && !overlay.isDestroyed()) return overlay

  const area = screen.getPrimaryDisplay().workArea
  overlay = new BrowserWindow({
    ...clampBounds({ x: area.x + area.width - 292, y: area.y + area.height - 292, width: 260, height: 260 }),
    alwaysOnTop: true,
    focusable: false,
    frame: false,
    hasShadow: false,
    maximizable: false,
    minimizable: false,
    resizable: false,
    skipTaskbar: true,
    transparent: true,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      preload: path.join(__dirname, 'preload.cjs')
    }
  })
  overlay.setAlwaysOnTop(true, 'screen-saver')
  overlay.loadFile(path.join(__dirname, 'overlay.html'))
  overlay.on('closed', () => { overlay = null; dragOffset = null })
  return overlay
}

ipcMain.handle('readmd-pet:drag-start', (event, point) => {
  const win = BrowserWindow.fromWebContents(event.sender)
  if (!win || !point) return { ok: false }
  const [x, y] = win.getPosition()
  dragOffset = { x: Math.round(point.screenX) - x, y: Math.round(point.screenY) - y }
  return { ok: true }
})

ipcMain.on('readmd-pet:drag-move', (event, point) => {
  const win = BrowserWindow.fromWebContents(event.sender)
  if (!win || !point || !dragOffset) return
  const [width, height] = win.getSize()
  win.setBounds(clampBounds({
    x: Math.round(point.screenX) - dragOffset.x,
    y: Math.round(point.screenY) - dragOffset.y,
    width,
    height
  }))
})

ipcMain.on('readmd-pet:drag-end', () => { dragOffset = null })
ipcMain.on('readmd-pet:close', event => BrowserWindow.fromWebContents(event.sender)?.close())

app.whenReady().then(createOverlay)
app.on('window-all-closed', () => app.quit())
