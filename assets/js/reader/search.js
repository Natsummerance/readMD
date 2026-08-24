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
let enterAdvancePending = false;

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

function pageSearchText(page) {
  if (typeof page.searchText === 'string') return page.searchText;
  const transformed = transformAcademicCallouts(page.content);
  const prot = protectMath(transformed);
  const html = marked.parse(prot.src, { gfm: true, breaks: false });
  const probe = document.createElement('div');
  probe.innerHTML = restoreMath(html, prot.saved);
  page.searchText = (probe.textContent || '').toLowerCase();
  return page.searchText;
}

function highlightTextMatches(body, query) {
  const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT, {
    acceptNode: node => {
      const parent = node.parentNode;
      if (!parent || parent.nodeName === 'SCRIPT' || parent.nodeName === 'STYLE') return NodeFilter.FILTER_REJECT;
      if (parent.nodeName === 'MARK' && parent.classList.contains('hl')) return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  const nodes = [];
  let node;
  while ((node = walker.nextNode())) nodes.push(node);
  const segments = nodes.map(textNode => textNode.textContent);
  const haystack = segments.join('').toLowerCase();
  const needle = query.toLowerCase();
  if (!needle) return;

  const positionAt = offset => {
    for (let index = 0; index < nodes.length; index += 1) {
      const length = segments[index].length;
      if (offset <= length) return { node: nodes[index], start: offset };
      offset -= length;
    }
    return null;
  };

  const ranges = [];
  let position = haystack.indexOf(needle);
  while (position !== -1) {
    const start = positionAt(position);
    const end = positionAt(position + needle.length);
    if (!start || !end) break;
    const range = document.createRange();
    range.setStart(start.node, start.start);
    range.setEnd(end.node, end.start);
    ranges.push(range);
    position = haystack.indexOf(needle, position + needle.length);
  }

  for (let index = ranges.length - 1; index >= 0; index -= 1) {
    const range = ranges[index];
    const mark = document.createElement('mark');
    mark.className = 'hl';
    mark.appendChild(range.extractContents());
    range.insertNode(mark);
    state.currentMarks.unshift(mark);
  }
}

function highlightCurrentPageForSearch(query) {
  const body = document.querySelector('#content .markdown-body');
  if (!body) return false;
  state.currentMarks = [];
  state.searchIndex = 0;
  highlightTextMatches(body, query);
  return true;
}

function focusPagedSearchMatch(match) {
  if (!match || match.pageIndex === state.pagination.currentPage) {
    if (match) jumpToLocalMark(match.matchIdxInPage);
    return;
  }

  // Page replacement removes the previous DOM marks synchronously.  Do not
  // queue the re-highlight behind a frame: low-end devices can starve frames
  // while laying out a long page, which would leave the search result lost.
  renderPage(match.pageIndex);
  if (!highlightCurrentPageForSearch(globalSearchState.query)) return;
  updateSearchCount();
  enterAdvancePending = true;
  jumpToLocalMark(match.matchIdxInPage);
}

function doSearch(q, jumpToIdx, { jump = true } = {}) {
  clearMarks();
  state.lastQuery = q;
  if (!q) {
    enterAdvancePending = false;
    updateSearchCount();
    return;
  }

  // 1. 分页模式下：在内存中预先对所有页进行关键词索引
  const isPaged = state.pagination && state.pagination.enabled && state.pagination.mode === 'paged' && state.pagination.pages && state.pagination.pages.length;
  if (isPaged) {
    const ql = q.toLowerCase();
    if (!Array.isArray(state.pagination.searchText) || state.pagination.searchText.length !== state.pagination.pages.length) {
      state.pagination.searchText = state.pagination.pages.map(pageSearchText);
    }
    const allMatches = [];
    state.pagination.pages.forEach((pg, pIdx) => {
      const text = state.pagination.searchText[pIdx];
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
  if (!isPaged) {
    globalSearchState = { query: q, matches: [], globalIndex: 0 };
  }
  if (!highlightCurrentPageForSearch(q)) return;

  updateSearchCount();
  enterAdvancePending = true;

  if (isPaged && globalSearchState.matches.length > 0) {
    const curMatch = globalSearchState.matches[globalSearchState.globalIndex];
    if (!jump) return;
    if (curMatch && curMatch.pageIndex !== state.pagination.currentPage) {
      focusPagedSearchMatch(curMatch);
      return;
    }
    if (curMatch && curMatch.pageIndex === state.pagination.currentPage) {
      jumpToLocalMark(curMatch.matchIdxInPage);
    }
  } else if (jump && state.currentMarks.length > 0) {
    jumpToLocalMark(0);
  }
}

function focusCurrentSearchMatch() {
  const isPaged = state.pagination?.enabled && state.pagination.mode === 'paged' && globalSearchState.matches.length > 0;
  if (isPaged) {
    const match = globalSearchState.matches[globalSearchState.globalIndex];
    if (!match) return;
    focusPagedSearchMatch(match);
    return;
  }
  jumpToLocalMark(0);
}

function jumpToLocalMark(idx) {
  if (!state.currentMarks.length) return;
  state.searchIndex = Math.max(0, Math.min(idx, state.currentMarks.length - 1));
  state.currentMarks.forEach((m, i) => m.classList.toggle('cur', i === state.searchIndex));
  const m = state.currentMarks[state.searchIndex];
  if (m) {
    m.tabIndex = -1;
    m.scrollIntoView({ behavior: preferredScrollBehavior(), block: 'center' });
    m.focus({ preventScroll: true });
    setTimeout(() => m.focus({ preventScroll: true }), 0);
  }
}

function jumpToMark(dir) {
  const isPaged = state.pagination && state.pagination.enabled && state.pagination.mode === 'paged' && globalSearchState.matches.length > 0;

  if (isPaged) {
    const total = globalSearchState.matches.length;
    const nextGlobal = (globalSearchState.globalIndex + dir + total) % total;
    globalSearchState.globalIndex = nextGlobal;
    const targetMatch = globalSearchState.matches[nextGlobal];

    if (targetMatch.pageIndex !== state.pagination.currentPage) {
      focusPagedSearchMatch(targetMatch);
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
  if (m) {
    m.tabIndex = -1;
    m.scrollIntoView({ behavior: preferredScrollBehavior(), block: 'center' });
    m.focus({ preventScroll: true });
    setTimeout(() => m.focus({ preventScroll: true }), 0);
  }
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
    $('search-count').textContent = total ? `${cur}/${total} (P.${pageNum})` : (state.lastQuery ? (_t('search.noMatches') || '无结果') : '');
    return;
  }

  const total = state.currentMarks.length;
  $('search-count').textContent = total ? ((state.searchIndex % total) + 1) + '/' + total : (state.lastQuery ? (_t('search.noMatches') || '无结果') : '');
}

function toggleSearch() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (state.mode === 'welcome' || (!state.file && !state.original)) {
    showToast(_t('toast.searchNeedsDocument') || '请先打开文档，再按 Ctrl+F 搜索');
    return;
  }
  const bar = $('search-bar');
  if (bar.classList.contains('hidden')) {
    bar.classList.remove('hidden');
    $('search-input').focus();
    $('search-input').select();
  } else {
    closeSearch({ restoreFocus: true });
  }
}

function closeSearch({ restoreFocus = false } = {}) {
  $('search-bar').classList.add('hidden');
  clearMarks();
  if (restoreFocus && $('btn-search')) $('btn-search').focus({ preventScroll: true });
}

function consumeInitialSearchJump() {
  focusCurrentSearchMatch();
}
