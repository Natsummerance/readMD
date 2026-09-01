const pages = await (await fetch(process.env.READMD_PET_CDP || 'http://127.0.0.1:19387/json')).json()
const socket = new WebSocket(pages[0].webSocketDebuggerUrl)
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject })
const payload = await new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(new Error('CDP timeout')), 5000)
  socket.onmessage = event => {
    const message = JSON.parse(String(event.data))
    if (message.id === 1) {
      clearTimeout(timer)
      if (message.result.exceptionDetails) reject(new Error(message.result.exceptionDetails.text || 'page evaluation failed'))
      else resolve(message.result.result.value)
      socket.close()
    }
  }
  socket.send(JSON.stringify({
    id: 1,
    method: 'Runtime.evaluate',
    params: {
      expression: process.argv[2] || `(() => { const r = document.querySelector('canvas').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, ow: window.outerWidth, oh: window.outerHeight, sx: window.screenX, sy: window.screenY } })()`,
      awaitPromise: true,
      returnByValue: true
    }
  }))
})
console.log(JSON.stringify(payload))
