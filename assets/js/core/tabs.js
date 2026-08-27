'use strict';
/* ============================================================
   ReadMD Core - Multi-Tab Management & Safety Dialog
   ============================================================ */

/* ---------------- 多标签页系统 (Multi-Tab Management) ---------------- */

let tabRenderEpoch = 0;

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
  state.browserCopy = tab.browserCopy || false;
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
  if (tab.readerMode) state.pagination.mode = tab.readerMode;
}

function captureReaderState(tab) {
  if (!tab) return;
  const pagination = state.pagination || {};
  tab.scrollPos = $('content')?.scrollTop || 0;
  tab.readerMode = pagination.enabled ? pagination.mode : 'paged';
  tab.readerPage = pagination.enabled && pagination.mode === 'paged'
    ? pagination.currentPage
    : 0;
  tab.continuousScroll = pagination.enabled && pagination.mode === 'continuous'
    ? ($('content')?.scrollTop || 0)
    : (tab.scrollPos || 0);
}

function renderTabsBar() {
  const bar = $('doc-tabs-bar');
  const secBar = $('doc-tabs-secondary-bar');
  const dropdown = $('doc-tabs-dropdown');
  const overflowWrap = $('doc-tabs-overflow-wrap');
  const btnHome = $('btn-home');
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;

  if (state.tabs.length === 0) {
    if (bar) bar.innerHTML = '';
    if (secBar) { secBar.innerHTML = ''; secBar.classList.add('hidden'); }
    if (overflowWrap) overflowWrap.classList.add('hidden');
    if (btnHome) btnHome.classList.add('hidden');
    return;
  }

  if (btnHome) {
    if (state.mode === 'welcome') {
      btnHome.classList.add('hidden');
    } else {
      btnHome.classList.remove('hidden');
    }
  }

  const createTabEl = (tab) => {
    const el = document.createElement('div');
    el.className = 'tab-item' + (tab.id === state.activeTabId ? ' active' : '');
    el.dataset.tabId = tab.id;
    el.setAttribute('role', 'tab');
    el.setAttribute('aria-selected', tab.id === state.activeTabId ? 'true' : 'false');
    el.setAttribute('aria-controls', 'content');
    el.setAttribute('aria-keyshortcuts', 'Alt+Left Arrow Alt+Right Arrow Delete Backspace');
    el.tabIndex = tab.id === state.activeTabId ? 0 : -1;
    el.draggable = true;

    el.title = `${tab.path || tab.title || tab.name} (${_t('tabs.closeTab')}: Delete)`;
    if (tab.isDirty) {
      const dot = document.createElement('span');
      const reason = tab.externalChanged
        ? (_t('toast.saveConflict') || '文件已被外部修改')
        : (_t('tabs.dirty') || '未保存');
      dot.className = tab.externalChanged ? 'tab-dirty tab-external-changed' : 'tab-dirty';
      dot.title = reason;
      dot.setAttribute('aria-hidden', 'true');
      el.setAttribute('aria-description', reason);
      el.appendChild(dot);
    } else if (tab.externalChanged) {
      const dot = document.createElement('span');
      dot.className = 'tab-dirty tab-external-changed';
      dot.title = _t('toast.saveConflict') || '文件已被外部修改';
      dot.setAttribute('aria-hidden', 'true');
      el.setAttribute('aria-description', dot.title);
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
    closeBtn.tabIndex = -1;
    closeBtn.setAttribute('aria-hidden', 'true');
    closeBtn.addEventListener('click', e => {
      e.stopPropagation();
      closeTab(tab.id);
    });
    el.appendChild(closeBtn);

    el.addEventListener('click', () => switchTab(tab.id));
    el.addEventListener('keydown', e => {
      if (e.altKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
        e.preventDefault();
        const index = state.tabs.findIndex(item => item.id === tab.id);
        const targetIndex = e.key === 'ArrowLeft' ? index - 1 : index + 1;
        if (targetIndex >= 0 && targetIndex < state.tabs.length) {
          reorderTabs(tab.id, state.tabs[targetIndex].id, e.key === 'ArrowRight');
          renderTabsBar();
          focusVisibleTab(tab.id);
        }
        e.stopPropagation();
        return;
      }
      if (['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(e.key)) {
        e.preventDefault();
        moveTabFocus(tab.id, e.key);
        return;
      }
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        switchTab(tab.id);
        return;
      }
      if (e.key === 'Delete' || e.key === 'Backspace') {
        e.preventDefault();
        closeTab(tab.id);
      }
      if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
        e.preventDefault();
        const rect = el.getBoundingClientRect();
        openTabContextMenu({ clientX: rect.left + 12, clientY: rect.bottom + 4 }, tab.id);
      }
    });

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
      item.setAttribute('role', 'menuitem');
      item.tabIndex = -1;
      item.className = 'doc-tabs-dropdown-item' + (tab.id === state.activeTabId ? ' active' : '');
      item.type = 'button';
      item.setAttribute('role', 'menuitem');
      item.tabIndex = -1;
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

  const revealActiveTab = (container) => {
    const active = container?.querySelector('.tab-item.active');
    if (!active) return;
    const containerRect = container.getBoundingClientRect();
    const activeRect = active.getBoundingClientRect();
    const left = activeRect.left - containerRect.left + container.scrollLeft;
    const right = left + active.offsetWidth;
    if (left < container.scrollLeft + 8) {
      container.scrollLeft = Math.max(0, left - 8);
    } else if (right > container.scrollLeft + container.clientWidth - 8) {
      container.scrollLeft = right - container.clientWidth + 8;
    }
  };
  revealActiveTab(bar);
  revealActiveTab(secBar);

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

function focusVisibleTab(tabId) {
  requestAnimationFrame(() => {
    const matches = Array.from(document.querySelectorAll(`[data-tab-id="${CSS.escape(tabId)}"]`));
    const next = matches.find(item => item.offsetParent !== null) || matches[0];
    if (next && document.activeElement !== next) next.focus({ preventScroll: true });
  });
}

function moveTabFocus(activeTabId, key) {
  const index = state.tabs.findIndex(tab => tab.id === activeTabId);
  if (index < 0) return;
  const last = state.tabs.length - 1;
  let target = index;
  if (key === 'ArrowRight') target = index === last ? 0 : index + 1;
  else if (key === 'ArrowLeft') target = index === 0 ? last : index - 1;
  else if (key === 'Home') target = 0;
  else if (key === 'End') target = last;
  if (target === index) return;
  const targetId = state.tabs[target].id;
  switchTab(targetId).then(() => focusVisibleTab(targetId));
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
  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'tab-title-input';
  input.spellcheck = false;
  input.autocomplete = 'off';
  input.setAttribute('aria-label', _t('tabs.rename') || '重命名文件');
  const extension = document.createElement('span');
  extension.className = 'tab-rename-ext';
  extension.textContent = ext;
  wrap.append(input, extension);
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
  const tabIndex = state.tabs.findIndex(tab => tab.id === tabId);
  menu.querySelector('[data-action="move-left"]').disabled = tabIndex <= 0;
  menu.querySelector('[data-action="move-right"]').disabled = tabIndex < 0 || tabIndex >= state.tabs.length - 1;
  menu.style.left = Math.min(window.innerWidth - 180, e.clientX) + 'px';
  menu.style.top = Math.min(window.innerHeight - 200, e.clientY) + 'px';
  menu.classList.remove('hidden');
  menu.dataset.returnFocus = tabId;
  setTimeout(() => menu.querySelector('button:not([disabled])')?.focus(), 20);
}

function closeTabContextMenu({ restoreFocus = false } = {}) {
  const menu = $('tab-context-menu');
  if (!menu || menu.classList.contains('hidden')) return;
  menu.classList.add('hidden');
  if (!restoreFocus) return;
  const tabId = menu.dataset.returnFocus;
  if (tabId) focusVisibleTab(tabId);
}

async function renderActiveTab({ restoreScroll = false } = {}) {
  const nextTab = getActiveTab();
  if (!nextTab) return;
  const renderEpoch = ++tabRenderEpoch;
  setFixes(nextTab.fixes || [], nextTab.stats || {});
  await renderContent(nextTab.content, nextTab.title || nextTab.name);
  if (renderEpoch !== tabRenderEpoch) return;
  document.title = (nextTab.title || nextTab.name) + ' - ReadMD';
  setFileTitle(nextTab.title || nextTab.name, !nextTab.isVirtual && hasPy, nextTab.path);
  if (restoreScroll && nextTab.scrollPos) {
    requestAnimationFrame(() => { $('content').scrollTop = nextTab.scrollPos; });
  }
  updateStatus();
  renderTabsBar();
  afterRender();
}

function syncActiveTabDirty() {
  if (!state.editing) return;
  const tab = getActiveTab();
  if (!tab) return;
  const dirty = hasUnsavedEditorChanges();
  if (dirty) {
    tab.content = getEditContent();
    tab.fixed = tab.content;
  }
  if (tab.isDirty !== dirty) {
    tab.isDirty = dirty;
    renderTabsBar();
  }
}

async function activateTabForSave(tabId) {
  if (state.activeTabId !== tabId) {
    const previousTab = getActiveTab();
    if (previousTab && state.editing) {
      if (hasUnsavedEditorChanges()) {
        previousTab.content = getEditContent();
        previousTab.fixed = previousTab.content;
        previousTab.isDirty = true;
      }
    }
    exitEdit();
    state.activeTabId = tabId;
    syncStateFromActiveTab();
    renderActiveTab();
  }
  if (!state.editing) await toggleEdit();
  const nextTab = getActiveTab();
  const draft = nextTab ? nextTab.content : '';
  if (typeof cmView !== 'undefined' && cmView) {
    cmView.dispatch({ changes: { from: 0, to: cmView.state.doc.length, insert: draft } });
  } else if ($('edit-area')) {
    $('edit-area').value = draft;
  }
}

async function switchTab(tabId) {
  if (state.activeTabId === tabId) return;
  const prevTab = getActiveTab();
  if (prevTab) {
    if (state.editing) {
      if (hasUnsavedEditorChanges()) {
        prevTab.isDirty = true;
        const action = await promptDirtyClose(prevTab.title || prevTab.name || 'document');
        if (action === 'cancel') return;
        if (action === 'save') {
          await saveEdit();
          if (state.editing) return;
        } else {
          exitEdit();
          prevTab.content = prevTab.original;
          prevTab.fixed = prevTab.original;
          prevTab.isDirty = false;
        }
      }
      else {
        exitEdit();
      }
    }
    captureReaderState(prevTab);
  }
  exitEdit();
  state.activeTabId = tabId;
  syncStateFromActiveTab();
  renderTabsBar();
  const nextTabState = getActiveTab();
  if (nextTabState?.externalChanged && nextTabState.path && !nextTabState.isDirty) {
    nextTabState.externalChanged = false;
    await loadFile(nextTabState.path, { force: true });
    return;
  }
  const preferredPage = Number(nextTabState?.readerPage || 0);
  const rendered = renderActiveTab({ restoreScroll: true });
  Promise.resolve(rendered).then(() => {
    if (state.pagination.enabled && state.pagination.mode === 'paged' && preferredPage >= 0) {
      renderPage(preferredPage, null, true);
    } else if (state.pagination.enabled && state.pagination.mode === 'continuous' && nextTabState.continuousScroll) {
      requestAnimationFrame(() => { $('content').scrollTop = nextTabState.continuousScroll; });
    }
  });
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
      resolve('cancel');
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
    const cancelBtn = $('close-confirm-cancel');
    if (cancelBtn) setTimeout(() => cancelBtn.focus(), 30);
  });
}


async function closeTab(tabId, force = false) {
  const tab = state.tabs.find(t => t.id === tabId);
  if (!tab) return;
  if (tab.isDirty && !force) {
    const action = await promptDirtyClose(tab.title || tab.name);
    if (action === 'cancel') return;
    if (action === 'save') {
      await activateTabForSave(tabId);
      const saved = await saveEdit();
      if (!saved || (getActiveTab()?.id === tabId && state.editing)) return;
    }
  }
  const idx = state.tabs.findIndex(t => t.id === tabId);
  const focusedTabId = document.activeElement instanceof Element ? document.activeElement.dataset.tabId : null;
  state.tabs.splice(idx, 1);
  if (state.activeTabId === tabId) {
    if (state.tabs.length > 0) {
      const nextIdx = Math.min(idx, state.tabs.length - 1);
      const nextTabId = state.tabs[nextIdx].id;
      switchTab(nextTabId).then(() => focusVisibleTab(nextTabId));
    } else {
      state.activeTabId = null;
      goHome();
    }
  }
  renderTabsBar();
  if (focusedTabId === tabId) {
    const fallbackTab = state.tabs[Math.min(idx, state.tabs.length - 1)];
    focusVisibleTab(fallbackTab ? fallbackTab.id : state.activeTabId);
  }
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
        await activateTabForSave(t.id);
        const saved = await saveEdit();
        if (!saved || state.tabs.some(item => item.id === t.id && item.isDirty)) return;
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
        await activateTabForSave(t.id);
        const saved = await saveEdit();
        if (!saved || (getActiveTab()?.isDirty || state.editing)) return;
      }
    }
  }
  state.tabs = [];
  state.activeTabId = null;
  goHome();
  renderTabsBar();
}
