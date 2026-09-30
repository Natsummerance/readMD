'use strict';
/* ============================================================
   ReadMD Core - Global Drag & Drop Management
   ============================================================ */

/* ---------------- 全局拖拽与标签交互支持 ---------------- */

let dragCounter = 0;
let lastNativeDropAt = 0;

/** 拖放条目的分类：zip / folder / binary（走转换） / text（直接打开）。 */
function classifyDroppedEntry(entry) {
  const name = entry.name || '';
  if (entry.isDir) return 'folder';
  if (/\.zip$/i.test(name)) return 'zip';
  const binary = (typeof CONVERT_BINARY_RE !== 'undefined' && CONVERT_BINARY_RE.test(name))
    || (typeof IMG_RE !== 'undefined' && IMG_RE.test(name));
  return binary ? 'binary' : 'text';
}

async function extractDroppedZip(entry) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  let res;
  try {
    if (entry.path) {
      const resp = await apiFetch('/api/batch/extract-zip', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: entry.path, confirm: true })
      });
      res = await resp.json().catch(() => ({ ok: false, error_code: 'server_error' }));
    } else {
      const resp = await apiFetch('/api/batch/extract-zip', {
        method: 'POST',
        headers: { 'Content-Type': 'application/zip', 'X-ReadMD-Confirm': 'true' },
        body: entry.file
      });
      res = await resp.json().catch(() => ({ ok: false, error_code: 'server_error' }));
    }
  } catch (err) {
    res = { ok: false, error_code: 'server_error' };
  }
  if (!res || res.ok === false) {
    const code = (res && res.error_code) || 'server_error';
    const kind = /corrupt|invalid|bad/.test(code) ? 'zip_corrupt'
      : /large|limit|size/.test(code) ? 'zip_too_large'
      : /unsupported|encrypt/.test(code) ? 'zip_unsupported' : 'server_error';
    showToast(_t('batch.zipFailed', { name: entry.name, reason: _t('batch.zipReason.' + kind) }) || `无法解压 ${entry.name}`);
    return { paths: [], skipped: 0 };
  }
  return { paths: Array.isArray(res.paths) ? res.paths : [], skipped: Number(res.skipped) || 0 };
}

/**
 * 统一的拖放分派。条目形如 {name, path?, file?, isDir?}：
 * 有 path 时直接按原路径打开（可写回原文件），没有 path 时上传副本。
 */
async function handleDroppedEntries(entries) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const groups = { zip: [], folder: [], binary: [], text: [] };
  for (const en of entries || []) groups[classifyDroppedEntry(en)].push(en);

  if (groups.folder.length && typeof listFolder === 'function') {
    await listFolder(groups.folder[0].path);
  }

  if (groups.zip.length) {
    showToast(_t('batch.extractingZip') || '正在解压压缩包...');
    const extracted = [];
    let skipped = 0;
    for (const zf of groups.zip) {
      const r = await extractDroppedZip(zf);
      extracted.push(...r.paths);
      skipped += r.skipped;
    }
    if (skipped > 0) showToast(_t('batch.zipSkipped', { count: skipped }) || `已跳过 ${skipped} 个不支持的文件`);
    if (extracted.length && typeof enqueueBatchFiles === 'function') enqueueBatchFiles(extracted, false);
  }

  for (const f of groups.text) {
    const path = f.path ? f.path : await uploadFile(f.file);
    if (path) await loadFile(path, { browserCopy: !f.path });
  }

  if (groups.binary.length) {
    const paths = [];
    for (const f of groups.binary) {
      const path = f.path ? f.path : await uploadFile(f.file);
      if (path) paths.push(path);
    }
    if (paths.length === 1 && typeof convertOrOcr === 'function') {
      convertOrOcr(paths[0], 'convert');
    } else if (paths.length && typeof enqueueBatchFiles === 'function') {
      enqueueBatchFiles(paths, !!($('convert-overwrite') && $('convert-overwrite').checked));
    }
  }
}

/** 桌面宿主（WRY）原生拖放入口：payload 为 {paths:[{path,name,isDir}]} 或 {state:'enter'|'leave'}。 */
window.__readmdNativeDrop = function (payload) {
  const overlay = $('drag-overlay');
  if (!payload) return;
  if (payload.state === 'enter') {
    if (overlay) {
      const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
      overlay.classList.remove('hidden');
      const title = $('drag-title');
      const desc = $('drag-desc');
      if (title) title.textContent = _t('dialog.dropTitle') || '松开以导入文档';
      if (desc) desc.textContent = _t('dialog.dropDesc') || 'Markdown 文件将在新标签页中打开；Word/PDF 等将自动导入转换';
    }
    return;
  }
  if (payload.state === 'leave') {
    dragCounter = 0;
    if (overlay) overlay.classList.add('hidden');
    return;
  }
  if (Array.isArray(payload.paths) && payload.paths.length) {
    lastNativeDropAt = Date.now();
    dragCounter = 0;
    if (overlay) overlay.classList.add('hidden');
    handleDroppedEntries(payload.paths.map(p => ({ name: p.name || '', path: p.path || '', isDir: !!p.isDir })));
  }
};

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
      const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
      overlay.classList.remove('hidden');
      if (title && desc) {
        if (types.includes('Files')) {
          title.textContent = _t('dialog.dropTitle') || '松开以导入文档';
          desc.textContent = _t('dialog.dropDesc') || 'Markdown 文件将在新标签页中打开；Word/PDF 等将自动导入转换';
        } else if (types.includes('text/uri-list')) {
          title.textContent = _t('dialog.dropUrlTitle') || '松开以抓取网页';
          desc.textContent = _t('dialog.dropUrlDesc') || '自动解析 URL 网页并提取为 Markdown 文档';
        } else {
          title.textContent = _t('dialog.dropTextTitle') || '松开以在此打开';
          desc.textContent = _t('dialog.dropTextDesc') || '拖入纯文本将自动生成为虚拟 Markdown 文档';
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

    // 1. 处理文件拖拽（万物皆可开：代码/配置/脚本/文本直接开，Office/PDF 走转换，ZIP 自动解压）
    if (dt.files && dt.files.length > 0) {
      // 桌面端的原生拖放会直接送来真实路径（__readmdNativeDrop），DOM 这里只是兜底。
      if (Date.now() - lastNativeDropAt < 1500) return;
      await handleDroppedEntries(Array.from(dt.files).map(f => ({ name: f.name || '', path: f.path || '', file: f })));
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
      const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
      const name = (_t('tabs.untitled') || '新建文本') + '-' + new Date().toISOString().slice(0, 10) + '.md';
      renderVirtual('clipboard', name, '', text, []);
      showToast(_t('toast.droppedCreated') || '已从拖拽文本新建文档（Ctrl+S 可保存）');
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
  if (paths.length > 0) enqueueBatchFiles(paths, false);
}


function bindTabOverflowEvents() {
  const overflowBtn = $('doc-tabs-overflow-btn');
  const dropdown = $('doc-tabs-dropdown');
  const overflowWrap = $('doc-tabs-overflow-wrap');
  if (!overflowBtn || !dropdown || !overflowWrap) return;

  let isPinned = false;

  const setOverflowMenu = visible => {
    isPinned = visible;
    dropdown.classList.toggle('hidden', !visible);
    overflowBtn.setAttribute('aria-expanded', visible ? 'true' : 'false');
    if (visible) setTimeout(() => dropdown.querySelector('[role="menuitem"]')?.focus(), 20);
  };

  overflowWrap.addEventListener('mouseenter', () => {
    if (!isPinned) setOverflowMenu(true);
  });
  overflowWrap.addEventListener('mouseleave', () => {
    if (!isPinned) setOverflowMenu(false);
  });

  overflowBtn.addEventListener('click', e => {
    e.stopPropagation();
    setOverflowMenu(!isPinned);
  });

  overflowBtn.addEventListener('keydown', event => {
    if (event.key !== 'ArrowDown' && event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    setOverflowMenu(true);
  });

  document.addEventListener('click', e => {
    if (!overflowWrap.contains(e.target)) {
      setOverflowMenu(false);
    }
  });

  dropdown.addEventListener('keydown', event => {
    const items = Array.from(dropdown.querySelectorAll('[role="menuitem"]'));
    const index = items.indexOf(document.activeElement);
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      items[(index + (event.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length]?.focus();
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      (event.key === 'Home' ? items[0] : items[items.length - 1])?.focus();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      setOverflowMenu(false);
      overflowBtn.focus();
    }
  });
}

function bindTabContextMenuEvents() {
  const menu = $('tab-context-menu');
  if (!menu) return;

  menu.addEventListener('keydown', event => {
    const items = Array.from(menu.querySelectorAll('[role="menuitem"]:not([disabled])'));
    const index = items.indexOf(document.activeElement);
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const next = event.key === 'ArrowDown'
        ? items[(index + 1) % items.length]
        : items[(index - 1 + items.length) % items.length];
      next?.focus();
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      (event.key === 'Home' ? items[0] : items[items.length - 1])?.focus();
    }
  });

  menu.querySelectorAll('button[data-action]').forEach(btn => {
    btn.addEventListener('click', () => {
      const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
      const tabId = menu.dataset.tabId;
      const action = btn.dataset.action;
      closeTabContextMenu();
      if (!tabId) return;
      const tab = state.tabs.find(t => t.id === tabId);
      if (!tab) return;

      if (action === 'close') {
        closeTab(tabId);
      } else if (action === 'close-others') {
        closeOtherTabs(tabId);
      } else if (action === 'close-all') {
        closeAllTabs();
      } else if (action === 'move-left' || action === 'move-right') {
        const index = state.tabs.findIndex(item => item.id === tabId);
        const targetIndex = action === 'move-left' ? index - 1 : index + 1;
        if (targetIndex >= 0 && targetIndex < state.tabs.length) {
          reorderTabs(tabId, state.tabs[targetIndex].id, action === 'move-right');
          focusVisibleTab(tabId);
        }
      } else if (action === 'rename') {
        const bar = $('doc-tabs-bar');
        const tabEl = bar ? bar.querySelector(`[data-tab-id="${tabId}"]`) : null;
        if (tabEl) {
          const titleSpan = tabEl.querySelector('.tab-title');
          if (titleSpan) startTabInlineRename(tab, titleSpan, tabEl);
        }
      } else if (action === 'copy-path') {
        copyText(tab.path || tab.title || '', _t('toast.copiedPath') || '已复制文件路径');
      }
    });
  });


  document.addEventListener('click', e => {
    if (!menu.contains(e.target)) {
      closeTabContextMenu();
    }
  });
}
