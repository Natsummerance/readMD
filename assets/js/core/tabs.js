'use strict';
/* ============================================================
   ReadMD Core - Multi-Tab Management & Safety Dialog
   ============================================================ */

/* ---------------- 多标签页系统 (Multi-Tab Management) ---------------- */

function getActiveTab() {
  return state.tabs.find(t => t.id === state.activeTabId) || null;
}

function findTabByPath(filePath) {
  if (!filePath) return null;
  const targetKey = normalizePath(filePath);
  return state.tabs.find(t => t.path && normalizePath(t.path) === targetKey) || null;
}

function syncStateFromActiveTab() {
  const tab = getActiveTab();
  if (!tab) return;
  state.mode = tab.mode || 'file';
  state.source = tab.source || 'file';
  state.sourceName = tab.title || tab.name || '';
  state.file = tab.path;
  state.dir = tab.dir || '';
  state.mtime = tab.mtime || 0;
  state.size = tab.size || 0;
  state.encoding = tab.encoding || 'utf-8';
  state.fixed = tab.content || '';
  state.original = tab.original || tab.content || '';
  state.fixes = tab.fixes || [];
  state.stats = tab.stats || {};
  state.webAssets = tab.webAssets || [];
}

function renderTabsBar() {
  const bar = $('doc-tabs-bar');
  const secBar = $('doc-tabs-secondary-bar');
  const dropdown = $('doc-tabs-dropdown');
  const overflowWrap = $('doc-tabs-overflow-wrap');
  const btnHome = $('btn-home');

  if (state.tabs.length === 0) {
    if (bar) bar.innerHTML = '';
    if (secBar) { secBar.innerHTML = ''; secBar.classList.add('hidden'); }
    if (overflowWrap) overflowWrap.classList.add('hidden');
    if (btnHome) btnHome.classList.add('hidden');
    return;
  }

  if (btnHome) btnHome.classList.remove('hidden');

  const createTabEl = (tab) => {
    const el = document.createElement('div');
    el.className = 'tab-item' + (tab.id === state.activeTabId ? ' active' : '');
    el.dataset.tabId = tab.id;
    el.draggable = true;
    el.title = tab.path || tab.title || tab.name;

    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    if (tab.isDirty) {
      const dot = document.createElement('span');
      dot.className = 'tab-dirty';
      dot.title = _t('tabs.dirty') || '未保存';
      el.appendChild(dot);
    }

    const titleSpan = document.createElement('span');
    titleSpan.className = 'tab-title';
    titleSpan.textContent = tab.title || tab.name || (_t('tabs.untitled') || '未命名');
    el.appendChild(titleSpan);

    const closeBtn = document.createElement('button');
    closeBtn.className = 'tab-close';
    closeBtn.innerHTML = '&times;';
    closeBtn.title = _t('tabs.closeTab') || '关闭标签';
    closeBtn.addEventListener('click', e => {
      e.stopPropagation();
      closeTab(tab.id);
    });
    el.appendChild(closeBtn);

    el.addEventListener('click', () => switchTab(tab.id));

    el.addEventListener('dblclick', e => {
      e.stopPropagation();

      startTabInlineRename(tab, titleSpan, el);
    });

    el.addEventListener('contextmenu', e => {
      e.preventDefault();
      openTabContextMenu(e, tab.id);
    });

    el.draggable = true;
    el.addEventListener('dragstart', e => {
      state.isDraggingTab = true;
      e.dataTransfer.setData('application/x-readmd-tab', tab.id);
      e.dataTransfer.setData('text/plain', tab.id);
      e.dataTransfer.effectAllowed = 'move';
      el.classList.add('tab-dragging');
    });
    el.addEventListener('dragover', e => {
      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = 'move';
      const rect = el.getBoundingClientRect();
      const mid = rect.left + rect.width / 2;
      const isRight = e.clientX > mid;
      el.classList.toggle('tab-drag-over-left', !isRight);
      el.classList.toggle('tab-drag-over-right', isRight);
    });
    el.addEventListener('dragleave', () => {
      el.classList.remove('tab-drag-over-left', 'tab-drag-over-right');
    });
    el.addEventListener('drop', e => {
      e.preventDefault();
      e.stopPropagation();
      el.classList.remove('tab-drag-over-left', 'tab-drag-over-right');
      const srcId = e.dataTransfer.getData('application/x-readmd-tab') || e.dataTransfer.getData('text/plain');
      if (srcId && srcId !== tab.id) {
        const rect = el.getBoundingClientRect();
        const isRight = e.clientX > (rect.left + rect.width / 2);
        reorderTabs(srcId, tab.id, isRight);
      }
    });
    el.addEventListener('dragend', () => {
      state.isDraggingTab = false;
      el.classList.remove('tab-dragging', 'tab-drag-over-left', 'tab-drag-over-right');
      document.querySelectorAll('.tab-item').forEach(t => t.classList.remove('tab-drag-over-left', 'tab-drag-over-right'));
    });

    return el;
  };


  if (bar) {
    bar.innerHTML = '';
    state.tabs.forEach(tab => bar.appendChild(createTabEl(tab)));
  }

  if (dropdown) {
    dropdown.innerHTML = '';
    state.tabs.forEach(tab => {
      const item = document.createElement('button');
      item.className = 'doc-tabs-dropdown-item' + (tab.id === state.activeTabId ? ' active' : '');
      item.innerHTML = '<span>' + (tab.title || tab.name) + (tab.isDirty ? ' &bull;' : '') + '</span><small>' + (tab.path || (tab.isVirtual ? (_t('tabs.virtual') || '虚拟') : '')) + '</small>';
      item.addEventListener('click', () => {
        switchTab(tab.id);
        dropdown.classList.add('hidden');
      });
      dropdown.appendChild(item);
    });

  }

  if (bar && overflowWrap) {
    const isOverflow = bar.scrollWidth > bar.clientWidth + 6;
    overflowWrap.classList.toggle('hidden', !isOverflow);
  }

  if (secBar) {
    if (window.innerWidth < 650 && state.tabs.length > 0) {
      secBar.innerHTML = '';
      state.tabs.forEach(tab => secBar.appendChild(createTabEl(tab)));
      secBar.classList.remove('hidden');
      if (bar && bar.parentElement) bar.parentElement.classList.add('hidden');
    } else {
      secBar.classList.add('hidden');
      if (bar && bar.parentElement) bar.parentElement.classList.remove('hidden');
    }
  }
}

function startTabInlineRename(tab, titleSpan, tabEl) {
  if (tabEl.querySelector('.tab-title-input') || tabEl.querySelector('.tab-rename-wrap')) return;
  const currentTitle = tab.title || tab.name || '';
  const dot = currentTitle.lastIndexOf('.');
  const stem = dot > 0 ? currentTitle.slice(0, dot) : currentTitle;
  const ext = dot > 0 ? currentTitle.slice(dot) : '';

  const tabsTrack = tabEl.parentElement;
  if (tabsTrack) tabsTrack.classList.add('tab-renaming-mode');
  tabEl.classList.add('tab-renaming-active');

  titleSpan.classList.add('hidden');
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const wrap = document.createElement('div');
  wrap.className = 'tab-rename-wrap';
  wrap.innerHTML = '<input type="text" class="tab-title-input" spellcheck="false" autocomplete="off" aria-label="' + (_t('tabs.rename') || '重命名文件') + '"><span class="tab-rename-ext">' + ext + '</span>';
  const input = wrap.querySelector('.tab-title-input');
  input.value = stem;
  tabEl.insertBefore(wrap, titleSpan);
  input.focus();
  input.select();

  let committed = false;
  const cleanup = () => {
    wrap.remove();
    titleSpan.classList.remove('hidden');
    tabEl.classList.remove('tab-renaming-active');
    if (tabsTrack) tabsTrack.classList.remove('tab-renaming-mode');
  };

  const commit = async () => {
    if (committed) return;
    committed = true;
    const newStem = input.value.trim();
    cleanup();
    if (newStem && newStem !== stem) {
      const newFullName = newStem + ext;
      await renameTab(tab.id, newFullName);
    }
  };

  input.addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); commit(); }
    else if (e.key === 'Escape') { e.preventDefault(); committed = true; cleanup(); }
  });
  input.addEventListener('blur', () => { setTimeout(() => { if (!committed) commit(); }, 120); });
}


async function renameTab(tabId, newTitle) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const tab = state.tabs.find(t => t.id === tabId);
  if (!tab || !newTitle) return;
  if (tab.mode === 'file' && tab.path && hasPy && py.rename_file) {
    busy(true);
    try {
      const r = await py.rename_file(tab.path, newTitle);
      if (r && r.ok) {
        tab.path = r.path;
        tab.name = r.name;
        tab.title = r.name;
        if (state.activeTabId === tab.id) {
          state.file = r.path;
          document.title = r.name + ' - ReadMD';
          setFileTitle(r.name, true, r.path);
        }
        showToast(_t('toast.renamedTo', { name: r.name }) || ('已重命名为 ' + r.name));
      } else {
        showToast((_t('toast.renameFailed') || '重命名失败：') + ((r && r.error) || ''));
      }
    } catch (e) {
      showToast((_t('toast.renameFailed') || '重命名失败：') + e.message);
    } finally {
      busy(false);
    }
  } else {
    tab.title = newTitle;
    tab.name = newTitle;

    if (state.activeTabId === tab.id) {
      state.sourceName = newTitle;
      document.title = newTitle + ' - ReadMD';
    }
  }
  renderTabsBar();
}

function reorderTabs(srcId, destId, insertAfter = false) {
  const srcIdx = state.tabs.findIndex(t => t.id === srcId);
  if (srcIdx < 0) return;
  const [removed] = state.tabs.splice(srcIdx, 1);
  let destIdx = state.tabs.findIndex(t => t.id === destId);
  if (destIdx < 0) {
    state.tabs.push(removed);
  } else {
    if (insertAfter) destIdx++;
    state.tabs.splice(destIdx, 0, removed);
  }
  renderTabsBar();
}


function openTabContextMenu(e, tabId) {
  const menu = $('tab-context-menu');
  if (!menu) return;
  menu.dataset.tabId = tabId;
  menu.style.left = Math.min(window.innerWidth - 180, e.clientX) + 'px';
  menu.style.top = Math.min(window.innerHeight - 200, e.clientY) + 'px';
  menu.classList.remove('hidden');
}

function switchTab(tabId) {
  if (state.activeTabId === tabId) return;
  const prevTab = getActiveTab();
  if (prevTab) {
    if (state.editing) {
      prevTab.content = getEditContent();
      prevTab.fixed = prevTab.content;
      prevTab.isDirty = true;
    }
    prevTab.scrollPos = $('content').scrollTop || 0;
  }
  exitEdit();
  state.activeTabId = tabId;
  syncStateFromActiveTab();
  const nextTab = getActiveTab();
  if (!nextTab) return;
  setFixes(nextTab.fixes || [], nextTab.stats || {});
  renderContent(nextTab.content, nextTab.title || nextTab.name);
  document.title = (nextTab.title || nextTab.name) + ' - ReadMD';
  setFileTitle(nextTab.title || nextTab.name, !nextTab.isVirtual && hasPy, nextTab.path);
  if (nextTab.scrollPos) {
    requestAnimationFrame(() => { $('content').scrollTop = nextTab.scrollPos; });
  }
  updateStatus();
  renderTabsBar();
  afterRender();
}

function promptDirtyClose(tabName) {
  const _t = (k, p) => {
    if (window.i18n && window.i18n.t) {
      const v = window.i18n.t(k, p);
      if (v && v !== k) return v;
    }
    return null;
  };
  return new Promise(resolve => {
    const modal = $('close-confirm-modal');
    if (!modal) {
      const ok = confirm((_t('dialog.unsavedMsg', { name: tabName })) || `文档「${tabName}」有未保存的修改，确定要关闭吗？`);
      resolve(ok ? 'discard' : 'cancel');
      return;
    }
    const titleEl = $('close-confirm-title');
    if (titleEl) titleEl.textContent = _t('dialog.unsavedTitle') || '是否保存对文档的修改？';
    const descEl = $('close-confirm-desc');
    if (descEl) descEl.textContent = _t('dialog.unsavedMsgDesc', { name: tabName || (_t('tabs.untitled') || '文档') }) || `「${tabName || '文档'}」已被修改，如果直接关闭，未保存的内容将会丢失。`;
    modal.classList.remove('hidden');

    const onKeyDown = (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        cleanUp('cancel');
      }
    };

    const onBackdropClick = (e) => {
      if (e.target === modal) {
        cleanUp('cancel');
      }
    };

    const cleanUp = (action) => {
      modal.classList.add('hidden');
      $('close-confirm-save').onclick = null;
      $('close-confirm-discard').onclick = null;
      $('close-confirm-cancel').onclick = null;
      modal.removeEventListener('click', onBackdropClick);
      document.removeEventListener('keydown', onKeyDown);
      resolve(action);
    };

    $('close-confirm-save').onclick = () => cleanUp('save');
    $('close-confirm-discard').onclick = () => cleanUp('discard');
    $('close-confirm-cancel').onclick = () => cleanUp('cancel');
    modal.addEventListener('click', onBackdropClick);
    document.addEventListener('keydown', onKeyDown);

    // 聚焦主要行动按钮
    const saveBtn = $('close-confirm-save');
    if (saveBtn) setTimeout(() => saveBtn.focus(), 30);
  });
}


async function closeTab(tabId, force = false) {
  const tab = state.tabs.find(t => t.id === tabId);
  if (!tab) return;
  if (tab.isDirty && !force) {
    const action = await promptDirtyClose(tab.title || tab.name);
    if (action === 'cancel') return;
    if (action === 'save') {
      if (state.activeTabId !== tabId) switchTab(tabId);
      await saveEdit();
    }
  }
  const idx = state.tabs.findIndex(t => t.id === tabId);
  state.tabs.splice(idx, 1);
  if (state.activeTabId === tabId) {
    if (state.tabs.length > 0) {
      const nextIdx = Math.min(idx, state.tabs.length - 1);
      switchTab(state.tabs[nextIdx].id);
    } else {
      state.activeTabId = null;
      goHome();
    }
  }
  renderTabsBar();
}

async function closeOtherTabs(keepTabId) {
  const keepTab = state.tabs.find(t => t.id === keepTabId);
  if (!keepTab) return;
  for (const t of [...state.tabs]) {
    if (t.id !== keepTabId) {
      if (t.isDirty) {
        const action = await promptDirtyClose(t.title || t.name);
        if (action === 'cancel') return;
        if (action === 'save') {
          switchTab(t.id);
          await saveEdit();
        }
      }
    }
  }
  state.tabs = [keepTab];
  state.activeTabId = keepTabId;
  syncStateFromActiveTab();
  renderTabsBar();
}

async function closeAllTabs() {
  for (const t of [...state.tabs]) {
    if (t.isDirty) {
      const action = await promptDirtyClose(t.title || t.name);
      if (action === 'cancel') return;
      if (action === 'save') {
        switchTab(t.id);
        await saveEdit();
      }
    }
  }
  state.tabs = [];
  state.activeTabId = null;
  goHome();
  renderTabsBar();
}

