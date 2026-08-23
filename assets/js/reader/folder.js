'use strict';
/* ============================================================
   ReadMD Reader - Folder Tree Browser
   ============================================================ */

/* ---------------- 文件夹浏览 ---------------- */

async function openFolder() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!hasPy) { showToast(_t('toast.openFolderBrowserNotice') || '浏览器模式下请使用“打开文件”'); return; }
  let dir;
  try { dir = await py.choose_folder(); } catch (e) { dir = null; }
  if (!dir) return;
  await listFolder(dir);
}

async function listFolder(dir) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    const r = await apiFetch('/api/list?p=' + encodeURIComponent(dir));
    const d = await r.json();
    state.folder = d.dir;
    state.folderFiles = d.files || [];
    renderFolderList();
    showSide('files');
  } catch (e) { showToast(_t('toast.readFolderFail') || '读取文件夹失败'); }
}


function renderFolderList() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const box = $('file-list');
  box.innerHTML = '';
  if (!state.folder) return;

  const folderName = state.folder.replace(/\\/g, '/').split('/').filter(Boolean).pop() || state.folder;
  const header = document.createElement('div');
  header.className = 'dir-header';
  header.textContent = folderName;
  header.title = state.folder;
  box.appendChild(header);

  if (!state.folderFiles || !state.folderFiles.length) {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    const empty = document.createElement('div');
    empty.className = 'dir-label';
    empty.textContent = _t('sidebar.emptyFiles') || '（未找到 Markdown 文件）';
    box.appendChild(empty);
    return;
  }


  const normRoot = state.folder.replace(/\\/g, '/').replace(/\/$/, '');
  const rootNode = { name: folderName, type: 'dir', path: state.folder, children: {} };

  state.folderFiles.forEach(fullPath => {
    const normPath = fullPath.replace(/\\/g, '/');
    let rel = normPath;
    if (normPath.toLowerCase().startsWith(normRoot.toLowerCase() + '/')) {
      rel = normPath.slice(normRoot.length + 1);
    }
    const parts = rel.split('/').filter(Boolean);
    let curr = rootNode;
    parts.forEach((part, idx) => {
      const isFile = (idx === parts.length - 1);
      if (!curr.children[part]) {
        curr.children[part] = {
          name: part,
          type: isFile ? 'file' : 'dir',
          path: isFile ? fullPath : (normRoot + '/' + parts.slice(0, idx + 1).join('/')),
          children: isFile ? null : {}
        };
      }
      if (!isFile) {
        curr = curr.children[part];
      }
    });
  });

  const treeContainer = document.createElement('div');
  treeContainer.className = 'tree-children';
  treeContainer.setAttribute('role', 'tree');
  treeContainer.setAttribute('aria-label', _t('sidebar.files') || '文件');
  renderTreeNodes(treeContainer, rootNode.children, 0);
  box.appendChild(treeContainer);
  updateTreeTabIndex();
}

function renderTreeNodes(container, childrenObj, depth) {
  const keys = Object.keys(childrenObj || {}).sort((a, b) => {
    const itemA = childrenObj[a];
    const itemB = childrenObj[b];
    if (itemA.type !== itemB.type) {
      return itemA.type === 'dir' ? -1 : 1;
    }
    return a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' });
  });

  keys.forEach(key => {
    const item = childrenObj[key];
    const nodeEl = document.createElement('div');
    nodeEl.className = 'tree-node';
    nodeEl.setAttribute('role', 'none');

    const row = document.createElement('button');
    row.type = 'button';
    row.className = 'tree-row';
    row.setAttribute('role', 'treeitem');
    row.setAttribute('aria-level', String(depth + 1));
    row.tabIndex = -1;
    if (item.type === 'file' && item.path === state.file) {
      row.classList.add('active');
      row.setAttribute('aria-selected', 'true');
      row.setAttribute('aria-current', 'true');
    } else {
      row.setAttribute('aria-selected', 'false');
    }

    const toggle = document.createElement('span');
    toggle.className = 'tree-toggle';
    toggle.setAttribute('aria-hidden', 'true');

    if (item.type === 'dir') {
      const childrenCount = Object.keys(item.children || {}).length;
      if (childrenCount > 0) {
        toggle.textContent = '▶';
        toggle.classList.add('open');
      } else {
        toggle.classList.add('empty');
      }

      const icon = document.createElement('span');
      icon.textContent = '📁 ';
      icon.style.fontSize = '12px';
      icon.setAttribute('aria-hidden', 'true');

      const nameEl = document.createElement('span');
      nameEl.className = 'tree-name';
      nameEl.textContent = item.name;

      row.appendChild(toggle);
      row.appendChild(icon);
      row.appendChild(nameEl);
      nodeEl.appendChild(row);

      const childrenContainer = document.createElement('div');
      childrenContainer.className = 'tree-children';
      renderTreeNodes(childrenContainer, item.children, depth + 1);
      nodeEl.appendChild(childrenContainer);
      row.setAttribute('aria-expanded', 'true');

      row.addEventListener('click', e => {
        e.stopPropagation();
        const collapsed = childrenContainer.classList.toggle('collapsed');
        row.setAttribute('aria-expanded', collapsed ? 'false' : 'true');
        if (collapsed) {
          toggle.classList.remove('open');
        } else {
          toggle.classList.add('open');
        }
        updateTreeTabIndex();
      });
    } else {
      toggle.classList.add('empty');

      const icon = document.createElement('span');
      icon.textContent = '📄 ';
      icon.style.fontSize = '12px';
      icon.setAttribute('aria-hidden', 'true');

      const nameEl = document.createElement('span');
      nameEl.className = 'tree-name';
      nameEl.textContent = item.name;

      row.appendChild(toggle);
      row.appendChild(icon);
      row.appendChild(nameEl);
      row.title = item.path;
      nodeEl.appendChild(row);

      row.addEventListener('click', e => {
        e.stopPropagation();
        loadFile(item.path);
      });
    }

    container.appendChild(nodeEl);
  });
}

function visibleTreeRows() {
  return Array.from(document.querySelectorAll('#file-list .tree-row'))
    .filter(row => row.offsetParent !== null);
}

function updateTreeTabIndex() {
  const rows = visibleTreeRows();
  rows.forEach((row, index) => { row.tabIndex = index === 0 ? 0 : -1; });
}

function handleTreeKeydown(event, row) {
  const rows = visibleTreeRows();
  const index = rows.indexOf(row);
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault();
    const next = rows[(index + (event.key === 'ArrowDown' ? 1 : rows.length - 1)) % rows.length];
    next.focus();
    return;
  }
  if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
  event.preventDefault();
  const expanded = row.getAttribute('aria-expanded');
  if (event.key === 'ArrowRight') {
    if (expanded === 'false') {
      row.click();
    } else {
      const childContainer = row.closest('.tree-node')?.querySelector(':scope > .tree-children');
      const childRow = childContainer?.querySelector(':scope > .tree-node > .tree-row');
      if (childRow && childRow.offsetParent) childRow.focus();
    }
    return;
  }
  if (expanded === 'true') {
    row.click();
    return;
  }
  row.closest('.tree-node')?.parentElement?.closest('.tree-node')?.querySelector(':scope > .tree-row')?.focus();
}

function showSide(tab) {
  $('side').classList.remove('hidden');
  const tabs = { toc: $('tab-toc'), files: $('tab-files') };
  Object.entries(tabs).forEach(([name, button]) => {
    const active = name === tab;
    button.classList.toggle('active', active);
    button.setAttribute('aria-selected', active ? 'true' : 'false');
    button.tabIndex = active ? 0 : -1;
  });
  $('file-list').classList.toggle('hidden', tab !== 'files');
  $('toc-list').classList.toggle('hidden', tab !== 'toc');
  if (tab === 'files') {
    if (state.folderFiles.length) renderFolderList();
    else listFolder(state.dir || '');
  } else {
    updateTreeTabIndex();
  }
}

(function bindSideTabKeys() {
  const tablist = $('#side-tabs') || document.getElementById('side-tabs');
  if (!tablist) return;
  tablist.addEventListener('keydown', event => {
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
    event.preventDefault();
    const next = event.key === 'ArrowRight' ? 'files' : 'toc';
    showSide(next);
    $(next === 'files' ? 'tab-files' : 'tab-toc').focus();
  });
  document.addEventListener('keydown', event => {
    const row = event.target instanceof Element ? event.target.closest('#file-list .tree-row') : null;
    if (row) handleTreeKeydown(event, row);
  });
})();

function toggleSide(tab) {
  const side = $('side');
  if (!side.classList.contains('hidden')) {
    side.classList.add('hidden');
    return;
  }
  showSide(tab || 'toc');
}
