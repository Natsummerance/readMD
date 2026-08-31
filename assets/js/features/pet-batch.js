'use strict';
/* Hermes pet -> existing ReadMD batch-workbench bridge.
   No second modal is created: a drop reuses the shared confirmation dialog
   and then the normal batch workbench, with its existing focus/i18n styling. */

const petBatchInbox = [];
let petBatchConfirming = false;

function petBatchText(key) {
  return window.i18n ? window.i18n.t(key) : key;
}

async function receivePetBatch(paths) {
  const safePaths = (paths || []).filter(path => typeof path === 'string' && path);
  if (!safePaths.length) return;
  petBatchInbox.push(safePaths);
  if (petBatchConfirming) return;
  petBatchConfirming = true;
  try {
    while (petBatchInbox.length) {
      const next = petBatchInbox.shift();
      const confirmed = await confirmAction({
        title: petBatchText('batch.title'),
        message: petBatchText('batch.note'),
        confirmText: petBatchText('dialog.confirm'),
        cancelText: petBatchText('dialog.cancel'),
      });
      if (confirmed) await enqueueBatchFiles(next, false);
    }
  } finally {
    petBatchConfirming = false;
  }
}

window.receivePetBatch = receivePetBatch;

function openPetQuickMenu() {
  const trigger = $('btn-more');
  const menu = $('more-menu');
  if (trigger && menu && !menu.classList.contains('open')) trigger.click();
}

window.openPetQuickMenu = openPetQuickMenu;

async function pollPetBatch() {
  try {
    const response = await apiFetch('/api/control/pet-batch');
    const payload = await response.json();
    if (payload && payload.pending) receivePetBatch(payload.paths);
  } catch (_error) { /* native bridge may be unavailable in browser mode */ }
}

async function pollPetQuickMenu() {
  try {
    const response = await apiFetch('/api/control/pet-menu');
    const payload = await response.json();
    if (payload && payload.pending) openPetQuickMenu();
  } catch (_error) { /* native bridge may be unavailable in browser mode */ }
}

window.addEventListener('load', () => {
  setTimeout(() => {
    setInterval(pollPetBatch, 1000);
    setInterval(pollPetQuickMenu, 1000);
  }, 1200);
});
