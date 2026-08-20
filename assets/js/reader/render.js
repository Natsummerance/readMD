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
  wrap.innerHTML = '<input id="file-rename-input" class="file-rename-input" value="' + stem.replace(/"/g, '&quot;') + '" spellcheck="false" autocomplete="off" aria-label="' + (_t('tabs.rename') || '文件名称') + '"><span id="file-rename-ext" class="file-rename-ext">' + ext + '</span>';
  title.parentNode.insertBefore(wrap, title.nextSibling);

  const input = wrap.querySelector('#file-rename-input');
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


async function loadFile(path) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!path) return;
  const existingTab = findTabByPath(path);
  if (existingTab) {
    switchTab(existingTab.id);
    return;
  }
  setProgress(8);
  try {
    const r = await apiFetch('/api/file?p=' + encodeURIComponent(path));
    if (!r.ok) {
      const d = await r.json().catch(() => ({}));
      showToast((_t('toast.openFailed') || '无法打开：') + (d.error || r.status));
      return;
    }
    const d = await r.json();
    const newTab = {
      id: 'tab_' + Date.now() + '_' + Math.random().toString(36).slice(2, 6),
      mode: 'file',
      source: 'file',
      path: d.path,
      dir: d.dir,
      name: d.name,
      title: d.name,
      content: d.content,
      original: d.original,
      fixed: d.content,
      fixes: d.fixes || [],
      stats: d.stats || {},
      size: d.size,
      mtime: d.mtime,
      encoding: d.encoding,
      webAssets: [],
      isDirty: false,
      scrollPos: 0,
      isVirtual: false,
    };
    state.tabs.push(newTab);
    state.activeTabId = newTab.id;
    syncStateFromActiveTab();
    await loadDocCitations(d.path);
    setFixes(d.fixes || [], d.stats || {});
    renderContent(d.content, d.name);

    document.title = d.name + ' - ReadMD';
    setFileTitle(d.name, hasPy, d.path);
    addRecent(d.path);
    pushHistory(d.path);
    saveLastFile(d.path);
    updateStatus();
    exitEdit();
    clearAiOutput();
    renderTabsBar();
    setProgress(100);
    if (d.structured) showToast(_t('toast.txtStructureRecognized') || '已智能识别 TXT 结构（标题 / 表格 / 列表 / 目录）');
    afterRender();
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
    if (sel.options.length !== total) {
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

  const stBadge = $('status-pagination');
  if (stBadge) {
    stBadge.textContent = p.mode === 'paged' ? `${cur + 1} / ${total}` : '全卷';
  }
}

function togglePaginationMode() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const p = state.pagination;
  if (!p || !p.enabled) return;

  if (p.mode === 'paged') {
    p.mode = 'continuous';
    showToast(_t('pagination.switchToContinuousToast') || '已切换至全卷连续阅读模式', 1800);
    renderContentIncremental(p.rawContent, 0);
  } else {
    p.mode = 'paged';
    showToast(_t('pagination.switchToPagedToast') || '已切换至智能分页阅读模式', 1800);
    renderPage(0, null, false);
  }
  updatePaginationBar();
}

function renderPage(pageIndex, targetHeadingId, preserveScroll) {
  if (!state.pagination.pages || !state.pagination.pages.length) return;
  pageIndex = Math.max(0, Math.min(pageIndex, state.pagination.pages.length - 1));
  state.pagination.currentPage = pageIndex;
  const page = state.pagination.pages[pageIndex];

  const el = $('content');
  if (!el) return;

  const transformed = transformAcademicCallouts(page.content);
  const prot = protectMath(transformed);
  const html = marked.parse(prot.src, { gfm: true, breaks: false });
  const finalHtml = restoreMath(html, prot.saved);
  el.innerHTML = '<article class="markdown-body">' + finalHtml + '</article>';

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
        targetEl.scrollIntoView({ behavior: 'smooth', block: 'start' });
        targetEl.classList.remove('heading-target-highlight');
        void targetEl.offsetWidth;
        targetEl.classList.add('heading-target-highlight');
        setTimeout(() => targetEl.classList.remove('heading-target-highlight'), 1500);
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

function renderContent(content, name) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const saved = state.scrollPos[normalizePath(name || state.file || '')] || 0;
  const linesCount = (content || '').split('\n').length;
  const isUltraLong = linesCount > PAGINATION_THRESHOLD_LINES || (content || '').length > PAGINATION_THRESHOLD_BYTES;

  if (isUltraLong) {
    state.pagination.enabled = true;
    state.pagination.rawContent = content;
    state.pagination.pages = splitMdIntoPages(content);
    state.pagination.totalPages = state.pagination.pages.length;
    if (state.pagination.mode === 'paged') {
      renderPage(0, null, false);
      showPaginationBar(true);
      return;
    } else {
      renderContentIncremental(content, saved);
      showPaginationBar(true);
      return;
    }
  } else {
    state.pagination.enabled = false;
    state.pagination.pages = [];
    state.pagination.totalPages = 0;
    showPaginationBar(false);
  }

  const big = content.length > INCREMENTAL_THRESHOLD || linesCount > INCREMENTAL_LINES;
  if (big) {
    renderContentIncremental(content, saved);
    return;
  }
  const transformed = transformAcademicCallouts(content);
  const prot = protectMath(transformed);
  const html = marked.parse(prot.src, { gfm: true, breaks: false });
  const finalHtml = restoreMath(html, prot.saved);
  $('content').innerHTML = '<article class="markdown-body">' + finalHtml + '</article>';
  postProcess();
  if (saved) requestAnimationFrame(() => { $('content').scrollTop = saved; });
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

async function renderContentIncremental(content, savedTop) {
  const el = $('content');
  el.innerHTML = '<article class="markdown-body"></article>';
  const body = el.querySelector('.markdown-body');
  const blocks = splitMdBlocks(content);
  const total = blocks.length;
  if (total <= 1) {
    const prot = protectMath(content);
    body.innerHTML = restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved);
    postProcess();
    if (savedTop) el.scrollTop = savedTop;
    return;
  }
  const prog = document.createElement('div');
  prog.id = 'render-progress';
  el.appendChild(prog);
  const CHUNK = 8;
  for (let i = 0; i < total; i += CHUNK) {
    const frag = document.createDocumentFragment();
    const end = Math.min(i + CHUNK, total);
    for (let k = i; k < end; k++) {
      const div = document.createElement('div');
      const prot = protectMath(blocks[k]);
      div.innerHTML = restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved);
      frag.appendChild(div);
    }
    body.appendChild(frag);
    const pct = Math.round((end / total) * 100);
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    if (pct >= 100 || pct % 10 < 8) prog.textContent = (_t('reader.renderingProgress', { percent: Math.min(pct, 100) }) || ('渲染中… ' + Math.min(pct, 100) + '%'));
    if (end < total) await new Promise(r => setTimeout(r, 0));

  }
  prog.remove();
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
    const replaced = text.replace(/\[@([a-zA-Z0-9_\-:]+)\]|@([a-zA-Z0-9_\-:]+)/g, (match, k1, k2) => {
      const citeKey = k1 || k2;
      const entry = currentDocCitations[citeKey];
      if (!entry) return match;
      usedKeys.add(citeKey);
      const label = entry.short_cite || `[${citeKey}]`;
      return `<span class="bib-cite-badge" data-citekey="${citeKey}">${label}</span>`;
    });
    if (replaced !== text) {
      const temp = document.createElement('span');
      temp.innerHTML = replaced;
      while (temp.firstChild) {
        parent.insertBefore(temp.firstChild, n);
      }
      parent.removeChild(n);
    }
  }

  body.querySelectorAll('.bib-cite-badge').forEach(badge => {
    badge.addEventListener('mouseenter', e => showBibHoverCard(e, badge.dataset.citekey));
    badge.addEventListener('mouseleave', () => scheduleHideBibHoverCard());
  });

  if (usedKeys.size > 0 && !body.querySelector('.academic-references')) {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    const refSection = document.createElement('section');
    refSection.className = 'academic-references';
    refSection.innerHTML = '<h3>' + (_t('reader.referencesHeading') || 'References / 参考文献') + '</h3><ol></ol>';
    const ol = refSection.querySelector('ol');

    for (const key of usedKeys) {
      const entry = currentDocCitations[key];
      const li = document.createElement('li');
      li.id = 'ref-' + key;
      if (entry && entry.full_reference) {
        li.innerHTML = marked.parseInline(entry.full_reference);
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
  bibCardEl.innerHTML = `
    <div class="bib-card-title">${entry.title || key}</div>
    <div class="bib-card-author">${entry.author || ''} ${entry.year ? `(${entry.year})` : ''}</div>
    <div class="bib-card-journal">${entry.journal || entry.booktitle || ''}</div>
    <div class="bib-card-actions">
      ${entry.doi ? `<a class="bib-card-btn" href="https://doi.org/${entry.doi}" target="_blank">DOI</a>` : ''}
      <button class="bib-card-btn" id="bib-copy-btn">${_t('reader.copyBibtex') || '复制 BibTeX'}</button>
    </div>
  `;
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


function postProcess() {
  const body = document.querySelector('#content .markdown-body');
  if (!body) return;
  ensureHeadingIds(body);
  fixLinks(body);
  fixImages(body);
  processBibCitations(body);
  buildToc();
  renderMath(body);
}


function resolvePath(baseDir, rel) {
  try {
    const url = new URL(rel, 'file:///' + String(baseDir || '').replace(/\\/g, '/') + '/');
    return decodeURIComponent(url.pathname.replace(/^\//, ''));
  } catch (e) {
    return rel;
  }
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
          el.scrollIntoView({ behavior: 'smooth', block: 'start' });
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
  const title = name || ((_t('tabs.untitled') || '未命名') + '.md');
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
    isDirty: source === 'clipboard',
    scrollPos: 0,
    isVirtual: true,
  };
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
    py.choose_file().then(p => { if (p) loadFile(p); });
    return;
  }
  const input = $('file-input');
  input.value = '';
  input.onchange = async () => {
    const f = input.files && input.files[0];
    if (!f) return;
    const p = await uploadFile(f);
    if (p && MD_RE.test(p)) loadFile(p);
    else if (p) convertOrOcr(p, 'convert');
  };
  input.click();
}
