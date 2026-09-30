'use strict';
/* Shell chrome: palette / sheet keys, theme cross-fade, welcome wiring. */
(function () {
  const byId = id => document.getElementById(id);
  const reduceMotion = () => !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);

  function withThemeTransition(apply) {
    const root = document.documentElement;
    if (typeof document.startViewTransition !== 'function' || reduceMotion() || document.hidden) { apply(); return; }
    root.classList.add('rm-theme-vt');
    let vt;
    try { vt = document.startViewTransition(() => { apply(); }); }
    catch (e) { root.classList.remove('rm-theme-vt'); apply(); return; }
    const done = () => root.classList.remove('rm-theme-vt');
    vt.finished.then(done, done);
  }
  function wrapToggleTheme() {
    const orig = window.toggleTheme;
    if (typeof orig !== 'function' || orig.__rmShell) return;
    const wrapped = function () { withThemeTransition(() => orig.apply(this, arguments)); };
    wrapped.__rmShell = true;
    window.toggleTheme = wrapped;
  }

  const isEditable = el => !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable || !!(el.closest && el.closest('.cm-editor, [contenteditable="true"]')));
  const inCodeMirror = el => !!(el && el.closest && el.closest('.cm-editor'));
  const shellLayer = () => {
    const top = window.ReadMDModal ? window.ReadMDModal.top() : null;
    return !top || top.id === 'command-palette-modal' || top.id === 'shortcuts-modal';
  };

  function onGlobalKey(e) {
    if (e.defaultPrevented || e.isComposing || e.keyCode === 229) return;
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    const target = e.target;
    if (mod && !e.shiftKey && !e.altKey && key === 'k' && !inCodeMirror(target)) {
      if (!shellLayer()) return;
      e.preventDefault(); e.stopPropagation();
      window.ReadMDPalette && window.ReadMDPalette.toggle();
      return;
    }
    if (mod && e.shiftKey && !e.altKey && key === 'p') {
      if (!shellLayer()) return;
      e.preventDefault(); e.stopPropagation();
      window.ReadMDPalette && window.ReadMDPalette.toggle();
      return;
    }
    if ((mod && !e.altKey && (key === '/' || e.code === 'Slash')) || (!mod && !e.altKey && e.key === '?' && !isEditable(target))) {
      if (mod && inCodeMirror(target)) return;
      if (!shellLayer()) return;
      e.preventDefault(); e.stopPropagation();
      window.ReadMDShortcuts.toggle();
      return;
    }
  }

  function fillKbd(root = document) {
    const A = window.ReadMDKeys;
    if (!A) return;
    root.querySelectorAll('[data-kbd]').forEach(el => {
      if (el.dataset.kbdDone === el.dataset.kbd) return;
      el.innerHTML = A.kbdHtml(el.dataset.kbd);
      el.dataset.kbdDone = el.dataset.kbd;
    });
  }

  function bindChrome() {
    fillKbd();
    const contentEl = byId('content');
    if (contentEl) new MutationObserver(() => { if (byId('welcome')) fillKbd(contentEl); }).observe(contentEl, { childList: true });
    const btn = byId('btn-palette');
    if (btn) btn.addEventListener('click', () => window.ReadMDPalette && window.ReadMDPalette.open());
    const content = byId('content');
    if (content) content.addEventListener('click', e => {
      const el = e.target.closest && e.target.closest('#welcome [data-action]');
      if (!el) return;
      if (el.dataset.action === 'palette') window.ReadMDPalette && window.ReadMDPalette.open();
      else if (el.dataset.action === 'shortcuts') window.ReadMDShortcuts && window.ReadMDShortcuts.open();
    });
  }

  window.addEventListener('keydown', onGlobalKey, true);
  wrapToggleTheme();
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', bindChrome);
  else bindChrome();

  window.ReadMDShell = { withThemeTransition };
})();
