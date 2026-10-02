/**
 * Bongo Cat Desktop Pet Engine - Multi-Character Sprite & Live2D Edition
 * Pure Rust ReadMD Desktop Pet Renderer
 *
 * Supported Characters:
 * - Mochi (小猫): White pixel cat with green scarf & pink toe beans
 * - Hermes (骑士): Blue armored winged knight with gold gauntlets & caduceus
 * - Amber (狐狸): Warm orange fox/cat with dark pads
 * - Moss (小草): Cute green sprout with leaf paws
 * - Arch-Chan (Live2D): Interactive Cubism 4 anime mascot with academy uniform
 *
 * Features:
 * - 100% Desktop Transparent DirectComposition
 * - Dual Instrument Modes: Mechanical Keyboard + Mouse OR Classic Bongo Drums
 * - Real-Time Spring-Physics Animated Paws with Character-Specific Aesthetics
 * - Real-Time Global Keystroke & Mouse Tracking via Rust InputWatcher IPC
 * - Dynamic Reaction: Idle Animation (Row 0) -> Typing Frenzy / Waving (Row 1)
 * - Head Petting: Blushing cheeks, purr audio, floating heart particles
 * - Audio Synthesis: Web Audio API mechanical switch clicks & bongo drum pops
 * - Translucent Glass Right-Click Context Menu for Character & Mode Switching
 */

(function () {
  'use strict';

  // --- Constants & Canvas Dimensions ---
  const CANVAS_WIDTH = 320;
  const CANVAS_HEIGHT = 380;
  const CAT_CENTER_X = 160;

  // --- Character Profiles ---
  const CHARACTER_PROFILES = {
    mochi: {
      id: 'mochi',
      name: 'Mochi (小猫)',
      icon: '🐱',
      sprite: './assets/mochi-sprite.png',
      scale: 0.78,
      offsetY: -55,
      headCenterY: 180,
      pawStyle: {
        type: 'cat',
        fill: '#fdfbf7',
        stroke: '#4a3b32',
        padFill: '#ffb2be',
        armWidth: 7
      }
    },
    hermes: {
      id: 'hermes',
      name: 'Hermes (骑士)',
      icon: '🛡️',
      sprite: './assets/hermes-sprite.png',
      scale: 0.75,
      offsetY: -45,
      headCenterY: 165,
      pawStyle: {
        type: 'knight',
        fill: '#2980b9',
        stroke: '#1a252f',
        padFill: '#f1c40f',
        armWidth: 8
      }
    },
    amber: {
      id: 'amber',
      name: 'Amber (狐狸)',
      icon: '🦊',
      sprite: './assets/amber-sprite.png',
      scale: 0.78,
      offsetY: -55,
      headCenterY: 180,
      pawStyle: {
        type: 'cat',
        fill: '#e67e22',
        stroke: '#5c2a00',
        padFill: '#d35400',
        armWidth: 7
      }
    },
    moss: {
      id: 'moss',
      name: 'Moss (小草)',
      icon: '🌱',
      sprite: './assets/moss-sprite.png',
      scale: 0.82,
      offsetY: -68,
      headCenterY: 190,
      pawStyle: {
        type: 'sprout',
        fill: '#a8e6cf',
        stroke: '#2e7d32',
        padFill: '#26de81',
        armWidth: 7
      }
    },
    'cache-capy': {
      id: 'cache-capy',
      name: 'Cache Capy (淡定水豚)',
      icon: '🦫',
      sprite: './assets/cache-capy-sprite.webp',
      frameW: 192,
      frameH: 208,
      scale: 1.25,
      offsetY: 25,
      headCenterY: 165,
      pawStyle: {
        type: 'capy',
        fill: '#ba8c59',
        stroke: '#52341d',
        padFill: '#38200e',
        armWidth: 8
      }
    },
    'niu-lai': {
      id: 'niu-lai',
      name: '牛来 (招财毛绒牛)',
      icon: '🐂',
      sprite: './assets/niu-lai-sprite.webp',
      frameW: 192,
      frameH: 208,
      scale: 1.22,
      offsetY: 22,
      headCenterY: 160,
      pawStyle: {
        type: 'cow',
        fill: '#f39c12',
        stroke: '#8c4b00',
        padFill: '#ff9ebb',
        armWidth: 8
      }
    },
    'arch-chan': {
      id: 'arch-chan',
      name: 'Arch-Chan (Live2D)',
      icon: '🎀',
      isLive2D: true,
      headCenterY: 120,
      pawStyle: {
        type: 'human',
        fill: '#fde9db',
        stroke: '#181b22',
        padFill: '#00d2d3', // Cyber cyan accent line
        sleeveFill: '#1c2028', // Dark hoodie sleeve
        armWidth: 8
      }
    }
  };

  // --- Sound Synthesis (Web Audio API) ---
  let audioCtx = null;
  let soundEnabled = localStorage.getItem('readmd-pet-sound') !== 'false';

  function initAudio() {
    if (!audioCtx) {
      try {
        const AudioContext = window.AudioContext || window.webkitAudioContext;
        audioCtx = new AudioContext();
      } catch (_) {}
    }
  }

  function playTapSound(isLeft, mode) {
    if (!soundEnabled || !audioCtx) return;
    try {
      if (audioCtx.state === 'suspended') audioCtx.resume();
      const now = audioCtx.currentTime;
      const osc = audioCtx.createOscillator();
      const gain = audioCtx.createGain();
      osc.connect(gain);
      gain.connect(audioCtx.destination);

      if (mode === 'bongos') {
        // Resonant bongo pop sound
        const freq = isLeft ? 180 : 250;
        osc.type = 'sine';
        osc.frequency.setValueAtTime(freq, now);
        osc.frequency.exponentialRampToValueAtTime(50, now + 0.08);
        gain.gain.setValueAtTime(0.24, now);
        gain.gain.exponentialRampToValueAtTime(0.001, now + 0.09);
        osc.start(now);
        osc.stop(now + 0.09);
      } else {
        // Crisp mechanical keyboard switch click
        const freq = isLeft ? 880 : 1150;
        osc.type = 'triangle';
        osc.frequency.setValueAtTime(freq, now);
        osc.frequency.exponentialRampToValueAtTime(220, now + 0.04);
        gain.gain.setValueAtTime(0.18, now);
        gain.gain.exponentialRampToValueAtTime(0.001, now + 0.045);
        osc.start(now);
        osc.stop(now + 0.045);
      }
    } catch (_) {}
  }

  function playPurrSound() {
    if (!soundEnabled || !audioCtx) return;
    try {
      if (audioCtx.state === 'suspended') audioCtx.resume();
      const now = audioCtx.currentTime;
      const osc = audioCtx.createOscillator();
      const gain = audioCtx.createGain();
      osc.connect(gain);
      gain.connect(audioCtx.destination);

      osc.type = 'sine';
      osc.frequency.setValueAtTime(440, now);
      osc.frequency.linearRampToValueAtTime(680, now + 0.12);
      osc.frequency.linearRampToValueAtTime(520, now + 0.22);
      gain.gain.setValueAtTime(0.15, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.25);
      osc.start(now);
      osc.stop(now + 0.25);
    } catch (_) {}
  }

  // --- Preload Sprite Sheets ---
  const loadedImages = {};
  for (const [key, profile] of Object.entries(CHARACTER_PROFILES)) {
    if (profile.sprite) {
      const img = new Image();
      img.src = profile.sprite;
      loadedImages[key] = img;
    }
  }

  // --- Pet State ---
  const urlParams = new URLSearchParams(window.location.search);
  const isUrlLive2D = urlParams.get('renderer') === 'live2d';

  const savedChar = localStorage.getItem('readmd-pet-character');
  const initialChar = isUrlLive2D ? 'arch-chan' : (CHARACTER_PROFILES[savedChar] ? savedChar : 'mochi');
  const savedMode = localStorage.getItem('readmd-pet-mode');
  const initialMode = savedMode === 'bongos' ? 'bongos' : 'keyboard';
  const savedShowDesk = localStorage.getItem('readmd-pet-showdesk');
  const initialShowDesk = isUrlLive2D ? false : (savedShowDesk !== 'false');

  const state = {
    character: initialChar,
    mode: initialMode, // 'keyboard' | 'bongos'
    showDesk: initialShowDesk,
    leftDown: false,
    rightDown: false,
    mouseDown: false,
    mouseX: 160,
    mouseY: 200,

    // Paw Kinematics & Spring Physics
    pawLeftY: 245,
    pawLeftTargetY: 245,
    pawLeftSquish: 1.0,

    pawRightY: 245,
    pawRightTargetY: 245,
    pawRightSquish: 1.0,

    // Petting & Expression
    pettingLevel: 0,
    typingBpm: 0,
    lastTapTime: 0,

    // Particles & FX
    hearts: [],
    sparks: [],
    drumRings: [],
    keyGlows: new Array(31).fill(0)
  };

  // --- Companion behaviours: typing combo, dozing off when idle, stroke to pet ---
  const SLEEP_AFTER_MS = 45000;
  const life = { combo: 0, comboAt: 0, comboPop: 0, lastInput: performance.now(), zz: [], zzAt: 0, stroke: 0, strokeAt: 0, lastX: null };

  function noteInput() {
    const now = performance.now();
    life.combo = now - life.comboAt < 900 ? life.combo + 1 : 1;
    life.comboAt = now;
    life.comboPop = 1;
    life.lastInput = now;
    life.zz.length = 0;
  }

  function isSleeping(now) {
    return now - life.lastInput > SLEEP_AFTER_MS && state.pettingLevel <= 0;
  }

  // Moving the cursor back and forth over the head strokes the pet.
  function noteStroke(x, y) {
    const profile = CHARACTER_PROFILES[state.character] || CHARACTER_PROFILES.mochi;
    const hy = profile.headCenterY || 180;
    const now = performance.now();
    if (Math.hypot(x - CAT_CENTER_X, y - hy) > 64) { life.lastX = null; return; }
    if (now - life.strokeAt > 600) life.stroke = 0;
    if (life.lastX != null) life.stroke += Math.abs(x - life.lastX);
    life.lastX = x;
    life.strokeAt = now;
    if (life.stroke > 240) {
      life.stroke = 0;
      life.lastInput = now;
      petPet();
    }
  }

  // --- DOM Setup & Responsive Geometry ---
  const canvas = document.getElementById('bongocat-canvas');
  const ctx = canvas.getContext('2d');
  const live2dStage = document.getElementById('live2d-stage');

  let currentScale = 1.0;
  let currentOffsetX = 0;
  let currentOffsetY = 0;

  function setupDpi() {
    const dpr = window.devicePixelRatio || 1;
    const viewW = window.innerWidth || CANVAS_WIDTH;
    const viewH = window.innerHeight || CANVAS_HEIGHT;
    canvas.width = Math.round(viewW * dpr);
    canvas.height = Math.round(viewH * dpr);
    canvas.style.width = '100%';
    canvas.style.height = '100%';

    // Scale uniformly to fit within window while preserving 320x380 aspect ratio
    currentScale = Math.min(viewW / CANVAS_WIDTH, viewH / CANVAS_HEIGHT);
    currentOffsetX = (viewW - CANVAS_WIDTH * currentScale) / 2;
    // Bottom-align so desk rests naturally on window base
    currentOffsetY = (viewH - CANVAS_HEIGHT * currentScale);
  }
  setupDpi();
  window.addEventListener('resize', setupDpi);

  // --- Live2D Lazy Mounting ---
  let live2dMounted = false;
  let live2dController = null;

  async function ensureLive2d() {
    if (live2dMounted) return;
    live2dMounted = true;
    try {
      const { mountLive2dStage } = await import('./stage-BjWFpm-D.js');
      live2dController = await mountLive2dStage();
      console.log('[BongoPet] Live2D Arch-Chan stage loaded successfully');
    } catch (err) {
      console.error('[BongoPet] Live2D load error:', err);
    }
  }

  async function applyCharacter(charId) {
    if (!charId) return;
    state.character = charId;
    localStorage.setItem('readmd-pet-character', charId);

    if (CHARACTER_PROFILES[charId] && CHARACTER_PROFILES[charId].sprite && !loadedImages[charId]) {
      const img = new Image();
      img.src = CHARACTER_PROFILES[charId].sprite;
      loadedImages[charId] = img;
    }

    if (charId === 'arch-chan') {
      if (live2dStage) live2dStage.style.display = 'block';
      await ensureLive2d();
    } else {
      if (live2dStage) live2dStage.style.display = 'none';
    }
    updateContextMenu();
  }

  // --- Context menu: compact and text-only, matching the app's quiet style ---
  const menu = document.createElement('div');
  menu.setAttribute('role', 'menu');
  menu.style.cssText = `
    position: absolute;
    display: none;
    z-index: 9999;
    min-width: 168px;
    max-height: calc(100% - 8px);
    overflow-y: auto;
    padding: 4px;
    background: rgba(28, 30, 36, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    font: 12px/1.4 -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
    color: #e8e8ea;
    user-select: none;
  `;

  function createMenuItem(label, onClick, checked) {
    const item = document.createElement('div');
    item.setAttribute('role', checked === undefined ? 'menuitem' : 'menuitemradio');
    if (checked !== undefined) item.setAttribute('aria-checked', String(!!checked));
    item.style.cssText = 'display:flex;align-items:center;gap:8px;padding:6px 10px;border-radius:6px;cursor:pointer;';
    const text = document.createElement('span');
    text.style.cssText = 'flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
    text.textContent = label;
    const mark = document.createElement('span');
    mark.style.cssText = 'width:12px;text-align:right;color:#8ab4ff;';
    mark.textContent = checked ? '✓' : '';
    item.append(text, mark);
    item.addEventListener('mouseenter', () => item.style.background = 'rgba(255, 255, 255, 0.08)');
    item.addEventListener('mouseleave', () => item.style.background = 'transparent');
    item.addEventListener('click', (e) => {
      e.stopPropagation();
      menu.style.display = 'none';
      onClick();
    });
    return item;
  }

  function createSeparator() {
    const sep = document.createElement('div');
    sep.style.cssText = 'height:1px;background:rgba(255,255,255,0.08);margin:4px 6px;';
    return sep;
  }

  function createLabel(text) {
    const label = document.createElement('div');
    label.style.cssText = 'padding:6px 10px 2px;font-size:11px;color:#8b8f99;';
    label.textContent = text;
    return label;
  }

  function setInstrument(mode, showDesk) {
    state.mode = mode;
    state.showDesk = showDesk;
    localStorage.setItem('readmd-pet-mode', mode);
    localStorage.setItem('readmd-pet-showdesk', String(showDesk));
    updateContextMenu();
  }

  function updateContextMenu() {
    menu.innerHTML = '';
    menu.appendChild(createLabel('角色'));
    for (const [id, prof] of Object.entries(CHARACTER_PROFILES)) {
      menu.appendChild(createMenuItem(prof.name, () => applyCharacter(id), state.character === id));
    }
    menu.appendChild(createSeparator());
    menu.appendChild(createLabel('乐器'));
    const desk = state.showDesk !== false;
    menu.appendChild(createMenuItem('键盘与鼠标', () => setInstrument('keyboard', true), desk && state.mode === 'keyboard'));
    menu.appendChild(createMenuItem('邦戈鼓', () => setInstrument('bongos', true), desk && state.mode === 'bongos'));
    menu.appendChild(createMenuItem('不显示乐器', () => setInstrument(state.mode, false), !desk));
    menu.appendChild(createSeparator());
    menu.appendChild(createMenuItem(soundEnabled ? '关闭按键音效' : '开启按键音效', () => {
      soundEnabled = !soundEnabled;
      localStorage.setItem('readmd-pet-sound', String(soundEnabled));
      updateContextMenu();
    }));
    menu.appendChild(createMenuItem('摸摸头', () => petPet()));
    menu.appendChild(createSeparator());
    menu.appendChild(createMenuItem('隐藏桌宠', () => {
      window.hermesDesktop?.petOverlay?.close?.();
    }));
  }

  document.body.appendChild(menu);
  updateContextMenu();

  // If initially arch-chan, mount Live2D
  if (initialChar === 'arch-chan') {
    applyCharacter('arch-chan');
  }

  // --- Interaction & Petting ---
  canvas.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    initAudio();
    updateContextMenu();
    menu.style.display = 'block';
    const maxX = (window.innerWidth || CANVAS_WIDTH) - menu.offsetWidth - 4;
    const maxY = (window.innerHeight || CANVAS_HEIGHT) - menu.offsetHeight - 4;
    const x = Math.min(e.clientX, maxX);
    const y = Math.min(e.clientY, maxY);
    menu.style.left = `${Math.max(4, x)}px`;
    menu.style.top = `${Math.max(4, y)}px`;
  });

  window.addEventListener('click', () => {
    menu.style.display = 'none';
  });

  canvas.addEventListener('pointerdown', (e) => {
    if (e.button === 2) return;
    initAudio();

    const rect = canvas.getBoundingClientRect();
    const screenX = e.clientX - rect.left;
    const screenY = e.clientY - rect.top;

    // Normalize screen click into 320x380 canonical canvas coordinate space
    const clickX = (screenX - currentOffsetX) / (currentScale || 1.0);
    const clickY = (screenY - currentOffsetY) / (currentScale || 1.0);

    const profile = CHARACTER_PROFILES[state.character] || CHARACTER_PROFILES.mochi;
    const hy = profile.headCenterY || 180;
    const dist = Math.hypot(clickX - CAT_CENTER_X, clickY - hy);

    if (dist < 60) {
      petPet();
    } else {
      window.hermesDesktop?.petOverlay?.startDrag?.();
    }

    canvas.style.cursor = 'grabbing';
  });

  canvas.addEventListener('pointermove', (e) => {
    if (e.buttons) return;
    const rect = canvas.getBoundingClientRect();
    noteStroke((e.clientX - rect.left - currentOffsetX) / (currentScale || 1.0), (e.clientY - rect.top - currentOffsetY) / (currentScale || 1.0));
  });

  window.addEventListener('pointerup', () => {
    canvas.style.cursor = 'grab';
  });

  function petPet() {
    initAudio();
    playPurrSound();
    state.pettingLevel = 45;

    if (state.character === 'arch-chan') {
      live2dController?.celebrate?.();
    }

    // Spawn floating heart particles
    for (let i = 0; i < 4; i++) {
      state.hearts.push({
        x: CAT_CENTER_X + (Math.random() * 50 - 25),
        y: (CHARACTER_PROFILES[state.character]?.headCenterY || 180) - 35 + (Math.random() * 20 - 10),
        vx: (Math.random() - 0.5) * 1.6,
        vy: -1.8 - Math.random() * 1.5,
        alpha: 1.0,
        scale: 0.8 + Math.random() * 0.5,
        color: Math.random() > 0.3 ? '#ff6b8b' : '#ff9ebb'
      });
    }
  }

  // --- Input Reactivity ---
  function recordTap() {
    const now = performance.now();
    const delta = now - state.lastTapTime;
    state.lastTapTime = now;
    if (delta < 500) {
      state.typingBpm = Math.min(180, state.typingBpm + 8);
    } else {
      state.typingBpm = Math.min(180, state.typingBpm + 4);
    }
  }

  function triggerLeftTap(isDown) {
    if (state.leftDown === isDown) return;
    state.leftDown = isDown;
    if (isDown) {
      initAudio();
      playTapSound(true, state.mode);
      recordTap();
      noteInput();
      state.pawLeftSquish = 0.65;

      if (state.mode === 'bongos') {
        state.drumRings.push({ x: 95, y: 275, r: 15, alpha: 0.9, color: 'rgba(230, 126, 34, 0.8)' });
      } else {
        lightKey('left');
        // Spawn sparks
        for (let i = 0; i < 3; i++) {
          state.sparks.push({
            x: 80 + Math.random() * 40,
            y: 285 + Math.random() * 20,
            vx: (Math.random() - 0.5) * 2.5,
            vy: -1.2 - Math.random() * 2.0,
            alpha: 1.0,
            color: '#9cc0ff'
          });
        }
      }
    }
  }

  function triggerRightTap(isDown) {
    if (state.rightDown === isDown) return;
    state.rightDown = isDown;
    if (isDown) {
      initAudio();
      playTapSound(false, state.mode);
      recordTap();
      noteInput();
      state.pawRightSquish = 0.65;

      if (state.mode === 'bongos') {
        state.drumRings.push({ x: 225, y: 275, r: 16, alpha: 0.9, color: 'rgba(243, 156, 18, 0.8)' });
      } else {
        lightKey('right');
        for (let i = 0; i < 3; i++) {
          state.sparks.push({
            x: 235 + Math.random() * 20,
            y: 290 + Math.random() * 15,
            vx: (Math.random() - 0.5) * 2.5,
            vy: -1.2 - Math.random() * 2.0,
            alpha: 1.0,
            color: '#9cc0ff'
          });
        }
      }
    }
  }

  // --- Global IPC Listener from Rust ---
  if (window.hermesDesktop?.petOverlay?.onBongoInput) {
    window.hermesDesktop.petOverlay.onBongoInput((payload) => {
      if (!payload) return;
      if (typeof payload.left_down === 'boolean') triggerLeftTap(payload.left_down);
      if (typeof payload.right_down === 'boolean') triggerRightTap(payload.right_down);
      if (typeof payload.mouse_down === 'boolean') {
        state.mouseDown = payload.mouse_down;
        if (payload.mouse_down) triggerRightTap(true);
        else triggerRightTap(false);
      }
      if (typeof payload.mouse_x === 'number') state.mouseX = payload.mouse_x;
      if (typeof payload.mouse_y === 'number') state.mouseY = payload.mouse_y;
    });
  }

  // Support state updates from host snapshot
  let lastSeenSnapshotChar = null;
  if (window.hermesDesktop?.petOverlay?.onState) {
    window.hermesDesktop.petOverlay.onState((snap) => {
      if (!snap) return;

      // Multi-source character detection
      let char = snap.character ||
                 (snap.info && snap.info.character) ||
                 (snap.info && snap.info.slug) ||
                 (snap.companion && snap.companion.character) ||
                 (snap.info && snap.info.companion && snap.info.companion.character);

      if (!char && snap.info && snap.info.displayName) {
        const name = String(snap.info.displayName).toLowerCase();
        if (name.includes('mochi') || name.includes('糯米') || name.includes('小猫')) char = 'mochi';
        else if (name.includes('amber') || name.includes('狐狸')) char = 'amber';
        else if (name.includes('hermes') || name.includes('骑士')) char = 'hermes';
        else if (name.includes('moss') || name.includes('小草')) char = 'moss';
        else if (name.includes('capy') || name.includes('水豚')) char = 'cache-capy';
        else if (name.includes('niu') || name.includes('牛')) char = 'niu-lai';
        else if (name.includes('arch') || name.includes('live2d')) char = 'arch-chan';
      }

      if (snap.info) {
        state.petInfo = snap.info;
      }

      if (char && char !== lastSeenSnapshotChar) {
        lastSeenSnapshotChar = char;
        applyCharacter(char);
      }

      // Dynamic custom spritesheet loading from base64
      const spritesheetB64 = (snap.info && snap.info.spritesheetBase64) || snap.spritesheetBase64;
      if (spritesheetB64) {
        const mime = (snap.info && snap.info.mime) || snap.mime || 'image/png';
        const targetKey = char || state.character;
        const img = new Image();
        img.onload = () => {
          loadedImages[targetKey] = img;
        };
        img.src = `data:${mime};base64,${spritesheetB64}`;
      }

      const m = snap.mode || (snap.info && snap.info.mode);
      if (m && (m === 'keyboard' || m === 'bongos')) {
        state.mode = m;
        updateContextMenu();
      }
    });
  }

  // Support host control events (character switch, mode, pet, tap)
  if (window.hermesDesktop?.petOverlay?.onControl) {
    window.hermesDesktop.petOverlay.onControl((payload) => {
      if (!payload) return;
      if (payload.type === 'character' && payload.character) {
        applyCharacter(payload.character);
      }
      if (payload.type === 'mode' && payload.mode) {
        state.mode = payload.mode;
        localStorage.setItem('readmd-pet-mode', payload.mode);
        updateContextMenu();
      }
      if (payload.type === 'pet') {
        petPet();
      }
      if (payload.type === 'tap') {
        if (payload.side === 'left') triggerLeftTap(!!payload.down);
        else if (payload.side === 'right') triggerRightTap(!!payload.down);
        else {
          triggerLeftTap(!!payload.down);
          triggerRightTap(!!payload.down);
        }
      }
    });
  }

  // Global helper for diagnostic tests
  window.__bongoPet = {
    state,
    applyCharacter,
    petPet,
    triggerLeftTap,
    triggerRightTap
  };

  // Local window keyboard fallback
  window.addEventListener('keydown', (e) => {
    if (e.repeat) return;
    initAudio();
    const code = e.code;
    if (['KeyQ', 'KeyW', 'KeyE', 'KeyR', 'KeyA', 'KeyS', 'KeyD', 'KeyF', 'KeyZ', 'KeyX', 'KeyC', 'KeyV', 'Space', 'Tab', 'ShiftLeft'].includes(code)) {
      triggerLeftTap(true);
    } else {
      triggerRightTap(true);
    }
  });

  window.addEventListener('keyup', (e) => {
    const code = e.code;
    if (['KeyQ', 'KeyW', 'KeyE', 'KeyR', 'KeyA', 'KeyS', 'KeyD', 'KeyF', 'KeyZ', 'KeyX', 'KeyC', 'KeyV', 'Space', 'Tab', 'ShiftLeft'].includes(code)) {
      triggerLeftTap(false);
    } else {
      triggerRightTap(false);
    }
  });

  // --- Rendering Functions ---

  // 1. Draw Character (Sprite or Live2D)
  function drawCharacter(ctx) {
    if (state.character === 'arch-chan') {
      // Live2D Arch-Chan runs in #live2d-stage
      return;
    }

    const charKey = state.character;
    const profile = CHARACTER_PROFILES[charKey] || {
      id: charKey,
      name: charKey,
      icon: '🐾',
      scale: 1.0,
      offsetY: 0,
      headCenterY: 180,
      pawStyle: { type: 'cat', fill: '#fdfbf7', stroke: '#4a3b32', padFill: '#ffb2be', armWidth: 7 }
    };
    const img = loadedImages[charKey];
    if (!img || !img.complete) return;

    let fw = profile.frameW;
    let fh = profile.frameH;
    if (!fw || !fh) {
      if (img.naturalWidth === 1536 && img.naturalHeight === 2288) {
        fw = 192;
        fh = 208;
      } else if (img.naturalWidth === 1536 && img.naturalHeight === 1024) {
        fw = 384;
        fh = 512;
      } else if (state.petInfo && state.petInfo.frameW && state.petInfo.frameH) {
        fw = state.petInfo.frameW;
        fh = state.petInfo.frameH;
      } else {
        fw = 384;
        fh = 512;
      }
    }

    const scale = profile.scale || (fw === 192 ? 1.25 : 0.78);
    const offsetY = profile.offsetY !== undefined ? profile.offsetY : (fw === 192 ? 25 : -55);

    // Decision: Row 0 is idle; Row 1 is wave / typing frenzy / petting celebration
    const isHappy = state.typingBpm > 25 || state.pettingLevel > 0;
    const row = isHappy ? 1 : 0;

    const cols = Math.floor(img.naturalWidth / fw) || 4;
    const maxCols = Math.min(cols, 4);

    const frameInterval = isHappy ? 130 : 160;
    const asleep = isSleeping(performance.now());
    const frameIndex = asleep ? 0 : Math.floor((performance.now() / frameInterval) % maxCols);

    const sx = frameIndex * fw;
    const sy = row * fh;

    const dw = fw * scale;
    const dh = fh * scale;
    const dx = (CANVAS_WIDTH - dw) / 2;
    const dy = offsetY;

    // Subtle breathing vertical oscillation
    const breathe = isHappy ? Math.sin(performance.now() * 0.008) * 2 : Math.sin(performance.now() * (asleep ? 0.0016 : 0.003)) * (asleep ? 2.2 : 1.5);

    ctx.save();
    ctx.imageSmoothingEnabled = false; // Pixel-perfect sharp rendering
    ctx.drawImage(img, sx, sy, fw, fh, dx, dy + breathe, dw, dh);

    // Blushing cheek effect during petting
    if (state.pettingLevel > 0) {
      const blushAlpha = Math.min(0.65, state.pettingLevel / 40);
      ctx.fillStyle = `rgba(255, 110, 150, ${blushAlpha})`;
      const hx = CAT_CENTER_X;
      const hy = profile.headCenterY || 180;
      ctx.beginPath();
      ctx.ellipse(hx - 28, hy, 10, 6, 0, 0, Math.PI * 2);
      ctx.ellipse(hx + 28, hy, 10, 6, 0, 0, Math.PI * 2);
      ctx.fill();
    }

    ctx.restore();
  }

  // 2. Desk: only a soft contact shadow, so the character stays the subject.
  function drawDeskSurface(ctx) {
    ctx.save();
    const g = ctx.createRadialGradient(CANVAS_WIDTH / 2, 342, 8, CANVAS_WIDTH / 2, 342, 150);
    g.addColorStop(0, 'rgba(0, 0, 0, 0.30)');
    g.addColorStop(1, 'rgba(0, 0, 0, 0)');
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.ellipse(CANVAS_WIDTH / 2, 342, 150, 15, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
  }

  // 3. Low-profile keyboard: a real key grid without legends (paws rest on it,
  //    so printed labels only ever showed up half-covered).
  const KB = { x: 32, y: 274, w: 166, h: 60, cols: 10, rows: 3, pad: 7, gap: 3, keyH: 9.5 };
  const KEY_COUNT = KB.cols * KB.rows + 1; // + space bar
  const KEY_IDLE = '#4a4e58';
  const KEY_LIT = '#9cc0ff';

  function mixHex(a, b, t) {
    const pa = parseInt(a.slice(1), 16), pb = parseInt(b.slice(1), 16);
    const ch = (s) => Math.round(((pa >> s) & 255) + ((((pb >> s) & 255) - ((pa >> s) & 255)) * t));
    return `rgb(${ch(16)}, ${ch(8)}, ${ch(0)})`;
  }

  function keyRect(i) {
    const kw = (KB.w - KB.pad * 2 - KB.gap * (KB.cols - 1)) / KB.cols;
    if (i === KEY_COUNT - 1) {
      const w = kw * 5 + KB.gap * 4;
      return { x: KB.x + (KB.w - w) / 2, y: KB.y + KB.pad + 3 * (KB.keyH + KB.gap), w, h: KB.keyH };
    }
    const r = Math.floor(i / KB.cols), c = i % KB.cols;
    return { x: KB.x + KB.pad + c * (kw + KB.gap), y: KB.y + KB.pad + r * (KB.keyH + KB.gap), w: kw, h: KB.keyH };
  }

  // Left-hand keys light under the left paw, right-hand keys under the right one.
  function lightKey(side) {
    if (side === 'left' && Math.random() < 0.18) { state.keyGlows[KEY_COUNT - 1] = 12; return; }
    const r = Math.floor(Math.random() * KB.rows);
    const c = side === 'left' ? Math.floor(Math.random() * 5) : 5 + Math.floor(Math.random() * 5);
    state.keyGlows[r * KB.cols + c] = 12;
  }

  function drawKeyboard(ctx) {
    ctx.save();
    ctx.shadowColor = 'rgba(0, 0, 0, 0.35)';
    ctx.shadowBlur = 10;
    ctx.shadowOffsetY = 4;
    const body = ctx.createLinearGradient(0, KB.y, 0, KB.y + KB.h);
    body.addColorStop(0, '#34373f');
    body.addColorStop(1, '#25272d');
    ctx.fillStyle = body;
    ctx.beginPath();
    ctx.roundRect(KB.x, KB.y, KB.w, KB.h, 9);
    ctx.fill();
    ctx.shadowColor = 'transparent';
    ctx.shadowBlur = 0;
    ctx.shadowOffsetY = 0;
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.09)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.roundRect(KB.x + 0.5, KB.y + 0.5, KB.w - 1, KB.h - 1, 8.5);
    ctx.stroke();

    for (let i = 0; i < KEY_COUNT; i++) {
      const k = keyRect(i);
      const glow = Math.min(1, (state.keyGlows[i] || 0) / 10);
      const y = k.y + glow * 1.2;
      ctx.fillStyle = 'rgba(0, 0, 0, 0.35)';
      ctx.beginPath();
      ctx.roundRect(k.x, k.y + 1.5, k.w, k.h, 2.5);
      ctx.fill();
      if (glow > 0) {
        ctx.shadowColor = 'rgba(138, 180, 255, 0.9)';
        ctx.shadowBlur = 8 * glow;
      }
      ctx.fillStyle = glow > 0 ? mixHex(KEY_IDLE, KEY_LIT, glow) : KEY_IDLE;
      ctx.beginPath();
      ctx.roundRect(k.x, y, k.w, k.h, 2.5);
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.fillStyle = 'rgba(255, 255, 255, 0.07)';
      ctx.fillRect(k.x + 1.5, y + 1, k.w - 3, 1);
    }
    ctx.restore();
  }

  // 4. Mouse: one smooth pebble with a quiet status light.
  function drawMouse(ctx) {
    ctx.save();
    const mx = 246, my = 304;
    const down = state.rightDown || state.mouseDown;
    ctx.shadowColor = 'rgba(0, 0, 0, 0.35)';
    ctx.shadowBlur = 10;
    ctx.shadowOffsetY = 4;
    const body = ctx.createLinearGradient(mx - 18, my - 26, mx + 18, my + 26);
    body.addColorStop(0, '#3a3d45');
    body.addColorStop(1, '#24262c');
    ctx.fillStyle = body;
    ctx.beginPath();
    ctx.ellipse(mx, my, 18, 26, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.shadowColor = 'transparent';
    ctx.shadowBlur = 0;
    ctx.shadowOffsetY = 0;
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.09)';
    ctx.lineWidth = 1;
    ctx.stroke();

    ctx.strokeStyle = 'rgba(0, 0, 0, 0.45)';
    ctx.beginPath();
    ctx.moveTo(mx, my - 26);
    ctx.lineTo(mx, my - 6);
    ctx.stroke();
    ctx.fillStyle = down ? KEY_LIT : '#5a5e68';
    ctx.beginPath();
    ctx.roundRect(mx - 2, my - 19, 4, 9, 2);
    ctx.fill();

    const pulse = down ? 1 : 0.35 + 0.15 * Math.sin(performance.now() / 900);
    ctx.strokeStyle = `rgba(138, 180, 255, ${pulse})`;
    ctx.lineWidth = 1.6;
    if (down) {
      ctx.shadowColor = 'rgba(138, 180, 255, 0.9)';
      ctx.shadowBlur = 8;
    }
    ctx.beginPath();
    ctx.arc(mx, my + 2, 13, 0.25 * Math.PI, 0.75 * Math.PI);
    ctx.stroke();
    ctx.restore();
  }

  // 5. Classic Bongo Drums (Bongo Mode)
  function drawBongos(ctx) {
    ctx.save();

    function drawDrum(x, y, radiusX, radiusY, angle, isHit) {
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);

      // Drum body
      const bodyGrad = ctx.createLinearGradient(-radiusX, 0, radiusX, 45);
      bodyGrad.addColorStop(0, '#8c4819');
      bodyGrad.addColorStop(0.5, '#b86221');
      bodyGrad.addColorStop(1, '#5e2b08');

      ctx.fillStyle = bodyGrad;
      ctx.strokeStyle = '#2d1504';
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.moveTo(-radiusX, 0);
      ctx.lineTo(-radiusX + 8, 48);
      ctx.quadraticCurveTo(0, 56, radiusX - 8, 48);
      ctx.lineTo(radiusX, 0);
      ctx.closePath();
      ctx.fill();
      ctx.stroke();

      // Steel tuning rings
      ctx.strokeStyle = '#bdc3c7';
      ctx.lineWidth = 2.5;
      ctx.beginPath();
      ctx.ellipse(0, 16, radiusX - 3, radiusY - 1, 0, 0, Math.PI * 2);
      ctx.stroke();

      // Drum skin top
      ctx.fillStyle = isHit ? '#fffbf0' : '#fdf6e2';
      ctx.strokeStyle = '#2d1504';
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.ellipse(0, 0, radiusX, radiusY, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();

      // Hit depression ring
      if (isHit) {
        ctx.strokeStyle = 'rgba(230, 126, 34, 0.7)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.ellipse(0, 0, radiusX * 0.65, radiusY * 0.65, 0, 0, Math.PI * 2);
        ctx.stroke();
      }

      ctx.restore();
    }

    // Desk surface beneath bongos
    ctx.save();
    const deskGrad = ctx.createLinearGradient(0, 260, 0, 355);
    deskGrad.addColorStop(0, '#242831');
    deskGrad.addColorStop(1, '#1b1d24');
    ctx.fillStyle = deskGrad;
    ctx.strokeStyle = '#3d4452';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(25, 262);
    ctx.lineTo(295, 262);
    ctx.quadraticCurveTo(312, 262, 312, 280);
    ctx.lineTo(312, 345);
    ctx.quadraticCurveTo(310, 355, 290, 355);
    ctx.lineTo(30, 355);
    ctx.quadraticCurveTo(10, 355, 8, 345);
    ctx.lineTo(8, 280);
    ctx.quadraticCurveTo(10, 262, 25, 262);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    ctx.restore();

    // Center connecting wooden bracket
    ctx.fillStyle = '#4a2206';
    ctx.strokeStyle = '#2d1504';
    ctx.lineWidth = 2.5;
    ctx.beginPath();
    ctx.roundRect(130, 274, 60, 24, 4);
    ctx.fill();
    ctx.stroke();

    // Steel bracket plate
    ctx.fillStyle = '#95a5a6';
    ctx.beginPath();
    ctx.roundRect(138, 281, 44, 9, 2);
    ctx.fill();

    // Left Bongo
    drawDrum(95, 275, 38, 18, 0.12, state.leftDown);

    // Right Bongo
    drawDrum(225, 275, 42, 19, -0.12, state.rightDown);

    // Shockwave expansion rings
    for (let i = state.drumRings.length - 1; i >= 0; i--) {
      const ring = state.drumRings[i];
      ctx.strokeStyle = ring.color;
      ctx.globalAlpha = ring.alpha;
      ctx.lineWidth = 2.2;
      ctx.beginPath();
      ctx.ellipse(ring.x, ring.y, ring.r, ring.r * 0.45, 0, 0, Math.PI * 2);
      ctx.stroke();
      ctx.globalAlpha = 1.0;

      ring.r += 2.2;
      ring.alpha -= 0.04;
      if (ring.alpha <= 0) state.drumRings.splice(i, 1);
    }

    ctx.restore();
  }

  // 6. Character-Adaptive Paws
  function drawPaws(ctx) {
    ctx.save();
    const profile = CHARACTER_PROFILES[state.character] || CHARACTER_PROFILES.mochi;
    const style = profile.pawStyle;

    ctx.fillStyle = style.fill;
    ctx.strokeStyle = style.sleeveFill || style.stroke;
    ctx.lineWidth = style.armWidth || 7;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';

    // --- Left Paw ---
    ctx.save();
    const lpx = 95;
    const lpy = state.pawLeftY;
    const lsquish = state.pawLeftSquish;

    ctx.translate(lpx, lpy);
    ctx.scale(2.0 - lsquish, lsquish);

    // Left Arm stroke originating from character's side (115, 220)
    ctx.beginPath();
    ctx.moveTo(25, -lpy + 215);
    ctx.quadraticCurveTo(8, -20, 0, 0);
    ctx.stroke();

    // Left Paw Head / Gauntlet
    drawPawHead(ctx, style, -0.15);
    ctx.restore();

    // --- Right Paw ---
    ctx.save();
    const rpx = state.mode === 'keyboard' ? 230 : 225;
    const rpy = state.pawRightY;
    const rsquish = state.pawRightSquish;

    ctx.translate(rpx, rpy);
    ctx.scale(2.0 - rsquish, rsquish);

    // Right Arm stroke originating from character's side (205, 220)
    ctx.beginPath();
    ctx.moveTo(-25, -rpy + 215);
    ctx.quadraticCurveTo(-8, -20, 0, 0);
    ctx.stroke();

    // Right Paw Head / Gauntlet
    drawPawHead(ctx, style, 0.15);
    ctx.restore();

    ctx.restore();
  }

  function drawPawHead(ctx, style, rotation) {
    ctx.save();
    ctx.rotate(rotation);

    if (style.type === 'knight') {
      // Blue armor gauntlet plate
      ctx.fillStyle = style.fill;
      ctx.strokeStyle = style.stroke;
      ctx.lineWidth = 2.5;

      ctx.beginPath();
      ctx.roundRect(-16, -12, 32, 22, 6);
      ctx.fill();
      ctx.stroke();

      // Gold trim rim
      ctx.fillStyle = style.padFill;
      ctx.beginPath();
      ctx.roundRect(-12, -4, 24, 6, 2);
      ctx.fill();

      // Rivets
      ctx.fillStyle = '#f39c12';
      ctx.beginPath();
      ctx.arc(-8, 5, 1.8, 0, Math.PI * 2);
      ctx.arc(8, 5, 1.8, 0, Math.PI * 2);
      ctx.fill();

    } else if (style.type === 'sprout') {
      // Cute plant sprout bulb
      ctx.fillStyle = style.fill;
      ctx.strokeStyle = style.stroke;
      ctx.lineWidth = 2.5;

      ctx.beginPath();
      ctx.ellipse(0, 0, 18, 14, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();

      // Leaf accent
      ctx.fillStyle = style.padFill;
      ctx.beginPath();
      ctx.ellipse(0, -2, 7, 4, 0, 0, Math.PI * 2);
      ctx.fill();

    } else if (style.type === 'human') {
      // Dark hoodie sleeve cuff
      ctx.fillStyle = style.sleeveFill;
      ctx.strokeStyle = style.stroke;
      ctx.lineWidth = 2.2;
      ctx.beginPath();
      ctx.roundRect(-16, -15, 32, 12, 4);
      ctx.fill();
      ctx.stroke();

      // Cyber cyan cuff trim stripe
      ctx.fillStyle = style.padFill;
      ctx.beginPath();
      ctx.roundRect(-14, -6, 28, 3.5, 1.5);
      ctx.fill();

      // Hand
      ctx.fillStyle = style.fill;
      ctx.strokeStyle = style.stroke;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.ellipse(0, 5, 12, 9, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();

    } else if (style.type === 'capy') {
      // Rounded cozy capybara paw with gentle claws
      ctx.fillStyle = style.fill;
      ctx.strokeStyle = style.stroke;
      ctx.lineWidth = 2.5;

      ctx.beginPath();
      ctx.ellipse(0, 0, 18, 14, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();

      // Cocoa capy pads
      ctx.fillStyle = style.padFill;
      ctx.beginPath();
      ctx.ellipse(-6, -4, 2.8, 3.8, -0.2, 0, Math.PI * 2);
      ctx.ellipse(0, -6, 3.0, 4.0, 0, 0, Math.PI * 2);
      ctx.ellipse(6, -4, 2.8, 3.8, 0.2, 0, Math.PI * 2);
      ctx.ellipse(0, 3, 7, 4.5, 0, 0, Math.PI * 2);
      ctx.fill();

    } else if (style.type === 'cow') {
      // Rounded golden calf hoof with cloven notch & pink pad
      ctx.fillStyle = style.fill;
      ctx.strokeStyle = style.stroke;
      ctx.lineWidth = 2.5;

      ctx.beginPath();
      ctx.ellipse(0, 0, 18, 14, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();

      // Cloven center groove
      ctx.strokeStyle = style.stroke;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(0, -14);
      ctx.lineTo(0, -2);
      ctx.stroke();

      // Sweet pink heart-shaped hoof pad
      ctx.fillStyle = style.padFill;
      ctx.beginPath();
      ctx.ellipse(-5, 0, 4, 5, -0.2, 0, Math.PI * 2);
      ctx.ellipse(5, 0, 4, 5, 0.2, 0, Math.PI * 2);
      ctx.fill();

    } else {
      // Classic Cute Cat / Fox Paw with Toe Beans!
      ctx.fillStyle = style.fill;
      ctx.strokeStyle = style.stroke;
      ctx.lineWidth = 2.5;

      ctx.beginPath();
      ctx.ellipse(0, 0, 19, 14, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();

      // Toe Beans
      ctx.fillStyle = style.padFill;
      ctx.beginPath();
      ctx.ellipse(-7, -4, 3.2, 4.0, -0.2, 0, Math.PI * 2);
      ctx.ellipse(0, -7, 3.4, 4.3, 0, 0, Math.PI * 2);
      ctx.ellipse(7, -4, 3.2, 4.0, 0.2, 0, Math.PI * 2);
      ctx.ellipse(0, 2, 6, 4.2, 0, 0, Math.PI * 2);
      ctx.fill();
    }

    ctx.restore();
  }

  // 7. Particles (Hearts, Sparks)
  function drawParticles(ctx) {
    ctx.save();

    // Sparks
    for (let i = state.sparks.length - 1; i >= 0; i--) {
      const sp = state.sparks[i];
      ctx.fillStyle = sp.color;
      ctx.globalAlpha = sp.alpha;
      ctx.beginPath();
      ctx.arc(sp.x, sp.y, 2.5, 0, Math.PI * 2);
      ctx.fill();

      sp.x += sp.vx;
      sp.y += sp.vy;
      sp.alpha -= 0.06;
      if (sp.alpha <= 0) state.sparks.splice(i, 1);
    }

    // Dozing z's
    ctx.font = '600 12px -apple-system, "Segoe UI", sans-serif';
    ctx.textAlign = 'left';
    for (let i = life.zz.length - 1; i >= 0; i--) {
      const z = life.zz[i];
      ctx.globalAlpha = z.alpha;
      ctx.fillStyle = '#c9d6ff';
      ctx.font = `600 ${z.size}px -apple-system, "Segoe UI", sans-serif`;
      ctx.fillText('z', z.x, z.y);
      z.x += 0.25; z.y -= 0.35; z.size += 0.04; z.alpha -= 0.006;
      if (z.alpha <= 0) life.zz.splice(i, 1);
    }

    // Typing combo: a small pill that pops on each hit and fades when typing stops.
    const comboAge = performance.now() - life.comboAt;
    if (life.combo >= 8 && comboAge < 1400) {
      ctx.globalAlpha = Math.min(1, (1400 - comboAge) / 400);
      const label = String(life.combo);
      ctx.font = '600 11px -apple-system, "Segoe UI", sans-serif';
      const w = ctx.measureText(label).width + 34;
      const x = 286 - w, y = 62, s = 1 + life.comboPop * 0.12;
      ctx.save();
      ctx.translate(x + w / 2, y + 10);
      ctx.scale(s, s);
      ctx.fillStyle = 'rgba(28, 30, 36, 0.88)';
      ctx.beginPath();
      ctx.roundRect(-w / 2, -10, w, 20, 10);
      ctx.fill();
      ctx.fillStyle = '#8b8f99';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      ctx.fillText('连击', -w / 2 + 9, 0.5);
      ctx.fillStyle = '#9cc0ff';
      ctx.textAlign = 'right';
      ctx.fillText(label, w / 2 - 9, 0.5);
      ctx.restore();
    }
    ctx.globalAlpha = 1;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'alphabetic';

    // Hearts
    ctx.font = 'bold 20px -apple-system, sans-serif';
    for (let i = state.hearts.length - 1; i >= 0; i--) {
      const h = state.hearts[i];
      ctx.fillStyle = h.color;
      ctx.globalAlpha = h.alpha;
      ctx.save();
      ctx.translate(h.x, h.y);
      ctx.scale(h.scale, h.scale);
      ctx.fillText('♥', 0, 0);
      ctx.restore();

      h.x += h.vx;
      h.y += h.vy;
      h.alpha -= 0.022;
      h.scale = Math.min(1.4, h.scale + 0.015);
      if (h.alpha <= 0) state.hearts.splice(i, 1);
    }

    ctx.globalAlpha = 1.0;
    ctx.restore();
  }

  // --- Main Animation & Physics Loop ---
  function updatePhysics() {
    // Spring physics on Left Paw
    state.pawLeftTargetY = state.leftDown ? 282 : 245;
    state.pawLeftY += (state.pawLeftTargetY - state.pawLeftY) * 0.45;
    state.pawLeftSquish += (1.0 - state.pawLeftSquish) * 0.35;

    // Spring physics on Right Paw
    state.pawRightTargetY = (state.rightDown || state.mouseDown) ? 282 : 245;
    state.pawRightY += (state.pawRightTargetY - state.pawRightY) * 0.45;
    state.pawRightSquish += (1.0 - state.pawRightSquish) * 0.35;

    // Petting level decay
    if (state.pettingLevel > 0) state.pettingLevel--;

    const nowMs = performance.now();
    if (isSleeping(nowMs) && nowMs - life.zzAt > 1400) {
      life.zzAt = nowMs;
      const hy = (CHARACTER_PROFILES[state.character] || CHARACTER_PROFILES.mochi).headCenterY || 180;
      life.zz.push({ x: CAT_CENTER_X + 34, y: hy - 40, alpha: 0.9, size: 11 });
    }
    life.comboPop = Math.max(0, life.comboPop - 0.08);

    // Key glow decay
    for (let i = 0; i < state.keyGlows.length; i++) {
      if (state.keyGlows[i] > 0) state.keyGlows[i]--;
    }

    // BPM decay
    if (performance.now() - state.lastTapTime > 600) {
      state.typingBpm = Math.max(0, state.typingBpm - 2);
    }
  }

  let renderErrors = 0;
  function render() {
    try {
      updatePhysics();

      const dpr = window.devicePixelRatio || 1;
      ctx.save();
      // Identity clear across total canvas buffer
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // Scaled & centered transform
      ctx.setTransform(
        dpr * currentScale, 0,
        0, dpr * currentScale,
        dpr * currentOffsetX, dpr * currentOffsetY
      );

      // 1. Draw Character (Sprite sheet frame or Live2D)
      drawCharacter(ctx);

      // 2. Draw Desk & Instruments (if desk is enabled)
      if (state.showDesk !== false) {
        if (state.mode === 'bongos') {
          drawBongos(ctx);
        } else {
          drawDeskSurface(ctx);
          drawKeyboard(ctx);
          drawMouse(ctx);
        }

        // 3. Draw Character-Specific Animated Paws
        drawPaws(ctx);
      }

      // 4. Draw Interactive Particles & Glows
      drawParticles(ctx);

      ctx.restore();
    } catch (e) {
      if (renderErrors < 5) {
        renderErrors++;
        console.error('[BongoPet] Render error:', e);
      }
    }
    requestAnimationFrame(render);
  }

  // --- Interaction Regions for Rust Click-Through ---
  function syncInteractionRegions() {
    const rects = [
      {
        x: Math.round(currentOffsetX + 20 * currentScale),
        y: Math.round(currentOffsetY + 40 * currentScale),
        width: Math.round(280 * currentScale),
        height: Math.round(340 * currentScale)
      }
    ];
    window.hermesDesktop?.petOverlay?.control?.({
      type: 'interaction-regions',
      rects
    });
  }

  // Start Animation Loop
  requestAnimationFrame(render);
  setInterval(syncInteractionRegions, 1000);
  syncInteractionRegions();

  // Signal Host that Renderer is Ready
  try {
    window.hermesDesktop?.petOverlay?.control?.({
      type: 'renderer-ready',
      renderer: 'hermes-sprite'
    });
    window.hermesDesktop?.petOverlay?.control?.({ type: 'ready' });
  } catch (_) {}

  console.log('[BongoPet] Multi-Character Bongo Pet Engine active');
})();
