/** ReadMD companions: Rust owns input/drag; each character keeps its authored art.
 * Only BongoCat uses a keyboard/mouse scene (see third_party/bongocat). */
(function () {
  'use strict';
  const WIDTH = 320, HEIGHT = 380;
  const PROFILES = {
    mochi: { name: 'Mochi', sprite: './assets/mochi-sprite.png', blink: true },
    hermes: { name: 'Hermes', sprite: './assets/hermes-sprite.png', spell: true },
    amber: { name: 'Amber', sprite: './assets/amber-sprite.png', blink: true },
    moss: { name: 'Moss', sprite: './assets/moss-sprite.png', blink: true },
    'cache-capy': { name: 'Cache Capy', sprite: './assets/cache-capy-sprite.webp', directions: true, calm: true },
    'niu-lai': { name: '牛来', sprite: './assets/niu-lai-sprite.webp', directions: true },
    'arch-chan': { name: 'Arch-Chan (Live2D)' }
  };
  const ROWS = ['idle','running-right','running-left','waving','jumping','failed','waiting','running','review'];
  const api = window.hermesDesktop?.petOverlay;
  const canvas = document.getElementById('bongocat-canvas'), ctx = canvas.getContext('2d');
  const classicRoot = document.getElementById('bongo-classic-stage'), liveRoot = document.getElementById('live2d-stage');
  const urlLive = new URLSearchParams(location.search).get('renderer') === 'live2d';
  const saved = localStorage.getItem('readmd-pet-character');
  let classicEnabled = !urlLive && localStorage.getItem('readmd-pet-bongo-classic') === 'true';
  const state = {
    character: urlLive ? 'arch-chan' : (PROFILES[saved] ? saved : 'mochi'),
    leftDown: false, rightDown: false, mouseDown: false, mouseButtons: 0,
    pressedKeys: new Set(), lastKey: null, leftTapAt: -Infinity, rightTapAt: -Infinity,
    pointerX: 0, pointerY: 0, tapCounts: { left: 0, right: 0 },
    pettingLevel: 0, hearts: [], dragging: false, mood: 'normal', activity: {}, petInfo: {}
  };
  let scale = 1, offsetX = 0, offsetY = 0;
  let classicController, classicPromise, liveController, livePromise;
  let spritePose = null, spriteRegions = { rects: [] }, geometryKey = '';
  let reaction = null, pointerMovedAt = -Infinity, busyStartedAt = 0;
  let lastInput, lastSnapshotCharacter, lastSheet, lastSheetCharacter;
  let lastFrameAt = performance.now(), sceneActive = true, hostBounds;
  let resolveFirstPaint;
  window.readmdSpriteReady = new Promise(resolve => { resolveFirstPaint = resolve; });
  let soundEnabled = localStorage.getItem('readmd-pet-sound') !== 'false', audio;
  const images = {}, characterInfo = {}, geometry = new WeakMap();
  for (const [id, profile] of Object.entries(PROFILES)) {
    if (profile.sprite) { const img = new Image(); img.src = profile.sprite; images[id] = img; }
  }
  function setupDpi() {
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    canvas.width = Math.round(innerWidth * dpr); canvas.height = Math.round(innerHeight * dpr);
    scale = Math.min(innerWidth / WIDTH, innerHeight / HEIGHT);
    offsetX = (innerWidth - WIDTH * scale) / 2; offsetY = innerHeight - HEIGHT * scale; geometryKey = '';
  }
  setupDpi(); window.addEventListener('resize', setupDpi);
  function sound(tap) {
    if (!soundEnabled || (tap && !classicEnabled)) return;
    try {
      audio ||= new (window.AudioContext || window.webkitAudioContext)();
      if (audio.state === 'suspended') audio.resume();
      const oscillator = audio.createOscillator(), gain = audio.createGain(), now = audio.currentTime;
      oscillator.connect(gain); gain.connect(audio.destination); oscillator.type = tap ? 'triangle' : 'sine';
      oscillator.frequency.setValueAtTime(tap ? 880 : 440, now);
      oscillator.frequency.exponentialRampToValueAtTime(tap ? 220 : 560, now + (tap ? .04 : .2));
      gain.gain.setValueAtTime(tap ? .10 : .08, now);
      gain.gain.exponentialRampToValueAtTime(.001, now + (tap ? .045 : .22));
      oscillator.start(now); oscillator.stop(now + (tap ? .045 : .22));
    } catch (_) { /* Audio is optional until the user interacts. */ }
  }
  function ensureClassic() {
    if (!classicPromise) classicPromise = window.readmdMountBongoClassic(classicRoot, () => state).then(controller => {
      classicController = controller; syncInteractionRegions(); return controller;
    });
    return classicPromise;
  }
  function ensureLive() {
    if (!livePromise) livePromise = window.readmdMountLive2d(liveRoot).then(controller => {
      liveController = controller; controller.setMood(state.mood); syncInteractionRegions(); return controller;
    });
    return livePromise;
  }
  function currentRegions() {
    if (classicEnabled) return classicController?.interactionRegions() || { rects: [] };
    if (state.character === 'arch-chan') return liveController?.interactionRegions() || { rects: [] };
    return spriteRegions;
  }
  function headInCanvas() {
    const h = currentRegions().head;
    return h ? { x: (h.x + h.width / 2 - offsetX) / scale, y: (h.y + h.height / 2 - offsetY) / scale } : { x: WIDTH / 2, y: 180 };
  }
  function respond(kind = 'pet') {
    const now = performance.now(); reaction = { kind, start: now, until: now + (kind === 'complete' ? 1300 : 1100) };
    if (state.mood !== 'normal' && !state.petInfo.companion?.resting) state.mood = 'normal';
    if (!classicEnabled && state.character === 'arch-chan') liveController?.celebrate();
  }
  function petPet() {
    respond(); sound(false); state.pettingLevel = 45;
    const h = headInCanvas();
    for (let i = 0; i < 4; i++) state.hearts.push({ x: h.x + Math.random() * 36 - 18,
      y: h.y - 24, vx: (Math.random() - .5) * 24, vy: -35 - Math.random() * 25, age: 0 });
    window.dispatchEvent(new Event('readmd-pet-interacted'));
  }
  window.readmdLive2dLife = {
    setMood(value) { state.mood = value; if (!classicEnabled && state.character === 'arch-chan') liveController?.setMood(value); },
    setTalking(value) { if (!classicEnabled && state.character === 'arch-chan') liveController?.setTalking(value); },
    celebrate: () => respond('complete'),
    getCharacterTop() { const r = currentRegions(); return r.rects.length ? Math.min(...r.rects.map(rect => rect.y)) : undefined; }
  };
  async function applyCharacter(id, preserveClassic = false) {
    if (!id) return;
    if (id === 'bongocat') return useClassic();
    if (!preserveClassic) { classicEnabled = false; localStorage.setItem('readmd-pet-bongo-classic', 'false'); }
    state.character = id; localStorage.setItem('readmd-pet-character', id);
    state.petInfo = characterInfo[id] || {};
    state.hearts.length = 0; state.pettingLevel = 0; reaction = null; spritePose = null; spriteRegions = { rects: [] };
    geometryKey = ''; pointerMovedAt = -Infinity; classicController?.setActive(false);
    liveRoot.style.display = !classicEnabled && id === 'arch-chan' ? 'block' : 'none';
    liveController?.setPresentation({ active: !classicEnabled && id === 'arch-chan' });
    if (!classicEnabled && id === 'arch-chan') await ensureLive();
    updateMenu(); syncInteractionRegions(); window.dispatchEvent(new Event('readmd-pet-character-changed'));
  }
  function useClassic() {
    classicEnabled = true; localStorage.setItem('readmd-pet-bongo-classic', 'true');
    state.hearts.length = 0; spritePose = null; liveRoot.style.display = 'none'; liveController?.setPresentation({ active: false });
    updateMenu(); return ensureClassic();
  }
  // Inspect alpha geometry once. The union keeps every complete authored action
  // frame inside the viewport; transparent sheet padding does not shrink the pet.
  function sheetGeometry(img, fw, fh) {
    const cached = geometry.get(img); if (cached?.fw === fw && cached?.fh === fh) return cached;
    const columns = Math.max(1, Math.floor(img.naturalWidth / fw)), rows = Math.max(1, Math.floor(img.naturalHeight / fh));
    const sample = document.createElement('canvas'); sample.width = img.naturalWidth; sample.height = img.naturalHeight;
    const c = sample.getContext('2d', { willReadFrequently: true }); c.drawImage(img, 0, 0);
    const data = c.getImageData(0, 0, sample.width, sample.height).data;
    const frames = Array.from({ length: rows * columns }, () => ({ left: fw, top: fh, right: 0, bottom: 0 }));
    const union = { left: fw, top: fh, right: 0, bottom: 0 };
    for (let y = 0; y < rows * fh; y++) for (let x = 0; x < columns * fw; x++) {
      if (data[(y * sample.width + x) * 4 + 3] <= 32) continue;
      const px = x % fw, py = y % fh, b = frames[Math.floor(y / fh) * columns + Math.floor(x / fw)];
      b.left = Math.min(b.left, px); b.top = Math.min(b.top, py); b.right = Math.max(b.right, px + 1); b.bottom = Math.max(b.bottom, py + 1);
      union.left = Math.min(union.left, px); union.top = Math.min(union.top, py); union.right = Math.max(union.right, px + 1); union.bottom = Math.max(union.bottom, py + 1);
    }
    const counts = Array.from({ length: rows }, (_, row) => {
      let n = columns; while (n > 1 && frames[row * columns + n - 1].right === 0) n--; return n;
    });
    const result = { fw, fh, columns, rows, frames, union, counts }; geometry.set(img, result); return result;
  }
  function rowFor(names, candidates, rows) {
    for (const name of candidates) { const i = names.indexOf(name); if (i >= 0 && i < rows) return i; } return -1;
  }
  function drawSprite(now) {
    const img = images[state.character]; if (!img?.complete || !img.naturalWidth) return;
    const profile = PROFILES[state.character] || {}, info = state.petInfo;
    const v2 = img.naturalWidth === 1536 && img.naturalHeight === 2288, duo = img.naturalWidth === 1536 && img.naturalHeight === 1024;
    let fw = Number(info.frameW) || (v2 ? 192 : duo ? 384 : img.naturalWidth);
    let fh = Number(info.frameH) || (v2 ? 208 : duo ? 512 : img.naturalHeight);
    fw = Math.max(1, Math.min(img.naturalWidth, Math.floor(fw))); fh = Math.max(1, Math.min(img.naturalHeight, Math.floor(fh)));
    const g = sheetGeometry(img, fw, fh);
    const names = Array.isArray(info.stateRows) ? info.stateRows : (g.rows === 2 ? ['idle', 'wave'] : ROWS);
    const resting = state.mood !== 'normal' || info.companion?.resting;
    const quiet = state.animation?.enabled === false || state.animation?.fpsCap === 0;
    let candidates = ['idle'], once = false, actionStart = 0;
    if (resting) candidates = ['sleeping','sleep','waiting','idle'];
    else if (reaction && now < reaction.until) {
      candidates = reaction.kind === 'complete' && !profile.calm ? ['jumping','jump','waving','wave','idle'] : ['waving','wave','idle'];
      once = true; actionStart = reaction.start;
    } else if (state.dragging) candidates = ['running-right','running','idle'];
    else if (state.activity.error) candidates = ['failed','idle'];
    else if (state.activity.busy) {
      // Hermes performs its own spell once, with a rest between casts.
      if (profile.spell && (now - busyStartedAt) % 6500 < 950) {
        candidates = ['wave','waving','idle']; once = true; actionStart = now - (now - busyStartedAt) % 6500;
      } else candidates = ['review','waiting','idle'];
    }
    let row = rowFor(names, candidates, g.rows); if (row < 0) row = 0;
    let action = names[row] || 'idle';
    const available = g.counts[row];
    const explicitCount = Number(info.framesByRow?.[names[row]] || info.framesByState?.[action] || (!PROFILES[state.character] ? info.framesPerState : 0));
    const count = Math.max(1, Math.min(available, explicitCount || available));
    const loop = Math.max(100, Number(info.loopMs) || count * (profile.calm ? 190 : 160));
    let frameIndex = quiet || resting ? 0 : Math.floor((once ? Math.min(.999, (now - actionStart) / loop) : now % loop / loop) * count);
    // Hermes' idle views are orientations. Other two-row pets blink using the
    // original closed-eye frame, rather than continuously spinning their views.
    if (action === 'idle' && duo && !quiet) frameIndex = profile.blink && now % 4300 < 150 ? Math.min(2, count - 1) : 0;
    if (resting && duo && profile.blink) frameIndex = Math.min(2, count - 1);
    if (!resting && !quiet && action === 'idle' && profile.directions && v2 && now - pointerMovedAt < 1800) {
      const sector = ((Math.round(Math.atan2(-state.pointerY, state.pointerX) / (Math.PI * 2) * 16) % 16) + 16) % 16;
      row = 9 + Math.floor(sector / 8); frameIndex = sector % 8; action = 'look';
    }
    const b = g.union; if (b.right <= b.left || b.bottom <= b.top) return;
    const factor = Math.min(1, Math.max(.45, (Number(info.scale) || .33) / .33));
    const s = Math.min(260 / (b.right - b.left), 275 / (b.bottom - b.top), 3) * factor;
    const dx = WIDTH / 2 - (b.left + b.right) / 2 * s;
    const dy = HEIGHT - 10 - b.bottom * s + (quiet ? 0 : Math.sin(now * (resting ? .0016 : .003)));
    spritePose = { img, fw, fh, sx: frameIndex * fw, sy: row * fh, dx, dy, dw: fw * s, dh: fh * s, row, frameIndex, action };
    ctx.imageSmoothingEnabled = false; ctx.drawImage(img, spritePose.sx, spritePose.sy, fw, fh, dx, dy, fw * s, fh * s);
    const rect = (x,y,w,h) => ({ x: offsetX + x * scale, y: offsetY + y * scale, width: w * scale, height: h * scale });
    const ib = g.frames[0].right > 0 ? g.frames[0] : b;
    const x = dx + ib.left * s, y = dy + ib.top * s, w = (ib.right - ib.left) * s, h = (ib.bottom - ib.top) * s;
    spriteRegions = { head: rect(x + w * .18, y + h * .06, w * .64, h * .42),
      rects: [rect(dx + b.left * s, dy + b.top * s, (b.right - b.left) * s, (b.bottom - b.top) * s)] };
    const key = [state.character,fw,fh,s,scale,offsetX,offsetY].join('/');
    if (key !== geometryKey) { geometryKey = key; queueMicrotask(() => { syncInteractionRegions(); window.dispatchEvent(new Event('readmd-pet-character-changed')); }); }
  }
  // Only BongoCat attacks key/mouse controls. Native counters retain fast edges.
  function consumeInput(payload) {
    if (!payload) return;
    const previous = lastInput;
    if (previous && Number.isInteger(payload.sequence) && Number.isInteger(previous.sequence) && ((payload.sequence - previous.sequence) >>> 0) > 0x7fffffff) return;
    lastInput = payload;
    const x = Number.isFinite(payload.pointer_x) ? payload.pointer_x : state.pointerX, y = Number.isFinite(payload.pointer_y) ? payload.pointer_y : state.pointerY;
    if (Math.hypot(x - state.pointerX,y - state.pointerY) > .03) pointerMovedAt = performance.now();
    state.pointerX = x; state.pointerY = y; state.lastKey = payload.last_key ?? state.lastKey;
    state.pressedKeys = new Set(Array.isArray(payload.pressed_keys) ? payload.pressed_keys.slice(0,256) : []);
    state.mouseButtons = payload.mouse_buttons || 0; state.mouseDown = !!payload.mouse_down;
    state.leftDown = !!(payload.keyboard_down ?? (payload.left_down || payload.right_down)); state.rightDown = state.mouseDown;
    const changed = key => Number.isInteger(payload[key]) && payload[key] !== (previous?.[key] ?? 0);
    if (classicEnabled) {
      if (changed('keyboard_taps')) { state.leftTapAt = performance.now(); state.tapCounts.left++; sound(true); }
      if (changed('mouse_taps')) { state.rightTapAt = performance.now(); state.tapCounts.right++; sound(true); }
    }
  }
  api?.onBongoInput?.(consumeInput);
  api?.onState?.(snapshot => {
    if (!snapshot) return;
    if (snapshot.bounds) hostBounds = snapshot.bounds;
    let character = snapshot.character || snapshot.info?.character || snapshot.info?.slug || state.character;
    if (urlLive) character = 'arch-chan';
    if (snapshot.info) characterInfo[character] = snapshot.info;
    if (character !== lastSnapshotCharacter) {
      const preserve = lastSnapshotCharacter === undefined && character === state.character;
      lastSnapshotCharacter = character; applyCharacter(character,preserve);
    }
    if (snapshot.info && character === state.character) state.petInfo = snapshot.info;
    const sheet = snapshot.info?.spritesheetBase64 || snapshot.spritesheetBase64;
    if (sheet && (sheet !== lastSheet || character !== lastSheetCharacter)) {
      lastSheet = sheet; lastSheetCharacter = character;
      const img = new Image(); img.readmdImported = true;
      img.onload = () => { if (lastSheet !== sheet || lastSheetCharacter !== character) return; images[character] = img; geometryKey = ''; };
      img.src = `data:${snapshot.info?.mime || snapshot.mime || 'image/png'};base64,${sheet}`;
    }
    const activity = snapshot.activity || {};
    if (activity.busy && !state.activity.busy) busyStartedAt = performance.now();
    if (activity.justCompleted && !state.activity.justCompleted) respond('complete');
    state.activity = activity; state.animation = snapshot.info?.animation;
    sceneActive = !snapshot.fullscreen && snapshot.visible !== false;
    updateMenu(); syncInteractionRegions();
  });
  api?.onControl?.(payload => {
    if (payload?.type === 'character') applyCharacter(payload.character);
    if (payload?.type === 'pet') petPet();
    if (payload?.type === 'play') respond('complete');
  });
  // One drag request after the threshold. Cancelled/moved gestures never pet.
  let gesture = null, strokeX, strokeDistance = 0, strokeAt = 0;
  canvas.addEventListener('pointerdown', event => {
    if (event.button !== 0) return;
    gesture = { x: event.screenX, y: event.screenY, pointer: event.pointerId, started: false }; canvas.setPointerCapture?.(event.pointerId);
  });
  canvas.addEventListener('pointermove', event => {
    if (gesture) {
      if (!gesture.started && (event.buttons & 1) && Math.hypot(event.screenX - gesture.x,event.screenY - gesture.y) > 4) {
        gesture.started = true; state.dragging = true; canvas.style.cursor = 'grabbing'; api?.startDrag?.();
      }
      return;
    }
    if (event.buttons) return;
    const h = currentRegions().head;
    if (!h || Math.hypot((event.clientX-h.x-h.width/2)/(h.width/2),(event.clientY-h.y-h.height/2)/(h.height/2)) > 1) { strokeX = undefined; strokeDistance = 0; return; }
    const now = performance.now(); if (now - strokeAt > 600) strokeDistance = 0;
    if (strokeX !== undefined) strokeDistance += Math.abs(event.clientX - strokeX);
    strokeAt = now; strokeX = event.clientX;
    if (strokeDistance > Math.max(80,h.width * 1.5)) { strokeDistance = 0; petPet(); }
  });
  function finishGesture(event,cancelled) {
    const ended = gesture; gesture = null; state.dragging = false; canvas.style.cursor = 'grab'; if (!ended) return;
    try { canvas.releasePointerCapture?.(ended.pointer); } catch (_) {}
    if (!cancelled && !ended.started && !window.__readmdRustDispatch) {
      const h = currentRegions().head;
      if (h && event.clientX >= h.x && event.clientX <= h.x+h.width && event.clientY >= h.y && event.clientY <= h.y+h.height) petPet();
    }
  }
  window.addEventListener('pointerup', e => finishGesture(e,false)); window.addEventListener('pointercancel', e => finishGesture(e,true));
  canvas.addEventListener('lostpointercapture', e => finishGesture(e,true)); window.addEventListener('blur', e => finishGesture(e,true));
  const menu = document.createElement('div'); menu.setAttribute('role','menu');
  menu.style.cssText = 'position:absolute;display:none;z-index:9999;min-width:168px;max-height:calc(100% - 8px);overflow:auto;padding:4px;background:rgba(28,30,36,.96);border:1px solid #ffffff14;border-radius:10px;font:13px/1.4 system-ui;color:#eee;user-select:none';
  const text = (key,fallback) => state.petInfo.lines?.[key] || fallback;
  function menuItem(label,run,checked) {
    const item = document.createElement('button'); item.type = 'button'; item.textContent = `${checked ? '✓ ' : ''}${label}`;
    item.setAttribute('role',checked === undefined ? 'menuitem' : 'menuitemradio'); if (checked !== undefined) item.setAttribute('aria-checked',String(checked));
    item.style.cssText = 'display:block;width:100%;min-height:44px;padding:8px 10px;border:0;border-radius:6px;background:transparent;color:inherit;font:inherit;text-align:left;cursor:pointer';
    item.addEventListener('mouseenter',() => item.style.background = '#ffffff14'); item.addEventListener('mouseleave',() => item.style.background = 'transparent');
    item.addEventListener('click',event => { event.stopPropagation(); menu.style.display = 'none'; run(); }); return item;
  }
  function updateMenu() {
    menu.replaceChildren();
    for (const [id,profile] of Object.entries(PROFILES)) menu.appendChild(menuItem(profile.name,() => applyCharacter(id),!classicEnabled && state.character === id));
    menu.appendChild(menuItem('BongoCat',useClassic,classicEnabled));
    menu.appendChild(menuItem(text('pet.action.pet','摸摸头'),petPet));
    menu.appendChild(menuItem(text('pet.action.play','玩耍'),() => respond('complete')));
    if (classicEnabled) menu.appendChild(menuItem(soundEnabled ? '关闭按键音效' : '开启按键音效',() => {
      soundEnabled = !soundEnabled; localStorage.setItem('readmd-pet-sound',String(soundEnabled)); updateMenu();
    }));
    menu.appendChild(menuItem(text('pet.hideTitle','隐藏桌宠'),() => api?.close?.()));
  }
  document.body.appendChild(menu); updateMenu();
  canvas.addEventListener('contextmenu',event => {
    event.preventDefault(); updateMenu(); menu.style.display = 'block';
    menu.style.left = `${Math.max(4,Math.min(event.clientX,innerWidth-menu.offsetWidth-4))}px`;
    menu.style.top = `${Math.max(4,Math.min(event.clientY,innerHeight-menu.offsetHeight-4))}px`;
  });
  window.addEventListener('click',() => menu.style.display = 'none');
  function drawHearts(dt) {
    ctx.save(); ctx.font = '18px system-ui'; ctx.textAlign = 'center'; ctx.fillStyle = '#ff7b9c';
    for (let i = state.hearts.length-1; i >= 0; i--) {
      const h = state.hearts[i]; h.age += dt; h.x += h.vx*dt; h.y += h.vy*dt;
      ctx.globalAlpha = Math.max(0,1-h.age/.9); ctx.fillText('♥',h.x,h.y); if (h.age >= .9) state.hearts.splice(i,1);
    }
    ctx.restore();
  }
  function syncInteractionRegions() {
    // WebView zoom changes CSS pixels independently of the native window's
    // logical size. DOM gestures keep CSS coordinates; native hit testing gets
    // window coordinates, including after DPI changes and resizing.
    const sx = hostBounds?.width ? hostBounds.width / innerWidth : 1;
    const sy = hostBounds?.height ? hostBounds.height / innerHeight : 1;
    const nativeRect = r => ({ x:r.x*sx,y:r.y*sy,width:r.width*sx,height:r.height*sy });
    const regions = currentRegions();
    api?.control?.({ type:'interaction-regions',rects:regions.rects.map(nativeRect),
      ...(regions.head ? {head:nativeRect(regions.head)} : {}) });
  }
  window.addEventListener('readmd-bongo-layout',syncInteractionRegions);
  let renderErrors = 0, lastPaintAt = -Infinity;
  function render(now) {
    const dt = Math.min(.05,Math.max(0,(now-lastFrameAt)/1000)); lastFrameAt = now;
    state.pettingLevel = Math.max(0,state.pettingLevel-dt*60);
    const active = sceneActive;
    classicController?.setActive(classicEnabled && active);
    liveController?.setPresentation({ active: !classicEnabled && state.character === 'arch-chan' && active });
    const cap = Number(state.animation?.fpsCap), interval = 1000/(cap > 0 ? Math.min(60,cap) : 60);
    if ((active || resolveFirstPaint) && now-lastPaintAt >= interval) try {
      lastPaintAt = now; ctx.setTransform(1,0,0,1,0,0); ctx.clearRect(0,0,canvas.width,canvas.height);
      const dpr = Math.min(2,devicePixelRatio || 1); ctx.setTransform(dpr*scale,0,0,dpr*scale,dpr*offsetX,dpr*offsetY);
      if (!classicEnabled && state.character !== 'arch-chan') drawSprite(now); drawHearts(dt);
      if (resolveFirstPaint && lastSnapshotCharacter !== undefined && (classicEnabled ? classicController : state.character === 'arch-chan' ? liveController : spritePose)) {
        syncInteractionRegions(); resolveFirstPaint(); resolveFirstPaint = undefined;
      }
    } catch (error) { if (renderErrors++ < 3) console.error('pet scene failed',error); }
    requestAnimationFrame(render);
  }
  window.__bongoPet = { state,applyCharacter,useClassic,petPet,consumeInput,
    get spritePose() { return spritePose; }, get presentation() { return classicEnabled ? 'bongocat' : state.character; },
    get interactionRegions() { return currentRegions(); } };
  if (state.character === 'arch-chan') window.readmdLive2dReady = applyCharacter('arch-chan');
  window.readmdClassicReady = classicEnabled ? ensureClassic() : Promise.resolve();
  api?.control?.({ type:window.__readmdRustDispatch ? 'state-request' : 'ready' });
  requestAnimationFrame(render); const timer = setInterval(syncInteractionRegions,1000);
  window.addEventListener('pagehide',() => clearInterval(timer),{ once: true });
})();
