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
  const prevTheme = document.body.dataset.theme;
  let theme = state.theme;
  if (theme === 'auto') {
    theme = (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light';
  }
  document.body.dataset.theme = theme;
  document.body.style.setProperty('--fs', (state.fontSize / 100).toFixed(2));
  document.body.style.setProperty('--line-width', state.lineWidth + 'px');
  document.body.style.setProperty('--ai-panel-width', state.aiPanelWidth + 'px');
  updateThemeButton();
  if (prevTheme && prevTheme !== theme) {
    if (typeof reloadAllDiagrams === 'function') reloadAllDiagrams();
    if (typeof applyCmTheme === 'function') applyCmTheme();
  }
}

/* \u4E3B\u9898\u5FAA\u73AF\uFF1Aauto \u2192 light \u2192 dark \u2192 sepia \u2192 auto\u3002state.theme \u5B58\u7528\u6237\u7684\u9009\u62E9\uFF08\u542B 'auto'\uFF09\uFF0C
   body[data-theme] \u6C38\u8FDC\u662F\u5B9E\u9645\u751F\u6548\u7684 light / dark / sepia\u3002 */
const THEME_ORDER = ['auto', 'light', 'dark', 'sepia'];
const THEME_ICONS = {
  auto: '<svg class="tb-ic" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 0 1 0 16z" fill="currentColor" stroke="none"/></svg>',
  light: '<svg class="tb-ic" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>',
  dark: '<svg class="tb-ic" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/></svg>',
  sepia: '<svg class="tb-ic" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5z"/><path d="M4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5"/><path d="M8 7h8M8 11h6"/></svg>',
};

function updateThemeButton() {
  const btn = $('btn-theme');
  if (!btn) return;
  const choice = THEME_ORDER.includes(state.theme) ? state.theme : 'auto';
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const name = _t('theme.' + choice);
  const label = _t('theme.current', { name }) + ' (Ctrl+D)';
  if (btn.dataset.themeIcon !== choice) {
    btn.innerHTML = THEME_ICONS[choice];
    btn.dataset.themeIcon = choice;
  }
  btn.title = label;
  btn.setAttribute('aria-label', label);
}

function toggleTheme() {
  const i = THEME_ORDER.indexOf(state.theme);
  state.theme = THEME_ORDER[(i + 1) % THEME_ORDER.length];
  applySettings();
  saveSettings();
  applyCmTheme();
  if (typeof showToast === 'function' && window.i18n) {
    showToast(window.i18n.t('theme.current', { name: window.i18n.t('theme.' + state.theme) }), 1400);
  }
}

if (window.matchMedia) {
  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  const onScheme = () => { if (state.theme === 'auto') applySettings(); };
  if (mq.addEventListener) mq.addEventListener('change', onScheme);
  else if (mq.addListener) mq.addListener(onScheme);
}
window.addEventListener('readmd:language-changed', () => updateThemeButton());

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
        showToast((_t('toast.autostartFail') || '设置开机自启失败：') + (res && res.error ? res.error : (_t('toast.unknownError') || '未知错误')));
      }
    }
  } catch (e) {
    if (typeof showToast === 'function') {
      const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
      showToast((_t('toast.autostartFail') || '设置开机自启失败：') + e.message);
    }
  }
}


