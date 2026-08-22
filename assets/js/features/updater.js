'use strict';
/* ============================================================
   ReadMD Features - In-App Auto Update System
   ============================================================ */

let updateInfo = null;
let updateTimer = null;
let isUpdating = false;
let upgradeUrl = null;

async function checkUpdate(silent = true) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  try {
    let res = null;
    if (hasPy && py.check_update) {
      res = await py.check_update();
    } else {
      const resp = await fetch('/api/update/check');
      if (resp.ok) res = await resp.json();
    }
    if (!res || !res.ok) {
      if (!silent) showToast(res && res.error ? _t('update.checkFail') + '：' + res.error : _t('update.checkFail'));
      return;
    }
    if (res && res.current_version) {
      if ($('status-version')) $('status-version').textContent = 'v' + res.current_version;
      if ($('menu-version-label')) $('menu-version-label').textContent = _t('update.currentVer', { ver: res.current_version }) || ('当前版本 v' + res.current_version);
    }
    if (res.has_update) {
      updateInfo = res;
      upgradeUrl = res.html_url;
      if ($('status-update-badge')) {
        $('status-update-badge').classList.remove('hidden');
        if ($('update-badge-ver')) $('update-badge-ver').textContent = res.latest_version;
      }
      if ($('update-menu-dot')) $('update-menu-dot').classList.remove('hidden');
      if (!silent) {
        openUpdateModal();
      } else {
        showToast(_t('update.foundNew', { ver: res.latest_version }) || ('发现新版本 ' + res.latest_version), 5000);
      }
    } else {
      const curVer = res.current_version || (typeof VERSION !== 'undefined' ? VERSION : '2.3.7');
      if (!silent) showToast(_t('update.latest', { ver: curVer, version: curVer }) || ('当前已是最新版本 (v' + curVer + ')'));
    }
  } catch (e) {
    if (!silent) showToast(_t('update.checkFail') + '：' + e.message);
  }
}


function openUpdateModal() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!updateInfo) {
    checkUpdate(false);
    return;
  }
  $('update-modal').classList.remove('hidden');
  $('update-new-ver').textContent = updateInfo.latest_version || '';
  $('update-pub-time').textContent = updateInfo.published_at ? new Date(updateInfo.published_at).toLocaleDateString() : '';

  const notesEl = $('update-notes-content');
  if (updateInfo.release_notes) {
    notesEl.innerHTML = marked.parse(updateInfo.release_notes);
  } else {
    notesEl.textContent = _t('update.noNotes') || '暂无详细更新说明。';
  }

  if (updateInfo.asset) {
    $('update-asset-name').textContent = updateInfo.asset.name || (_t('update.package') || '安装包');
    const mb = updateInfo.asset.size ? (updateInfo.asset.size / (1024 * 1024)).toFixed(1) + ' MB' : '';

    $('update-asset-size').textContent = mb;
    const verifiable = !!updateInfo.asset.expected_sha;
    $('btn-update-start').disabled = !verifiable;
    $('btn-update-start').textContent = verifiable
      ? (_t('update.installNow') || '立即下载并更新')
      : (_t('update.unverifiedPackage') || '无法验证更新包');
  } else {
    $('update-asset-name').textContent = _t('update.noAsset') || '未找到匹配当前系统的二进制资产';
    $('update-asset-size').textContent = '';
    $('btn-update-start').disabled = true;
    $('btn-update-start').textContent = _t('update.noAsset') || '暂无对应安装包';
  }
}


function closeUpdateModal() {
  $('update-modal').classList.add('hidden');
}

async function startUpdateDownload() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!updateInfo || !updateInfo.asset || isUpdating) return;
  const asset = updateInfo.asset;
  const useMirror = $('update-use-mirror') && $('update-use-mirror').checked;
  if (!asset.expected_sha) {
    showToast(_t('update.unverifiedPackage') || '无法验证更新包');
    return;
  }

  $('btn-update-start').disabled = true;
  $('btn-update-cancel').classList.remove('hidden');
  $('update-progress-wrap').classList.remove('hidden');
  $('update-progress-fill').style.width = '0%';
  $('update-progress-text').textContent = _t('update.prepDownload') || '准备下载…';
  $('update-progress-speed').textContent = '';
  isUpdating = true;

  try {
    let started = false;
    if (hasPy && py.start_download_update) {
      const res = await py.start_download_update(asset.download_url, asset.name, asset.expected_sha, useMirror);
      started = res && res.ok;
    } else {
      const resp = await fetch('/api/update/download', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          download_url: asset.download_url,
          target_filename: asset.name,
          expected_sha: asset.expected_sha,
          use_mirror: useMirror,
        }),
      });
      const res = await resp.json();
      started = res && res.ok;
    }

    if (!started) {
      showToast(_t('toast.updateStartFail') || '启动下载失败');
      isUpdating = false;
      $('btn-update-start').disabled = false;
      return;
    }

    if (updateTimer) clearInterval(updateTimer);
    updateTimer = setInterval(async () => {
      let st = null;
      if (hasPy && py.get_download_status) {
        st = await py.get_download_status();
      } else {
        const resp = await fetch('/api/update/status');
        if (resp.ok) st = await resp.json();
      }
      if (!st) return;

      if (st.status === 'downloading') {
        const pct = st.percent || 0;
        $('update-progress-fill').style.width = pct + '%';
        const speedMb = ((st.speed_bps || 0) / (1024 * 1024)).toFixed(1);
        const curMb = ((st.downloaded_bytes || 0) / (1024 * 1024)).toFixed(1);
        const totMb = ((st.total_bytes || 0) / (1024 * 1024)).toFixed(1);
        const dlLabel = _t('update.downloading') || '正在下载…';
        $('update-progress-text').textContent = `${dlLabel} ${pct}% (${curMb}MB / ${totMb}MB)`;
        $('update-progress-speed').textContent = `${speedMb} MB/s`;
      } else if (st.status === 'verifying') {
        $('update-progress-fill').style.width = '100%';
        $('update-progress-text').textContent = _t('update.verifying') || '正在校验文件完整性 (SHA256)…';
      } else if (st.status === 'ready') {
        clearInterval(updateTimer);
        updateTimer = null;
        isUpdating = false;
        $('update-progress-text').textContent = _t('update.downloadDone') || '下载校验完成！正在准备安装…';
        $('btn-update-start').textContent = _t('update.restarting') || '正在重启并安装…';
        setTimeout(async () => {
          if (hasPy && py.apply_update) {
            await py.apply_update(st.target_file, updateInfo.flavor);
          } else {
            await fetch('/api/update/apply', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ file_path: st.target_file, flavor: updateInfo.flavor }),
            });
          }
        }, 800);
      } else if (st.status === 'error') {
        clearInterval(updateTimer);
        updateTimer = null;
        isUpdating = false;
        $('update-progress-text').textContent = (_t('update.downloadFailPrefix') || '下载失败：') + (st.error || (_t('toast.unknownNetworkErr') || '未知网络错误'));
        $('btn-update-start').disabled = false;
        $('btn-update-start').textContent = _t('update.retryDownload') || '重试下载';
        $('btn-update-cancel').classList.add('hidden');
      } else if (st.status === 'cancelled') {
        clearInterval(updateTimer);
        updateTimer = null;
        isUpdating = false;
        $('update-progress-text').textContent = _t('update.downloadCancelled') || '下载已取消';
        $('btn-update-start').disabled = false;
        $('btn-update-start').textContent = _t('update.redownload') || '重新下载';
        $('btn-update-cancel').classList.add('hidden');
      }
    }, 400);

  } catch (e) {
    showToast((_t('toast.downloadError') || '下载出错：') + e.message);
    isUpdating = false;
    $('btn-update-start').disabled = false;
  }
}

async function cancelUpdateDownload() {
  if (hasPy && py.cancel_download) {
    await py.cancel_download();
  } else {
    await fetch('/api/update/cancel', { method: 'POST' });
  }
}

