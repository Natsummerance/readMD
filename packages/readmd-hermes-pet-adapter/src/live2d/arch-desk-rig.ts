// Retarget the original Arch-chan meshes at render time. Its published model
// has a mouse hand but no typing parameter; do not invent parameter names or
// paint unrelated hands over the character. All vertex edits are restored
// before Cubism evaluates the next frame, including when the desk is hidden.
type Point = { x: number; y: number }
type Chain = [Point, Point, Point]
const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value))
const smooth = (value: number) => { const t=clamp(value,0,1); return t*t*(3-2*t) }

export function bendChain(shoulder: Point, hand: Point, side: number, slack = 1.12): Chain {
  const dx=hand.x-shoulder.x,dy=hand.y-shoulder.y,d=Math.max(0.001,Math.hypot(dx,dy))
  const reach=d*slack, a=reach*0.48,b=reach*0.52
  const along=(a*a-b*b+d*d)/(2*d), across=Math.sqrt(Math.max(0,a*a-along*along))*side
  return [shoulder,{x:shoulder.x+dx/d*along-dy/d*across,y:shoulder.y+dy/d*along+dx/d*across},hand]
}

function project(point: Point, a: Point, b: Point) {
  const dx=b.x-a.x,dy=b.y-a.y,length=Math.max(0.0001,Math.hypot(dx,dy))
  const t=((point.x-a.x)*dx+(point.y-a.y)*dy)/(length*length), nearest=clamp(t,0,1)
  return { t, normal:((point.x-a.x)*-dy+(point.y-a.y)*dx)/length,
    distance:Math.hypot(point.x-a.x-nearest*dx,point.y-a.y-nearest*dy) }
}
function warp(point: Point, from: Chain, to: Chain): Point {
  const upper=project(point,from[0],from[1]),lower=project(point,from[1],from[2])
  const index=upper.distance<lower.distance?0:1,p=index===0?upper:lower
  const a=to[index],b=to[index+1],dx=b.x-a.x,dy=b.y-a.y,length=Math.max(0.0001,Math.hypot(dx,dy))
  return {x:a.x+p.t*dx-p.normal*dy/length,y:a.y+p.t*dy+p.normal*dx/length}
}

export function mountArchDeskRig(model: any, read: () => {
  active: boolean; mouse: Point; keyboard: Point
}) {
  const core=model.internalModel.coreModel, native=core._model
  if(!native?.drawables || !native?.canvasinfo)return undefined
  const drawables=native.drawables, ids=drawables.ids as string[]
  const indices=['ArtMesh14','ArtMesh15','AAA'].map(id=>ids.indexOf(id))
  // Only the shipped model has this rig. A future model keeps its authored
  // animation if it does not expose the expected mesh IDs.
  if(indices.some(i=>i<0))return undefined
  // Skin is several disconnected islands inside one drawable. Identify the
  // complete authored right hand by topology, so fingers move rigidly together
  // and never stretch into triangles when a vertex falls across a body mask.
  const skin=drawables.vertexPositions[indices[0]] as Float32Array
  const parents=Array.from({length:skin.length/2},(_,i)=>i)
  const root=(i: number): number => parents[i]===i?i:(parents[i]=root(parents[i]))
  const triangles=drawables.indices[indices[0]] as Uint16Array
  for(let i=0;i<triangles.length;i+=3){const a=root(triangles[i]);parents[root(triangles[i+1])]=a;parents[root(triangles[i+2])]=a}
  const groups=new Map<number,number[]>()
  parents.forEach((_,i)=>{const key=root(i);if(!groups.has(key))groups.set(key,[]);groups.get(key)!.push(i)})
  const handVertices=new Set([...groups.values()].find(group=>{
    const xs=group.map(i=>skin[i*2]),ys=group.map(i=>skin[i*2+1])
    return Math.min(...xs)>0 && Math.min(...ys)>0 && Math.max(...ys)<0.16
  }) || [])
  const backups=new Map<number,Float32Array>()
  const originalUpdate=core.update.bind(core)
  const probe={ active:false, left:[] as Point[], right:[] as Point[], frames:0 }
  const pixelUnit=native.canvasinfo.PixelsPerUnit
  const width=native.canvasinfo.CanvasWidth,height=native.canvasinfo.CanvasHeight
  const toCore=(point: Point): Point => ({
    x:((point.x-model.x)/model.scale.x-width/2)/pixelUnit,
    y:(height/2-(point.y-model.y)/model.scale.x)/pixelUnit
  })
  const toScreen=(point: Point): Point => ({
    x:model.x+(point.x*pixelUnit+width/2)*model.scale.x,
    y:model.y+(-point.y*pixelUnit+height/2)*model.scale.x
  })
  core.update=() => {
    for(const [index,vertices] of backups)drawables.vertexPositions[index].set(vertices)
    backups.clear()
    originalUpdate()
    const next=read();probe.active=next.active
    if(!next.active)return
    // Model coordinates are Y-up. Shoulder positions match the original coat,
    // with its left arm raised by MouseToggle and its right hand on the hip.
    const left: Chain=[{x:-0.105,y:0.345},{x:-0.16,y:0.29},{x:-0.145,y:0.225}]
    const right: Chain=[{x:0.095,y:0.345},{x:0.17,y:0.20},{x:0.105,y:0.08}]
    const leftTarget=bendChain(left[0],toCore(next.mouse),-1)
    const rightTarget=bendChain(right[0],toCore(next.keyboard),1)
    probe.left=leftTarget.map(toScreen);probe.right=rightTarget.map(toScreen);probe.frames++
    for(const index of indices){
      const positions=drawables.vertexPositions[index] as Float32Array
      backups.set(index,new Float32Array(positions))
      for(let j=0;j<positions.length;j+=2){
        const point={x:positions[j],y:positions[j+1]}
        // Hide the seated model's legs as degenerate triangles, retaining its
        // face, coat and all arm texture pixels. No lower-body fragments can
        // leak through the hand compositing masks in front of the keyboard.
        if(ids[index]!=='AAA' && point.y<0){positions[j]=0;positions[j+1]=0;continue}
        let weight=0,from=left,to=leftTarget
        if(ids[index]==='AAA')weight=1
        else if(ids[index]==='ArtMesh14'){
          if(!handVertices.has(j/2))continue
          weight=1;from=right;to=rightTarget
        }
        else if(point.y>0.025 && point.y<0.37){
          const shoulderFade=smooth((0.37-point.y)/0.075)
          if(point.x< -0.075)weight=smooth((-point.x-0.075)/0.04)*shoulderFade
          else if(point.x>0.075){weight=smooth((point.x-0.075)/0.045)*shoulderFade;from=right;to=rightTarget}
        }
        if(weight<=0)continue
        let mapped=warp(point,from,to)
        if(ids[index]==='AAA' || ids[index]==='ArtMesh14'){
          const angle=Math.atan2(to[2].y-to[1].y,to[2].x-to[1].x)-Math.atan2(from[2].y-from[1].y,from[2].x-from[1].x)
          const dx=point.x-from[2].x,dy=point.y-from[2].y
          mapped={x:to[2].x+dx*Math.cos(angle)-dy*Math.sin(angle),y:to[2].y+dx*Math.sin(angle)+dy*Math.cos(angle)}
        }
        positions[j]+= (mapped.x-point.x)*weight
        positions[j+1]+= (mapped.y-point.y)*weight
      }
      drawables.dynamicFlags[index]|=0x20
    }
  }
  return probe
}
