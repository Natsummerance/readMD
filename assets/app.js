'use strict';
/* ReadMD 前端逻辑：渲染、目录、搜索、主题、自动刷新、历史、转换 / 网页 / OCR / 编辑 */

const $ = id => document.getElementById(id);
const py = (window.pywebview && window.pywebview.api) ? window.pywebview.api : null;
const hasPy = !!py;

const MD_RE = /\.(md|markdown|mdown|mkd|mdx|txt)$/i;
const IMG_RE = /\.(png|jpe?g|bmp|webp|gif|tiff?)$/i;

const state = {
  file: null,          // 当前真实文件路径（虚拟文档为 null）
  dir: null,
  mtime: 0,
  size: 0,
  encoding: '',
  fixes: [],
  stats: null,
  original: '',
  fixed: '',
  mode: 'welcome',     // file | virtual
  source: '',          // file | convert | ocr | url
  sourceName: '',
  theme: 'auto',
  fontSize: 100,
  lineWidth: 860,
  autoReload: true,
  history: [],
  histIdx: -1,
  scrollPos: {},
  currentMarks: [],
  searchIndex: 0,
  lastQuery: '',
  folder: null,
  folderFiles: [],
  modules: {},         // convert/ocr/web -> idle|loading|ready|error
  modulesStarted: false,
  editing: false,
  busyCount: 0,
};

/* ---------------- 设置 ---------------- */

async function loadSettings() {
  try {
    if (hasPy) {
      const s = await py.get_settings();
      if (s && typeof s === 'object') Object.assign(state, s);
    } else {
      const s = JSON.parse(localStorage.getItem('readmd-settings') || '{}');
      Object.assign(state, s);
    }
  } catch (e) { /* ignore */ }
  applySettings();
}

async function saveSettings() {
  const s = {
    theme: state.theme, fontSize: state.fontSize, lineWidth: state.lineWidth,
    autoReload: state.autoReload,
  };
  try {
    if (hasPy) await py.save_settings(s);
    else localStorage.setItem('readmd-settings', JSON.stringify(s));
  } catch (e) { /* ignore */ }
}

function applySettings() {
  let theme = state.theme;
  if (theme === 'auto') {
    theme = (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light';
  }
  document.body.dataset.theme = theme;
  document.body.style.setProperty('--fs', (state.fontSize / 100).toFixed(2));
  document.body.style.setProperty('--line-width', state.lineWidth + 'px');
  $('btn-theme').textContent = theme === 'dark' ? '\u2600' : '\u263E';
}

function toggleTheme() {
  const cur = document.body.dataset.theme;
  const next = cur === 'dark' ? 'sepia' : (cur === 'sepia' ? 'light' : 'dark');
  state.theme = next;
  applySettings();
  saveSettings();
}

function zoom(delta) {
  state.fontSize = Math.max(70, Math.min(180, state.fontSize + delta));
  applySettings();
  saveSettings();
}

/* ---------------- 模块懒加载 ---------------- */

function startModules() {
  if (state.modulesStarted) return;
  state.modulesStarted = true;
  if (hasPy) { try { py.start_modules(); } catch (e) { /* ignore */ } }
  pollModules();
}

async function pollModules() {
  try {
    const r = await fetch('/api/modules');
    const d = await r.json();
    state.modules = d.modules || {};
    updateModuleUi();
    const pending = Object.values(state.modules).some(v => v === 'loading' || v === 'idle');
    if (pending && state.modulesStarted) setTimeout(pollModules, 900);
  } catch (e) {
    if (state.modulesStarted) setTimeout(pollModules, 2000);
  }
}

function updateModuleUi() {
  const m = state.modules;
  const ready = n => m[n] === 'ready';
  $('btn-convert').disabled = !ready('convert');
  $('btn-web').disabled = !ready('web');
  $('btn-ocr').disabled = !ready('ocr');
  ['w-convert', 'w-web', 'w-ocr'].forEach(id => {
    const el = $(id);
    if (el) el.disabled = false;
  });
  const parts = [];
  for (const [k, v] of Object.entries(m)) {
    const label = { convert: '转换', ocr: 'OCR', web: '网页' }[k] || k;
    if (v === 'ready') parts.push(label + '\u2713');
    else if (v === 'error') parts.push(label + '\u2717');
    else parts.push(label + '\u2026');
  }
  const el = $('status-mods');
  if (el) el.textContent = parts.length ? '模块 ' + parts.join(' ') : '';
}

/* ---------------- 最近文件 ---------------- */

async function refreshRecent() {
  const box = $('recent-box');
  const list = $('recent-list');
  if (!hasPy) { box.classList.add('hidden'); return; }
  let rec = [];
  try { rec = await py.get_recent() || []; } catch (e) { rec = []; }
  if (!rec.length) { box.classList.add('hidden'); return; }
  box.classList.remove('hidden');
  list.innerHTML = '';
  rec.forEach(p => {
    const li = document.createElement('li');
    const a = document.createElement('a');
    a.href = '#';
    a.textContent = p;
    a.title = p;
    a.addEventListener('click', e => { e.preventDefault(); loadFile(p); });
    li.appendChild(a);
    list.appendChild(li);
  });
}

async function addRecent(path) {
  if (hasPy && path) { try { await py.add_recent(path); } catch (e) { /* ignore */ } }
}

/* ---------------- 打开 / 渲染 ---------------- */

async function loadFile(path) {
  if (!path) return;
  setProgress(8);
  try {
    const r = await fetch('/api/file?p=' + encodeURIComponent(path));
    if (!r.ok) {
      const d = await r.json().catch(() => ({}));
      showToast('无法打开：' + (d.error || r.status));
      return;
    }
    const d = await r.json();
    state.mode = 'file';
    state.source = 'file';
    state.file = d.path;
    state.dir = d.dir;
    state.mtime = d.mtime;
    state.size = d.size;
    state.encoding = d.encoding;
    state.fixed = d.content;
    state.original = d.original;
    setFixes(d.fixes || [], d.stats || {});
    renderContent(d.content, d.name);
    document.title = d.name + ' - ReadMD';
    $('file-title').textContent = d.name;
    addRecent(d.path);
    pushHistory(d.path);
    saveLastFile(d.path);
    updateStatus();
    exitEdit();
    setProgress(100);
    afterRender();
  } catch (e) {
    console.error(e);
    showToast('加载失败：' + e.message);
    setProgress(0);
  }
}

function setFixes(fixes, stats) {
  state.fixes = fixes || [];
  state.stats = stats || {};
  $('btn-fix').textContent = state.fixes.length
    ? '\uD83D\uDEE0 修复 ' + state.fixes.length
    : '\uD83D\uDEE0 修复';
}

function renderContent(content, name) {
  const prot = protectMath(content);
  const html = marked.parse(prot.src, { gfm: true, breaks: false });
  const finalHtml = restoreMath(html, prot.saved);
  $('content').innerHTML = '<article class="markdown-body">' + finalHtml + '</article>';
  postProcess();
  const saved = state.scrollPos[normalizePath(name || state.file || '')];
  if (saved) requestAnimationFrame(() => { $('content').scrollTop = saved; });
}

function postProcess() {
  const body = document.querySelector('#content .markdown-body');
  if (!body) return;
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
  body.querySelectorAll('a').forEach(a => {
    const href = a.getAttribute('href') || '';
    if (href.startsWith('#')) return;
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

async function renderVirtual(source, name, dir, content, fixes) {
  exitEdit();
  state.mode = 'virtual';
  state.source = source;
  state.sourceName = name;
  state.file = null;
  state.dir = dir || '';
  state.mtime = 0;
  state.size = 0;
  state.encoding = 'utf-8';
  state.fixed = content;
  state.original = content;
  setFixes(fixes || [], {});
  renderContent(content, name);
  document.title = (name || '转换结果') + ' - ReadMD';
  $('file-title').textContent = (name || '转换结果').slice(0, 80);
  $('btn-reload').disabled = true;
  $('btn-edit').disabled = true;
  $('btn-saveas').disabled = false;
  updateStatus();
  setProgress(100);
  afterRender();
}

async function ensureModule(name, timeoutMs) {
  const t0 = Date.now();
  const limit = timeoutMs || 60000;
  while (Date.now() - t0 < limit) {
    try {
      const r = await fetch('/api/modules');
      const d = await r.json();
      const st = d.modules && d.modules[name];
      if (st === 'ready') return true;
      if (st === 'error') { showToast('模块「' + name + '」加载失败，请重试'); return false; }
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
    const r = await fetch('/api/convert?p=' + encodeURIComponent(path));
    const d = await r.json();
    if (r.status === 409) { showToast(d.error || '模块加载中…'); return; }
    if (!r.ok) { showToast(d.error || '转换失败'); return; }
    if (!d.content) { showToast(d.note || '未提取到内容'); return; }
    renderVirtual('convert', d.name, d.dir, d.content, d.fixes);
  } catch (e) { showToast('转换失败：' + e.message); }
  finally { busy(false); }
}

async function ocrFile(path) {
  if (!(await ensureModule('ocr'))) return;
  busy(true);
  try {
    const r = await fetch('/api/ocr?p=' + encodeURIComponent(path));
    const d = await r.json();
    if (r.status === 409) { showToast(d.error || '模块加载中…'); return; }
    if (!r.ok) { showToast(d.error || 'OCR 失败'); return; }
    if (!d.content) { showToast(d.note || '未识别到文字'); return; }
    renderVirtual('ocr', d.name, d.dir, d.content, d.fixes);
  } catch (e) { showToast('OCR 失败：' + e.message); }
  finally { busy(false); }
}

async function webToMd(url, crawl) {
  if (!url) return;
  if (!(await ensureModule('web'))) return;
  busy(true);
  try {
    const r = await fetch('/api/url?u=' + encodeURIComponent(url) + '&crawl=' + (crawl ? '1' : '0'));
    const d = await r.json();
    if (r.status === 409) { showToast(d.error || '模块加载中…'); return; }
    if (!r.ok) { showToast(d.error || '抓取失败'); return; }
    if (!d.content) { showToast(d.note || '未能提取到正文'); return; }
    renderVirtual('url', url, d.dir, d.content, d.fixes);
  } catch (e) { showToast('抓取失败：' + e.message); }
  finally { busy(false); }
}

/* ---------------- 文件选择（含浏览器兜底） ---------------- */

function chooseFile(mode) {
  if (hasPy) {
    py.choose_any_file().then(p => { if (p) convertOrOcr(p, mode); });
    return;
  }
  const input = $('file-input');
  input.value = '';
  input.onchange = async () => {
    const f = input.files && input.files[0];
    if (!f) return;
    const p = await uploadFile(f);
    if (p) convertOrOcr(p, mode);
  };
  input.click();
}

async function uploadFile(file) {
  const ext = '.' + (file.name.split('.').pop() || 'bin');
  try {
    const r = await fetch('/api/upload?ext=' + encodeURIComponent(ext), { method: 'POST', body: file });
    const d = await r.json();
    return d.path || null;
  } catch (e) { showToast('上传失败'); return null; }
}

function convertOrOcr(p, mode) {
  if (mode === 'ocr' || IMG_RE.test(p) || /\.pdf$/i.test(p)) ocrFile(p);
  else convertFile(p);
}

/* ---------------- 编辑模式 ---------------- */

function toggleEdit() {
  if (state.editing) { exitEdit(); return; }
  if (state.mode !== 'file' || !state.file) { showToast('仅本地 Markdown 文件可编辑'); return; }
  $('edit-area').value = state.original || '';
  $('edit-bar').classList.remove('hidden');
  $('edit-area').classList.remove('hidden');
  $('content').classList.add('hidden');
  state.editing = true;
  $('btn-edit').textContent = '\u270E 编辑中';
  $('edit-area').focus();
}

function exitEdit() {
  if (!state.editing) {
    $('edit-bar').classList.add('hidden');
    $('edit-area').classList.add('hidden');
    $('content').classList.remove('hidden');
    $('btn-edit').textContent = '编辑';
    return;
  }
  $('edit-bar').classList.add('hidden');
  $('edit-area').classList.add('hidden');
  $('content').classList.remove('hidden');
  state.editing = false;
  $('btn-edit').textContent = '编辑';
}

async function saveEdit() {
  if (!state.file || !state.editing) return;
  const content = $('edit-area').value;
  busy(true);
  try {
    let ok;
    if (hasPy) {
      ok = await py.save_file(state.file, content, state.encoding || 'utf-8');
    } else {
      const r = await fetch('/api/save', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: state.file, content, encoding: state.encoding || 'utf-8' }),
      });
      ok = await r.json();
    }
    if (ok && ok.ok !== false) {
      showToast(ok.backup ? '已保存（备份：' + ok.backup + '）' : '已保存');
      exitEdit();
      await loadFile(state.file);
    } else {
      showToast('保存失败：' + ((ok && ok.error) || '未知错误'));
    }
  } catch (e) { showToast('保存失败：' + e.message); }
  finally { busy(false); }
}

async function saveAs() {
  const content = state.fixed || state.original || '';
  const name = (state.sourceName || state.file || 'document').replace(/[\\/]/g, '_');
  const suggested = name.replace(/\.[^.]+$/, '') + '.md';
  if (hasPy) {
    const out = await py.save_as(content, suggested);
    if (out) showToast('已保存：' + out);
  } else {
    const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = suggested;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 3000);
    showToast('已下载：' + suggested);
  }
}

/* ---------------- 数学公式 ---------------- */

function protectMath(src) {
  const saved = [];
  const save = m => { saved.push(m); return '\x01M' + (saved.length - 1) + '\x01'; };
  const looksMath = body => /[\\^_{}]/.test(body) || (/[A-Za-z\u0391-\u03C9]/.test(body) && !/\s/.test(body));
  src = src.replace(/\$\$([\s\S]+?)\$\$/g, (m, b) => save('$$' + b + '$$'));
  src = src.replace(/\\\(([\s\S]+?)\\\)/g, (m, b) => save('\\(' + b + '\\)'));
  src = src.replace(/\\\[([\s\S]+?)\\\]/g, (m, b) => save('\\[' + b + '\\]'));
  src = src.replace(/(^|[^\\$A-Za-z0-9])\$([^$\n]+?)\$/g, (m, pre, b) => looksMath(b) ? pre + save('$' + b + '$') : m);
  return { src, saved };
}

function restoreMath(html, saved) {
  return html.replace(/\x01M(\d+)\x01/g, (m, i) => saved[+i] || m);
}

function renderMath(body) {
  const html = body.innerHTML;
  if (!/\$\$|\\\(|\\\[|\$[^$\n]+\$/.test(html)) return;
  if (window.MathJax) {
    try { MathJax.typesetPromise([body]).catch(() => {}); } catch (e) { /* ignore */ }
    return;
  }
  window.MathJax = {
    tex: { inlineMath: [['$', '$'], ['\\(', '\\)']], displayMath: [['$$', '$$'], ['\\[', '\\]']] },
    options: { skipHtmlTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'] },
    startup: { typeset: false },
  };
  const s = document.createElement('script');
  s.src = '/assets/vendor/mathjax/tex-svg.js';
  s.onload = () => { try { MathJax.typesetPromise([body]).catch(() => {}); } catch (e) { /* ignore */ } };
  document.head.appendChild(s);
}

/* ---------------- 目录 ---------------- */

function buildToc() {
  const list = $('toc-list');
  list.innerHTML = '';
  const headings = document.querySelectorAll('#content h1, #content h2, #content h3');
  headings.forEach((h, i) => {
    if (!h.id) h.id = 'toc-' + i;
    const a = document.createElement('a');
    a.href = '#' + h.id;
    a.textContent = h.textContent.trim() || ('章节 ' + (i + 1));
    a.className = 'lv' + (+h.tagName[1]);
    a.addEventListener('click', e => {
      e.preventDefault();
      const el = document.getElementById(h.id);
      if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });
    list.appendChild(a);
  });
}

/* ---------------- 搜索 ---------------- */

function clearMarks() {
  state.currentMarks.forEach(m => {
    const p = m.parentNode;
    if (!p) return;
    p.replaceChild(document.createTextNode(m.textContent), m);
    p.normalize();
  });
  state.currentMarks = [];
  state.searchIndex = 0;
  updateSearchCount();
}

function doSearch(q) {
  clearMarks();
  state.lastQuery = q;
  if (!q) { updateSearchCount(); return; }
  const body = document.querySelector('#content .markdown-body');
  if (!body) return;
  const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT, {
    acceptNode: n => {
      const p = n.parentNode;
      if (!p || p.tagName === 'SCRIPT' || p.tagName === 'STYLE') return NodeFilter.FILTER_REJECT;
      if (p.tagName === 'MARK' && p.classList.contains('hl')) return NodeFilter.FILTER_REJECT;
      return n.textContent.toLowerCase().includes(q.toLowerCase()) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
    },
  });
  const nodes = [];
  let node;
  while ((node = walker.nextNode())) nodes.push(node);
  nodes.forEach(n => {
    const text = n.textContent;
    const lower = text.toLowerCase();
    const ql = q.toLowerCase();
    const frag = document.createDocumentFragment();
    let i = 0, idx;
    while ((idx = lower.indexOf(ql, i)) !== -1) {
      if (idx > i) frag.appendChild(document.createTextNode(text.slice(i, idx)));
      const mark = document.createElement('mark');
      mark.className = 'hl';
      mark.textContent = text.slice(idx, idx + q.length);
      frag.appendChild(mark);
      state.currentMarks.push(mark);
      i = idx + q.length;
    }
    if (i < text.length) frag.appendChild(document.createTextNode(text.slice(i)));
    n.parentNode.replaceChild(frag, n);
  });
  updateSearchCount();
  jumpToMark(0);
}

function jumpToMark(dir) {
  if (!state.currentMarks.length) return;
  state.searchIndex = (state.searchIndex + dir + state.currentMarks.length) % state.currentMarks.length;
  state.currentMarks.forEach((m, i) => m.classList.toggle('cur', i === state.searchIndex));
  const m = state.currentMarks[state.searchIndex];
  m.scrollIntoView({ behavior: 'smooth', block: 'center' });
  updateSearchCount();
}

function updateSearchCount() {
  const total = state.currentMarks.length;
  $('search-count').textContent = total ? ((state.searchIndex % total) + 1) + '/' + total : (state.lastQuery ? '无结果' : '');
}

/* ---------------- 文件夹浏览 ---------------- */

async function openFolder() {
  if (!hasPy) { showToast('浏览器模式下请使用“打开文件”'); return; }
  let dir;
  try { dir = await py.choose_folder(); } catch (e) { dir = null; }
  if (!dir) return;
  await listFolder(dir);
}

async function listFolder(dir) {
  try {
    const r = await fetch('/api/list?p=' + encodeURIComponent(dir));
    const d = await r.json();
    state.folder = d.dir;
    state.folderFiles = d.files || [];
    renderFolderList();
    showSide('files');
  } catch (e) { showToast('读取文件夹失败'); }
}

function renderFolderList() {
  const box = $('file-list');
  box.innerHTML = '';
  const label = document.createElement('div');
  label.className = 'dir-label';
  label.textContent = state.folder || '';
  box.appendChild(label);
  if (!state.folderFiles.length) {
    const empty = document.createElement('div');
    empty.className = 'dir-label';
    empty.textContent = '（未找到 Markdown 文件）';
    box.appendChild(empty);
    return;
  }
  state.folderFiles.forEach(p => {
    const btn = document.createElement('button');
    btn.className = 'file-item';
    btn.textContent = p;
    btn.title = p;
    if (p === state.file) btn.style.color = 'var(--accent)';
    btn.addEventListener('click', () => loadFile(p));
    box.appendChild(btn);
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
  if (!side.classList.contains('hidden') && (tab === null || $('tab-' + tab).classList.contains('active'))) {
    side.classList.add('hidden');
    return;
  }
  showSide(tab || 'toc');
}

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

/* ---------------- 历史 / 状态 ---------------- */

function normalizePath(p) {
  return String(p || '').replace(/\\/g, '/').toLowerCase();
}

function pushHistory(path) {
  const n = normalizePath(path);
  state.history = state.history.slice(0, state.histIdx + 1);
  if (state.history[state.histIdx] !== n) {
    state.history.push(n);
    state.histIdx = state.history.length - 1;
  }
}

function historyBack() {
  if (state.histIdx > 0) {
    state.histIdx--;
    loadFile(state.history[state.histIdx]);
  }
}

function historyForward() {
  if (state.histIdx < state.history.length - 1) {
    state.histIdx++;
    loadFile(state.history[state.histIdx]);
  }
}

function updateStatus() {
  $('status-left').textContent = (state.mode === 'virtual' ? '[' + { convert: '转换', ocr: 'OCR', url: '网页' }[state.source] + '] ' : '') + (state.sourceName || state.file || '');
  const parts = [];
  if (state.stats) {
    const s = state.stats;
    const p2 = [];
    if (s.table) p2.push('表格 ' + s.table);
    if (s.bold) p2.push('加粗 ' + s.bold);
    if (s.math) p2.push('公式 ' + s.math);
    if (s.heading) p2.push('标题 ' + s.heading);
    if (s.misc) p2.push('其他 ' + s.misc);
    parts.push(p2.length ? '修正 ' + p2.join('、') : '无需修正');
  }
  if (state.size) parts.push((state.size / 1024).toFixed(1) + ' KB');
  if (state.encoding) parts.push(state.encoding);
  $('status-right').textContent = parts.join(' · ');
  const canEdit = state.mode === 'file' && MD_RE.test(state.file || '') && !state.editing;
  const canReload = state.mode === 'file';
  const canSaveas = state.mode === 'virtual' || state.fixed !== '';
  $('btn-edit').disabled = !canEdit && !state.editing;
  $('btn-reload').disabled = !canReload;
  $('btn-saveas').disabled = !canSaveas;
}

/* ---------------- 自动刷新 ---------------- */

let autoReloadTimer = null;
function startAutoReload() {
  stopAutoReload();
  autoReloadTimer = setInterval(async () => {
    if (!state.file || !state.autoReload || state.mode !== 'file') return;
    try {
      const r = await fetch('/api/file?p=' + encodeURIComponent(state.file) + '&meta=1');
      if (!r.ok) return;
      const d = await r.json();
      if (d.mtime !== state.mtime) {
        const sc = $('content').scrollTop;
        await loadFile(state.file);
        if (sc) $('content').scrollTop = sc;
      }
    } catch (e) { /* ignore */ }
  }, 2500);
}
function stopAutoReload() {
  if (autoReloadTimer) clearInterval(autoReloadTimer);
  autoReloadTimer = null;
}

/* ---------------- 工具 ---------------- */

function showToast(msg, ms) {
  const t = $('toast');
  t.textContent = msg;
  t.classList.remove('hidden');
  clearTimeout(showToast._t);
  showToast._t = setTimeout(() => t.classList.add('hidden'), ms || 2600);
}

function setProgress(p) {
  const el = $('progress');
  el.style.width = p + '%';
  if (p >= 100) setTimeout(() => { el.style.width = '0'; }, 400);
}

function busy(on) {
  state.busyCount = Math.max(0, state.busyCount + (on ? 1 : -1));
  $('busy').classList.toggle('hidden', state.busyCount === 0);
}

function saveLastFile(path) {
  localStorage.setItem('readmd-last', path);
  if (hasPy) {
    try { py.save_settings({ last: path }); } catch (e) { /* ignore */ }
  }
}

function afterRender() {
  startModules();
}

function installAssoc() {
  if (!hasPy) { showToast('浏览器模式下请在命令行运行 install.bat'); return; }
  py.install_association().then(ok => {
    showToast(ok === true ? '已设置为 .md 默认打开方式' : ('注册失败：' + ok));
  });
}

/* ---------------- 网页对话框 ---------------- */

function openWebDialog() {
  $('url-modal').classList.remove('hidden');
  $('url-input').focus();
}

function closeWebDialog() {
  $('url-modal').classList.add('hidden');
}

/* ---------------- 事件绑定 ---------------- */

function bindEvents() {
  $('btn-open').addEventListener('click', () => { loadFileDialog(); });
  $('w-open').addEventListener('click', () => { loadFileDialog(); });
  $('btn-folder').addEventListener('click', openFolder);
  $('w-folder').addEventListener('click', openFolder);

  $('btn-convert').addEventListener('click', () => chooseFile('convert'));
  $('w-convert').addEventListener('click', () => chooseFile('convert'));
  $('btn-ocr').addEventListener('click', () => chooseFile('ocr'));
  $('w-ocr').addEventListener('click', () => chooseFile('ocr'));
  $('btn-web').addEventListener('click', openWebDialog);
  $('w-web').addEventListener('click', openWebDialog);
  $('url-go').addEventListener('click', () => {
    const url = $('url-input').value.trim();
    const crawl = $('url-crawl').checked;
    closeWebDialog();
    webToMd(url, crawl);
  });
  $('url-close').addEventListener('click', closeWebDialog);
  $('url-modal').addEventListener('click', e => { if (e.target === $('url-modal')) closeWebDialog(); });
  $('url-input').addEventListener('keydown', e => { if (e.key === 'Enter') $('url-go').click(); });

  $('btn-edit').addEventListener('click', toggleEdit);
  $('edit-save').addEventListener('click', saveEdit);
  $('edit-cancel').addEventListener('click', exitEdit);
  $('btn-saveas').addEventListener('click', saveAs);

  $('btn-recent').addEventListener('click', refreshRecent);
  $('btn-reload').addEventListener('click', () => { if (state.file && state.mode === 'file') loadFile(state.file); });
  $('btn-toc').addEventListener('click', () => toggleSide('toc'));
  $('btn-fix').addEventListener('click', showFixModal);
  $('fix-close').addEventListener('click', () => $('fix-modal').classList.add('hidden'));
  $('fix-save').addEventListener('click', async () => {
    const content = state.fixed || '';
    if (!content) return;
    if (state.mode === 'virtual' || !state.file) { await saveAs(); return; }
    if (!hasPy) { showToast('浏览器模式请用“另存”'); return; }
    const out = await py.save_fixed(state.file, content);
    showToast(out ? '已保存：' + out : '保存失败');
  });
  $('fix-modal').addEventListener('click', e => { if (e.target === $('fix-modal')) $('fix-modal').classList.add('hidden'); });
  $('btn-search').addEventListener('click', toggleSearch);
  $('btn-theme').addEventListener('click', toggleTheme);
  $('btn-a').addEventListener('click', () => zoom(-10));
  $('btn-A').addEventListener('click', () => zoom(10));
  $('btn-print').addEventListener('click', () => window.print());
  $('btn-assoc').addEventListener('click', installAssoc);
  $('top-btn').addEventListener('click', () => { $('content').scrollTo({ top: 0, behavior: 'smooth' }); });
  $('recent-clear').addEventListener('click', async () => {
    if (hasPy) await py.clear_recent();
    refreshRecent();
  });

  $('search-close').addEventListener('click', closeSearch);
  $('search-next').addEventListener('click', () => jumpToMark(1));
  $('search-prev').addEventListener('click', () => jumpToMark(-1));
  $('search-input').addEventListener('input', e => doSearch(e.target.value));
  $('search-input').addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); jumpToMark(e.shiftKey ? -1 : 1); }
  });

  $('tab-toc').addEventListener('click', () => showSide('toc'));
  $('tab-files').addEventListener('click', () => showSide('files'));

  $('content').addEventListener('scroll', () => {
    $('top-btn').classList.toggle('hidden', $('content').scrollTop < 600);
  });

  document.addEventListener('keydown', e => {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === 'o') { e.preventDefault(); $('btn-open').click(); }
    else if (mod && e.key.toLowerCase() === 'f') { e.preventDefault(); toggleSearch(); }
    else if (mod && e.key.toLowerCase() === 'u') { e.preventDefault(); openWebDialog(); }
    else if (mod && e.key.toLowerCase() === 'e') { e.preventDefault(); if (!$('btn-edit').disabled) toggleEdit(); }
    else if (mod && e.key.toLowerCase() === 's') {
      if (state.editing) { e.preventDefault(); saveEdit(); }
    }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'f') { e.preventDefault(); toggleSide('toc'); }
    else if (mod && e.key.toLowerCase() === 'd') { e.preventDefault(); toggleTheme(); }
    else if (mod && e.key.toLowerCase() === 'r') { e.preventDefault(); if (state.file && state.mode === 'file') loadFile(state.file); }
    else if (mod && e.key.toLowerCase() === 'p') { e.preventDefault(); window.print(); }
    else if (mod && (e.key === '=' || e.key === '+')) { e.preventDefault(); zoom(10); }
    else if (mod && e.key === '-') { e.preventDefault(); zoom(-10); }
    else if (mod && e.key === 'ArrowLeft') { e.preventDefault(); historyBack(); }
    else if (mod && e.key === 'ArrowRight') { e.preventDefault(); historyForward(); }
    else if (e.key === 'Escape') {
      closeSearch();
      $('fix-modal').classList.add('hidden');
      closeWebDialog();
      if (state.editing) exitEdit();
    }
  });
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

function toggleSearch() {
  const bar = $('search-bar');
  if (bar.classList.contains('hidden')) {
    bar.classList.remove('hidden');
    $('search-input').focus();
    $('search-input').select();
  } else {
    closeSearch();
  }
}

function closeSearch() {
  $('search-bar').classList.add('hidden');
  clearMarks();
}

/* ---------------- 初始化 ---------------- */

async function openInitialFile(path) {
  if (!path) return;
  if (MD_RE.test(path)) { loadFile(path); return; }
  if (IMG_RE.test(path) || /\.pdf$/i.test(path)) { ocrFile(path); return; }
  convertFile(path);
}

async function init() {
  await loadSettings();
  bindEvents();
  refreshRecent();
  updateModuleUi();
  const params = new URLSearchParams(location.search);
  const file = params.get('file');
  if (file) {
    openInitialFile(file);
  } else {
    restoreLastFile();
  }
  startAutoReload();
}

async function restoreLastFile() {
  if (state.file) return;
  let last = null;
  try {
    if (hasPy) {
      const s = await py.get_settings();
      last = (s && s.last) || null;
    }
  } catch (e) { /* ignore */ }
  if (!last) last = localStorage.getItem('readmd-last');
  if (last && /\.(md|markdown|mdown|mkd|mdx|txt)$/i.test(last)) loadFile(last);
}

window.addEventListener('DOMContentLoaded', init);
window.addEventListener('beforeunload', () => {
  if (state.file && $('content')) {
    state.scrollPos[normalizePath(state.file)] = $('content').scrollTop;
  }
});