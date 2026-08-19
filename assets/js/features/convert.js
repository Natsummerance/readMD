'use strict';
/* ============================================================
   ReadMD Features - Batch File Conversion (All-to-MD)
   ============================================================ */

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
    if (!d._autoOpened) {
      d._autoOpened = true;
      const okItems = (d.items || []).filter(it => it.status === 'ok' && it.out);
      if (okItems.length > 0) {
        (async () => {
          for (const it of okItems) {
            await loadFile(it.out);
          }
          closeConvertModal();
          showToast('转换完成，已自动在新标签页中打开');
        })();
      }
    }
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
  if (mode === 'ocr' || (mode !== 'convert' && IMG_RE.test(p))) ocrFile(p);
  else convertFile(p);
}
