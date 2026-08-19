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
