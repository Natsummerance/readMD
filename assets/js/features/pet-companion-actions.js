/* Controls backed by the same persistent companion service as the desktop pet. */
(() => {
  const t = (key, params) => window.i18n?.t(key, params) || key;
  const fallbackText = {
    'pet.action.pet': '摸摸头',
    'pet.action.feed': '喂食',
    'pet.action.play': '玩耍',
    'pet.action.rest': '休息',
    'pet.action.wake': '唤醒',
    'pet.action.pet.done': '摸摸头真舒服，继续陪你读书！',
    'pet.action.feed.done': '谢谢投喂，补充好体力啦！',
    'pet.action.play.done': '玩得很开心！和你更亲近啦。',
    'pet.action.rest.done': '我先休息一会儿，慢慢恢复体力。',
    'pet.action.wake.done': '醒来啦，继续陪着你！',
    'pet.life.level': '等级',
    'pet.life.energy': '体力',
    'pet.life.mood': '心情',
    'pet.life.affection': '亲密度',
    'pet.life.needsRest': '体力不足，先喂食或休息一会儿吧。',
  };
  const getI18n = (key, params) => {
    const res = t(key, params);
    if (res && res !== key) return res;
    if (key === 'pet.life.cooldown' && params?.seconds !== undefined) {
      return `过 ${params.seconds} 秒再来一次吧。`;
    }
    return fallbackText[key] || res;
  };

  let current;
  function render(value) {
    current = value || current;
    const stats = document.getElementById('pet-life-stats');
    if (!stats || !current) return;
    stats.textContent = `${getI18n('pet.life.level')} ${current.level} · ${getI18n('pet.life.energy')} ${current.energy} · ${getI18n('pet.life.mood')} ${current.mood} · ${getI18n('pet.life.affection')} ${current.affection}`;
    document.querySelector('[data-pet-action="rest"]')?.classList.toggle('hidden', current.resting);
    document.querySelector('[data-pet-action="wake"]')?.classList.toggle('hidden', !current.resting);
  }

  function init() {
    const host = document.querySelector('.pet-companion-controls');
    if (!host || document.getElementById('pet-life-stats')) return;
    const life = document.createElement('section'); life.className = 'pet-life-controls';
    life.innerHTML = '<p id="pet-life-stats" role="status"></p><div class="pet-action-buttons"></div><p id="pet-life-feedback" role="status"></p>';
    for (const action of ['pet', 'feed', 'play', 'rest', 'wake']) {
      const button = document.createElement('button'); button.type = 'button';
      button.dataset.petAction = action; button.dataset.i18n = 'pet.action.' + action;
      button.textContent = getI18n('pet.action.' + action);
      button.onclick = async () => {
        life.querySelectorAll('button').forEach(item => { item.disabled = true; });
        try {
          const result = await petGalleryRequest('/api/pets/interact', { action });
          if (result && result.companion) {
            render(result.companion);
          }
          const feedback = document.getElementById('pet-life-feedback');
          let feedbackText = '';
          if (result.ok) {
            feedbackText = getI18n('pet.action.' + action + '.done');
          } else if (result.code === 'pet_action_cooldown') {
            feedbackText = getI18n('pet.life.cooldown', { seconds: result.retry_after });
          } else if (result.code === 'pet_needs_rest') {
            feedbackText = getI18n('pet.life.needsRest');
          } else {
            feedbackText = t('pet.configFailed', { code: result.code || result.error_code || 'interact_failed' });
          }
          if (feedback) {
            feedback.textContent = feedbackText;
          }
          if (feedbackText) {
            if (typeof window.showPetBubble === 'function') {
              window.showPetBubble(feedbackText, 4500, 2);
            }
            window.dispatchEvent(new CustomEvent('readmd:pet-message', {
              detail: { text: feedbackText, priority: 2 }
            }));
          }
        } catch (_) {
          const feedback = document.getElementById('pet-life-feedback');
          const errorText = t('pet.configFailed', { code: 'pet_connection_failed' });
          if (feedback) {
            feedback.textContent = errorText;
          }
          if (typeof window.showPetBubble === 'function') {
            window.showPetBubble(errorText, 3500, 2);
          }
          window.dispatchEvent(new CustomEvent('readmd:pet-message', {
            detail: { text: errorText, priority: 2 }
          }));
        } finally { life.querySelectorAll('button').forEach(item => { item.disabled = false; }); }
      };
      life.querySelector('div').appendChild(button);
    }
    host.appendChild(life);
    if (!current && window.currentPetRuntimeStatus?.companion) {
      render(window.currentPetRuntimeStatus.companion);
    } else {
      render();
    }
  }
  window.addEventListener('readmd:pet-state', e => { render(e.detail?.companion); });
  window.addEventListener('readmd:language-changed', () => render());
  window.addEventListener('readmd:pet-open-settings', init);
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init); else init();
})();

