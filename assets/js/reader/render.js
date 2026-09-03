'use strict';
/* ============================================================
   ReadMD Reader - Document Parsing & Chunked Rendering
   ============================================================ */

/* ---------------- 打开 / 渲染 ---------------- */

function setFileTitle(name, canRename, fullPath) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const el = $('file-title');
  if (!el) return;
  el.textContent = name || '';
  el.disabled = !canRename;
  el.title = canRename ? ((fullPath || name) + '\n' + (_t('reader.clickToRename') || '点击重命名（F2）')) : (name || '');
  el.setAttribute('aria-label', canRename ? ((_t('reader.currentFilePrefix') || '当前文件 ') + name + (_t('reader.clickToRenameSuffix') || '，点击重命名')) : (name || (_t('reader.currentDoc') || '当前文档')));
  if (state.tabs && state.tabs.length > 0) {
    el.classList.add('hidden');
    const tabsCont = $('doc-tabs-container');
    if (tabsCont) tabsCont.classList.remove('hidden');
  } else if (name) {
    el.classList.remove('hidden');
    const tabsCont = $('doc-tabs-container');
    if (tabsCont) tabsCont.classList.add('hidden');
  }
}

function cancelFileRename() {
  const wrap = $('file-rename-wrap');
  if (wrap) wrap.remove();
  const title = $('file-title');
  if (title && (!state.tabs || state.tabs.length === 0)) title.classList.remove('hidden');
}

function openFileRename() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (state.editing) {
    showToast(_t('toast.renameBlockedEdit') || '编辑模式下不可重命名，请先保存或退出编辑');
    return;
  }
  const activeTab = getActiveTab();
  if (activeTab && state.tabs.length > 0) {
    const bar = $('doc-tabs-bar');
    const tabEl = bar ? bar.querySelector(`[data-tab-id="${activeTab.id}"]`) : null;
    if (tabEl) {
      const titleSpan = tabEl.querySelector('.tab-title');
      if (titleSpan) { startTabInlineRename(activeTab, titleSpan, tabEl); return; }
    }
  }
  const title = $('file-title');
  if (!title) return;
  const current = state.file || title.textContent || '';
  if (!current) return;
  const oldName = current.split(/[\\/]/).pop() || '';
  const dot = oldName.lastIndexOf('.');
  const stem = dot > 0 ? oldName.slice(0, dot) : oldName;
  const ext = dot > 0 ? oldName.slice(dot) : '';

  cancelFileRename();
  title.classList.add('hidden');
  const wrap = document.createElement('div');
  wrap.id = 'file-rename-wrap';
  wrap.className = 'file-rename-wrap';
  const input = document.createElement('input');
  input.id = 'file-rename-input';
  input.className = 'file-rename-input';
  input.value = stem;
  input.spellcheck = false;
  input.autocomplete = 'off';
  input.setAttribute('aria-label', _t('tabs.rename') || '文件名称');
  const extension = document.createElement('span');
  extension.id = 'file-rename-ext';
  extension.className = 'file-rename-ext';
  extension.textContent = ext;
  wrap.append(input, extension);
  title.parentNode.insertBefore(wrap, title.nextSibling);

  input.focus();
  input.select();

  let committed = false;
  async function commit() {
    if (committed) return;
    committed = true;
    const nextStem = input.value.trim();
    if (!nextStem || nextStem === stem) { cancelFileRename(); return; }
    if (hasPy && py.rename_file && state.file) {
      try {
        const res = await py.rename_file(state.file, nextStem);
        if (res && res.ok) {
          state.file = res.path;
          state.sourceName = res.name;
          setFileTitle(res.name, true, res.path);
          addRecent(res.path);
          showToast(_t('toast.renamedTo', { name: res.name }) || ('已重命名为：' + res.name));
        } else {
          showToast((_t('toast.renameFailed') || '重命名失败：') + ((res && res.error) || (_t('toast.unknownError') || '未知错误')));
        }
      } catch (e) {
        showToast((_t('toast.renameFailed') || '重命名失败：') + e.message);
      }
    } else {
      setFileTitle(nextStem + ext, true, nextStem + ext);
    }
    cancelFileRename();
  }

  input.addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); commit(); }
    else if (e.key === 'Escape') { e.preventDefault(); cancelFileRename(); }
  });
  input.addEventListener('blur', () => { setTimeout(() => { if (!committed) commit(); }, 150); });
}


async function loadFile(path, { force = false, browserCopy = null } = {}) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!path) return;
  const existingTab = findTabByPath(path);
  const activeEditing = !!(existingTab && state.activeTabId === existingTab.id && state.editing);
  const activeDirty = activeEditing && hasUnsavedEditorChanges();
  if (existingTab && (existingTab.isDirty || activeDirty)) {
    showToast(_t('toast.reloadBlockedDirty') || '未保存修改已保留，未重新加载外部更改');
    return;
  }
  // A second open/double-click of the same clean file is a refresh request.
  // Leave edit mode first so the freshly loaded document is not hidden behind
  // a stale CodeMirror instance.
  if (activeEditing) exitEdit();
  if (existingTab && !force && state.activeTabId !== existingTab.id) {
    await switchTab(existingTab.id);
    if (state.activeTabId !== existingTab.id) return;
  }
  // Opening an already-open clean path is an explicit refresh request.  Start
  // the load only after tab switching, because switching invalidates older
  // document epochs by design.
  const loadEpoch = beginDocumentLoad();
  setProgress(8);
  try {
    const r = await apiFetch('/api/file?p=' + encodeURIComponent(path));
    if (!r.ok) {
      const d = await r.json().catch(() => ({}));
      showToast((_t('toast.openFailed') || '无法打开：') + (d.error || r.status));
      setProgress(0);
      return;
    }
    const d = await r.json();
    if (!isDocumentLoadCurrent(loadEpoch)) return;
    const isBrowserCopy = force && browserCopy === null
      ? existingTab.browserCopy === true
      : browserCopy === true;
    const fileFields = {
      mode: 'file',
      source: 'file',
      browserCopy: isBrowserCopy,
      path: d.path,
      dir: d.dir,
      name: d.name,
      content: d.content,
      original: d.original,
      fixed: d.content,
      title: isBrowserCopy ? `${d.name} (${_t('app.browserCopy') || 'browser copy'})` : d.name,
      fixes: d.fixes || [],
      stats: d.stats || {},
      size: d.size,
      mtime: d.mtime,
      encoding: d.encoding,
      webAssets: [],
      is_code: d.is_code || false,
      code_lang: d.code_lang || '',
      ext: d.ext || '',
    };

    if (existingTab) {
      if (!isDocumentLoadCurrent(loadEpoch)) return;
      const wasActive = state.activeTabId === existingTab.id;
      const previousPage = wasActive && state.pagination.enabled && state.pagination.mode === 'paged'
        ? state.pagination.currentPage
        : 0;
      const previousScroll = wasActive ? ($('content')?.scrollTop || 0) : (existingTab.scrollPos || 0);
      Object.assign(existingTab, fileFields, { isDirty: false });
      if (wasActive) syncStateFromActiveTab();
      if (!wasActive) existingTab.scrollPos = previousScroll;

      if (wasActive) {
        await prepareDocCitations(d.path, d.content);
        if (!isDocumentLoadCurrent(loadEpoch)) return;
        setFixes(d.fixes || [], d.stats || {});
        await renderContent(d.content, d.name);
        if (!isDocumentLoadCurrent(loadEpoch)) return;
        if (state.pagination.enabled && state.pagination.mode === 'paged' && previousPage > 0) {
          renderPage(previousPage, null, true);
        }
        requestAnimationFrame(() => {
          if (isDocumentLoadCurrent(loadEpoch)) $('content').scrollTop = previousScroll;
        });
        updateStatus();
      }
      if (!isDocumentLoadCurrent(loadEpoch)) return;
      showToast(_t('toolbar.reload') + ': ' + d.name);
    } else {
      if (!isDocumentLoadCurrent(loadEpoch)) return;
      const newTab = {
        id: 'tab_' + Date.now() + '_' + Math.random().toString(36).slice(2, 6),
        ...fileFields,
        isDirty: false,
        scrollPos: 0,
        isVirtual: false,
      };
      state.tabs.push(newTab);
      state.activeTabId = newTab.id;
      syncStateFromActiveTab();
      await prepareDocCitations(d.path, d.content);
      if (!isDocumentLoadCurrent(loadEpoch)) return;
      setFixes(d.fixes || [], d.stats || {});
      await renderContent(d.content, d.name);
      if (!isDocumentLoadCurrent(loadEpoch)) return;
      if (state.pagination.enabled && state.pagination.totalPages > 1) {
        showToast(_t('toast.openedPages', { name: d.name, count: state.pagination.totalPages }), 4000);
      } else {
        showToast(_t('toast.opened', { name: d.name }), 4000);
      }
      const displayTitle = isBrowserCopy ? `${d.name} (${_t('app.browserCopy') || 'browser copy'})` : d.name;
      document.title = displayTitle + ' - ReadMD';
      setFileTitle(displayTitle, hasPy, d.path);
      addRecent(d.path);
      pushHistory(d.path);
      saveLastFile(d.path);
      exitEdit();
      clearAiOutput();
      updateStatus();
      setProgress(100);
      if (d.structured) showToast(_t('toast.txtStructureRecognized') || '已智能识别 TXT 结构（标题 / 表格 / 列表 / 目录）');
      renderTabsBar();
      afterRender();
      return;
    }

    renderTabsBar();
    setProgress(100);
  } catch (e) {
    console.error(e);
    showToast((_t('toast.loadFailed') || '加载失败：') + e.message);
    setProgress(0);
  }
}
/* ---------------- 外部唤起（单实例常驻：托盘 / 双击 .md） ---------------- */

async function openExternalFile(path) {
  if (hasPy && py.show_window) {
    try { await py.show_window(); } catch (e) { /* ignore */ }
  }
  if (path) openInitialFile(path);
}
window.openExternalFile = openExternalFile;

let controlPollTimer = null;
async function pollControl() {
  try {
    const r = await apiFetch('/api/control/next');
    const d = await r.json();
    if (d && d.pending) openExternalFile(d.file || '');
  } catch (e) { /* ignore */ }
}
function startControlPoll() {
  stopControlPoll();
  controlPollTimer = setInterval(pollControl, 2000);
}
function stopControlPoll() {
  if (controlPollTimer) clearInterval(controlPollTimer);
  controlPollTimer = null;
}

function setEditBtn(label) {
  const el = $('btn-edit');
  if (!el) return;
  const sp = el.querySelector('span.tb-label');
  if (sp) sp.textContent = label;
  else el.textContent = label;
}

function setFixes(fixes, stats) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  state.fixes = fixes || [];
  state.stats = stats || {};
  const n = state.fixes.length;
  const el = $('btn-fix');
  if (!el) return;
  const sp = el.querySelector('span');
  const lbl = n ? (_t('reader.fixesBtn', { count: n }) || ('修复详情（' + n + '）')) : (_t('reader.fixesBtnSimple') || '修复详情');
  if (sp) sp.textContent = lbl;
  else el.textContent = lbl;
  el.title = n ? (_t('reader.fixesBtnTitle', { count: n }) || ('本次自动修正 ' + n + ' 处')) : (_t('reader.fixesBtnTitleSimple') || '本次自动修正详情');
}


const INCREMENTAL_THRESHOLD = 300 * 1024; // 300KB 以上走增量渲染
const INCREMENTAL_LINES = 6000;
const PAGINATION_THRESHOLD_LINES = 8000;  // 8000 行以上超长文档自动激活智能分页
const PAGINATION_THRESHOLD_BYTES = 500 * 1024; // 500KB 以上超长文档自动激活智能分页

let currentDocCitations = {};

function documentMayCite(content) {
  return /(?:^|\s)\[@[^\]\s]+|(?:^|\s)@[A-Za-z0-9_:-]+/.test(String(content || ''));
}

async function prepareDocCitations(filePath, content) {
  if (!documentMayCite(content)) {
    currentDocCitations = {};
    return;
  }
  await loadDocCitations(filePath);
}

async function loadDocCitations(filePath) {
  currentDocCitations = {};
  if (!filePath) return;
  try {
    if (typeof hasPy !== 'undefined' && hasPy && py.get_bibtex) {
      currentDocCitations = await py.get_bibtex(filePath) || {};
    } else {
      const resp = await fetch('/api/bibtex?p=' + encodeURIComponent(filePath));
      if (resp.ok) {
        const d = await resp.json();
        if (d.ok) currentDocCitations = d.citations || {};
      }
    }
  } catch (e) {
    console.debug('Load bibtex failed:', e);
  }
}

function transformAcademicCallouts(src) {
  if (!src || !src.includes(':::')) return src;
  const calloutMap = {
    theorem: { name: 'Theorem', cls: 'academic-theorem' },
    lemma: { name: 'Lemma', cls: 'academic-lemma' },
    proof: { name: 'Proof', cls: 'academic-proof' },
    definition: { name: 'Definition', cls: 'academic-definition' },
    corollary: { name: 'Corollary', cls: 'academic-corollary' },
    example: { name: 'Example', cls: 'academic-example' },
  };

  const re = /:::\s*(theorem|lemma|proof|definition|corollary|example)(?:\s+\[(.*?)\])?\s*\n([\s\S]*?)\n:::/gi;
  return src.replace(re, (m, type, title, body) => {
    const info = calloutMap[type.toLowerCase()] || { name: type, cls: 'academic-theorem' };
    const titleHtml = title ? `<span class="academic-callout-title">${title}</span>` : '';
    const qed = type.toLowerCase() === 'proof' ? ' <span class="proof-qed">■</span>' : '';
    return `<div class="academic-callout ${info.cls}"><div class="academic-callout-header"><span class="academic-callout-tag">${info.name}</span>${titleHtml}</div><div class="academic-callout-body">${marked.parse(body.trim())}${qed}</div></div>`;
  });
}

/* ---------------- 智能语义分章分页切分算法 ---------------- */

function splitMdIntoPages(md) {
  const lines = String(md || '').split('\n');
  const totalLines = lines.length;
  if (totalLines <= 2000) {
    let t = '';
    for (const l of lines) {
      const m = l.trim().match(/^#{1,3}\s+(.+)$/);
      if (m) { t = m[1].replace(/[*_`#]/g, '').trim(); break; }
    }
    return [{
      pageIndex: 0,
      title: t || '第 1 部分',
      startLine: 1,
      endLine: totalLines,
      content: md,
    }];
  }

  const pages = [];
  let currentLines = [];
  let currentStart = 1;
  let inFence = false;
  let fenceMarker = '';
  let inMath = false;
  let inTable = false;
  let pageChapterTitle = '';

  const TARGET_PAGE_LINES = 1800; // 目标每页行数
  const MIN_PAGE_LINES = 600;     // 触发标题分章的最小行数阈值
  const HARD_MAX_PAGE_LINES = 2600; // 强制分切最大行数

  for (let i = 0; i < totalLines; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    // 1. 代码块围栏跟踪 (``` 或 ~~~)
    if (!inFence && (/^```/.test(trimmed) || /^~~~/.test(trimmed))) {
      inFence = true;
      fenceMarker = trimmed.slice(0, 3);
    } else if (inFence && trimmed.startsWith(fenceMarker)) {
      inFence = false;
      fenceMarker = '';
    }

    // 2. 多行公式环境跟踪
    if (!inFence) {
      if (!inMath && (/^\$\$$/.test(trimmed) || /^\\begin\{(align\*?|aligned|equation\*?|cases|gather\*?|matrix|pmatrix|bmatrix)\}/.test(trimmed))) {
        inMath = true;
      } else if (inMath && (/^\$\$$/.test(trimmed) || /^\\end\{(align\*?|aligned|equation\*?|cases|gather\*?|matrix|pmatrix|bmatrix)\}/.test(trimmed))) {
        inMath = false;
      }
    }

    // 3. 表格行跟踪
    inTable = !inFence && !inMath && trimmed.startsWith('|') && trimmed.endsWith('|');

    // 记录页面主标题（第一遇到的 # / ## 标题）
    if (!pageChapterTitle && !inFence && !inMath && /^#{1,3}\s+(.+)$/.test(trimmed)) {
      const m = trimmed.match(/^#{1,3}\s+(.+)$/);
      if (m) pageChapterTitle = m[1].replace(/[*_`#]/g, '').trim();
    }

    const curLen = currentLines.length;
    const canBreak = !inFence && !inMath && !inTable;

    let shouldBreak = false;

    if (canBreak && curLen >= MIN_PAGE_LINES) {
      // 优先条件 1：遇上 1 级或 2 级标题 (# / ##)
      if (/^#{1,2}\s+/.test(trimmed) && curLen >= MIN_PAGE_LINES) {
        shouldBreak = true;
      }
      // 条件 2：行数达到目标且当前为空行
      else if (curLen >= TARGET_PAGE_LINES && trimmed === '') {
        shouldBreak = true;
      }
      // 条件 3：超过硬上限，在任意空行或 3/4 级标题处分切
      else if (curLen >= HARD_MAX_PAGE_LINES && (trimmed === '' || /^#{1,4}\s+/.test(trimmed))) {
        shouldBreak = true;
      }
    }

    if (shouldBreak && currentLines.length > 0) {
      const pageText = currentLines.join('\n');
      pages.push({
        pageIndex: pages.length,
        title: pageChapterTitle || `第 ${pages.length + 1} 部分`,
        startLine: currentStart,
        endLine: currentStart + currentLines.length - 1,
        content: pageText,
      });
      currentLines = [];
      currentStart = i + 1;
      pageChapterTitle = '';
    }

    currentLines.push(line);
  }

  if (currentLines.length > 0) {
    const pageText = currentLines.join('\n');
    pages.push({
      pageIndex: pages.length,
      title: pageChapterTitle || `第 ${pages.length + 1} 部分`,
      startLine: currentStart,
      endLine: currentStart + currentLines.length - 1,
      content: pageText,
    });
  }

  return pages;
}

function showPaginationBar(show) {
  const bar = $('pagination-bar');
  if (bar) bar.classList.toggle('hidden', !show);
  const stBadge = $('status-pagination');
  if (stBadge) stBadge.classList.toggle('hidden', !show);
}

function updatePaginationBar() {
  const p = state.pagination;
  if (!p || !p.enabled || !p.pages || !p.pages.length) {
    showPaginationBar(false);
    return;
  }
  showPaginationBar(true);

  const cur = p.currentPage;
  const total = p.totalPages;
  const curPage = p.pages[cur] || {};

  const chLabel = $('pg-chapter-label');
  if (chLabel) chLabel.textContent = curPage.title || '';

  const totalLbl = $('pg-total-label');
  if (totalLbl) totalLbl.textContent = `/ ${total}`;

  const btnFirst = $('pg-first-btn');
  if (btnFirst) btnFirst.disabled = (cur === 0 || p.mode === 'continuous');

  const btnPrev = $('pg-prev-btn');
  if (btnPrev) btnPrev.disabled = (cur === 0 || p.mode === 'continuous');

  const btnNext = $('pg-next-btn');
  if (btnNext) btnNext.disabled = (cur >= total - 1 || p.mode === 'continuous');

  const btnLast = $('pg-last-btn');
  if (btnLast) btnLast.disabled = (cur >= total - 1 || p.mode === 'continuous');

  const sel = $('pg-page-select');
  if (sel) {
    sel.disabled = (p.mode === 'continuous');
    const pageSignature = p.pages.map(page => `${page.pageIndex}:${page.title || ''}`).join('|');
    if (sel.options.length !== total || sel.dataset.pageSignature !== pageSignature) {
      sel.dataset.pageSignature = pageSignature;
      sel.innerHTML = '';
      p.pages.forEach((pg, idx) => {
        const opt = document.createElement('option');
        opt.value = idx;
        const num = idx + 1;
        const shortTitle = pg.title ? (pg.title.length > 22 ? pg.title.slice(0, 22) + '…' : pg.title) : `${num}`;
        opt.textContent = `${num}. ${shortTitle}`;
        sel.appendChild(opt);
      });
    }
    sel.value = cur;
  }

  const iconContinuous = $('pg-mode-icon-continuous');
  const iconPaged = $('pg-mode-icon-paged');
  if (iconContinuous && iconPaged) {
    if (p.mode === 'paged') {
      iconContinuous.classList.remove('hidden');
      iconPaged.classList.add('hidden');
    } else {
      iconContinuous.classList.add('hidden');
      iconPaged.classList.remove('hidden');
    }
  }
  $('pg-mode-toggle')?.setAttribute('aria-pressed', p.mode === 'paged' ? 'true' : 'false');

  const _t = (k, params) => window.i18n ? window.i18n.t(k, params) : k;
  const stBadge = $('status-pagination');
  if (stBadge) {
    stBadge.textContent = p.mode === 'paged'
      ? `${cur + 1} / ${total}`
      : (_t('pagination.continuousBadge') || '全卷');
    stBadge.title = p.mode === 'paged'
      ? (_t('pagination.pagedBadge') || '分页模式')
      : (_t('pagination.continuousBadge') || '全卷连续');
  }

  const live = $('pagination-live');
  if (live) {
    live.textContent = p.mode === 'paged'
      ? (_t('pagination.pageInfo', { current: cur + 1, total }) + (curPage.title ? ` · ${curPage.title}` : ''))
      : (_t('pagination.continuousBadge') || '全卷连续');
  }
}

function confirmContinuousMode(pageCount) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  return new Promise(resolve => {
    const modal = $('continuous-modal');
    if (!modal) {
      resolve(window.confirm(_t('pagination.continuousWarning', { count: pageCount }) || `连续模式将一次渲染 ${pageCount} 页，可能造成卡顿。是否继续？`));
      return;
    }
    modal.classList.remove('hidden');
    $('continuous-desc').textContent = _t('pagination.continuousWarning', { count: pageCount });
    const onKeyDown = event => {
      if (event.key === 'Escape') {
        event.preventDefault();
        finish(false);
      }
    };
    const finish = accepted => {
      modal.classList.add('hidden');
      $('continuous-confirm').onclick = null;
      $('continuous-cancel').onclick = null;
      modal.removeEventListener('keydown', onKeyDown);
      resolve(accepted);
    };
    modal.addEventListener('keydown', onKeyDown);
    setTimeout(() => $('continuous-confirm')?.focus(), 20);
    $('continuous-confirm').onclick = () => finish(true);
    $('continuous-cancel').onclick = () => finish(false);
  });
}

async function togglePaginationMode() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = state.pagination;
  if (!p || !p.enabled) return;
  const activeTab = typeof getActiveTab === 'function' ? getActiveTab() : null;
  const navigation = captureNavigationEpoch();

  if (p.mode === 'paged') {
    if (p.pages.length > 20 && !(await confirmContinuousMode(p.pages.length))) {
      return;
    }
    if (!isNavigationCurrent(navigation)) return;
    p.mode = 'continuous';
    if (activeTab) {
      activeTab.readerMode = 'continuous';
      activeTab.continuousScroll = $('content')?.scrollTop || 0;
    }
    showToast(_t('pagination.switchToContinuousToast') || '已切换至全卷连续阅读模式', 1800);
    await renderContentIncremental(p.rawContent, 0, beginReaderRender());
  } else {
    p.mode = 'paged';
    if (activeTab) {
      activeTab.readerMode = 'paged';
      activeTab.readerPage = 0;
    }
    showToast(_t('pagination.switchToPagedToast') || '已切换至智能分页阅读模式', 1800);
    beginReaderRender();
    renderPage(0, null, false);
  }
  updatePaginationBar();
}

function renderPage(pageIndex, targetHeadingId, preserveScroll) {
  if (!state.pagination.pages || !state.pagination.pages.length) return;
  pageIndex = Math.max(0, Math.min(pageIndex, state.pagination.pages.length - 1));
  state.pagination.currentPage = pageIndex;
  const activeTab = typeof getActiveTab === 'function' ? getActiveTab() : null;
  if (activeTab) {
    activeTab.readerMode = 'paged';
    activeTab.readerPage = pageIndex;
  }
  const page = state.pagination.pages[pageIndex];

  const el = $('content');
  if (!el) return;

  const transformed = transformAcademicCallouts(page.content);
  const prot = protectMath(transformed);
  const html = marked.parse(prot.src, { gfm: true, breaks: false });
  const finalHtml = restoreMath(html, prot.saved);
  el.innerHTML = '<article class="markdown-body">' + sanitizeRenderedHtml(finalHtml) + '</article>';

  if (state.pagination.allHeadings?.length) {
    const pageOutline = state.pagination.allHeadings.filter(heading => heading.pageIndex === pageIndex);
    const pageHeadings = el.querySelectorAll('.markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4, .markdown-body h5, .markdown-body h6');
    const outlineByLine = new Map(pageOutline.map(heading => [String(heading.sourceLine), heading]));
    let outlineCursor = 0;
    pageHeadings.forEach(heading => {
      const outline = outlineByLine.get(heading.dataset.sourceLine) || pageOutline[outlineCursor];
      if (outline) {
        heading.id = outline.id;
        outlineCursor += 1;
      }
    });
  }

  postProcess();
  updatePaginationBar();
  updateStatus();

  if (targetHeadingId) {
    requestAnimationFrame(() => {
      let targetEl = document.getElementById(targetHeadingId);
      if (!targetEl) {
        try {
          const dec = decodeURIComponent(targetHeadingId);
          targetEl = document.getElementById(dec) || el.querySelector('[name="' + CSS.escape(dec) + '"]');
        } catch (e) {}
      }
      if (targetEl) {
        targetEl.tabIndex = -1;
        targetEl.scrollIntoView({ behavior: preferredScrollBehavior(), block: 'start' });
        targetEl.focus({ preventScroll: true });
        document.querySelectorAll('#toc-list .toc-heading-active')
          .forEach(link => link.classList.remove('toc-heading-active'));
        const activeTocLink = document.querySelector(`#toc-list [data-heading-id="${CSS.escape(targetEl.id)}"]`);
        const targetGroup = document.querySelector(`#toc-list details[data-page-idx="${pageIndex}"]`);
        if (targetGroup && !targetGroup.open) targetGroup.open = true;
        if (activeTocLink) activeTocLink.classList.add('toc-heading-active');
        targetEl.classList.remove('heading-target-highlight');
        targetEl.classList.remove('search-arrival');
        void targetEl.offsetWidth;
        targetEl.classList.add('heading-target-highlight');
        targetEl.classList.add('search-arrival');
        setTimeout(() => {
          targetEl.classList.remove('heading-target-highlight');
          targetEl.classList.remove('search-arrival');
        }, 2400);
      } else {
        el.scrollTop = 0;
      }
    });
  } else if (!preserveScroll) {
    el.scrollTop = 0;
  }
}

let paginationEventsBound = false;
function initPaginationEvents() {
  if (paginationEventsBound) return;
  paginationEventsBound = true;

  const content = $('content');
  if (content) {
    let tocScrollFrame = 0;
    content.addEventListener('scroll', () => {
      if (tocScrollFrame) return;
      tocScrollFrame = requestAnimationFrame(() => {
        tocScrollFrame = 0;
        updateActiveTocHeading();
        const activeTab = typeof getActiveTab === 'function' ? getActiveTab() : null;
        if (activeTab) {
          activeTab.scrollPos = content.scrollTop || 0;
          const scrollKey = normalizePath(activeTab.title || activeTab.name || activeTab.path || '');
          if (scrollKey) state.scrollPos[scrollKey] = activeTab.scrollPos;
          if (state.pagination.enabled && state.pagination.mode === 'continuous') {
            activeTab.continuousScroll = content.scrollTop || 0;
          }
        }
      });
    });
  }

  const btnFirst = $('pg-first-btn');
  if (btnFirst) btnFirst.addEventListener('click', () => { if (state.pagination.enabled && state.pagination.mode === 'paged') renderPage(0); });

  const btnPrev = $('pg-prev-btn');
  if (btnPrev) btnPrev.addEventListener('click', () => { if (state.pagination.enabled && state.pagination.mode === 'paged') renderPage(state.pagination.currentPage - 1); });

  const btnNext = $('pg-next-btn');
  if (btnNext) btnNext.addEventListener('click', () => { if (state.pagination.enabled && state.pagination.mode === 'paged') renderPage(state.pagination.currentPage + 1); });

  const btnLast = $('pg-last-btn');
  if (btnLast) btnLast.addEventListener('click', () => { if (state.pagination.enabled && state.pagination.mode === 'paged') renderPage(state.pagination.totalPages - 1); });

  const sel = $('pg-page-select');
  if (sel) sel.addEventListener('change', e => {
    if (state.pagination.enabled && state.pagination.mode === 'paged') {
      const idx = parseInt(e.target.value, 10);
      if (!isNaN(idx)) renderPage(idx);
    }
  });

  const toggleBtn = $('pg-mode-toggle');
  if (toggleBtn) toggleBtn.addEventListener('click', togglePaginationMode);

  const stBadge = $('status-pagination');
  if (stBadge) {
    stBadge.addEventListener('click', togglePaginationMode);
    stBadge.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); togglePaginationMode(); } });
  }

  // 快捷键支持 (Alt + ArrowLeft/Right/Home/End)
  window.addEventListener('keydown', e => {
    if (state.editing || !state.pagination.enabled || state.pagination.mode !== 'paged') return;
    if (e.altKey && !e.ctrlKey && !e.shiftKey && !e.metaKey) {
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        renderPage(state.pagination.currentPage - 1);
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        renderPage(state.pagination.currentPage + 1);
      } else if (e.key === 'Home') {
        e.preventDefault();
        renderPage(0);
      } else if (e.key === 'End') {
        e.preventDefault();
        renderPage(state.pagination.totalPages - 1);
      }
    }
  });
}
window.initPaginationEvents = initPaginationEvents;
window.splitMdIntoPages = splitMdIntoPages;
window.renderPage = renderPage;
window.togglePaginationMode = togglePaginationMode;

async function processDocImports(mdText, filePath) {
  if (!mdText || !/@import\s+["']/.test(mdText)) return mdText;
  const dir = filePath ? filePath.replace(/[^\\/]+$/, '') : '';
  try {
    if (hasPy && py.process_imports) {
      const res = await py.process_imports(mdText, dir, filePath);
      return (res && res.ok) ? res.content : mdText;
    } else {
      const r = await apiFetch('/api/import/process', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content: mdText, base_dir: dir, current_file: filePath })
      });
      const d = await r.json();
      return (d && d.ok) ? d.content : mdText;
    }
  } catch (e) {
    return mdText;
  }
}
window.processDocImports = processDocImports;

function parseMarkdownWithSourceMap(content, options = {}) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const breaks = !!(state && state.breakOnSingleNewline);
  try {
    const renderer = new marked.Renderer();

    // 1. 分词并记录顶级 AST 节点的源码起始行号
    const tokens = marked.lexer(content, { gfm: true, breaks: breaks });
    let lineCursor = 1;
    tokens.forEach(tok => {
      tok.sourceLine = lineCursor;
      if (tok.raw) {
        lineCursor += tok.raw.split('\n').length - 1;
      }
    });

    // 2. 自定义渲染器注入 data-source-line 属性并正确解析 inline/nested 元素
    renderer.heading = function(token) {
      const text = (token && token.tokens && this.parser) ? this.parser.parseInline(token.tokens) : (token ? (token.text || '') : (arguments[0] || ''));
      const level = (token && token.depth) ? token.depth : (arguments[1] || 1);
      const lineAttr = (token && token.sourceLine) ? ` data-source-line="${token.sourceLine}"` : '';
      return `<h${level}${lineAttr}>${text}</h${level}>\n`;
    };

    renderer.paragraph = function(token) {
      const text = (token && token.tokens && this.parser) ? this.parser.parseInline(token.tokens) : (token ? (token.text || '') : (arguments[0] || ''));
      const lineAttr = (token && token.sourceLine) ? ` data-source-line="${token.sourceLine}"` : '';
      return `<p${lineAttr}>${text}</p>\n`;
    };

    renderer.blockquote = function(token) {
      const body = (token && token.tokens && this.parser) ? this.parser.parse(token.tokens) : (token ? (token.text || '') : (arguments[0] || ''));
      const lineAttr = (token && token.sourceLine) ? ` data-source-line="${token.sourceLine}"` : '';
      return `<blockquote${lineAttr}>\n${body}</blockquote>\n`;
    };

    renderer.table = function(token) {
      const html = marked.Renderer.prototype.table.call(this, token);
      const lineAttr = (token && token.sourceLine) ? ` data-source-line="${token.sourceLine}"` : '';
      if (lineAttr && html && html.startsWith('<table')) {
        return '<table' + lineAttr + html.slice(6);
      }
      return html;
    };

    renderer.code = function(token) {
      const code = typeof token === 'object' ? token.text : arguments[0];
      const infostring = typeof token === 'object' ? token.lang : arguments[1];
      const escaped = typeof token === 'object' ? token.escaped : arguments[2];
      const lineAttr = (typeof token === 'object' && token.sourceLine) ? ` data-source-line="${token.sourceLine}"` : '';
      const info = (infostring || '').trim();
      // Parse the same small attribute vocabulary used by MPE's fenced code
      // chunks.  The old implementation searched the raw info string, which
      // missed brace syntax (`{cmd output=markdown}`), quoted values and
      // command aliases (`cmd=python`).  Keep the parser deliberately
      // conservative: attributes only affect presentation/format selection;
      // execution still goes through the existing sandbox endpoint.
      const fenceTokens = info
        .replace(/[{}]/g, ' ')
        .match(/(?:[^\s"']+|"[^"]*"|'[^']*')+/g) || [];
      const lang = fenceTokens.shift() || '';
      const attributes = Object.create(null);
      const flags = Object.create(null);
      fenceTokens.forEach(rawToken => {
        const tokenText = String(rawToken).trim();
        if (!tokenText) return;
        const equalAt = tokenText.indexOf('=');
        if (equalAt < 0) {
          flags[tokenText.toLowerCase()] = true;
          return;
        }
        const key = tokenText.slice(0, equalAt).trim().toLowerCase();
        let value = tokenText.slice(equalAt + 1).trim();
        if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
          value = value.slice(1, -1);
        }
        attributes[key] = value;
      });
      const boolAttribute = (name, fallback = false) => {
        if (Object.prototype.hasOwnProperty.call(attributes, name)) {
          const value = String(attributes[name]).toLowerCase();
          return value === 'true' || value === '1' || value === 'yes' || value === 'on';
        }
        return Boolean(flags[name]) || fallback;
      };
      const commandAttribute = Object.prototype.hasOwnProperty.call(attributes, 'cmd')
        ? String(attributes.cmd).trim()
        : '';
      const commandDisabled = ['false', '0', 'no', 'off'].includes(commandAttribute.toLowerCase());
      const hasCmd = !commandDisabled && (boolAttribute('cmd') || Boolean(commandAttribute) || Boolean(flags.cmd));
      const executionLang = commandAttribute && !['true', '1', 'yes', 'on'].includes(commandAttribute.toLowerCase())
        ? commandAttribute
        : lang;

      // 1. Interactive Code Chunk
      if (hasCmd) {
        const encodedCode = encodeURIComponent(code);
        const isMatplotlib = boolAttribute('matplotlib');
        const isHidden = boolAttribute('hide') || (Object.prototype.hasOwnProperty.call(attributes, 'echo') && ['false', '0', 'no', 'off'].includes(String(attributes.echo).toLowerCase()));
        const outputValue = Object.prototype.hasOwnProperty.call(attributes, 'output') ? String(attributes.output).toLowerCase() : '';
        const hasOutput = ['true', '1', 'yes', 'on', 'text', 'markdown', 'html', 'json', 'png'].includes(outputValue) || boolAttribute('output');
        const outputFormat = hasOutput && outputValue && !['true', '1', 'yes', 'on'].includes(outputValue) ? outputValue : '';
        return `<div class="code-chunk-card" ${lineAttr} data-lang="${executionLang}" data-code="${encodedCode}" data-matplotlib="${isMatplotlib}" data-hide="${isHidden}" data-echo="${!isHidden}" data-output="${hasOutput}" data-output-format="${outputFormat}">
          <div class="code-chunk-header">
            <span class="code-chunk-badge">${executionLang.toUpperCase()}</span>
            <span class="code-chunk-status" role="status" aria-live="polite">${_t('status.ready')}</span>
            <span class="code-chunk-timer"></span>
            <div class="code-chunk-actions">
              <button class="code-chunk-run-btn" title="${_t('menu.runCode')} (Shift+Enter)" aria-label="${_t('menu.runCode')}">▶ ${_t('menu.runCode')}</button>
            </div>
          </div>
          <div class="code-chunk-src ${isHidden ? 'hidden' : ''}">
            <pre><code class="language-${executionLang}">${escaped ? code : (window.escapeHtml ? escapeHtml(code) : code)}</code></pre>
          </div>
          <div class="code-chunk-output hidden">
            <div class="code-chunk-output-header">
              <span>${_t('reader.executionOutput')}</span>
              <div class="code-chunk-out-actions">
                <button class="code-chunk-copy-btn" title="${_t('reader.copyOutput')}" aria-label="${_t('reader.copyOutput')}">${_t('reader.copyOutput')}</button>
                <button class="code-chunk-clear-btn" title="${_t('reader.clearOutput')}" aria-label="${_t('reader.clearOutput')}">${_t('reader.clearOutput')}</button>
              </div>
            </div>
            <pre class="code-chunk-stdout"></pre>
            <div class="code-chunk-plot"></div>
          </div>
        </div>\n`;
      }

      // 2. Specialized Diagrams
      const diagramLangs = ['mermaid', 'tikz', 'plantuml', 'puml', 'wsd', 'wavedrom', 'bitfield', 'viz', 'dot', 'graphviz', 'vega', 'vega-lite', 'chart', 'chartjs', 'chart.js', 'd2', 'ditaa'];
      if (diagramLangs.includes(lang.toLowerCase())) {
        const encodedCode = encodeURIComponent(code);
        return `<div class="diagram-card" ${lineAttr} data-diagram-engine="${lang.toLowerCase()}" data-diagram-code="${encodedCode}">
          <div class="diagram-header">
            <span class="diagram-badge">${_t('reader.diagramBadge', { lang: lang.toUpperCase() })}</span>
            <button class="diagram-reload-btn" title="${_t('reader.refresh')}" aria-label="${_t('reader.refresh')}">⟳ ${_t('reader.refresh')}</button>
          </div>
          <div class="diagram-preview"><div class="diagram-loading">${_t('reader.diagramLoading')}</div></div>
          <details class="diagram-src-wrap"><summary>${_t('reader.viewCode')}</summary><pre><code class="language-${lang}">${escaped ? code : (window.escapeHtml ? escapeHtml(code) : code)}</code></pre></details>
        </div>\n`;
      }

      // 3. Standard Code Block
      return `<pre ${lineAttr}><code class="language-${lang}">${escaped ? code : (window.escapeHtml ? escapeHtml(code) : code)}</code></pre>\n`;
    };

    return marked.parser(tokens, { renderer: renderer, gfm: true, breaks: breaks });
  } catch (e) {
    return marked.parse(content, { gfm: true, breaks: breaks });
  }
}
window.parseMarkdownWithSourceMap = parseMarkdownWithSourceMap;

const MARKDOWN_ALLOWED_TAGS = new Set([
  'a', 'abbr', 'article', 'b', 'blockquote', 'br', 'caption', 'cite', 'code',
  'dd', 'del', 'details', 'div', 'dl', 'dt', 'em', 'figcaption', 'figure',
  'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'hr', 'i', 'img', 'input', 'ins',
  'button', 'kbd', 'li', 'mark', 'ol', 'p', 'pre', 'q', 's', 'section', 'span',
  'strike', 'strong', 'sub', 'summary', 'sup', 'table', 'tbody', 'td',
  'tfoot', 'th', 'thead', 'time', 'tr', 'u', 'ul',
  // Safe, presentation-only SVG primitives emitted by the offline diagram
  // engines.  Script/event attributes are still rejected below.
  'svg', 'g', 'path', 'rect', 'circle', 'ellipse', 'line', 'polyline',
  'polygon', 'text', 'tspan', 'defs', 'marker', 'clippath', 'use',
]);
const MARKDOWN_REMOVED_TAGS = new Set([
  'base', 'embed', 'form', 'frame', 'frameset', 'iframe', 'link', 'meta',
  'noscript', 'object', 'script', 'style', 'template', 'title',
]);
const RENDER_BUTTON_CLASSES = new Set([
  'code-chunk-run-btn',
  'code-chunk-copy-btn',
  'code-chunk-clear-btn',
  'diagram-reload-btn',
]);
const MARKDOWN_RESERVED_IDS = new Set([
  'content', 'toolbar', 'statusbar', 'doc-tabs-container', 'doc-tabs-bar',
  'ai-output', 'update-notes-content', 'presentation-modal', 'share-modal',
]);
const MARKDOWN_RESERVED_CLASS_RE = /^(?:modal|modal-dialog|modal-header|modal-footer|drag-overlay|tab-item|tab-close|zen-mode|zen-entering|presentation-modal|presentation-iframe)$/;
let readerRenderEpoch = 0;
let readerRenderAborter = null;
let documentLoadEpoch = 0;

function beginReaderRender() {
  const epoch = ++readerRenderEpoch;
  if (readerRenderAborter) readerRenderAborter.abort();
  readerRenderAborter = new AbortController();
  return { epoch, signal: readerRenderAborter.signal };
}

function isReaderRenderCurrent(render) {
  return Boolean(render && render.epoch === readerRenderEpoch && !render.signal.aborted);
}

function beginDocumentLoad() {
  return ++documentLoadEpoch;
}

function invalidateDocumentLoads() {
  documentLoadEpoch += 1;
}

function isDocumentLoadCurrent(loadEpoch) {
  return loadEpoch === documentLoadEpoch;
}

function captureNavigationEpoch() {
  return { document: documentLoadEpoch, render: readerRenderEpoch };
}

function isNavigationCurrent(epoch) {
  return Boolean(epoch && epoch.document === documentLoadEpoch && epoch.render === readerRenderEpoch);
}
window.invalidateDocumentLoads = invalidateDocumentLoads;

function isSanctionedRenderButton(node) {
  const classes = Array.from(node.classList);
  return Boolean(node.closest('.code-chunk-card, .diagram-card'))
    && classes.length > 0
    && classes.every(className => RENDER_BUTTON_CLASSES.has(className));
}

function safeResourceUrl(value) {
  if (!value) return false;
  const trimmed = String(value).trim();
  if (trimmed.startsWith('#')) return true;
  if (/^(data|blob|javascript|vbscript):/i.test(trimmed)) return /^data:image\//i.test(trimmed);
  try {
    return ['http:', 'https:', 'mailto:', 'file:'].includes(new URL(trimmed, window.location.href).protocol);
  } catch (_) {
    return false;
  }
}

function sanitizeRenderedHtml(html, { allowInteractive = true } = {}) {
  const template = document.createElement('template');
  template.innerHTML = String(html || '');
  Array.from(template.content.querySelectorAll('*')).forEach(node => {
    const tag = node.tagName.toLowerCase();
    if (!allowInteractive && (tag === 'button' ||
        node.classList.contains('code-chunk-card') || node.classList.contains('diagram-card'))) {
      node.replaceWith(...node.childNodes);
      return;
    }
    if (MARKDOWN_REMOVED_TAGS.has(tag)) {
      node.remove();
      return;
    }
    if (!MARKDOWN_ALLOWED_TAGS.has(tag)) {
      node.replaceWith(...node.childNodes);
      return;
    }
    if (tag === 'button') {
      if (!isSanctionedRenderButton(node)) node.replaceWith(...node.childNodes);
    }
    if (tag === 'img') {
      const source = node.getAttribute('src') || '';
      if (/^https?:/i.test(source)) {
        const link = document.createElement('a');
        link.href = source;
        link.rel = 'noopener noreferrer';
        link.target = '_blank';
        link.className = 'remote-image-link';
        link.textContent = node.getAttribute('alt')
          || `[${new URL(source).hostname}]`;
        node.replaceWith(link);
        return;
      }
    }

    Array.from(node.attributes).forEach(attribute => {
      const name = attribute.name.toLowerCase();
      const value = attribute.value;
      if (name === 'id' && MARKDOWN_RESERVED_IDS.has(value)) {
        node.removeAttribute(attribute.name);
        return;
      }
      if (name === 'class' && value.split(/\s+/).some(className => MARKDOWN_RESERVED_CLASS_RE.test(className))) {
        node.removeAttribute(attribute.name);
        return;
      }
      if (name === 'role' || name === 'aria-hidden' || name === 'aria-live') {
        node.removeAttribute(attribute.name);
        return;
      }
      if (name.startsWith('data-') || name === 'class' || name.startsWith('aria-') ||
          ['title', 'lang', 'dir', 'alt', 'width', 'height', 'loading',
           'colspan', 'rowspan', 'datetime', 'cite'].includes(name)) return;
      if (tag === 'svg' && ['xmlns', 'viewbox', 'preserveaspectratio', 'role'].includes(name)) return;
      if (['g', 'path', 'rect', 'circle', 'ellipse', 'line', 'polyline', 'polygon', 'text', 'tspan', 'defs', 'marker', 'clippath', 'use'].includes(tag) &&
          ['d', 'fill', 'fill-rule', 'fill-opacity', 'stroke', 'stroke-width', 'stroke-linecap',
           'stroke-linejoin', 'stroke-opacity', 'transform', 'x', 'y', 'x1', 'x2', 'y1', 'y2',
           'cx', 'cy', 'r', 'rx', 'ry', 'points', 'dx', 'dy', 'font-size', 'font-family',
           'text-anchor', 'marker-end', 'marker-start', 'markerwidth', 'markerheight',
           'refx', 'refy', 'orient', 'viewbox', 'width', 'height', 'clip-path', 'id',
           'href', 'xlink:href'].includes(name)) {
        if (['href', 'xlink:href'].includes(name) && !value.startsWith('#')) node.removeAttribute(attribute.name);
        return;
      }
      if (name === 'id') {
        if (!/^[A-Za-z][A-Za-z0-9_:.-]*$/.test(value)) node.removeAttribute(attribute.name);
        return;
      }
      if ((tag === 'a' && name === 'href') || ((tag === 'img' || tag === 'source') && name === 'src')) {
        if (!safeResourceUrl(value)) node.removeAttribute(attribute.name);
        return;
      }
      if (name === 'srcset') {
        const urls = value.split(',').map(item => item.trim().split(/\s+/)[0]).filter(Boolean);
        if (!urls.length || !urls.every(safeResourceUrl)) node.removeAttribute(attribute.name);
        return;
      }
      if (tag === 'input' && ['type', 'checked', 'disabled'].includes(name)) {
        if (name === 'type' && value.toLowerCase() !== 'checkbox') node.removeAttribute(attribute.name);
        return;
      }
      if ((tag === 'ol' && ['start', 'type'].includes(name)) ||
          (tag === 'details' && name === 'open') || (tag === 'a' && name === 'target')) return;
      node.removeAttribute(attribute.name);
    });

    if (tag === 'a') {
      const href = node.getAttribute('href');
      if (!href) node.removeAttribute('target');
      else if (/^https?:/i.test(href)) node.setAttribute('rel', 'noopener noreferrer');
      else if (node.classList.contains('remote-image-link')) node.setAttribute('rel', 'noopener noreferrer');
    }
    if (tag === 'input' && !node.hasAttribute('disabled')) node.setAttribute('disabled', '');
    if (tag === 'button') {
      if (isSanctionedRenderButton(node)) node.setAttribute('type', 'button');
    }
  });

  return template.innerHTML;
}
window.sanitizeRenderedHtml = sanitizeRenderedHtml;

function renderSafeMarkdown(source, { breaks = false } = {}) {
  const prot = protectMath(String(source || ''));
  const html = marked.parse(prot.src, { gfm: true, breaks });
  return sanitizeRenderedHtml(restoreMath(html, prot.saved), { allowInteractive: false });
}
window.renderSafeMarkdown = renderSafeMarkdown;

async function renderContent(content, name) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const render = beginReaderRender();
  const saved = state.scrollPos[normalizePath(name || state.file || '')] || 0;

  // 预处理 @import
  if (content && /@import\s+["']/.test(content)) {
    try {
      content = await processDocImports(content, state.file || name || '');
    } catch (e) {}
  }
  if (!isReaderRenderCurrent(render)) return;

  const linesCount = (content || '').split('\n').length;
  const isUltraLong = linesCount > PAGINATION_THRESHOLD_LINES || (content || '').length > PAGINATION_THRESHOLD_BYTES;

  if (isUltraLong) {
    state.pagination.enabled = true;
    state.pagination.rawContent = content;
    state.pagination.searchText = null;
    state.pagination.pages = splitMdIntoPages(content);
    state.pagination.totalPages = state.pagination.pages.length;
    if (state.pagination.mode === 'paged') {
      renderPage(0, null, false);
      showPaginationBar(true);
      return;
    } else {
      await renderContentIncremental(content, saved, render);
      showPaginationBar(true);
      return;
    }
  } else {
    state.pagination.enabled = false;
    state.pagination.rawContent = null;
    state.pagination.pages = [];
    state.pagination.totalPages = 0;
    showPaginationBar(false);
  }

  const isCodeDoc = state.is_code || (name && !MD_RE.test(name) && typeof TEXT_CODE_RE !== 'undefined' && TEXT_CODE_RE.test(name));
  if (isCodeDoc) {
    const ext = state.ext || (name ? (name.match(/\.[^.]+$/) || [''])[0] : '');
    const lang = state.code_lang || ext.replace(/^\./, '');
    const sizeKb = ((content || '').length / 1024).toFixed(1);

    const headerHtml = `
      <div class="code-doc-header">
        <div class="code-doc-meta">
          <span class="code-doc-badge">${escapeHtml(lang || 'code')}</span>
          <span class="code-doc-path">${escapeHtml(name || '')}</span>
          <span class="code-doc-lines" data-i18n="codebar.lines">${_t('codebar.lines', { count: linesCount }) || (linesCount + ' 行')}</span>
          <span class="code-doc-size">${sizeKb} KB</span>
        </div>
        <div class="code-doc-actions">
          <button class="btn btn-sm btn-primary" id="btn-code-to-md" data-i18n="codebar.aiToMd" title="转换为结构化 Markdown 文档">${_t('codebar.aiToMd') || 'AI 结构化转 MD'}</button>
          <button class="btn btn-sm" id="btn-code-edit" data-i18n="codebar.edit" title="进入源码编辑器 (Ctrl+E)">${_t('codebar.edit') || '编辑源码 (Ctrl+E)'}</button>
          <button class="btn btn-sm" id="btn-code-ai-explain" data-i18n="codebar.aiExplain" title="调用 AI 进行深度解析与排错">${_t('codebar.aiExplain') || 'AI 深度解析'}</button>
          <button class="btn btn-sm" id="btn-code-copy" data-i18n="codebar.copyCode" title="复制代码正文">${_t('codebar.copyCode') || '复制代码'}</button>
        </div>
      </div>`;

    const codeBlockMarkdown = '```' + (lang || '') + '\n' + content + '\n```';
    const prot = protectMath(codeBlockMarkdown);
    const html = parseMarkdownWithSourceMap(prot.src);
    const finalHtml = restoreMath(html, prot.saved);
    if (!isReaderRenderCurrent(render)) return;
    $('content').innerHTML = '<article class="markdown-body">' + headerHtml + sanitizeRenderedHtml(finalHtml) + '</article>';
    postProcess();
    bindCodeDocActions(content, name, lang);
    if (saved) requestAnimationFrame(() => {
      if (isReaderRenderCurrent(render)) $('content').scrollTop = saved;
    });
    return;
  }

  const big = content.length > INCREMENTAL_THRESHOLD || linesCount > INCREMENTAL_LINES;
  if (big) {
    await renderContentIncremental(content, saved, render);
    return;
  }
  const transformed = transformAcademicCallouts(content);
  const prot = protectMath(transformed);
  const html = parseMarkdownWithSourceMap(prot.src);
  const finalHtml = restoreMath(html, prot.saved);
  if (!isReaderRenderCurrent(render)) return;
  $('content').innerHTML = '<article class="markdown-body">' + sanitizeRenderedHtml(finalHtml) + '</article>';
  postProcess();
  if (saved) requestAnimationFrame(() => {
    if (isReaderRenderCurrent(render)) $('content').scrollTop = saved;
  });
}

function bindCodeDocActions(content, name, lang) {
  const toMdBtn = $('btn-code-to-md');
  const editBtn = $('btn-code-edit');
  const aiBtn = $('btn-code-ai-explain');
  const copyBtn = $('btn-code-copy');
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;

  if (toMdBtn) {
    toMdBtn.addEventListener('click', async () => {
      if (typeof openAiPanelWithPrompt === 'function') {
        openAiPanelWithPrompt('code_to_doc', '', content);
      } else if (state.file) {
        await convertOrOcr(state.file, 'convert');
      }
    });
  }
  if (editBtn) {
    editBtn.addEventListener('click', () => {
      if (typeof enterEdit === 'function') enterEdit();
    });
  }
  if (aiBtn) {
    aiBtn.addEventListener('click', () => {
      if (typeof openAiPanelWithPrompt === 'function') {
        openAiPanelWithPrompt('code_analysis', '', content);
      }
    });
  }
  if (copyBtn) {
    copyBtn.addEventListener('click', async () => {
      try {
        await navigator.clipboard.writeText(content);
        showToast(_t('toast.copied') || '已复制代码内容');
      } catch (e) {
        showToast(_t('toast.copyFailed') || '复制失败');
      }
    });
  }
}


/* 大文档分块：优先按围栏代码块 / 空行切块，超长块按行硬切 */
function splitMdBlocks(md) {
  const lines = String(md || '').split('\n');
  const blocks = [];
  let buf = [];
  let inFence = false;
  const flush = () => {
    if (buf.length) { blocks.push(buf.join('\n')); buf = []; }
  };
  for (const line of lines) {
    const t = line.trim();
    if (!inFence && /^```/.test(t)) {
      flush();
      inFence = true;
      buf.push(line);
      continue;
    }
    if (inFence) {
      buf.push(line);
      if (t.startsWith('```')) { flush(); inFence = false; }
      continue;
    }
    if (t === '') {
      flush();
      continue;
    }
    buf.push(line);
  }
  flush();
  if (inFence && buf.length) blocks.push(buf.join('\n'));
  const MAX_BLOCK_LINES = 200;
  const out = [];
  for (const b of blocks) {
    const ls = b.split('\n');
    if (ls.length <= MAX_BLOCK_LINES) { out.push(b); continue; }
    for (let i = 0; i < ls.length; i += MAX_BLOCK_LINES) {
      out.push(ls.slice(i, i + MAX_BLOCK_LINES).join('\n'));
    }
  }
  return out;
}

async function renderContentIncremental(content, savedTop, render = null) {
  const task = render || beginReaderRender();
  if (!isReaderRenderCurrent(task)) return;
  const el = $('content');
  el.innerHTML = '<article class="markdown-body"></article>';
  const body = el.querySelector('.markdown-body');
  const blocks = splitMdBlocks(content);
  const total = blocks.length;
  let prog = null;
  try {
    if (total <= 1) {
      const prot = protectMath(content);
      body.innerHTML = sanitizeRenderedHtml(restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved));
      postProcess();
      if (savedTop) el.scrollTop = savedTop;
      return;
    }
    prog = document.createElement('div');
    prog.id = 'render-progress';
    prog.setAttribute('role', 'status');
    prog.setAttribute('aria-live', 'polite');
    prog.setAttribute('aria-atomic', 'true');
    el.appendChild(prog);
    const CHUNK = 8;
    for (let i = 0; i < total; i += CHUNK) {
      if (!isReaderRenderCurrent(task)) return;
      const frag = document.createDocumentFragment();
      const end = Math.min(i + CHUNK, total);
      for (let k = i; k < end; k++) {
        const div = document.createElement('div');
        const prot = protectMath(blocks[k]);
        div.innerHTML = sanitizeRenderedHtml(restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved));
        frag.appendChild(div);
      }
      body.appendChild(frag);
      const pct = Math.round((end / total) * 100);
      const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
      if (pct >= 100 || pct % 20 === 0) prog.textContent = (_t('reader.renderingProgress', { percent: Math.min(pct, 100) }) || ('渲染中… ' + Math.min(pct, 100) + '%'));
      if (end < total) await new Promise(r => setTimeout(r, 0));
    }
  } finally {
    prog?.remove();
  }
  if (!isReaderRenderCurrent(task)) return;
  if (savedTop) el.scrollTop = savedTop;
  postProcess();
  updatePaginationBar();
}

function normalizeHeadingText(text) {
  return String(text || '').trim().toLowerCase()
    .replace(/^[0-9]+[.\-、\s]*/, '')
    .replace(/[^\w\u4e00-\u9fff]/g, '');
}

function findMatchingHeading(targetAnchor, linkText, headings) {
  if (!headings || !headings.length) return null;
  let decodedAnchor = '';
  try { decodedAnchor = decodeURIComponent(targetAnchor).replace(/^#/, '').trim(); } catch (e) { decodedAnchor = targetAnchor.replace(/^#/, '').trim(); }
  const normAnchor = normalizeHeadingText(decodedAnchor);
  const normLinkText = normalizeHeadingText(linkText);

  // 1. 直接 ID 精确匹配
  let direct = document.getElementById(decodedAnchor) || document.getElementById(targetAnchor);
  if (direct && headings.includes(direct)) return direct;

  // 2. 标题文本完全匹配（不区分大小写）
  for (const h of headings) {
    const hText = h.textContent.trim();
    if (hText.toLowerCase() === decodedAnchor.toLowerCase() || (linkText && hText.toLowerCase() === linkText.trim().toLowerCase())) {
      return h;
    }
  }

  // 3. 归一化语义文本匹配（消除标点、空格、序号差异）
  if (normLinkText || normAnchor) {
    for (const h of headings) {
      const normH = normalizeHeadingText(h.textContent);
      if (normH && (normH === normLinkText || normH === normAnchor)) {
        return h;
      }
    }
  }

  // 4. 前缀与包含关系模糊匹配
  if (normLinkText && normLinkText.length >= 2) {
    for (const h of headings) {
      const normH = normalizeHeadingText(h.textContent);
      if (normH && (normH.includes(normLinkText) || normLinkText.includes(normH))) {
        return h;
      }
    }
  }
  if (normAnchor && normAnchor.length >= 2) {
    for (const h of headings) {
      const normH = normalizeHeadingText(h.textContent);
      if (normH && (normH.includes(normAnchor) || normAnchor.includes(normH))) {
        return h;
      }
    }
  }

  return null;
}

function ensureHeadingIds(body) {
  const headings = body.querySelectorAll('h1, h2, h3, h4, h5, h6');
  const seen = {};
  headings.forEach((h, i) => {
    if (!h.id) {
      let slug = h.textContent.trim().toLowerCase()
        .replace(/[^\w\u4e00-\u9fff\s-]/g, '')
        .replace(/\s+/g, '-');
      if (!slug) slug = 'toc-h-' + i;
      if (seen[slug]) {
        seen[slug]++;
        slug = slug + '-' + seen[slug];
      } else {
        seen[slug] = 1;
      }
      h.id = slug;
    }
  });
}

function processBibCitations(body) {
  if (!body) return;
  const keys = Object.keys(currentDocCitations);
  if (!keys.length) return;
  const usedKeys = new Set();

  const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT, null, false);
  const nodesToReplace = [];
  let node;
  while ((node = walker.nextNode())) {
    if (node.parentElement && ['SCRIPT', 'STYLE', 'CODE', 'PRE'].includes(node.parentElement.tagName)) continue;
    if (/@([a-zA-Z0-9_\-:]+)/.test(node.nodeValue)) {
      nodesToReplace.push(node);
    }
  }

  for (const n of nodesToReplace) {
    const parent = n.parentNode;
    if (!parent) continue;
    const text = n.nodeValue;
    const citationPattern = /\[@([a-zA-Z0-9_\-:]+)\]|@([a-zA-Z0-9_\-:]+)/g;
    let cursor = 0;
    let replaced = false;
    let match;
    while ((match = citationPattern.exec(text))) {
      const citeKey = match[1] || match[2];
      const entry = currentDocCitations[citeKey];
      if (!entry) continue;
      usedKeys.add(citeKey);
      const label = entry.short_cite || `[${citeKey}]`;
      if (match.index > cursor) parent.insertBefore(document.createTextNode(text.slice(cursor, match.index)), n);
      const badge = document.createElement('span');
      badge.className = 'bib-cite-badge';
      badge.dataset.citekey = citeKey;
      badge.textContent = label;
      parent.insertBefore(badge, n);
      cursor = citationPattern.lastIndex;
      replaced = true;
    }
    if (!replaced) continue;
    if (cursor < text.length) parent.insertBefore(document.createTextNode(text.slice(cursor)), n);
    parent.removeChild(n);
  }

  body.querySelectorAll('.bib-cite-badge').forEach(badge => {
    badge.addEventListener('mouseenter', e => showBibHoverCard(e, badge.dataset.citekey));
    badge.addEventListener('mouseleave', () => scheduleHideBibHoverCard());
  });

  if (usedKeys.size > 0 && !body.querySelector('.academic-references')) {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    const refSection = document.createElement('section');
    refSection.className = 'academic-references';
    const heading = document.createElement('h3');
    heading.textContent = _t('reader.referencesHeading') || 'References / 参考文献';
    const ol = document.createElement('ol');
    refSection.append(heading, ol);

    for (const key of usedKeys) {
      const entry = currentDocCitations[key];
      const li = document.createElement('li');
      li.id = 'ref-' + encodeURIComponent(key);
      if (entry && entry.full_reference) {
        li.textContent = entry.full_reference;
      } else {
        li.textContent = key;
      }
      ol.appendChild(li);
    }
    body.appendChild(refSection);
  }
}

let bibCardEl = null;
let bibHideTimer = null;

function showBibHoverCard(e, key) {
  if (bibHideTimer) {
    clearTimeout(bibHideTimer);
    bibHideTimer = null;
  }
  if (bibCardEl && bibCardEl.dataset.key === key) return;
  hideBibHoverCard();

  const entry = currentDocCitations[key];
  if (!entry) return;

  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  bibCardEl = document.createElement('div');
  bibCardEl.className = 'bib-hover-card';
  bibCardEl.dataset.key = key;
  const cardTitle = document.createElement('div');
  cardTitle.className = 'bib-card-title';
  cardTitle.textContent = entry.title || key;
  const cardAuthor = document.createElement('div');
  cardAuthor.className = 'bib-card-author';
  cardAuthor.textContent = `${entry.author || ''} ${entry.year ? `(${entry.year})` : ''}`.trim();
  const cardJournal = document.createElement('div');
  cardJournal.className = 'bib-card-journal';
  cardJournal.textContent = entry.journal || entry.booktitle || '';
  const cardActions = document.createElement('div');
  cardActions.className = 'bib-card-actions';
  if (/^10\.\d{4,9}\/[^\s<>"']+$/.test(entry.doi || '')) {
    const doiLink = document.createElement('a');
    doiLink.className = 'bib-card-btn';
    doiLink.href = `https://doi.org/${encodeURIComponent(entry.doi)}`;
    doiLink.target = '_blank';
    doiLink.rel = 'noopener noreferrer';
    doiLink.textContent = 'DOI';
    cardActions.appendChild(doiLink);
  }
  const copyButton = document.createElement('button');
  copyButton.className = 'bib-card-btn';
  copyButton.id = 'bib-copy-btn';
  copyButton.type = 'button';
  copyButton.textContent = _t('reader.copyBibtex') || '复制 BibTeX';
  cardActions.appendChild(copyButton);
  bibCardEl.append(cardTitle, cardAuthor, cardJournal, cardActions);
  document.body.appendChild(bibCardEl);

  bibCardEl.addEventListener('mouseenter', () => {
    if (bibHideTimer) {
      clearTimeout(bibHideTimer);
      bibHideTimer = null;
    }
  });
  bibCardEl.addEventListener('mouseleave', () => {
    scheduleHideBibHoverCard();
  });

  const rect = e.target.getBoundingClientRect();
  let top = rect.bottom + window.scrollY + 6;
  let left = Math.max(10, rect.left + window.scrollX - 20);
  bibCardEl.style.top = top + 'px';
  bibCardEl.style.left = left + 'px';

  bibCardEl.querySelector('#bib-copy-btn').addEventListener('click', () => {
    const bibText = `@${entry.entry_type || 'article'}{${key},\n  title={${entry.title || ''}},\n  author={${entry.author || ''}},\n  year={${entry.year || ''}}\n}`;
    navigator.clipboard.writeText(bibText);
    showToast(_t('toast.copiedBibtex') || '已复制 BibTeX 引用', 1500);
  });
}


function scheduleHideBibHoverCard() {
  if (bibHideTimer) clearTimeout(bibHideTimer);
  bibHideTimer = setTimeout(() => {
    hideBibHoverCard();
  }, 220);
}

function hideBibHoverCard() {
  if (bibHideTimer) {
    clearTimeout(bibHideTimer);
    bibHideTimer = null;
  }
  if (bibCardEl) {
    bibCardEl.remove();
    bibCardEl = null;
  }
}


function postProcess(container) {
  const body = container || document.querySelector('#content .markdown-body') || $('content');
  if (!body) return;
  ensureHeadingIds(body);
  fixLinks(body);
  fixImages(body);
  processBibCitations(body);
  buildToc();
  renderMath(body);
  renderAllCodeChunks(body);
  renderAllDiagrams(body);
}

function renderAllCodeChunks(container) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const cards = (container || document).querySelectorAll('.code-chunk-card');
  cards.forEach(card => {
    if (card._bound) return;
    card._bound = true;
    const btn = card.querySelector('.code-chunk-run-btn');
    const statusEl = card.querySelector('.code-chunk-status');
    const timerEl = card.querySelector('.code-chunk-timer');
    const outWrap = card.querySelector('.code-chunk-output');
    const stdoutEl = card.querySelector('.code-chunk-stdout');
    const plotEl = card.querySelector('.code-chunk-plot');

    const lang = card.dataset.lang || 'python';
    const code = decodeURIComponent(card.dataset.code || '');

    const run = async () => {
      statusEl.className = 'code-chunk-status running';
      statusEl.textContent = _t('reader.codeRunning');
      btn.disabled = true;
      btn.textContent = _t('reader.codeRunning');
      const t0 = Date.now();
      const interval = setInterval(() => {
        timerEl.textContent = ((Date.now() - t0) / 1000).toFixed(1) + 's';
      }, 100);

      try {
        let res;
        if (hasPy && py.run_code_chunk) {
          res = await py.run_code_chunk(lang, code, null, 10, true);
        } else {
          const r = await apiFetch('/api/code/run', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ lang: lang, code: code, confirm: true })
          });
          res = await r.json();
        }

        clearInterval(interval);
        timerEl.textContent = ((Date.now() - t0) / 1000).toFixed(2) + 's';

        if (res && res.ok) {
          statusEl.className = 'code-chunk-status success';
          statusEl.textContent = _t('convert.statusOk');
          const outputFormat = String(card.dataset.outputFormat || 'text').toLowerCase();
          const outputText = (res.stdout || '') + (res.stderr ? ('\n' + res.stderr) : '');
          // Match MPE's output channel semantics while keeping every path
          // inert by default: `none` hides output, `png` relies on captured
          // images, and all other formats remain escaped text in the output
          // preformatted node.  Source mutation still requires output=true
          // and the explicit editor save path below.
          if (outputFormat === 'none') {
            outWrap.classList.add('hidden');
            stdoutEl.textContent = '';
          } else {
            outWrap.classList.remove('hidden');
            stdoutEl.textContent = outputText;
            if (!stdoutEl.textContent.trim() && outputFormat !== 'png') stdoutEl.textContent = _t('reader.noConsoleOutput');
          }
          plotEl.innerHTML = '';
          if (res.images && res.images.length > 0) {
            res.images.forEach(imgSrc => {
              const img = document.createElement('img');
              img.src = imgSrc;
              img.alt = `${lang} plot output`;
              plotEl.appendChild(img);
            });
          }
          // Match MPE's explicit source-write opt-in.  A normal ``cmd``
          // block only renders into the preview; ``output=true`` adds a
          // bounded marker block after the matching fence and persists it
          // through the editor's normal history/save path.
          if (card.dataset.output === 'true' && outputFormat !== 'none' && state.editing) {
            await persistCodeChunkOutput(card, outputText);
          }
        } else {
          statusEl.className = 'code-chunk-status error';
          statusEl.textContent = _t('convert.statusFailed');
          outWrap.classList.remove('hidden');
          // The core returns stable error codes; never surface its internal
          // exception text (which may contain local paths or untranslated
          // backend strings) in the document UI.
          // Error details stay in the structured response for diagnostics;
          // the document UI only renders a localized, stable message.  This
          // prevents backend paths, subprocess output or untranslated text
          // from leaking into a document preview.
          stdoutEl.textContent = _t('toast.unknownError');
        }
      } catch (err) {
        clearInterval(interval);
        statusEl.className = 'code-chunk-status error';
        statusEl.textContent = _t('reader.callFailed');
        outWrap.classList.remove('hidden');
        stdoutEl.textContent = _t('toast.unknownError');
      } finally {
        btn.disabled = false;
        btn.textContent = `▶ ${_t('reader.runAgain')}`;
      }
    };

    if (btn) btn.addEventListener('click', run);

    // 绑定输出区复制与清空交互
    const copyBtn = card.querySelector('.code-chunk-copy-btn');
    if (copyBtn) {
      copyBtn.addEventListener('click', () => {
        const text = (stdoutEl ? stdoutEl.textContent : '') || '';
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(text);
          showToast(_t('toast.outputCopied'), 1200);
        }
      });
    }

    const clearBtn = card.querySelector('.code-chunk-clear-btn');
    if (clearBtn) {
      clearBtn.addEventListener('click', () => {
        outWrap.classList.add('hidden');
        if (stdoutEl) stdoutEl.textContent = '';
        if (plotEl) plotEl.innerHTML = '';
        statusEl.className = 'code-chunk-status';
        statusEl.textContent = _t('status.ready');
        if (timerEl) timerEl.textContent = '';
        showToast(_t('toast.outputCleared'), 1000);
      });
    }
  });
}
window.renderAllCodeChunks = renderAllCodeChunks;

async function persistCodeChunkOutput(card, outputText) {
  const sourceLine = Number(card && card.dataset && card.dataset.sourceLine);
  if (!Number.isInteger(sourceLine) || sourceLine < 1 || !state.editing) return false;
  const current = getEditContent();
  const lines = String(current || '').split('\n');
  const fenceStart = Math.max(0, Math.min(lines.length - 1, sourceLine - 1));
  if (!/^\s*```/.test(lines[fenceStart] || '')) return false;
  let fenceEnd = fenceStart + 1;
  while (fenceEnd < lines.length && !/^\s*```\s*$/.test(lines[fenceEnd])) fenceEnd += 1;
  if (fenceEnd >= lines.length) return false;

  const markerStart = '<!-- code_chunk_output -->';
  const markerEnd = '<!-- /code_chunk_output -->';
  let outputStart = -1;
  for (let index = fenceEnd + 1; index < Math.min(lines.length, fenceEnd + 7); index += 1) {
    if (lines[index].trim() === markerStart) { outputStart = index; break; }
  }
  let replaceFrom;
  let replaceTo;
  if (outputStart >= 0) {
    let outputEnd = outputStart + 1;
    while (outputEnd < lines.length && lines[outputEnd].trim() !== markerEnd) outputEnd += 1;
    if (outputEnd >= lines.length) return false;
    replaceFrom = outputStart;
    replaceTo = outputEnd + 1;
  } else {
    replaceFrom = fenceEnd + 1;
    replaceTo = fenceEnd + 1;
  }
  const safeText = String(outputText || '').replace(/\r\n?/g, '\n');
  const block = [
    '', markerStart, '', safeText, '', markerEnd, ''
  ].join('\n');
  const offsets = [0];
  for (const line of lines) offsets.push(offsets[offsets.length - 1] + line.length + 1);
  const from = offsets[replaceFrom];
  const to = offsets[replaceTo];
  const existing = lines.slice(replaceFrom, replaceTo).join('\n');
  if (existing === block.replace(/^\n/, '').replace(/\n$/, '')) return false;
  if (cmView) cmView.dispatch({ changes: { from, to, insert: block } });
  else if ($('edit-area')) $('edit-area').setRangeText(block, from, to, 'end');
  if (typeof saveEdit === 'function') await saveEdit();
  return true;
}
window.persistCodeChunkOutput = persistCodeChunkOutput;

async function runAllCodeChunks() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const cards = document.querySelectorAll('.code-chunk-card');
  if (!cards.length) {
    showToast(_t('toast.noRunnableChunks'));
    return;
  }
  showToast(_t('toast.batchRunStarted', { count: cards.length }), 1500);
  for (const card of cards) {
    const btn = card.querySelector('.code-chunk-run-btn');
    if (btn) {
      btn.click();
      await new Promise(r => setTimeout(r, 150));
    }
  }
}
window.runAllCodeChunks = runAllCodeChunks;

// Diagram engines are intentionally loaded only when a document contains the
// corresponding fence.  Keeping each upstream bundle as a separate static
// asset preserves the fast first paint while making the supported engines
// usable without a network connection.
const diagramScriptPromises = new Map();
let diagramWaveCounter = 0;
// TikZjax owns a single global TeX instance and temporarily swaps
// window.fetch, window.onload and console.log while compiling, so concurrent
// tikz cards cross-contaminate: one card's TeX error lines flow through the
// other card's console hook and abort its wait loop.  Compiles are queued.
let tikzRenderChain = Promise.resolve();
// Heading ids leak into `window` through named element access, so a document
// containing "## mermaid" makes `window.mermaid` an <h2> before the vendor
// bundle loads.  A plain truthiness check would then skip injecting the
// engine and every render would crash on the missing API.
function hasUsableDiagramGlobal(globalName) {
  if (!globalName) return false;
  const value = window[globalName];
  return value != null && !(value instanceof Element);
}
function loadDiagramScript(path, globalName) {
  if (hasUsableDiagramGlobal(globalName)) return Promise.resolve(window[globalName]);
  if (diagramScriptPromises.has(path)) return diagramScriptPromises.get(path);
  const promise = new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = path;
    script.async = true;
    script.onload = () => {
      if (globalName && !hasUsableDiagramGlobal(globalName)) {
        diagramScriptPromises.delete(path);
        reject(new Error('diagram_engine_unavailable'));
        return;
      }
      resolve(globalName ? window[globalName] : true);
    };
    script.onerror = () => {
      diagramScriptPromises.delete(path);
      reject(new Error('diagram_engine_load_failed'));
    };
    document.head.appendChild(script);
  });
  diagramScriptPromises.set(path, promise);
  return promise;
}

function diagramOutputMarkup(markup) {
  if (!markup) return '';
  const str = String(markup).trim();
  if (!str) return '';

  const template = document.createElement('template');
  template.innerHTML = str;

  // Dangerous executable / navigation tags to strictly eliminate
  const bannedTags = ['script', 'iframe', 'object', 'embed', 'applet', 'meta', 'link', 'base'];
  bannedTags.forEach(t => {
    template.content.querySelectorAll(t).forEach(el => el.remove());
  });

  // Sanitize every node in the diagram output while preserving rich SVG structure
  template.content.querySelectorAll('*').forEach(node => {
    const tag = node.tagName.toLowerCase();

    // Prevent UI hijacking buttons or nested cards inside diagram preview
    if (tag === 'button' || node.classList.contains('code-chunk-card') || node.classList.contains('diagram-card')) {
      node.replaceWith(...node.childNodes);
      return;
    }

    // Sanitize attributes
    Array.from(node.attributes).forEach(attr => {
      const name = attr.name.toLowerCase();
      const val = attr.value;
      // Disallow all inline event handlers
      if (name.startsWith('on')) {
        node.removeAttribute(attr.name);
      } else if (name === 'href' || name === 'xlink:href') {
        // Disallow javascript:, vbscript:, data:text/html
        if (/^\s*(javascript|vbscript|data:\s*text\/html)/i.test(val)) {
          node.removeAttribute(attr.name);
        }
      } else if (name === 'style') {
        // Disallow executable CSS expressions
        if (/expression|javascript:|behavior|-moz-binding/i.test(val)) {
          node.removeAttribute(attr.name);
        }
      }
    });

    // In <style> blocks, prevent CSS expressions, imports or external url execution
    if (tag === 'style') {
      let css = node.textContent || '';
      if (/expression|javascript:|behavior|-moz-binding|@import/i.test(css)) {
        node.textContent = css.replace(/expression\s*\([^)]*\)/gi, '')
                              .replace(/javascript:/gi, '')
                              .replace(/@import[^;]*;/gi, '');
      }
    }
  });

  return template.innerHTML;
}

// Server responses and client throws both carry stable diagram_* error codes.
// Map them to locale keys once so every failure path shows the same honest
// wording instead of a blanket "unknown error".
const DIAGRAM_ERROR_I18N = {
  diagram_engine_unavailable: 'reader.diagramEngineUnavailable',
  diagram_engine_load_failed: 'reader.diagramEngineLoadFailed',
  diagram_engine_timeout: 'reader.diagramTimeout',
  diagram_engine_unavailable_handler: 'reader.diagramEngineUnavailable',
  diagram_engine_unavailable_rendered: 'reader.diagramEngineUnavailable',
  diagram_invalid_input: 'reader.diagramInvalidInput',
  diagram_dependency_missing: 'reader.diagramDependencyMissing',
  diagram_client_renderer_required: 'reader.diagramClientRendererRequired',
  diagram_network_unavailable: 'reader.diagramNetworkUnavailable',
  diagram_input_too_large: 'reader.diagramInputTooLarge',
  diagram_render_failed: 'reader.diagramRenderFailed',
};

function escapeHtml(str) {
  return String(str || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
window.escapeHtml = escapeHtml;

function diagramErrorKey(code) {
  return (code && DIAGRAM_ERROR_I18N[String(code)]) || 'toast.unknownError';
}

function diagramFallbackMarkup(code, errorCode) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const reason = _t(diagramErrorKey(errorCode));
  const safeReason = window.escapeHtml ? escapeHtml(reason) : reason;
  const message = _t('reader.diagramError', { error: safeReason });
  const safeMessage = window.escapeHtml ? escapeHtml(message) : message;
  const safeCode = window.escapeHtml ? escapeHtml(String(code || '')) : String(code || '');
  const refreshLabel = _t('reader.refresh') || '重试';
  return `<div class="diagram-fallback-wrap"><div class="diagram-fallback-hint">${safeMessage}</div><button type="button" class="diagram-inline-retry" aria-label="${refreshLabel}"><svg class="tb-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:13px;height:13px;margin-right:4px;"><path d="M21 12a9 9 0 1 1-3-6.7"/><path d="M21 3v6h-6"/></svg><span>${refreshLabel}</span></button><pre class="diagram-fallback"><code>${safeCode}</code></pre></div>`;
}

function renderDiagramFallback(previewEl, engine, code, errorCode) {
  if (!previewEl) return;
  previewEl.innerHTML = diagramFallbackMarkup(code, errorCode);
}

// A few vendored renderers (notably bitfield) return ONML's array-shaped
// virtual tree instead of an HTML string.  Convert that trusted tree through
// the DOM before applying the normal SVG sanitizer; never concatenate it into
// markup because values may contain user-provided labels.
function diagramOnmlToSvg(node) {
  if (node == null || node === false) return null;
  if (typeof node === 'string' || typeof node === 'number') return document.createTextNode(String(node));
  if (!Array.isArray(node) || !node.length) return null;
  const tag = String(node[0] || '').toLowerCase();
  if (!/^[a-z][a-z0-9:-]*$/.test(tag)) return null;
  const el = document.createElementNS('http://www.w3.org/2000/svg', tag);
  let index = 1;
  const attrs = node[1];
  if (attrs && typeof attrs === 'object' && !Array.isArray(attrs)) {
    Object.entries(attrs).forEach(([name, value]) => {
      if (value == null || typeof value === 'function') return;
      el.setAttribute(name, String(value));
    });
    index = 2;
  }
  for (; index < node.length; index += 1) {
    const child = diagramOnmlToSvg(node[index]);
    if (child) el.appendChild(child);
  }
  return el;
}

function parseWaveDromSource(source) {
  const raw = String(source || '').trim();
  try { return JSON.parse(raw); } catch (_) { /* Common WaveDrom examples use JS-like keys. */ }
  const normalized = raw
    .replace(/([{,]\s*)([A-Za-z_$][\w$-]*)\s*:/g, '$1"$2":')
    .replace(/'([^'\\]*(?:\\.[^'\\]*)*)'/g, (_, value) => `"${value.replace(/"/g, '\\"')}"`)
    .replace(/,\s*([}\]])/g, '$1');
  try { return JSON.parse(normalized); } catch (_) { throw new Error('diagram_invalid_input'); }
}

async function renderTikzjaxDiagram(code, previewEl) {
  // TikZjax is a browser script that normally resolves its WASM/font assets
  // from an upstream S3 URL.  Keep the vendored source byte-for-byte intact,
  // but route those requests to our packaged assets while the renderer runs.
  // This makes the feature deterministic and offline without weakening CSP.
  const originalFetch = window.fetch;
  const localRoot = '/assets/vendor/diagrams/tikzjax/';
  window.fetch = function(input, init) {
    const raw = typeof input === 'string' ? input : (input && input.url) || '';
    if (raw.indexOf('https://s3.us-east-2.amazonaws.com/tikzjax.com/') === 0) {
      const filename = raw.slice(raw.lastIndexOf('/') + 1);
      return originalFetch.call(this, localRoot + encodeURIComponent(filename), init);
    }
    return originalFetch.call(this, input, init);
  };
  const previousOnload = window.onload;
  // TeX reports compile errors on the console, and the wasm glue captures
  // console.log while tikzjax.js itself executes.  The hook therefore has
  // to be installed before the script loads; otherwise invalid input
  // would silently hit the render deadline instead of failing fast.
  // Declared outside try: the finally restore references these, and
  // sibling blocks (try/finally) do not share block scope.
  const originalLog = console.log;
  let texError = '';
  const texLogHook = function(...args) {
    if (!texError) {
      const line = args.map(a => (typeof a === 'string' ? a : '')).join(' ');
      if (line.slice(0, 2) === '! ' || line.indexOf('Emergency stop') !== -1) texError = line;
    }
    return originalLog.apply(this, args);
  };
  console.log = texLogHook;
  try {
    // tikzjax.js installs its work via window.onload on execution.  The
    // promise cache would skip re-execution on every render after the
    // first, leaving window.onload unchanged and the diagram stuck.
    await loadDiagramScript('/assets/vendor/diagrams/tikzjax/tikzjax.js?reload=' + (++diagramWaveCounter));
    const handler = window.onload;
    if (typeof handler !== 'function' || handler === previousOnload) {
      throw new Error('diagram_engine_unavailable_handler');
    }
    const source = document.createElement('script');
    source.type = 'text/tikz';
    let tikzSource = String(code || '');
    // \draw and other TikZ commands are only defined inside the tikzpicture
    // environment, even in a full TeX install.  Bare snippets are therefore
    // invalid input as-is; wrap them instead of failing with "Undefined
    // control sequence" after a full 30s compile.
    if (!/\\begin\{document\}/.test(tikzSource) && !/\\begin\{tikzpicture\}/.test(tikzSource)) {
      tikzSource = '\\begin{tikzpicture}\n' + tikzSource + '\n\\end{tikzpicture}';
    }
    source.textContent = tikzSource;
    previewEl.replaceChildren(source);
    // TikZjax's onload handler starts an async reduce but does not reliably
    // return its final replacement promise in every browser.  Wait for the
    // script node to be replaced instead of racing an already-resolved
    // wrapper promise; this also prevents the renderer timeout from
    // removing the node while WASM is still compiling.
    handler.call(window);
    const deadline = Date.now() + 30000;
    while (source.parentNode && Date.now() < deadline && !texError) {
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    const rendered = previewEl.firstElementChild;
    if (!rendered || rendered.tagName.toLowerCase() === 'script') {
      if (texError) throw new Error('diagram_render_failed');
      throw new Error(Date.now() >= deadline ? 'diagram_engine_timeout' : 'diagram_engine_unavailable_rendered');
    }
    return diagramOutputMarkup(rendered.outerHTML);
  } finally {
    window.fetch = originalFetch;
    window.onload = previousOnload;
    if (console.log === texLogHook) console.log = originalLog;
  }
}

async function renderLocalDiagram(engine, code, previewEl) {
  const normalized = String(engine || '').toLowerCase();
  if (normalized === 'mermaid') {
    const mermaid = await loadDiagramScript('/assets/vendor/diagrams/mermaid/mermaid.min.js', 'mermaid');
    const isDark = document.body?.dataset?.theme === 'dark';
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: isDark ? 'dark' : 'default',
      fontFamily: 'var(--font-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif)',
      flowchart: {
        useMaxWidth: true,
        htmlLabels: true,
        curve: 'basis'
      },
      sequence: {
        useMaxWidth: true
      },
      gantt: {
        useMaxWidth: true
      }
    });
    const id = 'readmd-mermaid-' + Math.random().toString(36).slice(2);
    const rendered = await mermaid.render(id, code);
    return diagramOutputMarkup(rendered.svg || rendered);
  }
  if (normalized === 'wavedrom') {
    await loadDiagramScript('/assets/vendor/diagrams/wavedrom/skins/default.js', 'WaveSkin');
    await loadDiagramScript('/assets/vendor/diagrams/wavedrom/skins/narrow.js');
    const wavedrom = await loadDiagramScript('/assets/vendor/diagrams/wavedrom/wavedrom.min.js', 'WaveDrom');
    const source = parseWaveDromSource(code);
    const index = diagramWaveCounter++;
    const holder = document.createElement('div');
    const output = document.createElement('div');
    output.id = `readmd-wavedrom-output-${index}`;
    holder.append(output);
    previewEl.replaceChildren(holder);
    // RenderWaveForm accepts the already parsed object and writes only to the
    // supplied output node.  This avoids ProcessAll's global InputJSON_*
    // bookkeeping and keeps concurrent diagram cards isolated.
    wavedrom.RenderWaveForm(index, source, 'readmd-wavedrom-output-', false);
    const svg = output.querySelector('svg');
    if (!svg) throw new Error('diagram_invalid_input');
    const html = diagramOutputMarkup(svg.outerHTML);
    holder.remove();
    return html;
  }
  if (normalized === 'bitfield') {
    const bitfield = await loadDiagramScript('/assets/vendor/diagrams/bitfield/bitfield.min.js', 'bitfield');
    // Bitfield examples in the wild use the same JS-like JSON as WaveDrom
    // (unquoted keys, single quotes, trailing commas); reuse the tolerant
    // parser instead of a strict JSON.parse that rejects them.
    let description = parseWaveDromSource(code);
    if (description && !Array.isArray(description) && Array.isArray(description.reg)) description = description.reg;
    if (!Array.isArray(description)) throw new Error('diagram_invalid_input');
    const rendered = bitfield.render(description, {});
    const root = rendered && rendered.outerHTML
      ? rendered
      : diagramOnmlToSvg(rendered);
    if (!root || !root.outerHTML) throw new Error('diagram_invalid_input');
    return diagramOutputMarkup(root.outerHTML);
  }
  if (normalized === 'viz' || normalized === 'dot' || normalized === 'graphviz') {
    const viz = await loadDiagramScript('/assets/vendor/diagrams/viz/viz-standalone.js', 'Viz');
    const instance = await Promise.race([
      viz.instance(),
      new Promise((_, reject) => setTimeout(() => reject(new Error('diagram_engine_timeout')), 8000)),
    ]);
    return diagramOutputMarkup(await instance.renderString(code, {
      engine: normalized === 'viz' ? 'dot' : normalized,
      format: 'svg',
    }));
  }
  if (normalized === 'vega' || normalized === 'vega-lite') {
    const vega = await loadDiagramScript('/assets/vendor/diagrams/vega/vega.min.js', 'vega');
    const vegaLite = await loadDiagramScript('/assets/vendor/diagrams/vega-lite/vega-lite.min.js', 'vegaLite');
    let spec;
    try { spec = JSON.parse(String(code || '')); } catch (_) { throw new Error('diagram_invalid_input'); }
    if (!spec || typeof spec !== 'object' || Array.isArray(spec)) {
      throw new Error('diagram_invalid_input');
    }
    const isDark = document.body?.dataset?.theme === 'dark';
    const darkConfig = isDark ? {
      background: 'transparent',
      axis: {
        domainColor: '#868fa0',
        gridColor: 'rgba(255, 255, 255, 0.08)',
        tickColor: '#868fa0',
        labelColor: '#e7e9ee',
        titleColor: '#e7e9ee'
      },
      legend: {
        labelColor: '#e7e9ee',
        titleColor: '#e7e9ee'
      },
      text: {
        fill: '#e7e9ee'
      },
      title: {
        color: '#e7e9ee',
        subtitleColor: '#9aa3b2'
      }
    } : { background: 'transparent' };
    try {
      const compiled = normalized === 'vega-lite' && vegaLite ? vegaLite.compile(spec, { config: darkConfig }).spec : spec;
      const view = new vega.View(vega.parse(compiled, darkConfig), { renderer: 'none' });
      await view.runAsync();
      const svg = await view.toSVG();
      if (!svg || !svg.startsWith('<svg')) throw new Error('diagram_render_failed');
      return diagramOutputMarkup(svg);
    } catch (err) {
      if (err && (err.message === 'diagram_invalid_input' || err.message === 'diagram_render_failed')) throw err;
      throw new Error('diagram_render_failed');
    }
  }
  if (normalized === 'chart' || normalized === 'chartjs' || normalized === 'chart.js') {
    const Chart = await loadDiagramScript('/assets/vendor/diagrams/chart/chart.umd.js', 'Chart');
    let config;
    try { config = JSON.parse(String(code || '')); } catch (_) { throw new Error('diagram_invalid_input'); }
    if (!config || typeof config !== 'object' || Array.isArray(config)) {
      throw new Error('diagram_invalid_input');
    }
    const isDark = document.body?.dataset?.theme === 'dark';
    const primaryBlue = isDark ? 'rgba(59, 130, 246, 0.75)' : 'rgba(37, 99, 235, 0.8)';
    const primaryBlueBorder = isDark ? '#60a5fa' : '#2563eb';
    if (Chart && Chart.defaults) {
      Chart.defaults.color = isDark ? '#e7e9ee' : '#374151';
      Chart.defaults.borderColor = isDark ? 'rgba(255, 255, 255, 0.12)' : 'rgba(0, 0, 0, 0.1)';
      Chart.defaults.backgroundColor = primaryBlue;
      if (Chart.defaults.elements && Chart.defaults.elements.bar) {
        Chart.defaults.elements.bar.backgroundColor = primaryBlue;
        Chart.defaults.elements.bar.borderColor = primaryBlueBorder;
        Chart.defaults.elements.bar.borderWidth = 1;
      }
      if (Chart.defaults.elements && Chart.defaults.elements.line) {
        Chart.defaults.elements.line.borderColor = primaryBlueBorder;
        Chart.defaults.elements.line.backgroundColor = isDark ? 'rgba(59, 130, 246, 0.2)' : 'rgba(37, 99, 235, 0.2)';
      }
      if (Chart.defaults.elements && Chart.defaults.elements.point) {
        Chart.defaults.elements.point.backgroundColor = primaryBlueBorder;
        Chart.defaults.elements.point.borderColor = isDark ? '#1e293b' : '#ffffff';
      }
    }
    const defaultColors = [
      isDark ? 'rgba(59, 130, 246, 0.75)' : 'rgba(37, 99, 235, 0.8)',   // blue
      isDark ? 'rgba(16, 185, 129, 0.75)' : 'rgba(5, 150, 105, 0.8)',   // emerald
      isDark ? 'rgba(245, 158, 11, 0.75)' : 'rgba(217, 119, 6, 0.8)',   // amber
      isDark ? 'rgba(239, 68, 68, 0.75)' : 'rgba(220, 38, 38, 0.8)',    // red
      isDark ? 'rgba(139, 92, 246, 0.75)' : 'rgba(124, 58, 237, 0.8)',  // violet
      isDark ? 'rgba(6, 182, 212, 0.75)' : 'rgba(8, 145, 178, 0.8)',    // cyan
    ];
    const defaultBorders = [
      isDark ? '#60a5fa' : '#2563eb',
      isDark ? '#34d399' : '#059669',
      isDark ? '#fbbf24' : '#d97706',
      isDark ? '#f87171' : '#dc2626',
      isDark ? '#a78bfa' : '#7c3aed',
      isDark ? '#22d3ee' : '#0891b2',
    ];
    if (config && config.data && Array.isArray(config.data.datasets)) {
      config.data.datasets.forEach((ds, idx) => {
        if (!ds.backgroundColor) {
          ds.backgroundColor = defaultColors[idx % defaultColors.length];
        }
        if (!ds.borderColor) {
          ds.borderColor = defaultBorders[idx % defaultBorders.length];
          if (ds.borderWidth == null) ds.borderWidth = 1;
        }
      });
    }
    const canvas = document.createElement('canvas');
    canvas.className = 'diagram-chart-canvas';
    canvas.setAttribute('role', 'img');
    const chartLabel = window.i18n
      ? window.i18n.t('reader.diagramBadge', { lang: 'Chart.js' })
      : '';
    canvas.setAttribute('aria-label', chartLabel && chartLabel !== 'reader.diagramBadge'
      ? chartLabel : '');
    // A bounded canvas keeps malformed configs from forcing unbounded layout
    // growth while still allowing the card to size naturally in narrow panes.
    canvas.width = 960;
    canvas.height = 540;
    previewEl.replaceChildren(canvas);
    const context = canvas.getContext('2d');
    if (!context) throw new Error('diagram_engine_unavailable');
    let chart;
    try {
      chart = new Chart(context, config);
      // Wait for the first synchronous draw before returning the canvas.  The
      // instance is retained on the node so a reload can destroy it cleanly.
      canvas._readmdChart = chart;
    } catch (_) {
      canvas.remove();
      throw new Error('diagram_render_failed');
    }
    return canvas;
  }
  if (normalized === 'tikz') {
    // Serialized through tikzRenderChain: see the comment at its declaration.
    const turn = tikzRenderChain.then(() => renderTikzjaxDiagram(code, previewEl));
    tikzRenderChain = turn.then(() => {}, () => {});
    return turn;
  }
  // D2 has no bundled offline runtime in this release. Fail explicitly rather
  // than silently sending source code to an online renderer and reporting a
  // false success.
  throw new Error('diagram_engine_unavailable');
}

function renderAllDiagrams(container) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const root = container || document;
  const cards = root.matches && root.matches('.diagram-card')
    ? [root]
    : root.querySelectorAll('.diagram-card');
  cards.forEach(async card => {
    if (card._rendered) return;
    card._rendered = true;
    const engine = card.dataset.diagramEngine || 'plantuml';
    const code = decodeURIComponent(card.dataset.diagramCode || '');
    const previewEl = card.querySelector('.diagram-preview');
    const reloadBtn = card.querySelector('.diagram-reload-btn');

    const render = async () => {
      if (reloadBtn) {
        reloadBtn.disabled = true;
        reloadBtn.classList.add('is-loading');
      }
      const previousCanvas = previewEl && previewEl.querySelector('.diagram-chart-canvas');
      if (previousCanvas && previousCanvas._readmdChart && typeof previousCanvas._readmdChart.destroy === 'function') {
        try { previousCanvas._readmdChart.destroy(); } catch (_) { /* best effort cleanup */ }
      }
      previewEl.innerHTML = `<div class="diagram-loading">${_t('reader.renderingDiagram', { engine: engine.toUpperCase() })}</div>`;
      try {
        // Vega/Vega-Lite need expression evaluation.  They are rendered by
        // the bundled Node sidecar through /api/diagram/render so the secure
        // WebView CSP never needs unsafe-eval.  The remaining engines are
        // genuinely browser-local and lazy-loaded here.
        if (['mermaid', 'wavedrom', 'bitfield', 'viz', 'dot', 'graphviz', 'tikz', 'chart', 'chartjs', 'chart.js', 'vega', 'vega-lite'].includes(engine)) {
          const rendered = await renderLocalDiagram(engine, code, previewEl);
          if (rendered instanceof Element) previewEl.replaceChildren(rendered);
          else previewEl.innerHTML = rendered;
          return;
        }

        let res;
        const allowRemote = Boolean(card._allowRemote);
        if (hasPy && py.render_diagram) {
          res = await py.render_diagram(engine, code, { allow_remote: allowRemote });
        } else {
          const r = await apiFetch('/api/diagram/render', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ engine: engine, code: code, allow_remote: allowRemote })
          });
          res = await r.json();
        }

        if (res && res.ok) {
          if (res.type === 'html' && res.html) {
            previewEl.innerHTML = diagramOutputMarkup(res.html);
          } else if (res.svg) {
            previewEl.innerHTML = diagramOutputMarkup(res.svg);
          } else {
            throw new Error('diagram_engine_unavailable');
          }
          if (res.requires_network) {
            const badge = card.querySelector('.diagram-type');
            if (badge && !badge.querySelector('.diagram-network-indicator')) {
              const isZh = window.i18n && window.i18n.locale && window.i18n.locale.startsWith('zh');
              const netSpan = document.createElement('span');
              netSpan.className = 'diagram-network-indicator';
              netSpan.textContent = isZh ? ' · 在线代理' : ' · Online Proxy';
              netSpan.setAttribute('aria-label', isZh ? '在线代理渲染' : 'Rendered via online proxy');
              badge.appendChild(netSpan);
            }
          }
        } else {
          // Server responses intentionally carry only stable error codes.  Do
          // not leak those codes (or provider/host diagnostics) into the
          // rendered document; the locale owns the user-facing wording.
          console.warn('diagram render failed:', engine, res && res.error_code);
          if (res && res.error_code === 'diagram_dependency_missing' && res.remote_available) {
            const isZh = window.i18n && window.i18n.locale && window.i18n.locale.startsWith('zh');
            const confirmText = isZh ? '本机未就绪 PlantUML 环境。点击允许连接 plantuml.com 在线渲染（将上传图表源码）' : 'PlantUML local engine not found. Click to render via plantuml.com (diagram source will be sent)';
            const btnText = isZh ? '允许在线渲染' : 'Allow Online Render';
            const safeCode = window.escapeHtml ? escapeHtml(String(code || '')) : String(code || '');
            previewEl.innerHTML = `<div class="diagram-fallback-wrap"><div class="diagram-fallback-hint">${confirmText}</div><button type="button" class="diagram-inline-retry diagram-allow-remote-btn" aria-label="${btnText}"><svg class="tb-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:13px;height:13px;margin-right:4px;"><path d="M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2zm0 18a8 8 0 1 1 8-8 8 8 0 0 1-8 8z"/><path d="M12 6v6l4 2"/></svg><span>${btnText}</span></button><pre class="diagram-fallback"><code>${safeCode}</code></pre></div>`;
            const allowBtn = previewEl.querySelector('.diagram-allow-remote-btn');
            if (allowBtn) {
              allowBtn.addEventListener('click', () => {
                card._allowRemote = true;
                card._rendered = false;
                render();
              });
            }
          } else {
            previewEl.innerHTML = diagramFallbackMarkup(code, res && res.error_code);
          }
        }
      } catch (err) {
        console.warn('diagram render threw:', engine, err && err.message);
        previewEl.innerHTML = diagramFallbackMarkup(code, err && err.message);
      } finally {
        if (reloadBtn) {
          reloadBtn.disabled = false;
          reloadBtn.classList.remove('is-loading');
        }
        const inlineRetry = previewEl && previewEl.querySelector('.diagram-inline-retry');
        if (inlineRetry) {
          inlineRetry.onclick = () => { render(); };
        }
      }
    };

    card._renderFn = render;
    if (reloadBtn) reloadBtn.addEventListener('click', render);
    render();
  });
}
window.renderAllDiagrams = renderAllDiagrams;

function reloadAllDiagrams(container) {
  const root = container || document;
  const cards = root.matches && root.matches('.diagram-card')
    ? [root]
    : (root.querySelectorAll ? root.querySelectorAll('.diagram-card') : []);
  cards.forEach(card => {
    if (typeof card._renderFn === 'function') {
      card._renderFn();
    }
  });
}
window.reloadAllDiagrams = reloadAllDiagrams;

function toggleZenMode(force) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const toolbar = document.getElementById('toolbar');
  const toolbarHeight = toolbar?.getBoundingClientRect().height || 0;
  if (typeof force === 'boolean') {
    document.body.classList.toggle('zen-mode', force);
  } else {
    document.body.classList.toggle('zen-mode');
  }
  const isZen = document.body.classList.contains('zen-mode');
  document.body.classList.toggle('zen-entering', isZen);
  if (toolbar) toolbar.classList.remove('zen-toolbar-revealed');

  if (isZen) {
    document.body.classList.add('zen-toolbar-suppressed');
    const reader = document.getElementById('content');
    if (toolbar) {
      document.body.style.setProperty('--zen-toolbar-height', `${toolbarHeight}px`);
    }
    if (reader && !state.editing) {
      reader.tabIndex = -1;
      reader.focus({ preventScroll: true });
    } else if (window.cmView) {
      window.cmView.focus();
    }
    requestAnimationFrame(() => {
      requestAnimationFrame(() => document.body.classList.remove('zen-entering'));
    });
    window.addEventListener('pointermove', event => {
      if (event.clientY > 12) document.body.classList.remove('zen-toolbar-suppressed');
    }, { once: true });
  } else {
    document.body.classList.remove('zen-toolbar-suppressed');
    document.body.style.removeProperty('--zen-toolbar-height');
    showToast(_t('toast.zenExited') || '已退出禅模式', 1200);
  }
}
window.toggleZenMode = toggleZenMode;

let presentationOpener = null;
let presZenInitTimer = null;
let presToolbarHideTimer = null;
let presControlsHideTimer = null;
let presZenActive = false;

async function togglePresentationFullscreen(modal) {
  const doc = document;
  const nativeState = !!window.__readmdNativeFullscreen;
  const isFull = nativeState || !!(doc.fullscreenElement || doc.webkitFullscreenElement || doc.mozFullScreenElement || doc.msFullscreenElement);
  const btn = $('presentation-fullscreen-btn');
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;

  // pywebview embeds the page in a native window where the browser Fullscreen
  // API is unavailable. Prefer the host window in that environment and keep a
  // local state bit so Esc/F11 can restore the previous window state.
  if (window.pywebview?.api?.toggle_native_fullscreen) {
    const native = await window.pywebview.api.toggle_native_fullscreen();
    if (native && native.supported) {
      window.__readmdNativeFullscreen = !isFull;
      if (btn) {
        btn.classList.toggle('active', !isFull);
        btn.textContent = !isFull ? (_t('presentation.exitFullscreenLabel') || '退出全屏') : (_t('presentation.fullscreenLabel') || '全屏');
      }
      return;
    }
  }

  if (!isFull) {
    const target = modal || doc.documentElement;
    const req = target.requestFullscreen || target.webkitRequestFullscreen || target.mozRequestFullScreen || target.msRequestFullscreen || doc.documentElement.requestFullscreen || doc.documentElement.webkitRequestFullscreen;
    if (req) {
      try {
        await req.call(target);
      } catch (e) {
        try {
          if (doc.documentElement.requestFullscreen) await doc.documentElement.requestFullscreen();
        } catch (err) {}
      }
    }
    if (btn) {
      btn.classList.add('active');
      btn.textContent = _t('presentation.exitFullscreenLabel') || '退出全屏';
    }
  } else {
    const exit = doc.exitFullscreen || doc.webkitExitFullscreen || doc.mozCancelFullScreen || doc.msExitFullscreen;
    if (exit) {
      try {
        await exit.call(doc);
      } catch (e) {}
    }
    if (btn) {
      btn.classList.remove('active');
      btn.textContent = _t('presentation.fullscreenLabel') || '全屏';
    }
  }
}
window.togglePresentationFullscreen = togglePresentationFullscreen;

window.closePresentationMode = function closePresentationMode() {
  const modal = document.getElementById('presentation-modal');
  if (!modal || modal.classList.contains('hidden')) return false;

  clearTimeout(presZenInitTimer);
  clearTimeout(presToolbarHideTimer);
  clearTimeout(presControlsHideTimer);
  presZenActive = false;

  if (window.__readmdNativeFullscreen && window.pywebview?.api?.toggle_native_fullscreen) {
    window.pywebview.api.toggle_native_fullscreen().catch(() => {});
    window.__readmdNativeFullscreen = false;
  }
  if ((document.fullscreenElement || document.webkitFullscreenElement) && document.exitFullscreen) {
    document.exitFullscreen().catch(() => {});
  }
  modal.classList.add('hidden');
  modal.classList.remove('pres-zen-active');
  const toolbar = modal.querySelector('#presentation-toolbar');
  if (toolbar) toolbar.classList.remove('pres-toolbar-revealed');

  const iframe = modal.querySelector('.presentation-iframe');
  if (iframe) iframe.src = 'about:blank';
  if (presentationOpener?.isConnected) presentationOpener.focus({ preventScroll: true });
  presentationOpener = null;
  return true;
};

async function launchPresentationMode() {
  const content = rewritePresentationAssets(state.original || (cmView ? cmView.state.doc.toString() : ''));
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!content) {
    showToast(_t('presentation.noDoc') || '当前没有可演示的文档内容');
    return;
  }
  if (!document.activeElement?.closest('#presentation-modal')) presentationOpener = document.activeElement;
  let modal = document.getElementById('presentation-modal');

  const postToIframe = (data) => {
    const targetModal = document.getElementById('presentation-modal');
    const iframe = targetModal?.querySelector('.presentation-iframe');
    if (iframe && iframe.contentWindow) {
      iframe.contentWindow.postMessage(data, '*');
    }
  };

  if (!modal) {
    modal = document.createElement('div');
    modal.id = 'presentation-modal';
    modal.className = 'hidden';
    modal.setAttribute('role', 'dialog');
    modal.setAttribute('aria-modal', 'true');
    modal.setAttribute('aria-label', _t('menu.presentation') || '演讲演示');
    modal.innerHTML = `
      <div class="presentation-toolbar" id="presentation-toolbar">
        <div class="presentation-tool-item">
          <select id="presentation-theme-select" class="presentation-select" title="演说主题" data-i18n-title="presentation.themeTitle">
            <option value="black">Black</option>
            <option value="white">White</option>
            <option value="league">League</option>
            <option value="beige">Beige</option>
            <option value="night">Night</option>
            <option value="serif">Serif</option>
            <option value="simple">Simple</option>
            <option value="solarized">Solarized</option>
            <option value="blood">Blood</option>
            <option value="moon">Moon</option>
            <option value="sky">Sky</option>
          </select>
        </div>
        <div class="presentation-tool-item">
          <select id="presentation-transition-select" class="presentation-select" title="转场特效" data-i18n-title="presentation.transitionTitle">
            <option value="slide">${_t('presentation.transitionSlide')}</option>
            <option value="fade">${_t('presentation.transitionFade')}</option>
            <option value="zoom">${_t('presentation.transitionZoom')}</option>
            <option value="convex">${_t('presentation.transitionConvex')}</option>
            <option value="concave">${_t('presentation.transitionConcave')}</option>
            <option value="none">${_t('presentation.transitionNone')}</option>
          </select>
        </div>
        <div class="presentation-tool-item">
          <button type="button" class="presentation-btn" id="presentation-font-dec" title="缩小字号 (20px)" data-i18n-title="presentation.fontDec">A-</button>
          <button type="button" class="presentation-btn active" id="presentation-font-norm" title="标准字号 (24px)" data-i18n-title="presentation.fontNorm">A</button>
          <button type="button" class="presentation-btn" id="presentation-font-inc" title="放大字号 (28px)" data-i18n-title="presentation.fontInc">A+</button>
        </div>
        <button type="button" class="presentation-btn" id="presentation-overview-btn" title="${_t('presentation.overviewTitle')}">${_t('presentation.overviewLabel')}</button>
        <button type="button" class="presentation-btn" id="presentation-fullscreen-btn" title="${_t('presentation.fullscreenTitle')}">${_t('presentation.fullscreenLabel')}</button>
        <button type="button" class="presentation-close-btn" id="presentation-close-btn" title="${_t('presentation.closeTitle')}" data-i18n-title="presentation.closeTitle"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M18 6L6 18M6 6l12 12"/></svg></button>
      </div>
      <iframe class="presentation-iframe" title="${_t('menu.presentation') || '演讲演示'}" src="about:blank"></iframe>
    `;
    document.body.appendChild(modal);

    const themeSelect = $('presentation-theme-select');
    if (themeSelect) {
      themeSelect.addEventListener('change', () => {
        postToIframe({ type: 'set-theme', theme: themeSelect.value });
      });
    }

    const transSelect = $('presentation-transition-select');
    if (transSelect) {
      transSelect.addEventListener('change', () => {
        postToIframe({ type: 'set-transition', transition: transSelect.value });
      });
    }

    const setFontSize = (size, activeBtnId) => {
      postToIframe({ type: 'set-font-size', size: size });
      ['presentation-font-dec', 'presentation-font-norm', 'presentation-font-inc'].forEach(id => {
        if ($(id)) $(id).classList.toggle('active', id === activeBtnId);
      });
    };

    if ($('presentation-font-dec')) $('presentation-font-dec').addEventListener('click', () => setFontSize(20, 'presentation-font-dec'));
    if ($('presentation-font-norm')) $('presentation-font-norm').addEventListener('click', () => setFontSize(24, 'presentation-font-norm'));
    if ($('presentation-font-inc')) $('presentation-font-inc').addEventListener('click', () => setFontSize(28, 'presentation-font-inc'));

    if ($('presentation-overview-btn')) {
      $('presentation-overview-btn').addEventListener('click', () => {
        postToIframe({ type: 'toggle-overview' });
      });
    }

    if ($('presentation-fullscreen-btn')) {
      $('presentation-fullscreen-btn').addEventListener('click', () => {
        togglePresentationFullscreen(modal);
      });
    }

    const closePresentation = () => {
      window.closePresentationMode();
    };

    if ($('presentation-close-btn')) {
      $('presentation-close-btn').addEventListener('click', closePresentation);
    }

    // 监听全局全屏状态变化
    ['fullscreenchange', 'webkitfullscreenchange', 'mozfullscreenchange', 'MSFullscreenChange'].forEach(evt => {
      document.addEventListener(evt, () => {
        const isFull = !!(document.fullscreenElement || document.webkitFullscreenElement || document.mozFullScreenElement || document.msFullscreenElement);
        const btn = $('presentation-fullscreen-btn');
        if (btn) {
          btn.classList.toggle('active', isFull);
          btn.textContent = isFull ? (_t('presentation.exitFullscreenLabel') || '退出全屏') : (_t('presentation.fullscreenLabel') || '全屏');
        }
      });
    });

    // 监听来自 iframe 的消息（包含鼠标移动和全屏快捷键）
    window.addEventListener('message', (event) => {
      if (!event.data || typeof event.data !== 'object') return;
      if (event.data.type === 'pres-toggle-fullscreen') {
        togglePresentationFullscreen(modal);
      } else if (event.data.type === 'pres-mousemove') {
        handlePresPointerMove(event.data.clientX, event.data.clientY, event.data.innerWidth || window.innerWidth, event.data.innerHeight || window.innerHeight);
      }
    });

    modal.addEventListener('mousemove', (e) => {
      handlePresPointerMove(e.clientX, e.clientY, window.innerWidth, window.innerHeight);
    });
  }

  // 禅模式位置感应与渐隐逻辑
  function handlePresPointerMove(x, y, w, h) {
    if (!presZenActive) return;
    const toolbar = modal.querySelector('#presentation-toolbar');

    // 1. 顶部感应：鼠标移入上方（y <= 85）立即显现工具栏；移开后 3 秒渐渐消失
    if (y <= 85) {
      if (presToolbarHideTimer) {
        clearTimeout(presToolbarHideTimer);
        presToolbarHideTimer = null;
      }
      if (toolbar) toolbar.classList.add('pres-toolbar-revealed');
    } else {
      if (toolbar && toolbar.classList.contains('pres-toolbar-revealed') && !presToolbarHideTimer) {
        presToolbarHideTimer = setTimeout(() => {
          if (toolbar) toolbar.classList.remove('pres-toolbar-revealed');
          presToolbarHideTimer = null;
        }, 3000);
      }
    }

    // 2. 右下角感应：鼠标移入右下角（x >= w - 220 且 y >= h - 140）显现翻页按钮；移开后 3 秒渐渐消失
    const inBottomRight = (x >= (w || window.innerWidth) - 220) && (y >= (h || window.innerHeight) - 140);
    if (inBottomRight) {
      if (presControlsHideTimer) {
        clearTimeout(presControlsHideTimer);
        presControlsHideTimer = null;
      }
      postToIframe({ type: 'set-zen-controls', showControls: true });
    } else {
      if (!presControlsHideTimer) {
        presControlsHideTimer = setTimeout(() => {
          postToIframe({ type: 'set-zen-controls', showControls: false });
          presControlsHideTimer = null;
        }, 3000);
      }
    }
  }

  // 重置并初始化禅模式定时器（进入后保持 5 秒，之后自动收回工具栏与翻页按钮，常驻保留页码）
  clearTimeout(presZenInitTimer);
  clearTimeout(presToolbarHideTimer);
  clearTimeout(presControlsHideTimer);
  presZenActive = false;
  modal.classList.remove('pres-zen-active');
  const toolbar = modal.querySelector('#presentation-toolbar');
  if (toolbar) toolbar.classList.remove('pres-toolbar-revealed');

  presZenInitTimer = setTimeout(() => {
    presZenActive = true;
    modal.classList.add('pres-zen-active');
    postToIframe({ type: 'set-zen-controls', showControls: false });
  }, 5000);

  showToast(_t('toast.generatingPresentation') || '正在生成演示文稿...', 1000);
  try {
    let res;
    if (hasPy && py.export_presentation) {
      res = await py.export_presentation(content);
    } else {
      const r = await apiFetch('/api/export/presentation', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content: content })
      });
      res = await r.json();
    }
    if (res && res.ok && res.html) {
      modal.classList.remove('hidden');
      const iframe = modal.querySelector('.presentation-iframe');
      iframe.srcdoc = res.html;
      $('presentation-theme-select')?.focus({ preventScroll: true });
    } else {
      showToast((_t('toast.presentationFail') || '演示文稿生成失败：') + ((res && res.error) || '未知错误'));
    }
  } catch (e) {
    showToast((_t('toast.presentationFail') || '演示文稿生成失败：') + e.message);
  }
}
window.launchPresentationMode = launchPresentationMode;


function resolvePath(baseDir, rel) {
  try {
    const url = new URL(rel, 'file:///' + String(baseDir || '').replace(/\\/g, '/') + '/');
    return decodeURIComponent(url.pathname.replace(/^\//, ''));
  } catch (e) {
    return rel;
  }
}

/* 演示模式专用：把相对路径图片改写为 /raw 本地端点，避免 srcdoc 内相对路径失效 */
function rewritePresentationAssets(md) {
  if (!md || !state.dir) return md;
  const isRel = u => u && !/^(https?:|data:|blob:|file:|\/\/|\/)/i.test(u);
  const toRaw = u => '/raw?p=' + encodeURIComponent(resolvePath(state.dir, u));
  let out = md.replace(/(!\[[^\]]*\]\()([^)\s]+)([^)]*\))/g,
    (m, pre, src, post) => isRel(src) ? pre + toRaw(src) + post : m);
  out = out.replace(/(<img\b[^>]*\bsrc=["'])([^"']+)(["'])/gi,
    (m, pre, src, post) => isRel(src) ? pre + toRaw(src) + post : m);
  return out;
}

function fixLinks(body) {
  const allHeadings = Array.from(body.querySelectorAll('h1, h2, h3, h4, h5, h6'));
  body.querySelectorAll('a').forEach(a => {
    const href = a.getAttribute('href') || '';
    if (href.startsWith('#')) {
      const targetId = href.slice(1);
      a.addEventListener('click', e => {
        e.preventDefault();
        let el = document.getElementById(targetId);
        if (!el) {
          try {
            const decoded = decodeURIComponent(targetId);
            el = document.getElementById(decoded) || body.querySelector('[name="' + CSS.escape(decoded) + '"]');
          } catch (ex) {}
        }
        if (!el) {
          el = findMatchingHeading(targetId, a.textContent, allHeadings);
        }
        if (el) {
          el.tabIndex = -1;
          el.focus({ preventScroll: true });
          el.scrollIntoView({ behavior: preferredScrollBehavior(), block: 'start' });
          el.classList.remove('heading-target-highlight');
          void el.offsetWidth;
          el.classList.add('heading-target-highlight');
          setTimeout(() => el.classList.remove('heading-target-highlight'), 1500);
        } else {
          const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
          showToast((_t('toast.headingNotFound') || '未找到对应的文档小标题目标：') + (a.textContent || href), 2500);
        }
      });
      return;
    }

    if (/^(https?:|mailto:)/i.test(href)) {
      a.addEventListener('click', e => {
        e.preventDefault();
        if (hasPy) py.open_external(href); else window.open(href, '_blank');
      });
    } else if (/^file:/i.test(href)) {
      const p = decodeURIComponent(href.replace(/^file:\/\//i, ''));
      a.addEventListener('click', e => { e.preventDefault(); openPath(p); });
    } else {
      const p = resolvePath(state.dir, href.split('#')[0]);
      a.addEventListener('click', e => {
        e.preventDefault();
        if (MD_RE.test(p)) loadFile(p);
        else openPath(p);
      });
    }
  });
}

function fixImages(body) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  body.querySelectorAll('img').forEach(im => {
    let src = im.getAttribute('src') || '';
    if (/^(https?:|data:)/i.test(src)) return;
    let p;
    if (src.startsWith('file:')) {
      p = decodeURIComponent(src.replace(/^file:\/\//i, ''));
    } else if (/^[A-Za-z]:[\\/]/.test(src)) {
      p = src; // 绝对 Windows 路径（如转换器提取的临时图片）
    } else if (src.startsWith('/raw?')) {
      return; // 已经处理过
    } else {
      p = resolvePath(state.dir, src);
    }
    im.src = '/raw?p=' + encodeURIComponent(p);
    im.onerror = () => {
      im.style.opacity = .45;
      im.alt = (_t('reader.imgLoadFailAlt') || '[图片无法加载] ') + im.alt;
    };
  });
}

function openPath(p) {
  if (hasPy) py.open_path(p);
  else window.open('/raw?p=' + encodeURIComponent(p), '_blank');
}

/* ---------------- 虚拟文档（转换/网页/OCR） ---------------- */

async function renderVirtual(source, name, dir, content, fixes, extras) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  exitEdit();
  let cleanName = (name || '').trim();
  if (cleanName && !/\.md$/i.test(cleanName)) {
    cleanName = cleanName.replace(/\.[^./\\]+$/, '') + '.md';
  }
  const title = cleanName || ((_t('tabs.untitled') || '未命名') + '.md');
  const newTab = {
    id: 'tab_' + Date.now() + '_' + Math.random().toString(36).slice(2, 6),
    mode: 'virtual',
    source: source || 'virtual',
    path: null,
    dir: dir || '',
    name: title,
    title: title,
    content: content,
    original: content,
    fixed: content,
    fixes: fixes || [],
    stats: {},
    size: 0,
    mtime: 0,
    encoding: 'utf-8',
    webAssets: source === 'url' ? (((extras || {}).assets) || []) : [],
    originPath: (extras && extras.originPath) || state.file || null,
    isDirty: source === 'clipboard' || source === 'ai',
    scrollPos: 0,
    isVirtual: true,
  };
  window.invalidateDocumentLoads?.();
  state.tabs.push(newTab);
  state.activeTabId = newTab.id;
  syncStateFromActiveTab();
  setFixes(fixes || [], {});
  clearAiOutput();
  renderContent(content, title);
  document.title = title + ' - ReadMD';
  setFileTitle(title.slice(0, 80), false, '');
  $('btn-reload').disabled = true;
  updateStatus();
  renderTabsBar();
  setProgress(100);
  afterRender();
}


async function ensureModule(name, timeoutMs) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
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
      if (st === 'error') {
        delete moduleLoadRequests[name];
        showToast(_t('toast.moduleLoadFail', { name }) || ('模块「' + name + '」加载失败，请重试'));
        return false;
      }
    } catch (e) { /* ignore */ }
    await new Promise(r => setTimeout(r, 800));
  }
  showToast(_t('toast.moduleTimeout') || '模块加载超时，请重试');
  return false;
}

async function convertFile(path) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!(await ensureModule('convert'))) return;
  busy(true);
  try {
    const r = await apiFetch('/api/convert?p=' + encodeURIComponent(path));
    const d = await r.json();
    if (r.status === 409) { showToast(d.error || (_t('toast.moduleLoading') || '模块加载中…')); return; }
    if (!r.ok) { showToast(d.error || (_t('toast.convertFailed') || '转换失败')); return; }
    if (!d.content) { showToast(d.note || (_t('toast.convertNoContent') || '未提取到内容')); return; }
    showConvertWarns(d.warns);
    if (d.saved && d.out) {
      showToast((_t('toast.savedPrefix') || '已保存：') + d.out);
      await loadFile(d.out);
    } else if (d.skipped) {
      showToast(_t('toast.skippedExistsNotice') || '已存在同名 .md，跳过保存（可在批量转换中勾选“覆盖已存在”）', 3400);
      renderVirtual('convert', d.name, d.dir, d.content, d.fixes);
    } else {
      renderVirtual('convert', d.name, d.dir, d.content, d.fixes);
    }
  } catch (e) { showToast((_t('toast.convertFailPrefix') || '转换失败：') + e.message); }
  finally { busy(false); }
}

function showConvertWarns(warns) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!warns || !warns.length) return;
  const bad = warns.filter(w => w.level === 'warn' || w.level === 'error');
  if (bad.length) showToast(_t('toast.convertWarns', { count: bad.length, first: (bad[0].msg || (_t('toast.seeCheckReport') || '见校验报告')) }) || ('转换完成，' + bad.length + ' 条质量警告（' + (bad[0].msg || '见校验报告') + '）'), 3600);
}


function loadFileDialog() {
  if (hasPy) {
    py.choose_file().then(p => {
      if (p) {
        if (typeof CONVERT_BINARY_RE !== 'undefined' && CONVERT_BINARY_RE.test(p)) {
          convertOrOcr(p, 'convert');
        } else {
          loadFile(p);
        }
      }
    });
    return;
  }
  const input = $('file-input');
  input.value = '';
  input.onchange = async () => {
    const f = input.files && input.files[0];
    if (!f) return;
    const p = await uploadFile(f);
    if (p && MD_RE.test(p)) loadFile(p, { browserCopy: true });
    else if (p) convertOrOcr(p, 'convert');
  };
  input.click();
}
