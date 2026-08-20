'use strict';
/* ============================================================
   ReadMD Features - Mobile LAN Sharing
   ============================================================ */

/* ---------------- 移动端共享 ---------------- */

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
      $('share-url').textContent = (_t('share.mobileUrlLabel') || 'Open in mobile browser: ') + d.url;
      $('share-token').textContent = (_t('share.tokenLabel') || 'Access token: ') + d.token;
      renderQr(d.url);
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
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    const r = await apiFetch('/api/share/start', { method: 'POST' });
    const d = await r.json();
    if (d.error) { showToast(d.error); return; }
    showToast(_t('toast.shareStarted') || 'Sharing started');
  } catch (e) { showToast((_t('toast.shareStartFail') || 'Start failed: ') + e.message); }
  refreshShareStatus();
}

async function stopShare() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    await apiFetch('/api/share/stop', { method: 'POST' });
    showToast(_t('toast.shareStopped') || 'Sharing stopped');
  } catch (e) { showToast((_t('toast.shareStopFail') || 'Stop failed: ') + e.message); }
  refreshShareStatus();
}

