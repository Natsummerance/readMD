'use strict';

let activeConfirmFinish = null;

function confirmAction({ title, message = '', confirmText, cancelText, danger = false } = {}) {
  const modal = $('confirm-modal');
  if (!modal) return Promise.resolve(false);
  if (activeConfirmFinish) activeConfirmFinish(false);

  return new Promise(resolve => {
    const opener = document.activeElement;
    const confirmButton = $('confirm-action');
    const cancelButton = $('confirm-cancel');
    let finished = false;
    const finish = value => {
      if (finished) return;
      finished = true;
      activeConfirmFinish = null;
      confirmButton.removeEventListener('click', onConfirm);
      cancelButton.removeEventListener('click', onCancel);
      modal.removeEventListener('keydown', onKeyDown);
      modal.removeEventListener('click', onClick);
      modal.classList.add('hidden');
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus({ preventScroll: true });
      resolve(value);
    };
    const onKeyDown = event => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      finish(false);
    };
    const onConfirm = () => finish(true);
    const onCancel = () => finish(false);
    const onClick = event => {
      if (event.target === modal) finish(false);
    };

    activeConfirmFinish = () => finish(false);

    $('confirm-title').textContent = title || '';
    $('confirm-message').textContent = message;
    confirmButton.textContent = confirmText || 'OK';
    cancelButton.textContent = cancelText || 'Cancel';
    confirmButton.className = danger ? 'tb-btn danger' : 'tb-btn accent';

    confirmButton.addEventListener('click', onConfirm);
    cancelButton.addEventListener('click', onCancel);
    modal.addEventListener('keydown', onKeyDown);
    modal.addEventListener('click', onClick);
    modal.classList.remove('hidden');
    setTimeout(() => cancelButton.focus({ preventScroll: true }), 0);
  });
}
