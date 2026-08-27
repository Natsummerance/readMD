'use strict';
/* ============================================================
   ReadMD Core - History, Welcome & Auto Reload
   ============================================================ */

/* ---------------- 最近文件 ---------------- */

function renderRecentList(list, rec, onOpen) {
  if (!list) return;
  list.innerHTML = '';
  rec.slice(0, 24).forEach(p => {
    const li = document.createElement('li');
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'recent-card';
    const name = String(p).split(/[\\/]/).pop() || p;
    const dir = String(p).slice(0, String(p).length - name.length).replace(/[\\/]+$/, '') || '';
    const nm = document.createElement('span');
    nm.className = 'recent-name'; nm.textContent = name; nm.title = p;
    const dp = document.createElement('span');
    dp.className = 'recent-dir'; dp.textContent = dir; dp.title = p;
    btn.appendChild(nm); btn.appendChild(dp);
    btn.addEventListener('click', e => { e.preventDefault(); onOpen(p); });
    li.appendChild(btn); list.appendChild(li);
  });
}

async function getRecentEntries() {
  if (!hasPy) return [];
  try { return await py.get_recent() || []; } catch (e) { return []; }
}

async function refreshRecent() {
  const box = $('recent-box');
  if (!hasPy) { box.classList.add('hidden'); return; }
  const rec = await getRecentEntries();
  if (!rec.length) { box.classList.add('hidden'); return; }
  box.classList.remove('hidden');
  renderRecentList($('recent-list'), rec, loadFile);
}

async function openHistoryModal() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const rec = await getRecentEntries();
  const modal = $('history-modal');
  const list = $('history-list');
  list.innerHTML = '';
  if (!rec.length) {
    const li = document.createElement('li');
    li.className = 'empty'; li.textContent = _t('history.noRecentFiles') || '暂无最近文件'; list.appendChild(li);
  } else {
    renderRecentList(list, rec, p => { modal.classList.add('hidden'); loadFile(p); });
  }
  modal.classList.remove('hidden');
}

async function clearRecent() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (hasPy) await py.clear_recent();
  await refreshRecent();
  const list = $('history-list');
  if (list) list.innerHTML = '<li class="empty">' + (_t('history.noRecentFiles') || '暂无最近文件') + '</li>';
}


async function addRecent(path) {
  if (hasPy && path) { try { await py.add_recent(path); } catch (e) { /* ignore */ } }
}

/* ---------------- 历史 / 状态 ---------------- */

function normalizePath(p) {
  return String(p || '').replace(/\\/g, '/').toLowerCase();
}

function pushHistory(path) {
  const n = normalizePath(path);
  state.history = state.history.slice(0, state.histIdx + 1);
  if (state.history[state.histIdx] !== n) {
    state.history.push(n);
    state.histIdx = state.history.length - 1;
  }
}

function historyBack() {
  if (state.histIdx > 0) {
    state.histIdx--;
    loadFile(state.history[state.histIdx]);
  }
}

function historyForward() {
  if (state.histIdx < state.history.length - 1) {
    state.histIdx++;
    loadFile(state.history[state.histIdx]);
  }
}

function setUnavailableReason(element, reason) {
  if (!element) return;
  if (element.disabled) {
    if (!element.dataset.actionTitle) element.dataset.actionTitle = element.title || '';
    if (reason) {
      element.title = reason;
      element.setAttribute('aria-description', reason);
    }
    return;
  }
  element.removeAttribute('aria-description');
  if (element.dataset.actionTitle) {
    element.title = element.dataset.actionTitle;
    delete element.dataset.actionTitle;
  }
}

function updateStatus() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const srcLabels = {
    convert: _t('menu.convert'),
    ocr: _t('menu.ocr'),
    url: _t('menu.web'),
    clipboard: _t('menu.clipboard')
  };
  $('status-left').textContent = (state.mode === 'virtual' ? '[' + (srcLabels[state.source] || state.source) + '] ' : '') + (state.sourceName || state.file || '');
  const parts = [];
  if (state.stats) {
    const s = state.stats;
    const p2 = [];
    if (s.table) p2.push(_t('editor.table') + ' ' + s.table);
    if (s.bold) p2.push(_t('editor.bold') + ' ' + s.bold);
    if (s.math) p2.push(_t('editor.formula') + ' ' + s.math);
    if (s.heading) p2.push(_t('editor.h2') + ' ' + s.heading);
    if (s.misc) p2.push(_t('status.fixes') + ' ' + s.misc);
    parts.push(p2.length ? _t('status.fixes') + ' ' + p2.join('、') : _t('status.noFixNeeded'));
  }
  if (state.size) parts.push((state.size / 1024).toFixed(1) + ' KB');
  if (state.encoding) parts.push(state.encoding);
  $('status-right').textContent = parts.join(' · ');
  const isWelcome = state.mode === 'welcome';
  const hasDoc = (state.mode === 'file' || state.mode === 'virtual') && !!state.original;
  const canEdit = hasDoc && !state.editing;
  const canReload = state.mode === 'file';
  const canSaveas = hasDoc && (state.mode === 'virtual' || state.fixed !== '');
  $('btn-edit').disabled = !canEdit && !state.editing;
  setUnavailableReason($('btn-edit'), _t('toast.openDocumentToUse'));
  $('btn-reload').disabled = !canReload;
  $('btn-saveas').disabled = !canSaveas;
  setUnavailableReason($('btn-saveas'), _t('toast.openDocumentToUse'));
  if ($('btn-print')) {
    const browserOnly = !hasPy;
    $('btn-print').disabled = isWelcome || browserOnly;
    const exportHint = browserOnly
      ? '导出需使用桌面版；浏览器模式可另存或打印'
      : '导出文档 (Ctrl+P)';
    $('btn-print').title = exportHint;
    $('btn-print').setAttribute('aria-label', exportHint);
  }
  if ($('btn-a')) $('btn-a').disabled = isWelcome;
  if ($('btn-A')) $('btn-A').disabled = isWelcome;
  if ($('btn-search')) $('btn-search').disabled = isWelcome;
  setUnavailableReason($('btn-search'), _t('toast.searchNeedsDocument'));
  if ($('btn-presentation-menu')) $('btn-presentation-menu').disabled = !hasDoc;
  if ($('btn-run-all-chunks')) $('btn-run-all-chunks').disabled = !hasDoc;
  if ($('btn-share')) $('btn-share').disabled = !hasDoc;
  if ($('btn-fix')) $('btn-fix').disabled = !hasDoc;
  setUnavailableReason($('btn-fix'), _t('toast.openDocumentToUse'));

  const btnHome = $('btn-home');
  if (btnHome) {
    if (isWelcome) btnHome.classList.add('hidden');
    else btnHome.classList.remove('hidden');
  }
}

function goHome() {
  state.mode = 'welcome';
  state.file = null;
  state.sourceName = '';
  state.original = '';
  state.fixed = '';
  state.stats = null;
  state.size = 0;
  state.encoding = '';
  state.editing = false;
  state.activeTabId = null;
  state.headings = [];
  Object.assign(state.pagination, {
    enabled: false,
    mode: 'paged',
    rawContent: null,
    searchText: null,
    pages: [],
    allHeadings: [],
    totalPages: 0,
    currentPage: 0,
  });
  document.title = 'ReadMD';
  setFileTitle('', false);

  if ($('toc')) $('toc').innerHTML = '';
  if ($('outline')) $('outline').innerHTML = '';

  if (state.welcomeHtml) {
    $('content').innerHTML = state.welcomeHtml;
    refreshRecent();
    bindWelcomeEvents();
    if (window.i18n && typeof window.i18n.translateDOM === 'function') {
      window.i18n.translateDOM($('content'));
    }
  }
  const editBar = $('edit-bar'); if (editBar) editBar.classList.add('hidden');
  const editWrap = $('edit-wrap'); if (editWrap) editWrap.classList.add('hidden');
  const pvWrap = $('preview-wrap'); if (pvWrap) pvWrap.classList.add('hidden');
  const pvSplitter = $('pv-splitter'); if (pvSplitter) pvSplitter.classList.add('hidden');
  const content = $('content'); if (content) content.classList.remove('hidden');
  const side = $('side'); if (side) side.classList.add('hidden');
  document.querySelectorAll('#toolbar .tool-btn').forEach(b => b.classList.remove('active'));
  closeSearch();
  closeMdPopups();
  showPaginationBar(false);
  updateStatus();
  renderTabsBar();
}


function bindWelcomeEvents() {
  if ($('w-open')) $('w-open').onclick = () => { loadFileDialog(); };
  if ($('w-folder')) $('w-folder').onclick = openFolder;
  if ($('w-ai')) $('w-ai').onclick = toggleAiPanel;
  if ($('w-convert')) $('w-convert').onclick = openConvertModal;
  if ($('w-web')) $('w-web').onclick = openWebDialog;
  if ($('w-ocr')) $('w-ocr').onclick = () => chooseFile('ocr');
  if ($('recent-clear')) $('recent-clear').onclick = clearRecent;
}



/* ---------------- 自动刷新 ---------------- */

let autoReloadTimer = null;
function startAutoReload() {
  stopAutoReload();
  autoReloadTimer = setInterval(async () => {
    if (!state.autoReload || state.editing) return;
    try {
      for (const tab of [...state.tabs]) {
        if (tab.mode !== 'file' || !tab.path || tab.isDirty || tab.externalChanged) continue;
        const r = await apiFetch('/api/file?p=' + encodeURIComponent(tab.path) + '&meta=1');
        if (!r.ok) continue;
        const d = await r.json();
        if (d.mtime === tab.mtime) continue;
        if (tab.id === state.activeTabId) {
          const sc = $('content')?.scrollTop || 0;
          await loadFile(tab.path, { force: true });
          if (sc) $('content').scrollTop = sc;
        } else {
          tab.externalChanged = true;
          renderTabsBar();
        }
      }
    } catch (e) { /* ignore */ }
  }, 2500);
}
function stopAutoReload() {
  if (autoReloadTimer) clearInterval(autoReloadTimer);
  autoReloadTimer = null;
}

/* ---------------- 工具 ---------------- */

function showToast(msg, ms) {
  const t = $('toast');
  if (window.i18n && typeof msg === 'string') {
    msg = window.i18n.t(msg);
  }
  t.textContent = msg;
  t.classList.remove('hidden');
  clearTimeout(showToast._t);
  showToast._t = setTimeout(() => t.classList.add('hidden'), ms || 2600);
}


function setProgress(p) {
  const el = $('progress');
  el.style.width = p + '%';
  if (p >= 100) setTimeout(() => { el.style.width = '0'; }, 400);
}

function busy(on) {
  state.busyCount = Math.max(0, state.busyCount + (on ? 1 : -1));
  $('busy').classList.toggle('hidden', state.busyCount === 0);
}

function saveLastFile(path) {
  localStorage.setItem('readmd-last', path);
  if (hasPy) {
    try { py.save_settings({ last: path }); } catch (e) { /* ignore */ }
  }
}

function syncBuildVersionLabels() {
  const version = document.documentElement.dataset.version;
  if (!version) return;
  if ($('status-version')) $('status-version').textContent = 'v' + version;
  if ($('menu-version-label')) $('menu-version-label').textContent = '当前版本 v' + version;
}

function afterRender() {
  startModules();
}

function installAssoc() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!hasPy) { showToast(_t('toast.assocBrowserNotice') || '浏览器模式下请在命令行运行 install.bat'); return; }
  py.install_association().then(ok => {
    showToast(ok === true ? (_t('toast.assocSuccess') || '已设置为 .md 默认打开方式') : ((_t('toast.assocFailed') || '注册失败：') + ok));
  });
}

