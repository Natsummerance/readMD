'use strict';
/* ============================================================
   ReadMD Reader - In-Document Search & Highlighting
   ============================================================ */

/* ---------------- 搜索 ---------------- */

let globalSearchState = {
  query: '',
  matches: [],       // [{ pageIndex, matchIdxInPage }]
  globalIndex: 0,
};

function clearMarks() {
  state.currentMarks.forEach(m => {
    const p = m.parentNode;
    if (!p) return;
    p.replaceChild(document.createTextNode(m.textContent), m);
    p.normalize();
  });
  state.currentMarks = [];
  state.searchIndex = 0;
  globalSearchState = { query: '', matches: [], globalIndex: 0 };
  updateSearchCount();
}

function doSearch(q, jumpToIdx) {
  clearMarks();
  state.lastQuery = q;
  if (!q) { updateSearchCount(); return; }

  // 1. 分页模式下：在内存中预先对所有页进行关键词索引
  const isPaged = state.pagination && state.pagination.enabled && state.pagination.mode === 'paged' && state.pagination.pages && state.pagination.pages.length;
  if (isPaged) {
    const ql = q.toLowerCase();
    const allMatches = [];
    state.pagination.pages.forEach((pg, pIdx) => {
      const text = pg.content.toLowerCase();
      let pos = 0;
      let countInPage = 0;
      while ((pos = text.indexOf(ql, pos)) !== -1) {
        allMatches.push({ pageIndex: pIdx, matchIdxInPage: countInPage });
        countInPage++;
        pos += ql.length;
      }
    });
    globalSearchState = {
      query: q,
      matches: allMatches,
      globalIndex: typeof jumpToIdx === 'number' ? jumpToIdx : 0,
    };
  }

  // 2. 在当前页 DOM 中生成实际的高亮 mark 标签
  const body = document.querySelector('#content .markdown-body');
  if (!body) return;
  const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT, {
    acceptNode: n => {
      const p = n.parentNode;
      if (!p || p.tagName === 'SCRIPT' || p.tagName === 'STYLE') return NodeFilter.FILTER_REJECT;
      if (p.tagName === 'MARK' && p.classList.contains('hl')) return NodeFilter.FILTER_REJECT;
      return n.textContent.toLowerCase().includes(q.toLowerCase()) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
    },
  });
  const nodes = [];
  let node;
  while ((node = walker.nextNode())) nodes.push(node);
  nodes.forEach(n => {
    const text = n.textContent;
    const lower = text.toLowerCase();
    const ql = q.toLowerCase();
    const frag = document.createDocumentFragment();
    let i = 0, idx;
    while ((idx = lower.indexOf(ql, i)) !== -1) {
      if (idx > i) frag.appendChild(document.createTextNode(text.slice(i, idx)));
      const mark = document.createElement('mark');
      mark.className = 'hl';
      mark.textContent = text.slice(idx, idx + q.length);
      frag.appendChild(mark);
      state.currentMarks.push(mark);
      i = idx + q.length;
    }
    if (i < text.length) frag.appendChild(document.createTextNode(text.slice(i)));
    n.parentNode.replaceChild(frag, n);
  });

  updateSearchCount();

  if (isPaged && globalSearchState.matches.length > 0) {
    const curMatch = globalSearchState.matches[globalSearchState.globalIndex];
    if (curMatch && curMatch.pageIndex === state.pagination.currentPage) {
      jumpToLocalMark(curMatch.matchIdxInPage);
    }
  } else if (state.currentMarks.length > 0) {
    jumpToLocalMark(0);
  }
}

function jumpToLocalMark(idx) {
  if (!state.currentMarks.length) return;
  state.searchIndex = Math.max(0, Math.min(idx, state.currentMarks.length - 1));
  state.currentMarks.forEach((m, i) => m.classList.toggle('cur', i === state.searchIndex));
  const m = state.currentMarks[state.searchIndex];
  if (m) m.scrollIntoView({ behavior: 'smooth', block: 'center' });
}

function jumpToMark(dir) {
  const isPaged = state.pagination && state.pagination.enabled && state.pagination.mode === 'paged' && globalSearchState.matches.length > 0;

  if (isPaged) {
    const total = globalSearchState.matches.length;
    const nextGlobal = (globalSearchState.globalIndex + dir + total) % total;
    globalSearchState.globalIndex = nextGlobal;
    const targetMatch = globalSearchState.matches[nextGlobal];

    if (targetMatch.pageIndex !== state.pagination.currentPage) {
      renderPage(targetMatch.pageIndex);
      // 页面切换渲染后重新高亮本页并在本页定位对应匹配项
      requestAnimationFrame(() => {
        doSearch(globalSearchState.query, nextGlobal);
      });
    } else {
      updateSearchCount();
      jumpToLocalMark(targetMatch.matchIdxInPage);
    }
    return;
  }

  // 常规/连续模式单页跳转
  if (!state.currentMarks.length) return;
  state.searchIndex = (state.searchIndex + dir + state.currentMarks.length) % state.currentMarks.length;
  state.currentMarks.forEach((m, i) => m.classList.toggle('cur', i === state.searchIndex));
  const m = state.currentMarks[state.searchIndex];
  if (m) m.scrollIntoView({ behavior: 'smooth', block: 'center' });
  updateSearchCount();
}

function updateSearchCount() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const isPaged = state.pagination && state.pagination.enabled && state.pagination.mode === 'paged' && globalSearchState.matches.length > 0;

  if (isPaged) {
    const total = globalSearchState.matches.length;
    const cur = globalSearchState.globalIndex + 1;
    const pIdx = globalSearchState.matches[globalSearchState.globalIndex]?.pageIndex;
    const pageNum = (typeof pIdx === 'number' ? pIdx + 1 : state.pagination.currentPage + 1);
    $('search-count').textContent = total ? `${cur}/${total} (P.${pageNum})` : (state.lastQuery ? (_t('search.noMatches') || 'No results') : '');
    return;
  }

  const total = state.currentMarks.length;
  $('search-count').textContent = total ? ((state.searchIndex % total) + 1) + '/' + total : (state.lastQuery ? (_t('search.noMatches') || 'No results') : '');
}

function toggleSearch() {
  if (state.mode === 'welcome' || (!state.file && !state.original)) return;
  const bar = $('search-bar');
  if (bar.classList.contains('hidden')) {
    bar.classList.remove('hidden');
    $('search-input').focus();
    $('search-input').select();
  } else {
    closeSearch();
  }
}

function closeSearch() {
  $('search-bar').classList.add('hidden');
  clearMarks();
}
