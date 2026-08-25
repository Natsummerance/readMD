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
  if (existingTab && !force) {
    await switchTab(existingTab.id);
    return;
  }
  if (existingTab && (existingTab.isDirty || (state.activeTabId === existingTab.id && state.editing))) {
    showToast(_t('toast.reloadBlockedDirty') || '未保存修改已保留，未重新加载外部更改');
    return;
  }
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
    };

    if (existingTab) {
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
        setFixes(d.fixes || [], d.stats || {});
        await renderContent(d.content, d.name);
        if (state.pagination.enabled && state.pagination.mode === 'paged' && previousPage > 0) {
          renderPage(previousPage, null, true);
        }
        requestAnimationFrame(() => {
          $('content').scrollTop = previousScroll;
        });
        updateStatus();
      }
      showToast(_t('toolbar.reload') + ': ' + d.name);
    } else {
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
      setFixes(d.fixes || [], d.stats || {});
      await renderContent(d.content, d.name);
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
    const finish = accepted => {
      modal.classList.add('hidden');
      $('continuous-confirm').onclick = null;
      $('continuous-cancel').onclick = null;
      resolve(accepted);
    };
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

  if (p.mode === 'paged') {
    if (p.pages.length > 20 && !(await confirmContinuousMode(p.pages.length))) {
      return;
    }
    p.mode = 'continuous';
    if (activeTab) {
      activeTab.readerMode = 'continuous';
      activeTab.continuousScroll = $('content')?.scrollTop || 0;
    }
    showToast(_t('pagination.switchToContinuousToast') || '已切换至全卷连续阅读模式', 1800);
    renderContentIncremental(p.rawContent, 0);
  } else {
    p.mode = 'paged';
    if (activeTab) {
      activeTab.readerMode = 'paged';
      activeTab.readerPage = 0;
    }
    showToast(_t('pagination.switchToPagedToast') || '已切换至智能分页阅读模式', 1800);
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
      const parts = info.split(/\s+/);
      const lang = parts[0] || '';
      const hasCmd = info.includes('cmd=true') || info.includes('cmd=True') || info.includes('{cmd}');

      // 1. Interactive Code Chunk
      if (hasCmd) {
        const encodedCode = encodeURIComponent(code);
        const isMatplotlib = info.includes('matplotlib=true') || info.includes('matplotlib=True');
        const isHidden = info.includes('hide=true') || info.includes('hide=True');
        return `<div class="code-chunk-card" ${lineAttr} data-lang="${lang}" data-code="${encodedCode}" data-matplotlib="${isMatplotlib}" data-hide="${isHidden}">
          <div class="code-chunk-header">
            <span class="code-chunk-badge">${lang.toUpperCase()}</span>
            <span class="code-chunk-status" role="status" aria-live="polite">${_t('status.ready') || 'Ready'}</span>
            <span class="code-chunk-timer"></span>
            <div class="code-chunk-actions">
              <button class="code-chunk-run-btn" title="${_t('menu.runCode')} (Shift+Enter)" aria-label="${_t('menu.runCode')}">▶ ${_t('menu.runCode')}</button>
            </div>
          </div>
          <div class="code-chunk-src ${isHidden ? 'hidden' : ''}">
            <pre><code class="language-${lang}">${escaped ? code : (window.escapeHtml ? escapeHtml(code) : code)}</code></pre>
          </div>
          <div class="code-chunk-output hidden">
            <div class="code-chunk-output-header">
              <span>${_t('reader.executionOutput')}</span>
              <div class="code-chunk-out-actions">
                <button class="code-chunk-copy-btn" title="${_t('reader.copyOutput')}" aria-label="${_t('reader.copyOutput')}">📋 ${_t('reader.copyOutput')}</button>
                <button class="code-chunk-clear-btn" title="${_t('reader.clearOutput')}" aria-label="${_t('reader.clearOutput')}">✕ ${_t('reader.clearOutput')}</button>
              </div>
            </div>
            <pre class="code-chunk-stdout"></pre>
            <div class="code-chunk-plot"></div>
          </div>
        </div>\n`;
      }

      // 2. Specialized Diagrams
      const diagramLangs = ['tikz', 'plantuml', 'puml', 'wavedrom', 'bitfield', 'viz', 'dot', 'graphviz', 'vega', 'vega-lite', 'd2', 'ditaa'];
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

function sanitizeRenderedHtml(html) {
  const template = document.createElement('template');
  template.innerHTML = String(html || '');
  Array.from(template.content.querySelectorAll('*')).forEach(node => {
    const tag = node.tagName.toLowerCase();
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
      if (name.startsWith('data-') || name === 'class' || name.startsWith('aria-') ||
          ['title', 'lang', 'dir', 'role', 'alt', 'width', 'height', 'loading',
           'colspan', 'rowspan', 'datetime', 'cite'].includes(name)) return;
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

async function renderContent(content, name) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const saved = state.scrollPos[normalizePath(name || state.file || '')] || 0;
  
  // 预处理 @import
  if (content && /@import\s+["']/.test(content)) {
    try {
      content = await processDocImports(content, state.file || name || '');
    } catch (e) {}
  }

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
  const html = parseMarkdownWithSourceMap(prot.src);
  const finalHtml = restoreMath(html, prot.saved);
  $('content').innerHTML = '<article class="markdown-body">' + sanitizeRenderedHtml(finalHtml) + '</article>';
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
    body.innerHTML = sanitizeRenderedHtml(restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved));
    postProcess();
    if (savedTop) el.scrollTop = savedTop;
    return;
  }
  const prog = document.createElement('div');
  prog.id = 'render-progress';
  prog.setAttribute('role', 'status');
  prog.setAttribute('aria-live', 'polite');
  prog.setAttribute('aria-atomic', 'true');
  el.appendChild(prog);
  const CHUNK = 8;
  for (let i = 0; i < total; i += CHUNK) {
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
        btn.textContent = '⏳ ' + _t('reader.codeRunning');
      const t0 = Date.now();
      const interval = setInterval(() => {
        timerEl.textContent = ((Date.now() - t0) / 1000).toFixed(1) + 's';
      }, 100);

      try {
        let res;
        if (hasPy && py.run_code_chunk) {
          res = await py.run_code_chunk(lang, code);
        } else {
          const r = await apiFetch('/api/code/run', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ lang: lang, code: code })
          });
          res = await r.json();
        }

        clearInterval(interval);
        timerEl.textContent = ((Date.now() - t0) / 1000).toFixed(2) + 's';

        if (res && res.ok) {
          statusEl.className = 'code-chunk-status success';
          statusEl.textContent = _t('ai.statusComplete');
          outWrap.classList.remove('hidden');
          stdoutEl.textContent = (res.stdout || '') + (res.stderr ? ('\n' + res.stderr) : '');
          if (!stdoutEl.textContent.trim()) stdoutEl.textContent = _t('reader.noConsoleOutput');
          plotEl.innerHTML = '';
          if (res.images && res.images.length > 0) {
            res.images.forEach(imgSrc => {
              const img = document.createElement('img');
              img.src = imgSrc;
              img.alt = `${lang} plot output`;
              plotEl.appendChild(img);
            });
          }
        } else {
          statusEl.className = 'code-chunk-status error';
          statusEl.textContent = _t('convert.statusFailed');
          outWrap.classList.remove('hidden');
          stdoutEl.textContent = (res && res.error) || (res && res.stderr) || _t('toast.unknownError');
        }
      } catch (err) {
        clearInterval(interval);
        statusEl.className = 'code-chunk-status error';
        statusEl.textContent = _t('reader.callFailed');
        outWrap.classList.remove('hidden');
        stdoutEl.textContent = err.message || String(err);
      } finally {
        btn.disabled = false;
        btn.textContent = '▶ ' + _t('reader.runAgain');
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

async function runAllCodeChunks() {
  const cards = document.querySelectorAll('.code-chunk-card');
  if (!cards.length) {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
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

function renderAllDiagrams(container) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const cards = (container || document).querySelectorAll('.diagram-card');
  cards.forEach(async card => {
    if (card._rendered) return;
    card._rendered = true;
    const engine = card.dataset.diagramEngine || 'plantuml';
    const code = decodeURIComponent(card.dataset.diagramCode || '');
    const previewEl = card.querySelector('.diagram-preview');
    const reloadBtn = card.querySelector('.diagram-reload-btn');

    const render = async () => {
      previewEl.innerHTML = `<div class="diagram-loading">⏳ ${_t('reader.renderingDiagram', { engine: engine.toUpperCase() })}</div>`;
      try {
        if (engine === 'mermaid' && window.mermaid) {
          const id = 'mermaid-' + Math.random().toString(36).slice(2);
          const { svg } = await window.mermaid.render(id, code);
          previewEl.innerHTML = svg;
          return;
        }

        let res;
        if (hasPy && py.render_diagram) {
          res = await py.render_diagram(engine, code);
        } else {
          const r = await apiFetch('/api/diagram/render', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ engine: engine, code: code })
          });
          res = await r.json();
        }

        if (res && res.ok) {
          if (res.type === 'url' && res.svg_url) {
            previewEl.innerHTML = `<img src="${res.svg_url}" alt="${engine} diagram" style="max-width:100%;" />`;
          } else if (res.type === 'html' && res.html) {
            previewEl.innerHTML = res.html;
          } else if (res.svg) {
            previewEl.innerHTML = res.svg;
          } else {
            // Kroki 缺省在线矢量渲染
            const krokiUrl = `https://kroki.io/${engine}/svg`;
            const kr = await fetch(krokiUrl, {
              method: 'POST',
              headers: { 'Content-Type': 'text/plain; charset=utf-8' },
              body: code
            });
            if (kr.ok) {
              const svgText = await kr.text();
              previewEl.innerHTML = svgText;
            } else {
              previewEl.innerHTML = `<div class="diagram-fallback-wrap"><div class="diagram-fallback-hint">⚠️ ${_t('reader.renderFailed', { error: _t('toast.unknownNetworkErr') })}</div><pre class="diagram-fallback"><code>${window.escapeHtml ? escapeHtml(code) : code}</code></pre></div>`;
            }
          }
        } else {
          previewEl.innerHTML = `<div class="diagram-fallback-wrap"><div class="diagram-fallback-hint">⚠️ ${_t('reader.diagramError', { error: (res && res.error) || _t('toast.unknownError') })}</div><pre class="diagram-fallback"><code>${window.escapeHtml ? escapeHtml(code) : code}</code></pre></div>`;
        }
      } catch (err) {
        previewEl.innerHTML = `<div class="diagram-fallback-wrap"><div class="diagram-fallback-hint">⚠️ ${_t('reader.renderFailed', { error: err.message || String(err) })}</div><pre class="diagram-fallback"><code>${window.escapeHtml ? escapeHtml(code) : code}</code></pre></div>`;
      }
    };

    if (reloadBtn) reloadBtn.addEventListener('click', render);
    render();
  });
}
window.renderAllDiagrams = renderAllDiagrams;

function toggleZenMode(force) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (typeof force === 'boolean') {
    document.body.classList.toggle('zen-mode', force);
  } else {
    document.body.classList.toggle('zen-mode');
  }
  const isZen = document.body.classList.contains('zen-mode');
  const toolbar = document.getElementById('toolbar');
  if (toolbar) toolbar.classList.remove('zen-toolbar-revealed');
  
  if (isZen) {
    showToast(_t('toast.zenEntered') || '已进入禅模式（鼠标移至顶部可唤出工具栏，按 Esc 退出）', 2200);
    if (window.cmView) window.cmView.focus();
  } else {
    showToast(_t('toast.zenExited') || '已退出禅模式', 1200);
  }
}
window.toggleZenMode = toggleZenMode;

async function launchPresentationMode() {
  const content = rewritePresentationAssets(state.original || (cmView ? cmView.state.doc.toString() : ''));
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!content) {
    showToast(_t('presentation.noDoc') || '当前没有可演示的文档内容');
    return;
  }
  let modal = document.getElementById('presentation-modal');
  if (!modal) {
    modal = document.createElement('div');
    modal.id = 'presentation-modal';
    modal.className = 'hidden';
    modal.innerHTML = `
      <div class="presentation-toolbar" id="presentation-toolbar">
        <div class="presentation-tool-item">
          <select id="presentation-theme-select" class="presentation-select" title="演说主题" data-i18n-title="presentation.themeTitle">
            <option value="black">深黑 (Black)</option>
            <option value="white">亮白 (White)</option>
            <option value="league">英雄联盟 (League)</option>
            <option value="beige">米黄 (Beige)</option>
            <option value="night">暗夜 (Night)</option>
            <option value="serif">衬线典雅 (Serif)</option>
            <option value="simple">极简现代 (Simple)</option>
            <option value="solarized">日光色调 (Solarized)</option>
            <option value="blood">暗红 (Blood)</option>
            <option value="moon">月光 (Moon)</option>
            <option value="sky">天蓝 (Sky)</option>
          </select>
        </div>
        <div class="presentation-tool-item">
          <select id="presentation-transition-select" class="presentation-select" title="转场特效" data-i18n-title="presentation.transitionTitle">
            <option value="slide">平移 (Slide)</option>
            <option value="fade">渐变 (Fade)</option>
            <option value="zoom">缩放 (Zoom)</option>
            <option value="convex">凸面 (Convex)</option>
            <option value="concave">凹面 (Concave)</option>
            <option value="none">无动画 (None)</option>
          </select>
        </div>
        <div class="presentation-tool-item">
          <button type="button" class="presentation-btn" id="presentation-font-dec" title="缩小字号 (20px)" data-i18n-title="presentation.fontDec">A-</button>
          <button type="button" class="presentation-btn active" id="presentation-font-norm" title="标准字号 (24px)" data-i18n-title="presentation.fontNorm">A</button>
          <button type="button" class="presentation-btn" id="presentation-font-inc" title="放大字号 (28px)" data-i18n-title="presentation.fontInc">A+</button>
        </div>
        <button type="button" class="presentation-btn" id="presentation-overview-btn" title="总览视图 (快捷键 O)" data-i18n-title="presentation.overviewTitle">总览</button>
        <button type="button" class="presentation-btn" id="presentation-fullscreen-btn" title="全屏放映 (F11)" data-i18n-title="presentation.fullscreenTitle">全屏</button>
        <button type="button" class="presentation-close-btn" id="presentation-close-btn" title="退出演示 (Esc)" data-i18n-title="presentation.closeTitle">✕</button>
      </div>
      <iframe class="presentation-iframe" src="about:blank"></iframe>
    `;
    document.body.appendChild(modal);

    const postToIframe = (data) => {
      const iframe = modal.querySelector('.presentation-iframe');
      if (iframe && iframe.contentWindow) {
        iframe.contentWindow.postMessage(data, '*');
      }
    };

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
        if (!document.fullscreenElement) {
          if (modal.requestFullscreen) modal.requestFullscreen();
        } else {
          if (document.exitFullscreen) document.exitFullscreen();
        }
      });
    }

    const closePresentation = () => {
      if (document.fullscreenElement && document.exitFullscreen) {
        document.exitFullscreen().catch(() => {});
      }
      modal.classList.add('hidden');
      const iframe = modal.querySelector('.presentation-iframe');
      if (iframe) iframe.src = 'about:blank';
    };

    if ($('presentation-close-btn')) {
      $('presentation-close-btn').addEventListener('click', closePresentation);
    }
  }

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
    if (p && MD_RE.test(p)) loadFile(p, { browserCopy: true });
    else if (p) convertOrOcr(p, 'convert');
  };
  input.click();
}
