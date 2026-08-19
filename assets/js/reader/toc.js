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
  const headings = document.querySelectorAll('#content h1, #content h2, #content h3, #content h4, #content h5, #content h6');
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
      }
    });
    list.appendChild(a);
  });
}
