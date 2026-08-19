'use strict';
/* ============================================================
   ReadMD Core - Module Management & Lifecycle
   ============================================================ */

/* ---------------- 模块懒加载 ---------------- */

function startModules() {
  if (state.modulesStarted) return;
  state.modulesStarted = true;
  if (hasPy) { try { py.start_modules(); } catch (e) { /* ignore */ } }
  pollModules();
}

async function pollModules() {
  try {
    const r = await apiFetch('/api/modules');
    const d = await r.json();
    state.modules = d.modules || {};
    state.win7 = !!d.win7;
    updateModuleUi();
    const pending = Object.values(state.modules).some(v => v === 'loading' || v === 'idle');
    if (pending && state.modulesStarted) setTimeout(pollModules, 900);
  } catch (e) {
    if (state.modulesStarted) setTimeout(pollModules, 2000);
  }
}

function updateModuleUi() {
  const m = state.modules;
  const ready = n => m[n] === 'ready';
  const disabled = n => m[n] === 'disabled';
  [['btn-convert', 'convert'], ['btn-web', 'web'], ['btn-ocr', 'ocr'], ['btn-ai', 'ai']].forEach(([id, key]) => {
    const el = $(id);
    if (!el) return;
    el.disabled = false;
    if (disabled(key)) {
      el.title = 'Win7 版暂不支持该功能';
    }
  });
  ['w-convert', 'w-web', 'w-ocr', 'w-ai'].forEach(id => {
    const el = $(id);
    if (el) el.disabled = false;
  });
  if (ready('ai') && !state.ai.config) loadAiConfig();
  const parts = [];
  for (const [k, v] of Object.entries(m)) {
    const label = { convert: '转换', ocr: 'OCR', web: '网页', ai: 'AI' }[k] || k;
    if (v === 'ready') parts.push(label + '\u2713');
    else if (v === 'error') parts.push(label + '\u2717');
    else if (v === 'disabled') parts.push(label + ' Win7 暂不支持');
    else parts.push(label + '\u2026');
  }
  const el = $('status-mods');
  if (el) el.textContent = parts.length ? '模块 ' + parts.join(' ') : '';
}

function moduleBlocked(name) {
  if (state.modules[name] === 'disabled') {
    showToast('该功能在 Win7 版暂不支持（本版本仅保留 docx / pdf 转 MD 与导出功能）', 3400);
    return true;
  }
  return false;
}

async function ensureModule(name, timeoutMs) {
  const t0 = Date.now();
  const limit = timeoutMs || 60000;
  if (!moduleLoadRequests[name]) {
    moduleLoadRequests[name] = apiFetch('/api/modules/load', {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name }),
    }).catch(() => null);
  }
  await moduleLoadRequests[name];
  while (Date.now() - t0 < limit) {
    try {
      const r = await apiFetch('/api/modules');
      const d = await r.json();
      const st = d.modules && d.modules[name];
      if (st === 'ready') return true;
      if (st === 'error') { delete moduleLoadRequests[name]; showToast('模块「' + name + '」加载失败，请重试'); return false; }
    } catch (e) { /* ignore */ }
    await new Promise(r => setTimeout(r, 800));
  }
  showToast('模块加载超时，请重试');
  return false;
}
