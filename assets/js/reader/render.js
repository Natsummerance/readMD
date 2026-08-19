'use strict';
/* ============================================================
   ReadMD Reader - Document Parsing & Chunked Rendering
   ============================================================ */

/* ---------------- 打开 / 渲染 ---------------- */

function setFileTitle(name, canRename, fullPath) {
  const el = $('file-title');
  if (!el) return;
  el.textContent = name || '';
  el.disabled = !canRename;
  el.title = canRename ? ((fullPath || name) + '\n点击重命名（F2）') : (name || '');
  el.setAttribute('aria-label', canRename ? ('当前文件 ' + name + '，点击重命名') : (name || '当前文档'));
  if (state.tabs && state.tabs.length > 0) {
    el.classList.add('hidden');
    const tabsCont = $('doc-tabs-container');
    if (tabsCont) tabsCont.classList.remove('hidden');
  } else if (name) {
    el.classList.remove('hidden');
    const tabsCont = $('doc-tabs-container');
    if (tabsCont) tabsCont.classList.add('hidden');
  }
}

function cancelFileRename() {
  const wrap = $('file-rename-wrap');
  if (wrap) wrap.remove();
  const title = $('file-title');
  if (title && (!state.tabs || state.tabs.length === 0)) title.classList.remove('hidden');
}

function openFileRename() {
  if (state.editing) {
    showToast('编辑模式下不可重命名，请先保存或退出编辑');
    return;
  }
  const activeTab = getActiveTab();
  if (activeTab && state.tabs.length > 0) {
    const bar = $('doc-tabs-bar');
    const tabEl = bar ? bar.querySelector(`[data-tab-id="${activeTab.id}"]`) : null;
    if (tabEl) {
      const titleSpan = tabEl.querySelector('.tab-title');
      if (titleSpan) { startTabInlineRename(activeTab, titleSpan, tabEl); return; }
    }
  }
  const title = $('file-title');
  if (!title) return;
  const current = state.file || title.textContent || '';
  if (!current) return;
  const oldName = current.split(/[\\/]/).pop() || '';
  const dot = oldName.lastIndexOf('.');
  const stem = dot > 0 ? oldName.slice(0, dot) : oldName;
  const ext = dot > 0 ? oldName.slice(dot) : '';

  cancelFileRename();
  title.classList.add('hidden');
  const wrap = document.createElement('div');
  wrap.id = 'file-rename-wrap';
  wrap.className = 'file-rename-wrap';
  wrap.innerHTML = '<input id="file-rename-input" class="file-rename-input" value="' + stem.replace(/"/g, '&quot;') + '" spellcheck="false" autocomplete="off" aria-label="文件名称"><span id="file-rename-ext" class="file-rename-ext">' + ext + '</span>';
  title.parentNode.insertBefore(wrap, title.nextSibling);

  const input = wrap.querySelector('#file-rename-input');
  input.focus();
  input.select();

  let committed = false;
  async function commit() {
    if (committed) return;
    committed = true;
    const nextStem = input.value.trim();
    if (!nextStem || nextStem === stem) { cancelFileRename(); return; }
    if (hasPy && py.rename_file && state.file) {
      try {
        const res = await py.rename_file(state.file, nextStem);
        if (res && res.ok) {
          state.file = res.path;
          state.sourceName = res.name;
          setFileTitle(res.name, true, res.path);
          addRecent(res.path);
          showToast('已重命名为：' + res.name);
        } else {
          showToast('重命名失败：' + ((res && res.error) || '未知错误'));
        }
      } catch (e) {
        showToast('重命名失败：' + e.message);
      }
    } else {
      setFileTitle(nextStem + ext, true, nextStem + ext);
    }
    cancelFileRename();
  }

  input.addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); commit(); }
    else if (e.key === 'Escape') { e.preventDefault(); cancelFileRename(); }
  });
  input.addEventListener('blur', () => { setTimeout(() => { if (!committed) commit(); }, 150); });
}


async function loadFile(path) {
  if (!path) return;
  const existingTab = findTabByPath(path);
  if (existingTab) {
    switchTab(existingTab.id);
    return;
  }
  setProgress(8);
  try {
    const r = await apiFetch('/api/file?p=' + encodeURIComponent(path));
    if (!r.ok) {
      const d = await r.json().catch(() => ({}));
      showToast('无法打开：' + (d.error || r.status));
      return;
    }
    const d = await r.json();
    const newTab = {
      id: 'tab_' + Date.now() + '_' + Math.random().toString(36).slice(2, 6),
      mode: 'file',
      source: 'file',
      path: d.path,
      dir: d.dir,
      name: d.name,
      title: d.name,
      content: d.content,
      original: d.original,
      fixed: d.content,
      fixes: d.fixes || [],
      stats: d.stats || {},
      size: d.size,
      mtime: d.mtime,
      encoding: d.encoding,
      webAssets: [],
      isDirty: false,
      scrollPos: 0,
      isVirtual: false,
    };
    state.tabs.push(newTab);
    state.activeTabId = newTab.id;
    syncStateFromActiveTab();
    setFixes(d.fixes || [], d.stats || {});
    renderContent(d.content, d.name);
    document.title = d.name + ' - ReadMD';
    setFileTitle(d.name, hasPy, d.path);
    addRecent(d.path);
    pushHistory(d.path);
    saveLastFile(d.path);
    updateStatus();
    exitEdit();
    clearAiOutput();
    renderTabsBar();
    setProgress(100);
    if (d.structured) showToast('已智能识别 TXT 结构（标题 / 表格 / 列表 / 目录）');
    afterRender();
  } catch (e) {
    console.error(e);
    showToast('加载失败：' + e.message);
    setProgress(0);
  }
}


/* ---------------- 外部唤起（单实例常驻：托盘 / 双击 .md） ---------------- */

async function openExternalFile(path) {
  if (hasPy && py.show_window) {
    try { await py.show_window(); } catch (e) { /* ignore */ }
  }
  if (path) openInitialFile(path);
}
window.openExternalFile = openExternalFile;

let controlPollTimer = null;
async function pollControl() {
  try {
    const r = await apiFetch('/api/control/next');
    const d = await r.json();
    if (d && d.pending) openExternalFile(d.file || '');
  } catch (e) { /* ignore */ }
}
function startControlPoll() {
  stopControlPoll();
  controlPollTimer = setInterval(pollControl, 2000);
}
function stopControlPoll() {
  if (controlPollTimer) clearInterval(controlPollTimer);
  controlPollTimer = null;
}

function setEditBtn(label) {
  const el = $('btn-edit');
  if (!el) return;
  const sp = el.querySelector('span.tb-label');
  if (sp) sp.textContent = label;
  else el.textContent = label;
}

function setFixes(fixes, stats) {
  state.fixes = fixes || [];
  state.stats = stats || {};
  const n = state.fixes.length;
  const el = $('btn-fix');
  if (!el) return;
  const sp = el.querySelector('span');
  const lbl = n ? ('修复详情（' + n + '）') : '修复详情';
  if (sp) sp.textContent = lbl;
  else el.textContent = lbl;
  el.title = n ? ('本次自动修正 ' + n + ' 处') : '本次自动修正详情';
}

const INCREMENTAL_THRESHOLD = 300 * 1024; // 300KB 以上走增量渲染
const INCREMENTAL_LINES = 6000;

function renderContent(content, name) {
  const saved = state.scrollPos[normalizePath(name || state.file || '')] || 0;
  const big = content.length > INCREMENTAL_THRESHOLD || content.split('\n').length > INCREMENTAL_LINES;
  if (big) {
    renderContentIncremental(content, saved);
    return;
  }
  const prot = protectMath(content);
  const html = marked.parse(prot.src, { gfm: true, breaks: false });
  const finalHtml = restoreMath(html, prot.saved);
  $('content').innerHTML = '<article class="markdown-body">' + finalHtml + '</article>';
  postProcess();
  if (saved) requestAnimationFrame(() => { $('content').scrollTop = saved; });
}

/* 大文档分块：优先按围栏代码块 / 空行切块，超长块按行硬切 */
function splitMdBlocks(md) {
  const lines = String(md || '').split('\n');
  const blocks = [];
  let buf = [];
  let inFence = false;
  const flush = () => {
    if (buf.length) { blocks.push(buf.join('\n')); buf = []; }
  };
  for (const line of lines) {
    const t = line.trim();
    if (!inFence && /^```/.test(t)) {
      flush();
      inFence = true;
      buf.push(line);
      continue;
    }
    if (inFence) {
      buf.push(line);
      if (t.startsWith('```')) { flush(); inFence = false; }
      continue;
    }
    if (t === '') {
      flush();
      continue;
    }
    buf.push(line);
  }
  flush();
  if (inFence && buf.length) blocks.push(buf.join('\n'));
  const MAX_BLOCK_LINES = 200;
  const out = [];
  for (const b of blocks) {
    const ls = b.split('\n');
    if (ls.length <= MAX_BLOCK_LINES) { out.push(b); continue; }
    for (let i = 0; i < ls.length; i += MAX_BLOCK_LINES) {
      out.push(ls.slice(i, i + MAX_BLOCK_LINES).join('\n'));
    }
  }
  return out;
}

async function renderContentIncremental(content, savedTop) {
  const el = $('content');
  el.innerHTML = '<article class="markdown-body"></article>';
  const body = el.querySelector('.markdown-body');
  const blocks = splitMdBlocks(content);
  const total = blocks.length;
  if (total <= 1) {
    const prot = protectMath(content);
    body.innerHTML = restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved);
    postProcess();
    if (savedTop) el.scrollTop = savedTop;
    return;
  }
  const prog = document.createElement('div');
  prog.id = 'render-progress';
  el.appendChild(prog);
  const CHUNK = 8;
  for (let i = 0; i < total; i += CHUNK) {
    const frag = document.createDocumentFragment();
    const end = Math.min(i + CHUNK, total);
    for (let k = i; k < end; k++) {
      const div = document.createElement('div');
      const prot = protectMath(blocks[k]);
      div.innerHTML = restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved);
      frag.appendChild(div);
    }
    body.appendChild(frag);
    const pct = Math.round((end / total) * 100);
    if (pct >= 100 || pct % 10 < 8) prog.textContent = '渲染中… ' + Math.min(pct, 100) + '%';
    if (end < total) await new Promise(r => setTimeout(r, 0));
  }
  prog.remove();
  if (savedTop) el.scrollTop = savedTop;
  postProcess();
}

function normalizeHeadingText(text) {
  return String(text || '').trim().toLowerCase()
    .replace(/^[0-9]+[.\-、\s]*/, '')
    .replace(/[^\w\u4e00-\u9fff]/g, '');
}

function findMatchingHeading(targetAnchor, linkText, headings) {
  if (!headings || !headings.length) return null;
  let decodedAnchor = '';
  try { decodedAnchor = decodeURIComponent(targetAnchor).replace(/^#/, '').trim(); } catch (e) { decodedAnchor = targetAnchor.replace(/^#/, '').trim(); }
  const normAnchor = normalizeHeadingText(decodedAnchor);
  const normLinkText = normalizeHeadingText(linkText);

  // 1. 直接 ID 精确匹配
  let direct = document.getElementById(decodedAnchor) || document.getElementById(targetAnchor);
  if (direct && headings.includes(direct)) return direct;

  // 2. 标题文本完全匹配（不区分大小写）
  for (const h of headings) {
    const hText = h.textContent.trim();
    if (hText.toLowerCase() === decodedAnchor.toLowerCase() || (linkText && hText.toLowerCase() === linkText.trim().toLowerCase())) {
      return h;
    }
  }

  // 3. 归一化语义文本匹配（消除标点、空格、序号差异）
  if (normLinkText || normAnchor) {
    for (const h of headings) {
      const normH = normalizeHeadingText(h.textContent);
      if (normH && (normH === normLinkText || normH === normAnchor)) {
        return h;
      }
    }
  }

  // 4. 前缀与包含关系模糊匹配
  if (normLinkText && normLinkText.length >= 2) {
    for (const h of headings) {
      const normH = normalizeHeadingText(h.textContent);
      if (normH && (normH.includes(normLinkText) || normLinkText.includes(normH))) {
        return h;
      }
    }
  }
  if (normAnchor && normAnchor.length >= 2) {
    for (const h of headings) {
      const normH = normalizeHeadingText(h.textContent);
      if (normH && (normH.includes(normAnchor) || normAnchor.includes(normH))) {
        return h;
      }
    }
  }

  return null;
}

function ensureHeadingIds(body) {
  const headings = body.querySelectorAll('h1, h2, h3, h4, h5, h6');
  const seen = {};
  headings.forEach((h, i) => {
    if (!h.id) {
      let slug = h.textContent.trim().toLowerCase()
        .replace(/[^\w\u4e00-\u9fff\s-]/g, '')
        .replace(/\s+/g, '-');
      if (!slug) slug = 'toc-h-' + i;
      if (seen[slug]) {
        seen[slug]++;
        slug = slug + '-' + seen[slug];
      } else {
        seen[slug] = 1;
      }
      h.id = slug;
    }
  });
}

function postProcess() {
  const body = document.querySelector('#content .markdown-body');
  if (!body) return;
  ensureHeadingIds(body);
  fixLinks(body);
  fixImages(body);
  buildToc();
  renderMath(body);
}

function resolvePath(baseDir, rel) {
  try {
    const url = new URL(rel, 'file:///' + String(baseDir || '').replace(/\\/g, '/') + '/');
    return decodeURIComponent(url.pathname.replace(/^\//, ''));
  } catch (e) {
    return rel;
  }
}

function fixLinks(body) {
  const allHeadings = Array.from(body.querySelectorAll('h1, h2, h3, h4, h5, h6'));
  body.querySelectorAll('a').forEach(a => {
    const href = a.getAttribute('href') || '';
    if (href.startsWith('#')) {
      const targetId = href.slice(1);
      a.addEventListener('click', e => {
        e.preventDefault();
        let el = document.getElementById(targetId);
        if (!el) {
          try {
            const decoded = decodeURIComponent(targetId);
            el = document.getElementById(decoded) || body.querySelector('[name="' + CSS.escape(decoded) + '"]');
          } catch (ex) {}
        }
        if (!el) {
          el = findMatchingHeading(targetId, a.textContent, allHeadings);
        }
        if (el) {
          el.scrollIntoView({ behavior: 'smooth', block: 'start' });
          el.classList.remove('heading-target-highlight');
          void el.offsetWidth;
          el.classList.add('heading-target-highlight');
          setTimeout(() => el.classList.remove('heading-target-highlight'), 1500);
        } else {
          showToast('未找到对应的文档小标题目标：' + (a.textContent || href), 2500);
        }
      });
      return;
    }

    if (/^(https?:|mailto:)/i.test(href)) {
      a.addEventListener('click', e => {
        e.preventDefault();
        if (hasPy) py.open_external(href); else window.open(href, '_blank');
      });
    } else if (/^file:/i.test(href)) {
      const p = decodeURIComponent(href.replace(/^file:\/\//i, ''));
      a.addEventListener('click', e => { e.preventDefault(); openPath(p); });
    } else {
      const p = resolvePath(state.dir, href.split('#')[0]);
      a.addEventListener('click', e => {
        e.preventDefault();
        if (MD_RE.test(p)) loadFile(p);
        else openPath(p);
      });
    }
  });
}

function fixImages(body) {
  body.querySelectorAll('img').forEach(im => {
    let src = im.getAttribute('src') || '';
    if (/^(https?:|data:)/i.test(src)) return;
    let p;
    if (src.startsWith('file:')) {
      p = decodeURIComponent(src.replace(/^file:\/\//i, ''));
    } else if (/^[A-Za-z]:[\\/]/.test(src)) {
      p = src; // 绝对 Windows 路径（如转换器提取的临时图片）
    } else if (src.startsWith('/raw?')) {
      return; // 已经处理过
    } else {
      p = resolvePath(state.dir, src);
    }
    im.src = '/raw?p=' + encodeURIComponent(p);
    im.onerror = () => { im.style.opacity = .45; im.alt = '[图片无法加载] ' + im.alt; };
  });
}

function openPath(p) {
  if (hasPy) py.open_path(p);
  else window.open('/raw?p=' + encodeURIComponent(p), '_blank');
}

/* ---------------- 虚拟文档（转换/网页/OCR） ---------------- */

async function renderVirtual(source, name, dir, content, fixes, extras) {
  exitEdit();
  const title = name || '未命名.md';
  const newTab = {
    id: 'tab_' + Date.now() + '_' + Math.random().toString(36).slice(2, 6),
    mode: 'virtual',
    source: source || 'virtual',
    path: null,
    dir: dir || '',
    name: title,
    title: title,
    content: content,
    original: content,
    fixed: content,
    fixes: fixes || [],
    stats: {},
    size: 0,
    mtime: 0,
    encoding: 'utf-8',
    webAssets: source === 'url' ? (((extras || {}).assets) || []) : [],
    isDirty: source === 'clipboard',
    scrollPos: 0,
    isVirtual: true,
  };
  state.tabs.push(newTab);
  state.activeTabId = newTab.id;
  syncStateFromActiveTab();
  setFixes(fixes || [], {});
  clearAiOutput();
  renderContent(content, title);
  document.title = title + ' - ReadMD';
  setFileTitle(title.slice(0, 80), false, '');
  $('btn-reload').disabled = true;
  updateStatus();
  renderTabsBar();
  setProgress(100);
  afterRender();
}


async function ensureModule(name, timeoutMs) {
  const t0 = Date.now();
  const limit = timeoutMs || 60000;
  if (!moduleLoadRequests[name]) {
    moduleLoadRequests[name] = apiFetch('/api/modules/load', {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name }),
    }).catch(() => null);
  }
  await moduleLoadRequests[name];
  while (Date.now() - t0 < limit) {
    try {
      const r = await apiFetch('/api/modules');
      const d = await r.json();
      const st = d.modules && d.modules[name];
      if (st === 'ready') return true;
      if (st === 'error') { delete moduleLoadRequests[name]; showToast('模块「' + name + '」加载失败，请重试'); return false; }
    } catch (e) { /* ignore */ }
    await new Promise(r => setTimeout(r, 800));
  }
  showToast('模块加载超时，请重试');
  return false;
}

async function convertFile(path) {
  if (!(await ensureModule('convert'))) return;
  busy(true);
  try {
    const r = await apiFetch('/api/convert?p=' + encodeURIComponent(path));
    const d = await r.json();
    if (r.status === 409) { showToast(d.error || '模块加载中…'); return; }
    if (!r.ok) { showToast(d.error || '转换失败'); return; }
    if (!d.content) { showToast(d.note || '未提取到内容'); return; }
    showConvertWarns(d.warns);
    if (d.saved && d.out) {
      showToast('已保存：' + d.out);
      await loadFile(d.out);
    } else if (d.skipped) {
      showToast('已存在同名 .md，跳过保存（可在批量转换中勾选“覆盖已存在”）', 3400);
      renderVirtual('convert', d.name, d.dir, d.content, d.fixes);
    } else {
      renderVirtual('convert', d.name, d.dir, d.content, d.fixes);
    }
  } catch (e) { showToast('转换失败：' + e.message); }
  finally { busy(false); }
}

function showConvertWarns(warns) {
  if (!warns || !warns.length) return;
  const bad = warns.filter(w => w.level === 'warn' || w.level === 'error');
  if (bad.length) showToast('转换完成，' + bad.length + ' 条质量警告（' + (bad[0].msg || '见校验报告') + '）', 3600);
}

function loadFileDialog() {
  if (hasPy) {
    py.choose_file().then(p => { if (p) loadFile(p); });
    return;
  }
  const input = $('file-input');
  input.value = '';
  input.onchange = async () => {
    const f = input.files && input.files[0];
    if (!f) return;
    const p = await uploadFile(f);
    if (p && MD_RE.test(p)) loadFile(p);
    else if (p) convertOrOcr(p, 'convert');
  };
  input.click();
}
