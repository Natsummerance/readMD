'use strict';
/* ============================================================
   ReadMD Features - Unified Batch Workbench (convert + OCR)
   ============================================================ */

/* ---------------- 批量工作台：文档批量转换 + 图片逐张 OCR ---------------- */

let batchJobTimer = null;
let batchJobId = null;
let batchCancelRequested = false;
let batchOcrCanceled = false;
let batchDocsDone = false;
let batchOcrDone = false;
let batchFinished = false;
let batchCount = { ok: 0, skipped: 0, error: 0, canceled: 0 };
/** 文件夹批量时所选的根目录；“打开结果目录”优先用它。 */
let batchFolderRoot = '';
const batchRowsBySrc = Object.create(null);

function batchT(k, p) {
  return window.i18n ? window.i18n.t(k, p) : k;
}

function openBatchModal() {
  stopBatchPoll();
  setBatchTriggersBusy(false);
  batchJobId = null;
  batchCancelRequested = false;
  batchOcrCanceled = false;
  batchFinished = false;
  batchCount = { ok: 0, skipped: 0, error: 0, canceled: 0 };
  batchFolderRoot = '';
  for (const k in batchRowsBySrc) delete batchRowsBySrc[k];
  const modal = $('convert-modal');
  if (modal) modal.classList.remove('hidden');
  // Batch and single-file conversion share one surface; never leak the
  // single-file "open output" action into a fresh batch run.
  $('convert-open-dir')?.classList.add('hidden');
  $('convert-list').innerHTML = '';
  $('convert-status').textContent = '';
  const note = $('convert-note');
  if (note) note.textContent = batchT('batch.note') || '';
  $('batch-cancel').classList.add('hidden');
}

function closeBatchModal() {
  stopBatchPoll();
  // Polling stops with the surface, so the lock must not outlive it.
  setBatchTriggersBusy(false);
  $('convert-modal').classList.add('hidden');
}

function stopBatchPoll() {
  if (batchJobTimer) { clearInterval(batchJobTimer); batchJobTimer = null; }
}

function makeBatchRow(path) {
  const row = document.createElement('div');
  row.className = 'batch-item queued';
  const nm = document.createElement('span');
  nm.className = 'batch-name';
  nm.textContent = path.split(/[\\/]/).pop();
  nm.title = path;
  const st = document.createElement('span');
  st.className = 'batch-state';
  st.textContent = batchT('batch.statusQueued') || '';
  row.appendChild(nm); row.appendChild(st);
  row.addEventListener('click', async () => {
    if (!row.dataset.done || !row.dataset.out) return;
    closeBatchModal();
    await loadFile(row.dataset.out);
  });
  return row;
}

function setBatchRow(row, status, title) {
  row.className = 'batch-item ' + status;
  const st = row.querySelector('.batch-state');
  if (st) {
    const labels = {
      queued: batchT('batch.statusQueued') || '',
      running: batchT('batch.statusRunning') || '',
      ok: batchT('batch.statusOk') || '',
      skipped: batchT('batch.statusSkipped') || '',
      error: batchT('batch.statusError') || '',
      canceled: batchT('batch.statusCanceled') || '',
    };
    st.textContent = labels[status] || status;
    if (status === 'error' && title) st.title = title;
  }
}

function countBatchRow(row, status) {
  if (row.dataset.done) return;
  row.dataset.done = '1';
  if (status in batchCount) batchCount[status]++;
}

async function enqueueBatchFiles(paths, overwrite) {
  const list = (paths || []).filter(p => typeof p === 'string' && p.trim());
  if (!list.length) return;
  if (list.some(p => IMG_RE.test(p)) && moduleBlocked('ocr')) return;
  if (list.some(p => !IMG_RE.test(p)) && moduleBlocked('convert')) return;
  openBatchModal();
  const docs = [], images = [];
  const listEl = $('convert-list');
  list.forEach(p => {
    const row = makeBatchRow(p);
    listEl.appendChild(row);
    if (IMG_RE.test(p)) images.push([p, row]);
    else { docs.push(p); batchRowsBySrc[p] = row; }
  });
  batchDocsDone = docs.length === 0;
  batchOcrDone = images.length === 0;
  batchFinished = false;
  setBatchTriggersBusy(true);
  $('batch-cancel').classList.remove('hidden');
  $('convert-status').textContent = batchT('batch.preparing') || '';
  if (docs.length) runBatchDocsLane(docs, overwrite);
  if (images.length) runBatchOcrLane(images);
}

async function runBatchDocsLane(paths, overwrite) {
  try {
    if (!(await ensureModule('convert'))) throw new Error('convert_module_unavailable');
    const r = await apiFetch('/api/convert/batch', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ paths, overwrite: !!overwrite, confirm: true }),
    });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.error_code || ('http_' + r.status));
    batchJobId = d.job;
    if (batchCancelRequested) sendBatchCancel();
    pollBatchJob(d.job);
  } catch (e) {
    failBatchDocsLane(e && e.message);
  }
}

function sendBatchCancel() {
  if (!batchJobId || !batchCancelRequested) return;
  apiFetch('/api/convert/cancel', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ job: batchJobId }),
  }).catch(() => {});
}

function pollBatchJob(jid) {
  stopBatchPoll();
  batchJobTimer = setInterval(async () => {
    try {
      const r = await apiFetch('/api/convert/progress?job=' + encodeURIComponent(jid));
      if (!r.ok) { failBatchDocsLane('HTTP ' + r.status); return; }
      const d = await r.json();
      renderBatchProgress(d);
      if (d.finished) {
        stopBatchPoll();
        batchDocsDone = true;
        maybeFinishBatch();
      }
    } catch (e) {
      failBatchDocsLane(e && e.message);
    }
  }, 600);
}

function failBatchDocsLane(message) {
  stopBatchPoll();
  for (const src in batchRowsBySrc) {
    const row = batchRowsBySrc[src];
    if (!row.dataset.done) {
      setBatchRow(row, 'error', message || '');
      countBatchRow(row, 'error');
    }
  }
  batchDocsDone = true;
  maybeFinishBatch();
}

function renderBatchProgress(d) {
  (d.items || []).forEach(it => {
    const row = batchRowsBySrc[it.src];
    if (!row) return;
    const status = it.status || 'queued';
    if (status !== 'queued') {
      if ((status === 'ok' || status === 'skipped') && it.out) row.dataset.out = it.out;
      setBatchRow(row, status, it.error || '');
      countBatchRow(row, status);
    }
  });
  if (d.running) {
    $('convert-status').textContent = batchT('batch.converting', { done: d.done, total: d.total })
      || '';
  }
}

async function runBatchOcrLane(items) {
  if (!(await ensureModule('ocr'))) {
    items.forEach(([, row]) => { setBatchRow(row, 'error'); countBatchRow(row, 'error'); });
    batchOcrDone = true;
    maybeFinishBatch();
    return;
  }
  for (let i = 0; i < items.length; i++) {
    const [path, row] = items[i];
    // 首项不检查取消标记：保证队列启动后至少处理一个任务，且取消语义可预期
    if (i > 0 && batchOcrCanceled && !row.dataset.done) {
      setBatchRow(row, 'canceled');
      countBatchRow(row, 'canceled');
      continue;
    }
    setBatchRow(row, 'running');
    try {
      const overwrite = $('convert-overwrite') && $('convert-overwrite').checked;
      const r = await apiFetch('/api/ocr?p=' + encodeURIComponent(path) + '&save=1&on_exists=' + (overwrite ? 'overwrite' : 'skip'));
      const d = await r.json().catch(() => ({}));
      if (r.ok && d.empty) {
        // 识别成功但没有文字：不写文件，标注“无文字”。
        setBatchRow(row, 'skipped', batchT('batch.ocrNoText') || '');
        const st = row.querySelector('.batch-state');
        if (st) st.textContent = batchT('batch.ocrNoText') || '无文字';
        countBatchRow(row, 'skipped');
      } else if (r.ok && d.content) {
        if (d.out && (d.saved || d.skipped)) row.dataset.out = d.out;
        const status = d.skipped ? 'skipped' : 'ok';
        setBatchRow(row, status);
        countBatchRow(row, status);
      } else {
        const code = d.error_code || 'ocr_failed';
        const msg = batchT('batch.err.' + code);
        setBatchRow(row, 'error', msg && msg !== 'batch.err.' + code ? msg : (d.error || code));
        countBatchRow(row, 'error');
      }
    } catch (e) {
      setBatchRow(row, 'error', e && e.message);
      countBatchRow(row, 'error');
    }
  }
  batchOcrDone = true;
  maybeFinishBatch();
}

/** 最长公共父目录；没有公共部分时取第一个输出所在目录。 */
function commonOutputDir(paths) {
  const dirs = paths.map(p => p.replace(/\\/g, '/').replace(/\/[^/]*$/, ''));
  if (!dirs.length) return '';
  const split = dirs.map(d => d.split('/'));
  const first = split[0];
  let n = first.length;
  for (const s of split.slice(1)) {
    let i = 0;
    while (i < n && i < s.length && s[i].toLowerCase() === first[i].toLowerCase()) i++;
    n = i;
  }
  const common = first.slice(0, n).join('/');
  return common && !/^[A-Za-z]:$/.test(common) ? common : dirs[0];
}

function showBatchOpenDir() {
  const btn = $('convert-open-dir');
  if (!btn) return;
  const outs = Array.from($('convert-list').querySelectorAll('.batch-item'))
    .map(r => r.dataset.out).filter(Boolean);
  if (!outs.length) { btn.classList.add('hidden'); return; }
  const dir = (typeof batchFolderRoot === 'string' && batchFolderRoot) || commonOutputDir(outs);
  btn.classList.remove('hidden');
  btn.onclick = async () => {
    try {
      const r = await apiFetch('/api/system/open-path', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: dir }),
      });
      const d = await r.json().catch(() => ({}));
      if (!r.ok || d.ok === false) {
        showToast(d.error_code === 'path_not_found'
          ? (batchT('toast.pathNotFound') || '路径不存在')
          : (batchT('toast.openFailed') || '无法打开：') + dir);
      }
    } catch (e) { showToast((batchT('toast.openFailed') || '无法打开：') + dir); }
  };
}

/** True from enqueue until both lanes have finished. */
let batchActive = false;
function isBatchRunning() {
  return batchActive;
}

/** The pickers stay disabled while a batch is running (no parallel jobs). */
function setBatchTriggersBusy(on) {
  batchActive = on;
  ['convert-files', 'convert-folder'].forEach(id => {
    const el = $(id);
    if (!el) return;
    el.disabled = on;
    if (on) el.setAttribute('aria-busy', 'true'); else el.removeAttribute('aria-busy');
  });
}

function maybeFinishBatch() {
  if (batchFinished || !batchDocsDone || !batchOcrDone) return;
  batchFinished = true;
  setBatchTriggersBusy(false);
  showBatchOpenDir();
  const c = batchCount;
  let text = batchT('batch.summary', { ok: c.ok, skipped: c.skipped, failed: c.error })
    || '';
  if (c.canceled) {
    text += ' · ' + (batchT('batch.summaryCanceled', { canceled: c.canceled }) || '');
  }
  $('convert-status').textContent = text;
  $('batch-cancel').classList.add('hidden');
}

function onBatchCancel() {
  batchOcrCanceled = true;
  batchCancelRequested = true;
  sendBatchCancel();
}
