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
  const modal = $('batch-modal');
  if (modal) modal.classList.remove('hidden');
  $('batch-list').innerHTML = '';
  $('batch-status').textContent = '';
  $('batch-cancel').classList.add('hidden');
}

function closeBatchModal() {
  stopBatchPoll();
  $('batch-modal').classList.add('hidden');
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
  st.textContent = batchT('batch.statusQueued') || '排队中';
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
      queued: batchT('batch.statusQueued') || '排队中',
      running: batchT('batch.statusRunning') || '处理中',
      ok: batchT('batch.statusOk') || '成功',
      skipped: batchT('batch.statusSkipped') || '跳过（已存在）',
      error: batchT('batch.statusError') || '失败',
      canceled: batchT('batch.statusCanceled') || '已取消',
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
  const listEl = $('batch-list');
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
  $('batch-status').textContent = batchT('batch.preparing') || '准备中…';
  if (docs.length) runBatchDocsLane(docs, overwrite);
  if (images.length) runBatchOcrLane(images);
}

async function runBatchDocsLane(paths, overwrite) {
  try {
    if (!(await ensureModule('convert'))) throw new Error(batchT('convert.statusError') || '失败');
    const r = await apiFetch('/api/convert/batch', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ paths, overwrite: !!overwrite }),
    });
    const d = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(d.error || ('HTTP ' + r.status));
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
    $('batch-status').textContent = batchT('batch.converting', { done: d.done, total: d.total })
      || ('转换中 ' + d.done + '/' + d.total + '…');
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
      setBatchRow(row, ok ? 'ok' : 'error', ok ? '' : (d.error || d.note || ('HTTP ' + r.status)));
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
    || ('完成：成功 ' + c.ok + ' · 跳过 ' + c.skipped + ' · 失败 ' + c.error);
  if (c.canceled) {
    text += ' · ' + (batchT('batch.summaryCanceled', { canceled: c.canceled }) || ('已取消 ' + c.canceled));
  }
  $('batch-status').textContent = text;
  $('batch-cancel').classList.add('hidden');
}

function onBatchCancel() {
  batchOcrCanceled = true;
  batchCancelRequested = true;
  sendBatchCancel();
}

function pickBatchFiles() {
  if (hasPy) {
    py.choose_many_files().then(files => {
      if (files && files.length) enqueueBatchFiles(files, false);
    }).catch(() => {});
    return;
  }
  const input = $('batch-file-input');
  input.value = '';
  input.onchange = async () => {
    const files = Array.from(input.files || []);
    if (!files.length) return;
    const paths = [];
    for (const f of files) {
      const p = await uploadFile(f);
      if (p) paths.push(p);
    }
    if (paths.length) enqueueBatchFiles(paths, false);
  };
  input.click();
}

async function pickBatchFolder() {
  if (!hasPy) { showToast(batchT('toast.convertBrowserNotice') || '浏览器模式请使用“选择文件”转换'); return; }
  let dir = null;
  try { dir = await py.choose_folder(); } catch (e) { dir = null; }
  if (!dir) return;
  try {
    const r = await apiFetch('/api/convert/collect?dir=' + encodeURIComponent(dir));
    const d = await r.json();
    if (!r.ok) throw new Error(d.error || (batchT('convert.statusError') || '收集失败'));
    const files = d.files || [];
    if (!files.length) { showToast(batchT('convert.noConvertibleFiles') || '该目录下没有可转换的文件'); return; }
    enqueueBatchFiles(files, false);
  } catch (e) {
    showToast((batchT('toast.collectFilesFail') || '收集文件失败：') + e.message);
  }
}
