'use strict';
/* ============================================================
   ReadMD Core - Settings & Preferences
   ============================================================ */

/* ---------------- 设置 ---------------- */

async function loadSettings() {
  try {
    if (hasPy) {
      const s = await py.get_settings();
      if (s && typeof s === 'object') Object.assign(state, s);
    } else {
      const s = JSON.parse(localStorage.getItem('readmd-settings') || '{}');
      Object.assign(state, s);
    }
  } catch (e) { /* ignore */ }
  applySettings();
}

async function saveSettings() {
  const s = {
    theme: state.theme, fontSize: state.fontSize, lineWidth: state.lineWidth, aiPanelWidth: state.aiPanelWidth,
    autoReload: state.autoReload, pvLayout: state.pvLayout, pvSync: state.pvSync,
    pvSplitX: state.pvSplitX, pvSplitY: state.pvSplitY,
  };
  try {
    if (hasPy) await py.save_settings(s);
    else localStorage.setItem('readmd-settings', JSON.stringify(s));
  } catch (e) { /* ignore */ }
}

function applySettings() {
  let theme = state.theme;
  if (theme === 'auto') {
    theme = (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light';
  }
  document.body.dataset.theme = theme;
  document.body.style.setProperty('--fs', (state.fontSize / 100).toFixed(2));
  document.body.style.setProperty('--line-width', state.lineWidth + 'px');
  document.body.style.setProperty('--ai-panel-width', state.aiPanelWidth + 'px');
  $('btn-theme').textContent = theme === 'dark' ? '\u2600' : '\u263E';
}

function toggleTheme() {
  const cur = document.body.dataset.theme;
  const next = cur === 'dark' ? 'sepia' : (cur === 'sepia' ? 'light' : 'dark');
  state.theme = next;
  applySettings();
  saveSettings();
  applyCmTheme();
}

function zoom(delta) {
  state.fontSize = Math.max(70, Math.min(180, state.fontSize + delta));
  applySettings();
  saveSettings();
}

async function checkAutostart() {
  try {
    let enabled = false;
    if (typeof hasPy !== 'undefined' && hasPy && py.get_autostart) {
      enabled = await py.get_autostart();
    } else {
      const r = await fetch('/api/autostart/get');
      if (r.ok) {
        const j = await r.json();
        enabled = !!j.enabled;
      }
    }
    updateAutostartUI(enabled);
  } catch (e) { /* ignore */ }
}

function updateAutostartUI(enabled) {
  const lbl = $('autostart-status-label');
  if (lbl) {
    lbl.textContent = enabled ? (window.i18n ? window.i18n.t('app.enabled') : '已开启') : (window.i18n ? window.i18n.t('app.disabled') : '未开启');
  }
  state.autostart = enabled;
}

async function toggleAutostart() {
  const next = !state.autostart;
  try {
    let res = null;
    if (typeof hasPy !== 'undefined' && hasPy && py.set_autostart) {
      res = await py.set_autostart(next);
    } else {
      const r = await fetch('/api/autostart/set', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: next })
      });
      if (r.ok) res = await r.json();
    }
    if (res && res.ok) {
      updateAutostartUI(next);
      if (typeof showToast === 'function') {
        showToast(next ? (window.i18n ? window.i18n.t('app.autostartOn') : '已开启开机自启动') : (window.i18n ? window.i18n.t('app.autostartOff') : '已关闭开机自启动'));
      }
    } else {
      if (typeof showToast === 'function') {
        const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
        showToast((_t('toast.autostartFail') || 'Failed to set auto-start: ') + (res && res.error ? res.error : (_t('toast.unknownError') || 'Unknown error')));
      }
    }
  } catch (e) {
    if (typeof showToast === 'function') {
      const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
      showToast((_t('toast.autostartFail') || 'Failed to set auto-start: ') + e.message);
    }
  }
}


