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

const petT = (key, params) => window.i18n ? window.i18n.t(key, params) : key;

function setPetMenuStatus(status) {
  const label = $('pet-status-label');
  if (!label) return;
  if (status && status.enabled) label.textContent = petT('app.enabled');
  else if (status && status.adapter && status.adapter.available) label.textContent = petT('app.disabled');
  else label.textContent = petT('menu.petSub');
}

async function refreshPetMenuStatus() {
  if (!hasPy || !py.get_pet_runtime_status) return;
  try { setPetMenuStatus(await py.get_pet_runtime_status()); } catch (_error) { /* optional plugin */ }
}

async function configurePetFromMenu() {
  closeMoreMenu();
  if (!hasPy || !py.get_pet_runtime_status || !py.configure_pet) return;
  try {
    let status = await py.get_pet_runtime_status();
    if (status.enabled) {
      await py.configure_pet({ enabled: false, renderer: 'hermes-sprite' });
      await refreshPetMenuStatus();
      if (typeof showToast === 'function') showToast(petT('app.disabled'));
      return;
    }
    if (!status.adapter || !status.adapter.available) {
      const archive = py.choose_pet_plugin ? await py.choose_pet_plugin() : null;
      if (!archive) return;
      const confirmed = await confirmAction({
        title: petT('menu.pet'), message: petT('menu.petSub'),
        confirmText: petT('update.installNow'), cancelText: petT('dialog.cancel'),
      });
      if (!confirmed) return;
      const installed = await py.install_pet_plugin(archive, true);
      if (!installed || !installed.ok) {
        if (typeof showToast === 'function') showToast(petT('app.failed'));
        await refreshPetMenuStatus();
        return;
      }
      status = await py.get_pet_runtime_status();
    }
    const enabled = await py.configure_pet({ enabled: true, renderer: 'hermes-sprite' });
    if (!enabled || !enabled.ok) {
      if (typeof showToast === 'function') showToast(petT('app.failed'));
    } else if (typeof showToast === 'function') {
      showToast(petT('app.enabled'));
    }
    await refreshPetMenuStatus();
  } catch (_error) {
    if (typeof showToast === 'function') showToast(petT('app.failed'));
  }
}

window.configurePetFromMenu = configurePetFromMenu;
window.refreshPetMenuStatus = refreshPetMenuStatus;

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
    refreshPetMenuStatus();
    setInterval(pollPetBatch, 1000);
    setInterval(pollPetQuickMenu, 1000);
  }, 1200);
});

window.addEventListener('readmd:language-changed', refreshPetMenuStatus);
