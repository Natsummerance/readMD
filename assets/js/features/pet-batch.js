'use strict';
/**
 * ReadMD Desktop Pet & Reading Companion
 * Adheres to: /apple-design, /design-taste-frontend, /frontend-design
 * Supports direct pointer manipulation (setPointerCapture, grab offset),
 * file drag-and-drop directly onto pet for batch convert,
 * reading progress observer with gentle encouraging bubbles,
 * and dual-channel pywebview bridge + HTTP REST API.
 */

const petBatchInbox = [];
let petBatchConfirming = false;
let activePetSettingsStatus = null;

const petT = (key, params, fallback = '') => {
  if (!window.i18n) return fallback || '';
  const value = window.i18n.t(key, params);
  return value && value !== key ? value : (fallback || '');
};

function petPercent(value, fallback) {
  const number = Number(value);
  return Math.round((Number.isFinite(number) ? number : fallback) * 100);
}

// --------------------------------------------------------------------------
// Dual-Channel API (Native Pywebview Bridge + HTTP REST Fallback)
// --------------------------------------------------------------------------

async function fetchPetRuntimeStatus() {
  if (window.hasPy && window.py && typeof window.py.get_pet_runtime_status === 'function') {
    try {
      return await window.py.get_pet_runtime_status();
    } catch (_err) { /* fallback to HTTP */ }
  }
  try {
    const res = await (typeof apiFetch === 'function' ? apiFetch('/api/pets/status') : fetch('/api/pets/status'));
    if (res && res.ok) {
      const payload = await res.json();
      return (payload && payload.status) ? payload.status : payload;
    }
  } catch (_err) { /* offline or mock */ }

  return {
    adapter: { available: false, name: 'Hermes Pet Adapter' },
    active_pet: 'hermes-sprite',
    active_slug: 'Hermes',
    enabled: false,
    installed: false,
    in_app: true,
    preferences: { renderer: 'hermes-sprite', scale: 0.33, opacity: 1.0 },
    running: false
  };
}

async function requestConfigurePet(config) {
  if (window.hasPy && window.py && typeof window.py.configure_pet === 'function') {
    try {
      return await window.py.configure_pet(config);
    } catch (_err) { /* fallback to HTTP */ }
  }
  try {
    const res = await (typeof apiFetch === 'function' ? apiFetch('/api/pets/configure', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(config)
    }) : fetch('/api/pets/configure', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(config)
    }));
    if (res && res.ok) {
      return await res.json();
    }
  } catch (_err) { /* ignore */ }

  return { ok: false, code: 'pet_connection_failed' };
}

// --------------------------------------------------------------------------
// File Batch Drop & Process Bridge
// --------------------------------------------------------------------------

let petDropQueue = [];
let petDropProcessing = false;

async function handlePetDroppedFiles(rawItems) {
  if (!rawItems || !rawItems.length) return;
  const paths = [];
  for (const item of rawItems) {
    if (typeof item === 'string' && item) {
      paths.push(item);
    } else if (item && typeof item === 'object') {
      const p = item.path ? item.path : (typeof uploadFile === 'function' ? await uploadFile(item) : null);
      if (p) paths.push(p);
    }
  }
  if (!paths.length) return;

  const isConvert = (p) => {
    const isBin = typeof CONVERT_BINARY_RE !== 'undefined' ? CONVERT_BINARY_RE.test(p) : /\.(docx?|pptx?|xlsx?|pdf|epub|mobi|rtf|odt)$/i.test(p);
    const isImg = typeof IMG_RE !== 'undefined' ? IMG_RE.test(p) : /\.(png|jpe?g|bmp|webp|gif|tiff?)$/i.test(p);
    return isBin || isImg;
  };

  const convertFiles = paths.filter(isConvert);
  const textFiles = paths.filter(p => !isConvert(p));

  // 1. 单个 Markdown / 纯文本 / 代码文件 -> 直接在阅读器中加载打开
  if (textFiles.length === 1 && convertFiles.length === 0) {
    const filePath = textFiles[0];
    const fileName = filePath.split(/[/\\]/).pop() || filePath;
    showPetBubble(petT('pet.bubbleOpening', { name: fileName }, `正在为你打开 ${fileName}...`), 3000, PET_BUBBLE_PRIORITY.CRITICAL);
    if (typeof loadFile === 'function') {
      await loadFile(filePath, { force: true, browserCopy: false });
      showPetBubble(petT('pet.bubbleOpened', { name: fileName }, `${fileName} 已打开！`), 3500, PET_BUBBLE_PRIORITY.CRITICAL);
    }
    return;
  }

  // 2. 单个需转换/OCR文档（PDF, Word, 图片等） -> 自动启动转换并直接打开阅读
  if (convertFiles.length === 1 && textFiles.length === 0) {
    const filePath = convertFiles[0];
    const fileName = filePath.split(/[/\\]/).pop() || filePath;
    showPetBubble(petT('pet.bubbleConverting', { name: fileName }, `正在为你转换并打开 ${fileName}...`), 4000, PET_BUBBLE_PRIORITY.CRITICAL);
    if (typeof convertOrOcr === 'function') {
      convertOrOcr(filePath, 'convert');
    } else if (typeof enqueueBatchFiles === 'function') {
      await enqueueBatchFiles([filePath], false);
    }
    return;
  }

  // 3. 多个纯文本 / Markdown 文件 -> 多标签页依次直接打开
  if (textFiles.length > 1 && convertFiles.length === 0) {
    showPetBubble(petT('pet.bubbleOpeningMulti', { count: textFiles.length }, `正在为你打开 ${textFiles.length} 篇文档...`), 3500, PET_BUBBLE_PRIORITY.CRITICAL);
    if (typeof loadFile === 'function') {
      for (const p of textFiles) {
        await loadFile(p);
      }
      showPetBubble(petT('pet.bubbleOpenedMulti', { count: textFiles.length }, `已全部打开 ${textFiles.length} 篇文档！`), 3500, PET_BUBBLE_PRIORITY.CRITICAL);
    }
    return;
  }

  // 4. 包含需转换的多个文档或混合文档：文本文件先打开，转换文件进批量工作台
  if (textFiles.length > 0 && typeof loadFile === 'function') {
    for (const p of textFiles) {
      await loadFile(p);
    }
  }

  if (convertFiles.length > 0 && typeof enqueueBatchFiles === 'function') {
    showPetBubble(petT('pet.bubbleBatchConverting', { count: convertFiles.length }, `已将 ${convertFiles.length} 个文档加入批量转换工作台`), 4000, PET_BUBBLE_PRIORITY.CRITICAL);
    await enqueueBatchFiles(convertFiles, false);
  }
}

async function receivePetBatch(paths) {
  const safePaths = (paths || []).filter(path => typeof path === 'string' && path);
  if (!safePaths.length) return;
  petDropQueue.push(safePaths);
  if (petDropProcessing) return;
  petDropProcessing = true;
  try {
    while (petDropQueue.length) {
      const next = petDropQueue.shift();
      try {
        await handlePetDroppedFiles(next);
      } catch (batchErr) {
        console.error('Failed to process dropped batch:', batchErr);
        if (typeof showPetBubble === 'function') {
          showPetBubble(petT('pet.bubbleBatchFailed', {}, '部分文件处理失败'), 4000, PET_BUBBLE_PRIORITY.CRITICAL);
        }
      }
    }
  } finally {
    petDropProcessing = false;
  }
}

window.handlePetDroppedFiles = handlePetDroppedFiles;
window.receivePetBatch = receivePetBatch;

// --------------------------------------------------------------------------
// Speech Bubble & Interaction (Ported from stevenjoezhang/live2d-widget Priority Queue)
// --------------------------------------------------------------------------

const PET_BUBBLE_PRIORITY = {
  LOW_IDLE: 1,       // 问候、发呆、时段提醒
  INTERACTION: 2,    // 点击互动、戳一戳反馈
  MILESTONE: 3,      // 25%、50%、80%、100% 伴读进度达成
  CRITICAL: 4        // 文件拖拽导入、错误、系统配置变更
};

let currentBubblePriority = 0;
let petBubbleTimer = null;
let petLastReadingMilestone = 0;
let petPokeComboCount = 0;
let petLastPokeTime = 0;

/**
 * 优先级气泡管理器：高优先级气泡展示期间，低优先级消息不可抢占
 * 移植自 stevenjoezhang/live2d-widget (10.9k stars) message.ts 调度逻辑
 */
function showPetBubble(text, durationMs = 4500, priority = PET_BUBBLE_PRIORITY.LOW_IDLE) {
  const bubble = $('pet-bubble');
  const bubbleText = $('pet-bubble-text');
  if (!bubble || !bubbleText || !text) return;

  // 优先级互斥：低优先级不得打断高优先级
  if (priority < currentBubblePriority) {
    return;
  }

  if (petBubbleTimer) {
    clearTimeout(petBubbleTimer);
    petBubbleTimer = null;
  }

  currentBubblePriority = priority;
  bubbleText.textContent = text;
  bubble.classList.add('is-visible');

  if (durationMs > 0) {
    petBubbleTimer = setTimeout(() => {
      bubble.classList.remove('is-visible');
      petBubbleTimer = null;
      currentBubblePriority = 0;
    }, durationMs);
  }
}

window.showPetBubble = showPetBubble;

function hidePetBubble() {
  const bubble = $('pet-bubble');
  if (bubble) bubble.classList.remove('is-visible');
  if (petBubbleTimer) {
    clearTimeout(petBubbleTimer);
    petBubbleTimer = null;
  }
  currentBubblePriority = 0;
}

/**
 * 时段情境问候系统 (Ported from stevenjoezhang/live2d-widget)
 */
function getContextualGreeting() {
  const hour = new Date().getHours();
  if (hour >= 5 && hour < 9) {
    return petT('pet.greetingEarlyMorning') || '一日之计在于晨，今天也要元气满满地阅读哦！☀️';
  } else if (hour >= 9 && hour < 12) {
    return petT('pet.greetingMorning') || '上午专注时光，静心阅读效率更高呢~ ☕';
  } else if (hour >= 12 && hour < 14) {
    return petT('pet.greetingNoon') || '午后小憩片刻，看书也要注意劳逸结合呀 🥪';
  } else if (hour >= 14 && hour < 18) {
    return petT('pet.greetingAfternoon') || '下午好！一杯清茶，一本好书，继续探索新知吧 🍵';
  } else if (hour >= 18 && hour < 22) {
    return petT('pet.greetingEvening') || '晚上好！今晚的阅读清单完成得怎么样了？✨';
  } else {
    return petT('pet.greetingNight') || '夜深了，注意保护视力，早点休息不要太辛苦啦 🌙';
  }
}

// --------------------------------------------------------------------------
// In-App Widget State & Direct Manipulation Dragging
// --------------------------------------------------------------------------

function applyWidgetAppearance(scaleFraction, opacityFraction) {
  const widget = $('readmd-pet-widget');
  const character = $('pet-character');
  const previewChar = $('pet-preview-character');
  if (!widget) return;

  const scale = Number.isFinite(scaleFraction) ? scaleFraction : 0.33;
  const opacity = Number.isFinite(opacityFraction)
    ? Math.max(0.1, Math.min(1.0, opacityFraction))
    : 1.0;

  // Scale map: 0.18 -> ~0.7, 0.33 -> 1.0, 0.72 -> ~1.4
  const displayScale = Math.max(0.6, Math.min(1.6, scale * 3.0));

  if (character) {
    character.style.transform = `scale(${displayScale})`;
    character.style.opacity = String(opacity);
  }
  if (previewChar) {
    previewChar.style.transform = `scale(${displayScale})`;
    previewChar.style.opacity = String(opacity);
  }
}

function syncPetWidgetVisibility(status) {
  const widget = $('readmd-pet-widget');
  if (!widget) return;
  const enabled = Boolean(status && status.enabled && status.in_app !== false);
  if (enabled) {
    widget.classList.remove('hidden');
    const prefs = (status && status.preferences) || {};
    applyWidgetAppearance(prefs.scale, prefs.opacity);
    restoreWidgetPosition();
  } else {
    widget.classList.add('hidden');
    hidePetBubble();
  }
}

function restoreWidgetPosition() {
  const widget = $('readmd-pet-widget');
  if (!widget) return;

  const rect = widget.getBoundingClientRect();
  const width = rect && rect.width > 0 ? rect.width : 120;
  const height = rect && rect.height > 0 ? rect.height : 150;
  const maxX = Math.max(12, window.innerWidth - width - 12);
  const maxY = Math.max(48, window.innerHeight - height - 12);

  try {
    const saved = localStorage.getItem('readmd_pet_pos');
    if (saved) {
      const pos = JSON.parse(saved);
      if (pos && typeof pos.left === 'number' && typeof pos.top === 'number') {
        const clampedX = Math.max(12, Math.min(maxX, pos.left));
        const clampedY = Math.max(48, Math.min(maxY, pos.top));
        widget.style.left = `${clampedX}px`;
        widget.style.top = `${clampedY}px`;
        widget.style.right = 'auto';
        widget.style.bottom = 'auto';
        return;
      }
    }
  } catch (_e) { /* ignore */ }

  if (widget.style.left && widget.style.left !== 'auto') {
    const currentLeft = parseFloat(widget.style.left);
    const currentTop = parseFloat(widget.style.top);
    if (Number.isFinite(currentLeft) && Number.isFinite(currentTop)) {
      const clampedX = Math.max(12, Math.min(maxX, currentLeft));
      const clampedY = Math.max(48, Math.min(maxY, currentTop));
      widget.style.left = `${clampedX}px`;
      widget.style.top = `${clampedY}px`;
      return;
    }
  }

  widget.style.left = '';
  widget.style.top = '';
  widget.style.right = '28px';
  widget.style.bottom = '32px';
}

function resetWidgetPosition() {
  try {
    localStorage.removeItem('readmd_pet_pos');
  } catch (_e) { /* ignore */ }
  restoreWidgetPosition();
  if (typeof showToast === 'function') {
    showToast(petT('pet.resetPosDone') || '已重置桌宠位置');
  }
  showPetBubble(petT('pet.bubbleReset') || '我回到默认位置啦！', 3000);
}

function initPetDirectManipulation() {
  const widget = $('readmd-pet-widget');
  const charWrap = $('pet-character-wrap');
  const character = $('pet-character');
  if (!widget || !charWrap) return;

  let isDragging = false;
  let hasMoved = false;
  let startX = 0;
  let startY = 0;
  let grabOffsetX = 0;
  let grabOffsetY = 0;

  charWrap.addEventListener('pointerdown', (e) => {
    // Ignore clicks on quick toolbar or non-primary button
    if (e.target.closest('.pet-widget-quick-bar') || e.button !== 0) return;

    const rect = widget.getBoundingClientRect();
    startX = e.clientX;
    startY = e.clientY;
    grabOffsetX = e.clientX - rect.left;
    grabOffsetY = e.clientY - rect.top;
    isDragging = true;
    hasMoved = false;

    try {
      charWrap.setPointerCapture(e.pointerId);
    } catch (_err) { /* ignore */ }
    e.preventDefault();
  });

  charWrap.addEventListener('pointermove', (e) => {
    if (!isDragging) return;
    const dist = Math.hypot(e.clientX - startX, e.clientY - startY);
    if (dist > 4) {
      hasMoved = true;
      const rect = widget.getBoundingClientRect();
      const maxX = Math.max(12, window.innerWidth - rect.width - 12);
      const maxY = Math.max(48, window.innerHeight - rect.height - 12);
      const targetX = Math.max(12, Math.min(maxX, e.clientX - grabOffsetX));
      const targetY = Math.max(48, Math.min(maxY, e.clientY - grabOffsetY));

      widget.style.left = `${targetX}px`;
      widget.style.top = `${targetY}px`;
      widget.style.right = 'auto';
      widget.style.bottom = 'auto';
    }
  });

  const handlePointerEnd = (e) => {
    if (!isDragging) return;
    isDragging = false;
    try {
      if (charWrap.hasPointerCapture(e.pointerId)) {
        charWrap.releasePointerCapture(e.pointerId);
      }
    } catch (_err) { /* ignore */ }

    if (hasMoved) {
      const rect = widget.getBoundingClientRect();
      // 边缘平滑物理吸附 (Ported from hacxy/l2d-widget edge clamping)
      const snapThreshold = 40;
      let finalLeft = rect.left;
      if (rect.left < snapThreshold) {
        finalLeft = 12;
      } else if (window.innerWidth - (rect.left + rect.width) < snapThreshold) {
        finalLeft = window.innerWidth - rect.width - 12;
      }
      widget.style.left = `${finalLeft}px`;

      try {
        localStorage.setItem('readmd_pet_pos', JSON.stringify({ left: finalLeft, top: rect.top }));
      } catch (_err) { /* ignore */ }
    } else {
      handlePetInteractiveClick();
    }
  };

  charWrap.addEventListener('pointerup', handlePointerEnd);
  charWrap.addEventListener('pointercancel', handlePointerEnd);

  // File Drag & Drop Direct Target
  charWrap.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.stopPropagation();
    charWrap.classList.add('is-drop-target');
  });

  charWrap.addEventListener('dragleave', (e) => {
    e.preventDefault();
    e.stopPropagation();
    charWrap.classList.remove('is-drop-target');
  });

  charWrap.addEventListener('drop', async (e) => {
    e.preventDefault();
    e.stopPropagation();
    charWrap.classList.remove('is-drop-target');

    const dt = e.dataTransfer;
    if (dt && dt.files && dt.files.length) {
      await handlePetDroppedFiles(Array.from(dt.files));
    }
  });

  // Quick Buttons
  $('pet-quick-settings')?.addEventListener('click', (e) => {
    e.stopPropagation();
    openPetSettings();
  });

  $('pet-quick-hide')?.addEventListener('click', async (e) => {
    e.stopPropagation();
    const enabledInput = $('pet-enabled');
    if (enabledInput) enabledInput.checked = false;
    await savePetSettings();
  });

  // Window Resize Clamping: keep pet in visible bounds on resize
  window.addEventListener('resize', () => {
    restoreWidgetPosition();
  });
}

function handlePetInteractiveClick() {
  const char = $('pet-character');
  if (char) {
    char.classList.add('pet-bounce', 'hermes-waving');
    setTimeout(() => {
      char.classList.remove('pet-bounce', 'hermes-waving');
    }, 1200);
  }

  // 戳一戳连击检测 (Ported from clawd-on-desk poke interaction)
  const now = Date.now();
  if (now - petLastPokeTime < 1500) {
    petPokeComboCount++;
  } else {
    petPokeComboCount = 1;
  }
  petLastPokeTime = now;

  if (petPokeComboCount >= 4) {
    petPokeComboCount = 0;
    const pokeResponses = [
      petT('pet.pokeQuote1') || '哇！别戳啦别戳啦，在看书呢！🙈',
      petT('pet.pokeQuote2') || '再戳我就要变成猫咪逃走啦~ 🐾',
      petT('pet.pokeQuote3') || '哼，一直戳我，是不是想偷懒不读书了？👀',
      petT('pet.pokeQuote4') || '好啦好啦，知道你在关注我，快看正文吧！📚'
    ];
    const pokeText = pokeResponses[Math.floor(Math.random() * pokeResponses.length)];
    showPetBubble(pokeText, 3500, PET_BUBBLE_PRIORITY.INTERACTION);
    return;
  }

  // 常规互动：50% 概率触发时段问候，50% 概率触发鼓励台词
  if (Math.random() < 0.5) {
    showPetBubble(getContextualGreeting(), 4500, PET_BUBBLE_PRIORITY.INTERACTION);
    return;
  }

  const quotes = [
    petT('pet.bubbleQuote1') || '嗨！我是你的伴读伙伴，随时为你效劳~',
    petT('pet.bubbleQuote2') || '今天读书很专注哦，继续保持！✨',
    petT('pet.bubbleQuote3') || '直接拖拽 Markdown、PDF 或音视频给我，我能帮你转换哦！',
    petT('pet.bubbleQuote4') || '累了就放松一下眼睛，看看远方吧~ ☕'
  ];
  const text = quotes[Math.floor(Math.random() * quotes.length)];
  showPetBubble(text, 4500, PET_BUBBLE_PRIORITY.INTERACTION);
}

// --------------------------------------------------------------------------
// Reading Companion Progress Observer
// --------------------------------------------------------------------------

function initReadingProgressObserver() {
  let scrollThrottle = null;

  window.addEventListener('scroll', () => {
    if (scrollThrottle) return;
    scrollThrottle = setTimeout(() => {
      scrollThrottle = null;
      checkReadingProgress();
    }, 250);
  }, { passive: true });

  if ($('content')) {
    $('content').addEventListener('scroll', () => {
      if (scrollThrottle) return;
      scrollThrottle = null;
      checkReadingProgress();
    }, { passive: true });
  }
}

function checkReadingProgress() {
  const widget = $('readmd-pet-widget');
  if (!widget || widget.classList.contains('hidden')) return;

  const bubbleToggle = $('pet-bubble-toggle');
  if (bubbleToggle && !bubbleToggle.checked) return;

  const scrollEl = document.scrollingElement || document.documentElement;
  const maxScroll = scrollEl.scrollHeight - window.innerHeight;
  if (maxScroll <= 200) return;

  const progress = Math.round((scrollEl.scrollTop / maxScroll) * 100);

  if (progress >= 25 && progress < 45 && petLastReadingMilestone < 25) {
    petLastReadingMilestone = 25;
    showPetBubble(petT('pet.reading25') || '很好，已经阅读 25% 啦，保持专注！📖', 4000, PET_BUBBLE_PRIORITY.MILESTONE);
  } else if (progress >= 50 && progress < 75 && petLastReadingMilestone < 50) {
    petLastReadingMilestone = 50;
    showPetBubble(petT('pet.reading50'), 4000, PET_BUBBLE_PRIORITY.MILESTONE);
  } else if (progress >= 80 && progress < 95 && petLastReadingMilestone < 80) {
    petLastReadingMilestone = 80;
    showPetBubble(petT('pet.reading80'), 4000, PET_BUBBLE_PRIORITY.MILESTONE);
  } else if (progress >= 98 && petLastReadingMilestone < 100) {
    petLastReadingMilestone = 100;
    showPetBubble(petT('pet.reading100'), 4500, PET_BUBBLE_PRIORITY.MILESTONE);
  } else if (progress < 15) {
    petLastReadingMilestone = 0;
  }
}

// --------------------------------------------------------------------------
// Settings Modal & Preferences
// --------------------------------------------------------------------------

function setPetMenuStatus(status) {
  const label = $('pet-status-label');
  if (!label) return;
  if (status && status.enabled) label.textContent = petT('app.enabled');
  else label.textContent = petT('app.disabled');
}

async function refreshPetMenuStatus() {
  try {
    const status = await fetchPetRuntimeStatus();
    setPetMenuStatus(status);
    syncPetWidgetVisibility(status);
  } catch (_error) { /* ignore */ }
}

function renderPetSettings(status) {
  activePetSettingsStatus = status || null;
  const preferences = status && status.preferences ? status.preferences : {};
  const enabled = $('pet-enabled');
  const renderer = $('pet-renderer');
  const scale = $('pet-scale');
  const opacity = $('pet-opacity');

  if (enabled) enabled.checked = Boolean(status && status.enabled);
  if (renderer) renderer.value = preferences.renderer || 'hermes-sprite';
  if ($('pet-runtime')) $('pet-runtime').value = status?.in_app === false ? 'desktop' : 'in-app';
  if (scale) scale.value = String(petPercent(preferences.scale, 0.33));
  if (opacity) opacity.value = String(petPercent(preferences.opacity, 1.0));

  updatePetRangeLabels();

  const isInstalled = Boolean(status && (status.installed ?? status.adapter?.available));
  const installBtn = $('pet-install');
  if (installBtn) {
    installBtn.classList.remove('hidden');
    if (isInstalled) {
      installBtn.className = 'tb-btn danger';
      installBtn.textContent = petT('pet.disable');
      installBtn.dataset.action = 'uninstall';
    } else {
      installBtn.className = 'tb-btn accent';
      installBtn.textContent = petT('pet.enable');
      installBtn.dataset.action = 'install';
    }
  }

  // 动态切换舞台角色外观预览 (Hermes vs Arch-Chan)
  const currentRenderer = (renderer ? renderer.value : '') || (preferences && preferences.renderer) || 'hermes-sprite';
  updateCharacterPreview(currentRenderer);

  const statusDot = $('pet-status-dot');
  const statusText = $('pet-status-text');
  const statusLine = $('pet-status-line');

  if (statusDot && statusText) {
    statusDot.classList.remove('is-running', 'is-stopped', 'is-unavailable');
    if (status && status.enabled) {
      statusDot.classList.add('is-running');
      statusText.textContent = petT('pet.statusRunning');
    } else if (isInstalled) {
      statusDot.classList.add('is-stopped');
      statusText.textContent = petT('pet.statusStopped');
    } else {
      statusDot.classList.add('is-unavailable');
      statusText.textContent = petT('pet.statusNotInstalled');
    }
  }

  if (statusLine) {
    if (isInstalled) {
      statusLine.textContent = petT('pet.installSuccess');
    } else {
      statusLine.textContent = petT('pet.statusEnableHint');
    }
  }

  syncPetWidgetVisibility(status);
}

function updateCharacterPreview(rendererVal) {
  const charEl = document.querySelector('.pet-preview-character');
  const slugEl = $('pet-active-slug');
  const isLive2d = rendererVal === 'live2d';
  if (charEl) {
    charEl.classList.remove('is-hermes', 'is-live2d', 'is-arch-chan');
    charEl.classList.add(isLive2d ? 'is-arch-chan' : 'is-hermes');
  }
  if (slugEl) {
    slugEl.textContent = isLive2d ? petT('pet.renderer.live2d') : petT('pet.renderer.sprite');
  }
}

function updatePetRangeLabels() {
  const scale = $('pet-scale');
  const opacity = $('pet-opacity');
  const scaleVal = scale ? Number(scale.value) : 33;
  const opacityVal = opacity ? Number(opacity.value) : 100;

  if ($('pet-scale-value') && scale) $('pet-scale-value').textContent = scale.value + '%';
  if ($('pet-opacity-value') && opacity) $('pet-opacity-value').textContent = opacity.value + '%';

  applyWidgetAppearance(scaleVal / 100, opacityVal / 100);
}

function closePetSettings() {
  $('pet-settings-modal')?.classList.add('hidden');
}

async function savePetSettings() {
  const enabled = Boolean($('pet-enabled')?.checked);
  const scale = Number($('pet-scale')?.value || 33) / 100;
  const opacity = Number($('pet-opacity')?.value || 100) / 100;
  const renderer = $('pet-renderer')?.value || 'hermes-sprite';

  const isDesktopChoice = $('pet-runtime')?.value === 'desktop';
  const config = {
    enabled,
    scale,
    opacity,
    renderer,
    in_app: renderer !== 'live2d' && !isDesktopChoice
  };

  const stateChanged = Boolean(activePetSettingsStatus && activePetSettingsStatus.enabled !== enabled);
  if (enabled && !config.in_app && !activePetSettingsStatus?.adapter?.available) {
    const installed = await installDefaultPetRuntime();
    if (!installed.ok) {
      renderPetSettings(await fetchPetRuntimeStatus());
      return;
    }
  }
  const result = await requestConfigurePet(config);
  if (!result || !result.ok) {
    const code = (result && result.code) || 'unknown';
    if (typeof showToast === 'function') showToast(petT('pet.configFailed', { code }));
  } else if (stateChanged) {
    if (typeof showToast === 'function') showToast(enabled ? petT('pet.enabledToast') : petT('pet.disabledToast'));
  }

  const updatedStatus = await fetchPetRuntimeStatus();
  renderPetSettings(updatedStatus);
  setPetMenuStatus(updatedStatus);
  syncPetWidgetVisibility(updatedStatus);
}

async function openPetSettings() {
  if (typeof closeMoreMenu === 'function') closeMoreMenu();
  const modal = $('pet-settings-modal');
  if (!modal) return;
  modal.classList.remove('hidden');
  const immediateRenderer = $('pet-renderer')?.value || 'hermes-sprite';
  updateCharacterPreview(immediateRenderer);
  try {
    const status = await fetchPetRuntimeStatus();
    renderPetSettings(status);
    await refreshPetGallery();
  } catch (_error) {
    console.warn('Failed to load pet status:', _error);
  }
}

window.openPetSettings = openPetSettings;
window.closePetSettings = closePetSettings;
window.refreshPetMenuStatus = refreshPetMenuStatus;
window.fetchPetRuntimeStatus = fetchPetRuntimeStatus;
window.requestConfigurePet = requestConfigurePet;
window.syncPetWidgetVisibility = syncPetWidgetVisibility;
window.initPetDirectManipulation = initPetDirectManipulation;
window.savePetSettings = savePetSettings;
window.applyWidgetAppearance = applyWidgetAppearance;
window.restoreWidgetPosition = restoreWidgetPosition;
window.hidePetBubble = hidePetBubble;
window.PET_BUBBLE_PRIORITY = PET_BUBBLE_PRIORITY;

// --------------------------------------------------------------------------
// Background Polling & Handlers
// --------------------------------------------------------------------------

async function pollPetControls() {
  try {
    const fetchFn = typeof apiFetch === 'function' ? apiFetch : fetch;
    const [batchRes, menuRes] = await Promise.allSettled([
      fetchFn('/api/control/pet-batch'),
      fetchFn('/api/control/pet-menu')
    ]);

    if (batchRes.status === 'fulfilled' && batchRes.value && batchRes.value.ok) {
      const payload = await batchRes.value.json();
      if (payload && payload.pending) receivePetBatch(payload.paths);
    }

    if (menuRes.status === 'fulfilled' && menuRes.value && menuRes.value.ok) {
      const payload = await menuRes.value.json();
      if (payload && payload.pending) {
        const trigger = $('btn-more');
        const menu = $('more-menu');
        if (trigger && menu && !menu.classList.contains('open')) trigger.click();
      }
    }
  } catch (_error) { /* optional native bridge */ }
}

// --------------------------------------------------------------------------
// Initialization
// --------------------------------------------------------------------------

let petInitialized = false;
function initPetSystem() {
  if (petInitialized) return;
  petInitialized = true;

  refreshPetMenuStatus();
  void refreshPetGallery();
  initPetDirectManipulation();
  initReadingProgressObserver();

  $('pet-settings-close')?.addEventListener('click', closePetSettings);
  const petModal = $('pet-settings-modal');
  if (petModal) {
    petModal.addEventListener('click', e => {
      if (e.target === petModal) closePetSettings();
    });
  }
  $('pet-reset-pos')?.addEventListener('click', resetWidgetPosition);

  // 安装 / 卸载点击事件闭环 (支持 Native Pywebview 双通道与 HTTP 兜底)
  $('pet-install')?.addEventListener('click', async () => {
    const btn = $('pet-install');
    const isUninstall = btn?.dataset.action === 'uninstall';

    if (isUninstall) {
      const confirmText = petT('pet.uninstallConfirm') || '是否卸载桌宠伴侣插件？卸载后将移除本地扩展组件。';
      if (!window.confirm(confirmText)) return;
    }

    const endpoint = isUninstall ? '/api/pets/uninstall' : '/api/pets/install';
    const pendingText = isUninstall ? (petT('pet.uninstalling') || '正在卸载…') : (petT('pet.installing') || '正在安装…');
    const successText = isUninstall ? (petT('pet.uninstallSuccess') || '已卸载桌宠插件') : (petT('pet.installSuccess') || '桌宠伴读扩展已就绪');
    const failText = isUninstall ? (petT('app.failed') || '卸载失败') : (petT('app.failed') || '安装失败');

    if (btn) {
      btn.disabled = true;
      btn.textContent = pendingText;
    }
    try {
      let data = null;
      if (window.hasPy && window.py) {
        try {
          if (isUninstall && typeof window.py.uninstall_companion_pet === 'function') {
            data = await window.py.uninstall_companion_pet();
          } else if (!isUninstall && typeof window.py.install_companion_pet === 'function') {
            data = await window.py.install_companion_pet();
          }
        } catch (pyErr) {
          console.warn('Native pet lifecycle call failed, trying HTTP:', pyErr);
        }
      }
      if (!data) {
        const fetchFn = typeof apiFetch === 'function' ? apiFetch : fetch;
        const res = await fetchFn(endpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: '{}'
        });
        if (res && res.ok) {
          data = await res.json();
        }
      }
      if (data && data.ok) {
        if (typeof showToast === 'function') showToast(successText);
        const updated = await fetchPetRuntimeStatus();
        renderPetSettings(updated);
        setPetMenuStatus(updated);
        syncPetWidgetVisibility(updated);
      } else {
        if (typeof showToast === 'function') showToast(failText);
        const curStatus = await fetchPetRuntimeStatus();
        renderPetSettings(curStatus);
      }
    } catch (err) {
      console.error(`pet ${isUninstall ? 'uninstall' : 'install'} error:`, err);
      if (typeof showToast === 'function') showToast(failText);
      try {
        const curStatus = await fetchPetRuntimeStatus();
        renderPetSettings(curStatus);
      } catch (_) {}
    } finally {
      if (btn) btn.disabled = false;
    }
  });

  $('pet-enabled')?.addEventListener('change', () => { void savePetSettings(); });
  $('pet-runtime')?.addEventListener('change', () => { void savePetSettings(); });
  $('pet-install-runtime')?.addEventListener('click', async () => {
    await installDefaultPetRuntime();
    renderPetSettings(await fetchPetRuntimeStatus());
  });
  $('pet-gallery-import')?.addEventListener('change', async e => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      if (file.size > 17 * 1024 * 1024) throw new Error('pet_spritesheet_too_large');
      const encoded = await new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result).split(',')[1]);
        reader.onerror = reject;
        reader.readAsDataURL(file);
      });
      const slug = file.name.replace(/\.[^.]+$/, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 50) || `pet-${Date.now()}`;
      const result = await petGalleryRequest('/api/pets/import', {
        slug, display_name: file.name, image_base64: encoded, confirm: true
      });
      if (!result.ok) throw new Error(result.error_code || 'pet_import_failed');
      await refreshPetGallery();
    } catch (error) { showToast(petT('pet.configFailed', { code: error.message })); }
    finally { e.target.value = ''; }
  });
  $('pet-gallery')?.addEventListener('change', async e => {
    const result = await petGalleryRequest('/api/pets/active', { slug: e.target.value, confirm: true });
    if (!result.ok) showToast(petT('pet.configFailed', { code: result.error_code || result.code }));
    await refreshPetGallery();
    if (result.ok) await requestConfigurePet({});
  });
  $('pet-renderer')?.addEventListener('change', (e) => {
    updateCharacterPreview(e.target.value);
    void savePetSettings();
  });
  $('pet-scale')?.addEventListener('input', updatePetRangeLabels);
  $('pet-opacity')?.addEventListener('input', updatePetRangeLabels);
  $('pet-scale')?.addEventListener('change', () => { void savePetSettings(); });
  $('pet-opacity')?.addEventListener('change', () => { void savePetSettings(); });

  setInterval(pollPetControls, 2000);
  initPetIdleFSM();
}

// --------------------------------------------------------------------------
// Idle & Sleep Cycle FSM (Ported from rullerzhou-afk/clawd-on-desk 6.1k stars)
// --------------------------------------------------------------------------

const PET_STATE = {
  ACTIVE: 'active',
  IDLE: 'idle',
  BORED: 'bored',
  DOZING: 'dozing',
  SLEEPING: 'sleeping'
};

let currentPetState = PET_STATE.IDLE;
let lastUserActivityTime = Date.now();
let idleFSMInterval = null;

function markPetUserActive() {
  lastUserActivityTime = Date.now();
  if (currentPetState === PET_STATE.DOZING || currentPetState === PET_STATE.SLEEPING) {
    // 唤醒动画
    const char = $('pet-character');
    if (char) {
      char.classList.remove('pet-sleeping', 'pet-dozing');
      char.classList.add('pet-bounce');
      setTimeout(() => char.classList.remove('pet-bounce'), 1000);
    }
    showPetBubble(petT('pet.returnQuote') || '唔……你回来啦！继续一起阅读吧 ✨', 3500, PET_BUBBLE_PRIORITY.LOW_IDLE);
  }
  currentPetState = PET_STATE.ACTIVE;
}

function initPetIdleFSM() {
  // 监听用户活跃行为（指针移动、按键、滚动）
  window.addEventListener('pointermove', () => { markPetUserActive(); }, { passive: true });
  window.addEventListener('keydown', () => { markPetUserActive(); }, { passive: true });
  window.addEventListener('scroll', () => { markPetUserActive(); }, { passive: true });

  if (idleFSMInterval) clearInterval(idleFSMInterval);
  idleFSMInterval = setInterval(checkPetIdleState, 15000);
}

function checkPetIdleState() {
  const widget = $('readmd-pet-widget');
  if (!widget || widget.classList.contains('hidden')) return;

  const now = Date.now();
  const idleDuration = now - lastUserActivityTime;
  const char = $('pet-character');

  // 10分钟无操作 -> 深度睡眠
  if (idleDuration > 10 * 60 * 1000) {
    if (currentPetState !== PET_STATE.SLEEPING) {
      currentPetState = PET_STATE.SLEEPING;
      if (char) {
        char.classList.remove('pet-dozing');
        char.classList.add('pet-sleeping');
      }
      showPetBubble(petT('pet.sleepQuote1') || 'zZ... 呼……噜…… (睡着了) 💤', 4000, PET_BUBBLE_PRIORITY.LOW_IDLE);
    }
  }
  // 4分钟无操作 -> 打瞌睡
  else if (idleDuration > 4 * 60 * 1000) {
    if (currentPetState !== PET_STATE.DOZING && currentPetState !== PET_STATE.SLEEPING) {
      currentPetState = PET_STATE.DOZING;
      if (char) {
        char.classList.add('pet-dozing');
      }
      showPetBubble(petT('pet.sleepQuote2') || '有点困困的呢…… (揉眼睛) 🥱', 4000, PET_BUBBLE_PRIORITY.LOW_IDLE);
    }
  }
  // 1.5分钟无操作 -> 发呆动作池
  else if (idleDuration > 90 * 1000) {
    if (currentPetState === PET_STATE.ACTIVE || currentPetState === PET_STATE.IDLE) {
      currentPetState = PET_STATE.BORED;
      const boredQuotes = [
        petT('pet.idleQuote1') || '静静地看着你读书~ 🍵',
        petT('pet.idleQuote2') || '你在读哪一章呀？我也想瞧瞧 👀',
        petT('pet.idleQuote3') || '窗外微风正好，好适合安静看书呀 🍃'
      ];
      const quote = boredQuotes[Math.floor(Math.random() * boredQuotes.length)];
      showPetBubble(quote, 4000, PET_BUBBLE_PRIORITY.LOW_IDLE);
    }
  } else {
    currentPetState = PET_STATE.ACTIVE;
    if (char) char.classList.remove('pet-sleeping', 'pet-dozing');
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initPetSystem);
} else {
  initPetSystem();
}
window.addEventListener('load', initPetSystem);

window.addEventListener('readmd:language-changed', refreshPetMenuStatus);

async function petGalleryRequest(url, body) {
  try {
    const res = await (typeof apiFetch === 'function' ? apiFetch : fetch)(url, body ? {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body)
    } : undefined);
    return await res.json();
  } catch (_) { return { ok: false, code: 'pet_connection_failed' }; }
}

let petRuntimeInstallPromise = null;
async function installDefaultPetRuntime() {
  if (petRuntimeInstallPromise) return petRuntimeInstallPromise;
  const button = $('pet-install-runtime');
  if (button) button.disabled = true;
  petRuntimeInstallPromise = (async () => {
    try {
      const result = await petGalleryRequest('/api/pets/runtime/install', { confirm: true });
      showToast(result.ok ? petT('pet.installSuccess') : petT('pet.configFailed', { code: result.code || result.error_code }));
      return result;
    } finally {
      if (button) button.disabled = false;
      petRuntimeInstallPromise = null;
    }
  })();
  return petRuntimeInstallPromise;
}

async function refreshPetGallery() {
  const result = await petGalleryRequest('/api/pets');
  if (!result.ok) return;
  const select = $('pet-gallery');
  if (select) {
    select.replaceChildren(new Option('Hermes', ''));
    for (const pet of result.pets || []) select.add(new Option(pet.display_name || pet.slug, pet.slug));
    select.value = result.active || '';
  }
  const image = result.active ? `url("/api/pets/thumb?slug=${encodeURIComponent(result.active)}")` : '';
  for (const id of ['pet-character', 'pet-preview-character']) {
    const element = $(id);
    if (element) element.style.backgroundImage = image;
  }
}
