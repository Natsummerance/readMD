'use strict';
/* ReadMD 前端逻辑：渲染、目录、搜索、主题、自动刷新、历史、转换 / 网页 / OCR / 编辑 */

const $ = id => document.getElementById(id);
let py = (window.pywebview && window.pywebview.api) ? window.pywebview.api : null;
let hasPy = !!py;

/* pywebview 桥接注入可能晚于页面脚本执行（低配机实测晚 ~1s）。
   启动前短暂等待桥接，确保 report_ready / 托盘打开 / 单实例控制轮询可用。 */
function bindPy() {
  if (!hasPy && window.pywebview && window.pywebview.api) {
    py = window.pywebview.api;
    hasPy = true;
  }
  return hasPy;
}

const LAN_TOKEN = window.LAN_TOKEN || null;

function apiFetch(url, opts) {
  opts = opts || {};
  if (LAN_TOKEN) {
    opts.headers = Object.assign({}, opts.headers || {}, { 'X-ReadMD-Token': LAN_TOKEN });
  }
  return fetch(url, opts);
}

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
  webAssets: [],       // 网页图片临时资源；另存时复制到 <文档名>.assets
  theme: 'auto',
  fontSize: 100,
  lineWidth: 860,
  aiPanelWidth: 432,
  autoReload: true,
  history: [],
  histIdx: -1,
  scrollPos: {},
  currentMarks: [],
  searchIndex: 0,
  lastQuery: '',
  folder: null,
  folderFiles: [],
  modules: {},         // convert/ocr/web -> idle|loading|ready|error|disabled
  win7: false,         // Win7 版：功能裁剪与固定版运行时
  modulesStarted: false,
  editing: false,
  busyCount: 0,
  ai: {
    config: null, providers: [], busy: false, aborter: null, raw: '',
    templates: [], templateId: '', messages: [], sessionId: null, sessions: [],
    usage: null, sessUsage: { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 },
  },
  pvLayout: 'none', pvSync: false, pvSplitX: 50, pvSplitY: 46,
  export: {
    fmt: 'pdf', defaults: null, presets: {}, custom: {}, options: null, last: null, ready: false,
  },
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
    theme: state.theme, fontSize: state.fontSize, lineWidth: state.lineWidth, aiPanelWidth: state.aiPanelWidth,
    autoReload: state.autoReload, pvLayout: state.pvLayout, pvSync: state.pvSync,
    pvSplitX: state.pvSplitX, pvSplitY: state.pvSplitY,
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
  document.body.style.setProperty('--ai-panel-width', state.aiPanelWidth + 'px');
  $('btn-theme').textContent = theme === 'dark' ? '\u2600' : '\u263E';
}

function toggleTheme() {
  const cur = document.body.dataset.theme;
  const next = cur === 'dark' ? 'sepia' : (cur === 'sepia' ? 'light' : 'dark');
  state.theme = next;
  applySettings();
  saveSettings();
  applyCmTheme();
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
    const r = await apiFetch('/api/modules');
    const d = await r.json();
    state.modules = d.modules || {};
    state.win7 = !!d.win7;
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
  const disabled = n => m[n] === 'disabled';
  [['btn-convert', 'convert'], ['btn-web', 'web'], ['btn-ocr', 'ocr'], ['btn-ai', 'ai']].forEach(([id, key]) => {
    const el = $(id);
    if (!el) return;
    if (disabled(key)) {
      // Win7 版：按钮保持可点击，点击后提示“暂不支持”
      el.disabled = false;
      el.title = 'Win7 版暂不支持该功能';
    } else {
      el.disabled = !ready(key);
    }
  });
  ['w-convert', 'w-web', 'w-ocr', 'w-ai'].forEach(id => {
    const el = $(id);
    if (el) el.disabled = false;
  });
  if (ready('ai') && !state.ai.config) loadAiConfig();
  const parts = [];
  for (const [k, v] of Object.entries(m)) {
    const label = { convert: '转换', ocr: 'OCR', web: '网页', ai: 'AI' }[k] || k;
    if (v === 'ready') parts.push(label + '\u2713');
    else if (v === 'error') parts.push(label + '\u2717');
    else if (v === 'disabled') parts.push(label + ' Win7 暂不支持');
    else parts.push(label + '\u2026');
  }
  const el = $('status-mods');
  if (el) el.textContent = parts.length ? '模块 ' + parts.join(' ') : '';
}

function moduleBlocked(name) {
  if (state.modules[name] === 'disabled') {
    showToast('该功能在 Win7 版暂不支持（本版本仅保留 docx / pdf 转 MD 与导出功能）', 3400);
    return true;
  }
  return false;
}

/* ---------------- 最近文件 ---------------- */

function renderRecentList(list, rec, onOpen) {
  if (!list) return;
  list.innerHTML = '';
  rec.slice(0, 24).forEach(p => {
    const li = document.createElement('li');
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'recent-card';
    const name = String(p).split(/[\\/]/).pop() || p;
    const dir = String(p).slice(0, String(p).length - name.length).replace(/[\\/]+$/, '') || '';
    const nm = document.createElement('span');
    nm.className = 'recent-name'; nm.textContent = name; nm.title = p;
    const dp = document.createElement('span');
    dp.className = 'recent-dir'; dp.textContent = dir; dp.title = p;
    btn.appendChild(nm); btn.appendChild(dp);
    btn.addEventListener('click', e => { e.preventDefault(); onOpen(p); });
    li.appendChild(btn); list.appendChild(li);
  });
}

async function getRecentEntries() {
  if (!hasPy) return [];
  try { return await py.get_recent() || []; } catch (e) { return []; }
}

async function refreshRecent() {
  const box = $('recent-box');
  if (!hasPy) { box.classList.add('hidden'); return; }
  const rec = await getRecentEntries();
  if (!rec.length) { box.classList.add('hidden'); return; }
  box.classList.remove('hidden');
  renderRecentList($('recent-list'), rec, loadFile);
}

async function openHistoryModal() {
  const rec = await getRecentEntries();
  const modal = $('history-modal');
  const list = $('history-list');
  list.innerHTML = '';
  if (!rec.length) {
    const li = document.createElement('li');
    li.className = 'empty'; li.textContent = '暂无最近文件'; list.appendChild(li);
  } else {
    renderRecentList(list, rec, p => { modal.classList.add('hidden'); loadFile(p); });
  }
  modal.classList.remove('hidden');
}

async function clearRecent() {
  if (hasPy) await py.clear_recent();
  await refreshRecent();
  const list = $('history-list');
  if (list) list.innerHTML = '<li class="empty">暂无最近文件</li>';
}

async function addRecent(path) {
  if (hasPy && path) { try { await py.add_recent(path); } catch (e) { /* ignore */ } }
}

/* ---------------- 打开 / 渲染 ---------------- */

async function loadFile(path) {
  if (!path) return;
  setProgress(8);
  try {
    const r = await apiFetch('/api/file?p=' + encodeURIComponent(path));
    if (!r.ok) {
      const d = await r.json().catch(() => ({}));
      showToast('无法打开：' + (d.error || r.status));
      return;
    }
    const d = await r.json();
    state.mode = 'file';
    state.source = 'file';
    state.webAssets = [];
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
    clearAiOutput();
    setProgress(100);
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

async function renderVirtual(source, name, dir, content, fixes, extras) {
  exitEdit();
  state.mode = 'virtual';
  state.source = source;
  state.sourceName = name;
  state.webAssets = source === 'url' ? (((extras || {}).assets) || []) : [];
  state.file = null;
  state.dir = dir || '';
  state.mtime = 0;
  state.size = 0;
  state.encoding = 'utf-8';
  state.fixed = content;
  state.original = content;
  setFixes(fixes || [], {});
  clearAiOutput();
  renderContent(content, name);
  document.title = (name || '转换结果') + ' - ReadMD';
  $('file-title').textContent = (name || '转换结果').slice(0, 80);
  $('btn-reload').disabled = true;
  updateStatus();
  setProgress(100);
  afterRender();
}

async function ensureModule(name, timeoutMs) {
  const t0 = Date.now();
  const limit = timeoutMs || 60000;
  while (Date.now() - t0 < limit) {
    try {
      const r = await apiFetch('/api/modules');
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

/* ---------------- 批量转换（转 MD） ---------------- */

let convertJobTimer = null;
let convertLastDir = null;

async function openConvertModal() {
  if (!hasPy) { showToast('浏览器模式请使用“打开文件”转换'); return; }
  const note = $('convert-note');
  if (note) note.textContent = state.win7 ? 'Win7 版仅支持 docx / pdf 转 Markdown；转换结果自动保存为源文件同目录同名 .md。' : '转换结果自动保存为源文件同目录同名 .md（如 report.docx → report.md）。docx 公式、PDF 表格走专用解析，其余格式自动回退通用转换；输出经过严格校验（表格 / 代码围栏 / 公式 / 图片引用）。';
  $('convert-modal').classList.remove('hidden');
  $('convert-list').innerHTML = '';
  $('convert-status').textContent = '';
  $('convert-open-dir').classList.add('hidden');
}

function closeConvertModal() {
  stopConvertPoll();
  $('convert-modal').classList.add('hidden');
}

async function pickConvertFiles() {
  let files = [];
  try { files = await py.choose_many_files(); } catch (e) { files = []; }
  if (!files || !files.length) return;
  await startBatchConvert(files, $('convert-overwrite').checked);
}

async function pickConvertFolder() {
  let dir = null;
  try { dir = await py.choose_folder(); } catch (e) { dir = null; }
  if (!dir) return;
  try {
    const r = await apiFetch('/api/convert/collect?dir=' + encodeURIComponent(dir));
    const d = await r.json();
    if (!r.ok) throw new Error(d.error || '收集失败');
    const files = d.files || [];
    if (!files.length) { showToast('该目录下没有可转换的文件'); return; }
    convertLastDir = dir;
    await startBatchConvert(files, $('convert-overwrite').checked);
  } catch (e) { showToast('收集文件失败：' + e.message); }
}

async function startBatchConvert(files, overwrite) {
  if (!(await ensureModule('convert'))) return;
  const list = $('convert-list');
  list.innerHTML = '';
  files.forEach(p => {
    const row = document.createElement('div');
    row.className = 'convert-item queued';
    const nm = document.createElement('span');
    nm.className = 'convert-name';
    nm.textContent = p.split(/[\\/]/).pop();
    nm.title = p;
    const st = document.createElement('span');
    st.className = 'convert-state';
    st.textContent = '排队中';
    row.appendChild(nm); row.appendChild(st);
    list.appendChild(row);
  });
  $('convert-status').textContent = '准备中…';
  try {
    const r = await apiFetch('/api/convert/batch', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ paths: files, overwrite: !!overwrite }),
    });
    const d = await r.json();
    if (!r.ok) throw new Error(d.error || '启动失败');
    if (files.length) {
      const parts = files[0].split(/[\\/]/);
      parts.pop();
      convertLastDir = parts.join('\\');
    }
    pollConvertJob(d.job);
  } catch (e) {
    $('convert-status').textContent = '启动失败：' + e.message;
  }
}

function pollConvertJob(jid) {
  stopConvertPoll();
  convertJobTimer = setInterval(async () => {
    try {
      const r = await apiFetch('/api/convert/progress?job=' + encodeURIComponent(jid));
      if (!r.ok) { stopConvertPoll(); return; }
      const d = await r.json();
      renderConvertProgress(d);
      if (d.finished) stopConvertPoll();
    } catch (e) { stopConvertPoll(); }
  }, 600);
}

function stopConvertPoll() {
  if (convertJobTimer) { clearInterval(convertJobTimer); convertJobTimer = null; }
}

function renderConvertProgress(d) {
  const rows = $('convert-list').querySelectorAll('.convert-item');
  const statusMap = { ok: '\u2713 成功', skipped: '跳过（已存在）', error: '失败', canceled: '已取消', queued: '排队中' };
  let ok = 0, skipped = 0, err = 0, warnCount = 0;
  (d.items || []).forEach((it, i) => {
    const row = rows[i];
    if (row) {
      row.className = 'convert-item ' + (it.status || 'queued');
      const st = row.querySelector('.convert-state');
      if (st) {
        st.textContent = statusMap[it.status] || it.status;
        if (it.status === 'error' && it.error) st.title = it.error;
      }
    }
    if (it.status === 'ok') { ok++; warnCount += (it.warns || []).filter(w => w.level !== 'auto').length; }
    else if (it.status === 'skipped') skipped++;
    else if (it.status === 'error') err++;
  });
  const status = $('convert-status');
  if (!status) return;
  if (d.running) {
    status.textContent = '转换中 ' + d.done + '/' + d.total + '…';
  } else {
    status.textContent = '完成：成功 ' + ok + ' · 跳过 ' + skipped + ' · 失败 ' + err + (warnCount ? ' · 警告 ' + warnCount : '');
    $('convert-open-dir').classList.remove('hidden');
  }
}



async function ocrFile(path) {
  if (!(await ensureModule('ocr'))) return;
  busy(true);
  try {
    const r = await apiFetch('/api/ocr?p=' + encodeURIComponent(path));
    const d = await r.json();
    if (r.status === 409) { showToast(d.error || '模块加载中…'); return; }
    if (!r.ok) { showToast(d.error || 'OCR 失败'); return; }
    if (!d.content) { showToast(d.note || '未识别到文字'); return; }
    renderVirtual('ocr', d.name, d.dir, d.content, d.fixes);
  } catch (e) { showToast('OCR 失败：' + e.message); }
  finally { busy(false); }
}

const webRun = { running: false, cancelled: false, taskId: '', lastUrl: '' };

function normalizeWebUrl(url) {
  url = String(url || '').trim();
  if (url && !/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) url = 'https://' + url;
  return url;
}

function setWebStatus(text, kind) {
  const el = $('url-status');
  el.textContent = text || '';
  el.classList.toggle('error', kind === 'error');
  el.classList.toggle('success', kind === 'success');
}

function setWebProgress(percent, title, count) {
  const wrap = $('url-progress');
  wrap.classList.remove('hidden');
  wrap.setAttribute('aria-hidden', 'false');
  $('url-progress-bar').style.width = Math.max(0, Math.min(100, percent || 0)) + '%';
  $('url-progress-title').textContent = title || '处理中…';
  $('url-progress-count').textContent = count || '';
}

function setWebRunning(running) {
  webRun.running = running;
  $('url-go').disabled = running;
  $('url-render').disabled = running || !hasPy;
  $('url-cancel').classList.toggle('hidden', !running);
  $('url-input').disabled = running;
  $('url-mode').disabled = running;
  $('url-crawl').disabled = running;
  $('url-images').disabled = running;
}

async function postWebExtract(payload) {
  const response = await apiFetch('/api/web/extract', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  let data = {};
  try { data = await response.json(); } catch (e) { data = { error: '服务器返回了无法解析的响应' }; }
  if (!response.ok) {
    const error = new Error(data.error || ('网页转换失败（HTTP ' + response.status + '）'));
    error.code = data.code || 'request_failed';
    throw error;
  }
  return data;
}

async function extractOneWebPage(url, options, forceRender) {
  const base = {
    task_id: webRun.taskId, url, mode: options.mode,
    download_images: options.downloadImages,
  };
  let data = null;
  if (!forceRender) {
    setWebProgress(options.progress || 12, '下载并分析静态页面…', options.count || '');
    data = await postWebExtract(base);
    if (data.ok) return data;
    if (!data.render_required) {
      const error = new Error(data.error || '未能提取网页正文');
      error.code = data.code || 'extract_failed';
      throw error;
    }
  }
  if (!hasPy || !py.render_web_page) {
    const error = new Error(LAN_TOKEN
      ? '该页面需要 JavaScript。请在 ReadMD 桌面应用中使用动态渲染抓取。'
      : '当前环境不支持系统 WebView 动态渲染。');
    error.code = 'render_unavailable';
    throw error;
  }
  setWebProgress(Math.max(options.progress || 12, 24), '使用系统浏览器内核渲染…', options.count || '最长 15 秒');
  const rendered = await py.render_web_page(url, webRun.taskId, 15000);
  if (!rendered || !rendered.ok) {
    const error = new Error((rendered && rendered.error) || '动态网页渲染失败');
    error.code = (rendered && rendered.code) || 'render_failed';
    throw error;
  }
  setWebProgress(Math.max(options.progress || 12, 32), '使用 Mozilla Readability 提取…', options.count || '');
  data = await postWebExtract(Object.assign({}, base, {
    html: rendered.html || '', final_url: rendered.final_url || url,
    readability: rendered.readability || null,
  }));
  if (!data.ok) {
    const error = new Error(data.error || '动态页面中仍未识别到正文');
    error.code = data.code || 'extract_failed';
    throw error;
  }
  return data;
}

async function cancelWebTask() {
  if (!webRun.running) return;
  webRun.cancelled = true;
  setWebStatus('正在取消网页转换…');
  try {
    await apiFetch('/api/web/cancel', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ task_id: webRun.taskId }),
    });
  } catch (e) { /* local cancellation still applies */ }
  try { if (hasPy && py.cancel_web_render) await py.cancel_web_render(webRun.taskId); } catch (e) { /* ignore */ }
}

async function webToMd(url, crawl, forceRender) {
  url = normalizeWebUrl(url);
  if (!url || webRun.running) return;
  $('url-input').value = url;
  if (!(await ensureModule('web'))) return;
  webRun.taskId = 'web-' + Date.now() + '-' + Math.random().toString(16).slice(2);
  webRun.lastUrl = url;
  webRun.cancelled = false;
  setWebRunning(true);
  setWebStatus('正在准备网页转换…');
  const options = {
    mode: $('url-mode').value === 'full' ? 'full' : 'smart',
    downloadImages: $('url-images').checked,
  };
  const sections = [], assets = [], warnings = [], failures = [];
  try {
    const first = await extractOneWebPage(url, Object.assign({}, options, { progress: 10 }), !!forceRender);
    if (webRun.cancelled) throw Object.assign(new Error('已取消网页转换'), { code: 'cancelled' });
    sections.push(first.content);
    assets.push(...(first.assets || []));
    warnings.push(...(first.warnings || []));
    const links = crawl ? (first.links || []).slice(0, 9) : [];
    const total = 1 + links.length;
    for (let i = 0; i < links.length; i++) {
      if (webRun.cancelled) throw Object.assign(new Error('已取消网页转换'), { code: 'cancelled' });
      const pageNo = i + 2;
      const progress = 35 + Math.round((i / Math.max(1, links.length)) * 55);
      setWebProgress(progress, '抓取同站页面…', pageNo + ' / ' + total);
      try {
        const result = await extractOneWebPage(links[i], Object.assign({}, options, {
          progress, count: pageNo + ' / ' + total,
        }), false);
        sections.push(result.content.replace(/^# /, '## '));
        assets.push(...(result.assets || []));
        warnings.push(...(result.warnings || []));
      } catch (error) {
        failures.push({ url: links[i], error: error.message });
      }
    }
    if (crawl) {
      const successCount = sections.length;
      sections.push('\n---\n\n## 抓取统计\n\n成功 ' + successCount + ' 页，失败 ' + failures.length + ' 页。' +
        (failures.length ? '\n\n' + failures.map(x => '- ' + x.url + '：' + x.error).join('\n') : ''));
    }
    const content = sections.join('\n\n---\n\n');
    setWebProgress(100, '网页转换完成', (crawl ? sections.length - 1 : sections.length) + ' 页');
    setWebStatus('提取成功' + (warnings.length ? '，有 ' + warnings.length + ' 条提示' : '') + '。', 'success');
    const title = (first.meta && first.meta.title) || url;
    await renderVirtual('url', title, first.asset_dir || '', content, [], { assets });
    if (warnings.length) showToast(warnings[0] + (warnings.length > 1 ? '（另有 ' + (warnings.length - 1) + ' 条）' : ''));
  } catch (error) {
    const cancelled = error.code === 'cancelled' || webRun.cancelled;
    setWebStatus(cancelled ? '网页转换已取消。' : (error.message || '网页转换失败'), cancelled ? '' : 'error');
    setWebProgress(0, cancelled ? '已取消' : '转换未完成', '');
  } finally {
    setWebRunning(false);
  }
}

/* ---------------- 文件选择（含浏览器兜底） ---------------- */

function chooseFile(mode) {
  if (moduleBlocked(mode)) return;
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
    const r = await apiFetch('/api/upload?ext=' + encodeURIComponent(ext), { method: 'POST', body: file });
    const d = await r.json();
    return d.path || null;
  } catch (e) { showToast('上传失败'); return null; }
}

function convertOrOcr(p, mode) {
  if (mode === 'ocr' || IMG_RE.test(p) || /\.pdf$/i.test(p)) ocrFile(p);
  else convertFile(p);
}

/* ---------------- AI 助手 ---------------- */

const AI_ACTIONS = {
  quick_read: '快速阅读', polish: '润色', modify: '修改',
  expand: '扩充', continue: '续写', translate: '翻译', ask: '提问',
};

const AI_SYSTEM = {
  quick_read: '你是 ReadMD 的文档阅读助手。对用户给出的 Markdown 文档做快速阅读，输出：1) 一句话概述；2) 核心要点列表；3) 文档结构目录；4) 值得注意的细节或疑问。使用 Markdown 格式。',
  polish: '你是资深中文编辑。润色用户给出的 Markdown 文档：修正错别字、病句、表达生硬之处，保留原有结构与全部 Markdown 标记，只输出润色后的完整文档，不要加任何解释。',
  modify: '你是文档修订助手。根据用户要求修改文档，修正明显错误（错别字、标点、Markdown 格式错误）。只输出修改后的完整文档，不要加任何解释。',
  expand: '你是文档扩充助手。在保持原有结构与语气的前提下，为文档补充细节、示例、解释，使内容更丰富。只输出扩充后的完整文档，不要加任何解释。',
  continue: '你是文档续写助手。从文档末尾自然延续写作，保持风格一致。只输出续写的新增内容，不要重复原文。',
  translate: '你是专业翻译。将用户给出的文档翻译成指定语言，保留 Markdown 结构、表格与代码块，只输出译文。',
  ask: '你是文档问答助手。基于用户给出的文档内容回答问题；文档中没有的内容请明确说明。',
};

function toggleAiPanel() {
  if (moduleBlocked('ai')) return;
  const p = $('ai-panel');
  p.classList.toggle('hidden');
  if (!p.classList.contains('hidden')) {
    updateAiUsage();
    if (!state.ai.config) loadAiConfig();
    else { loadAiPrompts(); loadAiSessions(); }
  }
}
/* ---------------- Prompt 模板 ---------------- */

async function loadAiPrompts() {
  try {
    const r = await apiFetch('/api/ai/prompts');
    if (!r.ok) return;
    state.ai.templates = (await r.json()).templates || [];
    fillAiTemplates();
  } catch (e) { /* ignore */ }
}

function fillAiTemplates() {
  const sel = $('ai-template');
  if (!sel) return;
  const cur = state.ai.templateId;
  sel.innerHTML = '';
  const none = document.createElement('option');
  none.value = '';
  none.textContent = '默认动作（不使用模板）';
  sel.appendChild(none);
  (state.ai.templates || []).forEach(t => {
    const o = document.createElement('option');
    o.value = t.id;
    o.textContent = (t.builtin ? '◆ ' : '◇ ') + t.name;
    sel.appendChild(o);
  });
  if (cur && [...sel.options].some(o => o.value === cur)) sel.value = cur;
  else state.ai.templateId = '';
}

function currentAiTemplate() {
  const id = $('ai-template').value;
  return (state.ai.templates || []).find(t => t.id === id) || null;
}

function onAiTemplateChange() {
  const t = currentAiTemplate();
  state.ai.templateId = t ? t.id : '';
  document.querySelectorAll('.ai-act').forEach(b => {
    b.classList.toggle('active', !!(t && t.action && t.action !== 'custom' && b.dataset.act === t.action));
  });
  if (t && t.action === 'translate') $('ai-prompt').placeholder = '翻译：目标语言（如：英语 / 日语）';
  else $('ai-prompt').placeholder = '补充要求 / 提问内容 / 翻译目标语言（可选）';
}

function openTplModal() {
  $('tpl-modal').classList.remove('hidden');
  if (!state.ai.templates.length) loadAiPrompts();
  renderTplList();
  selectTpl(null);
}

function renderTplList() {
  const list = $('tpl-list');
  list.innerHTML = '';
  (state.ai.templates || []).forEach(t => {
    const li = document.createElement('li');
    li.textContent = (t.builtin ? '◆ ' : '◇ ') + t.name;
    li.dataset.id = t.id;
    li.title = '动作：' + (t.action || 'custom') + (t.user ? ' · 含用户消息模板' : '');
    li.addEventListener('click', () => selectTpl(t.id));
    list.appendChild(li);
  });
}

function selectTpl(id) {
  const t = (state.ai.templates || []).find(x => x.id === id) || null;
  document.querySelectorAll('#tpl-list li').forEach(li => li.classList.toggle('active', li.dataset.id === id));
  $('tpl-id').value = t ? t.id : '';
  $('tpl-name').value = t ? t.name : '';
  $('tpl-action').value = (t && t.action) || 'custom';
  $('tpl-system').value = t ? (t.system || '') : '';
  $('tpl-user').value = t ? (t.user || '') : '';
  $('tpl-del').disabled = !t;
}

async function saveTplForm() {
  const t = {
    id: $('tpl-id').value || undefined,
    name: $('tpl-name').value.trim(),
    action: $('tpl-action').value,
    system: $('tpl-system').value,
    user: $('tpl-user').value,
  };
  if (!t.name) { showToast('请填写模板名称'); return; }
  try {
    const r = await apiFetch('/api/ai/prompts', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'save', template: t }),
    });
    const d = await r.json();
    if (!r.ok || !d.ok) throw new Error(d.error || '保存失败');
    await loadAiPrompts();
    renderTplList();
    const saved = d.template || {};
    selectTpl(saved.id);
    $('ai-template').value = saved.id;
    onAiTemplateChange();
    showToast('模板已保存');
  } catch (e) { showToast('保存失败：' + e.message); }
}

async function deleteTplForm() {
  const id = $('tpl-id').value;
  if (!id) return;
  const t = (state.ai.templates || []).find(x => x.id === id);
  const isBuiltin = t && t.builtin;
  try {
    const r = await apiFetch('/api/ai/prompts', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'delete', id }),
    });
    const d = await r.json();
    if (!r.ok || !d.ok) throw new Error(d.error || '删除失败');
    await loadAiPrompts();
    renderTplList();
    selectTpl(null);
    showToast(isBuiltin ? '已恢复内置模板默认' : '模板已删除');
  } catch (e) { showToast('删除失败：' + e.message); }
}

/* ---------------- AI 历史会话 ---------------- */

async function loadAiSessions() {
  try {
    const r = await apiFetch('/api/ai/history');
    if (!r.ok) return;
    state.ai.sessions = (await r.json()).sessions || [];
    fillAiSessions();
  } catch (e) { /* ignore */ }
}

function fillAiSessions() {
  const sel = $('ai-session');
  if (!sel) return;
  sel.innerHTML = '';
  const fresh = document.createElement('option');
  fresh.value = '';
  fresh.textContent = '＋ 新会话（不加载）';
  sel.appendChild(fresh);
  (state.ai.sessions || []).forEach(s => {
    const o = document.createElement('option');
    o.value = s.id;
    o.textContent = (s.title || '未命名会话').slice(0, 22) + ' · ' + fmtTime(s.updated) + ' · ' + (s.msgCount || 0) + ' 条';
    sel.appendChild(o);
  });
  sel.value = state.ai.sessionId || '';
}

function fmtTime(ts) {
  if (!ts) return '';
  const d = new Date(ts * 1000);
  const p = n => String(n).padStart(2, '0');
  return d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate()) + ' ' + p(d.getHours()) + ':' + p(d.getMinutes());
}

async function onAiSessionChange() {
  const id = $('ai-session').value;
  if (!id) return;
  try {
    const r = await apiFetch('/api/ai/history?id=' + encodeURIComponent(id));
    if (!r.ok) { showToast('加载会话失败'); return; }
    const s = (await r.json()).session;
    if (!s) { showToast('会话不存在'); return; }
    const savedProvider = (state.ai.providers || []).find(p => p.id === s.provider || p.name === s.provider);
    if (savedProvider) {
      $('ai-provider').value = savedProvider.id;
      onAiProviderChange();
      if (s.model) $('ai-model').value = s.model;
      syncAiKey();
    }
    state.ai.messages = s.messages || [];
    state.ai.sessionId = s.id;
    state.ai.raw = '';
    state.ai.usage = null;
    state.ai.sessUsage = s.usage || { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
    updateAiUsage();
    renderAiHistory();
    showToast('已加载会话');
  } catch (e) { showToast('加载会话失败'); }
}

function renderAiHistory() {
  const out = $('ai-output');
  out.innerHTML = '';
  const msgs = state.ai.messages || [];
  let uSeq = 0, aSeq = 0;
  msgs.forEach((m, i) => {
    if (m.role === 'user') { uSeq++;
      const ub = document.createElement('div');
      ub.className = 'ai-msg user';
      const tag = document.createElement('div');
      tag.className = 'ai-msg-tag';
      tag.textContent = '我 · 提问 ' + uSeq;
      const body = document.createElement('div');
      body.className = 'ai-msg-body';
      body.textContent = m.content.length > 3000 ? m.content.slice(0, 3000) + '\n…（已省略）' : m.content;
      ub.appendChild(tag); ub.appendChild(body);
      out.appendChild(ub);
    } else if (m.role === 'assistant' && m.content) { aSeq++;
      const ab = document.createElement('div');
      ab.className = 'ai-msg ai';
      const tag = document.createElement('div');
      tag.className = 'ai-msg-tag';
      tag.textContent = 'AI · 回答 ' + aSeq + (m.model ? ' · ' + m.model : '') + fmtAiUsage(m.usage);
      const body = document.createElement('div');
      body.className = 'ai-msg-body';
      const prot = protectMath(m.content);
      body.innerHTML = restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved);
      ab.appendChild(tag); ab.appendChild(body);
      out.appendChild(ab);
    }
  });
  out.scrollTop = out.scrollHeight;
  const last = msgs[msgs.length - 1];
  if (last && last.role === 'assistant') state.ai.raw = last.content || '';
  updateAiRawButtons();
}

async function saveCurrentSession() {
  const msgs = state.ai.messages || [];
  if (!msgs.length) { showToast('当前没有对话内容'); return; }
  const title = ($('ai-prompt').value.trim() || msgs[0].content || '未命名会话').slice(0, 40).replace(/\s+/g, ' ');
  const sess = {
    id: state.ai.sessionId || undefined,
    title: title,
    provider: $('ai-provider').value,
    model: $('ai-model').value,
    doc: state.mode === 'file' ? (state.file || '') : (state.sourceName || ''),
    messages: msgs,
    usage: state.ai.sessUsage,
  };
  try {
    const r = await apiFetch('/api/ai/history', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'save', session: sess }),
    });
    const d = await r.json();
    if (!r.ok || !d.ok) throw new Error(d.error || '保存失败');
    state.ai.sessionId = d.session.id;
    await loadAiSessions();
    $('ai-session').value = state.ai.sessionId;
    showToast('会话已保存');
  } catch (e) { showToast('保存失败：' + e.message); }
}

async function deleteCurrentSession() {
  const id = $('ai-session').value;
  if (!id) return;
  try {
    const r = await apiFetch('/api/ai/history', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'delete', id }),
    });
    const d = await r.json();
    if (!r.ok || !d.ok) throw new Error(d.error || '删除失败');
    if (state.ai.sessionId === id) { state.ai.sessionId = null; state.ai.messages = []; clearAiOutput(); }
    await loadAiSessions();
    showToast('会话已删除');
  } catch (e) { showToast('删除失败：' + e.message); }
}

function clearAiContext() {
  if (!(state.ai.messages || []).length) { showToast('当前没有上下文'); return; }
  state.ai.messages = [];
  state.ai.sessionId = null;
  state.ai.raw = '';
  state.ai.usage = null;
  state.ai.sessUsage = { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
  updateAiUsage();
  clearAiOutput();
  $('ai-session').value = '';
  showToast('已清空上下文，开始新一轮');
}


function clearAiOutput() {
  state.ai.raw = '';
  state.ai.aborter = null;
  const out = $('ai-output');
  if (out) out.innerHTML = '';
  updateAiRawButtons();
}

async function loadAiConfig() {
  for (let attempt = 0; attempt < 25; attempt++) {
  try {
    const r = await apiFetch('/api/ai/config');
    if (r.status === 409) { await new Promise(r2 => setTimeout(r2, 800)); continue; }
    if (!r.ok) return null;
    state.ai.config = await r.json();
    const cfg = state.ai.config;
    state.ai.providers = mergeAiProviders(cfg.custom || [], cfg.presets || []);
    fillAiProviders(state.ai.providers, cfg.current || {});
    loadAiPrompts();
    loadAiSessions();
    return cfg;
  } catch (e) { /* ignore */ return null; }
  }
  return null;
}

function mergeAiProviders(custom, presets) {
  return [...(custom || []), ...(presets || [])];
}

function fillAiProviders(merged, current) {
  const sel = $('ai-provider');
  const curId = (current && (current.provider_id || current.provider)) || (merged[0] && merged[0].id) || '';
  sel.innerHTML = '';
  const customGroup = document.createElement('optgroup'); customGroup.label = '自定义连接';
  const presetGroup = document.createElement('optgroup'); presetGroup.label = '官方预设';
  merged.forEach(p => {
    const o = document.createElement('option');
    o.value = p.id;
    o.textContent = p.name;
    (p.custom ? customGroup : presetGroup).appendChild(o);
  });
  if (customGroup.children.length) sel.appendChild(customGroup);
  if (presetGroup.children.length) sel.appendChild(presetGroup);
  if (curId) sel.value = curId;
  onAiProviderChange();
}

function currentAiProvider() {
  const id = $('ai-provider').value;
  return (state.ai.providers || []).find(p => p.id === id || p.name === id) || null;
}

function aiPresetBase(p) {
  return (p && p.base_url) || '';
}

function fillAiModels(models, selected) {
  const sel = $('ai-model');
  sel.innerHTML = '';
  const list = Array.isArray(models) ? models.filter(Boolean) : [];
  const placeholder = new Option(list.length ? '选择模型' : '请先获取模型', '');
  placeholder.disabled = true; placeholder.selected = !list.length;
  sel.appendChild(placeholder);
  list.forEach(id => sel.appendChild(new Option(id, id)));
  sel.disabled = !list.length;
  if (list.length) sel.value = list.indexOf(selected) >= 0 ? selected : list[0];
}

function onAiProviderChange() {
  const p = currentAiProvider();
  if (!p) { fillAiModels([], ''); syncAiKey(); return; }
  const base = aiPresetBase(p);
  $('ai-base-url').value = base;
  const mode = p.mode || (p.format === 'anthropic' ? 'messages' : 'auto');
  $('ai-mode').value = (mode === 'anthropic') ? 'messages' : mode;
  const current = (state.ai.config && state.ai.config.current) || {};
  fillAiModels(p.models, (current.provider_id || current.provider) === p.id ? current.model : '');
  $('ai-provider-name').value = p.name || '';
  $('ai-provider-name').disabled = !p.custom;
  $('ai-provider-delete').disabled = !p.custom;
  syncAiKey();
}

function syncAiKey() {
  const p = currentAiProvider();
  const inp = $('ai-key');
  const status = $('ai-conn-status');
  if (!p) { inp.value = ''; inp.placeholder = ''; if (status) status.textContent = ''; return; }
  // API Key 不会从后端回传；切换连接时也不保留前一个连接的输入值。
  inp.value = '';
  inp.placeholder = (p.key_source && p.key_source.indexOf('env:') === 0)
    ? '已从环境变量 ' + p.key_source.slice(4) + ' 读取，可覆盖'
    : (p.name.indexOf('Ollama') >= 0 ? 'API Key（本地 Ollama 可留空）' : 'API Key（必填）');
  if (status) {
    status.textContent = p.has_key
      ? (p.key_source ? 'Key 就绪（' + p.key_source + '）' : 'Key 已配置')
      : (p.name.indexOf('Ollama') >= 0 ? '本地模型无需 Key' : '未配置 Key');
  }
}

function newAiProvider() {
  if (!state.ai.config) return;
  const custom = state.ai.config.custom || (state.ai.config.custom = []);
  let seq = custom.length + 1;
  let name = '自定义连接 ' + seq;
  while ((state.ai.providers || []).some(p => p.name === name)) name = '自定义连接 ' + (++seq);
  const uid = (crypto.randomUUID ? crypto.randomUUID().replace(/-/g, '') : String(Date.now()) + Math.random().toString(16).slice(2));
  const p = { id: 'custom:' + uid, name, custom: true, base_url: '', format: 'openai', mode: 'auto', models: [] };
  custom.push(p);
  state.ai.providers = mergeAiProviders(custom, state.ai.config.presets || []);
  fillAiProviders(state.ai.providers, { provider_id: p.id, model: '' });
  $('ai-provider-name').focus(); $('ai-provider-name').select();
}

async function deleteAiProvider() {
  const p = currentAiProvider();
  if (!p || !p.custom || !state.ai.config) return;
  if (!window.confirm('删除自定义连接“' + p.name + '”？此操作不会影响官方预设。')) return;
  const custom = (state.ai.config.custom || []).filter(c => c.id !== p.id);
  const fallback = (state.ai.config.presets || [])[0] || custom[0] || {};
  const current = { provider_id: fallback.id || '', model: '' };
  try {
    const r = await apiFetch('/api/ai/config', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ providers: custom, current }),
    });
    if (!r.ok) throw new Error((await r.json().catch(() => ({}))).error || '保存失败');
    state.ai.config.custom = custom; state.ai.config.current = current;
    state.ai.providers = mergeAiProviders(custom, state.ai.config.presets || []);
    fillAiProviders(state.ai.providers, current);
    showToast('已删除自定义连接');
  } catch (e) { showToast('删除失败：' + e.message); }
}

async function saveAiSelection(silent) {
  const p = currentAiProvider();
  if (!p || !state.ai.config) return;
  const custom = (state.ai.config.custom || []).map(c => Object.assign({}, c));
  const keyVal = $('ai-key').value.trim();
  const baseUrl = $('ai-base-url').value.trim();
  const mode = $('ai-mode').value || 'auto';
  const requestedName = $('ai-provider-name').value.trim() || p.name;
  if (p.custom && requestedName !== p.name && custom.some(c => c.name === requestedName)) {
    showToast('自定义连接名称已存在'); return;
  }
  let over = custom.find(c => c.id === p.id);
  if (!over) {
    over = Object.assign({}, p);
    delete over.has_key; delete over.key_source;
    if (!String(over.id || '').startsWith('custom:')) {
      const uid = (crypto.randomUUID ? crypto.randomUUID().replace(/-/g, '') : String(Date.now()) + Math.random().toString(16).slice(2));
      over.id = 'custom:' + uid;
    }
    over.custom = true;
    custom.push(over);
  }
  if (p.custom && requestedName !== p.name) {
    over.name = requestedName;
  }
  if (baseUrl) over.base_url = baseUrl;
  else delete over.base_url;
  over.mode = mode;
  if (mode === 'messages') over.format = 'anthropic';
  else over.format = 'openai';
  if (keyVal) over.api_key = keyVal;
  over.models = Array.from($('ai-model').options).map(o => o.value).filter(Boolean);
  if (p.clear_key) over.clear_key = true;
  const current = { provider_id: over.id, model: $('ai-model').value || '' };
  try {
    const r = await apiFetch('/api/ai/config', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ providers: custom, current }),
    });
    if (r.ok) {
      await loadAiConfig();
      const status = $('ai-conn-status');
      if (status) status.textContent = '已保存✓';
      if (!silent) showToast('连接设置已保存');
    } else {
      const d = await r.json().catch(() => ({}));
      throw new Error(d.error || 'HTTP ' + r.status);
    }
  } catch (e) {
    showToast('保存失败：' + e.message);
  }
}

function getAiTargetText() {
  let sel = '';
  if ($('ai-selection').checked) {
    sel = ((window.getSelection && window.getSelection()) || {}).toString() || '';
    if (!sel) showToast('未选中文字，将处理全文');
  }
  if (sel) return { text: sel, isSelection: true };
  const src = state.mode === 'file'
    ? (state.original || state.fixed || '')
    : (state.fixed || state.original || '');
  return { text: src, isSelection: false };
}

function setAiBusy(b) {
  state.ai.busy = b;
  $('ai-run').disabled = b;
  $('ai-stop').disabled = !b;
  $('ai-status').textContent = b ? '生成中…' : '';
}

function updateAiRawButtons() {
  const has = !!state.ai.raw;
  $('ai-apply').disabled = !has;
  $('ai-copy').disabled = !has;
  $('ai-saveas').disabled = !has;
}

async function loadAiModels() {
  const baseUrl = $('ai-base-url').value.trim();
  const key = $('ai-key').value.trim();
  const mode = $('ai-mode').value || 'auto';
  if (!baseUrl) { showToast('请先填写 Base URL'); return; }
  const p = currentAiProvider();
  const local = p && p.name.indexOf('Ollama') >= 0;
  if (!local && !key && !(p && p.has_key)) { showToast('请先填写 API Key'); return; }
  const btn = $('ai-models-btn');
  const old = btn.textContent;
  btn.disabled = true; btn.textContent = '获取中…';
  const status = $('ai-conn-status');
  try {
    const q = new URLSearchParams({ provider: (p && p.id) || '', base_url: baseUrl, key: key, mode: mode });
    const r = await apiFetch('/api/ai/models?' + q.toString());
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.error || ('HTTP ' + r.status));
    const ids = d.models || [];
    if (ids.length) {
      p.models = ids;
      fillAiModels(ids, $('ai-model').value);
      await saveAiSelection(true);
      if (status) status.textContent = '获取到 ' + ids.length + ' 个模型✓';
      showToast('已获取 ' + ids.length + ' 个模型');
    } else {
      fillAiModels([], '');
      if (status) status.textContent = '接口未返回可选模型';
      showToast('接口未返回可选模型');
    }
  } catch (e) {
    if (status) status.textContent = '获取失败';
    showToast('获取模型失败：' + e.message);
  } finally {
    btn.disabled = false;
    btn.textContent = old;
  }
}

function toggleAiKey() {
  const inp = $('ai-key');
  inp.type = (inp.type === 'password') ? 'text' : 'password';
  $('ai-key-toggle').title = inp.type === 'password' ? '显示 / 隐藏' : '隐藏';
}

function clearAiKey() {
  const p = currentAiProvider();
  if (!p) return;
  p.clear_key = true;
  p.has_key = false;
  $('ai-key').value = '';
  $('ai-conn-status').textContent = '保存后清除已存 Key';
}

function resetAiUrl() {
  const p = currentAiProvider();
  if (!p) return;
  $('ai-base-url').value = p.base_url || '';
  const mode = p.mode || (p.format === 'anthropic' ? 'messages' : 'auto');
  $('ai-mode').value = (mode === 'anthropic') ? 'messages' : mode;
  showToast('已恢复预设地址');
}

function updateAiUsage() {
  const el = $('ai-usage');
  if (!el) return;
  const u = state.ai.usage;
  const s = state.ai.sessUsage;
  const fmt = n => (n == null ? 0 : n);
  el.textContent = '本次 ' + fmt(u && u.prompt_tokens) + '/' + fmt(u && u.completion_tokens) + '/' + fmt(u && u.total_tokens)
    + ' · 会话累计 ' + fmt(s.prompt_tokens) + '/' + fmt(s.completion_tokens) + '/' + fmt(s.total_tokens);
}

async function runAi(action) {
  const p = currentAiProvider();
  if (!p) { showToast('请先选择 AI 提供商'); return; }
  const keyVal = $('ai-key').value.trim();
  const local = p.name.indexOf('Ollama') >= 0;
  if (!local && !keyVal) { showToast('未配置 API Key（请填写后重试）'); return; }
  const { text, isSelection } = getAiTargetText();
  if (!text || !text.trim()) { showToast('没有可处理的文档内容'); return; }
  const prompt = $('ai-prompt').value.trim();
  const model = $('ai-model').value.trim() || (p.models || [''])[0] || '';
  const mode = $('ai-mode').value || 'auto';
  const baseUrl = $('ai-base-url').value.trim();
  const stream = $('ai-stream').checked;
  saveAiSelection();

  const tpl = currentAiTemplate();
  let sys = (tpl && tpl.system) || AI_SYSTEM[action] || '你是 ReadMD 的文档助手。';
  if (action === 'translate' && prompt && !(tpl && tpl.system)) {
    sys = '你是专业翻译。将用户给出的文档翻译成「' + prompt + '」，保留 Markdown 结构、表格与代码块，只输出译文。';
  }
  const docs = text.length > 120000 ? text.slice(0, 120000) + '\n\n[内容过长已截断，请分段处理]' : text;
  const fill = s => String(s || '').replace(/\{doc\}/g, docs).replace(/\{prompt\}/g, prompt || '');
  let userMsg;
  if (tpl && tpl.user) {
    userMsg = fill(tpl.user);
  } else if (action === 'ask' && prompt) userMsg = '文档如下：\n\n' + docs + '\n\n问题：' + prompt;
  else if (action === 'modify' && prompt) userMsg = '文档如下：\n\n' + docs + '\n\n修改要求：' + prompt;
  else if (prompt) userMsg = '文档如下：\n\n' + docs + '\n\n补充要求：' + prompt;
  else userMsg = '文档如下：\n\n' + docs;

  const msgs = (state.ai.messages || []).slice(-40);
  msgs.push({ role: 'user', content: userMsg });

  const out = $('ai-output');
  const userBubble = document.createElement('div');
  userBubble.className = 'ai-msg user';
  const uTag = document.createElement('div');
  uTag.className = 'ai-msg-tag';
  const userSeq = (state.ai.messages || []).filter(m => m.role === 'user').length + 1;
  uTag.textContent = '我 · 提问 ' + userSeq + ' · ' + (AI_ACTIONS[action] || action) + (isSelection ? '（选中文字）' : '（全文）') + ' · ' + model;
  const uBody = document.createElement('div');
  uBody.className = 'ai-msg-body';
  uBody.textContent = userMsg.length > 2000 ? userMsg.slice(0, 2000) + '\n…（文档内容较长已省略）' : userMsg;
  userBubble.appendChild(uTag); userBubble.appendChild(uBody);
  out.appendChild(userBubble);

  const aiBubble = document.createElement('div');
  aiBubble.className = 'ai-msg ai';
  const aiTag = document.createElement('div');
  aiTag.className = 'ai-msg-tag';
  aiTag.textContent = 'AI 生成中…';
  const aiBody = document.createElement('div');
  aiBody.className = 'ai-msg-body';
  aiBubble.appendChild(aiTag); aiBubble.appendChild(aiBody);
  out.appendChild(aiBubble);
  out.scrollTop = out.scrollHeight;

  state.ai.raw = '';
  updateAiRawButtons();
  setAiBusy(true);
  const ctrl = new AbortController();
  state.ai.aborter = ctrl;
  let renderTimer = null;
  const render = () => {
    renderTimer = null;
    if (!state.ai.raw) return;
    const prot = protectMath(state.ai.raw);
    const html = marked.parse(prot.src, { gfm: true, breaks: false });
    aiBody.innerHTML = restoreMath(html, prot.saved);
    out.scrollTop = out.scrollHeight;
  };
  try {
    const r = await apiFetch('/api/ai/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        provider: p.id, model: model, api_key: keyVal || undefined,
        base_url: baseUrl || undefined, mode: mode, stream: stream,
        messages: [{ role: 'system', content: sys }].concat(msgs),
        temperature: 0.7,
      }),
      signal: ctrl.signal,
    });
    if (!r.ok) {
      const d = await r.json().catch(() => ({}));
      throw new Error(d.error || ('HTTP ' + r.status));
    }
    const reader = r.body.getReader();
    const dec = new TextDecoder('utf-8');
    let buf = '';
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      let idx;
      while ((idx = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, idx).trim();
        buf = buf.slice(idx + 1);
        if (line.indexOf('data:') !== 0) continue;
        const data = line.slice(5).trim();
        if (!data) continue;
        let obj;
        try { obj = JSON.parse(data); } catch (e) { continue; }
        if (obj.error) throw new Error(obj.error);
        if (obj.done) break;
        if (obj.usage) {
          state.ai.usage = obj.usage;
          const s = state.ai.sessUsage;
          s.prompt_tokens += obj.usage.prompt_tokens || 0;
          s.completion_tokens += obj.usage.completion_tokens || 0;
          s.total_tokens += obj.usage.total_tokens || 0;
          updateAiUsage();
          continue;
        }
        if (obj.d === undefined) continue;
        state.ai.raw += obj.d;
        if (!renderTimer) renderTimer = setTimeout(render, state.ai.raw.length > 150000 ? 500 : 120);
      }
    }
    if (renderTimer) { clearTimeout(renderTimer); renderTimer = null; render(); }
    renderMath(aiBody);
    aiTag.textContent = 'AI · 回答 ' + userSeq + ' · ' + model + fmtAiUsage(state.ai.usage);
    if (state.ai.raw) {
      const last = { role: 'assistant', content: state.ai.raw };
      if (state.ai.usage) last.usage = state.ai.usage;
      msgs.push(last);
      state.ai.messages = msgs;
      state.ai.sessionId = null;
      $('ai-session').value = '';
      updateAiRawButtons();
      showToast('AI 完成（可点“存”保存会话）');
    } else {
      msgs.pop();
    }
  } catch (e) {
    if (e.name === 'AbortError') {
      aiTag.textContent = 'AI · 回答 ' + userSeq + '（已停止）';
      if (state.ai.raw) {
        const last = { role: 'assistant', content: state.ai.raw };
        if (state.ai.usage) last.usage = state.ai.usage;
        msgs.push(last);
        state.ai.messages = msgs;
      }
      showToast('已停止');
    } else {
      aiTag.textContent = 'AI · 出错';
      showToast('AI 出错：' + e.message);
      aiBody.innerHTML = '<p class="ai-err">' + String(e.message).replace(/[<>&]/g, c => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' }[c])) + '</p>';
    }
  } finally {
    setAiBusy(false);
    state.ai.aborter = null;
  }
}

function fmtAiUsage(u) {
  if (!u) return '';
  const t = u.total_tokens != null ? u.total_tokens : ((u.prompt_tokens || 0) + (u.completion_tokens || 0));
  return t ? ' · ' + t + ' tokens' : '';
}

async function copyAi() {
  if (!state.ai.raw) return;
  try {
    await navigator.clipboard.writeText(state.ai.raw);
    showToast('已复制');
  } catch (e) {
    const ta = document.createElement('textarea');
    ta.value = state.ai.raw;
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand('copy'); showToast('已复制'); } catch (e2) { showToast('复制失败'); }
    ta.remove();
  }
}

async function applyAi() {
  if (!state.ai.raw) return;
  const selOnly = $('ai-selection').checked;
  if (state.mode === 'file') {
    let next = state.ai.raw;
    if (selOnly) {
      const sel = ((window.getSelection && window.getSelection()) || {}).toString() || '';
      const cur = state.original || state.fixed || '';
      const i = sel ? cur.indexOf(sel) : -1;
      if (i >= 0) next = cur.slice(0, i) + state.ai.raw + cur.slice(i + sel.length);
      else { showToast('未定位到选中文字，已改为全文应用'); }
    }
    state.original = next;
    state.fixed = next;
    exitEdit();
    await toggleEdit();
    showToast('已应用，请检查后 Ctrl+S 保存（首存自动备份）');
  } else {
    state.fixed = state.ai.raw;
    state.original = state.ai.raw;
    renderContent(state.ai.raw, (state.sourceName || 'AI 结果') + ' · AI');
    updateStatus();
    showToast('已应用（虚拟文档），可另存为 .md');
  }
}

async function saveAiAs() {
  if (!state.ai.raw) return;
  const base = (state.sourceName || state.file || 'document').replace(/[\\/]/g, '_');
  const suggested = base.replace(/\.[^.]+$/, '') + '.ai.md';
  if (hasPy) {
    const out = await py.save_as(state.ai.raw, suggested);
    if (out) showToast('已保存：' + out);
  } else {
    const blob = new Blob([state.ai.raw], { type: 'text/markdown;charset=utf-8' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = suggested;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 3000);
  }
}

/* ---------------- 移动端共享 ---------------- */

async function openShareModal() {
  $('share-modal').classList.remove('hidden');
  refreshShareStatus();
}

async function refreshShareStatus() {
  try {
    const r = await apiFetch('/api/share/status');
    const d = await r.json();
    if (d.running) {
      $('share-start').disabled = true;
      $('share-stop').disabled = false;
      $('share-url').textContent = '手机浏览器打开：' + d.url;
      $('share-token').textContent = '访问令牌：' + d.token;
      renderQr(d.url);
    } else {
      $('share-start').disabled = false;
      $('share-stop').disabled = true;
      $('share-url').textContent = '';
      $('share-token').textContent = '';
      const q = $('share-qr');
      q.innerHTML = '<p class="fix-note">尚未开启共享</p>';
    }
  } catch (e) { /* ignore */ }
}

function renderQr(text) {
  const box = $('share-qr');
  box.innerHTML = '';
  try {
    if (typeof qrcode !== 'function') { box.textContent = text; return; }
    const qr = qrcode(0, 'M');
    qr.addData(text);
    qr.make();
    box.innerHTML = qr.createImgTag(6, 10);
  } catch (e) {
    box.textContent = text;
  }
}

async function startShare() {
  try {
    const r = await apiFetch('/api/share/start', { method: 'POST' });
    const d = await r.json();
    if (d.error) { showToast(d.error); return; }
    showToast('共享已开启');
  } catch (e) { showToast('开启失败：' + e.message); }
  refreshShareStatus();
}

async function stopShare() {
  try {
    await apiFetch('/api/share/stop', { method: 'POST' });
    showToast('共享已关闭');
  } catch (e) { showToast('关闭失败：' + e.message); }
  refreshShareStatus();
}
/* ---------------- 编辑模式（CodeMirror 6：自动补全 + 语法引用） ---------------- */

let cmView = null;
let cmReady = false;
let cmLoading = false;
let cmThemeCompartment = null;

function loadCodeMirror() {
  return new Promise((resolve, reject) => {
    if (window.ReadMDCodeMirror) { cmReady = true; resolve(); return; }
    if (cmLoading) {
      const t0 = Date.now();
      const iv = setInterval(() => {
        if (window.ReadMDCodeMirror) { clearInterval(iv); cmReady = true; cmLoading = false; resolve(); }
        else if (Date.now() - t0 > 15000) { clearInterval(iv); cmLoading = false; reject(new Error('编辑器组件加载超时')); }
      }, 100);
      return;
    }
    cmLoading = true;
    const s = document.createElement('script');
    s.src = '/assets/vendor/codemirror.bundle.js';
    s.onload = () => { cmReady = true; cmLoading = false; resolve(); };
    s.onerror = () => { cmLoading = false; reject(new Error('编辑器组件加载失败，已退回基础编辑')); };
    document.head.appendChild(s);
  });
}

function createEditor(doc) {
  destroyEditor();
  if (!window.ReadMDCodeMirror) return false;
  const CM = window.ReadMDCodeMirror;
  const dark = document.body.dataset.theme === 'dark';
  cmThemeCompartment = new CM.Compartment();
  const st = CM.EditorState.create({
    doc: doc,
    extensions: [
      CM.lineNumbers(),
      CM.highlightActiveLineGutter(),
      CM.highlightActiveLine(),
      CM.drawSelection(),
      CM.dropCursor(),
      CM.bracketMatching(),
      CM.indentOnInput(),
      CM.foldGutter(),
      CM.syntaxHighlighting(CM.defaultHighlightStyle, { fallback: true }),
      CM.history(),
      CM.markdown({ base: CM.markdownLanguage, codeLanguages: CM.languages }),
      CM.autocompletion({ override: [cmMarkdownCompletions()], activateOnTyping: true }),
      CM.closeBrackets(),
      CM.keymap.of([CM.indentWithTab, ...CM.closeBracketsKeymap, ...CM.defaultKeymap, ...CM.historyKeymap, ...CM.completionKeymap]),
      cmThemeCompartment.of(dark ? CM.oneDark : []),
      CM.EditorView.lineWrapping,
      CM.EditorView.updateListener.of(u => { if (u.docChanged) schedulePreview(); }),
    ],
  });
  cmView = new CM.EditorView({ state: st, parent: $('edit-cm') });
  cmView.dom.addEventListener('keydown', e => {
    if (e.key !== '/' || e.ctrlKey || e.metaKey || e.altKey) return;
    const pos = cmView.state.selection.main.head;
    const line = cmView.state.doc.lineAt(pos);
    if (!cmView.state.sliceDoc(line.from, pos).trim()) { e.preventDefault(); openMdCommandPalette(); }
  });
  cmView.focus();
  return true;
}

function destroyEditor() {
  if (cmView) {
    try { cmView.destroy(); } catch (e) { /* ignore */ }
    cmView = null;
  }
  const c = $('edit-cm');
  if (c) c.innerHTML = '';
  cmThemeCompartment = null;
}

function applyCmTheme() {
  if (!cmView || !window.ReadMDCodeMirror || !cmThemeCompartment) return;
  const CM = window.ReadMDCodeMirror;
  const dark = document.body.dataset.theme === 'dark';
  cmView.dispatch({ effects: cmThemeCompartment.reconfigure(dark ? CM.oneDark : []) });
}

/* Markdown 自动补全（基于 GitHub 开源 @codemirror/autocomplete） */
function cmMarkdownCompletions() {
  const CM = window.ReadMDCodeMirror;
  const item = (label, snippetText, detail, type) => ({
    label, detail, type, apply: CM.snippet(snippetText),
  });
  const ALL = [
    item("# 标题", "# ${标题}", "一级标题", "markdown"),
    item("## 标题", "## ${标题}", "二级标题", "markdown"),
    item("### 标题", "### ${标题}", "三级标题", "markdown"),
    item("#### 标题", "#### ${标题}", "四级标题", "markdown"),
    item("**加粗**", "**${文本}**", "加粗", "markdown"),
    item("*斜体*", "*${文本}*", "斜体", "markdown"),
    item("~~删除线~~", "~~${文本}~~", "删除线", "markdown"),
    item("`行内代码`", "`${代码}`", "行内代码", "markdown"),
    item("```代码块", "```\n${代码}\n```", "代码块", "markdown"),
    item("[链接文本](url)", "[${文本}](url)", "链接", "markdown"),
    item("![图片描述](url)", "![${描述}](url)", "图片", "markdown"),
    item("> 引用", "> ${引用}", "引用块", "markdown"),
    item("$公式$", "$x^2$", "行内公式", "markdown"),
    item("$$公式$$", "$$\n${公式}\n$$", "块级公式", "markdown"),
    item("| 表格 |", "| 列1 | 列2 |\n|---|---|\n| ${值} |  |", "表格", "markdown"),
    item("- 列表项", "- ${项目}", "无序列表", "markdown"),
    item("- [ ] 任务", "- [ ] ${任务}", "任务列表", "markdown"),
    item("--- 分隔线", "---", "分隔线", "markdown"),
  ];  return context => {
    const before = context.matchBefore(/[\w#*_`\[!>|\$~:]{0,8}/);
    if (!before) return null;
    const w = before.text.toLowerCase();
    const matched = ALL.filter(c => c.label.toLowerCase().startsWith(w) || c.label.toLowerCase().includes(w));
    if (!matched.length) return null;
    return { from: before.from, options: matched.slice(0, 12) };
  };
}

/* 语法引用 / 插入工具栏 */
function cmInsertSyntax(kind) {
  if (!cmView) return;
  const sel = cmView.state.selection.main;
  const selected = cmView.state.sliceDoc(sel.from, sel.to);
  let insert = null;
  let cursor = sel.from;
  const wrap = (b, d, a) => {
    insert = b + (selected || d) + a;
    cursor = sel.from + b.length + (selected || d).length;
  };
  switch (kind) {
    case 'bold': wrap('**', '文本', '**'); break;
    case 'italic': wrap('*', '文本', '*'); break;
    case 'strike': wrap('~~', '文本', '~~'); break;
    case 'code': wrap('`', '代码', '`'); break;
    case 'math': wrap('$', 'x^2', '$'); break;
    case 'mathblock': insert = '$$\n' + (selected || 'x^2') + '\n$$'; cursor = sel.from + insert.length - 3; break;
    case 'h2': insert = '## ' + (selected || '标题'); cursor = sel.from + insert.length; break;
    case 'quote': insert = '> ' + (selected || '引用'); cursor = sel.from + insert.length; break;
    case 'list': insert = '- ' + (selected || '项目'); cursor = sel.from + insert.length; break;
    case 'ordered': insert = '1. ' + (selected || '项目'); cursor = sel.from + insert.length; break;
    case 'task': insert = '- [ ] ' + (selected || '任务'); cursor = sel.from + insert.length; break;
    case 'link': insert = '[' + (selected || '文本') + '](url)'; cursor = sel.from + 1 + (selected || '文本').length; break;
    case 'image': insert = '![' + (selected || '描述') + '](url)'; cursor = sel.from + 2 + (selected || '描述').length; break;
    case 'codeblock': insert = '```\n' + (selected || '代码') + '\n```'; cursor = sel.from + 4 + (selected || '代码').length; break;
    case 'table': insert = '| 列1 | 列2 |\n|---|---|\n| ' + (selected || '值') + ' |  |'; cursor = sel.from + insert.length; break;
    case 'hr': insert = '\n---\n'; cursor = sel.from + insert.length; break;
    default: return;
  }
  if (insert === null) return;
  cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert }, selection: { anchor: cursor } });
  cmView.focus();
}

const MD_COMMANDS = [
  ['加粗', 'bold', '**文本**'], ['斜体', 'italic', '*文本*'], ['删除线', 'strike', '~~文本~~'],
  ['二级标题', 'h2', '## 标题'], ['引用', 'quote', '> 引用'], ['无序列表', 'list', '- 项目'],
  ['有序列表', 'ordered', '1. 项目'], ['任务列表', 'task', '- [ ] 任务'], ['链接', 'link', '[文本](url)'],
  ['图片', 'image', '本地图片或 URL'], ['行内代码', 'code', '`代码`'], ['代码块', 'codeblock', '```'],
  ['表格', 'table', '| 列1 | 列2 |'], ['分隔线', 'hr', '---'], ['行内公式', 'math', '$x^2$'], ['块级公式', 'mathblock', '$$…$$'],
];
let mdCommandIndex = 0;

function closeMdPopups() {
  document.querySelectorAll('.md-menu, .pv-menu').forEach(el => el.classList.add('hidden'));
  const trigger = $('pv-trigger'); if (trigger) trigger.setAttribute('aria-expanded', 'false');
}

function openMdCommandPalette() {
  if (!state.editing) return;
  closeMdPopups();
  $('md-command-modal').classList.remove('hidden');
  $('md-command-search').value = '';
  mdCommandIndex = 0; renderMdCommands();
  setTimeout(() => $('md-command-search').focus(), 0);
}

function closeMdCommandPalette() { $('md-command-modal').classList.add('hidden'); if (cmView) cmView.focus(); }

function renderMdCommands() {
  const q = $('md-command-search').value.trim().toLowerCase();
  const rows = MD_COMMANDS.filter(c => !q || (c[0] + ' ' + c[2]).toLowerCase().includes(q));
  mdCommandIndex = Math.max(0, Math.min(mdCommandIndex, rows.length - 1));
  const list = $('md-command-list'); list.innerHTML = '';
  rows.forEach((c, i) => {
    const b = document.createElement('button'); b.className = 'command-item' + (i === mdCommandIndex ? ' active' : '');
    b.innerHTML = '<span></span><small></small>'; b.querySelector('span').textContent = c[0]; b.querySelector('small').textContent = c[2];
    b.addEventListener('click', () => runMdCommand(c[1])); list.appendChild(b);
  });
}

function runMdCommand(kind) { closeMdCommandPalette(); if (kind === 'image') openImgModal(); else if (kind === 'math' || kind === 'mathblock') openFormulaModal(kind === 'mathblock' ? 'block' : 'inline'); else cmInsertSyntax(kind); }

const FORMULAS = [
  ['常用','平方根','sqrt root','\\sqrt{x}'], ['常用','分式','fraction frac','\\frac{a}{b}'], ['常用','幂与下标','power subscript','x^{n}_{i}'], ['常用','二次公式','quadratic','x=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}'],
  ['希腊','阿尔法','alpha','\\alpha'], ['希腊','贝塔','beta','\\beta'], ['希腊','伽马','gamma','\\gamma'], ['希腊','派','pi','\\pi'], ['希腊','西塔','theta','\\theta'], ['希腊','欧米伽','omega','\\omega'],
  ['运算','乘除','times divide','a\\times b\\div c'], ['运算','正负','plus minus pm','a\\pm b'], ['运算','点乘','dot','a\\cdot b'],
  ['关系','小于等于','less equal','a\\le b'], ['关系','大于等于','greater equal','a\\ge b'], ['关系','不等于','not equal','a\\ne b'], ['关系','约等于','approx','a\\approx b'],
  ['箭头','右箭头','right arrow','A\\rightarrow B'], ['箭头','双向箭头','leftright arrow','A\\leftrightarrow B'], ['箭头','推出','implies','A\\Rightarrow B'],
  ['函数','正弦','sin','\\sin x'], ['函数','对数','log','\\log_{a}x'], ['函数','指数','exp','e^{x}'],
  ['结构','求和','sum','\\sum_{i=1}^{n} x_i'], ['结构','积分','integral','\\int_{a}^{b} f(x)\\,dx'], ['结构','极限','limit','\\lim_{x\\to 0} f(x)'], ['结构','矩阵','matrix','\\begin{bmatrix}a&b\\\\c&d\\end{bmatrix}'], ['结构','分段函数','cases','f(x)=\\begin{cases}x,&x\\ge0\\\\-x,&x<0\\end{cases}'],
];
let formulaCategory = '常用';

function openFormulaModal(mode) { if (!state.editing) return; closeMdPopups(); $('formula-mode').value = mode || 'inline'; $('formula-modal').classList.remove('hidden'); $('formula-search').value = ''; renderFormulaPicker(); setTimeout(() => $('formula-search').focus(), 0); }
function closeFormulaModal() { $('formula-modal').classList.add('hidden'); if (cmView) cmView.focus(); }
function renderFormulaPicker() {
  const cats = [...new Set(FORMULAS.map(f => f[0]))]; const catBox = $('formula-cats'); catBox.innerHTML = '';
  cats.forEach(c => { const b = document.createElement('button'); b.textContent = c; b.classList.toggle('active', c === formulaCategory); b.addEventListener('click', () => { formulaCategory = c; renderFormulaPicker(); }); catBox.appendChild(b); });
  const q = $('formula-search').value.trim().toLowerCase(); const rows = FORMULAS.filter(f => (q ? (f.join(' ').toLowerCase().includes(q)) : f[0] === formulaCategory));
  const list = $('formula-list'); list.innerHTML = '';
  rows.forEach(f => { const b = document.createElement('button'); b.className = 'formula-item'; b.innerHTML = '<span></span><small></small>'; b.querySelector('span').textContent = f[1]; b.querySelector('small').textContent = f[3]; b.addEventListener('mouseenter', () => previewFormula(f[3])); b.addEventListener('focus', () => previewFormula(f[3])); b.addEventListener('click', () => insertFormula(f[3])); list.appendChild(b); });
}
function previewFormula(tex) { const p = $('formula-preview'); p.textContent = '$$' + tex + '$$'; renderMath(p); }
function insertFormula(tex) { const mode = $('formula-mode').value; closeFormulaModal(); if (!cmView) return; const sel = cmView.state.selection.main; const selected = cmView.state.sliceDoc(sel.from, sel.to); const body = selected || tex; const insert = mode === 'block' ? '\n$$\n' + body + '\n$$\n' : '$' + body + '$'; cmView.dispatch({changes:{from:sel.from,to:sel.to,insert},selection:{anchor:sel.from+insert.length}}); cmView.focus(); }

/* ---------------- 图片编辑器（插入 / 裁剪 / 缩放 / 旋转） ---------------- */

const imgState = {
  img: null, rawW: 0, rawH: 0,
  angle: 0, scale: 100, ratio: 'free', viewZoom: 100, panX: 0, panY: 0,
  flipX: false, flipY: false, sizeLock: true, outW: 0, outH: 0,
  rotW: 0, rotH: 0, fitScale: 1, offX: 0, offY: 0,
  crop: { x: 0, y: 0, w: 0, h: 0 },
  drag: null, history: [], redo: [], spaceDown: false,
};

function imgSnapshot() { return {angle:imgState.angle,scale:imgState.scale,ratio:imgState.ratio,viewZoom:imgState.viewZoom,panX:imgState.panX,panY:imgState.panY,flipX:imgState.flipX,flipY:imgState.flipY,sizeLock:imgState.sizeLock,outW:imgState.outW,outH:imgState.outH,crop:Object.assign({},imgState.crop)}; }
function pushImgHistory() { if (!imgState.img) return; imgState.history.push(imgSnapshot()); if (imgState.history.length > 40) imgState.history.shift(); imgState.redo = []; updateImgHistoryButtons(); }
function restoreImgSnapshot(s) { if (!s) return; Object.assign(imgState, s); imgState.crop = Object.assign({}, s.crop); syncImgControls(); drawImg(); }
function undoImg() { const s=imgState.history.pop(); if (!s) return; imgState.redo.push(imgSnapshot()); restoreImgSnapshot(s); updateImgHistoryButtons(); }
function redoImg() { const s=imgState.redo.pop(); if (!s) return; imgState.history.push(imgSnapshot()); restoreImgSnapshot(s); updateImgHistoryButtons(); }
function updateImgHistoryButtons() { $('img-undo').disabled=!imgState.history.length; $('img-redo').disabled=!imgState.redo.length; }

function openImgModal() {
  if (!state.dir) { showToast('图片编辑仅支持本地 Markdown 文件'); return; }
  $('img-modal').classList.remove('hidden');
  resetImg();
  drawImg();
  updateImgInfo();
}

function closeImgModal() {
  $('img-modal').classList.add('hidden');
  imgState.img = null;
  imgState.drag = null;
}

function loadImgFromFile(file) {
  if (!file) return;
  const fr = new FileReader();
  fr.onload = () => {
    try { loadImgSrc(fr.result); } catch (e) { showToast('图片读取失败：' + e.message); }
  };
  fr.onerror = () => showToast('图片读取失败');
  fr.readAsDataURL(file);
}

function loadImgSrc(src) {
  const im = new Image();
  im.onload = () => {
    imgState.img = im;
    imgState.rawW = im.naturalWidth || im.width;
    imgState.rawH = im.naturalHeight || im.height;
    resetImg();
    $('img-hint').style.display = 'none';
    $('img-insert').disabled = false;
    $('img-crop').classList.add('active');
    updateImgInfo();
  };
  im.onerror = () => showToast('图片加载失败（URL 可能被跨域限制）');
  im.src = src;
}

function computeRotated() {
  const a = ((imgState.angle % 360) + 360) % 360;
  const rad = a * Math.PI / 180;
  imgState.rotW = Math.max(1, Math.round(Math.abs(imgState.rawW * Math.cos(rad)) + Math.abs(imgState.rawH * Math.sin(rad))));
  imgState.rotH = Math.max(1, Math.round(Math.abs(imgState.rawW * Math.sin(rad)) + Math.abs(imgState.rawH * Math.cos(rad))));
}

function imgRect() {
  return { x: imgState.offX, y: imgState.offY, w: imgState.rotW * imgState.fitScale, h: imgState.rotH * imgState.fitScale };
}

function drawImg() {
  const canvas = $('img-canvas');
  const stage = $('img-stage');
  if (!canvas || !imgState.img) return;
  const cw = stage.clientWidth;
  const ch = stage.clientHeight;
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(cw * dpr);
  canvas.height = Math.round(ch * dpr);
  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = '#101418';
  ctx.fillRect(0, 0, cw, ch);
  computeRotated();
  imgState.fitScale = Math.min(cw / imgState.rotW, ch / imgState.rotH) * imgState.viewZoom / 100;
  imgState.offX = (cw - imgState.rotW * imgState.fitScale) / 2 + imgState.panX;
  imgState.offY = (ch - imgState.rotH * imgState.fitScale) / 2 + imgState.panY;
  const tmp = document.createElement('canvas');
  tmp.width = imgState.rotW;
  tmp.height = imgState.rotH;
  const tctx = tmp.getContext('2d');
  tctx.translate(imgState.rotW / 2, imgState.rotH / 2);
  tctx.rotate(imgState.angle * Math.PI / 180);
  tctx.scale(imgState.flipX ? -1 : 1, imgState.flipY ? -1 : 1);
  tctx.drawImage(imgState.img, -imgState.rawW / 2, -imgState.rawH / 2, imgState.rawW, imgState.rawH);
  ctx.drawImage(tmp, imgState.offX, imgState.offY, imgState.rotW * imgState.fitScale, imgState.rotH * imgState.fitScale);
  clampCrop();
  updateCropUI();
  updateImgInfo();
}

function ratioValue() {
  if (imgState.ratio === '1:1') return 1;
  if (imgState.ratio === '4:3') return 4 / 3;
  if (imgState.ratio === '3:2') return 3 / 2;
  if (imgState.ratio === '16:9') return 16 / 9;
  if (imgState.ratio === 'orig') {
    const r = imgState.rotW / imgState.rotH;
    return isFinite(r) && r > 0 ? r : 1;
  }
  return 0;
}

function clampCrop() {
  const r = imgRect();
  const min = 24;
  let { x, y, w, h } = imgState.crop;
  if (!imgState.img || !r.w || !r.h) { x = r.x; y = r.y; w = r.w; h = r.h; }
  w = Math.max(min, Math.min(w, r.w));
  h = Math.max(min, Math.min(h, r.h));
  x = Math.max(r.x, Math.min(x, r.x + r.w - w));
  y = Math.max(r.y, Math.min(y, r.y + r.h - h));
  imgState.crop = { x, y, w, h };
  const rv = ratioValue();
  if (rv > 0) {
    let nw = w, nh = nw / rv;
    if (nh > r.h) { nh = r.h; nw = nh * rv; }
    imgState.crop.w = nw;
    imgState.crop.h = nh;
    imgState.crop.x = Math.max(r.x, Math.min(imgState.crop.x, r.x + r.w - nw));
    imgState.crop.y = Math.max(r.y, Math.min(imgState.crop.y, r.y + r.h - nh));
  }
}

function updateCropUI() {
  const c = $('img-crop');
  if (!c) return;
  c.style.left = imgState.crop.x + 'px';
  c.style.top = imgState.crop.y + 'px';
  c.style.width = imgState.crop.w + 'px';
  c.style.height = imgState.crop.h + 'px';
}

function updateImgInfo() {
  const el = $('img-info');
  if (!el) return;
  const r = imgRect();
  if (!imgState.img || !r.w) { el.textContent = '尚未加载图片'; return; }
  const naturalW = Math.max(1, Math.round(imgState.crop.w / imgState.fitScale));
  const naturalH = Math.max(1, Math.round(imgState.crop.h / imgState.fitScale));
  if (!imgState.outW || !imgState.outH) { imgState.outW = naturalW; imgState.outH = naturalH; }
  $('img-out-w').value = imgState.outW; $('img-out-h').value = imgState.outH;
  el.textContent = '原图 ' + imgState.rawW + '×' + imgState.rawH + ' · 裁剪 ' + naturalW + '×' + naturalH + ' · 输出 ' + imgState.outW + '×' + imgState.outH + ' px';
}

function resetImg() {
  imgState.angle = 0;
  imgState.scale = 100;
  imgState.ratio = 'free';
  imgState.viewZoom = 100; imgState.panX = 0; imgState.panY = 0; imgState.flipX = false; imgState.flipY = false; imgState.outW = 0; imgState.outH = 0; imgState.sizeLock = true;
  imgState.history = []; imgState.redo = []; syncImgControls(); updateImgHistoryButtons();
  if (imgState.img) {
    computeRotated();
    const stage = $('img-stage');
    const cw = stage.clientWidth, ch = stage.clientHeight;
    imgState.fitScale = Math.min(cw / imgState.rotW, ch / imgState.rotH);
    imgState.offX = (cw - imgState.rotW * imgState.fitScale) / 2;
    imgState.offY = (ch - imgState.rotH * imgState.fitScale) / 2;
    const r = imgRect();
    imgState.crop = { x: r.x, y: r.y, w: r.w, h: r.h };
    $('img-hint').style.display = 'none';
    $('img-crop').classList.add('active');
    $('img-insert').disabled = false;
  } else {
    imgState.rotW = 0; imgState.rotH = 0;
    $('img-hint').style.display = '';
    $('img-crop').classList.remove('active');
    $('img-insert').disabled = true;
  }
  drawImg();
}

function resetImgEditing() { if (!imgState.img) { resetImg(); return; } const previous=imgSnapshot(); resetImg(); imgState.history=[previous]; imgState.redo=[]; updateImgHistoryButtons(); }

function rotateImg(delta) {
  if (!imgState.img) return;
  pushImgHistory(); imgState.angle += delta;
  while (imgState.angle > 180) imgState.angle -= 360; while (imgState.angle < -180) imgState.angle += 360;
  imgState.outW = 0; imgState.outH = 0; syncImgControls();
  drawImg();
}

function syncImgControls() { $('img-angle').value=imgState.angle; $('img-angle-number').value=imgState.angle; $('img-view-zoom').value=imgState.viewZoom; $('img-view-zoom-val').textContent=Math.round(imgState.viewZoom)+'%'; $('img-ratio').value=imgState.ratio; $('img-size-lock').classList.toggle('active',imgState.sizeLock); $('img-size-lock').setAttribute('aria-pressed',imgState.sizeLock?'true':'false'); }

function setImgAngle(v) { if (!imgState.img) return; imgState.angle=Math.max(-180,Math.min(180,Number(v)||0)); imgState.outW=0; imgState.outH=0; syncImgControls(); drawImg(); }
function setImgZoom(v, keepHistory) { if (!imgState.img) return; if (keepHistory) pushImgHistory(); const old=imgRect(); const crop=Object.assign({},imgState.crop); imgState.viewZoom=Math.max(25,Math.min(400,Number(v)||100)); drawImg(); const now=imgRect(); if (old.w>0) { imgState.crop={x:now.x+(crop.x-old.x)/old.w*now.w,y:now.y+(crop.y-old.y)/old.h*now.h,w:crop.w/old.w*now.w,h:crop.h/old.h*now.h}; clampCrop(); updateCropUI(); } syncImgControls(); updateImgInfo(); }
function flipImg(axis) { if(!imgState.img)return; pushImgHistory(); if(axis==='x')imgState.flipX=!imgState.flipX; else imgState.flipY=!imgState.flipY; drawImg(); }

function applyRatio() {
  if (!imgState.img) return;
  clampCrop();
  updateCropUI();
  updateImgInfo();
}

function stagePointer(e) {
  if (!imgState.img) return;
  const stage = $('img-stage');
  const rect = stage.getBoundingClientRect();
  const px = e.clientX - rect.left;
  const py = e.clientY - rect.top;
  const handle = e.target && e.target.dataset && e.target.dataset.handle;
  const inCrop = px >= imgState.crop.x - 4 && px <= imgState.crop.x + imgState.crop.w + 4 &&
                 py >= imgState.crop.y - 4 && py <= imgState.crop.y + imgState.crop.h + 4;
  pushImgHistory();
  if (imgState.spaceDown || e.button === 1) {
    imgState.drag = {mode:'pan',sx:px,sy:py,panX:imgState.panX,panY:imgState.panY};
    stage.setPointerCapture(e.pointerId); e.preventDefault(); return;
  }
  if (handle || (inCrop && !e.shiftKey)) {
    imgState.drag = handle
      ? { mode: 'resize', handle, sx: px, sy: py, cx: imgState.crop.x, cy: imgState.crop.y, cw: imgState.crop.w, ch: imgState.crop.h }
      : { mode: 'move', sx: px, sy: py, cx: imgState.crop.x, cy: imgState.crop.y, cw: imgState.crop.w, ch: imgState.crop.h };
    stage.setPointerCapture(e.pointerId);
    e.preventDefault();
  } else {
    // 在空白处拖拽 = 从按下点画新裁剪框
    imgState.drag = { mode: 'draw', sx: px, sy: py, cw: 0, ch: 0 };
    stage.setPointerCapture(e.pointerId);
    e.preventDefault();
  }
}

function stagePointerMove(e) {
  if (!imgState.drag || !imgState.img) return;
  const stage = $('img-stage');
  const rect = stage.getBoundingClientRect();
  const px = e.clientX - rect.left;
  const py = e.clientY - rect.top;
  const r = imgRect();
  const d = imgState.drag;
  const rv = ratioValue();
  if (d.mode === 'pan') {
    imgState.panX=d.panX+(px-d.sx); imgState.panY=d.panY+(py-d.sy); drawImg();
  } else if (d.mode === 'move') {
    let nx = d.cx + (px - d.sx);
    let ny = d.cy + (py - d.sy);
    nx = Math.max(r.x, Math.min(nx, r.x + r.w - d.cw));
    ny = Math.max(r.y, Math.min(ny, r.y + r.h - d.ch));
    imgState.crop.x = nx; imgState.crop.y = ny;
  } else if (d.mode === 'resize') {
    let l=d.cx,t=d.cy,rr=d.cx+d.cw,bb=d.cy+d.ch; const dx=px-d.sx,dy=py-d.sy;
    if(d.handle.includes('w'))l+=dx; if(d.handle.includes('e'))rr+=dx; if(d.handle.includes('n'))t+=dy; if(d.handle.includes('s'))bb+=dy;
    l=Math.max(r.x,Math.min(l,rr-24)); rr=Math.min(r.x+r.w,Math.max(rr,l+24)); t=Math.max(r.y,Math.min(t,bb-24)); bb=Math.min(r.y+r.h,Math.max(bb,t+24));
    let w=rr-l,h=bb-t;
    if(rv>0){ if(d.handle==='n'||d.handle==='s'){w=h*rv;l=(l+rr-w)/2;rr=l+w;} else {h=w/rv;t=(t+bb-h)/2;bb=t+h;} if(l<r.x){l=r.x;rr=l+w;} if(rr>r.x+r.w){rr=r.x+r.w;l=rr-w;} if(t<r.y){t=r.y;bb=t+h;} if(bb>r.y+r.h){bb=r.y+r.h;t=bb-h;} }
    imgState.crop={x:l,y:t,w:rr-l,h:bb-t}; imgState.outW=0; imgState.outH=0;
  } else if (d.mode === 'draw') {
    let x = Math.min(d.sx, px), y = Math.min(d.sy, py);
    let w = Math.abs(px - d.sx), h = Math.abs(py - d.sy);
    x = Math.max(r.x, Math.min(x, r.x + r.w));
    y = Math.max(r.y, Math.min(y, r.y + r.h));
    w = Math.max(24, Math.min(w, r.x + r.w - x));
    h = Math.max(24, Math.min(h, r.y + r.h - y));
    if (rv > 0) {
      if (w / rv > r.h) { w = r.h * rv; h = r.h; }
      else h = w / rv;
      x = Math.max(r.x, Math.min(x, r.x + r.w - w));
      y = Math.max(r.y, Math.min(y, r.y + r.h - h));
    }
    imgState.crop = { x, y, w, h };
    imgState.outW=0; imgState.outH=0;
  }
  updateCropUI();
  updateImgInfo();
  e.preventDefault();
}

function stagePointerUp(e) {
  imgState.drag = null;
  updateImgHistoryButtons();
  try { $('img-stage').releasePointerCapture(e.pointerId); } catch (err) { /* ignore */ }
}

function insertImgUrl() {
  const url = $('img-url-input').value.trim();
  if (!url) { showToast('请输入图片 URL'); return; }
  if (!cmView) { showToast('请先进入编辑模式'); return; }
  cmInsertImage(url);
  closeImgModal();
  showToast('已插入图片 URL');
}

function cmInsertImage(rel) {
  if (!cmView) return;
  const sel = cmView.state.selection.main;
  const insert = '![图片](' + rel + ')';
  cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert }, selection: { anchor: sel.from + insert.length } });
  cmView.focus();
}

async function exportAndInsertImg() {
  if (!imgState.img) return;
  const r = imgRect();
  const srcX = (imgState.crop.x - r.x) / imgState.fitScale;
  const srcY = (imgState.crop.y - r.y) / imgState.fitScale;
  const srcW = imgState.crop.w / imgState.fitScale;
  const srcH = imgState.crop.h / imgState.fitScale;
  const outW = Math.max(1, Math.round(imgState.outW || srcW));
  const outH = Math.max(1, Math.round(imgState.outH || srcH));
  const tmp = document.createElement('canvas');
  tmp.width = imgState.rotW;
  tmp.height = imgState.rotH;
  const tctx = tmp.getContext('2d');
  tctx.translate(imgState.rotW / 2, imgState.rotH / 2);
  tctx.rotate(imgState.angle * Math.PI / 180);
  tctx.scale(imgState.flipX ? -1 : 1, imgState.flipY ? -1 : 1);
  tctx.drawImage(imgState.img, -imgState.rawW / 2, -imgState.rawH / 2, imgState.rawW, imgState.rawH);
  const out = document.createElement('canvas');
  out.width = outW;
  out.height = outH;
  const octx = out.getContext('2d');
  octx.imageSmoothingEnabled = true;
  octx.imageSmoothingQuality = 'high';
  octx.drawImage(tmp, srcX, srcY, srcW, srcH, 0, 0, outW, outH);
  const blob = await new Promise(res => out.toBlob(res, 'image/png'));
  if (!blob) { showToast('图片导出失败'); return; 
  }
  const b64 = await new Promise(res => {
    const fr = new FileReader();
    fr.onload = () => res(String(fr.result).split(',')[1] || '');
    fr.readAsDataURL(blob);
  });
  busy(true);
  try {
    const resp = await apiFetch('/api/image/save', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ dir: state.dir, data: b64, format: 'png', name: 'img_' + Date.now() }),
    });
    const d = await resp.json();
    if (!resp.ok || !d.ok) throw new Error(d.error || '保存失败');
    cmInsertImage(d.rel);
    closeImgModal();
    showToast('图片已插入（' + d.rel + '）');
  } catch (e) {
    showToast('图片保存失败：' + e.message);
  } finally {
    busy(false);
  }
}
/* ---------------- 编辑实时预览（左/右/下/上 + 滚动同步） ---------------- */

let pvTimer = null;
let pvLast = '';
let pvEditorEl = null;

function getEditContent() {
  return cmView ? cmView.state.doc.toString() : ($('edit-area') && $('edit-area').value || '');
}

function setPvLayout(layout) {
  if (['none', 'left', 'right', 'bottom', 'top'].indexOf(layout) < 0) layout = 'none';
  state.pvLayout = layout;
  document.querySelectorAll('.pv-btn').forEach(b => b.classList.toggle('active', b.dataset.pv === layout));
  const names = {none:'无',left:'左',right:'右',bottom:'下',top:'上'};
  const narrow = window.innerWidth < 600 && (layout === 'left' || layout === 'right');
  if ($('pv-trigger')) $('pv-trigger').textContent = narrow ? '预览：' + names[layout] + '（窄屏置底）⌄' : '预览：' + names[layout] + '⌄';
  const mc = $('main-col');
  const pw = $('preview-wrap');
  if (!mc || !pw) return;
  mc.classList.remove('pv-left', 'pv-right', 'pv-bottom', 'pv-top');
  if (state.editing && layout !== 'none') {
    mc.classList.add('pv-' + layout);
    pw.classList.remove('hidden');
    $('pv-splitter').classList.remove('hidden');
    applyPvSplit();
    schedulePreview();
  } else {
    pw.classList.add('hidden');
    $('pv-splitter').classList.add('hidden');
  }
  saveSettings();
}

function applyPvSplit() {
  const pw = $('preview-wrap'); if (!pw) return;
  const horizontal = state.pvLayout === 'left' || state.pvLayout === 'right';
  const pct = horizontal ? state.pvSplitX : state.pvSplitY;
  pw.style.flexBasis = pct + '%';
}

function bindPvSplitter() {
  const bar = $('pv-splitter'); const mc = $('main-col'); if (!bar || !mc) return;
  const update = e => {
    const r = mc.getBoundingClientRect(); let pct;
    if (state.pvLayout === 'left') pct = (e.clientX - r.left) / r.width * 100;
    else if (state.pvLayout === 'right') pct = (r.right - e.clientX) / r.width * 100;
    else if (state.pvLayout === 'top') pct = (e.clientY - r.top) / r.height * 100;
    else pct = (r.bottom - e.clientY) / r.height * 100;
    pct = Math.max(25, Math.min(70, pct));
    if (state.pvLayout === 'left' || state.pvLayout === 'right') state.pvSplitX = pct; else state.pvSplitY = pct;
    applyPvSplit();
  };
  bar.addEventListener('pointerdown', e => { bar.setPointerCapture(e.pointerId); update(e); });
  bar.addEventListener('pointermove', e => { if (bar.hasPointerCapture(e.pointerId)) update(e); });
  bar.addEventListener('pointerup', e => { if (bar.hasPointerCapture(e.pointerId)) bar.releasePointerCapture(e.pointerId); saveSettings(); });
  bar.addEventListener('keydown', e => { if (!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)) return; e.preventDefault(); const delta = (e.key === 'ArrowRight' || e.key === 'ArrowDown') ? 2 : -2; if (state.pvLayout === 'left' || state.pvLayout === 'right') state.pvSplitX = Math.max(25, Math.min(70, state.pvSplitX + delta)); else state.pvSplitY = Math.max(25, Math.min(70, state.pvSplitY + delta)); applyPvSplit(); saveSettings(); });
  window.addEventListener('resize', () => setPvLayout(state.pvLayout));
}

function schedulePreview() {
  if (pvTimer) clearTimeout(pvTimer);
  pvTimer = setTimeout(renderPreview, 300);
}

function renderPreview() {
  pvTimer = null;
  const pane = $('preview-pane');
  if (!pane || state.pvLayout === 'none' || !state.editing) return;
  const src = getEditContent();
  if (src === pvLast) return;
  pvLast = src;
  let html;
  try {
    const prot = protectMath(src);
    html = restoreMath(marked.parse(prot.src, { gfm: true, breaks: false }), prot.saved);
  } catch (e) {
    html = '<p class="ai-err">预览渲染失败</p>';
  }
  pane.innerHTML = html;
  fixLinks(pane);
  fixImages(pane);
  renderMath(pane);
}

function pvSyncFromEditor() {
  if (!state.pvSync || state.pvLayout === 'none') return;
  const src = pvEditorEl || $('edit-area');
  const dst = $('preview-wrap');
  if (!src || !dst) return;
  const maxSrc = src.scrollHeight - src.clientHeight;
  const maxDst = dst.scrollHeight - dst.clientHeight;
  if (maxSrc <= 0 || maxDst <= 0) return;
  dst.scrollTop = (src.scrollTop / maxSrc) * maxDst;
}

function pvSyncFromPreview() {
  if (!state.pvSync || state.pvLayout === 'none') return;
  const src = $('preview-wrap');
  const dst = pvEditorEl || $('edit-area');
  if (!src || !dst) return;
  const maxSrc = src.scrollHeight - src.clientHeight;
  const maxDst = dst.scrollHeight - dst.clientHeight;
  if (maxSrc <= 0 || maxDst <= 0) return;
  dst.scrollTop = (src.scrollTop / maxSrc) * maxDst;
}

function applyPvUi() {
  document.querySelectorAll('.pv-btn').forEach(b => b.classList.toggle('active', b.dataset.pv === state.pvLayout));
  const sync = $('pv-sync');
  if (sync) sync.checked = !!state.pvSync;
  setPvLayout(state.pvLayout);
}

async function toggleEdit() {
  if (state.editing) { exitEdit(); return; }
  if (state.original === undefined || state.original === '') { showToast('没有可编辑的内容'); return; }
  $('edit-bar').classList.remove('hidden');
  $('content').classList.add('hidden');
  state.editing = true;
  setEditBtn('编辑中');
  pvLast = '';
  try {
    await loadCodeMirror();
  } catch (e) { /* 退回 textarea */ }
  if (window.ReadMDCodeMirror) {
    $('edit-area').classList.add('hidden');
    $('edit-wrap').classList.remove('hidden');
    createEditor(state.original || '');
    pvEditorEl = cmView ? cmView.scrollDOM : null;
    if (pvEditorEl) pvEditorEl.addEventListener('scroll', pvSyncFromEditor);
  } else {
    $('edit-wrap').classList.add('hidden');
    $('edit-area').classList.remove('hidden');
    $('edit-area').value = state.original || '';
    pvEditorEl = $('edit-area');
    pvEditorEl.addEventListener('scroll', pvSyncFromEditor);
    $('edit-area').focus();
  }
  applyPvUi();
}

function exitEdit() {
  if (pvTimer) { clearTimeout(pvTimer); pvTimer = null; }
  if (pvEditorEl) {
    pvEditorEl.removeEventListener('scroll', pvSyncFromEditor);
    pvEditorEl = null;
  }
  const pw = $('preview-wrap');
  if (pw) pw.classList.add('hidden');
  const mc = $('main-col');
  if (mc) mc.classList.remove('pv-left', 'pv-right', 'pv-bottom', 'pv-top');
  pvLast = '';
  if (!state.editing) {
    $('edit-bar').classList.add('hidden');
    $('edit-area').classList.add('hidden');
    $('edit-wrap').classList.add('hidden');
    $('content').classList.remove('hidden');
    setEditBtn('编辑');
    return;
  }
  destroyEditor();
  $('edit-bar').classList.add('hidden');
  $('edit-area').classList.add('hidden');
  $('edit-wrap').classList.add('hidden');
  $('content').classList.remove('hidden');
  state.editing = false;
  setEditBtn('编辑');
}

async function saveEdit() {
  if (!state.editing) return;
  const content = cmView ? cmView.state.doc.toString() : $('edit-area').value;
  if (!state.file) {
    // 虚拟文档（转换 / OCR / 网页）：另存为 .md 后切换为文件模式
    const name = (state.sourceName || 'document').replace(/[\\/]/g, '_');
    const suggested = name.replace(/\.[^.]+$/, '') + '.md';
    let out = null;
    if (hasPy) {
      busy(true);
      try { out = await py.save_as(content, suggested, state.webAssets || []); }
      catch (e) { showToast('保存失败：' + e.message); busy(false); return; }
      busy(false);
      if (!out) { showToast('已取消保存'); return; }
      showToast('已保存：' + out);
      exitEdit();
      await loadFile(out);
    } else {
      const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = suggested;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 3000);
      showToast('已下载：' + suggested);
    }
    return;
  }
  busy(true);
  try {
    let ok;
    if (hasPy) {
      ok = await py.save_file(state.file, content, state.encoding || 'utf-8');
    } else {
      const r = await apiFetch('/api/save', {
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
    const out = await py.save_as(content, suggested, state.webAssets || []);
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
    const r = await apiFetch('/api/list?p=' + encodeURIComponent(dir));
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
  const canEdit = (state.mode === 'file' || state.mode === 'virtual') && !!state.original && !state.editing;
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
      const r = await apiFetch('/api/file?p=' + encodeURIComponent(state.file) + '&meta=1');
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
  if (moduleBlocked('web')) return;
  $('url-modal').classList.remove('hidden');
  $('url-render').disabled = !hasPy;
  $('url-progress').classList.add('hidden');
  $('url-progress').setAttribute('aria-hidden', 'true');
  setWebStatus(LAN_TOKEN
    ? '局域网页面支持增强静态抓取；动态渲染请使用桌面应用。'
    : '请输入公开的 HTTP/HTTPS 网页地址。');
  $('url-input').focus();
}

function closeWebDialog() {
  if (webRun.running) { cancelWebTask(); return; }
  $('url-modal').classList.add('hidden');
}

function bindAiResize() {
  const handle = $('ai-resize-handle');
  if (!handle) return;
  let startX = 0, startWidth = 0;
  handle.addEventListener('pointerdown', e => {
    startX = e.clientX; startWidth = state.aiPanelWidth;
    handle.setPointerCapture(e.pointerId);
    document.body.classList.add('ai-resizing');
  });
  handle.addEventListener('pointermove', e => {
    if (!handle.hasPointerCapture(e.pointerId)) return;
    const max = Math.max(360, Math.floor(window.innerWidth * 0.94));
    state.aiPanelWidth = Math.max(360, Math.min(max, startWidth + startX - e.clientX));
    document.body.style.setProperty('--ai-panel-width', state.aiPanelWidth + 'px');
  });
  const finish = e => {
    if (!handle.hasPointerCapture(e.pointerId)) return;
    handle.releasePointerCapture(e.pointerId);
    document.body.classList.remove('ai-resizing');
    saveSettings();
  };
  handle.addEventListener('pointerup', finish);
  handle.addEventListener('pointercancel', finish);
}

/* ---------------- 事件绑定 ---------------- */

function bindEvents() {
  $('btn-open').addEventListener('click', () => { loadFileDialog(); });
  $('w-open').addEventListener('click', () => { loadFileDialog(); });
  $('btn-folder').addEventListener('click', openFolder);
  $('w-folder').addEventListener('click', openFolder);

  const moreBtn = $('btn-more');
  const moreMenu = $('more-menu');
  if (moreBtn && moreMenu) {
    moreBtn.addEventListener('click', e => {
      e.stopPropagation();
      moreMenu.classList.toggle('open');
    });
    document.addEventListener('click', e => {
      if (moreMenu.classList.contains('open') && !moreMenu.contains(e.target) && e.target !== moreBtn) {
        moreMenu.classList.remove('open');
      }
    });
  }

  $('btn-convert').addEventListener('click', openConvertModal);
  $('w-convert').addEventListener('click', openConvertModal);
  $('convert-files').addEventListener('click', pickConvertFiles);
  $('convert-folder').addEventListener('click', pickConvertFolder);
  $('convert-close').addEventListener('click', closeConvertModal);
  $('convert-open-dir').addEventListener('click', () => {
    if (convertLastDir && py.open_dir) py.open_dir(convertLastDir);
  });
  $('convert-modal').addEventListener('click', e => { if (e.target === $('convert-modal')) closeConvertModal(); });
  $('btn-ocr').addEventListener('click', () => chooseFile('ocr'));
  $('w-ocr').addEventListener('click', () => chooseFile('ocr'));
  $('btn-web').addEventListener('click', openWebDialog);
  $('w-web').addEventListener('click', openWebDialog);
  $('url-go').addEventListener('click', () => {
    const url = $('url-input').value.trim();
    const crawl = $('url-crawl').checked;
    webToMd(url, crawl, false);
  });
  $('url-render').addEventListener('click', () => webToMd($('url-input').value.trim(), $('url-crawl').checked, true));
  $('url-cancel').addEventListener('click', cancelWebTask);
  $('url-close').addEventListener('click', closeWebDialog);
  $('url-modal').addEventListener('click', e => { if (e.target === $('url-modal') && !webRun.running) closeWebDialog(); });
  $('url-input').addEventListener('keydown', e => { if (e.key === 'Enter') $('url-go').click(); });
  $('url-modal').addEventListener('keydown', e => { if (e.key === 'Escape') { e.preventDefault(); closeWebDialog(); } });

  $('btn-edit').addEventListener('click', toggleEdit);
  document.querySelectorAll('#md-tool [data-md]').forEach(b => b.addEventListener('click', () => {
    closeMdPopups(); if (b.dataset.md === 'image') openImgModal(); else cmInsertSyntax(b.dataset.md);
  }));
  document.querySelectorAll('#md-tool [data-menu]').forEach(b => b.addEventListener('click', e => {
    e.stopPropagation(); const menu = $(b.dataset.menu); const wasHidden = menu.classList.contains('hidden'); closeMdPopups(); if (wasHidden) menu.classList.remove('hidden');
  }));
  $('md-command-open').addEventListener('click', openMdCommandPalette);
  $('md-command-search').addEventListener('input', () => { mdCommandIndex = 0; renderMdCommands(); });
  $('md-command-search').addEventListener('keydown', e => { const items = [...$('md-command-list').querySelectorAll('.command-item')]; if (e.key === 'ArrowDown') { e.preventDefault(); mdCommandIndex = Math.min(items.length - 1, mdCommandIndex + 1); renderMdCommands(); } else if (e.key === 'ArrowUp') { e.preventDefault(); mdCommandIndex = Math.max(0, mdCommandIndex - 1); renderMdCommands(); } else if (e.key === 'Enter' && items[mdCommandIndex]) { e.preventDefault(); items[mdCommandIndex].click(); } else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); closeMdCommandPalette(); } });
  $('md-command-modal').addEventListener('click', e => { if (e.target === $('md-command-modal')) closeMdCommandPalette(); });
  $('formula-open').addEventListener('click', () => openFormulaModal('inline'));
  $('formula-close').addEventListener('click', closeFormulaModal);
  $('formula-search').addEventListener('input', renderFormulaPicker);
  $('formula-modal').addEventListener('click', e => { if (e.target === $('formula-modal')) closeFormulaModal(); });
  $('edit-save').addEventListener('click', saveEdit);
  $('edit-cancel').addEventListener('click', exitEdit);
  $('pv-trigger').addEventListener('click', e => { e.stopPropagation(); const m = $('pv-menu'); const show = m.classList.contains('hidden'); closeMdPopups(); m.classList.toggle('hidden', !show); $('pv-trigger').setAttribute('aria-expanded', show ? 'true' : 'false'); });
  document.querySelectorAll('.pv-btn').forEach(b => b.addEventListener('click', () => { setPvLayout(b.dataset.pv); closeMdPopups(); }));
  const pvSyncEl = $('pv-sync');
  if (pvSyncEl) pvSyncEl.addEventListener('change', e => { state.pvSync = e.target.checked; saveSettings(); });
  const pvWrap = $('preview-wrap');
  if (pvWrap) pvWrap.addEventListener('scroll', pvSyncFromPreview);
  bindPvSplitter();
  $('img-file').addEventListener('click', () => $('img-file-input').click());
  $('img-file-input').addEventListener('change', e => {
    const f = e.target.files && e.target.files[0];
    if (f) loadImgFromFile(f);
    e.target.value = '';
  });
  $('img-url-load').addEventListener('click', insertImgUrl);
  $('img-url-input').addEventListener('keydown', e => { if (e.key === 'Enter') insertImgUrl(); });
  $('img-rot-l').addEventListener('click', () => rotateImg(-90));
  $('img-rot-r').addEventListener('click', () => rotateImg(90));
  $('img-angle').addEventListener('pointerdown', pushImgHistory);
  $('img-angle').addEventListener('input', e => setImgAngle(e.target.value));
  $('img-angle-number').addEventListener('change', e => { pushImgHistory(); setImgAngle(e.target.value); });
  $('img-view-zoom').addEventListener('pointerdown', pushImgHistory);
  $('img-view-zoom').addEventListener('input', e => setImgZoom(e.target.value, false));
  $('img-flip-x').addEventListener('click', () => flipImg('x'));
  $('img-flip-y').addEventListener('click', () => flipImg('y'));
  $('img-ratio').addEventListener('change', e => { pushImgHistory(); imgState.ratio = e.target.value; imgState.outW=0; imgState.outH=0; applyRatio(); });
  $('img-size-lock').addEventListener('click', () => { imgState.sizeLock=!imgState.sizeLock; syncImgControls(); });
  $('img-out-w').addEventListener('change', e => { const oldW=imgState.outW||1, oldH=imgState.outH||1; imgState.outW=Math.max(1,Math.min(16000,+e.target.value||1)); if(imgState.sizeLock)imgState.outH=Math.max(1,Math.round(imgState.outW*oldH/oldW)); updateImgInfo(); });
  $('img-out-h').addEventListener('change', e => { const oldW=imgState.outW||1, oldH=imgState.outH||1; imgState.outH=Math.max(1,Math.min(16000,+e.target.value||1)); if(imgState.sizeLock)imgState.outW=Math.max(1,Math.round(imgState.outH*oldW/oldH)); updateImgInfo(); });
  $('img-undo').addEventListener('click', undoImg); $('img-redo').addEventListener('click', redoImg);
  $('img-reset').addEventListener('click', resetImgEditing);
  $('img-insert').addEventListener('click', exportAndInsertImg);
  $('img-close').addEventListener('click', closeImgModal);
  $('img-close-x').addEventListener('click', closeImgModal);
  $('img-modal').addEventListener('click', e => { if (e.target === $('img-modal')) closeImgModal(); });
  const stage = $('img-stage');
  stage.addEventListener('pointerdown', stagePointer);
  stage.addEventListener('pointermove', stagePointerMove);
  stage.addEventListener('pointerup', stagePointerUp);
  stage.addEventListener('pointercancel', stagePointerUp);
  stage.addEventListener('wheel', e => { if(!imgState.img)return; e.preventDefault(); setImgZoom(imgState.viewZoom*(e.deltaY>0?.9:1.1), true); }, {passive:false});
  stage.addEventListener('keydown', e => { if(e.key===' '){imgState.spaceDown=true;e.preventDefault();return;} if((e.key==='+'||e.key==='=')&&imgState.img){e.preventDefault();setImgZoom(imgState.viewZoom+10,true);return;} if(e.key==='-'&&imgState.img){e.preventDefault();setImgZoom(imgState.viewZoom-10,true);return;} if(!e.key.startsWith('Arrow')||!imgState.img)return; e.preventDefault();pushImgHistory(); const n=e.shiftKey?10:1; if(e.key==='ArrowLeft')imgState.crop.x-=n;if(e.key==='ArrowRight')imgState.crop.x+=n;if(e.key==='ArrowUp')imgState.crop.y-=n;if(e.key==='ArrowDown')imgState.crop.y+=n;clampCrop();updateCropUI();updateImgInfo(); });
  stage.addEventListener('keyup', e => { if(e.key===' ')imgState.spaceDown=false; });
  stage.addEventListener('blur', () => { imgState.spaceDown=false; });
  $('btn-saveas').addEventListener('click', saveAs);

  $('btn-recent').addEventListener('click', openHistoryModal);
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

/* ---------------- 导出面板（PDF / DOCX / HTML + 样式定制） ---------------- */

const EXPORT_FONTS = ['MicrosoftYaHei', 'SimHei', 'SimSun', 'KaiTi', 'DengXian', 'Arial'];
const EXPORT_MONO = ['Consolas', 'Courier New', 'SimHei'];
const EXPORT_ALIGNS = ['left', 'center', 'right', 'justify'];
const EXPORT_PAGES = ['A4', 'A5', 'B5', 'Letter', 'Legal'];
const EXPORT_PRESET_NAMES = { minimal: '简约', classic: '经典', business: '商务' };

/* 每个字段: {k: 点路径, label, type, opts, min, max, step, full, fmts} */
const EXPORT_SECTIONS = [
  { title: '页面设置', fmts: ['pdf', 'docx'], fields: [
    { k: 'page.size', label: '纸张', type: 'select', opts: EXPORT_PAGES },
    { k: 'page.orientation', label: '方向', type: 'select', opts: [['portrait', '纵向'], ['landscape', '横向']] },
    { k: 'page.marginTop', label: '上边距 mm', type: 'number', min: 0, max: 60 },
    { k: 'page.marginRight', label: '右边距 mm', type: 'number', min: 0, max: 60 },
    { k: 'page.marginBottom', label: '下边距 mm', type: 'number', min: 0, max: 60 },
    { k: 'page.marginLeft', label: '左边距 mm', type: 'number', min: 0, max: 60 },
  ]},
  { title: '封面与目录', fmts: ['pdf', 'docx'], fields: [
    { k: 'cover.enabled', label: '启用封面页', type: 'checkbox' },
    { k: 'cover.title', label: '封面标题（留空用文件名）', type: 'text', full: true },
    { k: 'cover.subtitle', label: '封面副标题', type: 'text', full: true },
    { k: 'cover.date', label: '封面日期', type: 'text' },
    { k: 'cover.align', label: '封面对齐', type: 'select', opts: [['center', '居中'], ['left', '左对齐'], ['right', '右对齐']] },
    { k: 'toc.enabled', label: 'PDF 目录页', type: 'checkbox', fmts: ['pdf'] },
  ]},
  { title: '正文排版', fmts: ['pdf', 'docx', 'html'], fields: [
    { k: 'typography.font', label: '正文字体', type: 'select', opts: EXPORT_FONTS.map(f => [f, f]) },
    { k: 'typography.size', label: '字号 pt', type: 'number', min: 8, max: 20 },
    { k: 'typography.lineHeight', label: '行距', type: 'number', min: 1, max: 2.5, step: 0.1 },
    { k: 'typography.spacing', label: '段间距 pt', type: 'number', min: 0, max: 30 },
    { k: 'typography.color', label: '正文颜色', type: 'color' },
    { k: 'typography.align', label: '对齐', type: 'select', opts: EXPORT_ALIGNS.map(a => [a, a]) },
  ]},
  { title: '标题（各级颜色 / 字号 / 加粗 / 对齐）', fmts: ['pdf', 'docx', 'html'], headingRows: true },
  { title: '表格', fmts: ['pdf', 'docx', 'html'], fields: [
    { k: 'table.headerBg', label: '表头背景', type: 'color' },
    { k: 'table.headerColor', label: '表头文字色', type: 'color' },
    { k: 'table.headerBold', label: '表头加粗', type: 'checkbox' },
    { k: 'table.borderColor', label: '边框颜色', type: 'color' },
    { k: 'table.borderWidth', label: '边框宽度 pt', type: 'number', min: 0, max: 3, step: 0.25 },
    { k: 'table.banded', label: '斑马纹', type: 'checkbox' },
    { k: 'table.bandColor', label: '斑马纹颜色', type: 'color' },
    { k: 'table.cellSize', label: '单元格字号 pt', type: 'number', min: 7, max: 16 },
    { k: 'table.cellPadding', label: '单元格内边距 pt', type: 'number', min: 0, max: 20 },
    { k: 'table.align', label: '对齐', type: 'select', opts: EXPORT_ALIGNS.map(a => [a, a]) },
    { k: 'table.widthPct', label: '表格宽度 %', type: 'number', min: 50, max: 100 },
  ]},
  { title: '代码块', fmts: ['pdf', 'docx', 'html'], fields: [
    { k: 'code.bg', label: '背景色', type: 'color' },
    { k: 'code.color', label: '文字色', type: 'color' },
    { k: 'code.font', label: '等宽字体', type: 'select', opts: EXPORT_MONO.map(f => [f, f]) },
    { k: 'code.size', label: '字号 pt', type: 'number', min: 6, max: 16 },
    { k: 'code.borderColor', label: '边框颜色', type: 'color' },
    { k: 'code.borderWidth', label: '边框宽度 pt', type: 'number', min: 0, max: 3, step: 0.25 },
    { k: 'code.rounded', label: '圆角（HTML）', type: 'checkbox', fmts: ['html'] },
  ]},
  { title: '引用与链接', fmts: ['pdf', 'docx', 'html'], fields: [
    { k: 'quote.barColor', label: '引用左边条色', type: 'color' },
    { k: 'quote.bg', label: '引用背景', type: 'color' },
    { k: 'quote.color', label: '引用文字色', type: 'color' },
    { k: 'link.color', label: '链接颜色', type: 'color' },
    { k: 'hr.color', label: '分割线颜色', type: 'color' },
  ]},
  { title: '页脚与元数据', fmts: ['pdf', 'docx'], fields: [
    { k: 'footer.pageNumbers', label: '显示页码', type: 'checkbox' },
    { k: 'footer.text', label: '页脚文字', type: 'text', full: true },
    { k: 'meta.title', label: '文档标题（PDF 元数据）', type: 'text', full: true },
    { k: 'meta.author', label: '作者', type: 'text' },
    { k: 'meta.subject', label: '主题', type: 'text' },
  ]},
  { title: '数学公式', fmts: ['pdf', 'docx'], fields: [
    { k: 'math.dpi', label: '渲染分辨率 DPI', type: 'number', min: 100, max: 500, step: 10 },
  ]},
  { title: 'HTML 主题', fmts: ['html'], fields: [
    { k: 'htmlTheme', label: '页面主题', type: 'select', opts: [['light', '亮色'], ['dark', '暗色'], ['sepia', '米色']] },
  ]},
];

function expGet(obj, path) {
  return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), obj);
}
function expSet(obj, path, val) {
  const ks = path.split('.');
  let o = obj;
  for (let i = 0; i < ks.length - 1; i++) {
    if (typeof o[ks[i]] !== 'object' || o[ks[i]] === null) o[ks[i]] = {};
    o = o[ks[i]];
  }
  o[ks[ks.length - 1]] = val;
}
function expDeepMerge(base, over) {
  const out = JSON.parse(JSON.stringify(base || {}));
  if (!over || typeof over !== 'object') return out;
  Object.keys(over).forEach(k => {
    const v = over[k];
    if (v && typeof v === 'object' && !Array.isArray(v) && out[k] && typeof out[k] === 'object') {
      out[k] = expDeepMerge(out[k], v);
    } else if (v !== undefined) out[k] = JSON.parse(JSON.stringify(v));
  });
  return out;
}

async function loadExportPresets() {
  if (state.export.defaults) return true;
  if (!bindPy()) return false;
  try {
    const d = await py.get_export_presets();
    if (!d || d.error) throw new Error((d && d.error) || 'no data');
    state.export.defaults = d.defaults || {};
    state.export.presets = d.presets || {};
    state.export.custom = d.custom || {};
    state.export.last = d.last || null;
    if (state.export.last && state.export.last.options) {
      state.export.options = expDeepMerge(state.export.defaults, state.export.last.options);
    } else {
      state.export.options = expDeepMerge(state.export.defaults, {});
    }
    return true;
  } catch (e) {
    console.error(e);
    return false;
  }
}

function openExportModal() {
  if (!bindPy()) { showToast('浏览器模式请使用桌面版导出'); return; }
  if (!state.export.ready) {
    loadExportPresets().then(ok => {
      if (ok) { state.export.ready = true; renderExportModal(); }
      else showToast('导出模块加载失败');
    });
    return;
  }
  renderExportModal();
}

function closeExportModal() { $('export-modal').classList.add('hidden'); }

function currentExportContent() {
  if (state.editing) {
    return (window.cmView && window.cmView.state) ? window.cmView.state.doc.toString()
      : ($('edit-area') && $('edit-area').value || '');
  }
  if (state.mode === 'file') return state.original || state.fixed || '';
  return state.fixed || '';
}
function currentExportName() {
  let n = '';
  if (state.mode === 'file' && state.file) n = state.file.split(/[\\/]/).pop();
  else n = (state.sourceName || '导出').split(/[\\/]/).pop();
  n = n.replace(/\.[^.]+$/, '');
  return n || '导出';
}

function renderExportModal() {
  $('export-modal').classList.remove('hidden');
  renderExportSections();
  renderExportPresetSelect();
  const r = $('export-result');
  r.textContent = ''; r.className = 'export-result';
  $('export-open').classList.add('hidden');
  $('export-reveal').classList.add('hidden');
}

function expFieldApplicable(f, fmt) {
  const own = f.fmts || EXPORT_SECTIONS.reduce((a, s) => a.concat((s.fields || []).map(x => x.k)), []);
  return (f.fmts || ['pdf', 'docx', 'html']).indexOf(fmt) >= 0;
}

function renderExportSections() {
  const fmt = state.export.fmt;
  const host = $('export-opts');
  host.textContent = '';
  EXPORT_SECTIONS.forEach(sec => {
    const fields = sec.fields || [];
    const applicable = sec.headingRows ? (sec.fmts.indexOf(fmt) >= 0)
      : fields.some(f => expFieldApplicable(f, fmt));
    if (!applicable) return;
    const wrap = document.createElement('div');
    wrap.className = 'exp-sec open';
    const head = document.createElement('button');
    head.type = 'button';
    head.className = 'exp-sec-head';
    head.innerHTML = '<span class="exp-arrow">&#9654;</span>' + sec.title;
    const body = document.createElement('div');
    body.className = 'exp-sec-body';
    if (sec.headingRows) {
      for (let i = 1; i <= 6; i++) {
        const row = document.createElement('div');
        row.className = 'exp-field full exp-h-row';
        row.innerHTML =
          '<label>H' + i + '</label>' +
          '<input type="number" data-k="headings.h' + i + '.size" min="8" max="40" title="字号">' +
          '<input type="color" data-k="headings.h' + i + '.color" title="颜色">' +
          '<label class="exp-check">加粗<input type="checkbox" data-k="headings.h' + i + '.bold"></label>' +
          '<select data-k="headings.h' + i + '.align">' + EXPORT_ALIGNS.map(a => '<option value="' + a + '">' + a + '</option>').join('') + '</select>';
        body.appendChild(row);
      }
    } else {
      fields.forEach(f => {
        if (!expFieldApplicable(f, fmt)) return;
        body.appendChild(expFieldEl(f));
      });
    }
    head.addEventListener('click', () => wrap.classList.toggle('open'));
    wrap.appendChild(head);
    wrap.appendChild(body);
    host.appendChild(wrap);
  });
  applyExportOptionsToDom();
}

function expFieldEl(f) {
  const box = document.createElement('div');
  box.className = 'exp-field' + (f.full ? ' full' : '');
  let inner = '<label>' + f.label + '</label>';
  if (f.type === 'select') {
    inner += '<select data-k="' + f.k + '">' + (f.opts || []).map(o =>
      '<option value="' + (Array.isArray(o) ? o[0] : o) + '">' + (Array.isArray(o) ? o[1] : o) + '</option>'
    ).join('') + '</select>';
  } else if (f.type === 'checkbox') {
    inner = '<label class="exp-check"><input type="checkbox" data-k="' + f.k + '"> ' + f.label + '</label>';
  } else if (f.type === 'color') {
    inner += '<input type="color" data-k="' + f.k + '">';
  } else if (f.type === 'number') {
    inner += '<input type="number" data-k="' + f.k + '" min="' + (f.min != null ? f.min : '') + '" max="' + (f.max != null ? f.max : '') + '" step="' + (f.step != null ? f.step : '1') + '">';
  } else {
    inner += '<input type="text" data-k="' + f.k + '">';
  }
  box.innerHTML = inner;
  return box;
}

function applyExportOptionsToDom() {
  const opts = state.export.options || {};
  document.querySelectorAll('#export-opts [data-k]').forEach(el => {
    const v = expGet(opts, el.dataset.k);
    if (v === undefined || v === null) return;
    if (el.type === 'checkbox') el.checked = !!v;
    else el.value = v;
  });
}

function collectExportOptions() {
  const opts = expDeepMerge(state.export.defaults, {});
  document.querySelectorAll('#export-opts [data-k]').forEach(el => {
    let v;
    if (el.type === 'checkbox') v = el.checked;
    else if (el.type === 'number') v = parseFloat(el.value);
    else v = el.value;
    expSet(opts, el.dataset.k, v);
  });
  return opts;
}

function renderExportPresetSelect() {
  const sel = $('exp-preset');
  sel.textContent = '';
  const names = Object.keys(state.export.presets || {}).concat(Object.keys(state.export.custom || {}));
  sel.appendChild(new Option('自定义', '__custom__'));
  names.forEach(n => {
    sel.appendChild(new Option(EXPORT_PRESET_NAMES[n] || n, n));
  });
  sel.value = '__custom__';
  sel.onchange = () => {
    const v = sel.value;
    if (v === '__custom__') return;
    const preset = (state.export.presets[v] || state.export.custom[v] || {});
    state.export.options = expDeepMerge(state.export.defaults, preset);
    renderExportSections();
  };
}

async function runExport() {
  const fmt = state.export.fmt;
  const options = collectExportOptions();
  const payload = {
    content: currentExportContent(),
    baseDir: state.dir || '',
    suggestedName: currentExportName(),
    options: options,
  };
  busy(true);
  let r = null;
  try {
    r = await py.export_doc(fmt, payload);
  } catch (e) { showToast('导出失败：' + e.message); busy(false); return; }
  busy(false);
  if (!r) { showToast('导出失败'); return; }
  if (r.canceled) return;
  if (!r.ok) { showToast('导出失败：' + (r.error || '未知错误')); return; }
  const res = $('export-result');
  res.textContent = '已导出：' + r.path;
  res.className = 'export-result ok';
  $('export-open').classList.remove('hidden');
  $('export-reveal').classList.remove('hidden');
  $('export-open').onclick = () => py.open_path(r.path);
  $('export-reveal').onclick = () => py.reveal_path(r.path);
  try { py.save_export_presets({ last: { fmt: fmt, options: options } }); } catch (e) { /* ignore */ }
  if (r.warns && r.warns.length) showToast('导出完成，' + r.warns.length + ' 条提示');
  else showToast('导出成功');
}

async function expSavePreset() {
  const box = $('exp-save-name');
  box.classList.remove('hidden');
  const input = $('exp-save-input');
  input.value = '';
  input.focus();
  $('exp-save-ok').onclick = async () => {
    const name = input.value.trim();
    if (!name) { showToast('请输入预设名称'); return; }
    if (EXPORT_PRESET_NAMES[name]) { showToast('名称与内置预设冲突'); return; }
    state.export.custom[name] = collectExportOptions();
    try { await py.save_export_presets({ custom: state.export.custom }); } catch (e) { /* ignore */ }
    renderExportPresetSelect();
    box.classList.add('hidden');
    showToast('预设已保存：' + name);
  };
  $('exp-save-cancel').onclick = () => box.classList.add('hidden');
}

  $('btn-print').addEventListener('click', openExportModal);

  // 导出面板事件
  $('export-close').addEventListener('click', closeExportModal);
  $('export-modal').addEventListener('click', e => { if (e.target === $('export-modal')) closeExportModal(); });
  document.querySelectorAll('.exp-fmt').forEach(btn => btn.addEventListener('click', () => {
    document.querySelectorAll('.exp-fmt').forEach(b => { b.classList.toggle('active', b === btn); b.setAttribute('aria-selected', b === btn ? 'true' : 'false'); });
    state.export.fmt = btn.dataset.fmt;
    renderExportSections();
  }));
  $('export-print').addEventListener('click', () => window.print());
  $('export-run').addEventListener('click', runExport);
  $('exp-save-preset').addEventListener('click', expSavePreset);
  $('exp-reset').addEventListener('click', () => {
    state.export.options = expDeepMerge(state.export.defaults, {});
    renderExportSections();
    const sel = $('exp-preset'); if (sel) sel.value = '__custom__';
  });
  $('export-box').addEventListener('keydown', e => {
    if (e.key === 'Escape' && !$('export-modal').classList.contains('hidden')) { e.stopPropagation(); closeExportModal(); }
  });
  $('btn-assoc').addEventListener('click', installAssoc);
  $('top-btn').addEventListener('click', () => { $('content').scrollTo({ top: 0, behavior: 'smooth' }); });
  $('recent-clear').addEventListener('click', clearRecent);
  $('history-clear').addEventListener('click', clearRecent);
  $('history-close').addEventListener('click', () => $('history-modal').classList.add('hidden'));
  $('history-modal').addEventListener('click', e => { if (e.target === $('history-modal')) $('history-modal').classList.add('hidden'); });

  $('search-close').addEventListener('click', closeSearch);
  $('search-next').addEventListener('click', () => jumpToMark(1));
  $('search-prev').addEventListener('click', () => jumpToMark(-1));
  $('search-input').addEventListener('input', e => doSearch(e.target.value));
  $('search-input').addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); jumpToMark(e.shiftKey ? -1 : 1); }
  });

  $('btn-ai').addEventListener('click', toggleAiPanel);
  $('w-ai').addEventListener('click', toggleAiPanel);
  $('ai-close').addEventListener('click', () => { $('ai-panel').classList.add('hidden'); });
  $('ai-provider').addEventListener('change', onAiProviderChange);
  $('ai-provider-new').addEventListener('click', newAiProvider);
  $('ai-provider-delete').addEventListener('click', deleteAiProvider);
  $('ai-mode').addEventListener('change', () => { /* 协议变更由保存设置时生效 */ });
  $('ai-url-reset').addEventListener('click', resetAiUrl);
  $('ai-key-toggle').addEventListener('click', toggleAiKey);
  $('ai-key-clear').addEventListener('click', clearAiKey);
  $('ai-models-btn').addEventListener('click', loadAiModels);
  $('ai-save-key').addEventListener('click', saveAiSelection);
  document.querySelectorAll('.ai-act').forEach(b => b.addEventListener('click', () => runAi(b.dataset.act)));
  $('ai-run').addEventListener('click', () => runAi('ask'));
  $('ai-stop').addEventListener('click', () => { if (state.ai.aborter) state.ai.aborter.abort(); });
  $('ai-apply').addEventListener('click', applyAi);
  $('ai-copy').addEventListener('click', copyAi);
  $('ai-saveas').addEventListener('click', saveAiAs);
  $('ai-prompt').addEventListener('keydown', e => { if (e.key === 'Enter') $('ai-run').click(); });
  $('ai-template').addEventListener('change', onAiTemplateChange);
  $('ai-tpl-btn').addEventListener('click', openTplModal);
  $('tpl-new').addEventListener('click', () => selectTpl(null));
  $('tpl-save').addEventListener('click', saveTplForm);
  $('tpl-del').addEventListener('click', deleteTplForm);
  $('tpl-close').addEventListener('click', () => $('tpl-modal').classList.add('hidden'));
  $('tpl-modal').addEventListener('click', e => { if (e.target === $('tpl-modal')) $('tpl-modal').classList.add('hidden'); });
  $('ai-session').addEventListener('change', onAiSessionChange);
  $('ai-save-session').addEventListener('click', saveCurrentSession);
  $('ai-del-session').addEventListener('click', deleteCurrentSession);
  $('ai-clear-ctx').addEventListener('click', clearAiContext);
  bindAiResize();

  $('btn-share').addEventListener('click', openShareModal);
  $('share-start').addEventListener('click', startShare);
  $('share-stop').addEventListener('click', stopShare);
  $('share-close').addEventListener('click', () => { $('share-modal').classList.add('hidden'); });
  $('share-modal').addEventListener('click', e => { if (e.target === $('share-modal')) $('share-modal').classList.add('hidden'); });

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
    else if (mod && !e.shiftKey && e.key.toLowerCase() === 'p') { e.preventDefault(); openExportModal(); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'a') { e.preventDefault(); toggleAiPanel(); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'p' && state.editing) { e.preventDefault(); openMdCommandPalette(); }
    else if (mod && (e.key === '=' || e.key === '+')) { e.preventDefault(); zoom(10); }
    else if (mod && e.key === '-') { e.preventDefault(); zoom(-10); }
    else if (mod && e.key === 'ArrowLeft') { e.preventDefault(); historyBack(); }
    else if (mod && e.key === 'ArrowRight') { e.preventDefault(); historyForward(); }
    else if (e.key === 'Escape') {
      if (!$('md-command-modal').classList.contains('hidden')) { closeMdCommandPalette(); return; }
      if (!$('formula-modal').classList.contains('hidden')) { closeFormulaModal(); return; }
      if (!$('img-modal').classList.contains('hidden')) { closeImgModal(); return; }
      if (!$('history-modal').classList.contains('hidden')) { $('history-modal').classList.add('hidden'); return; }
      if (moreMenu && moreMenu.classList.contains('open')) { moreMenu.classList.remove('open'); }
      closeSearch();
      $('fix-modal').classList.add('hidden');
      closeWebDialog();
      $('ai-panel').classList.add('hidden');
      $('share-modal').classList.add('hidden');
      $('tpl-modal').classList.add('hidden');
      $('convert-modal').classList.add('hidden');
      closeMdCommandPalette(); closeFormulaModal(); closeMdPopups();
      stopConvertPoll();
      if (state.editing) exitEdit();
    }
  });
  document.addEventListener('click', closeMdPopups);
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
  bindPy();
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
  finishInit();
}

function finishInit() {
  if (hasPy) {
    if (py.report_ready) { try { py.report_ready(); } catch (e) { /* ignore */ } }
    window.__trayOpenFile = loadFileDialog;
  }
  startControlPoll();
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

window.addEventListener('pywebviewready', async () => {
  const upgraded = !hasPy && bindPy();
  if (upgraded) {
    await loadSettings();
    refreshRecent();
    finishInit();
  }
});
window.addEventListener('DOMContentLoaded', init);
window.addEventListener('beforeunload', () => {
  if (state.file && $('content')) {
    state.scrollPos[normalizePath(state.file)] = $('content').scrollTop;
  }
});
