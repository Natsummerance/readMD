'use strict';
/* ============================================================
   ReadMD Editor - Image Editor (Crop / Rotate / Scale)
   ============================================================ */

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
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!state.dir) { showToast(_t('toast.imgLocalOnly') || '图片编辑仅支持本地 Markdown 文件'); return; }
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
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!file) return;
  const fr = new FileReader();
  fr.onload = () => {
    try { loadImgSrc(fr.result); } catch (e) { showToast((_t('toast.imgReadFail') || '图片读取失败：') + e.message); }
  };
  fr.onerror = () => showToast(_t('toast.imgReadFail') || '图片读取失败');
  fr.readAsDataURL(file);
}

function loadImgSrc(src) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
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
  im.onerror = () => showToast(_t('toast.imgLoadCorsFail') || '图片加载失败（URL 可能被跨域限制）');
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
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const el = $('img-info');
  if (!el) return;
  const r = imgRect();
  if (!imgState.img || !r.w) { el.textContent = _t('img.noImageLoaded') || '尚未加载图片'; return; }
  const naturalW = Math.max(1, Math.round(imgState.crop.w / imgState.fitScale));
  const naturalH = Math.max(1, Math.round(imgState.crop.h / imgState.fitScale));
  if (!imgState.outW || !imgState.outH) { imgState.outW = naturalW; imgState.outH = naturalH; }
  $('img-out-w').value = imgState.outW; $('img-out-h').value = imgState.outH;
  el.textContent = (_t('img.origImage') || '原图') + ' ' + imgState.rawW + '×' + imgState.rawH + ' · ' + (_t('img.cropLabel') || '裁剪') + ' ' + naturalW + '×' + naturalH + ' · ' + (_t('img.outLabel') || '输出') + ' ' + imgState.outW + '×' + imgState.outH + ' px';
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
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const url = $('img-url-input').value.trim();
  if (!url) { showToast(_t('toast.imgEnterUrl') || '请输入图片 URL'); return; }
  if (!cmView) { showToast(_t('toast.imgEnterEditFirst') || '请先进入编辑模式'); return; }
  cmInsertImage(url);
  closeImgModal();
  showToast(_t('toast.imgUrlInserted') || '已插入图片 URL');
}

function cmInsertImage(rel) {
  if (!cmView) return;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = cmView.state.selection.main;
  const insert = '![' + (_t('editor.image') || '图片') + '](' + rel + ')';
  cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert }, selection: { anchor: sel.from + insert.length } });
  cmView.focus();
}

async function exportAndInsertImg() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
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
  if (!blob) { showToast(_t('toast.imgExportFail') || '图片导出失败'); return; 
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
    if (!resp.ok || !d.ok) throw new Error(d.error || _t('toast.unknownError'));
    cmInsertImage(d.rel);
    closeImgModal();
    showToast(_t('toast.imgInsertedRel', { rel: d.rel }) || ('图片已插入（' + d.rel + '）'));
  } catch (e) {
    showToast((_t('toast.imgSaveFail') || '图片保存失败：') + e.message);
  } finally {
    busy(false);
  }
}
