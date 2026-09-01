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
const batchRowsBySrc = Object.create(null);

function batchT(k, p) {
  return window.i18n ? window.i18n.t(k, p) : k;
}

function openBatchModal() {
  stopBatchPoll();
  batchJobId = null;
  batchCancelRequested = false;
  batchOcrCanceled = false;
  batchFinished = false;
  batchCount = { ok: 0, skipped: 0, error: 0, canceled: 0 };
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
      if (status === 'ok' && it.out) row.dataset.out = it.out;
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
      const r = await apiFetch('/api/ocr?p=' + encodeURIComponent(path));
      const d = await r.json().catch(() => ({}));
      const ok = r.ok && d.content;
      setBatchRow(row, ok ? 'ok' : 'error', ok ? '' : (d.error_code || 'ocr_failed'));
      countBatchRow(row, ok ? 'ok' : 'error');
    } catch (e) {
      setBatchRow(row, 'error', e && e.message);
      countBatchRow(row, 'error');
    }
  }
  batchOcrDone = true;
  maybeFinishBatch();
}

function maybeFinishBatch() {
  if (batchFinished || !batchDocsDone || !batchOcrDone) return;
  batchFinished = true;
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
