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
  ai: {
    config: null, providers: [], busy: false, aborter: null, raw: '',
    templates: [], templateId: '', messages: [], sessionId: null, sessions: [],
  },
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
  $('btn-ai').disabled = !ready('ai');
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
  rec.slice(0, 12).forEach(p => {
    const li = document.createElement('li');
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'recent-card';
    const name = String(p).split(/[\\/]/).pop() || p;
    const dir = String(p).slice(0, String(p).length - name.length).replace(/[\\/]+$/, '') || '';
    const nm = document.createElement('span');
    nm.className = 'recent-name';
    nm.textContent = name;
    nm.title = p;
    const dp = document.createElement('span');
    dp.className = 'recent-dir';
    dp.textContent = dir;
    dp.title = p;
    btn.appendChild(nm);
    btn.appendChild(dp);
    btn.addEventListener('click', e => { e.preventDefault(); loadFile(p); });
    li.appendChild(btn);
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
    const r = await apiFetch('/api/file?p=' + encodeURIComponent(path));
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
  clearAiOutput();
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
    renderVirtual('convert', d.name, d.dir, d.content, d.fixes);
  } catch (e) { showToast('转换失败：' + e.message); }
  finally { busy(false); }
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

async function webToMd(url, crawl) {
  if (!url) return;
  if (!(await ensureModule('web'))) return;
  busy(true);
  try {
    const r = await apiFetch('/api/url?u=' + encodeURIComponent(url) + '&crawl=' + (crawl ? '1' : '0'));
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
  const p = $('ai-panel');
  p.classList.toggle('hidden');
  if (!p.classList.contains('hidden') && !state.ai.config) loadAiConfig();
  else if (!p.classList.contains('hidden')) { loadAiPrompts(); loadAiSessions(); }
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
    if (s.provider && [...$('ai-provider').options].some(o => o.value === s.provider)) {
      $('ai-provider').value = s.provider;
      onAiProviderChange();
      if (s.model && [...$('ai-model').options].some(o => o.value === s.model)) $('ai-model').value = s.model;
      syncAiKey();
    }
    state.ai.messages = s.messages || [];
    state.ai.sessionId = s.id;
    state.ai.raw = '';
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
      tag.textContent = 'AI · 回答 ' + aSeq;
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
  try {
    const r = await apiFetch('/api/ai/config');
    if (!r.ok) return null;
    state.ai.config = await r.json();
    const cfg = state.ai.config;
    state.ai.providers = [...(cfg.custom || []), ...(cfg.presets || [])];
    fillAiProviders(state.ai.providers, cfg.current || {});
    loadAiPrompts();
    loadAiSessions();
    return cfg;
  } catch (e) { /* ignore */ return null; }
}

function fillAiProviders(merged, current) {
  const sel = $('ai-provider');
  const curName = (current && current.provider) || (merged[0] && merged[0].name) || '';
  sel.innerHTML = '';
  merged.forEach(p => {
    const o = document.createElement('option');
    o.value = p.name;
    o.textContent = p.name + (p.custom ? ' (自定义)' : '');
    sel.appendChild(o);
  });
  if (curName) sel.value = curName;
  onAiProviderChange();
  const curModel = (current && current.model) || '';
  if (curModel && $('ai-model').value !== curModel && [...($('ai-model').options || [])].some(o => o.value === curModel)) {
    $('ai-model').value = curModel;
  }
  syncAiKey();
}

function currentAiProvider() {
  const name = $('ai-provider').value;
  return (state.ai.providers || []).find(p => p.name === name) || null;
}

function onAiProviderChange() {
  const p = currentAiProvider();
  const m = $('ai-model');
  m.innerHTML = '';
  if (!p) { syncAiKey(); return; }
  (p.models || ['']).forEach(md => {
    const o = document.createElement('option');
    o.value = md;
    o.textContent = md;
    m.appendChild(o);
  });
  const curModel = (state.ai.config && state.ai.config.current && state.ai.config.current.model) || '';
  if (curModel && (p.models || []).includes(curModel)) m.value = curModel;
  syncAiKey();
}

function syncAiKey() {
  const p = currentAiProvider();
  const inp = $('ai-key');
  const usage = $('ai-usage');
  if (!p) { inp.value = ''; inp.placeholder = ''; usage.textContent = ''; return; }
  inp.value = p.api_key || '';
  inp.placeholder = (p.key_source && p.key_source.indexOf('env:') === 0)
    ? '已从环境变量 ' + p.key_source.slice(4) + ' 读取，可覆盖'
    : 'API Key（留空则读取环境变量）';
  usage.textContent = p.has_key
    ? (p.key_source ? 'Key 就绪（' + p.key_source + '）' : 'Key 已配置')
    : '未配置 Key';
}

async function saveAiSelection() {
  const p = currentAiProvider();
  if (!p || !state.ai.config) return;
  const custom = (state.ai.config.custom || []).map(c => Object.assign({}, c));
  const keyVal = $('ai-key').value.trim();
  if (p.custom) {
    const t = custom.find(c => c.name === p.name);
    if (t) { if (keyVal) t.api_key = keyVal; else delete t.api_key; }
  } else {
    let over = custom.find(c => c.name === p.name);
    if (!over) {
      over = Object.assign({}, p);
      delete over.has_key; delete over.key_source;
      over.custom = true; over.api_key = '';
      custom.push(over);
    }
    if (keyVal) over.api_key = keyVal; else delete over.api_key;
  }
  const current = { provider: p.name, model: $('ai-model').value };
  try {
    await apiFetch('/api/ai/config', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ providers: custom, current }),
    });
  } catch (e) { /* ignore */ }
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

async function runAi(action) {
  const p = currentAiProvider();
  if (!p) { showToast('请先选择 AI 提供商'); return; }
  const keyVal = $('ai-key').value.trim();
  if (!p.has_key && !keyVal) { showToast('未配置 API Key（可填写或设置环境变量）'); return; }
  const { text, isSelection } = getAiTargetText();
  if (!text || !text.trim()) { showToast('没有可处理的文档内容'); return; }
  const prompt = $('ai-prompt').value.trim();
  const model = $('ai-model').value || (p.models || [''])[0] || '';
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
  uTag.textContent = '我 · 提问 ' + userSeq + ' · ' + (AI_ACTIONS[action] || action) + (isSelection ? '（选中文字）' : '（全文）') + ' · ' + p.name + ' · ' + model;
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
        provider: p.name, model: model, api_key: keyVal || undefined,
        messages: [{ role: 'system', content: sys }].concat(msgs),
        stream: true,
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
        if (obj.d === undefined) continue;
        state.ai.raw += obj.d;
        if (!renderTimer) renderTimer = setTimeout(render, state.ai.raw.length > 150000 ? 500 : 120);
      }
    }
    if (renderTimer) { clearTimeout(renderTimer); renderTimer = null; render(); }
    renderMath(aiBody);
    aiTag.textContent = 'AI · 回答 ' + userSeq;
    if (state.ai.raw) {
      msgs.push({ role: 'assistant', content: state.ai.raw });
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
        msgs.push({ role: 'assistant', content: state.ai.raw });
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
    ],
  });
  cmView = new CM.EditorView({ state: st, parent: $('edit-cm') });
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
    case 'h2': insert = '## ' + (selected || '标题'); cursor = sel.from + insert.length; break;
    case 'quote': insert = '> ' + (selected || '引用'); cursor = sel.from + insert.length; break;
    case 'list': insert = '- ' + (selected || '项目'); cursor = sel.from + insert.length; break;
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

/* ---------------- 图片编辑器（插入 / 裁剪 / 缩放 / 旋转） ---------------- */

const imgState = {
  img: null, rawW: 0, rawH: 0,
  angle: 0, scale: 100, ratio: 'free',
  rotW: 0, rotH: 0, fitScale: 1, offX: 0, offY: 0,
  crop: { x: 0, y: 0, w: 0, h: 0 },
  drag: null, // { mode:'move'|'resize', sx, sy, cx, cy, cw, ch }
};

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
  imgState.fitScale = Math.min(cw / imgState.rotW, ch / imgState.rotH);
  imgState.offX = (cw - imgState.rotW * imgState.fitScale) / 2;
  imgState.offY = (ch - imgState.rotH * imgState.fitScale) / 2;
  const tmp = document.createElement('canvas');
  tmp.width = imgState.rotW;
  tmp.height = imgState.rotH;
  const tctx = tmp.getContext('2d');
  tctx.translate(imgState.rotW / 2, imgState.rotH / 2);
  tctx.rotate(imgState.angle * Math.PI / 180);
  tctx.drawImage(imgState.img, -imgState.rawW / 2, -imgState.rawH / 2, imgState.rawW, imgState.rawH);
  ctx.drawImage(tmp, imgState.offX, imgState.offY, imgState.rotW * imgState.fitScale, imgState.rotH * imgState.fitScale);
  clampCrop();
  updateCropUI();
  updateImgInfo();
}

function ratioValue() {
  if (imgState.ratio === '1:1') return 1;
  if (imgState.ratio === '4:3') return 4 / 3;
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
  const ow = Math.max(1, Math.round(imgState.crop.w / imgState.fitScale * imgState.scale / 100));
  const oh = Math.max(1, Math.round(imgState.crop.h / imgState.fitScale * imgState.scale / 100));
  el.textContent = '原图 ' + imgState.rawW + '×' + imgState.rawH + ' · 角度 ' + imgState.angle + '° · 缩放 ' + imgState.scale + '% · 输出 ' + ow + '×' + oh + ' px';
}

function resetImg() {
  imgState.angle = 0;
  imgState.scale = 100;
  imgState.ratio = 'free';
  $('img-angle').value = 0;
  $('img-scale').value = 100;
  $('img-scale-val').textContent = '100%';
  $('img-ratio').value = 'free';
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

function rotateImg(delta) {
  if (!imgState.img) return;
  imgState.angle = ((imgState.angle + delta) % 360 + 360) % 360;
  $('img-angle').value = imgState.angle;
  drawImg();
}

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
  const cropEl = $('img-crop');
  const handle = e.target && e.target.classList && e.target.classList.contains('crop-handle');
  const inCrop = px >= imgState.crop.x - 4 && px <= imgState.crop.x + imgState.crop.w + 4 &&
                 py >= imgState.crop.y - 4 && py <= imgState.crop.y + imgState.crop.h + 4;
  if (handle || (inCrop && !e.shiftKey)) {
    imgState.drag = handle
      ? { mode: 'resize', sx: px, sy: py, cx: imgState.crop.x, cy: imgState.crop.y, cw: imgState.crop.w, ch: imgState.crop.h }
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
  if (d.mode === 'move') {
    let nx = d.cx + (px - d.sx);
    let ny = d.cy + (py - d.sy);
    nx = Math.max(r.x, Math.min(nx, r.x + r.w - d.cw));
    ny = Math.max(r.y, Math.min(ny, r.y + r.h - d.ch));
    imgState.crop.x = nx; imgState.crop.y = ny;
  } else if (d.mode === 'resize') {
    let nw = Math.max(24, Math.min(px - d.cx, r.w));
    let nh = Math.max(24, Math.min(py - d.cy, r.h));
    if (rv > 0) {
      if (nw / rv > r.h) { nw = r.h * rv; nh = r.h; }
      else nh = nw / rv;
    }
    imgState.crop.w = nw; imgState.crop.h = nh;
    if (imgState.crop.x + nw > r.x + r.w) imgState.crop.x = r.x + r.w - nw;
    if (imgState.crop.y + nh > r.y + r.h) imgState.crop.y = r.y + r.h - nh;
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
  }
  updateCropUI();
  updateImgInfo();
  e.preventDefault();
}

function stagePointerUp(e) {
  imgState.drag = null;
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
  const outW = Math.max(1, Math.round(srcW * imgState.scale / 100));
  const outH = Math.max(1, Math.round(srcH * imgState.scale / 100));
  const tmp = document.createElement('canvas');
  tmp.width = imgState.rotW;
  tmp.height = imgState.rotH;
  const tctx = tmp.getContext('2d');
  tctx.translate(imgState.rotW / 2, imgState.rotH / 2);
  tctx.rotate(imgState.angle * Math.PI / 180);
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
async function toggleEdit() {
  if (state.editing) { exitEdit(); return; }
  if (state.mode !== 'file' || !state.file) { showToast('仅本地 Markdown 文件可编辑'); return; }
  $('edit-bar').classList.remove('hidden');
  $('content').classList.add('hidden');
  state.editing = true;
  setEditBtn('编辑中');
  try {
    await loadCodeMirror();
  } catch (e) { /* 退回 textarea */ }
  if (window.ReadMDCodeMirror) {
    $('edit-area').classList.add('hidden');
    $('edit-wrap').classList.remove('hidden');
    createEditor(state.original || '');
  } else {
    $('edit-wrap').classList.add('hidden');
    $('edit-area').classList.remove('hidden');
    $('edit-area').value = state.original || '';
    $('edit-area').focus();
  }
}

function exitEdit() {
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
  if (!state.file || !state.editing) return;
  const content = cmView ? cmView.state.doc.toString() : $('edit-area').value;
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
  document.querySelectorAll('#md-tool .md-tool-btn').forEach(b => b.addEventListener('click', () => {
    if (b.dataset.md === 'image') openImgModal();
    else cmInsertSyntax(b.dataset.md);
  }));
  $('edit-save').addEventListener('click', saveEdit);
  $('edit-cancel').addEventListener('click', exitEdit);
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
  $('img-angle').addEventListener('input', e => { imgState.angle = +e.target.value; drawImg(); });
  $('img-scale').addEventListener('input', e => {
    imgState.scale = +e.target.value;
    $('img-scale-val').textContent = imgState.scale + '%';
    updateImgInfo();
  });
  $('img-ratio').addEventListener('change', e => { imgState.ratio = e.target.value; applyRatio(); });
  $('img-reset').addEventListener('click', resetImg);
  $('img-insert').addEventListener('click', exportAndInsertImg);
  $('img-close').addEventListener('click', closeImgModal);
  $('img-modal').addEventListener('click', e => { if (e.target === $('img-modal')) closeImgModal(); });
  const stage = $('img-stage');
  stage.addEventListener('pointerdown', stagePointer);
  stage.addEventListener('pointermove', stagePointerMove);
  stage.addEventListener('pointerup', stagePointerUp);
  stage.addEventListener('pointercancel', stagePointerUp);
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

  $('btn-ai').addEventListener('click', toggleAiPanel);
  $('w-ai').addEventListener('click', toggleAiPanel);
  $('ai-close').addEventListener('click', () => { $('ai-panel').classList.add('hidden'); });
  $('ai-provider').addEventListener('change', onAiProviderChange);
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
    else if (mod && e.key.toLowerCase() === 'p') { e.preventDefault(); openExportModal(); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'a') { e.preventDefault(); toggleAiPanel(); }
    else if (mod && (e.key === '=' || e.key === '+')) { e.preventDefault(); zoom(10); }
    else if (mod && e.key === '-') { e.preventDefault(); zoom(-10); }
    else if (mod && e.key === 'ArrowLeft') { e.preventDefault(); historyBack(); }
    else if (mod && e.key === 'ArrowRight') { e.preventDefault(); historyForward(); }
    else if (e.key === 'Escape') {
      if (moreMenu && moreMenu.classList.contains('open')) { moreMenu.classList.remove('open'); }
      closeSearch();
      $('fix-modal').classList.add('hidden');
      closeWebDialog();
      $('ai-panel').classList.add('hidden');
      $('share-modal').classList.add('hidden');
      $('tpl-modal').classList.add('hidden');
      $('img-modal').classList.add('hidden');
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