'use strict';
/* ============================================================
   ReadMD Reader - Table of Contents & Heading Navigation
   ============================================================ */

/* ---------------- 目录 ---------------- */

function buildToc() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const list = $('toc-list');
  if (!list) return;
  list.innerHTML = '';

  // 1. 分页模式下：从全文所有分页提取全局完整大纲
  if (state.pagination && state.pagination.enabled && state.pagination.mode === 'paged' && state.pagination.pages && state.pagination.pages.length) {
    const seen = {};
    const globalHeadings = [];

    state.pagination.pages.forEach((pg, pageIdx) => {
      const lines = pg.content.split('\n');
      let inFence = false;
      lines.forEach(line => {
        const trimmed = line.trim();
        if (/^```/.test(trimmed)) { inFence = !inFence; return; }
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
            level: Math.min(level, 3),
            pageIndex: pageIdx,
          });
        }
      });
    });

    state.pagination.allHeadings = globalHeadings;

    if (!globalHeadings.length) {
      list.innerHTML = `<div class="side-empty">${_t('sidebar.emptyToc') || '（当前文档暂无标题大纲）'}</div>`;
      return;
    }

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

      a.addEventListener('click', e => {
        e.preventDefault();
        if (h.pageIndex === state.pagination.currentPage) {
          const el = document.getElementById(h.id);
          if (el) {
            el.scrollIntoView({ behavior: 'smooth', block: 'start' });
            el.classList.remove('heading-target-highlight');
            void el.offsetWidth;
            el.classList.add('heading-target-highlight');
            setTimeout(() => el.classList.remove('heading-target-highlight'), 1500);
          }
        } else {
          renderPage(h.pageIndex, h.id);
        }
      });

      container.appendChild(a);
    };

    const fillTocGroup = (container, headings) => {
      if (container.childElementCount) return;
      headings.forEach(h => appendTocHeading(container, h));
    };

    headingGroups.forEach((headings, pageIndex) => {
      const group = document.createElement('details');
      group.className = 'toc-page-group';
      group.dataset.pageIdx = String(pageIndex);
      const summary = document.createElement('summary');
      summary.textContent = `P.${pageIndex + 1} · ${headings.length}`;
      const container = document.createElement('div');
      container.className = 'toc-group-body';
      group.addEventListener('toggle', () => {
        if (group.open) fillTocGroup(container, headings);
      });
      if (pageIndex === state.pagination.currentPage) {
        group.open = true;
        fillTocGroup(container, headings);
      }
      group.append(summary, container);
      list.appendChild(group);
    });
    return;
  }

  // 2. 连续/常规模式下：从当前 DOM 提取大纲
  const headings = document.querySelectorAll('#content h1, #content h2, #content h3, #content h4, #content h5, #content h6');
  if (!headings.length) {
    list.innerHTML = `<div class="side-empty">${_t('sidebar.emptyToc') || '（当前文档暂无标题大纲）'}</div>`;
    return;
  }

  headings.forEach((h, i) => {
    if (!h.id) h.id = 'toc-h-' + i;
    const a = document.createElement('a');
    a.href = '#' + h.id;
    a.textContent = h.textContent.trim() || ((_t('toc.sectionDefault') || '章节') + ' ' + (i + 1));
    const lv = Math.min(+h.tagName[1], 3);

    a.className = 'lv' + lv;
    a.addEventListener('click', e => {
      e.preventDefault();
      const el = document.getElementById(h.id);
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'start' });
        el.classList.remove('heading-target-highlight');
        void el.offsetWidth;
        el.classList.add('heading-target-highlight');
        setTimeout(() => el.classList.remove('heading-target-highlight'), 1500);
      }
    });
    list.appendChild(a);
  });
}
