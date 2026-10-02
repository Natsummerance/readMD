'use strict';

/*
 * Modal layer manager.
 *
 * Every `[role="dialog"]` overlay in index.html is shown and hidden by toggling
 * the `hidden` class, from dozens of call sites.  Instead of rewriting each
 * opener, this module observes that class and maintains a layer stack:
 *
 *  - the most recently shown dialog is the top layer;
 *  - Tab / Shift+Tab wrap inside the top layer (focus can never escape);
 *  - everything outside the stack is `inert` while any layer is open
 *    (reference counted, so nested layers restore correctly);
 *  - one global Esc handler closes exactly the top layer, wherever focus is,
 *    and ignores Esc while an IME composition is active;
 *  - closing a layer returns focus to the element that opened it.
 *
 * Esc runs the dialog's own close path: an explicit `data-modal-close`
 * element, else the first close/cancel button in the dialog, else the
 * `hidden` class is simply added.  Dialogs that must not close on Esc (a
 * running update download, for example) set `data-modal-esc="off"` or
 * register a guard with `ReadMDModal.setGuard(id, fn)`.
 */
(function () {
  const stack = [];                       // [{ el, opener }]
  let lastOutside = null;                 // last focused element while no layer was open
  const guards = new Map();               // id -> () => boolean (true = may close)
  const FOCUSABLE = [
    'a[href]', 'area[href]', 'button:not([disabled])', 'input:not([disabled]):not([type="hidden"])',
    'select:not([disabled])', 'textarea:not([disabled])', 'iframe', 'audio[controls]', 'video[controls]',
    '[contenteditable]:not([contenteditable="false"])', '[tabindex]:not([tabindex="-1"])',
  ].join(',');

  const isShown = el => el.isConnected && !el.classList.contains('hidden') && getComputedStyle(el).display !== 'none';

  function focusables(root) {
    return [...root.querySelectorAll(FOCUSABLE)].filter(el => {
      if (el.closest('[inert]') && !root.contains(el.closest('[inert]'))) return false;
      if (el.closest('.hidden')) return false;
      const rect = el.getClientRects();
      return rect.length > 0 && getComputedStyle(el).visibility !== 'hidden';
    });
  }

  /** Top-level siblings that must go inert while `el` is a layer. */
  function backgroundOf(el) {
    const out = [];
    let node = el;
    while (node && node !== document.body && node.parentElement) {
      for (const sib of node.parentElement.children) {
        if (sib === node || sib.tagName === 'SCRIPT' || sib.tagName === 'STYLE') continue;
        out.push(sib);
      }
      node = node.parentElement;
    }
    return out;
  }

  function refreshInert() {
    // Recomputed from the stack on every change: everything outside the top
    // layer's ancestry is inert.  Only nodes this module marked are released,
    // so an `inert` set elsewhere is never cleared by accident.
    document.querySelectorAll('[data-rm-inert]').forEach(n => { n.removeAttribute('inert'); n.removeAttribute('data-rm-inert'); });
    const top = stack[stack.length - 1];
    if (!top) return;
    for (const n of backgroundOf(top.el)) {
      if (n.hasAttribute('inert')) continue;
      n.setAttribute('inert', '');
      n.setAttribute('data-rm-inert', '');
    }
  }

  function initialFocus(el) {
    const explicit = el.querySelector('[autofocus], [data-modal-initial]');
    if (explicit && focusables(el).includes(explicit)) return explicit;
    // Prefer the first form field, then the first non-close control.
    const list = focusables(el);
    const field = list.find(n => n.matches('input, select, textarea, [contenteditable]'));
    if (field) return field;
    const nonClose = list.find(n => !isCloseControl(n));
    return nonClose || list[0] || null;
  }

  function isCloseControl(n) {
    const id = n.id || '';
    return n.hasAttribute('data-modal-close') || /(^|-)(close|close-x)$/.test(id);
  }

  function push(el) {
    if (stack.some(l => l.el === el)) return;
    const active = document.activeElement;
    let opener = active instanceof HTMLElement && active !== document.body ? active : null;
    // The first layer returns to the last focus outside every layer, even if
    // the opener already moved focus into the dialog before we observed it.
    if (!stack.length && (!opener || el.contains(opener))) opener = lastOutside && lastOutside.isConnected ? lastOutside : null;
    stack.push({ el, opener });
    refreshInert();
    // Let the opener's own focus logic run first; only step in if focus is outside.
    requestAnimationFrame(() => {
      if (stack[stack.length - 1]?.el !== el) return;
      if (!el.contains(document.activeElement)) {
        const target = initialFocus(el);
        if (target) target.focus({ preventScroll: true });
        else { el.setAttribute('tabindex', '-1'); el.focus({ preventScroll: true }); }
      }
    });
  }

  function pop(el) {
    const at = stack.findIndex(l => l.el === el);
    if (at < 0) return;
    const [layer] = stack.splice(at, 1);
    refreshInert();
    const next = stack[stack.length - 1];
    const back = layer.opener;
    const target = back && back.isConnected && !back.closest('[inert]') ? back : (next ? initialFocus(next.el) : null);
    if (target && (!next || next.el.contains(target))) target.focus({ preventScroll: true });
    else if (next) { const f = initialFocus(next.el); if (f) f.focus({ preventScroll: true }); }
  }

  function sync(el) {
    if (isShown(el)) push(el);
    else pop(el);
  }

  function closeTop() {
    const top = stack[stack.length - 1];
    if (!top) return false;
    const el = top.el;
    if (el.dataset.modalEsc === 'off') return true;
    const guard = guards.get(el.id);
    if (guard && guard() === false) return true;
    const closer = el.querySelector('[data-modal-close]') ||
      [...el.querySelectorAll('button')].find(b => /(^|-)(close|close-x|cancel)$/.test(b.id) && !b.disabled && !b.closest('.hidden'));
    if (closer) closer.click();
    // No close button (e.g. choice-modal builds its buttons at runtime): every
    // promise-based dialog treats a click on its own backdrop as "cancel".
    else el.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    // Anything that is still open after its close path gets hidden directly.
    if (isShown(el) && stack[stack.length - 1]?.el === el) {
      el.classList.add('hidden');
      sync(el);
    }
    return true;
  }

  function onKeyDown(event) {
    const top = stack[stack.length - 1];
    if (!top) return;
    if (event.key === 'Escape') {
      // Esc inside an IME composition cancels the composition only: keep the
      // browser default, but no dialog handler may treat it as "close".
      if (event.isComposing || event.keyCode === 229) { event.stopImmediatePropagation(); return; }
      event.preventDefault();
      event.stopImmediatePropagation();
      closeTop();
      return;
    }
    if (event.key !== 'Tab') return;
    const list = focusables(top.el);
    if (!list.length) { event.preventDefault(); return; }
    const first = list[0];
    const last = list[list.length - 1];
    const cur = document.activeElement;
    if (!top.el.contains(cur)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    } else if (event.shiftKey && cur === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && cur === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function onFocusIn(event) {
    const top = stack[stack.length - 1];
    if (!top) {
      // Remember where focus was before any layer opened: an opener that
      // moves focus itself (e.g. into a search box) runs before the observer.
      const t = event.target;
      let inLayer = false;
      for (let p = t instanceof Element ? t : null; p && !inLayer; p = p.parentElement) inLayer = !!p.__rmModal;
      if (t instanceof HTMLElement && t !== document.body && !inLayer) lastOutside = t;
      return;
    }
    if (top.el.contains(event.target)) return;
    // Something outside (e.g. a toast) grabbed focus: pull it back.
    const f = initialFocus(top.el);
    if (f) f.focus({ preventScroll: true });
  }

  function watch(el) {
    if (el.__rmModal) return;
    el.__rmModal = true;
    if (!el.hasAttribute('aria-modal')) el.setAttribute('aria-modal', 'true');
    new MutationObserver(() => sync(el)).observe(el, { attributes: true, attributeFilter: ['class', 'style'] });
    if (isShown(el)) push(el);
  }

  function scan(root = document) {
    root.querySelectorAll('[role="dialog"]').forEach(el => {
      // Only full-screen overlays are layers; inline role=dialog popovers are not.
      if (el.id && /-modal$/.test(el.id)) watch(el);
      else if (el.classList.contains('modal-overlay') || el.dataset.modalLayer === 'on') watch(el);
    });
  }

  // A click on the dim backdrop (pressed and released outside the dialog box)
  // closes the top layer through its own close button, like Esc.  Dialogs that
  // hold unsaved input or ask for a decision opt out with data-backdrop="static".
  let pressedBackdrop = null;
  document.addEventListener('mousedown', event => {
    const top = stack[stack.length - 1];
    pressedBackdrop = top && event.target === top.el ? top.el : null;
  }, true);
  document.addEventListener('click', event => {
    const top = stack[stack.length - 1];
    const el = top && top.el;
    if (!el || event.target !== el) return;
    // Static dialogs ignore backdrop clicks entirely, including legacy per-modal handlers.
    if (el.dataset.backdrop === 'static') { event.stopImmediatePropagation(); return; }
    if (pressedBackdrop !== el) return;
    pressedBackdrop = null;
    if (!isShown(el)) return;
    const closer = el.querySelector('[data-modal-close]') ||
      [...el.querySelectorAll('button')].find(b => /(^|-)(close|close-x|cancel)$/.test(b.id) && !b.disabled && !b.closest('.hidden'));
    if (closer) closeTop();
  }, true);

  // Capture phase: runs before the legacy per-modal Esc listeners, so exactly
  // one layer closes per key press.
  document.addEventListener('keydown', onKeyDown, true);
  document.addEventListener('focusin', onFocusIn);
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => scan());
  else scan();
  // Runtime-built overlays (presentation, knowledge graph) are appended to <body>;
  // watching only its direct children keeps preview re-renders out of this path.
  const watchBody = () => new MutationObserver(records => {
    for (const r of records) for (const n of r.addedNodes) {
      if (n.nodeType !== 1) continue;
      if (n.matches('[role="dialog"]')) { if (n.id && /-modal$/.test(n.id)) watch(n); }
      else scan(n);
    }
  }).observe(document.body, { childList: true });
  if (document.body) watchBody();
  else document.addEventListener('DOMContentLoaded', watchBody);

  const api = {
    /** Show a dialog element (by id or node). */
    open(target) { const el = typeof target === 'string' ? document.getElementById(target) : target; if (!el) return; watch(el); el.classList.remove('hidden'); sync(el); },
    /** Hide a dialog element (by id or node). */
    close(target) { const el = typeof target === 'string' ? document.getElementById(target) : target; if (!el) return; el.classList.add('hidden'); sync(el); },
    /** `fn()` returning false vetoes an Esc close of dialog `id`. */
    setGuard(id, fn) { if (fn) guards.set(id, fn); else guards.delete(id); },
    top() { return stack[stack.length - 1]?.el || null; },
    depth() { return stack.length; },
    closeTop,
  };
  if (typeof window !== 'undefined') window.ReadMDModal = api;
})();
