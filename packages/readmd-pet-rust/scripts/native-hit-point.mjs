// Use the real rendered silhouette for native smoke coordinates. Node built-ins
// only; WebView2 remote debugging is enabled only by the isolated smoke runner.
const port = Number(process.argv[2]);
const targets = await fetch(`http://127.0.0.1:${port}/json/list`).then(r => r.json());
const target = targets.find(t => t.type === 'page');
if (!target) throw new Error('No isolated pet page');
const socket = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.addEventListener('open', resolve, {once:true}); socket.addEventListener('error', reject, {once:true}); });
const response = new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(new Error('Pet geometry query timed out')), 10000);
  socket.addEventListener('message', event => { const value=JSON.parse(event.data); if(value.id===1) {clearTimeout(timer);resolve(value);} });
});
socket.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{returnByValue:true,expression:`(() => {
  const pet=window.__bongoPet;
  const rects=pet?.opaqueRegions || [];
  const r=[...rects].sort((a,b)=>b.width*b.height-a.width*a.height)[0];
  if(!r) throw new Error('No opaque pet geometry');
  return {x:r.x+r.width/2,y:r.y+r.height/2,width:innerWidth,height:innerHeight,rects:rects.length};
})()`}}));
const result = await response;
socket.close();
if(result.result?.exceptionDetails || !result.result?.result?.value) throw new Error('Opaque pet geometry unavailable');
console.log(JSON.stringify(result.result.result.value));
