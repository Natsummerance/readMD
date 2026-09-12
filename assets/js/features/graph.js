// Three-dimensional layout, perspective projection and accessible note navigation.
(function () {
  'use strict';
  const t = (key, params) => window.i18n?.t(key, params) || key;
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const api = (url) => (typeof apiFetch === 'function' ? apiFetch(url) : fetch(url)).then(async res => {
    const data = await res.json();
    if (!res.ok || !data.ok) throw new Error(data.error || `HTTP ${res.status}`);
    return data;
  });
  let modal, canvas, ctx, drawer, opener, requestId = 0, backlinkRequest = 0;
  let nodes = [], edges = [], projected = [], selected = null, hovered = null, currentFile = null;
  let yaw = .35, pitch = -.22, zoom = 1, panX = 0, panY = 0, frame = 0, ticks = 0;
  let is3d = true, onlyNeighbors = false, labels = true, query = '', linkQuery = '', linkTab = 'incoming';
  let backlinkData = { backlinks: [], forward_links: [] };
  const pointers = new Map();
  let gesture = null;
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
  const visible = () => modal && !modal.classList.contains('hidden');
  const activeDoc = () => typeof state !== 'undefined' && ['file', 'virtual'].includes(state.mode) && !!state.original;
  const basename = path => String(path || '').split(/[\\/]/).pop();

  function createModal() {
    if (modal) return;
    modal = document.createElement('div');
    modal.id = 'graph-modal'; modal.className = 'graph-modal hidden';
    modal.setAttribute('role', 'dialog'); modal.setAttribute('aria-modal', 'true'); modal.setAttribute('aria-labelledby', 'graph-title');
    modal.innerHTML = `<div class="graph-backdrop"></div><div class="graph-dialog">
      <header class="graph-header"><div class="graph-title-row"><h3 id="graph-title">${t('graph.title')}</h3><span id="graph-stats-badge" class="graph-stats-badge" aria-live="polite"></span></div>
      <div class="graph-toolbar"><button id="graph-btn-dimension" class="graph-tool-btn" aria-pressed="true">3D</button><button id="graph-btn-reset" class="graph-tool-btn" title="${t('graph.reset')}">↺</button><button id="graph-btn-close" class="graph-tool-btn" aria-label="${t('toolbar.close')}">×</button></div></header>
      <div class="graph-workspace"><aside class="graph-sidebar"><label class="ux-search"><span>⌕</span><input id="graph-search" type="search" placeholder="${t('ux.graphSearch')}" aria-label="${t('ux.graphSearch')}"></label>
      <div class="graph-options"><label><input id="graph-labels" type="checkbox" checked> ${t('ux.labels')}</label><label><input id="graph-neighbors" type="checkbox"> ${t('ux.neighbors')}</label></div>
      <div id="graph-node-list" class="graph-node-list" aria-label="${t('ux.notes')}"></div></aside>
      <div class="graph-body"><canvas id="graph-canvas" tabindex="0" aria-label="${t('ux.graphControls')}"></canvas>
      <div id="graph-loading" class="graph-loading hidden" role="status"></div>
      <div id="graph-tooltip" class="graph-tooltip hidden"></div>
      <div class="graph-viewport-tools"><button id="graph-btn-zoom-out" class="graph-tool-btn" aria-label="${t('graph.zoomOut')}">−</button><output id="graph-zoom">100%</output><button id="graph-btn-zoom-in" class="graph-tool-btn" aria-label="${t('graph.zoomIn')}">+</button></div>
      <p class="graph-control-hint">${t('ux.graphControls')}</p>
      <section id="graph-detail" class="graph-detail hidden" aria-live="polite"></section></div></div></div>`;
    document.body.appendChild(modal);
    canvas = modal.querySelector('canvas'); ctx = canvas.getContext('2d');
    modal.querySelector('.graph-backdrop').onclick = close;
    modal.querySelector('#graph-btn-close').onclick = close;
    modal.querySelector('#graph-btn-reset').onclick = reset;
    modal.querySelector('#graph-btn-zoom-in').onclick = () => setZoom(zoom * 1.2);
    modal.querySelector('#graph-btn-zoom-out').onclick = () => setZoom(zoom / 1.2);
    modal.querySelector('#graph-btn-dimension').onclick = e => {
      is3d = !is3d; e.currentTarget.textContent = is3d ? '3D' : '2D';
      e.currentTarget.setAttribute('aria-pressed', String(is3d)); render();
    };
    modal.querySelector('#graph-search').oninput = e => { query = e.target.value.trim().toLowerCase(); renderList(); render(); };
    modal.querySelector('#graph-labels').onchange = e => { labels = e.target.checked; render(); };
    modal.querySelector('#graph-neighbors').onchange = e => { onlyNeighbors = e.target.checked; renderList(); render(); };
    modal.addEventListener('keydown', e => {
      if (e.key === 'Escape') { e.stopPropagation(); close(); }
      if (e.key === 'Tab') trapFocus(e, modal);
    });
    new ResizeObserver(() => { if (visible()) resize(); }).observe(modal.querySelector('.graph-body'));
    canvas.addEventListener('wheel', e => { e.preventDefault(); setZoom(zoom * Math.exp(-e.deltaY * .001)); }, { passive: false });
    canvas.oncontextmenu = e => e.preventDefault();
    canvas.onpointerdown = e => {
      if (e.button > 1) return;
      canvas.setPointerCapture(e.pointerId); canvas.focus({ preventScroll: true });
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      gesture = { x: e.clientX, y: e.clientY, moved: false, pan: e.shiftKey || e.button === 1 || !is3d };
    };
    canvas.onpointermove = e => {
      const old = pointers.get(e.pointerId);
      if (old && gesture) {
        const dx = e.clientX - old.x, dy = e.clientY - old.y;
        if (Math.hypot(e.clientX - gesture.x, e.clientY - gesture.y) > 4) gesture.moved = true;
        const other = [...pointers.entries()].find(([id]) => id !== e.pointerId)?.[1];
        if (other) {
          const before = Math.hypot(old.x - other.x, old.y - other.y);
          if (before > 1) setZoom(zoom * Math.hypot(e.clientX - other.x, e.clientY - other.y) / before);
          gesture.moved = true;
        } else if (gesture.pan) { panX += dx; panY += dy; }
        else { yaw += dx * .007; pitch = Math.max(-1.4, Math.min(1.4, pitch + dy * .007)); }
        pointers.set(e.pointerId, { x: e.clientX, y: e.clientY }); render();
      } else {
        const rect = canvas.getBoundingClientRect();
        hovered = hit(e.clientX - rect.left, e.clientY - rect.top)?.node || null;
        canvas.style.cursor = hovered ? 'pointer' : 'grab'; render();
      }
    };
    canvas.onpointerup = e => {
      if (gesture && !gesture.moved) {
        const r = canvas.getBoundingClientRect(); select(hit(e.clientX - r.left, e.clientY - r.top)?.node || null);
      }
      pointers.delete(e.pointerId); if (!pointers.size) gesture = null;
    };
    canvas.onpointercancel = e => { pointers.delete(e.pointerId); gesture = null; };
    canvas.onpointerleave = () => { hovered = null; render(); };
    canvas.onkeydown = e => {
      if (e.key === '+') setZoom(zoom * 1.2);
      else if (e.key === '-') setZoom(zoom / 1.2);
      else if (e.key === '0') reset();
      else if (e.key === 'ArrowLeft') yaw -= .15;
      else if (e.key === 'ArrowRight') yaw += .15;
      else if (e.key === 'ArrowUp') pitch -= .1;
      else if (e.key === 'ArrowDown') pitch += .1;
      else return;
      e.preventDefault(); render();
    };
  }
  function trapFocus(e, parent) {
    const items = [...parent.querySelectorAll('button,input,select,textarea,[tabindex="0"]')].filter(el => !el.disabled && el.getClientRects().length);
    const first = items[0], last = items.at(-1);
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
  }
  function matches(n) { return (!query || `${n.label} ${n.path}`.toLowerCase().includes(query)) && (!onlyNeighbors || !selected || n === selected || selected.neighbors.has(n.id)); }
  function renderList() {
    const list = modal.querySelector('#graph-node-list');
    const filtered = nodes.filter(matches);
    list.innerHTML = filtered.length ? filtered.map(n => `<button class="graph-node-row ${n === selected ? 'selected' : ''}" data-node="${esc(n.id)}" aria-pressed="${n === selected}"><span class="graph-node-dot ${n.is_deadlink ? 'dead' : ''}"></span><span>${esc(n.label)}</span><small>${n.neighbors.size}</small></button>`).join('') : `<p class="ux-empty">${t('ux.noResults')}</p>`;
    list.querySelectorAll('button').forEach(el => { el.onclick = () => select(nodes.find(n => String(n.id) === el.dataset.node)); });
  }
  function select(n) {
    selected = n; renderList(); render();
    const detail = modal.querySelector('#graph-detail'); detail.classList.toggle('hidden', !n);
    if (!n) return;
    detail.innerHTML = `<div><strong>${esc(n.label)}</strong><p>${esc(n.path || t('graph.deadlinkUncreated'))}</p><small>${t('ux.connections', { count: n.neighbors.size })}</small></div><button class="tb-btn accent" ${!n.path ? 'disabled' : ''}>${t('ux.openNote')}</button>`;
    detail.querySelector('button').onclick = () => { close(); window.loadFile?.(n.path); };
  }
  function init(data) {
    selected = hovered = null; query = ''; modal.querySelector('#graph-search').value = ''; modal.querySelector('#graph-detail').classList.add('hidden');
    const raw = (data.nodes || []).slice(0, 500), count = raw.length;
    nodes = raw.map((n, i) => {
      const z = 1 - 2 * (i + .5) / Math.max(count, 1), phi = i * 2.399963;
      const r = Math.max(100, Math.sqrt(count) * 21), ring = Math.sqrt(1 - z * z);
      return { ...n, label: String(n.label || basename(n.path) || n.id), x: Math.cos(phi) * ring * r, y: Math.sin(phi) * ring * r, z: z * r, vx: 0, vy: 0, vz: 0, neighbors: new Set(), radius: Math.min(13, 5 + Math.sqrt(n.degree || 0) * 2) };
    });
    const byId = new Map(nodes.map(n => [n.id, n]));
    edges = (data.edges || []).map(e => ({ s: byId.get(e.source), t: byId.get(e.target) })).filter(e => e.s && e.t);
    edges.forEach(({s, t}) => { s.neighbors.add(t.id); t.neighbors.add(s.id); });
    modal.querySelector('#graph-stats-badge').textContent = t('graph.statsSimple', { nodes: count, links: edges.length });
    renderList(); reset(); ticks = 0; wake();
  }
  function physics() {
    for (let i = 0; i < nodes.length; i++) for (let j = i + 1; j < nodes.length; j++) {
      const a = nodes[i], b = nodes[j], dx = b.x - a.x, dy = b.y - a.y, dz = b.z - a.z;
      const d2 = dx*dx + dy*dy + dz*dz + 4, f = 1800 / (d2 * Math.sqrt(d2));
      a.vx -= dx*f; a.vy -= dy*f; a.vz -= dz*f; b.vx += dx*f; b.vy += dy*f; b.vz += dz*f;
    }
    edges.forEach(({s, t}) => {
      const dx = t.x-s.x, dy = t.y-s.y, dz = t.z-s.z, d = Math.hypot(dx,dy,dz) || 1, f = (d-100)*.016/d;
      s.vx += dx*f; s.vy += dy*f; s.vz += dz*f; t.vx -= dx*f; t.vy -= dy*f; t.vz -= dz*f;
    });
    let energy = 0;
    nodes.forEach(n => {
      for (const axis of ['x','y','z']) { const v = 'v'+axis; n[v] = Math.max(-8, Math.min(8, (n[v]-n[axis]*.002)*.83)); n[axis] += n[v]; energy += Math.abs(n[v]); }
    });
    return energy;
  }
  function wake() {
    cancelAnimationFrame(frame);
    const loop = () => {
      if (!visible() || document.hidden) { frame = 0; return; }
      const energy = physics(); render(); ticks++;
      if (!reduced() && ticks < 200 && energy > .3) frame = requestAnimationFrame(loop); else frame = 0;
    }; frame = requestAnimationFrame(loop);
  }
  function resize() {
    const r = canvas.parentElement.getBoundingClientRect(), d = Math.min(devicePixelRatio || 1, 2);
    canvas.width = Math.round(r.width*d); canvas.height = Math.round(r.height*d);
    canvas.style.width = `${r.width}px`; canvas.style.height = `${r.height}px`; render();
  }
  function reset() { yaw = .35; pitch = -.22; panX = panY = 0; zoom = Math.min(1.5, 280 / Math.max(180, Math.sqrt(nodes.length)*21)); render(); }
  function setZoom(value) { zoom = Math.max(.15, Math.min(4, value)); render(); }
  function render() {
    if (!ctx || !visible()) return;
    const w = canvas.clientWidth, h = canvas.clientHeight, d = Math.min(devicePixelRatio || 1, 2);
    ctx.setTransform(d,0,0,d,0,0); ctx.clearRect(0,0,w,h);
    const style = getComputedStyle(document.body), fg = style.getPropertyValue('--fg').trim() || '#172133', accent = style.getPropertyValue('--accent').trim() || '#3478f6';
    const cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch);
    projected = nodes.map(node => {
      const rx = is3d ? node.x*cy-node.z*sy : node.x, rz = is3d ? node.x*sy+node.z*cy : 0;
      const ry = is3d ? node.y*cp-rz*sp : node.y, depth = is3d ? node.y*sp+rz*cp : 0;
      const perspective = 900 / Math.max(300, 900+depth), scale = perspective*zoom;
      return { node, x: w/2+panX+rx*scale, y: h/2+panY+ry*scale, depth, r: Math.max(3,node.radius*scale), scale, match: matches(node) };
    }).sort((a,b) => b.depth-a.depth);
    const map = new Map(projected.map(p => [p.node.id,p]));
    edges.forEach(({s,t}) => {
      const a=map.get(s.id),b=map.get(t.id), focus = selected || hovered;
      ctx.globalAlpha = a.match && b.match ? (focus && (s===focus||t===focus) ? .65 : .14) : .035;
      ctx.strokeStyle = t.is_deadlink ? '#df6363' : focus && (s===focus||t===focus) ? accent : fg;
      ctx.lineWidth = 1; ctx.setLineDash(t.is_deadlink ? [3,4] : []); ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);ctx.stroke();
    }); ctx.setLineDash([]);
    projected.forEach(p => {
      const n=p.node, focus=n===selected||n===hovered||n.path===currentFile;
      ctx.globalAlpha=p.match ? Math.max(.45,Math.min(1,p.scale/zoom)) : .13;
      if (focus) { ctx.beginPath();ctx.arc(p.x,p.y,p.r+5,0,Math.PI*2);ctx.strokeStyle=accent;ctx.lineWidth=1.5;ctx.stroke(); }
      ctx.beginPath();ctx.arc(p.x,p.y,p.r,0,Math.PI*2);
      const fill=ctx.createRadialGradient(p.x-p.r*.3,p.y-p.r*.35,0,p.x,p.y,p.r);
      fill.addColorStop(0,n.is_deadlink?'#f9aaaa':'#b7d6ff');fill.addColorStop(1,n.is_deadlink?'#cc5252':focus?accent:'#547da9');ctx.fillStyle=fill;ctx.fill();
      if (p.match && (focus || query || (labels && (nodes.length<75 || n.neighbors.size>3)))) {
        ctx.globalAlpha=focus?1:.78;ctx.fillStyle=fg;ctx.font=`${focus?'600':'400'} 12px system-ui`;ctx.textAlign='center';
        ctx.fillText(n.label.length>24?n.label.slice(0,22)+'…':n.label,p.x,p.y+p.r+17);
      }
    }); ctx.globalAlpha=1;
    modal.querySelector('#graph-zoom').textContent = `${Math.round(zoom*100)}%`;
    canvas.dataset.dimensions = is3d ? '3' : '2'; canvas.dataset.yaw = yaw.toFixed(3); canvas.dataset.nodeCount = String(nodes.length);
  }
  function hit(x,y) { return [...projected].reverse().find(p => p.match && Math.hypot(x-p.x,y-p.y)<p.r+8); }
  async function open(directory) {
    if (!activeDoc()) { if (typeof showToast==='function') showToast(t('toast.openDocumentToUse')); return; }
    createModal(); opener=document.activeElement; modal.classList.remove('hidden'); resize();
    modal.querySelector('#graph-search').focus();
    const id=++requestId, loading=modal.querySelector('#graph-loading'); loading.classList.remove('hidden'); loading.textContent=t('graph.loading');
    try {
      const res=window.pywebview?.api?.get_links_graph ? await window.pywebview.api.get_links_graph(directory||'') : await api(`/api/links/graph?dir=${encodeURIComponent(directory||'')}&max_nodes=500`);
      if(id!==requestId||!visible()) return;
      if(!res?.ok||!res.graph) throw new Error('graph_unavailable');
      init(res.graph); loading.classList.toggle('hidden',nodes.length>0); loading.textContent=t('graph.noLinks');
    } catch (_) { if(id===requestId) { nodes=[];edges=[];render();renderList();loading.textContent=t('ux.loadFailed'); } }
  }
  function close() { if(!modal) return; ++requestId; modal.classList.add('hidden');cancelAnimationFrame(frame);frame=0;pointers.clear();gesture=null;opener?.isConnected&&opener.focus({preventScroll:true}); }

  function createDrawer() {
    if(drawer) return;
    drawer=document.createElement('aside');drawer.id='backlinks-panel';drawer.className='backlinks-panel hidden';drawer.setAttribute('aria-labelledby','backlinks-title');
    drawer.innerHTML=`<header class="backlinks-header"><div><h3 id="backlinks-title">${t('graph.backlinks')}</h3><p id="backlinks-file"></p></div><button id="backlinks-btn-close" class="graph-tool-btn" aria-label="${t('toolbar.close')}">×</button></header>
      <label class="ux-search"><span>⌕</span><input id="backlinks-search" type="search" placeholder="${t('ux.filterLinks')}" aria-label="${t('ux.filterLinks')}"></label>
      <div class="backlinks-tabs" role="tablist"><button data-tab="incoming" role="tab" aria-selected="true"></button><button data-tab="outgoing" role="tab" aria-selected="false"></button></div>
      <div id="backlinks-content" class="backlinks-content" role="tabpanel" aria-live="polite"></div>`;
    document.body.appendChild(drawer);
    drawer.querySelector('#backlinks-btn-close').onclick=()=>{drawer.classList.add('hidden');document.getElementById('btn-backlinks-menu')?.focus();};
    drawer.querySelector('#backlinks-search').oninput=e=>{linkQuery=e.target.value.trim().toLowerCase();renderLinks();};
    drawer.querySelectorAll('[data-tab]').forEach(btn=>{
      btn.onclick=()=>{linkTab=btn.dataset.tab;renderLinks();};
      btn.onkeydown=e=>{if(e.key==='ArrowLeft'||e.key==='ArrowRight'){e.preventDefault();const next=drawer.querySelector(`[data-tab="${linkTab==='incoming'?'outgoing':'incoming'}"]`);next.click();next.focus();}};
    });
  }
  function renderLinks() {
    const incoming=linkTab==='incoming', all=incoming?backlinkData.backlinks:backlinkData.forward_links;
    drawer.querySelectorAll('[data-tab]').forEach(btn=>{
      const isIn=btn.dataset.tab==='incoming';btn.textContent=`${t(isIn?'graph.backlinks':'graph.outgoing')} · ${(isIn?backlinkData.backlinks:backlinkData.forward_links).length}`;
      btn.setAttribute('aria-selected',String(btn.dataset.tab===linkTab));btn.tabIndex=btn.dataset.tab===linkTab?0:-1;
    });
    const rows=all.filter(item=>JSON.stringify(item).toLowerCase().includes(linkQuery));
    const content=drawer.querySelector('#backlinks-content');
    content.innerHTML=rows.length?rows.map(item=>{
      const path=incoming?item.source_path:item.target_path, title=incoming?(item.source_title||basename(path)):(item.target_clean||item.target_raw);
      return `<button class="backlink-item ${!path?'deadlink':''}" data-path="${esc(path||'')}" ${!path?'disabled':''}><span class="backlink-title">${esc(title)}</span><span class="backlink-context">${esc(item.alias||'')}${item.line_no?' · '+t('graph.linePrefix',{line:item.line_no}):''}</span><span class="backlink-path">${esc(path||t('graph.deadlinkUncreated'))}</span></button>`;
    }).join(''):`<div class="ux-empty"><strong>${t(linkQuery?'ux.noResults':'graph.noLinks')}</strong><p>${t('ux.linkHint')}</p></div>`;
    content.querySelectorAll('button[data-path]').forEach(btn=>{btn.onclick=()=>window.loadFile?.(btn.dataset.path);});
  }
  async function refreshBacklinks(filePath) {
    currentFile=filePath||null;createDrawer();const id=++backlinkRequest;
    drawer.querySelector('#backlinks-file').textContent=basename(filePath)||t('ux.notes');
    backlinkData={backlinks:[],forward_links:[]};renderLinks();
    if(!filePath){updateVisibility(false);return;}
    drawer.querySelector('#backlinks-content').textContent=t('graph.loading');
    try {
      const res=window.pywebview?.api?.get_backlinks?await window.pywebview.api.get_backlinks(filePath):await api(`/api/links/backlinks?path=${encodeURIComponent(filePath)}`);
      if(id!==backlinkRequest)return;if(!res?.ok)throw new Error('backlinks_unavailable');
      backlinkData={backlinks:res.backlinks||[],forward_links:res.forward_links||[]};renderLinks();
      updateVisibility(backlinkData.backlinks.length>0||backlinkData.forward_links.length>0||hasGraph());
    }catch(_){if(id===backlinkRequest){drawer.querySelector('#backlinks-content').textContent=t('ux.loadFailed');updateVisibility();}}
  }
  function toggleDrawer(){if(!activeDoc())return;createDrawer();drawer.classList.toggle('hidden');if(!drawer.classList.contains('hidden')){refreshBacklinks(currentFile||state.path);drawer.querySelector('input').focus();}}
  const WIKILINK_RE=/\[\[([^\]\n|#]+)(?:#([^\]\n|]+))?(?:\|([^\]\n]+))?\]\]/;
  function hasGraph(content){if(typeof content==='string')return WIKILINK_RE.test(content);return activeDoc()&&WIKILINK_RE.test(state.fixed||state.original||'');}
  function updateVisibility(force){const btn=document.getElementById('btn-graph');if(btn){btn.disabled=!activeDoc();btn.classList.toggle('hidden',!activeDoc()||!(force??hasGraph()));}}
  document.addEventListener('visibilitychange',()=>{if(visible()){if(document.hidden){cancelAnimationFrame(frame);frame=0;}else wake();}});
  new MutationObserver(()=>render()).observe(document.body,{attributes:true,attributeFilter:['data-theme','class']});
  window.ReadMDGraph={open,close,refreshBacklinks,toggleDrawer,updateVisibility,hasGraph};
})();
