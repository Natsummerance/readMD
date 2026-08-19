'use strict';
/* ============================================================
   ReadMD Reader - In-Document Search & Highlighting
   ============================================================ */

/* ---------------- 搜索 ---------------- */

function clearMarks() {
  state.currentMarks.forEach(m => {
    const p = m.parentNode;
    if (!p) return;
    p.replaceChild(document.createTextNode(m.textContent), m);
    p.normalize();
  });
  state.currentMarks = [];
  state.searchIndex = 0;
  updateSearchCount();
}

function doSearch(q) {
  clearMarks();
  state.lastQuery = q;
  if (!q) { updateSearchCount(); return; }
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
  jumpToMark(0);
}

function jumpToMark(dir) {
  if (!state.currentMarks.length) return;
  state.searchIndex = (state.searchIndex + dir + state.currentMarks.length) % state.currentMarks.length;
  state.currentMarks.forEach((m, i) => m.classList.toggle('cur', i === state.searchIndex));
  const m = state.currentMarks[state.searchIndex];
  m.scrollIntoView({ behavior: 'smooth', block: 'center' });
  updateSearchCount();
}

function updateSearchCount() {
  const total = state.currentMarks.length;
  $('search-count').textContent = total ? ((state.searchIndex % total) + 1) + '/' + total : (state.lastQuery ? '无结果' : '');
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
