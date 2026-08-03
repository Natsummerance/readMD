'use strict';
/* ReadMD 前端逻辑：渲染、目录、搜索、主题、自动刷新、历史、转换 / 网页 / OCR / 编辑 */

const $ = id => document.getElementById(id);
const py = (window.pywebview && window.pywebview.api) ? window.pywebview.api : null;
const hasPy = !!py;
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

function setFixes(fixes, stats) {
  state.fixes = fixes || [];
  state.stats = stats || {};
  $('btn-fix').textContent = state.fixes.length
    ? '\uD83D\uDEE0 修复 ' + state.fixes.length
    : '\uD83D\uDEE0 修复';
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

function applyAi() {
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
    toggleEdit();
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
    else if (mod && e.key.toLowerCase() === 'p') { e.preventDefault(); window.print(); }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'a') { e.preventDefault(); toggleAiPanel(); }
    else if (mod && (e.key === '=' || e.key === '+')) { e.preventDefault(); zoom(10); }
    else if (mod && e.key === '-') { e.preventDefault(); zoom(-10); }
    else if (mod && e.key === 'ArrowLeft') { e.preventDefault(); historyBack(); }
    else if (mod && e.key === 'ArrowRight') { e.preventDefault(); historyForward(); }
    else if (e.key === 'Escape') {
      closeSearch();
      $('fix-modal').classList.add('hidden');
      closeWebDialog();
      $('ai-panel').classList.add('hidden');
      $('share-modal').classList.add('hidden');
      $('tpl-modal').classList.add('hidden');
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