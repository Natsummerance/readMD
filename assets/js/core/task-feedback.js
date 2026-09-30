'use strict';

/*
 * Single-flight task runner for long operations (export, conversion, OCR).
 *
 *   ReadMDTask.run('export', fn, { trigger, status })
 *
 * - A second call with the same key while one is running is ignored (no
 *   duplicate request reaches the server, however fast the user clicks).
 * - `trigger` buttons are disabled and marked `aria-busy` while running.
 * - `status` (an element) shows "working… 12 s", switching to a stalled hint
 *   after 120 s without `progress()` calls, and is cleared on completion.
 * - `cancel: { id, button, onState }` shows `button` while running; a click
 *   posts `/api/task/cancel {id}` once and reports the kernel's answer
 *   (`cancelling` | `finished` | `unknown`) to `onState`.
 * - State is idle → running → (succeeded | failed | cancelled); exactly one
 *   terminal state.
 */
(function () {
  const STALL_MS = 120000;
  const running = new Map(); // key -> { startedAt, lastProgress, timer, triggers, status }

  const tr = (k, p, fallback) => {
    const text = window.i18n ? window.i18n.t(k, p) : k;
    return text && text !== k ? text : fallback;
  };

  function paint(key) {
    const job = running.get(key);
    if (!job || !job.status) return;
    const now = Date.now();
    const secs = Math.floor((now - job.startedAt) / 1000);
    const stalled = now - job.lastProgress >= STALL_MS;
    job.status.classList.toggle('is-stalled', stalled);
    job.painted = stalled
      ? tr('task.stalled', null, 'Still working — this is taking longer than usual')
      : (job.label || tr('task.running', { seconds: secs }, `Working… ${secs} s`)).replace('{seconds}', String(secs));
    job.status.textContent = job.painted;
  }

  function setBusy(els, on) {
    for (const el of els) {
      if (!el) continue;
      if (on) {
        el.dataset.taskWasDisabled = el.disabled ? '1' : '';
        el.disabled = true;
        el.setAttribute('aria-busy', 'true');
        el.classList.add('is-busy');
      } else {
        el.disabled = el.dataset.taskWasDisabled === '1';
        delete el.dataset.taskWasDisabled;
        el.removeAttribute('aria-busy');
        el.classList.remove('is-busy');
      }
    }
  }

  const byId = t => (typeof t === 'string' ? document.getElementById(t) : t);

  // Ask the kernel to cancel a task registered with `task_id` (or a batch job).
  async function requestCancel(id) {
    if (!id) return 'unknown';
    try {
      const fetcher = typeof apiFetch === 'function' ? apiFetch : fetch;
      const r = await fetcher('/api/task/cancel', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id }),
      });
      const d = await r.json().catch(() => ({}));
      return d.state || 'unknown';
    } catch (e) {
      return 'unknown';
    }
  }

  function newTaskId(prefix) {
    const rand = (window.crypto && crypto.randomUUID) ? crypto.randomUUID() : Math.random().toString(36).slice(2) + Date.now().toString(36);
    return (prefix || 'task') + '-' + rand;
  }

  async function run(key, fn, opts = {}) {
    if (running.has(key)) return undefined;
    const triggers = [].concat(opts.trigger || []).map(byId).filter(Boolean);
    const status = typeof opts.status === 'string' ? document.getElementById(opts.status) : opts.status || null;
    const cancelBtn = opts.cancel ? byId(opts.cancel.button) : null;
    const job = { startedAt: Date.now(), lastProgress: Date.now(), triggers, status, label: opts.label || '', cancelled: false };
    running.set(key, job);
    setBusy(triggers, true);
    const onCancel = async () => {
      if (job.cancelled) return;
      job.cancelled = true;
      cancelBtn.disabled = true;
      const state = await requestCancel(opts.cancel.id);
      if (opts.cancel.onState) opts.cancel.onState(state);
    };
    if (cancelBtn) {
      cancelBtn.disabled = false;
      cancelBtn.classList.remove('hidden');
      cancelBtn.addEventListener('click', onCancel);
    }
    if (status) {
      status.classList.remove('hidden', 'ok', 'err');
      status.classList.add('is-running');
      status.setAttribute('role', 'status');
      status.setAttribute('aria-live', 'polite');
      paint(key);
      job.timer = setInterval(() => paint(key), 1000);
    }
    try {
      return await fn({
        progress: label => { job.lastProgress = Date.now(); if (label) job.label = label; paint(key); },
        isCancelled: () => job.cancelled,
      });
    } finally {
      clearInterval(job.timer);
      if (cancelBtn) {
        cancelBtn.removeEventListener('click', onCancel);
        cancelBtn.classList.add('hidden');
      }
      if (status) {
        status.classList.remove('is-running', 'is-stalled');
        // Leave a result the task wrote; only clear our own progress text.
        if (status.textContent === job.painted) status.textContent = '';
      }
      setBusy(triggers, false);
      running.delete(key);
    }
  }

  const api = { run, isRunning: key => running.has(key), newTaskId, requestCancel, STALL_MS };
  if (typeof window !== 'undefined') window.ReadMDTask = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
