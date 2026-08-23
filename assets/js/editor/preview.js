'use strict';
/* ============================================================
   ReadMD Editor - Live Split Preview & Scroll Sync
   ============================================================ */

/* ---------------- 编辑实时预览（左/右/下/上 + 滚动同步） ---------------- */

let pvTimer = null;
let pvLast = '';
let pvEditorEl = null;

function hasUnsavedEditorChanges() {
  if (!state.editing) return false;
  return getEditContent() !== (state.original || '');
}

function syncSavedTab(path, content) {
  const tab = typeof findTabByPath === 'function' ? findTabByPath(path) : null;
  if (!tab) return;
  tab.content = content;
  tab.original = content;
  tab.fixed = content;
  tab.fixes = [];
  tab.isDirty = false;
}

function applySavedMtime(result) {
  if (result && typeof result.mtime === 'number') {
    state.mtime = result.mtime;
    const tab = typeof getActiveTab === 'function' ? getActiveTab() : null;
    if (tab) tab.mtime = result.mtime;
  }
}

async function renderSavedDocument(content) {
  state.original = content;
  state.fixed = content;
  state.fixes = [];
  state.stats = {};
  if (typeof setFixes === 'function') setFixes([], {});
  if (typeof renderContent === 'function') {
    const contentEl = document.getElementById('content');
    const page = state.pagination?.enabled && state.pagination.mode === 'paged' ? state.pagination.currentPage : 0;
    const scroll = contentEl?.scrollTop || 0;
    await renderContent(content, state.sourceName || (state.file ? state.file.split(/[\\/]/).pop() : 'document'));
    if (page > 0) await renderPage(page, null, true);
    requestAnimationFrame(() => {
      if (contentEl) contentEl.scrollTop = scroll;
    });
  }
}

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
  if ($('pv-trigger')) $('pv-trigger').textContent = narrow ? previewLabel + '：' + names[layout] + '（' + (_t('editor.narrowScreenBottom') || '窄屏置底') + '）⌄' : previewLabel + '：' + names[layout] + '⌄';
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

let isSyncingFromEditor = false;
let isSyncingFromPreview = false;

function schedulePreview() {
  if (pvTimer) clearTimeout(pvTimer);
  if (state.liveUpdate === false) return; // Save-only mode
  pvTimer = setTimeout(renderPreview, 300);
}

async function renderPreview() {
  pvTimer = null;
  const pane = $('preview-pane');
  if (!pane || state.pvLayout === 'none' || !state.editing) return;
  let src = getEditContent();
  if (src === pvLast) return;
  pvLast = src;

  // 预处理 @import
  if (window.processDocImports) {
    src = await window.processDocImports(src, state.file || '');
  }

  let html;
  try {
    const transformed = window.transformAcademicCallouts ? transformAcademicCallouts(src) : src;
    const prot = protectMath(transformed);
    if (window.parseMarkdownWithSourceMap) {
      html = restoreMath(parseMarkdownWithSourceMap(prot.src), prot.saved);
    } else {
      html = restoreMath(marked.parse(prot.src, { gfm: true, breaks: !!(state && state.breakOnSingleNewline) }), prot.saved);
    }
  } catch (e) {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    html = '<p class="ai-err">' + (_t('editor.previewRenderFail') || '预览渲染失败') + '</p>';
  }
  pane.innerHTML = window.sanitizeRenderedHtml ? window.sanitizeRenderedHtml(html) : html;
  fixLinks(pane);
  fixImages(pane);
  renderMath(pane);
  if (window.renderAllCodeChunks) renderAllCodeChunks(pane);
  if (window.renderAllDiagrams) renderAllDiagrams(pane);
}

function getEditorVisibleLine() {
  if (cmView && cmView.lineBlockAtHeight) {
    try {
      const lineBlock = cmView.lineBlockAtHeight(cmView.scrollDOM.scrollTop);
      return cmView.state.doc.lineAt(lineBlock.from).number;
    } catch (e) {
      return 1;
    }
  } else if ($('edit-area')) {
    const ta = $('edit-area');
    const totalLines = ta.value.split('\n').length;
    const pct = ta.scrollTop / Math.max(1, ta.scrollHeight - ta.clientHeight);
    return Math.max(1, Math.round(pct * totalLines));
  }
  return 1;
}

function pvSyncFromEditor() {
  if (!state.pvSync || state.pvLayout === 'none' || isSyncingFromPreview) return;
  isSyncingFromEditor = true;

  const dst = $('preview-wrap');
  if (!dst) { isSyncingFromEditor = false; return; }

  const currentLine = getEditorVisibleLine();
  const pane = $('preview-pane');
  if (!pane) { isSyncingFromEditor = false; return; }

  // 查找带有 data-source-line 的所有元素
  const lineEls = Array.from(pane.querySelectorAll('[data-source-line]'));
  if (lineEls.length === 0) {
    const src = pvEditorEl || $('edit-area');
    if (src) {
      const maxSrc = src.scrollHeight - src.clientHeight;
      const maxDst = dst.scrollHeight - dst.clientHeight;
      if (maxSrc > 0 && maxDst > 0) dst.scrollTop = (src.scrollTop / maxSrc) * maxDst;
    }
    isSyncingFromEditor = false;
    return;
  }

  let targetEl = lineEls[0];
  let nextEl = null;
  for (let i = 0; i < lineEls.length; i++) {
    const l = parseInt(lineEls[i].dataset.sourceLine, 10);
    if (l <= currentLine) {
      targetEl = lineEls[i];
    } else {
      nextEl = lineEls[i];
      break;
    }
  }

  if (targetEl) {
    let targetScrollTop = targetEl.offsetTop;
    if (nextEl) {
      const l1 = parseInt(targetEl.dataset.sourceLine, 10);
      const l2 = parseInt(nextEl.dataset.sourceLine, 10);
      if (l2 > l1) {
        const factor = (currentLine - l1) / (l2 - l1);
        targetScrollTop += factor * (nextEl.offsetTop - targetEl.offsetTop);
      }
    }
    dst.scrollTop = Math.max(0, targetScrollTop - 20);
  }

  setTimeout(() => { isSyncingFromEditor = false; }, 50);
}

function pvSyncFromPreview() {
  if (!state.pvSync || state.pvLayout === 'none' || isSyncingFromEditor) return;
  isSyncingFromPreview = true;

  const src = $('preview-wrap');
  const pane = $('preview-pane');
  if (!src || !pane) { isSyncingFromPreview = false; return; }

  const scrollTop = src.scrollTop;
  const lineEls = Array.from(pane.querySelectorAll('[data-source-line]'));
  if (lineEls.length === 0) {
    const dst = pvEditorEl || $('edit-area');
    if (dst) {
      const maxSrc = src.scrollHeight - src.clientHeight;
      const maxDst = dst.scrollHeight - dst.clientHeight;
      if (maxSrc > 0 && maxDst > 0) dst.scrollTop = (src.scrollTop / maxSrc) * maxDst;
    }
    isSyncingFromPreview = false;
    return;
  }

  let matchedLine = 1;
  for (let i = 0; i < lineEls.length; i++) {
    if (lineEls[i].offsetTop <= scrollTop + 30) {
      matchedLine = parseInt(lineEls[i].dataset.sourceLine, 10);
    } else {
      break;
    }
  }

  if (cmView && window.ReadMDCodeMirror) {
    try {
      const doc = cmView.state.doc;
      const targetLine = Math.min(doc.lines, Math.max(1, matchedLine));
      const pos = doc.line(targetLine).from;
      cmView.dispatch({
        effects: window.ReadMDCodeMirror.EditorView.scrollIntoView(pos, { y: 'start' })
      });
    } catch (e) {}
  } else if ($('edit-area')) {
    const ta = $('edit-area');
    const totalLines = ta.value.split('\n').length;
    ta.scrollTop = (matchedLine / totalLines) * (ta.scrollHeight - ta.clientHeight);
  }

  setTimeout(() => { isSyncingFromPreview = false; }, 50);
}

function alignEditorAndPreview() {
  pvSyncFromEditor();
  showToast('已完成编辑器与预览视图精确行对齐', 1200);
}
window.alignEditorAndPreview = alignEditorAndPreview;

function applyPvUi() {
  document.querySelectorAll('.pv-btn').forEach(b => b.classList.toggle('active', b.dataset.pv === state.pvLayout));
  const sync = $('pv-sync');
  if (sync) sync.checked = !!state.pvSync;
  setPvLayout(state.pvLayout);
}

async function toggleEdit() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (state.editing) {
    if (!await confirmExitEdit()) return;
    applyPvUi();
    return;
  }
  if (state.original === undefined || state.original === '') { showToast(_t('toast.noEditableContent') || '没有可编辑的内容'); return; }
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

async function confirmExitEdit() {
  if (!hasUnsavedEditorChanges()) {
    exitEdit();
    return true;
  }
  const action = await promptDirtyClose(state.sourceName || state.file || 'document');
  if (action === 'cancel') return false;
  if (action === 'save') {
    await saveEdit();
    return !state.editing;
  }
  const activeTab = typeof getActiveTab === 'function' ? getActiveTab() : null;
  if (activeTab) {
    activeTab.content = state.original;
    activeTab.fixed = state.original;
    activeTab.isDirty = false;
    if (typeof renderTabsBar === 'function') renderTabsBar();
  }
  exitEdit();
  return true;
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
  if (typeof updateUnloadGuard === 'function') updateUnloadGuard();
}

async function saveEdit() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.editing) return false;
  const content = cmView ? cmView.state.doc.toString() : $('edit-area').value;
  if (!state.file) {
    // 虚拟文档（转换 / OCR / 网页）：另存为 .md 后切换为文件模式
    const name = (state.sourceName || 'document').replace(/[\\/]/g, '_');
    const suggested = name.replace(/\.[^.]+$/, '') + '.md';
    let out = null;
    if (hasPy) {
      busy(true);
      try { out = await py.save_as(content, suggested, state.webAssets || []); }
      catch (e) { showToast((_t('toast.saveFailed') || '保存失败：') + e.message); busy(false); return false; }
      busy(false);
      if (!out) { showToast(_t('toast.saveCancelled') || '已取消保存'); return false; }
      showToast((_t('toast.savedPrefix') || '已保存：') + out);
      exitEdit();
      await loadFile(out);
      return Boolean(out);
    }
    const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = suggested;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 3000);
    showToast((_t('toast.downloadedPrefix') || '已下载：') + suggested);
    return true;
  }
  busy(true);
  try {
    let ok;
    if (hasPy) {
      ok = await py.save_file(state.file, content, state.encoding || 'utf-8', state.mtime || null);
    } else {
      const r = await apiFetch('/api/save', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          path: state.file,
          content,
          encoding: state.encoding || 'utf-8',
          expected_mtime: state.mtime || null,
        }),
      });
      ok = await r.json();
    }
    if (ok && ok.ok !== false) {
      syncSavedTab(state.file, content);
      applySavedMtime(ok);
      await renderSavedDocument(content);
      showToast(ok.backup ? (_t('toast.savedWithBackup', { backup: ok.backup }) || ('已保存（备份：' + ok.backup + '）')) : (_t('toast.savedSuccess') || '已保存'));
      exitEdit();
      return true;
    } else {
      if (ok && ok.conflict) {
        const action = await promptSaveConflict();
        if (action === 'save-as') {
          const activeTab = getActiveTab();
          if (activeTab) {
            activeTab.content = content;
            activeTab.fixed = content;
          }
          state.fixed = content;
          const saved = await saveAs(content);
          if (!saved) return false;
          exitEdit();
          await loadFile(state.file, { force: true });
          return true;
        }
        if (action === 'reload') {
          exitEdit();
          await loadFile(state.file, { force: true });
          return true;
        }
      } else {
        showToast((_t('toast.saveFailed') || '保存失败：') + ((ok && ok.error) || (_t('toast.unknownError') || '未知错误')));
      }
      return false;
    }
    return false;
  } catch (e) {
    showToast((_t('toast.saveFailed') || '保存失败：') + e.message);
    return false;
  } finally { busy(false); }
}

function promptSaveConflict() {
  return new Promise(resolve => {
    const modal = $('save-conflict-modal');
    if (!modal) {
      resolve('cancel');
      return;
    }
    modal.classList.remove('hidden');
    const cancel = $('save-conflict-cancel');
    setTimeout(() => cancel?.focus(), 20);

    const finish = action => {
      modal.classList.add('hidden');
      $('save-conflict-save-as').onclick = null;
      $('save-conflict-reload').onclick = null;
      $('save-conflict-cancel').onclick = null;
      modal.removeEventListener('click', onBackdrop);
      document.removeEventListener('keydown', onKey);
      resolve(action);
    };
    const onBackdrop = event => { if (event.target === modal) finish('cancel'); };
    const onKey = event => {
      if (event.key === 'Escape') { event.preventDefault(); finish('cancel'); }
    };
    $('save-conflict-save-as').onclick = () => finish('save-as');
    $('save-conflict-reload').onclick = () => finish('reload');
    cancel.onclick = () => finish('cancel');
    modal.addEventListener('click', onBackdrop);
    document.addEventListener('keydown', onKey);
  });
}

async function saveAs(contentOverride = null) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const content = contentOverride ?? state.fixed ?? state.original ?? '';
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
        activeTab.content = content;
        activeTab.original = content;
        activeTab.fixed = content;
        state.file = out;
        state.original = content;
        state.fixed = content;
        state.dir = activeTab.dir;
        state.mode = 'file';
        renderTabsBar();
        document.title = activeTab.name + ' - ReadMD';
        setFileTitle(activeTab.name, true, out);
        addRecent(out);
      }
      showToast((_t('toast.savedPrefix') || '已保存：') + out);
      return true;
    }
  } else {
    const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = suggested;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 3000);
    showToast((_t('toast.downloadedPrefix') || '已下载：') + suggested);
    return true;
  }
  return false;
}

