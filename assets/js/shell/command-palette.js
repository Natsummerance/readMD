'use strict';
/* Shell loader: key chips, palette/sheet stubs, lazy shell-deferred.{js,css}. */
(function () {
  const VERSION = (document.documentElement && document.documentElement.dataset.version) || '';
  const q = VERSION ? '?v=' + encodeURIComponent(VERSION) : '';
  const JS = '/assets/js/shell/shell-deferred.js' + q;
  const CSS = '/assets/css/shell-deferred.css' + q;
  let cssPending = null;
  let pending = null;

  function loadCss() {
    if (cssPending) return cssPending;
    cssPending = new Promise(resolve => {
      const link = document.createElement('link');
      link.rel = 'stylesheet';
      link.href = CSS;
      link.dataset.rmShellDeferred = '';
      link.onload = link.onerror = () => resolve();
      const anchor = document.querySelector('link[href*="/css/shell.css"]');
      if (anchor && anchor.parentNode) anchor.parentNode.insertBefore(link, anchor.nextSibling);
      else document.head.appendChild(link);
    });
    return cssPending;
  }
  function loadJs() {
    return new Promise((resolve, reject) => {
      if (window.ReadMDActions && window.ReadMDActions.__real) { resolve(); return; }
      const s = document.createElement('script');
      s.src = JS;
      s.async = true;
      s.onload = () => resolve();
      s.onerror = () => reject(new Error('palette module failed to load'));
      document.head.appendChild(s);
    });
  }
  function load() {
    if (!pending) {
      pending = loadCss().then(loadJs).catch(err => { pending = null; throw err; });
    }
    return pending;
  }

  const stub = (name, methods) => {
    const api = {};
    for (const m of methods) {
      api[m] = (...args) => {
        const real = window['__rm' + name];
        if (real) return real[m](...args);
        if (m === 'isOpen') return false;
        if (m === 'close') return undefined;
        return load().then(() => window['__rm' + name] && window['__rm' + name][m](...args)).catch(() => {});
      };
    }
    return api;
  };
  window.ReadMDPalette = stub('Palette', ['open', 'close', 'toggle', 'isOpen']);
  window.ReadMDShortcuts = stub('Shortcuts', ['open', 'close', 'toggle', 'isOpen']);
  window.openMdCommandPalette = () => window.ReadMDPalette.open();

  const IS_MAC = /Mac|iPhone|iPad|iPod/.test((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '');
  const KEY_LABEL = {
    Mod: IS_MAC ? '⌘' : 'Ctrl', Ctrl: IS_MAC ? '⌃' : 'Ctrl', Shift: IS_MAC ? '⇧' : 'Shift',
    Alt: IS_MAC ? '⌥' : 'Alt', ArrowLeft: '←', ArrowRight: '→', ArrowUp: '↑', ArrowDown: '↓',
    Enter: '↵', Escape: 'Esc', Delete: 'Del', '=': '+', Space: 'Space',
  };
  const esc = s => String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  function formatShortcut(spec) {
    if (!spec) return [];
    return String(spec).split('+').filter(Boolean).map(k => KEY_LABEL[k] || (k.length === 1 ? k.toUpperCase() : k));
  }
  const kbdHtml = spec => formatShortcut(spec).map(k => `<kbd class="rm-kbd">${esc(k)}</kbd>`).join('');
  window.ReadMDKeys = { isMac: IS_MAC, formatShortcut, kbdHtml, escapeHtml: esc };

  window.ReadMDShellLoader = { load };
  const warmCss = () => { loadCss(); };
  ['pointerover', 'pointerdown', 'keydown', 'focusin', 'dragenter', 'touchstart'].forEach(ev => window.addEventListener(ev, warmCss, { capture: true, once: true, passive: true }));
  const watchTabs = () => {
    const bars = ['doc-tabs-bar', 'doc-tabs-secondary-bar'].map(id => document.getElementById(id)).filter(Boolean);
    if (!bars.length || !window.MutationObserver) return;
    const now = () => { mo.disconnect(); load().catch(() => {}); };
    const mo = new MutationObserver(now);
    bars.forEach(bar => mo.observe(bar, { childList: true }));
    if (bars.some(bar => bar.children.length)) now();
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', watchTabs);
  else watchTabs();
  const warm = () => setTimeout(() => load().catch(() => {}), 1200);
  if (window.requestIdleCallback) window.addEventListener('load', () => window.requestIdleCallback(warm, { timeout: 3000 }));
  else window.addEventListener('load', warm);
})();
