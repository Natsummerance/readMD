'use strict';
/* ============================================================
   ReadMD Core - Global Drag & Drop Management
   ============================================================ */

/* ---------------- 全局拖拽与标签交互支持 ---------------- */

let dragCounter = 0;

function bindGlobalDragAndDrop() {
  const overlay = $('drag-overlay');
  const title = $('drag-title');
  const desc = $('drag-desc');

  window.addEventListener('dragenter', e => {
    if (state.isDraggingTab) return;
    const types = (e.dataTransfer && e.dataTransfer.types) ? Array.from(e.dataTransfer.types) : [];
    if (types.includes('application/x-readmd-tab')) return;
    if (!types.includes('Files') && !types.includes('text/uri-list') && !types.includes('text/plain')) return;
    e.preventDefault();
    e.stopPropagation();
    dragCounter++;
    if (overlay) {
      overlay.classList.remove('hidden');
      if (title && desc) {
        if (types.includes('Files')) {
          title.textContent = '松开以导入文档';
          desc.textContent = 'Markdown 文件将在新标签页中打开；Word/PDF 等将自动导入转换';
        } else if (types.includes('text/uri-list')) {
          title.textContent = '松开以抓取网页';
          desc.textContent = '自动解析 URL 网页并提取为 Markdown 文档';
        } else {
          title.textContent = '松开以在此打开';
          desc.textContent = '拖入纯文本将自动生成为虚拟 Markdown 文档';
        }
      }
    }
  });

  window.addEventListener('dragover', e => {
    if (state.isDraggingTab) return;
    const types = (e.dataTransfer && e.dataTransfer.types) ? Array.from(e.dataTransfer.types) : [];
    if (types.includes('application/x-readmd-tab')) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
  });

  window.addEventListener('dragleave', e => {
    if (state.isDraggingTab) return;
    e.preventDefault();
    e.stopPropagation();
    dragCounter--;
    if (dragCounter <= 0) {
      dragCounter = 0;
      if (overlay) overlay.classList.add('hidden');
    }
  });

  window.addEventListener('drop', async e => {
    if (state.isDraggingTab) {
      state.isDraggingTab = false;
      return;
    }
    const types = (e.dataTransfer && e.dataTransfer.types) ? Array.from(e.dataTransfer.types) : [];
    if (types.includes('application/x-readmd-tab')) return;
    e.preventDefault();
    e.stopPropagation();
    dragCounter = 0;
    if (overlay) overlay.classList.add('hidden');


    const dt = e.dataTransfer;
    if (!dt) return;

    // 1. 处理文件拖拽
    if (dt.files && dt.files.length > 0) {
      const files = Array.from(dt.files);
      const mdFiles = files.filter(f => MD_RE.test(f.name || ''));
      const otherFiles = files.filter(f => !MD_RE.test(f.name || ''));

      if (mdFiles.length > 0) {
        for (const f of mdFiles) {
          const path = f.path ? f.path : await uploadFile(f);
          if (path) await loadFile(path);
        }
      }
      if (otherFiles.length > 0) {
        for (const f of otherFiles) {
          const path = f.path ? f.path : await uploadFile(f);
          if (path) await convertOrOcr(path, 'convert');
        }
      }
      return;
    }

    // 2. 处理 URL 或纯文本拖拽
    const uri = dt.getData('text/uri-list') || '';
    const text = dt.getData('text/plain') || '';
    const targetUrl = (uri || text).trim();
    if (/^https?:\/\//i.test(targetUrl)) {
      openWebDialog();
      const input = $('url-input');
      if (input) {
        input.value = targetUrl;
        $('url-go').click();
      }
    } else if (text.trim()) {
      const name = '新建文本-' + new Date().toISOString().slice(0, 10) + '.md';
      renderVirtual('clipboard', name, '', text, []);
      showToast('已从拖拽文本新建文档（Ctrl+S 可保存）');
    }
  });
}

async function openConvertModalWithFiles(files) {
  if (!files || !files.length) return;
  const paths = [];
  for (const f of files) {
    const path = f.path ? f.path : await uploadFile(f);
    if (path) paths.push(path);
  }
  if (paths.length > 0) {
    openConvertModal();
    await startBatchConvert(paths, $('convert-overwrite') ? $('convert-overwrite').checked : false);
  }
}


function bindTabOverflowEvents() {
  const overflowBtn = $('doc-tabs-overflow-btn');
  const dropdown = $('doc-tabs-dropdown');
  const overflowWrap = $('doc-tabs-overflow-wrap');
  if (!overflowBtn || !dropdown || !overflowWrap) return;

  let isPinned = false;

  overflowWrap.addEventListener('mouseenter', () => {
    if (!isPinned) dropdown.classList.remove('hidden');
  });
  overflowWrap.addEventListener('mouseleave', () => {
    if (!isPinned) dropdown.classList.add('hidden');
  });

  overflowBtn.addEventListener('click', e => {
    e.stopPropagation();
    isPinned = !isPinned;
    dropdown.classList.toggle('hidden', !isPinned);
  });

  document.addEventListener('click', e => {
    if (!overflowWrap.contains(e.target)) {
      isPinned = false;
      dropdown.classList.add('hidden');
    }
  });
}

function bindTabContextMenuEvents() {
  const menu = $('tab-context-menu');
  if (!menu) return;

  menu.querySelectorAll('button[data-action]').forEach(btn => {
    btn.addEventListener('click', () => {
      const tabId = menu.dataset.tabId;
      const action = btn.dataset.action;
      menu.classList.add('hidden');
      if (!tabId) return;
      const tab = state.tabs.find(t => t.id === tabId);
      if (!tab) return;

      if (action === 'close') {
        closeTab(tabId);
      } else if (action === 'close-others') {
        closeOtherTabs(tabId);
      } else if (action === 'close-all') {
        closeAllTabs();
      } else if (action === 'rename') {
        const bar = $('doc-tabs-bar');
        const tabEl = bar ? bar.querySelector(`[data-tab-id="${tabId}"]`) : null;
        if (tabEl) {
          const titleSpan = tabEl.querySelector('.tab-title');
          if (titleSpan) startTabInlineRename(tab, titleSpan, tabEl);
        }
      } else if (action === 'copy-path') {
        copyText(tab.path || tab.title || '', '已复制文件路径');
      }
    });
  });

  document.addEventListener('click', e => {
    if (!menu.contains(e.target)) {
      menu.classList.add('hidden');
    }
  });
}
