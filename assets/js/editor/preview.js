'use strict';
/* ============================================================
   ReadMD Editor - Live Split Preview & Scroll Sync
   ============================================================ */

/* ---------------- 编辑实时预览（左/右/下/上 + 滚动同步） ---------------- */

let pvTimer = null;
let pvLast = '';
let pvEditorEl = null;
let pvRenderEpoch = 0;

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
  tab.externalChanged = false;
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
  if (layout === 'none' && typeof switchEditAiToChatPanel === 'function') {
    switchEditAiToChatPanel();
  }
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
  const trigger = $('pv-trigger');
  if (trigger) {
    // Short visible label; the full state (incl. narrow-screen note) is in title/aria-label.
    const full = narrow
      ? previewLabel + '：' + names[layout] + '（' + (_t('editor.narrowScreenBottom') || '窄屏置底') + '）'
      : previewLabel + '：' + names[layout];
    trigger.innerHTML = '<svg class="tb-ic" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M12 4v16"/></svg><span class="pv-trigger-label"></span><svg class="md-caret" viewBox="0 0 24 24" aria-hidden="true"><path d="m7 10 5 5 5-5"/></svg>';
    trigger.querySelector('.pv-trigger-label').textContent = full;
    trigger.title = full;
    trigger.setAttribute('aria-label', full);
    trigger.classList.toggle('is-on', layout !== 'none');
  }
  const mc = $('main-col');

  const pw = $('preview-wrap');
  if (!mc || !pw) return;
  mc.classList.remove('pv-left', 'pv-right', 'pv-bottom', 'pv-top');
  mc.classList.remove('pv-auto-hidden');
  if (state.editing && layout !== 'none') {
    // 预览界面和AI对话界面不能同时出现：若开启预览，自动收起 AI 对话面板
    const aiPanel = $('ai-panel');
    if (aiPanel && !aiPanel.classList.contains('hidden')) {
      aiPanel.classList.add('hidden');
    }
    mc.classList.add('pv-' + layout);
    const bounds = mc.getBoundingClientRect();
    const horizontal = layout === 'left' || layout === 'right';
    // Decide on window width: main-col width is circular here (it shrinks when the
    // preview pane is already flexed), while the specs pin the boundary at 720/760.
    const constrained = horizontal ? window.innerWidth < 740 : bounds.height < 520;
    mc.classList.toggle('pv-auto-hidden', constrained);
    pw.classList.toggle('hidden', constrained);
    $('pv-splitter').classList.toggle('hidden', constrained);
    if (!constrained) {
      applyPvSplit();
      schedulePreview();
    }
  } else {
    pw.classList.add('hidden');
    $('pv-splitter').classList.add('hidden');
  }
  // AI 写入时临时收起预览（_pvLayoutBeforeAi），不能覆盖用户记住的布局。
  if (!state._pvLayoutBeforeAi) saveSettings();
}

/* 每个文档记住上次的光标与滚动位置，再次进入编辑时恢复。 */
function editMemoryKey() {
  const id = state.file || state.sourceName || '';
  return id ? 'readmd_edit_pos:' + id : '';
}

function rememberEditPosition() {
  const key = editMemoryKey();
  if (!key || !cmView) return;
  try {
    localStorage.setItem(key, JSON.stringify({ a: cmView.state.selection.main.head, t: cmView.scrollDOM.scrollTop }));
  } catch (e) { /* storage full or disabled */ }
}

/* 新建空白文档并直接进入编辑（欢迎页按钮、Ctrl+N、命令面板共用）。 */
async function newDocument() {
  if (state.editing && !await confirmExitEdit()) return;
  await renderVirtual('', '', '', '', []);
  await toggleEdit();
}

function restoreEditPosition() {
  const key = editMemoryKey();
  if (!key || !cmView) return;
  try {
    const m = JSON.parse(localStorage.getItem(key) || 'null');
    if (!m) return;
    const anchor = Math.max(0, Math.min(Number(m.a) || 0, cmView.state.doc.length));
    cmView.dispatch({ selection: { anchor } });
    requestAnimationFrame(() => { if (cmView) cmView.scrollDOM.scrollTop = Number(m.t) || 0; });
  } catch (e) { /* ignore corrupt entry */ }
}

function applyPvSplit() {
  const pw = $('preview-wrap'); if (!pw) return;
  const horizontal = state.pvLayout === 'left' || state.pvLayout === 'right';
  const key = horizontal ? 'pvSplitX' : 'pvSplitY';
  const raw = Number(state[key]);
  const pct = Math.max(25, Math.min(70, Number.isFinite(raw) ? raw : (horizontal ? 50 : 46)));
  state[key] = pct;
  pw.style.flexBasis = pct + '%';
  const splitter = $('pv-splitter');
  if (splitter) {
    splitter.setAttribute('aria-valuemin', '25');
    splitter.setAttribute('aria-valuemax', '70');
    splitter.setAttribute('aria-valuenow', String(Math.round(pct)));
    splitter.setAttribute('aria-valuetext', `${Math.round(pct)}%`);
    splitter.setAttribute('aria-orientation', horizontal ? 'vertical' : 'horizontal');
  }
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
  window.addEventListener('resize', () => requestAnimationFrame(() => setPvLayout(state.pvLayout)));
}

/* Which pane the user is driving: scroll events from the other pane are
   echoes of our own programmatic scrolling and must not bounce back. */
let pvScrollDriver = null;
let pvScrollDriverTimer = null;
let pvSyncFrame = 0;
function pvClaimScroll(who) {
  pvScrollDriver = who;
  if (pvScrollDriverTimer) clearTimeout(pvScrollDriverTimer);
  pvScrollDriverTimer = setTimeout(() => { pvScrollDriver = null; }, 160);
}
// Kept for callers that read these flags.
let isSyncingFromEditor = false;
let isSyncingFromPreview = false;

/* Preview refresh: the first keystroke after a pause renders on the next
   frame (instant feel), a burst of typing is coalesced, and the delay grows
   with document size so huge files stay responsive. */
let pvLastRenderAt = 0;
function schedulePreview() {
  if (pvTimer) clearTimeout(pvTimer);
  if (state.liveUpdate === false) return; // Save-only mode
  if (state.pvLayout === 'none' || !state.editing) return;
  const size = getEditContent().length;
  const settle = size >= 100000 ? 600 : size >= 30000 ? 320 : 140;
  const idle = Date.now() - pvLastRenderAt > settle * 2;
  pvTimer = setTimeout(renderPreview, idle ? 16 : settle);
}

/* Replace only the top-level blocks whose HTML changed, so MathJax output,
   diagrams and images elsewhere in the pane are not re-rendered. */
function pvPatchPane(pane, html) {
  const tpl = document.createElement('template');
  tpl.innerHTML = html;
  const next = Array.from(tpl.content.childNodes).filter(n => n.nodeType === 1 || (n.nodeType === 3 && n.textContent.trim()));
  const prev = Array.from(pane.childNodes);
  if (!prev.length || !pane.__pvSource || Math.abs(prev.length - next.length) > 400) {
    pane.innerHTML = '';
    next.forEach((n, i) => { if (n.nodeType === 1) n.__pvSource = n.outerHTML; pane.appendChild(n); });
    pane.__pvSource = true;
    return next.filter(n => n.nodeType === 1);
  }
  const key = n => n.nodeType === 1 ? (n.__pvSource || n.outerHTML).replace(/\sdata-source-line="\d+"/, '') : n.textContent;
  const prevKeys = prev.map(key);
  const nextKeys = next.map(n => n.nodeType === 1 ? n.outerHTML.replace(/\sdata-source-line="\d+"/, '') : n.textContent);
  let head = 0;
  while (head < prev.length && head < next.length && prevKeys[head] === nextKeys[head]) head++;
  let tail = 0;
  while (tail < prev.length - head && tail < next.length - head && prevKeys[prev.length - 1 - tail] === nextKeys[next.length - 1 - tail]) tail++;
  // Unchanged blocks keep their node, only their source line is refreshed.
  const syncLine = (oldNode, newNode) => {
    if (oldNode.nodeType !== 1) return;
    const l = newNode.getAttribute('data-source-line');
    if (l) oldNode.setAttribute('data-source-line', l);
  };
  for (let i = 0; i < head; i++) syncLine(prev[i], next[i]);
  for (let i = 0; i < tail; i++) syncLine(prev[prev.length - 1 - i], next[next.length - 1 - i]);
  const anchor = tail ? prev[prev.length - tail] : null;
  for (let i = head; i < prev.length - tail; i++) prev[i].remove();
  const fresh = [];
  for (let i = head; i < next.length - tail; i++) {
    const n = next[i];
    if (n.nodeType === 1) { n.__pvSource = n.outerHTML; fresh.push(n); }
    pane.insertBefore(n, anchor);
  }
  return fresh;
}

async function renderPreview() {
  pvTimer = null;
  const renderEpoch = ++pvRenderEpoch;
  const pane = $('preview-pane');
  if (!pane || state.pvLayout === 'none' || !state.editing) return;
  let src = getEditContent();
  if (src === pvLast) return;
  pvLast = src;

  // 预处理 @import
  if (window.processDocImports) {
    src = await window.processDocImports(src, state.file || '');
    if (renderEpoch !== pvRenderEpoch) return;
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
  if (renderEpoch !== pvRenderEpoch) return;
  pvLastRenderAt = Date.now();
  const safe = window.sanitizeRenderedHtml ? window.sanitizeRenderedHtml(html) : html;
  const fresh = pvPatchPane(pane, safe);
  if (!fresh.length) return;
  // Post-process only the blocks that changed: fixLinks binds click handlers,
  // so it sees just the fresh links while heading lookup spans the whole pane.
  const freshLinks = fresh.flatMap(n => (n.tagName === 'A' ? [n] : Array.from(n.querySelectorAll('a'))));
  fixLinks({ querySelectorAll: sel => (sel === 'a' ? freshLinks : pane.querySelectorAll(sel)) });
  fixImages(pane);
  fresh.forEach(n => renderMath(n));
  if (window.renderAllCodeChunks) renderAllCodeChunks(pane);
  if (window.renderAllDiagrams) renderAllDiagrams(pane);
  if (state.pvSync) pvSyncFromEditor();
}

function getEditorVisibleLine() {
  const pos = pvEditorTopPosition();
  return pos ? Math.max(1, Math.floor(pos.line)) : 1;
}

/* Fractional source line at the top edge of the editor viewport. */
function pvEditorTopPosition() {
  if (cmView && cmView.lineBlockAtHeight) {
    try {
      const top = cmView.scrollDOM.scrollTop;
      // Heights inside lineBlockAtHeight are relative to the document top.
      const docTop = cmView.documentTop - cmView.scrollDOM.getBoundingClientRect().top + top;
      const y = Math.max(0, top - docTop);
      const block = cmView.lineBlockAtHeight(y);
      const line = cmView.state.doc.lineAt(block.from).number;
      const frac = block.height > 0 ? Math.min(1, Math.max(0, (y - block.top) / block.height)) : 0;
      return { line: line + frac, max: cmView.scrollDOM.scrollHeight - cmView.scrollDOM.clientHeight, top };
    } catch (e) {
      return null;
    }
  }
  const ta = $('edit-area');
  if (ta) {
    const totalLines = ta.value.split('\n').length;
    const pct = ta.scrollTop / Math.max(1, ta.scrollHeight - ta.clientHeight);
    return { line: 1 + pct * (totalLines - 1), max: ta.scrollHeight - ta.clientHeight, top: ta.scrollTop };
  }
  return null;
}

/* [{line, top}] for every preview element carrying a source line, sorted. */
function pvAnchors(wrap, pane) {
  const base = wrap.getBoundingClientRect().top - wrap.scrollTop;
  const out = [];
  pane.querySelectorAll('[data-source-line]').forEach(el => {
    const line = parseInt(el.dataset.sourceLine, 10);
    if (!line || !el.getClientRects().length) return;
    const top = el.getBoundingClientRect().top - base;
    if (out.length && (line <= out[out.length - 1].line || top < out[out.length - 1].top)) return;
    out.push({ line, top });
  });
  return out;
}

function pvSyncFromEditor() {
  if (!state.pvSync || state.pvLayout === 'none' || pvScrollDriver === 'preview') return;
  if (pvSyncFrame) return;
  pvSyncFrame = requestAnimationFrame(() => {
    pvSyncFrame = 0;
    pvClaimScroll('editor');
    const dst = $('preview-wrap');
    const pane = $('preview-pane');
    const pos = pvEditorTopPosition();
    if (!dst || !pane || !pos) return;
    const maxDst = dst.scrollHeight - dst.clientHeight;
    if (maxDst <= 0) return;
    if (pos.top <= 1) { dst.scrollTop = 0; return; }
    if (pos.max > 0 && pos.top >= pos.max - 1) { dst.scrollTop = maxDst; return; }
    const anchors = pvAnchors(dst, pane);
    if (!anchors.length) {
      if (pos.max > 0) dst.scrollTop = (pos.top / pos.max) * maxDst;
      return;
    }
    let i = 0;
    while (i + 1 < anchors.length && anchors[i + 1].line <= pos.line) i++;
    const a = anchors[i];
    const b = anchors[i + 1];
    let y;
    if (pos.line < a.line) y = a.top * (pos.line - 1) / Math.max(1, a.line - 1);
    else if (b) y = a.top + (b.top - a.top) * (pos.line - a.line) / Math.max(1e-6, b.line - a.line);
    else {
      const lines = cmView ? cmView.state.doc.lines : a.line + 1;
      y = a.top + (dst.scrollHeight - a.top) * (pos.line - a.line) / Math.max(1, lines + 1 - a.line);
    }
    dst.scrollTop = Math.max(0, Math.min(maxDst, y - 12));
  });
}

function pvSyncFromPreview() {
  if (!state.pvSync || state.pvLayout === 'none' || pvScrollDriver === 'editor') return;
  if (pvSyncFrame) return;
  pvSyncFrame = requestAnimationFrame(() => {
    pvSyncFrame = 0;
    pvClaimScroll('preview');
    const src = $('preview-wrap');
    const pane = $('preview-pane');
    if (!src || !pane) return;
    const maxSrc = src.scrollHeight - src.clientHeight;
    const y = src.scrollTop + 12;
    const scroller = cmView ? cmView.scrollDOM : $('edit-area');
    if (!scroller) return;
    const maxDst = scroller.scrollHeight - scroller.clientHeight;
    if (src.scrollTop <= 1) { scroller.scrollTop = 0; return; }
    if (maxSrc > 0 && src.scrollTop >= maxSrc - 1) { scroller.scrollTop = maxDst; return; }
    const anchors = pvAnchors(src, pane);
    if (!anchors.length || !cmView) {
      if (maxSrc > 0) scroller.scrollTop = (src.scrollTop / maxSrc) * maxDst;
      return;
    }
    let i = 0;
    while (i + 1 < anchors.length && anchors[i + 1].top <= y) i++;
    const a = anchors[i], b = anchors[i + 1];
    let line;
    if (y < a.top) line = 1 + (a.line - 1) * (y / Math.max(1, a.top));
    else if (b) line = a.line + (b.line - a.line) * (y - a.top) / Math.max(1, b.top - a.top);
    else line = a.line + (y - a.top) / Math.max(1, src.scrollHeight - a.top) * (cmView.state.doc.lines + 1 - a.line);
    const doc = cmView.state.doc;
    const n = Math.min(doc.lines, Math.max(1, Math.floor(line)));
    try {
      const block = cmView.lineBlockAt(doc.line(n).from);
      const docTop = cmView.documentTop - scroller.getBoundingClientRect().top + scroller.scrollTop;
      const target = docTop + block.top + block.height * Math.min(1, line - n);
      scroller.scrollTop = Math.max(0, Math.min(maxDst, target));
    } catch (e) { /* ignore */ }
  });
}

function alignEditorAndPreview() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  pvSyncFromEditor();
  showToast(_t('editor.previewAligned'), 1200);
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
  // 没有打开文档时不能编辑（新建文档请用 Ctrl+N / 欢迎页“新建”）；空文件可以编辑。
  if (state.original == null || (state.mode === 'welcome' && !state.file)) { showToast(_t('toast.noEditableContent') || '没有可编辑的内容'); return; }
  $('edit-bar').classList.remove('hidden');
  $('content').classList.add('hidden');
  state.editing = true;
  $('main-col')?.classList.add('is-editing');
  setEditBtn(_t('editor.editing') || '编辑中');
  pvLast = '';
  try {
    await loadCodeMirror();
  } catch (e) { /* 退回 textarea */ }
  let cmMounted = false;
  if (window.ReadMDCodeMirror) {
    $('edit-area').classList.add('hidden');
    $('edit-wrap').classList.remove('hidden');
    // 旧版或损坏的 CodeMirror 包会让 createEditor 抛错：退回 textarea，避免卡在空白编辑页。
    try { createEditor(state.original || ''); cmMounted = !!cmView; } catch (e) {
      console.error(e);
      try { destroyEditor(); } catch (_) { /* ignore */ }
    }
  }
  if (cmMounted) {
    pvEditorEl = cmView.scrollDOM;
    pvEditorEl.addEventListener('scroll', pvSyncFromEditor);
    restoreEditPosition();
    cmView.focus();
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
    await saveEdit({ exitAfterSave: true });
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
  if (state.editing) rememberEditPosition();
  if (typeof switchEditAiToChatPanel === 'function') switchEditAiToChatPanel();
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (pvTimer) { clearTimeout(pvTimer); pvTimer = null; }
  if (pvEditorEl) {
    pvEditorEl.removeEventListener('scroll', pvSyncFromEditor);
    pvEditorEl = null;
  }
  const pw = $('preview-wrap');
  if (pw) pw.classList.add('hidden');
  const mc = $('main-col');
  if (mc) mc.classList.remove('pv-left', 'pv-right', 'pv-bottom', 'pv-top', 'pv-auto-hidden', 'is-editing');
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

async function saveEdit(options = {}) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const exitAfterSave = Boolean(options && options.exitAfterSave);
  if (!state.editing) return false;
  const saveBtn = $('edit-save');
  if (saveBtn) { saveBtn.disabled = true; saveBtn.classList.add('btn-loading'); }
  try {
    const content = cmView ? cmView.state.doc.toString() : $('edit-area').value;
    if (!state.file) {
      const activeTab = typeof getActiveTab === 'function' ? getActiveTab() : null;
      if (activeTab && activeTab.source === 'ai' && (activeTab.dir || state.dir)) {
        return await autoSaveAiCopyTab(activeTab, { content, exitAfterSave });
      }

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
      if (r.status === 403) {
        showToast(_t('toast.saveDenied') || '保存被拒绝：请重新打开文档后再保存');
        return false;
      }
      ok = await r.json();
    }
    // The file's original encoding cannot hold a character that was typed:
    // offer to switch the document to UTF-8 instead of replacing it silently.
    if (ok && ok.error_code === 'encoding_unrepresentable') {
      busy(false);
      const switchToUtf8 = await confirmAction({
        title: _t('dialog.encodingTitle') || '无法按原编码保存',
        message: _t('dialog.encodingMessage', { encoding: ok.encoding || state.encoding, char: ok.char || '' })
          || `当前文件编码（${ok.encoding || state.encoding}）无法表示字符“${ok.char || ''}”。是否改为 UTF-8 保存？`,
        confirmText: _t('dialog.encodingUseUtf8') || '改用 UTF-8 保存',
        cancelText: _t('common.cancel') || '取消',
      });
      if (!switchToUtf8) return false;
      state.encoding = 'utf-8';
      const activeTab = getActiveTab();
      if (activeTab) activeTab.encoding = 'utf-8';
      busy(true);
      if (hasPy) {
        ok = await py.save_file(state.file, content, 'utf-8', state.mtime || null);
      } else {
        const r2 = await apiFetch('/api/save', {
          method: 'POST', headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ path: state.file, content, encoding: 'utf-8', expected_mtime: state.mtime || null }),
        });
        ok = await r2.json();
      }
    }
    if (ok && ok.ok !== false) {
      syncSavedTab(state.file, content);
      applySavedMtime(ok);
      await renderSavedDocument(content);
      if (typeof renderTabsBar === 'function') renderTabsBar();
      if (typeof updateUnloadGuard === 'function') updateUnloadGuard();
      const savedTarget = state.browserCopy
        ? `${state.sourceName || state.file} (${_t('app.browserCopy') || 'browser copy'})`
        : (state.file || state.sourceName || 'document');
      showToast(ok.backup
        ? (_t('toast.savedWithBackup', { backup: ok.backup }) || ('已保存（备份：' + ok.backup + '）'))
        : ((_t('toast.savedPrefix') || '已保存：') + savedTarget));
      if (exitAfterSave) exitEdit();
      return true;
    } else {
      if (ok && ok.conflict) {
        const action = await promptSaveConflict();
        if (action === 'save-as') {
          const activeTab = getActiveTab();
          const suggested = (state.sourceName || state.file || 'document')
            .replace(/[\\/]/g, '_')
            .replace(/\.[^.]+$/, '') + '.md';
          if (activeTab) {
            activeTab.content = content;
            activeTab.fixed = content;
          }
          state.fixed = content;
          const saved = await saveAs(content);
          if (!hasPy && saved) {
            showToast((_t('toast.downloadedPrefix') || '已下载：') + suggested);
            return false;
          }
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
        if (action === 'cancel') {
          showToast(_t('toast.reloadBlockedDirty') || '未保存修改已保留，未重新加载外部更改');
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
  } finally {
    if (saveBtn) { saveBtn.disabled = false; saveBtn.classList.remove('btn-loading'); }
    busy(false);
  }
}

function promptSaveConflict() {
  return new Promise(resolve => {
    const modal = $('save-conflict-modal');
    if (!modal) {
      resolve('cancel');
      return;
    }
    let opener = document.activeElement;
    if (!(opener instanceof HTMLElement) || !opener.isConnected || opener === document.body) {
      opener = $('edit-save');
    }
    modal.classList.remove('hidden');
    const reload = $('save-conflict-reload');
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    reload.textContent = `${_t('toolbar.reload') || 'Reload'} (${_t('dialog.dontSave') || 'Do not save'})`;
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
      setTimeout(() => {
        if (opener instanceof HTMLElement && opener.isConnected) opener.focus({ preventScroll: true });
      }, 60);
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
  const sourceName = getActiveTab()?.name
    || String(state.file || '').split(/[\\/]/).pop()
    || state.sourceName
    || 'document';
  const name = String(sourceName).replace(/[\\/]/g, '_');
  const extension = name.match(/\.[^.]+$/)?.[0] || '.md';
  const suggested = name.replace(/\.[^.]+$/, '') + extension;
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
        activeTab.browserCopy = false;
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
        state.browserCopy = false;
        state.sourceName = activeTab.name;
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

async function autoSaveAiCopyTab(tab, options = {}) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const currentTab = tab || (typeof getActiveTab === 'function' ? getActiveTab() : null);
  if (!currentTab) return false;
  const targetDir = currentTab.dir || state.dir;
  if (!targetDir) {
    if (typeof saveAs === 'function') return saveAs();
    return false;
  }
  const content = options.content != null ? options.content : (currentTab.content || state.original || '');
  const targetName = currentTab.name || state.name || 'AI-document.md';
  const sep = targetDir.includes('/') ? '/' : '\\';
  const targetPath = targetDir.replace(/[\\/]+$/, '') + sep + targetName;
  busy(true);
  let res = null;
  try {
    if (hasPy) {
      res = await py.save_file(targetPath, content, 'utf-8', null);
    } else {
      const originPath = currentTab.originPath || (currentTab.dir ? (currentTab.dir.replace(/[\\/]+$/, '') + sep + (state.sourceName || 'document.md')) : (state.file || ''));
      const r = await apiFetch('/api/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: targetPath, file: targetPath, origin_path: originPath, content: content, encoding: 'utf-8' })
      });
      res = await r.json().catch(() => ({ ok: r.ok }));
    }
  } catch (err) {
    showToast((_t('toast.saveFailed') || '保存失败：') + err.message);
    busy(false);
    return false;
  } finally {
    busy(false);
  }
  const isSuccess = Boolean(res && (res === true || res.ok === true || (typeof res === 'object' && res.ok !== false && !res.error)));
  if (isSuccess) {
    state.file = targetPath;
    currentTab.path = targetPath;
    currentTab.mode = 'file';
    currentTab.isVirtual = false;
    currentTab.isDirty = false;
    currentTab.content = content;
    currentTab.original = content;
    currentTab.fixed = content;
    state.content = content;
    state.original = content;
    state.fixed = content;
    if (typeof renderTabsBar === 'function') renderTabsBar();
    showToast((_t('toast.savedPrefix') || '已保存：') + targetPath);
    if (options.exitAfterSave && typeof exitEdit === 'function') exitEdit();
    return true;
  }
  const errMsg = (res && res.error) ? res.error : (_t('toast.saveFailed') || '保存失败');
  showToast((_t('toast.saveFailed') || '保存失败：') + errMsg);
  return false;
}
window.autoSaveAiCopyTab = autoSaveAiCopyTab;
