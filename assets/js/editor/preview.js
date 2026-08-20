'use strict';
/* ============================================================
   ReadMD Editor - Live Split Preview & Scroll Sync
   ============================================================ */

/* ---------------- 编辑实时预览（左/右/下/上 + 滚动同步） ---------------- */

let pvTimer = null;
let pvLast = '';
let pvEditorEl = null;

function getEditContent() {
  return cmView ? cmView.state.doc.toString() : ($('edit-area') && $('edit-area').value || '');
}

function setPvLayout(layout) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (['none', 'left', 'right', 'bottom', 'top'].indexOf(layout) < 0) layout = 'none';
  state.pvLayout = layout;
  document.querySelectorAll('.pv-btn').forEach(b => b.classList.toggle('active', b.dataset.pv === layout));
  const names = {
    none: _t('editor.previewNone') || '无',
    left: _t('editor.previewLeft') || '左',
    right: _t('editor.previewRight') || '右',
    bottom: _t('editor.previewBottom') || '下',
    top: _t('editor.previewTop') || '上'
  };
  const narrow = window.innerWidth < 600 && (layout === 'left' || layout === 'right');
  const previewLabel = _t('editor.preview') || '预览';
  if ($('pv-trigger')) $('pv-trigger').textContent = narrow ? previewLabel + '：' + names[layout] + '（' + (_t('editor.narrowScreenBottom') || 'Bottom on narrow screen') + '）⌄' : previewLabel + '：' + names[layout] + '⌄';
  const mc = $('main-col');

  const pw = $('preview-wrap');
  if (!mc || !pw) return;
  mc.classList.remove('pv-left', 'pv-right', 'pv-bottom', 'pv-top');
  if (state.editing && layout !== 'none') {
    mc.classList.add('pv-' + layout);
    pw.classList.remove('hidden');
    $('pv-splitter').classList.remove('hidden');
    applyPvSplit();
    schedulePreview();
  } else {
    pw.classList.add('hidden');
    $('pv-splitter').classList.add('hidden');
  }
  saveSettings();
}

function applyPvSplit() {
  const pw = $('preview-wrap'); if (!pw) return;
  const horizontal = state.pvLayout === 'left' || state.pvLayout === 'right';
  const pct = horizontal ? state.pvSplitX : state.pvSplitY;
  pw.style.flexBasis = pct + '%';
}

function bindPvSplitter() {
  const bar = $('pv-splitter'); const mc = $('main-col'); if (!bar || !mc) return;
  const update = e => {
    const r = mc.getBoundingClientRect(); let pct;
    if (state.pvLayout === 'left') pct = (e.clientX - r.left) / r.width * 100;
    else if (state.pvLayout === 'right') pct = (r.right - e.clientX) / r.width * 100;
    else if (state.pvLayout === 'top') pct = (e.clientY - r.top) / r.height * 100;
    else pct = (r.bottom - e.clientY) / r.height * 100;
    pct = Math.max(25, Math.min(70, pct));
    if (state.pvLayout === 'left' || state.pvLayout === 'right') state.pvSplitX = pct; else state.pvSplitY = pct;
    applyPvSplit();
  };
  bar.addEventListener('pointerdown', e => { bar.setPointerCapture(e.pointerId); update(e); });
  bar.addEventListener('pointermove', e => { if (bar.hasPointerCapture(e.pointerId)) update(e); });
  bar.addEventListener('pointerup', e => { if (bar.hasPointerCapture(e.pointerId)) bar.releasePointerCapture(e.pointerId); saveSettings(); });
  bar.addEventListener('keydown', e => { if (!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)) return; e.preventDefault(); const delta = (e.key === 'ArrowRight' || e.key === 'ArrowDown') ? 2 : -2; if (state.pvLayout === 'left' || state.pvLayout === 'right') state.pvSplitX = Math.max(25, Math.min(70, state.pvSplitX + delta)); else state.pvSplitY = Math.max(25, Math.min(70, state.pvSplitY + delta)); applyPvSplit(); saveSettings(); });
  window.addEventListener('resize', () => setPvLayout(state.pvLayout));
}

function schedulePreview() {
  if (pvTimer) clearTimeout(pvTimer);
  pvTimer = setTimeout(renderPreview, 300);
}

function renderPreview() {
  pvTimer = null;
  const pane = $('preview-pane');
  if (!pane || state.pvLayout === 'none' || !state.editing) return;
  const src = getEditContent();
  if (src === pvLast) return;
  pvLast = src;
  let html;
  try {
    const prot = protectMath(src);
    html = restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved);
  } catch (e) {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    html = '<p class="ai-err">' + (_t('editor.previewRenderFail') || '预览渲染失败') + '</p>';
  }
  pane.innerHTML = html;
  fixLinks(pane);
  fixImages(pane);
  renderMath(pane);
}

function pvSyncFromEditor() {
  if (!state.pvSync || state.pvLayout === 'none') return;
  const src = pvEditorEl || $('edit-area');
  const dst = $('preview-wrap');
  if (!src || !dst) return;
  const maxSrc = src.scrollHeight - src.clientHeight;
  const maxDst = dst.scrollHeight - dst.clientHeight;
  if (maxSrc <= 0 || maxDst <= 0) return;
  dst.scrollTop = (src.scrollTop / maxSrc) * maxDst;
}

function pvSyncFromPreview() {
  if (!state.pvSync || state.pvLayout === 'none') return;
  const src = $('preview-wrap');
  const dst = pvEditorEl || $('edit-area');
  if (!src || !dst) return;
  const maxSrc = src.scrollHeight - src.clientHeight;
  const maxDst = dst.scrollHeight - dst.clientHeight;
  if (maxSrc <= 0 || maxDst <= 0) return;
  dst.scrollTop = (src.scrollTop / maxSrc) * maxDst;
}

function applyPvUi() {
  document.querySelectorAll('.pv-btn').forEach(b => b.classList.toggle('active', b.dataset.pv === state.pvLayout));
  const sync = $('pv-sync');
  if (sync) sync.checked = !!state.pvSync;
  setPvLayout(state.pvLayout);
}

async function toggleEdit() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (state.editing) { exitEdit(); return; }
  if (state.original === undefined || state.original === '') { showToast(_t('toast.noEditableContent') || 'No editable content'); return; }
  $('edit-bar').classList.remove('hidden');
  $('content').classList.add('hidden');
  state.editing = true;
  setEditBtn(_t('editor.editing') || '编辑中');
  pvLast = '';
  try {
    await loadCodeMirror();
  } catch (e) { /* 退回 textarea */ }
  if (window.ReadMDCodeMirror) {
    $('edit-area').classList.add('hidden');
    $('edit-wrap').classList.remove('hidden');
    createEditor(state.original || '');
    pvEditorEl = cmView ? cmView.scrollDOM : null;
    if (pvEditorEl) pvEditorEl.addEventListener('scroll', pvSyncFromEditor);
  } else {
    $('edit-wrap').classList.add('hidden');
    $('edit-area').classList.remove('hidden');
    $('edit-area').value = state.original || '';
    pvEditorEl = $('edit-area');
    pvEditorEl.addEventListener('scroll', pvSyncFromEditor);
    $('edit-area').focus();
  }
  applyPvUi();
}

function exitEdit() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (pvTimer) { clearTimeout(pvTimer); pvTimer = null; }
  if (pvEditorEl) {
    pvEditorEl.removeEventListener('scroll', pvSyncFromEditor);
    pvEditorEl = null;
  }
  const pw = $('preview-wrap');
  if (pw) pw.classList.add('hidden');
  const mc = $('main-col');
  if (mc) mc.classList.remove('pv-left', 'pv-right', 'pv-bottom', 'pv-top');
  pvLast = '';
  if (!state.editing) {
    $('edit-bar').classList.add('hidden');
    $('edit-area').classList.add('hidden');
    $('edit-wrap').classList.add('hidden');
    $('content').classList.remove('hidden');
    setEditBtn(_t('toolbar.edit') || '编辑');
    return;
  }
  destroyEditor();
  $('edit-bar').classList.add('hidden');
  $('edit-area').classList.add('hidden');
  $('edit-wrap').classList.add('hidden');
  $('content').classList.remove('hidden');
  state.editing = false;
  setEditBtn(_t('toolbar.edit') || '编辑');
}

async function saveEdit() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.editing) return;
  const content = cmView ? cmView.state.doc.toString() : $('edit-area').value;
  if (!state.file) {
    // 虚拟文档（转换 / OCR / 网页）：另存为 .md 后切换为文件模式
    const name = (state.sourceName || 'document').replace(/[\\/]/g, '_');
    const suggested = name.replace(/\.[^.]+$/, '') + '.md';
    let out = null;
    if (hasPy) {
      busy(true);
      try { out = await py.save_as(content, suggested, state.webAssets || []); }
      catch (e) { showToast((_t('toast.saveFailed') || 'Save failed: ') + e.message); busy(false); return; }
      busy(false);
      if (!out) { showToast(_t('toast.saveCancelled') || 'Save cancelled'); return; }
      showToast((_t('toast.savedPrefix') || 'Saved: ') + out);
      exitEdit();
      await loadFile(out);
    } else {
      const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = suggested;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 3000);
      showToast((_t('toast.downloadedPrefix') || 'Downloaded: ') + suggested);
    }
    return;
  }
  busy(true);
  try {
    let ok;
    if (hasPy) {
      ok = await py.save_file(state.file, content, state.encoding || 'utf-8');
    } else {
      const r = await apiFetch('/api/save', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: state.file, content, encoding: state.encoding || 'utf-8' }),
      });
      ok = await r.json();
    }
    if (ok && ok.ok !== false) {
      showToast(ok.backup ? (_t('toast.savedWithBackup', { backup: ok.backup }) || ('已保存（备份：' + ok.backup + '）')) : (_t('toast.savedSuccess') || 'Saved'));
      exitEdit();
      await loadFile(state.file);
    } else {
      showToast((_t('toast.saveFailed') || 'Save failed: ') + ((ok && ok.error) || (_t('toast.unknownError') || 'Unknown error')));
    }
  } catch (e) { showToast((_t('toast.saveFailed') || 'Save failed: ') + e.message); }
  finally { busy(false); }
}

async function saveAs() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const content = state.fixed || state.original || '';
  const name = (state.sourceName || state.file || 'document').replace(/[\\/]/g, '_');
  const suggested = name.replace(/\.[^.]+$/, '') + '.md';
  if (hasPy) {
    const out = await py.save_as(content, suggested, state.webAssets || []);
    if (out) {
      const activeTab = getActiveTab();
      if (activeTab) {
        activeTab.path = out;
        activeTab.dir = String(out).replace(/[\\/][^\\/]*$/, '');
        activeTab.mode = 'file';
        activeTab.isVirtual = false;
        activeTab.isDirty = false;
        activeTab.name = String(out).split(/[\\/]/).pop();
        activeTab.title = activeTab.name;
        state.file = out;
        state.dir = activeTab.dir;
        state.mode = 'file';
        renderTabsBar();
        document.title = activeTab.name + ' - ReadMD';
        setFileTitle(activeTab.name, true, out);
        addRecent(out);
      }
      showToast((_t('toast.savedPrefix') || 'Saved: ') + out);
    }
  } else {
    const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = suggested;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 3000);
    showToast((_t('toast.downloadedPrefix') || 'Downloaded: ') + suggested);
  }
}

