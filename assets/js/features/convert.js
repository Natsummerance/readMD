'use strict';
/* ============================================================
   ReadMD Features - Batch File Conversion (All-to-MD)
   ============================================================ */

/* ---------------- 批量转换（转 MD） ---------------- */

const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
let convertLastDir = null;

async function openConvertModal() {
  const note = $('convert-note');
  if (note) note.textContent = state.win7 ? (_t('convert.noteWin7') || 'Win7 版仅支持 docx / pdf 转 Markdown；转换结果自动保存为源文件同目录同名 .md。') : (_t('convert.note') || '转换结果自动保存为源文件同目录同名 .md（如 report.docx → report.md）。docx 公式、PDF 表格走专用解析，其余格式自动回退通用转换；输出经过严格校验（表格 / 代码围栏 / 公式 / 图片引用）。');
  $('convert-modal').classList.remove('hidden');
  $('convert-list').innerHTML = '';
  $('convert-status').textContent = '';
  $('batch-cancel')?.classList.add('hidden');
  $('convert-open-dir').classList.add('hidden');
}

function closeConvertModal() {
  if (typeof stopBatchPoll === 'function') stopBatchPoll();
  $('convert-modal').classList.add('hidden');
}

async function pickConvertFiles() {
  let files = [];
  if (hasPy) {
    try { files = await py.choose_many_files(); } catch (e) { files = []; }
  } else {
    const input = $('file-input');
    if (!input) return;
    input.value = '';
    input.multiple = true;
    input.onchange = async () => {
      const uploaded = [];
      for (const file of Array.from(input.files || [])) {
        const path = await uploadFile(file);
        if (path) uploaded.push(path);
      }
      if (uploaded.length) await startBatchConvert(uploaded, $('convert-overwrite').checked);
    };
    input.click();
    return;
  }
  if (files.length) await startBatchConvert(files, $('convert-overwrite').checked);
}

async function pickConvertFolder() {
  let dir = null;
  try { dir = await py.choose_folder(); } catch (e) { dir = null; }
  if (!dir) return;
  try {
    const r = await apiFetch('/api/convert/collect?dir=' + encodeURIComponent(dir));
    const d = await r.json();
    if (!r.ok) throw new Error(d.error || (_t('convert.statusError') || '收集失败'));
    const files = d.files || [];
    if (!files.length) { showToast(_t('convert.noConvertibleFiles') || '该目录下没有可转换的文件'); return; }
    convertLastDir = dir;
    await startBatchConvert(files, $('convert-overwrite').checked);
  } catch (e) { showToast((_t('toast.collectFilesFail') || '收集文件失败：') + e.message); }
}

async function startBatchConvert(files, overwrite) {
  if (typeof enqueueBatchFiles === 'function') {
    return enqueueBatchFiles(files, overwrite);
  }
  // The batch module is part of the generated boot bundle.  Keep a stable
  // error instead of maintaining a second conversion implementation when a
  // custom host accidentally omits it.
  showToast(_t('convert.moduleUnavailable'));
}





async function ocrFile(path) {
  if (!(await ensureModule('ocr'))) return;
  busy(true);
  try {
    const r = await apiFetch('/api/ocr?p=' + encodeURIComponent(path));
    const d = await r.json();
    if (r.status === 409) { showToast(d.error || (_t('toast.moduleLoading') || '模块加载中…')); return; }
    if (!r.ok) { showToast(d.error || (_t('toast.ocrFail') || 'OCR 失败')); return; }
    if (!d.content) { showToast(d.note || (_t('toast.ocrNoText') || '未识别到文字')); return; }
    renderVirtual('ocr', d.name, d.dir, d.content, d.fixes);
  } catch (e) { showToast((_t('toast.ocrFailPrefix') || 'OCR 失败：') + e.message); }
  finally { busy(false); }
}

/* ---------------- 文件选择（含浏览器兜底） ---------------- */

function chooseFile(mode) {
  if (moduleBlocked(mode)) return;
  if (hasPy) {
    if (mode === 'ocr') {
      py.choose_many_files().then(async files => {
        if (!files || !files.length) return;
        if (files.length === 1) {
          convertOrOcr(files[0], 'ocr');
        } else {
          showToast(_t('toast.batchOcrStarting', { count: files.length }) || `已选择 ${files.length} 个文件，正在进行批量 OCR 识别…`, 3000);
          for (let i = 0; i < files.length; i++) {
            await ocrFile(files[i]);
          }
          showToast(_t('toast.batchOcrComplete', { count: files.length }) || `批量 OCR 完成，已识别 ${files.length} 个文件并新建标签页`);
        }
      });
      return;
    }
    py.choose_any_file().then(p => { if (p) convertOrOcr(p, mode); });
    return;
  }
  const input = $('file-input');
  input.value = '';
  input.multiple = (mode === 'ocr');
  input.onchange = async () => {
    const files = Array.from(input.files || []);
    if (!files.length) return;
    if (files.length === 1) {
      const p = await uploadFile(files[0]);
      if (p) convertOrOcr(p, mode);
    } else {
      showToast(_t('toast.batchUploadStarting', { count: files.length }) || `正在批量上传并识别 ${files.length} 个文件…`, 3000);
      for (const f of files) {
        const p = await uploadFile(f);
        if (p) await convertOrOcr(p, mode);
      }
      showToast(_t('toast.batchUploadComplete', { count: files.length }) || `批量 OCR 完成，已处理 ${files.length} 个文件`);
    }
  };
  input.click();
}


async function uploadFile(file) {
  const fileName = file.name || 'document.bin';
  const ext = '.' + (fileName.split('.').pop() || 'bin');
  try {
    const qs = '?ext=' + encodeURIComponent(ext) + '&name=' + encodeURIComponent(fileName);
    const r = await apiFetch('/api/upload' + qs, { method: 'POST', body: file });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.error || `HTTP ${r.status}`);
    return d.path || null;
  } catch (e) {
    showToast((_t('toast.uploadFailed') || '上传失败：') + (e.message || e));
    return null;
  }
}

function convertOrOcr(p, mode) {
  if (mode === 'ocr' || (mode !== 'convert' && IMG_RE.test(p))) ocrFile(p);
  else convertFile(p);
}


/* ---------------- 插件管理中心 (Plugin Center) ---------------- */

let pluginPollTimer = null;

async function openPluginModal() {
  const modal = $('plugin-modal');
  if (!modal) return;
  modal.classList.remove('hidden');
  await refreshPluginList();
}

function closePluginModal() {
  const modal = $('plugin-modal');
  if (modal) modal.classList.add('hidden');
  if (pluginPollTimer) {
    clearInterval(pluginPollTimer);
    pluginPollTimer = null;
  }
}

async function refreshPluginList() {
  const grid = $('plugin-cards-grid');
  const ffmpegBadge = $('plugin-ffmpeg-badge');
  const sandboxPath = $('plugin-sandbox-path');
  if (!grid) return;

  try {
    const res = await apiFetch('/api/plugins/list');
    const data = await res.json();
    if (!data.ok) throw new Error(data.error || 'Failed to list plugins');

    if (ffmpegBadge) {
      if (data.ffmpeg) {
        ffmpegBadge.textContent = _t('plugin.ffmpegReady') || '已就绪';
        ffmpegBadge.className = 'plugin-badge ready';
      } else {
        ffmpegBadge.textContent = _t('plugin.ffmpegMissing') || '未检测到（音视频转写建议配置）';
        ffmpegBadge.className = 'plugin-badge missing';
      }
    }

    if (sandboxPath && data.sandbox_dir) {
      sandboxPath.textContent = data.sandbox_dir;
      sandboxPath.title = data.sandbox_dir;
    }

    renderPluginCards(data.plugins || {});

    // 如果有安装任务进行中，保持轮询
    const hasInstalling = Object.values(data.plugins || {}).some(p => p.installing);
    if (hasInstalling && !pluginPollTimer) {
      pluginPollTimer = setInterval(refreshPluginList, 1500);
    } else if (!hasInstalling && pluginPollTimer) {
      clearInterval(pluginPollTimer);
      pluginPollTimer = null;
    }
  } catch (err) {
    console.error('refreshPluginList error:', err);
  }
}

const PLUGIN_ICONS = {
  whisper: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" x2="12" y1="19" y2="22"/></svg>',
  audio: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" x2="12" y1="19" y2="22"/></svg>',
  rapidocr: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/><line x1="7" x2="17" y1="9" y2="9"/><line x1="12" x2="12" y1="9" y2="17"/><line x1="9" x2="15" y1="17" y2="17"/></svg>',
  easyocr: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/><line x1="7" x2="17" y1="9" y2="9"/><line x1="12" x2="12" y1="9" y2="17"/><line x1="9" x2="15" y1="17" y2="17"/></svg>',
  ocr: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/><line x1="7" x2="17" y1="9" y2="9"/><line x1="12" x2="12" y1="9" y2="17"/><line x1="9" x2="15" y1="17" y2="17"/></svg>',
  rapid_table: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 9h18"/><path d="M3 15h18"/><path d="M9 3v18"/><path d="M15 3v18"/></svg>',
  pylatexenc: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m4 8 2.5 8 3-12 3 12 2.5-8"/><line x1="17" x2="21" y1="12" y2="12"/><line x1="17" x2="21" y1="16" y2="16"/></svg>',
  latex: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m4 8 2.5 8 3-12 3 12 2.5-8"/><line x1="17" x2="21" y1="12" y2="12"/><line x1="17" x2="21" y1="16" y2="16"/></svg>',
  jieba: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="6" r="3"/><circle cx="18" cy="6" r="3"/><circle cx="12" cy="18" r="3"/><line x1="8.5" x2="15.5" y1="7.5" y2="7.5"/><line x1="7.5" x2="10.5" y1="8.5" y2="15.5"/><line x1="16.5" x2="13.5" y1="8.5" y2="15.5"/></svg>',
  pygments: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>',
  default: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m7.5 4.27 9 5.15"/><path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/></svg>'
};

function getPluginIconSvg(id, category) {
  return PLUGIN_ICONS[id] || PLUGIN_ICONS[category] || PLUGIN_ICONS.default;
}

// 每个错误码对应一个字面量 _t() 调用：key 只有在调用点写成字面量时
// tools/check_js_i18n_keys.py 才能静态校验，变量形式的 _t(key) 会绕过门禁。
const PLUGIN_ERROR_TEXT = {
  pip_network: () => _t('plugin.error.pip_network'),
  pip_timeout: () => _t('plugin.error.pip_timeout'),
  pip_permission: () => _t('plugin.error.pip_permission'),
  pip_no_distribution: () => _t('plugin.error.pip_no_distribution'),
  pip_unavailable: () => _t('plugin.error.pip_unavailable'),
  pip_unknown: () => _t('plugin.error.pip_unknown'),
  uninstall_locked: () => _t('plugin.error.uninstall_locked'),
};

// 后缀来自 p.category / 插件 id 等运行时变量，_t() 只能拼出前缀，
// 所以这里逐类写成字面量调用，否则 tools/check_js_i18n_keys.py 看不到。
const PLUGIN_CATEGORY_TEXT = {
  audio: () => _t('plugin.category.audio'),
  code: () => _t('plugin.category.code'),
  document: () => _t('plugin.category.document'),
  latex: () => _t('plugin.category.latex'),
  ocr: () => _t('plugin.category.ocr'),
  text: () => _t('plugin.category.text'),
  tools: () => _t('plugin.category.tools'),
};

// i18n.t() 查不到 key 时返回 key 本身，所以 `_t(k) || '中文'` 结构上永远兜不了底，
// 只会把 `plugin.xxx` 打到 46 种语言的界面上。这里显式判「是否未命中」再退回后端值。
function translatePluginText(key, fallback) {
  const text = _t(key);
  return text === key ? (fallback || '') : text;
}

function pluginErrorMarkup(p) {
  const resolve = PLUGIN_ERROR_TEXT[p.install_error_code] || PLUGIN_ERROR_TEXT.pip_unknown;
  const detail = p.install_error_detail || '';
  const tip = `<span class="plugin-error-msg">${escapeHtml(resolve())}</span>`;
  if (!detail) return `<div class="plugin-error-wrap">${tip}</div>`;
  return `
    <div class="plugin-error-wrap">
      ${tip}
      <details class="plugin-error-detail">
        <summary>${escapeHtml(_t('plugin.errorDetails'))}</summary>
        <pre class="plugin-error-raw">${escapeHtml(detail)}</pre>
      </details>
    </div>
  `;
}

function renderPluginCards(plugins) {
  const grid = $('plugin-cards-grid');
  if (!grid) return;
  grid.innerHTML = '';

  for (const [id, p] of Object.entries(plugins)) {
    const card = document.createElement('div');
    card.className = 'plugin-card' + (p.enabled ? ' is-enabled' : '');
    card.dataset.pluginId = id;

    const title = translatePluginText('plugin.' + id + '.name', p.name_key || id);
    const desc = translatePluginText('plugin.' + id + '.desc', p.desc_key);
    const category = PLUGIN_CATEGORY_TEXT[p.category] ? PLUGIN_CATEGORY_TEXT[p.category]() : (p.category || '');
    const isCached = Boolean(p.installed && p.cached);
    const sizeStr = p.approx_size ? p.approx_size : '';
    const metaParts = [];
    if (sizeStr) metaParts.push(sizeStr);
    if (category) metaParts.push(category);
    metaParts.push(isCached ? _t('plugin.cacheReady') : (p.installed ? _t('plugin.installed') : _t('plugin.available')));

    const iconSvg = getPluginIconSvg(id, p.category);

    let footLeft = '';
    let footRight = '';
    let progressHtml = '';

    if (p.installing) {
      const pct = typeof p.progress === 'number' && p.progress > 0 ? Math.min(100, Math.max(0, p.progress)) : null;
      footLeft = `
        <div class="plugin-spinner-row">
          <span class="plugin-spinner" aria-hidden="true"></span>
          <span class="plugin-status-txt installing">${_t('plugin.installing')}</span>
        </div>
      `;
      footRight = pct !== null ? `<span class="plugin-progress-text">${pct}%</span>` : '';
      progressHtml = `
        <div class="plugin-progress-wrap">
          <div class="plugin-progress-track" role="progressbar" aria-valuenow="${pct || 0}" aria-valuemin="0" aria-valuemax="100">
            <div class="plugin-progress-fill ${pct === null ? 'is-indeterminate' : ''}" style="width: ${pct !== null ? pct + '%' : '35%'};"></div>
          </div>
          ${p.last_log ? `<div class="plugin-log-tip" title="${escapeHtml(p.last_log)}">${escapeHtml(p.last_log)}</div>` : ''}
        </div>
      `;
    } else if (p.installed) {
      footLeft = `
        <label class="apple-switch plugin-switch">
          <input type="checkbox" class="plugin-switch-input" ${p.enabled ? 'checked' : ''} data-action="toggle" aria-label="${escapeHtml(title)}">
          <span class="apple-switch-track plugin-switch-track" aria-hidden="true"><span class="apple-switch-thumb plugin-switch-thumb"></span></span>
          <span class="plugin-status-txt ${p.enabled ? 'active' : ''}">${p.enabled ? _t('plugin.enabled') : _t('plugin.disabled')}</span>
        </label>
      `;
      footRight = `
        <button class="plugin-action-uninstall-btn" data-action="uninstall">${_t('plugin.uninstall')}</button>
      `;
      if (p.install_error_code) progressHtml = pluginErrorMarkup(p);
    } else {
      const hasErr = Boolean(p.install_error_code);
      footLeft = `
        <div class="plugin-status-dot-indicator ${hasErr ? 'is-error' : ''}">
          <span class="plugin-dot-pip ${hasErr ? 'error' : ''}"></span>
          <span>${hasErr ? _t('plugin.installFailed') : _t('plugin.notInstalled')}</span>
        </div>
      `;
      footRight = `<button class="plugin-action-get-btn ${hasErr ? 'is-retry' : ''}" data-action="install">${hasErr ? _t('plugin.retry') : _t('plugin.install')}</button>`;
      if (hasErr) progressHtml = pluginErrorMarkup(p);
    }

    card.innerHTML = `
      <div class="plugin-card-head">
        <div class="plugin-app-icon" aria-hidden="true">${iconSvg}</div>
        <div class="plugin-card-meta-wrap">
          <h4 class="plugin-card-title">${escapeHtml(title)}</h4>
          <div class="plugin-card-submeta">
            ${escapeHtml(metaParts.join(' · '))}
          </div>
        </div>
      </div>
      <p class="plugin-card-desc">${escapeHtml(desc)}</p>
      ${progressHtml}
      <div class="plugin-card-foot">
        <div class="plugin-foot-left">${footLeft}</div>
        <div class="plugin-foot-right">${footRight}</div>
      </div>
    `;

    // 绑定卡片内按钮事件
    const toggleInput = card.querySelector('input[data-action="toggle"]');
    if (toggleInput) {
      toggleInput.addEventListener('change', async (e) => {
        await setPluginToggle(id, e.target.checked, title);
      });
    }

    const installBtn = card.querySelector('button[data-action="install"]');
    if (installBtn) {
      installBtn.addEventListener('click', async () => {
        await startPluginInstall(id, title, sizeStr);
      });
    }

    const uninstallBtn = card.querySelector('button[data-action="uninstall"]');
    if (uninstallBtn) {
      uninstallBtn.addEventListener('click', async () => {
        await startPluginUninstall(id, title);
      });
    }

    grid.appendChild(card);
  }
}

async function setPluginToggle(id, enabled, name) {
  try {
    const res = await apiFetch('/api/plugins/toggle', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ plugin_id: id, enabled: Boolean(enabled) }),
    });
    const data = await res.json();
    if (!data.ok) throw new Error(data.error || 'Toggle failed');
    await refreshPluginList();
  } catch (err) {
    console.warn('plugin toggle failed:', err);
    showToast(_t('plugin.toggleFailed', { name: name || id }));
    await refreshPluginList();
  }
}

async function startPluginInstall(id, name, size) {
  const msg = _t('plugin.installConfirm', { name: name || id, size: size || '' }) ||
    `确定要在沙箱中安装「${name || id}」吗？将使用 pip 自动拉取依赖，不污染全局环境。`;

  let confirmed = false;
  if (typeof confirmAction === 'function') {
    confirmed = await confirmAction({
      title: _t('plugin.install') || '安装插件',
      message: msg,
      confirmText: _t('plugin.install') || '安装',
      cancelText: _t('dialog.cancel') || '取消',
    });
  } else {
    confirmed = window.confirm(msg);
  }
  if (!confirmed) return;

  showToast(_t('plugin.startingInstall', { name: name || id }));
  try {
    const res = await apiFetch('/api/plugins/install', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ plugin_id: id }),
    });
    const data = await res.json();
    if (!data.ok) throw new Error(data.error || 'Install request failed');
    await refreshPluginList();
  } catch (err) {
    console.warn('plugin install request failed:', err);
    showToast(_t('plugin.installFail', { name: name || id }));
    await refreshPluginList();
  }
}

async function startPluginUninstall(id, name) {
  const msg = _t('plugin.uninstallConfirm', { name: name || id }) || `确定要从沙箱中卸载「${name || id}」吗？`;

  let confirmed = false;
  if (typeof confirmAction === 'function') {
    confirmed = await confirmAction({
      title: _t('plugin.uninstall') || '卸载插件',
      message: msg,
      confirmText: _t('plugin.uninstall') || '卸载',
      cancelText: _t('dialog.cancel') || '取消',
      danger: true,
    });
  } else {
    confirmed = window.confirm(msg);
  }
  if (!confirmed) return;

  try {
    const res = await apiFetch('/api/plugins/uninstall', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ plugin_id: id }),
    });
    const data = await res.json();
    if (!data.ok) throw new Error(data.error || 'Uninstall request failed');
    showToast(_t('plugin.uninstallSuccess', { name: name || id }));
    await refreshPluginList();
  } catch (err) {
    console.warn('plugin uninstall request failed:', err);
    showToast(_t('plugin.uninstallFail', { name: name || id }));
    await refreshPluginList();
  }
}

