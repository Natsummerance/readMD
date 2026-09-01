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

let activePetSettingsStatus = null;

function petPercent(value, fallback) {
  const number = Number(value);
  return Math.round((Number.isFinite(number) ? number : fallback) * 100);
}

function renderPetSettings(status) {
  activePetSettingsStatus = status || null;
  const preferences = status && status.preferences ? status.preferences : {};
  const enabled = $('pet-enabled');
  const scale = $('pet-scale');
  const opacity = $('pet-opacity');
  if (enabled) enabled.checked = Boolean(status && status.enabled);
  if (scale) scale.value = String(petPercent(preferences.scale, 0.33));
  if (opacity) opacity.value = String(petPercent(preferences.opacity, 1));
  updatePetRangeLabels();
  const install = $('pet-install');
  if (install) install.classList.toggle('hidden', Boolean(status && status.adapter && status.adapter.available));
}

function updatePetRangeLabels() {
  const scale = $('pet-scale');
  const opacity = $('pet-opacity');
  if ($('pet-scale-value') && scale) $('pet-scale-value').textContent = scale.value + '%';
  if ($('pet-opacity-value') && opacity) $('pet-opacity-value').textContent = opacity.value + '%';
}

function closePetSettings() {
  $('pet-settings-modal')?.classList.add('hidden');
}

async function installPetPlugin() {
  if (!hasPy || !py.choose_pet_plugin || !py.install_pet_plugin) return false;
  const archive = await py.choose_pet_plugin();
  if (!archive) return false;
  const confirmed = await confirmAction({
    title: petT('menu.pet'), message: petT('menu.petSub'),
    confirmText: petT('update.installNow'), cancelText: petT('dialog.cancel'),
  });
  if (!confirmed) return false;
  const result = await py.install_pet_plugin(archive, true);
  if (!result || !result.ok) {
    if (typeof showToast === 'function') showToast(petT('app.failed'));
    return false;
  }
  return true;
}

async function savePetSettings({ allowInstall = false } = {}) {
  if (!hasPy || !py.get_pet_runtime_status || !py.configure_pet) return;
  const enabled = Boolean($('pet-enabled')?.checked);
  let status = activePetSettingsStatus || await py.get_pet_runtime_status();
  if (enabled && (!status.adapter || !status.adapter.available)) {
    if (!allowInstall || !(await installPetPlugin())) {
      renderPetSettings(status);
      return;
    }
    status = await py.get_pet_runtime_status();
  }
  const result = await py.configure_pet({
    enabled,
    opacity: Number($('pet-opacity')?.value || 100) / 100,
    renderer: 'hermes-sprite',
    scale: Number($('pet-scale')?.value || 33) / 100,
  });
  if (!result || !result.ok) {
    if (typeof showToast === 'function') showToast(petT('app.failed'));
  } else if (typeof showToast === 'function') {
    showToast(enabled ? petT('app.enabled') : petT('app.disabled'));
  }
  await refreshPetMenuStatus();
  renderPetSettings(await py.get_pet_runtime_status());
}

async function openPetSettings() {
  closeMoreMenu();
  if (!hasPy || !py.get_pet_runtime_status) return;
  const modal = $('pet-settings-modal');
  if (!modal) return;
  modal.classList.remove('hidden');
  try {
    renderPetSettings(await py.get_pet_runtime_status());
  } catch (_error) {
    if (typeof showToast === 'function') showToast(petT('app.failed'));
  }
}

window.openPetSettings = openPetSettings;
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
    $('pet-settings-close')?.addEventListener('click', closePetSettings);
    $('pet-install')?.addEventListener('click', async () => {
      if (await installPetPlugin()) renderPetSettings(await py.get_pet_runtime_status());
    });
    $('pet-enabled')?.addEventListener('change', () => { void savePetSettings({ allowInstall: true }); });
    $('pet-scale')?.addEventListener('input', updatePetRangeLabels);
    $('pet-opacity')?.addEventListener('input', updatePetRangeLabels);
    $('pet-scale')?.addEventListener('change', () => { void savePetSettings(); });
    $('pet-opacity')?.addEventListener('change', () => { void savePetSettings(); });
    setInterval(pollPetBatch, 1000);
    setInterval(pollPetQuickMenu, 1000);
  }, 1200);
});

window.addEventListener('readmd:language-changed', refreshPetMenuStatus);
