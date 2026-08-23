'use strict';
/* ============================================================
   ReadMD Core - State & Basic Utilities
   ============================================================ */

const $ = id => document.getElementById(id);

function preferredScrollBehavior() {
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth';
}

let py = (window.pywebview && window.pywebview.api) ? window.pywebview.api : null;
let hasPy = !!py;
const moduleLoadRequests = Object.create(null);

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
const APP_TOKEN = document.querySelector('meta[name="readmd-app-token"]')?.content || null;

function apiFetch(url, opts) {
  opts = opts || {};
  if (LAN_TOKEN) {
    opts.headers = Object.assign({}, opts.headers || {}, { 'X-ReadMD-Token': LAN_TOKEN });
  }
  if (APP_TOKEN) {
    opts.headers = Object.assign({}, opts.headers || {}, { 'X-ReadMD-App-Token': APP_TOKEN });
  }
  return fetch(url, opts);
}

const MD_RE = /\.(md|markdown|mdown|mkd|mdx|txt)$/i;
const IMG_RE = /\.(png|jpe?g|bmp|webp|gif|tiff?)$/i;
const CONVERT_EXTS = ['.docx', '.doc', '.pptx', '.ppt', '.xlsx', '.xls', '.pdf', '.html', '.htm', '.epub', '.mobi', '.rtf', '.odt', '.csv', '.tsv', '.json', '.xml', '.yaml', '.yml', '.rst', '.tex', '.latex'];

const state = {
  tabs: [],            // 多标签列表：{ id, mode, source, path, dir, name, title, content, original, fixed, fixes, stats, size, mtime, encoding, webAssets, isDirty, scrollPos, isVirtual }
  activeTabId: null,   // 当前活动标签 ID
  file: null,          // 当前真实文件路径（虚拟文档为 null）
  dir: null,
  mtime: 0,
  size: 0,
  encoding: '',
  fixes: [],
  stats: null,
  original: '',
  fixed: '',
  mode: 'welcome',     // file | virtual | welcome
  source: '',          // file | convert | ocr | url | clipboard
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
  pagination: {
    enabled: false,       // 是否为超长文档并激活分页逻辑
    mode: 'paged',        // 'paged' | 'continuous'
    pages: [],            // [{ index, title, startLine, endLine, content, headingIds }]
    currentPage: 0,       // 当前页码 (0-indexed)
    totalPages: 0,
    allHeadings: [],      // 全文大纲目录项 [{ id, text, level, pageIndex }]
    rawContent: '',       // 原始全文内容备份
  },
};
window.state = state;


/* ---------------- 工具 ---------------- */

function showToast(msg, ms) {
  const t = $('toast');
  if (window.i18n && typeof msg === 'string') {
    msg = window.i18n.t(msg);
  }
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
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!hasPy) { showToast(_t('toast.assocBrowserNotice') || '浏览器模式下请在命令行运行 install.bat'); return; }
  py.install_association().then(ok => {
    showToast(ok === true ? (_t('toast.assocSuccess') || '已设置为 .md 默认打开方式') : ((_t('toast.assocFailed') || '注册失败：') + ok));
  });
}

