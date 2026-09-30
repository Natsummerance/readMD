'use strict';
/* ============================================================
   ReadMD Reader - Table of Contents & Heading Navigation
   ============================================================ */

/* ---------------- 目录 ---------------- */

let tocCache = { source: null, pageCount: 0 };

function refreshCurrentTocPage(list) {
  const currentPage = state.pagination.currentPage;
  const currentGroup = list.querySelector(`details.toc-page-group[data-page-idx="${CSS.escape(String(currentPage))}"]`);
  list.querySelectorAll('.toc-cur-page').forEach(link => link.classList.remove('toc-cur-page'));
  list.querySelectorAll('.toc-page-group').forEach(group => {
    // Preserve groups the user explicitly opened (including programmatic
    // embedders), but collapse untouched groups so their lazy bodies remain
    // out of the active layout and DOM budget.
    group.open = group === currentGroup || group.__tocExplicitOpen === true;
  });
  currentGroup?.querySelector('a')?.classList.add('toc-cur-page');
}

function buildToc() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const list = $('toc-list');
  if (!list) return;

  if (state.mode === 'welcome' || (!state.file && !state.original && !state.fixed && (!state.tabs || !state.tabs.length))) {
    list.innerHTML = `
      <div class="side-empty">
        <p>${_t('sidebar.emptyToc') || '（当前文档暂无标题大纲）'}</p>
        <button type="button" class="side-empty-close" id="toc-empty-close">${_t('toolbar.close') || '收起侧栏'}</button>
      </div>`;
    const emptyCloseBtn = list.querySelector('#toc-empty-close');
    if (emptyCloseBtn) {
      emptyCloseBtn.addEventListener('click', e => {
        e.preventDefault();
        $('side')?.classList.add('hidden');
      });
    }
    tocCache = { source: null, pageCount: 0 };
    return;
  }

  // 1. 分页模式下：从全文所有分页提取全局完整大纲
  if (state.pagination && state.pagination.enabled && state.pagination.mode === 'paged' && state.pagination.pages && state.pagination.pages.length) {
    const canReuseOutline =
      tocCache.source === state.pagination.rawContent &&
      tocCache.pageCount === state.pagination.pages.length &&
      list.childElementCount;
    if (canReuseOutline) {
      refreshCurrentTocPage(list);
      if (window.ReadMDReader && !list.querySelector(':scope > .rd-toc-head')) window.ReadMDReader.decorateToc(list, null);
      if (typeof updateActiveTocHeading === 'function') updateActiveTocHeading();
      return;
    }

    list.innerHTML = '';
    const seen = {};
    const globalHeadings = [];

    state.pagination.pages.forEach((pg, pageIdx) => {
    const lines = pg.content.split('\n');
    let inFence = false;
    let fenceMarker = '';
    lines.forEach((line, lineIndex) => {
      const trimmed = line.trim();
      if (/^(```|~~~)/.test(trimmed)) {
        const marker = trimmed.slice(0, 3);
        if (!inFence) {
          inFence = true;
          fenceMarker = marker;
        } else if (trimmed.startsWith(fenceMarker)) {
          inFence = false;
          fenceMarker = '';
        }
        return;
      }
      if (inFence) return;

        const m = trimmed.match(/^(#{1,6})\s+(.+)$/);
        if (m) {
          const level = m[1].length;
          const rawText = m[2].replace(/[*_`#]/g, '').trim();
          let slug = rawText.toLowerCase()
            .replace(/[^\w\u4e00-\u9fff\s-]/g, '')
            .replace(/\s+/g, '-');
          if (!slug) slug = 'toc-h-' + globalHeadings.length;
          if (seen[slug]) {
            seen[slug]++;
            slug = slug + '-' + seen[slug];
          } else {
            seen[slug] = 1;
          }
          globalHeadings.push({
            id: slug,
            text: rawText,
            level,
            pageIndex: pageIdx,
            sourceLine: lineIndex + 1,
          });
        }
      });
    });

    state.pagination.allHeadings = globalHeadings;

    if (!globalHeadings.length) {
      list.innerHTML = `<div class="side-empty">${_t('sidebar.emptyToc') || '（当前文档暂无标题大纲）'}</div>`;
      return;
    }
    if (window.ReadMDReader) window.ReadMDReader.decorateToc(list, null);

    const headingGroups = new Map();
    globalHeadings.forEach(h => {
      if (!headingGroups.has(h.pageIndex)) headingGroups.set(h.pageIndex, []);
      headingGroups.get(h.pageIndex).push(h);
    });

    const appendTocHeading = (container, h) => {
      const a = document.createElement('a');
      a.href = '#' + h.id;
      a.textContent = h.text || ((_t('toc.sectionDefault') || '章节'));
      a.className = 'lv' + h.level;
      if (h.pageIndex === state.pagination.currentPage) a.classList.add('toc-cur-page');
      a.setAttribute('data-page-idx', h.pageIndex);
      a.setAttribute('data-heading-id', h.id);

      container.appendChild(a);
    };

    const fillTocGroup = (container, headings) => {
      if (container.childElementCount) return;
      const fragment = document.createDocumentFragment();
      headings.forEach(h => appendTocHeading(fragment, h));
      container.appendChild(fragment);
    };

    // A few embedders open <details> programmatically instead of dispatching
    // the native toggle event.  Observe the open attribute as a safety net,
    // while retaining lazy heading links so a 50k-line document does not
    // inflate the live DOM just by opening the TOC.
    if (!list.__tocOpenObserver) {
      list.__tocOpenObserver = new MutationObserver(records => {
        records.forEach(record => {
          if (record.type !== 'attributes' || record.attributeName !== 'open') return;
          const group = record.target;
          const pageIndex = Number(group.dataset.pageIdx);
          const headings = headingGroups.get(pageIndex);
          const container = group.querySelector('.toc-group-body');
          group.__tocExplicitOpen = group.open;
          if (group.open && headings && container) fillTocGroup(container, headings);
        });
      });
      list.__tocOpenObserver.observe(list, { subtree: true, attributes: true, attributeFilter: ['open'] });
    }

    if (!list.dataset.delegationBound) {
      list.dataset.delegationBound = 'true';
      list.addEventListener('click', event => {
        const link = event.target.closest('[data-heading-id]');
        if (!link) return;
        event.preventDefault();
        const pageIndex = Number(link.dataset.pageIdx);
        const headingId = link.dataset.headingId;
        if (pageIndex === state.pagination.currentPage) {
          const el = document.getElementById(headingId);
          if (el) {
            el.tabIndex = -1;
            el.scrollIntoView({ behavior: preferredScrollBehavior(), block: 'start' });
            el.focus({ preventScroll: true });
            el.classList.remove('heading-target-highlight');
            void el.offsetWidth;
            el.classList.add('heading-target-highlight');
            setTimeout(() => el.classList.remove('heading-target-highlight'), 1500);
          }
        } else {
          renderPage(pageIndex, headingId);
        }
      });
    }

    headingGroups.forEach((headings, pageIndex) => {
      const group = document.createElement('details');
      group.className = 'toc-page-group';
      group.dataset.pageIdx = String(pageIndex);
      const summary = document.createElement('summary');
      summary.textContent = `P.${pageIndex + 1} · ${headings.length}`;
      const container = document.createElement('div');
      container.className = 'toc-group-body';
      group.addEventListener('toggle', () => {
        group.__tocExplicitOpen = group.open;
        if (group.open) fillTocGroup(container, headings);
      });
      if (pageIndex === state.pagination.currentPage) {
        group.open = true;
        fillTocGroup(container, headings);
      }
      group.append(summary, container);
      list.appendChild(group);
    });
    tocCache = { source: state.pagination.rawContent, pageCount: state.pagination.pages.length };
    if (typeof updateActiveTocHeading === 'function') updateActiveTocHeading();
    return;
  }

  // 2. 连续/常规模式下：从当前 DOM 提取大纲
  tocCache = { source: null, pageCount: 0 };
  list.innerHTML = '';
  const headings = document.querySelectorAll('#content h1, #content h2, #content h3, #content h4, #content h5, #content h6');
  if (!headings.length) {
    list.innerHTML = `<div class="side-empty">${_t('sidebar.emptyToc') || '（当前文档暂无标题大纲）'}</div>`;
    return;
  }

  const fragment = document.createDocumentFragment();
  const items = [];
  headings.forEach((h, i) => {
    if (!h.id) h.id = 'toc-h-' + i;
    const lv = +h.tagName[1];
    const item = document.createElement('div');
    item.className = 'rd-toc-item';
    item.dataset.level = String(lv);
    const a = document.createElement('a');
    a.href = '#' + h.id;
    a.textContent = tocHeadingText(h) || ((_t('toc.sectionDefault') || '章节') + ' ' + (i + 1));
    a.className = 'lv' + lv;
    a.dataset.headingId = h.id;
    a.addEventListener('click', e => {
      e.preventDefault();
      const el = document.getElementById(h.id);
      if (el) {
        el.tabIndex = -1;
        el.scrollIntoView({ behavior: preferredScrollBehavior(), block: 'start' });
        el.focus({ preventScroll: true });
        el.classList.remove('heading-target-highlight');
        void el.offsetWidth;
        el.classList.add('heading-target-highlight');
        setTimeout(() => el.classList.remove('heading-target-highlight'), 1500);
        setActiveTocLink(list, h.id);
      }
    });
    item.appendChild(a);
    items.push(item);
    fragment.appendChild(item);
  });
  list.appendChild(fragment);
  if (window.ReadMDReader) window.ReadMDReader.decorateToc(list, items);
  if (typeof window.invalidateTocSpy === 'function') window.invalidateTocSpy();
  updateActiveTocHeading();
}

function tocHeadingText(h) {
  const clone = h.cloneNode(true);
  clone.querySelectorAll('[data-rd-chrome]').forEach(node => node.remove());
  return clone.textContent.trim();
}

/* ---------------- 滚动同步（scroll-spy） ---------------- */

let tocSpy = { headings: null, tops: null, scrollHeight: 0, width: 0 };
window.invalidateTocSpy = function invalidateTocSpy() {
  tocSpy = { headings: null, tops: null, scrollHeight: 0, width: 0 };
};

function tocSpyPositions(content) {
  if (!content.__rdFocusBound) {
    content.__rdFocusBound = true;
    content.addEventListener('focusin', () => { content.__rdFocusAt = Date.now(); });
  }
  const body = content.querySelector(':scope > .markdown-body');
  if (!body) return null;
  if (tocSpy.headings && tocSpy.scrollHeight === content.scrollHeight && tocSpy.width === content.clientWidth &&
      tocSpy.headings.length && tocSpy.headings[0].isConnected) return tocSpy;
  const headings = Array.from(body.querySelectorAll('h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]'));
  const base = content.getBoundingClientRect().top - content.scrollTop;
  tocSpy = {
    headings,
    tops: headings.map(h => h.getBoundingClientRect().top - base),
    scrollHeight: content.scrollHeight,
    width: content.clientWidth,
  };
  return tocSpy;
}

function setActiveTocLink(list, id) {
  const current = list.querySelector('.toc-heading-active');
  const link = id ? list.querySelector(`[data-heading-id="${CSS.escape(id)}"]`) : null;
  if (current === link) return;
  list.querySelectorAll('.toc-heading-active').forEach(el => el.classList.remove('toc-heading-active'));
  list.querySelectorAll('.rd-toc-item.is-trail').forEach(el => el.classList.remove('is-trail'));
  if (!link) return;
  link.classList.add('toc-heading-active');
  const item = link.closest('.rd-toc-item');
  if (item) {
    let level = +item.dataset.level;
    for (let prev = item.previousElementSibling; prev && level > 1; prev = prev.previousElementSibling) {
      if (!prev.classList.contains('rd-toc-item')) continue;
      const prevLevel = +prev.dataset.level;
      if (prevLevel < level) { prev.classList.add('is-trail'); level = prevLevel; }
    }
  }
  // Keep the active entry in view inside the outline without moving the page.
  const visible = link.offsetParent ? link : list.querySelector('.rd-toc-item.is-trail:not(.is-hidden) > a');
  if (!visible || list.classList.contains('hidden')) return;
  const lr = list.getBoundingClientRect();
  const r = visible.getBoundingClientRect();
  if (r.top < lr.top + 48 || r.bottom > lr.bottom - 16) {
    list.scrollTop += r.top - lr.top - lr.height / 3;
  }
}

function updateActiveTocHeading() {
  const content = $('content');
  const list = $('toc-list');
  if (!content || !list || state.editing) return;
  const spy = tocSpyPositions(content);
  if (!spy || !spy.headings.length) return;
  const top = content.scrollTop;
  const threshold = top + Math.min(96, content.clientHeight * 0.25);
  const focused = content.contains(document.activeElement) && document.activeElement?.id ? document.activeElement : null;
  let active = null;
  if (focused) {
    // A heading reached through the outline or a link wins while its jump is
    // in flight and while it stays on screen; manual scrolling takes over after.
    const index = spy.headings.indexOf(focused);
    const recent = Date.now() - (content.__rdFocusAt || 0) < 1200;
    if (index >= 0 && (recent || (spy.tops[index] >= top - 8 && spy.tops[index] < top + content.clientHeight))) active = focused;
  }
  if (!active) {
    // Binary search: last heading whose top is above the reading line.
    let lo = 0;
    let hi = spy.tops.length - 1;
    let found = 0;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (spy.tops[mid] <= threshold) { found = mid; lo = mid + 1; } else hi = mid - 1;
    }
    if (content.scrollTop + content.clientHeight >= content.scrollHeight - 2) {
      // At the very end the last short sections can never reach the reading line.
      for (let i = spy.tops.length - 1; i > found; i -= 1) {
        if (spy.tops[i] < top + content.clientHeight * 0.6) { found = i; break; }
      }
    }
    active = spy.headings[found];
  }
  setActiveTocLink(list, active && active.id);
}
