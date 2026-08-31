import fs from 'node:fs'

const endpoint = process.env.READMD_PET_CDP || 'http://127.0.0.1:19387/json'
const pages = await (await fetch(endpoint)).json()
const page = pages.find(item => item.type === 'page')
if (!page?.webSocketDebuggerUrl) throw new Error('pet page is not debuggable')

const socket = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((resolve, reject) => {
  socket.addEventListener('open', resolve, { once: true })
  socket.addEventListener('error', reject, { once: true })
})
let nextId = 1
const pending = new Map()
const diagnostics = []
socket.addEventListener('message', event => {
  const message = JSON.parse(String(event.data))
  if (message.method === 'Runtime.exceptionThrown') diagnostics.push(message.params.exceptionDetails.text)
  if (message.method === 'Log.entryAdded') diagnostics.push(message.params.entry.text)
  const resolve = pending.get(message.id)
  if (resolve) { pending.delete(message.id); resolve(message) }
})
const call = (method, params = {}) => new Promise((resolve, reject) => {
  const id = nextId++
  pending.set(id, resolve)
  socket.send(JSON.stringify({ id, method, params }))
  setTimeout(() => { if (pending.delete(id)) reject(new Error(`timeout: ${method}`)) }, 5000)
})
const evaluate = async expression => {
  const response = await call('Runtime.evaluate', { expression, returnByValue: true })
  if (response.result?.exceptionDetails) throw new Error(response.result.exceptionDetails.text)
  return response.result?.result?.value
}

if (process.env.READMD_PET_DIAG === '1') {
  await call('Log.enable')
  await call('Runtime.enable')
  await call('Page.reload', { ignoreCache: true })
  await new Promise(resolve => setTimeout(resolve, 500))
}

const state = await evaluate(`(() => {
  const canvas = document.querySelector('canvas')
  return {
    canvas: canvas ? { width: canvas.width, height: canvas.height, cssWidth: canvas.style.width, cssHeight: canvas.style.height } : null,
    hasHermesBridge: Boolean(window.hermesDesktop?.petOverlay),
    rootHtml: document.getElementById('root')?.innerHTML.slice(0, 1000) || '',
    rootBackground: getComputedStyle(document.documentElement).backgroundColor,
    bodyBackground: getComputedStyle(document.body).backgroundColor,
    title: document.title
  }
})()`)
if (diagnostics.length) state.diagnostics = diagnostics
state.receivedInfo = await evaluate(`new Promise(resolve => {
  let settled = false
  const off = window.hermesDesktop?.petOverlay?.onState(payload => {
    if (settled) return
    settled = true
    off?.()
    const info = payload?.info || {}
    resolve({ enabled: info.enabled, hasSprite: Boolean(info.spritesheetBase64), spriteLength: String(info.spritesheetBase64 || '').length })
  })
  window.hermesDesktop?.petOverlay?.control({ type: 'ready' })
  setTimeout(() => { if (!settled) resolve({ timeout: true }) }, 500)
})`)
if (!state.hasHermesBridge || !state.canvas) throw new Error(`overlay did not mount: ${JSON.stringify(state)}`)
if (process.env.READMD_PET_EXERCISE === '1') {
  const rect = await evaluate(`(() => { const r = document.querySelector('canvas').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 } })()`)
  const before = await evaluate('({ x: window.screenX, y: window.screenY, width: window.outerWidth, height: window.outerHeight, canvas: document.querySelector("canvas").style.width })')
  await call('Input.dispatchMouseEvent', { type: 'mousePressed', x: rect.x, y: rect.y, button: 'left', buttons: 1, clickCount: 1 })
  await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x + 30, y: rect.y + 18, button: 'left', buttons: 1 })
  await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x: rect.x + 30, y: rect.y + 18, button: 'left', buttons: 0, clickCount: 1 })
  await new Promise(resolve => setTimeout(resolve, 350))
  const afterDrag = await evaluate('({ x: window.screenX, y: window.screenY })')
  if (Math.abs(afterDrag.x - before.x) < 20 || Math.abs(afterDrag.y - before.y) < 10) throw new Error(`native drag did not move: ${JSON.stringify({ before, afterDrag })}`)
  await call('Input.dispatchMouseEvent', { type: 'mouseWheel', x: rect.x, y: rect.y, deltaX: 0, deltaY: -100, modifiers: 1 })
  await new Promise(resolve => setTimeout(resolve, 350))
  const afterWheel = await evaluate('({ width: window.outerWidth, height: window.outerHeight, canvas: document.querySelector("canvas").style.width })')
  if (afterWheel.canvas === before.canvas) throw new Error(`Hermes zoom gesture did not change scale: ${JSON.stringify({ before, afterWheel })}`)
  state.exercise = { afterDrag, afterWheel, before }
}
if (process.env.READMD_PET_QUICK_MENU === '1') {
  const bridge = process.env.READMD_PET_BRIDGE
  if (!bridge) throw new Error('READMD_PET_BRIDGE is required for quick-menu verification')
  try { fs.unlinkSync(`${bridge}.command`) } catch { /* no prior command */ }
  await evaluate("window.hermesDesktop.petOverlay.control({ type: 'open-menu' })")
  await new Promise(resolve => setTimeout(resolve, 150))
  const command = JSON.parse(fs.readFileSync(`${bridge}.command`, 'utf8')).command
  if (command?.type !== 'open-menu') throw new Error(`quick menu was not bridged: ${JSON.stringify(command)}`)
  state.quickMenu = command
}
const captured = await call('Page.captureScreenshot', { format: 'png' })
if (process.env.READMD_PET_SCREENSHOT) fs.writeFileSync(process.env.READMD_PET_SCREENSHOT, Buffer.from(captured.result.data, 'base64'))
console.log(JSON.stringify(state))
socket.close()
