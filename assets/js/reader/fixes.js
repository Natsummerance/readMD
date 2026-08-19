'use strict';
/* ============================================================
   ReadMD Reader - Document Fixes Modal
   ============================================================ */

/* ---------------- 修正详情 ---------------- */

function showFixModal() {
  const list = $('fix-list');
  list.innerHTML = '';
  const fixes = state.fixes || [];
  $('fix-count').textContent = fixes.length ? '（共 ' + fixes.length + ' 处）' : '';
  if (!fixes.length) {
    const li = document.createElement('li');
    li.className = 'empty';
    li.textContent = '本篇文档未发现需要修正的内容';
    list.appendChild(li);
  } else {
    fixes.forEach(f => {
      const li = document.createElement('li');
      li.textContent = f;
      list.appendChild(li);
    });
  }
  $('fix-modal').classList.remove('hidden');
}
