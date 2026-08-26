'use strict';
/* ============================================================
   ReadMD Features - Mobile LAN Sharing
   ============================================================ */

/* ---------------- 移动端共享 ---------------- */

let qrLibraryLoader;

function loadQrLibrary() {
  if (typeof qrcode === 'function') return Promise.resolve();
  qrLibraryLoader ||= new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = '/assets/vendor/qrcode.min.js';
    script.onload = resolve;
    script.onerror = () => reject(new Error('QR library failed to load'));
    document.head.appendChild(script);
  }).catch(error => {
    qrLibraryLoader = null;
    throw error;
  });
  return qrLibraryLoader;
}

async function openShareModal() {
  $('share-modal').classList.remove('hidden');
  refreshShareStatus();
}

async function refreshShareStatus() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    const r = await apiFetch('/api/share/status');
    const d = await r.json();
    if (d.running) {
      $('share-start').disabled = true;
      $('share-stop').disabled = false;
      $('share-url').textContent = (_t('share.mobileUrlLabel') || '手机浏览器打开：') + d.url;
      $('share-token').textContent = (_t('share.tokenLabel') || '访问令牌：') + d.token;
      await renderQr(d.url);
    } else {
      $('share-start').disabled = false;
      $('share-stop').disabled = true;
      $('share-url').textContent = '';
      $('share-token').textContent = '';
      const q = $('share-qr');
      q.innerHTML = '<p class="fix-note">' + (_t('share.notRunning') || '尚未开启共享') + '</p>';
    }
  } catch (e) { /* ignore */ }
}

async function renderQr(text) {
  const box = $('share-qr');
  box.innerHTML = '';
  try {
    await loadQrLibrary();
    const qr = qrcode(0, 'M');
    qr.addData(text);
    qr.make();
    box.innerHTML = qr.createImgTag(6, 10);
  } catch (e) {
    box.textContent = text;
  }
}

async function startShare() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    const r = await apiFetch('/api/share/start', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ current_file: state.file || null }),
    });
    const d = await r.json();
    if (d.error) { showToast(d.error); return; }
    showToast(_t('toast.shareStarted') || '共享已开启');
  } catch (e) { showToast((_t('toast.shareStartFail') || '开启失败：') + e.message); }
  refreshShareStatus();
}

async function stopShare() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    await apiFetch('/api/share/stop', { method: 'POST' });
    showToast(_t('toast.shareStopped') || '共享已关闭');
  } catch (e) { showToast((_t('toast.shareStopFail') || '关闭失败：') + e.message); }
  refreshShareStatus();
}

