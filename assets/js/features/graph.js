// Knowledge graph "cosmos": a 3D force layout drawn on a 2D canvas with layered motion —
// warp-in starfield, drifting nebula, meteors, cluster colours, glowing orbs, curved gradient
// edges with flowing comets, hop-by-hop selection wave, sonar on the current note, orbit rings,
// click shockwaves, node dragging, rotation inertia, zoom-to-cursor, fly-to focus, 2D/3D morph
// and idle auto-rotation. prefers-reduced-motion (or the effects toggle) gives a calm frame.
(function () {
  const GRAPH_FALLBACKS = {
    'graph.title': '知识图谱',
    'graph.reset': '重置视角',
    'toolbar.close': '关闭',
    'graph.zoomOut': '缩小',
    'graph.zoomIn': '放大',
    'ux.graphSearch': '搜索节点与别名',
    'ux.labels': '显示名称',
    'ux.neighbors': '仅邻近关联',
    'ux.notes': '关联笔记列表',
    'ux.graphControls': '拖拽旋转 · 滚轮缩放 · 2D/3D切换',
    'ux.noResults': '未找到匹配节点',
    'ux.connections': '{count} 处关联',
    'ux.openNote': '打开笔记',
    'ux.loadFailed': '加载失败',
    'ux.filterLinks': '过滤双向链接...',
    'ux.linkHint': '使用 [[双链]] 建立笔记关联',
    'graph.noLinks': '暂无双向链接',
    'graph.deadlinkUncreated': '未创建的笔记',
    'graph.effects': '流光特效',
    'graph.hint': '拖拽节点牵引 · 双击打开笔记 · Shift+拖拽平移',
    'graph.indexing': '正在建立链接索引…'
  };
  const t = (key, params) => {
    const val = window.i18n?.t(key, params);
    if (val && val !== key) return val;
    let fallback = GRAPH_FALLBACKS[key] || key;
    if (typeof fallback === 'string' && params) {
      for (const [k, v] of Object.entries(params)) {
        fallback = fallback.replace(new RegExp(`\\{${k}\\}`, 'g'), v);
      }
    }
    return fallback;
  };
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const call = (url, opts) => (typeof apiFetch === 'function' ? apiFetch(url, opts) : fetch(url, opts)).then(async res => {
    const data = await res.json();
    if (!res.ok || !data.ok) throw new Error(data.error || `HTTP ${res.status}`);
    return data;
  });
  const api = url => call(url);
  const HUES = [205, 275, 325, 165, 38, 120, 245, 190, 300, 15], WAVE = 170, TAU = Math.PI * 2;
  let modal, canvas, ctx, drawer, opener, tip, zoomOut, requestId = 0, backlinkRequest = 0;
  let nodes = [], edges = [], projected = [], byKey = new Map(), projMap = new Map();
  let selected = null, hovered = null, currentFile = null, followNode = null;
  let yaw = .35, pitch = -.22, vYaw = 0, vPitch = 0, morph = 1, is3d = true;
  const cam = { zoom: 1, panX: 0, panY: 0, tx: 0, ty: 0, tz: 0 };
  const goal = { zoom: 1, panX: 0, panY: 0, tx: 0, ty: 0, tz: 0 };
  let raf = 0, lastFrame = 0, ticks = 0, energy = 0, introStart = -1e9, waveStart = 0, lastInteract = 0, graphRadius = 200;
  let onlyNeighbors = false, labels = true, query = '', linkQuery = '', linkTab = 'incoming', animateList = false;
  let hops = new Map(), shocks = [], meteors = [], stars = [];
  let backlinkData = { backlinks: [], forward_links: [] };
  let fxOn = true;
  try { fxOn = localStorage.getItem('readmd-graph-fx') !== '0'; } catch (_) { /* storage unavailable */ }
  const pointers = new Map();
  let gesture = null;
  const sprites = new Map();
  const FONT = 'ui-sans-serif, system-ui, "Segoe UI", "Microsoft YaHei", sans-serif';
  const NEBULA = [{ x: .24, y: .3, r: .55, h: 265, a: .13 }, { x: .78, y: .66, r: .5, h: 195, a: .11 }, { x: .56, y: .14, r: .38, h: 320, a: .07 }];
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
  const fx = () => fxOn && !reduced();
  const visible = () => modal && !modal.classList.contains('hidden');
  const activeDoc = () => typeof state !== 'undefined' && ['file', 'virtual'].includes(state.mode) && !!state.original;
  const basename = path => String(path || '').split(/[\\/]/).pop();
  const normPath = p => String(p || '').replace(/\\/g, '/').toLowerCase();
  const isCurrent = n => n.key === '__current__' || (!!currentFile && !!n.path && normPath(n.path) === normPath(currentFile));
  const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
  const easeOutCubic = x => 1 - Math.pow(1 - x, 3);
  const easeOutBack = x => { const c = 1.70158; return 1 + (c + 1) * Math.pow(x - 1, 3) + c * Math.pow(x - 1, 2); };

  // Pre-rendered glow and orb sprites per hue keep hundreds of nodes cheap to draw every frame.
  function sprite(kind, hue) {
    const key = kind + hue;
    if (sprites.has(key)) return sprites.get(key);
    const c = document.createElement('canvas'), size = kind === 'glow' ? 96 : 64, g = c.getContext('2d'), m = size / 2;
    c.width = c.height = size;
    if (kind === 'glow') {
      const grad = g.createRadialGradient(m, m, 0, m, m, m);
      grad.addColorStop(0, `hsla(${hue},100%,90%,.95)`); grad.addColorStop(.16, `hsla(${hue},100%,72%,.6)`);
      grad.addColorStop(.42, `hsla(${hue},95%,56%,.17)`); grad.addColorStop(1, `hsla(${hue},95%,50%,0)`);
      g.fillStyle = grad; g.fillRect(0, 0, size, size);
    } else {
      const grad = g.createRadialGradient(m * .72, m * .62, 0, m, m, m);
      grad.addColorStop(0, `hsl(${hue},100%,97%)`); grad.addColorStop(.32, `hsl(${hue},92%,70%)`);
      grad.addColorStop(.82, `hsl(${hue},82%,43%)`); grad.addColorStop(1, `hsl(${hue},88%,28%)`);
      g.fillStyle = grad; g.beginPath(); g.arc(m, m, m - 1, 0, TAU); g.fill();
      g.strokeStyle = `hsla(${hue},100%,85%,.7)`; g.lineWidth = 2; g.beginPath(); g.arc(m, m, m - 2, 0, TAU); g.stroke();
    }
    sprites.set(key, c);
    return c;
  }
  const glow = hue => sprite('glow', hue);
  const orb = hue => sprite('orb', hue);
  function createModal() {
    if (modal) return;
    modal = document.createElement('div');
    modal.id = 'graph-modal'; modal.className = 'graph-modal hidden';
    modal.setAttribute('role', 'dialog'); modal.setAttribute('aria-modal', 'true'); modal.setAttribute('aria-labelledby', 'graph-title');
    modal.innerHTML = `<div class="graph-backdrop"></div><div class="graph-dialog">
      <header class="graph-header"><div class="graph-title-row"><span class="graph-title-orb" aria-hidden="true"></span><h3 id="graph-title">${t('graph.title')}</h3><span id="graph-stats-badge" class="graph-stats-badge" aria-live="polite"></span></div>
      <div class="graph-toolbar"><button id="graph-btn-fx" class="graph-tool-btn graph-fx-btn" aria-pressed="${fxOn}" title="${t('graph.effects')}" aria-label="${t('graph.effects')}">✦</button><button id="graph-btn-dimension" class="graph-tool-btn" aria-pressed="true">3D</button><button id="graph-btn-reset" class="graph-tool-btn" title="${t('graph.reset')}" aria-label="${t('graph.reset')}">↺</button><button id="graph-btn-close" class="graph-tool-btn" aria-label="${t('toolbar.close')}">×</button></div></header>
      <div class="graph-workspace"><aside class="graph-sidebar"><label class="ux-search"><span>⌕</span><input id="graph-search" type="search" placeholder="${t('ux.graphSearch')}" aria-label="${t('ux.graphSearch')}"></label>
      <div class="graph-options"><label><input id="graph-labels" type="checkbox" checked> ${t('ux.labels')}</label><label><input id="graph-neighbors" type="checkbox"> ${t('ux.neighbors')}</label></div>
      <div id="graph-node-list" class="graph-node-list" aria-label="${t('ux.notes')}"></div></aside>
      <div class="graph-body"><canvas id="graph-canvas" tabindex="0" aria-label="${t('ux.graphControls')}"></canvas>
      <div id="graph-loading" class="graph-loading hidden" role="status"><span class="graph-loading-orbit" aria-hidden="true"><i></i><i></i><i></i></span><span class="graph-loading-text"></span></div>
      <div id="graph-tooltip" class="graph-tooltip hidden" aria-hidden="true"></div>
      <div class="graph-viewport-tools"><button id="graph-btn-zoom-out" class="graph-tool-btn" aria-label="${t('graph.zoomOut')}">−</button><output id="graph-zoom">100%</output><button id="graph-btn-zoom-in" class="graph-tool-btn" aria-label="${t('graph.zoomIn')}">+</button></div>
      <p class="graph-control-hint">${t('ux.graphControls')} · ${t('graph.hint')}</p>
      <section id="graph-detail" class="graph-detail hidden" aria-live="polite"></section></div></div></div>`;
    document.body.appendChild(modal);
    canvas = modal.querySelector('canvas'); ctx = canvas.getContext('2d');
    tip = modal.querySelector('#graph-tooltip'); zoomOut = modal.querySelector('#graph-zoom');
    modal.querySelector('.graph-backdrop').onclick = close;
    modal.querySelector('#graph-btn-close').onclick = close;
    modal.querySelector('#graph-btn-reset').onclick = reset;
    modal.querySelector('#graph-btn-zoom-in').onclick = () => setZoom(goal.zoom * 1.25);
    modal.querySelector('#graph-btn-zoom-out').onclick = () => setZoom(goal.zoom / 1.25);
    modal.querySelector('#graph-btn-fx').onclick = e => {
      fxOn = !fxOn; e.currentTarget.setAttribute('aria-pressed', String(fxOn));
      try { localStorage.setItem('readmd-graph-fx', fxOn ? '1' : '0'); } catch (_) { /* ignore */ }
      if (!fxOn) settleLayout(); requestDraw();
    };
    modal.querySelector('#graph-btn-dimension').onclick = e => {
      is3d = !is3d; e.currentTarget.textContent = is3d ? '3D' : '2D';
      e.currentTarget.setAttribute('aria-pressed', String(is3d)); interact(); syncData(); requestDraw();
    };
    const search = modal.querySelector('#graph-search');
    search.oninput = e => { query = e.target.value.trim().toLowerCase(); renderList(); requestDraw(); };
    search.onkeydown = e => { if (e.key === 'Enter') { const first = nodes.find(matches); if (first) { e.preventDefault(); select(first, true); } } };
    modal.querySelector('#graph-labels').onchange = e => { labels = e.target.checked; requestDraw(); };
    modal.querySelector('#graph-neighbors').onchange = e => { onlyNeighbors = e.target.checked; renderList(); requestDraw(); };
    modal.addEventListener('keydown', e => {
      if (e.key === 'Escape') { e.stopPropagation(); if (selected) select(null); else close(); }
      if (e.key === 'Tab') trapFocus(e, modal);
    });
    new ResizeObserver(() => { if (visible()) resize(); }).observe(modal.querySelector('.graph-body'));
    canvas.addEventListener('wheel', e => {
      e.preventDefault(); const r = canvas.getBoundingClientRect();
      setZoom(goal.zoom * Math.exp(-e.deltaY * .0015), e.clientX - r.left, e.clientY - r.top);
    }, { passive: false });
    canvas.oncontextmenu = e => e.preventDefault();
    canvas.onpointerdown = onPointerDown;
    canvas.onpointermove = onPointerMove;
    canvas.onpointerup = onPointerUp;
    canvas.onpointercancel = e => { pointers.delete(e.pointerId); releaseDrag(); gesture = null; };
    canvas.onpointerleave = () => { if (!gesture) { hovered = null; showTip(null); requestDraw(); } };
    canvas.ondblclick = e => {
      const r = canvas.getBoundingClientRect(), p = hit(e.clientX - r.left, e.clientY - r.top);
      if (p?.node.path) { close(); window.loadFile?.(p.node.path); }
    };
    canvas.onkeydown = e => {
      if (e.key === '+' || e.key === '=') setZoom(goal.zoom * 1.25);
      else if (e.key === '-') setZoom(goal.zoom / 1.25);
      else if (e.key === '0') reset();
      else if (e.key === 'ArrowLeft') yaw -= .15;
      else if (e.key === 'ArrowRight') yaw += .15;
      else if (e.key === 'ArrowUp') pitch = clamp(pitch - .1, -1.4, 1.4);
      else if (e.key === 'ArrowDown') pitch = clamp(pitch + .1, -1.4, 1.4);
      else if (e.key === 'Enter' && selected?.path) { close(); window.loadFile?.(selected.path); return; }
      else return;
      e.preventDefault(); vYaw = vPitch = 0; interact(); syncData(); requestDraw();
    };
  }
  function trapFocus(e, parent) {
    const items = [...parent.querySelectorAll('button,input,select,textarea,[tabindex="0"]')].filter(el => !el.disabled && el.getClientRects().length);
    const first = items[0], last = items.at(-1);
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
  }
  const interact = () => { lastInteract = performance.now(); };

  function onPointerDown(e) {
    if (e.button > 1) return;
    canvas.setPointerCapture(e.pointerId); canvas.focus({ preventScroll: true });
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const r = canvas.getBoundingClientRect(), p = pointers.size === 1 ? hit(e.clientX - r.left, e.clientY - r.top) : null;
    gesture = { x: e.clientX, y: e.clientY, moved: false, pan: e.shiftKey || e.button === 1 || !is3d, node: p?.node || null };
    vYaw = vPitch = 0; interact();
  }
  function onPointerMove(e) {
    const old = pointers.get(e.pointerId), r = canvas.getBoundingClientRect();
    if (old && gesture) {
      const dx = e.clientX - old.x, dy = e.clientY - old.y;
      if (Math.hypot(e.clientX - gesture.x, e.clientY - gesture.y) > 4) gesture.moved = true;
      const other = [...pointers.entries()].find(([id]) => id !== e.pointerId)?.[1];
      if (other) {
        const before = Math.hypot(old.x - other.x, old.y - other.y);
        if (before > 1) setZoom(goal.zoom * Math.hypot(e.clientX - other.x, e.clientY - other.y) / before);
        gesture.moved = true; gesture.node = null;
      } else if (gesture.node && gesture.moved) dragNode(gesture.node, dx, dy);
      else if (gesture.node) { /* wait for the drag threshold */ }
      else if (gesture.pan) { goal.panX += dx; goal.panY += dy; cam.panX += dx; cam.panY += dy; followNode = null; }
      else { vYaw = dx * .007; vPitch = dy * .007; yaw += vYaw; pitch = clamp(pitch + vPitch, -1.4, 1.4); }
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY }); interact(); syncData(); requestDraw();
    } else {
      const p = hit(e.clientX - r.left, e.clientY - r.top), next = p?.node || null;
      if (next !== hovered) { hovered = next; requestDraw(); }
      canvas.style.cursor = hovered ? 'pointer' : 'grab';
      showTip(p, e.clientX - r.left, e.clientY - r.top);
    }
  }
  function onPointerUp(e) {
    const r = canvas.getBoundingClientRect();
    if (gesture && !gesture.moved) {
      const p = hit(e.clientX - r.left, e.clientY - r.top);
      if (p) shock(p.x, p.y, p.node.hue);
      select(p?.node || null, !!p);
    }
    pointers.delete(e.pointerId);
    if (!pointers.size) { releaseDrag(); gesture = null; }
    requestDraw();
  }
  // Move a node in the camera plane: invert the view rotation so the drag follows the cursor.
  function dragNode(n, dx, dy) {
    const p = projMap.get(n.key); if (!p) return;
    const ey = yaw * morph, ep = pitch * morph, sx = dx / p.scale, sy = dy / p.scale;
    const y = sy * Math.cos(ep), rz = -sy * Math.sin(ep);
    n.x += sx * Math.cos(ey) + rz * Math.sin(ey); n.z += -sx * Math.sin(ey) + rz * Math.cos(ey); n.y += y;
    n.fixed = true; n.vx = n.vy = n.vz = 0; energy = Math.max(energy, .35); followNode = null;
    canvas.style.cursor = 'grabbing';
  }
  function releaseDrag() { if (gesture?.node) { gesture.node.fixed = false; energy = Math.max(energy, .25); } }
  function shock(x, y, hue) { if (fx()) shocks.push({ x, y, hue, t0: performance.now() }); }
  function showTip(p, x, y) {
    if (!tip) return;
    if (!p || gesture) { tip.classList.add('hidden'); return; }
    const n = p.node;
    tip.innerHTML = `<strong>${esc(n.label)}</strong><span>${esc(n.path ? basename(n.path) : t('graph.deadlinkUncreated'))}</span><small>${t('ux.connections', { count: n.neighbors.size })}</small>`;
    tip.style.setProperty('--tip-h', n.hue);
    tip.classList.remove('hidden');
    const w = canvas.clientWidth, tw = tip.offsetWidth, th = tip.offsetHeight;
    tip.style.left = `${clamp(x + 16, 8, w - tw - 8)}px`;
    tip.style.top = `${y - th - 14 < 8 ? y + 18 : y - th - 14}px`;
  }
  function matches(n) { return (!query || `${n.label} ${n.path || ''}`.toLowerCase().includes(query)) && (!onlyNeighbors || !selected || n === selected || selected.neighbors.has(n.key)); }
  function renderList() {
    const list = modal.querySelector('#graph-node-list');
    const filtered = nodes.filter(matches).sort((a, b) => b.neighbors.size - a.neighbors.size || a.label.localeCompare(b.label));
    list.innerHTML = filtered.length ? filtered.map((n, i) => `<button class="graph-node-row ${n === selected ? 'selected' : ''} ${animateList ? 'enter' : ''}" style="--h:${n.hue};--i:${Math.min(i, 24)}" data-node="${esc(n.key)}" aria-pressed="${n === selected}"><span class="graph-node-dot ${n.is_deadlink ? 'dead' : ''}"></span><span>${esc(n.label)}</span><small>${n.neighbors.size}</small></button>`).join('') : `<p class="ux-empty">${t('ux.noResults')}</p>`;
    animateList = false;
    list.querySelectorAll('button').forEach(el => {
      el.onclick = () => select(byKey.get(el.dataset.node), true);
      el.onmouseenter = () => { hovered = byKey.get(el.dataset.node) || null; requestDraw(); };
      el.onmouseleave = () => { hovered = null; requestDraw(); };
    });
  }
  // Breadth-first hop distances drive the ripple that runs outward from a selected note.
  function computeHops(start) {
    hops = new Map(); if (!start) return;
    hops.set(start.key, 0); let frontier = [start];
    for (let h = 1; h <= 3 && frontier.length; h++) {
      const next = [];
      frontier.forEach(n => n.neighbors.forEach(k => { if (!hops.has(k)) { hops.set(k, h); next.push(byKey.get(k)); } }));
      frontier = next;
    }
  }
  function select(n, fly) {
    selected = n || null; computeHops(selected); waveStart = performance.now(); interact();
    renderList(); requestDraw();
    const row = modal.querySelector('.graph-node-row.selected'); row?.scrollIntoView?.({ block: 'nearest' });
    const detail = modal.querySelector('#graph-detail'); detail.classList.toggle('hidden', !n);
    if (!n) { followNode = null; return; }
    if (fly) { followNode = n; goal.zoom = Math.max(goal.zoom, Math.min(2.2, fitZoom() * 1.6)); goal.panX = goal.panY = 0; }
    const near = [...n.neighbors].map(k => byKey.get(k)).filter(Boolean).sort((a, b) => b.neighbors.size - a.neighbors.size).slice(0, 6);
    detail.style.setProperty('--h', n.hue);
    detail.innerHTML = `<span class="graph-detail-orb" aria-hidden="true"></span><div><strong>${esc(n.label)}</strong><p>${esc(n.path || t('graph.deadlinkUncreated'))}</p><small>${t('ux.connections', { count: n.neighbors.size })}</small>${near.length ? `<div class="graph-detail-chips">${near.map(m => `<button class="graph-chip" style="--h:${m.hue}" data-node="${esc(m.key)}">${esc(m.label)}</button>`).join('')}</div>` : ''}</div><button class="tb-btn accent graph-open-btn" ${!n.path ? 'disabled' : ''}>${t('ux.openNote')}</button>`;
    detail.querySelector('.graph-open-btn').onclick = () => { close(); window.loadFile?.(n.path); };
    detail.querySelectorAll('.graph-chip').forEach(chip => { chip.onclick = () => select(byKey.get(chip.dataset.node), true); });
  }
  // Label propagation gives each linked neighbourhood its own colour family.
  function communities() {
    const label = new Map(nodes.map((n, i) => [n.key, i]));
    for (let pass = 0; pass < 8; pass++) {
      let changed = false;
      for (const n of nodes) {
        if (!n.neighbors.size) continue;
        const votes = new Map();
        n.neighbors.forEach(k => { const l = label.get(k); votes.set(l, (votes.get(l) || 0) + 1 + (byKey.get(k).neighbors.size > 3 ? .5 : 0)); });
        let best = label.get(n.key), score = -1;
        votes.forEach((v, l) => { if (v > score || (v === score && l < best)) { best = l; score = v; } });
        if (best !== label.get(n.key)) { label.set(n.key, best); changed = true; }
      }
      if (!changed) break;
    }
    const ids = [...new Set(nodes.map(n => label.get(n.key)))];
    const size = new Map(ids.map(id => [id, 0])); nodes.forEach(n => size.set(label.get(n.key), size.get(label.get(n.key)) + 1));
    ids.sort((a, b) => size.get(b) - size.get(a));
    nodes.forEach(n => { n.group = ids.indexOf(label.get(n.key)); n.hue = n.is_deadlink ? 352 : HUES[n.group % HUES.length]; });
  }
  function init(data) {
    selected = hovered = followNode = null; hops = new Map(); query = ''; shocks = [];
    modal.querySelector('#graph-search').value = ''; modal.querySelector('#graph-detail').classList.add('hidden');
    const raw = (data.nodes || []).slice(0, 500), count = raw.length;
    graphRadius = Math.max(120, Math.sqrt(count) * 30);
    nodes = raw.map((n, i) => {
      const z = 1 - 2 * (i + .5) / Math.max(count, 1), phi = i * 2.399963, ring = Math.sqrt(1 - z * z), r = graphRadius * .08;
      return { ...n, key: String(n.id), label: String(n.label || basename(n.path) || n.id), x: Math.cos(phi) * ring * r, y: Math.sin(phi) * ring * r, z: z * r, vx: 0, vy: 0, vz: 0, neighbors: new Set(), phase: Math.random() * TAU, hover: 0, appear: 0 };
    });
    byKey = new Map(nodes.map(n => [n.key, n]));
    const seen = new Set();
    edges = (data.edges || []).map(e => ({ s: byKey.get(String(e.source)), t: byKey.get(String(e.target)) }))
      .filter(e => e.s && e.t && e.s !== e.t && !seen.has(`${e.s.key}\u0000${e.t.key}`) && seen.add(`${e.s.key}\u0000${e.t.key}`));
    edges.forEach((e, i) => {
      e.s.neighbors.add(e.t.key); e.t.neighbors.add(e.s.key);
      e.bend = (i % 2 ? 1 : -1) * (.1 + (i % 5) * .025); e.phase = Math.random(); e.speed = .16 + Math.random() * .22;
      e.mutual = seen.has(`${e.t.key}\u0000${e.s.key}`);
    });
    nodes.forEach(n => { n.radius = Math.min(16, 5 + Math.sqrt(n.neighbors.size || n.degree || 0) * 2.3); });
    communities();
    const order = [...nodes].sort((a, b) => b.neighbors.size - a.neighbors.size), step = Math.min(26, 1100 / Math.max(count, 1));
    order.forEach((n, i) => { n.delay = 260 + i * step; });
    modal.querySelector('#graph-stats-badge').textContent = t('graph.statsSimple', { nodes: count, links: edges.length });
    alpha = 1; ticks = 0; introStart = performance.now(); animateList = true;
    if (!fx()) { settleLayout(); introStart = -1e9; }
    renderList(); reset(true); syncData(); requestDraw();
  }
  // Force layout with alpha cooling: charge repulsion, curved springs, cluster cohesion, gravity.
  let alpha = 1, bgLayer = null, nebulaLayer = null, vignetteLayer = null;
  function physics() {
    if (energy) { alpha = Math.max(alpha, energy); energy = 0; } // drag requests a reheat
    const count = nodes.length; if (!count) return;
    const k = alpha, flat = !is3d;
    for (let i = 0; i < count; i++) {
      const a = nodes[i];
      for (let j = i + 1; j < count; j++) {
        const b = nodes[j], dx = b.x - a.x, dy = b.y - a.y, dz = b.z - a.z, d2 = dx * dx + dy * dy + dz * dz + 30;
        if (d2 > 640000) continue;
        const f = 2400 * k / (d2 * Math.sqrt(d2));
        a.vx -= dx * f; a.vy -= dy * f; a.vz -= dz * f; b.vx += dx * f; b.vy += dy * f; b.vz += dz * f;
      }
    }
    for (const e of edges) {
      const s = e.s, d = e.t, dx = d.x - s.x, dy = d.y - s.y, dz = d.z - s.z, len = Math.hypot(dx, dy, dz) || 1;
      const rest = 70 + Math.min(60, (s.neighbors.size + d.neighbors.size) * 3), f = (len - rest) * .02 * k / len;
      s.vx += dx * f; s.vy += dy * f; s.vz += dz * f; d.vx -= dx * f; d.vy -= dy * f; d.vz -= dz * f;
    }
    const centers = new Map();
    for (const n of nodes) { const c = centers.get(n.group) || { x: 0, y: 0, z: 0, n: 0 }; c.x += n.x; c.y += n.y; c.z += n.z; c.n++; centers.set(n.group, c); }
    for (const n of nodes) {
      const c = centers.get(n.group);
      if (c.n > 1) { n.vx += (c.x / c.n - n.x) * .006 * k; n.vy += (c.y / c.n - n.y) * .006 * k; n.vz += (c.z / c.n - n.z) * .006 * k; }
      n.vx -= n.x * .004 * k; n.vy -= n.y * .004 * k; n.vz -= n.z * (flat ? .08 : .004 * k);
      if (n.fixed) { n.vx = n.vy = n.vz = 0; continue; }
      n.vx = clamp(n.vx * .78, -14, 14); n.vy = clamp(n.vy * .78, -14, 14); n.vz = clamp(n.vz * .78, -14, 14);
      n.x += n.vx; n.y += n.vy; n.z += n.vz;
    }
    alpha *= .985; ticks++;
  }
  function settleLayout() { for (let i = 0; i < 320 && alpha > .02; i++) physics(); }
  function fitZoom() {
    const w = canvas?.clientWidth || 800, h = canvas?.clientHeight || 600;
    let extent = 0; for (const n of nodes) extent = Math.max(extent, Math.hypot(n.x, n.y, n.z));
    if (extent < graphRadius * .4) extent = graphRadius;
    return clamp(Math.min(w, h) * .42 / Math.max(60, extent), .2, 1.8);
  }
  function reset(intro) {
    yaw = .35; pitch = -.22; vYaw = vPitch = 0; followNode = null;
    Object.assign(goal, { zoom: fitZoom(), panX: 0, panY: 0, tx: 0, ty: 0, tz: 0 });
    if (intro && fx()) { Object.assign(cam, goal, { zoom: goal.zoom * .3 }); yaw = .35 - 1.4; vYaw = .07; }
    else if (!fx()) Object.assign(cam, goal);
    interact(); syncData(); requestDraw();
  }
  function setZoom(value, cx, cy) {
    const next = clamp(value, .12, 5), ratio = next / goal.zoom, w = canvas.clientWidth, h = canvas.clientHeight;
    if (cx != null) { goal.panX = cx - w / 2 - (cx - w / 2 - goal.panX) * ratio; goal.panY = cy - h / 2 - (cy - h / 2 - goal.panY) * ratio; }
    goal.zoom = next; if (!fx()) Object.assign(cam, goal);
    interact(); syncData(); requestDraw();
  }
  function syncData() {
    if (!canvas) return;
    const y = yaw.toFixed(3), d = is3d ? '3' : '2', c = String(nodes.length), z = `${Math.round(goal.zoom * 100)}%`;
    if (canvas.dataset.yaw !== y) canvas.dataset.yaw = y;
    if (canvas.dataset.dimensions !== d) canvas.dataset.dimensions = d;
    if (canvas.dataset.nodeCount !== c) canvas.dataset.nodeCount = c;
    if (zoomOut.textContent !== z) zoomOut.textContent = z;
  }
  function resize() {
    const r = canvas.parentElement.getBoundingClientRect(), d = Math.min(devicePixelRatio || 1, 2);
    canvas.width = Math.round(r.width * d); canvas.height = Math.round(r.height * d);
    canvas.style.width = `${r.width}px`; canvas.style.height = `${r.height}px`;
    bakeLayers(r.width, r.height);
    if (!stars.length) stars = Array.from({ length: 260 }, () => ({ x: Math.random() * 2 - 1, y: Math.random() * 2 - 1, z: Math.random() * .98 + .02, tw: Math.random() * TAU, hue: [210, 260, 190, 40][Math.floor(Math.random() * 4)] }));
    requestDraw();
  }
  // Full-screen gradients are baked once per size at half resolution; each frame only blits them.
  function layer(w, h, paint) {
    const c = document.createElement('canvas'), q = .5;
    c.width = Math.max(1, Math.round(w * q)); c.height = Math.max(1, Math.round(h * q));
    const g = c.getContext('2d'); g.scale(q, q); paint(g, w, h); return c;
  }
  function bakeLayers(w, h) {
    if (w < 2 || h < 2) return;
    bgLayer = layer(w, h, g => {
      const grad = g.createLinearGradient(0, 0, w * .4, h);
      grad.addColorStop(0, 'hsl(232,48%,7%)'); grad.addColorStop(.55, 'hsl(248,44%,9%)'); grad.addColorStop(1, 'hsl(222,52%,6%)');
      g.fillStyle = grad; g.fillRect(0, 0, w, h);
    });
    nebulaLayer = layer(w * 1.2, h * 1.2, (g, W, H) => {
      g.globalCompositeOperation = 'lighter';
      NEBULA.forEach(b => {
        const x = b.x * W, y = b.y * H, r = b.r * Math.max(W, H), grad = g.createRadialGradient(x, y, 0, x, y, r);
        grad.addColorStop(0, `hsla(${b.h},85%,58%,${b.a})`); grad.addColorStop(1, 'hsla(0,0%,0%,0)');
        g.fillStyle = grad; g.fillRect(0, 0, W, H);
      });
    });
    vignetteLayer = layer(w, h, g => {
      const grad = g.createRadialGradient(w / 2, h / 2, Math.min(w, h) * .32, w / 2, h / 2, Math.hypot(w, h) * .62);
      grad.addColorStop(0, 'rgba(0,0,0,0)'); grad.addColorStop(1, 'rgba(0,0,0,.6)'); g.fillStyle = grad; g.fillRect(0, 0, w, h);
    });
  }
  function requestDraw() { if (!raf && visible() && ctx) raf = requestAnimationFrame(tick); }
  function tick(now) {
    raf = 0; if (!visible() || document.hidden) { lastFrame = 0; return; }
    const dt = Math.min(48, lastFrame ? now - lastFrame : 16); lastFrame = now;
    const busy = step(dt, now); draw(now, dt); syncData();
    if (busy || fx()) raf = requestAnimationFrame(tick); else lastFrame = 0;
  }
  function step(dt, now) {
    const motion = fx(), f = dt / 16.7;
    if (alpha > .015 || energy) physics();
    const morphGoal = is3d ? 1 : 0;
    morph = motion ? morph + (morphGoal - morph) * Math.min(1, .12 * f) : morphGoal;
    if (!gesture) {
      yaw += vYaw * f; pitch = clamp(pitch + vPitch * f, -1.4, 1.4);
      const fr = Math.pow(.93, f); vYaw *= fr; vPitch *= fr;
      if (Math.abs(vYaw) < 1e-4) vYaw = 0; if (Math.abs(vPitch) < 1e-4) vPitch = 0;
      if (motion && is3d && !selected && !hovered && now - lastInteract > 3500) yaw += .0011 * f;
    }
    if (followNode) { goal.tx = followNode.x; goal.ty = followNode.y; goal.tz = followNode.z; }
    let moving = false;
    for (const key of Object.keys(goal)) {
      const diff = goal[key] - cam[key];
      if (!motion || Math.abs(diff) < (key === 'zoom' ? 1e-4 : .05)) cam[key] = goal[key];
      else { cam[key] += diff * Math.min(1, .13 * f); moving = true; }
    }
    for (const n of nodes) n.hover += ((n === hovered || n === selected ? 1 : 0) - n.hover) * (motion ? Math.min(1, .2 * f) : 1);
    shocks = shocks.filter(s => now - s.t0 < 900);
    if (motion && Math.random() < dt / 3800) {
      const w = canvas.clientWidth, h = canvas.clientHeight, ang = .5 + Math.random() * .5;
      meteors.push({ x: Math.random() * w * .9, y: -20, vx: Math.cos(ang) * .9 * (Math.random() < .5 ? 1 : -1), vy: Math.sin(ang) * .9, t0: now, life: 900 + Math.random() * 500, h });
    }
    meteors = meteors.filter(m => now - m.t0 < m.life);
    return alpha > .015 || moving || vYaw !== 0 || vPitch !== 0 || Math.abs(morph - morphGoal) > .001 || shocks.length > 0;
  }
  const labelWidths = new Map();
  function labelText(n) { return n.label.length > 24 ? n.label.slice(0, 22) + '…' : n.label; }
  function measure(text, font) {
    const key = font + text; let w = labelWidths.get(key);
    if (w == null) { ctx.font = font; w = ctx.measureText(text).width; labelWidths.set(key, w); }
    return w;
  }
  function bezierPoint(a, c, b, u) { const v = 1 - u; return [v * v * a.x + 2 * v * u * c[0] + u * u * b.x, v * v * a.y + 2 * v * u * c[1] + u * u * b.y]; }
  function drawBackdrop(w, h, now, motion, intro) {
    ctx.globalCompositeOperation = 'source-over'; ctx.globalAlpha = 1;
    if (bgLayer) ctx.drawImage(bgLayer, 0, 0, w, h);
    ctx.globalCompositeOperation = 'lighter';
    if (nebulaLayer) {
      const drift = motion ? now / 9000 : 0;
      const dx = -w * .1 + Math.sin(drift) * w * .04 + clamp(cam.panX * .05, -w * .05, w * .05);
      const dy = -h * .1 + Math.cos(drift * .8) * h * .04 + clamp(cam.panY * .05, -h * .05, h * .05);
      ctx.globalAlpha = motion ? .85 + Math.sin(now / 4000) * .15 : 1; ctx.drawImage(nebulaLayer, dx, dy, w * 1.2, h * 1.2);
    }
    const warp = motion ? clamp(1 - (now - introStart) / 1100, 0, 1) : 0, cx = w / 2, cy = h / 2;
    for (const s of stars) {
      const near = 1 - s.z, px = ((((s.x * .5 + .5) + yaw * .035 * near + cam.panX / w * .25 * near) % 1) + 1) % 1 * w;
      const py = ((((s.y * .5 + .5) + pitch * .035 * near + cam.panY / h * .25 * near) % 1) + 1) % 1 * h;
      const tw = motion ? .62 + .38 * Math.sin(now / 650 + s.tw) : .8, size = near * 1.7 + .35;
      ctx.globalAlpha = (.18 + .65 * near) * tw * (intro < 1 ? .4 + .6 * intro : 1);
      ctx.fillStyle = `hsl(${s.hue},90%,${80 + near * 15}%)`;
      if (warp > 0) {
        const k = warp * warp * .9; ctx.strokeStyle = ctx.fillStyle; ctx.lineWidth = size;
        ctx.beginPath(); ctx.moveTo(cx + (px - cx) * (1 - k), cy + (py - cy) * (1 - k)); ctx.lineTo(px, py); ctx.stroke();
      } else ctx.fillRect(px - size / 2, py - size / 2, size, size);
    }
    for (const m of meteors) {
      const age = (now - m.t0) / m.life, x = m.x + m.vx * (now - m.t0), y = m.y + m.vy * (now - m.t0);
      const g = ctx.createLinearGradient(x, y, x - m.vx * 160, y - m.vy * 160);
      g.addColorStop(0, `hsla(${200 + age * 60},100%,92%,${.85 * (1 - age)})`); g.addColorStop(1, 'hsla(220,100%,70%,0)');
      ctx.globalAlpha = 1; ctx.strokeStyle = g; ctx.lineWidth = 1.6;
      ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x - m.vx * 160, y - m.vy * 160); ctx.stroke();
    }
  }
  function drawOrbits(proj, now, motion, intro) {
    if (morph < .05 || !nodes.length) return;
    const R = graphRadius;
    [[1.05, 0, 205], [1.4, .9, 275], [1.75, -.6, 325]].forEach(([k, tilt, hue], ri) => {
      ctx.globalAlpha = .16 * morph * intro; ctx.strokeStyle = `hsl(${hue},90%,70%)`; ctx.lineWidth = 1;
      ctx.setLineDash([2, 9]); ctx.lineDashOffset = motion ? -now / (60 + ri * 25) : 0; ctx.beginPath();
      for (let i = 0; i <= 72; i++) {
        const a = i / 72 * TAU, x = Math.cos(a) * R * k, z = Math.sin(a) * R * k, y = Math.sin(a) * R * k * Math.sin(tilt) * .35;
        const p = proj(x + cam.tx, y + cam.ty, z * Math.cos(tilt * .3) + cam.tz); i ? ctx.lineTo(p[0], p[1]) : ctx.moveTo(p[0], p[1]);
      }
      ctx.stroke();
      if (motion) {
        const a = now / (5200 + ri * 1900) + ri * 2, p = proj(Math.cos(a) * R * k + cam.tx, Math.sin(a) * R * k * Math.sin(tilt) * .35 + cam.ty, Math.sin(a) * R * k * Math.cos(tilt * .3) + cam.tz);
        ctx.globalAlpha = .7 * morph * intro; const s = 18; ctx.drawImage(glow(hue), p[0] - s / 2, p[1] - s / 2, s, s);
      }
    });
    ctx.setLineDash([]);
  }
  function draw(now) {
    if (!ctx || !visible()) return;
    const w = canvas.clientWidth, h = canvas.clientHeight, dpr = Math.min(devicePixelRatio || 1, 2), motion = fx();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const introT = motion ? clamp((now - introStart) / 700, 0, 1) : 1, intro = easeOutCubic(introT);
    drawBackdrop(w, h, now, motion, intro);
    const ey = yaw * morph, ep = pitch * morph, cy = Math.cos(ey), sy = Math.sin(ey), cp = Math.cos(ep), sp = Math.sin(ep);
    const ox = w / 2 + cam.panX, oy = h / 2 + cam.panY;
    const proj = (x, y, z) => {
      const X = x - cam.tx, Y = y - cam.ty, Z = (z - cam.tz) * morph;
      const rx = X * cy - Z * sy, rz = X * sy + Z * cy, ry = Y * cp - rz * sp, depth = Y * sp + rz * cp;
      const s = 900 / Math.max(260, 900 + depth) * cam.zoom;
      return [ox + rx * s, oy + ry * s, depth, s];
    };
    drawOrbits(proj, now, motion, intro);
    const focus = hovered || selected;
    projected = nodes.map(n => {
      const [x, y, depth, scale] = proj(n.x, n.y, n.z);
      n.appear = motion ? clamp((now - introStart - n.delay) / 650, 0, 1) : 1;
      const grow = motion ? easeOutBack(n.appear) : 1, bob = motion ? Math.sin(now / 1300 + n.phase) * 1.6 : 0;
      return { node: n, x, y: y + bob * n.appear, depth, scale, r: Math.max(2.5, n.radius * scale) * grow * (1 + n.hover * .28), match: matches(n) };
    }).sort((a, b) => b.depth - a.depth);
    projMap = new Map(projected.map(p => [p.node.key, p]));
    const depthMin = projected.length ? projected.at(-1).depth : 0, depthMax = projected.length ? projected[0].depth : 1;
    const fog = d => depthMax - depthMin < 1 ? 1 : .45 + .55 * (1 - (d - depthMin) / (depthMax - depthMin));
    const waveAge = now - waveStart, lit = (n) => !focus || n === focus || focus.neighbors.has(n.key);
    const heavy = edges.length > 700, solid = edges.length > 140;
    ctx.globalCompositeOperation = 'lighter'; ctx.lineCap = 'round';
    for (const e of edges) {
      const a = projMap.get(e.s.key), b = projMap.get(e.t.key); if (!a || !b) continue;
      const appear = Math.min(e.s.appear, e.t.appear); if (appear <= 0) continue;
      const hot = focus && (e.s === focus || e.t === focus);
      const hs = hops.get(e.s.key), ht = hops.get(e.t.key), waveHop = selected && hs != null && ht != null ? Math.max(hs, ht) : -1;
      const waveOn = motion && waveHop > 0 && waveAge > (waveHop - 1) * WAVE && waveAge < waveHop * WAVE + 900;
      let alpha = a.match && b.match ? (focus ? (hot ? .9 : .05) : .3) : .03;
      alpha *= appear * Math.min(fog(a.depth), fog(b.depth)) * intro;
      if (waveOn && !hot) alpha = Math.max(alpha, .5 * (1 - Math.max(0, waveAge - waveHop * WAVE) / 900));
      const dx = b.x - a.x, dy = b.y - a.y, len = Math.hypot(dx, dy) || 1;
      const c = [(a.x + b.x) / 2 - dy * e.bend, (a.y + b.y) / 2 + dx * e.bend];
      ctx.globalAlpha = alpha;
      if (e.t.is_deadlink) { ctx.strokeStyle = 'hsl(352,90%,66%)'; ctx.setLineDash([3, 5]); }
      else if (!solid || hot) {
        const g = ctx.createLinearGradient(a.x, a.y, b.x, b.y);
        g.addColorStop(0, `hsl(${e.s.hue},95%,${hot ? 72 : 62}%)`); g.addColorStop(1, `hsl(${e.t.hue},95%,${hot ? 72 : 62}%)`);
        ctx.strokeStyle = g; ctx.setLineDash([]);
      } else { ctx.strokeStyle = `hsl(${e.s.hue},80%,60%)`; ctx.setLineDash([]); }
      ctx.lineWidth = (hot ? 2.2 : 1) * Math.max(.6, Math.min(1.6, (a.scale + b.scale) / 2 / Math.max(cam.zoom, .01)));
      ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.quadraticCurveTo(c[0], c[1], b.x, b.y); ctx.stroke();
      if (hot && motion) { ctx.globalAlpha = alpha * .35; ctx.lineWidth *= 3.5; ctx.stroke(); }
      // Comets ride the curve from source to target; mutual links get a returning comet too.
      if (motion && alpha > .02 && len > 12 && (!heavy || hot)) {
        const lanes = hot ? 3 : 1, size = hot ? 14 : 9;
        for (let l = 0; l < lanes; l++) {
          const u = ((now / 1000) * e.speed * (hot ? 1.8 : 1) + e.phase + l / lanes) % 1;
          for (let k = 0; k < 4; k++) {
            const uu = u - k * .022; if (uu < 0) break;
            const [px, py] = bezierPoint(a, c, b, uu);
            ctx.globalAlpha = Math.min(1, alpha * 2.4) * (1 - k / 4);
            const s = size * (1 - k * .18); ctx.drawImage(glow(e.s.hue), px - s / 2, py - s / 2, s, s);
          }
          if (e.mutual) {
            const [qx, qy] = bezierPoint(a, c, b, 1 - u);
            ctx.globalAlpha = Math.min(1, alpha * 1.8); ctx.drawImage(glow(e.t.hue), qx - size / 2.6, qy - size / 2.6, size / 1.3, size / 1.3);
          }
        }
      }
    }
    ctx.setLineDash([]);
    for (const p of projected) {
      const n = p.node; if (n.appear <= 0) continue;
      const fade = (p.match ? (lit(n) ? 1 : .16) : .08) * fog(p.depth) * Math.min(1, n.appear * 1.6);
      const hop = hops.get(n.key), waveHit = motion && selected && hop != null && hop > 0 && waveAge > hop * WAVE && waveAge < hop * WAVE + 700;
      const pulse = motion ? 1 + Math.sin(now / 900 + n.phase) * .08 : 1, current = isCurrent(n);
      const halo = p.r * (3.2 + n.hover * 1.6 + (waveHit ? 1.5 * (1 - (waveAge - hop * WAVE) / 700) : 0)) * pulse;
      ctx.globalCompositeOperation = 'lighter';
      ctx.globalAlpha = fade * (.55 + n.hover * .45); ctx.drawImage(glow(n.hue), p.x - halo, p.y - halo, halo * 2, halo * 2);
      if (current) {
        for (let k = 0; k < 3; k++) {
          const ph = motion ? ((now / 2200) + k / 3) % 1 : .35 + k * .2, rr = p.r * (1.4 + ph * 4.5);
          ctx.globalAlpha = (1 - ph) * .7 * fade; ctx.strokeStyle = `hsl(${n.hue},100%,78%)`; ctx.lineWidth = 1.5;
          ctx.beginPath(); ctx.arc(p.x, p.y, rr, 0, TAU); ctx.stroke();
        }
      }
      if (motion && query && p.match) {
        const ph = (now / 1100) % 1; ctx.globalAlpha = (1 - ph) * .8; ctx.strokeStyle = 'hsl(48,100%,70%)'; ctx.lineWidth = 1.4;
        ctx.beginPath(); ctx.arc(p.x, p.y, p.r * (1.3 + ph * 2.2), 0, TAU); ctx.stroke();
      }
      ctx.globalCompositeOperation = 'source-over';
      ctx.globalAlpha = Math.min(1, fade * 1.15);
      ctx.drawImage(orb(n.hue), p.x - p.r, p.y - p.r, p.r * 2, p.r * 2);
      if (n.is_deadlink) {
        ctx.strokeStyle = 'hsl(352,100%,80%)'; ctx.lineWidth = 1.2; ctx.setLineDash([2, 3]);
        ctx.beginPath(); ctx.arc(p.x, p.y, p.r + 3, 0, TAU); ctx.stroke(); ctx.setLineDash([]);
      }
      if (n === selected) {
        const rot = motion ? now / 900 : 0; ctx.globalAlpha = 1; ctx.strokeStyle = `hsl(${n.hue},100%,82%)`; ctx.lineWidth = 2;
        ctx.setLineDash([p.r * .9, p.r * .55]); ctx.lineDashOffset = -rot * p.r;
        ctx.beginPath(); ctx.arc(p.x, p.y, p.r + 7, 0, TAU); ctx.stroke(); ctx.setLineDash([]); ctx.lineDashOffset = 0;
        for (let k = 0; k < 3; k++) {
          const a = rot * 1.6 + k * TAU / 3, sx = p.x + Math.cos(a) * (p.r + 13), syy = p.y + Math.sin(a) * (p.r + 13);
          ctx.globalCompositeOperation = 'lighter'; ctx.drawImage(glow(HUES[(n.group + k + 1) % HUES.length]), sx - 7, syy - 7, 14, 14); ctx.globalCompositeOperation = 'source-over';
        }
      }
    }
    // Labels: nearest and most important first, skipping any that would collide.
    const taken = [];
    const wantLabel = p => p.match && p.node.appear > .6 && (p.node === hovered || p.node === selected || isCurrent(p.node) || (focus && focus.neighbors.has(p.node.key)) || (query && p.match) || (labels && (nodes.length < 60 || p.node.neighbors.size > 2 || p.scale / Math.max(cam.zoom, .01) > 1.05)));
    const order = projected.filter(wantLabel).sort((a, b) => (b.node.hover - a.node.hover) || (b.r - a.r));
    ctx.textAlign = 'center'; ctx.textBaseline = 'top';
    for (const p of order.slice(0, 140)) {
      const n = p.node, strong = n === hovered || n === selected || isCurrent(n), text = labelText(n);
      const font = `${strong ? 600 : 500} ${strong ? 13 : 11.5}px ${FONT}`, tw = measure(text, font) + 12, th = strong ? 20 : 17;
      const x = p.x - tw / 2, y = p.y + p.r + 6;
      if (!strong && taken.some(b => x < b[0] + b[2] && x + tw > b[0] && y < b[1] + b[3] && y + th > b[1])) continue;
      taken.push([x, y, tw, th]);
      ctx.globalAlpha = (strong ? 1 : .85) * fog(p.depth) * (lit(n) ? 1 : .35) * clamp((n.appear - .6) * 2.5, 0, 1);
      ctx.fillStyle = strong ? `hsla(${n.hue},70%,14%,.86)` : 'rgba(8,10,24,.55)';
      ctx.beginPath(); ctx.roundRect ? ctx.roundRect(x, y, tw, th, th / 2) : ctx.rect(x, y, tw, th); ctx.fill();
      if (strong) { ctx.strokeStyle = `hsla(${n.hue},100%,75%,.55)`; ctx.lineWidth = 1; ctx.stroke(); }
      ctx.font = font; ctx.fillStyle = strong ? '#fff' : `hsl(${n.hue},60%,90%)`; ctx.fillText(text, p.x, y + (strong ? 3.5 : 2.5));
    }
    ctx.textBaseline = 'alphabetic';
    ctx.globalCompositeOperation = 'lighter';
    for (const s of shocks) {
      const k = (now - s.t0) / 900, e = easeOutCubic(k);
      ctx.globalAlpha = (1 - k) * .9; ctx.strokeStyle = `hsl(${s.hue},100%,75%)`; ctx.lineWidth = 2.5 * (1 - k) + .5;
      ctx.beginPath(); ctx.arc(s.x, s.y, 8 + e * 120, 0, TAU); ctx.stroke();
      ctx.globalAlpha = (1 - k) * .5; ctx.beginPath(); ctx.arc(s.x, s.y, 4 + e * 70, 0, TAU); ctx.stroke();
    }
    ctx.globalCompositeOperation = 'source-over'; ctx.globalAlpha = 1;
    if (vignetteLayer) ctx.drawImage(vignetteLayer, 0, 0, w, h);
    if (motion && introT < 1) { ctx.globalAlpha = 1 - intro; ctx.fillStyle = 'hsl(232,48%,5%)'; ctx.fillRect(0, 0, w, h); ctx.globalAlpha = 1; }
  }
  function hit(x, y) { return [...projected].reverse().find(p => p.match && p.node.appear > .3 && Math.hypot(x - p.x, y - p.y) < p.r + 8); }

  // The link index is only filled on demand; build it for the folder before asking for the graph.
  async function fetchGraph(directory) {
    const dir = directory || '';
    const bridge = window.pywebview?.api;
    const load = () => bridge?.get_links_graph ? bridge.get_links_graph(dir) : api(`/api/links/graph?dir=${encodeURIComponent(dir)}&max_nodes=500`);
    if (dir) {
      try {
        if (bridge?.index_directory_links) await bridge.index_directory_links(dir, false);
        else await call('/api/links/index', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ dir, force: false }) });
      } catch (_) { /* fall back to whatever is already indexed */ }
    }
    return load();
  }
  // A document that is not on disk (clipboard, web, new) still gets a graph of its own [[links]].
  function virtualGraph() {
    const text = typeof state !== 'undefined' ? (state.fixed || state.original || '') : '';
    const re = new RegExp(WIKILINK_RE.source, 'g'), seen = new Map();
    let m; while ((m = re.exec(text))) { const name = m[1].trim(); if (name && !seen.has(name.toLowerCase())) seen.set(name.toLowerCase(), name); }
    const title = (typeof state !== 'undefined' && (state.sourceName || basename(state.file))) || t('graph.title');
    const nodesV = [{ id: '__current__', path: (typeof state !== 'undefined' && state.file) || '', label: String(title).replace(/\.(md|markdown|txt)$/i, ''), degree: seen.size }];
    seen.forEach((name, key) => nodesV.push({ id: 'v:' + key, path: '', label: name, is_deadlink: true, degree: 1 }));
    return { nodes: nodesV, edges: [...seen.keys()].map(key => ({ source: '__current__', target: 'v:' + key })) };
  }
  async function open(directory) {
    if (!activeDoc()) { if (typeof showToast === 'function') showToast(t('toast.openDocumentToUse')); return; }
    createModal(); opener = document.activeElement; currentFile = (typeof state !== 'undefined' && state.file) || currentFile;
    modal.classList.remove('hidden'); resize();
    modal.querySelector('#graph-search').focus();
    const id = ++requestId, loading = modal.querySelector('#graph-loading'), text = loading.querySelector('.graph-loading-text');
    loading.classList.remove('hidden'); text.textContent = directory ? t('graph.indexing') : t('graph.loading');
    nodes = []; edges = []; projected = []; introStart = performance.now(); requestDraw();
    try {
      let graph;
      if (directory) {
        const res = await fetchGraph(directory);
        if (id !== requestId || !visible()) return;
        if (!res?.ok || !res.graph) throw new Error('graph_unavailable');
        graph = res.graph;
      }
      if (!graph?.nodes?.length) graph = virtualGraph();
      init(graph);
      loading.classList.toggle('hidden', nodes.length > 1 || edges.length > 0); text.textContent = t('graph.noLinks');
    } catch (_) {
      if (id === requestId) { nodes = []; edges = []; renderList(); requestDraw(); text.textContent = t('ux.loadFailed'); }
    }
  }
  function close() {
    if (!modal) return; ++requestId;
    modal.classList.add('hidden'); cancelAnimationFrame(raf); raf = 0; lastFrame = 0;
    pointers.clear(); gesture = null; tip?.classList.add('hidden');
    opener?.isConnected && opener.focus({ preventScroll: true });
  }

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
  function toggleDrawer(){if(!activeDoc())return;createDrawer();drawer.classList.toggle('hidden');if(!drawer.classList.contains('hidden')){refreshBacklinks(currentFile||state.file);drawer.querySelector('input').focus();}}
  const WIKILINK_RE=/\[\[([^\]\n|#]+)(?:#([^\]\n|]+))?(?:\|([^\]\n]+))?\]\]/;
  function hasGraph(content){if(typeof content==='string')return WIKILINK_RE.test(content);return activeDoc()&&WIKILINK_RE.test(state.fixed||state.original||'');}
  function updateVisibility(force){const btn=document.getElementById('btn-graph');if(btn){btn.disabled=!activeDoc();btn.classList.toggle('hidden',!activeDoc()||!(force??hasGraph()));}}
  document.addEventListener('visibilitychange',()=>{if(visible()&&!document.hidden)requestDraw();});
  window.ReadMDGraph={open,close,refreshBacklinks,toggleDrawer,updateVisibility,hasGraph};
})();
