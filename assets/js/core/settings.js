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
