'use strict';

const TREE_ICONS = {
  dir: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 20h16a1 1 0 0 0 1-1V8a1 1 0 0 0-1-1h-8L9 5H4a1 1 0 0 0-1 1v13a1 1 0 0 0 1 1Z"/></svg>',
  file: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z"/><path d="M14 3v5h5"/></svg>'
};
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
  const box = $('file-list');
  box.innerHTML = '';
  if (!state.folder) return;

  const folderName = state.folder.replace(/\\/g, '/').split('/').filter(Boolean).pop() || state.folder;
  const header = document.createElement('div');
  header.className = 'dir-header';
  header.innerHTML = TREE_ICONS.dir + '<span></span>';
  header.querySelector('span').textContent = folderName;
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
  renderTreeNodes(treeContainer, rootNode.children, 0);
  box.appendChild(treeContainer);
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

    const row = document.createElement('div');
    row.className = 'tree-row';
    if (item.type === 'file' && item.path === state.file) {
      row.classList.add('active');
    }

    const toggle = document.createElement('span');
    toggle.className = 'tree-toggle';

    if (item.type === 'dir') {
      const childrenCount = Object.keys(item.children || {}).length;
      if (childrenCount > 0) {
        toggle.textContent = '▶';
        toggle.classList.add('open');
      } else {
        toggle.classList.add('empty');
      }

      const icon = document.createElement('span');
      icon.className = 'tree-icon';
      icon.innerHTML = TREE_ICONS.dir;

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

      row.addEventListener('click', e => {
        e.stopPropagation();
        const collapsed = childrenContainer.classList.toggle('collapsed');
        if (collapsed) {
          toggle.classList.remove('open');
        } else {
          toggle.classList.add('open');
        }
      });
    } else {
      toggle.classList.add('empty');

      const icon = document.createElement('span');
      icon.className = 'tree-icon';
      icon.innerHTML = TREE_ICONS.file;

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

function showSide(tab) {
  $('side').classList.remove('hidden');
  if (tab === 'files') {
    $('tab-files').classList.add('active');
    $('tab-toc').classList.remove('active');
    $('file-list').classList.remove('hidden');
    $('toc-list').classList.add('hidden');
    if (state.folderFiles.length) renderFolderList();
    else listFolder(state.dir || '');
  } else {
    $('tab-toc').classList.add('active');
    $('tab-files').classList.remove('active');
    $('toc-list').classList.remove('hidden');
    $('file-list').classList.add('hidden');
  }
}

function toggleSide(tab) {
  const side = $('side');
  if (!side.classList.contains('hidden')) {
    side.classList.add('hidden');
    return;
  }
  showSide(tab || 'toc');
}
