/* ReadMD's own companion controls; no third-party character assets or hooks. */
(function () {
  'use strict';
  const FALLBACKS = {
    'ux.petSections': '桌宠设置分组',
    'ux.petCharacters': '角色库',
    'ux.petCompanion': '陪伴方式',
    'ux.petSettings': '显示与运行',
    'ux.petStyle': '选择陪伴节奏',
    'ux.petSocial': '互动陪伴',
    'ux.petQuiet': '安静阅读',
    'ux.petQuietHint': '安静模式暂停主动问候和阅读提醒；你仍可点击互动，文件处理结果也会正常提示。',
    'ux.petActions': '快捷互动',
    'ux.petHello': '打个招呼',
    'ux.petGreeting': '我在这里，陪你读完这一页。',
    'ux.petPreviewHint': '角色外观预览 · 拖放文件到桌宠即可转换',
    'ux.petLive2d': 'Live2D · 独立桌面',
    'ux.petSprite': '精灵动画',
    'ai.title': 'AI 助手',
    'pet.gallery.hermes': '伴读使者'
  };
  const t = (key, params) => {
    const val = window.i18n?.t(key, params);
    if (val && val !== key) return val;
    return FALLBACKS[key] || key;
  };
  let gallery = { pets: [], active: '', catalog: [] }, status = null, activeTab = 'characters', initialized = false;
  let choosing = false, lastTap = 0, query = '', catalogPets = [];
  try { window.petQuietMode = localStorage.getItem('readmd.pet.quiet') === 'true'; } catch (_) { window.petQuietMode = false; }

  function getFavorites() {
    try {
      const raw = localStorage.getItem('readmd.pet.favorites');
      if (raw) {
        const arr = JSON.parse(raw);
        if (Array.isArray(arr)) return new Set(arr);
      }
    } catch (_) {}
    return new Set();
  }

  function toggleFavorite(slug) {
    const favs = getFavorites();
    if (favs.has(slug)) {
      favs.delete(slug);
    } else {
      favs.add(slug);
    }
    try {
      localStorage.setItem('readmd.pet.favorites', JSON.stringify(Array.from(favs)));
    } catch (_) {}
    renderRoster();
  }

  async function loadCatalog() {
    try {
      const res = await fetch('/assets/pet/catalog.json');
      if (res.ok) {
        const data = await res.json();
        if (Array.isArray(data) && data.length) {
          catalogPets = data;
          renderRoster();
        }
      }
    } catch (_) {}
  }

  function translateWorkbench() {
    const box = document.getElementById('pet-settings-box');
    if (!box) return;
    const tabs = box.querySelector('.pet-workbench-tabs');
    if (tabs) {
      tabs.setAttribute('aria-label', t('ux.petSections'));
      tabs.querySelectorAll('button[data-pet-section]').forEach(btn => {
        const key = btn.getAttribute('data-i18n');
        if (key) btn.textContent = t(key);
      });
    }
    const companion = box.querySelector('.pet-companion-controls');
    if (companion) {
      companion.querySelectorAll('[data-i18n]').forEach(el => {
        const key = el.getAttribute('data-i18n');
        if (key) el.textContent = t(key);
      });
    }
    const hint = box.querySelector('.pet-workbench-preview-hint');
    if (hint) {
      hint.textContent = t('ux.petPreviewHint');
    }
    const search = document.getElementById('pet-roster-search');
    if (search) search.placeholder = t('toolbar.search');
    for (const [id, key] of [['pet-quick-chat', 'ai.title'], ['pet-quick-quiet', 'ux.petQuiet']]) {
      const button = document.getElementById(id);
      if (button) { button.title = t(key); button.setAttribute('aria-label', t(key)); }
    }
  }

  function init() {
    if (initialized) return;
    const box = document.getElementById('pet-settings-box'); if (!box) return; initialized = true;
    box.classList.add('pet-workbench');
    const tabs = document.createElement('nav');
    tabs.className = 'pet-workbench-tabs';
    tabs.setAttribute('aria-label', t('ux.petSections'));
    tabs.setAttribute('data-i18n-aria', 'ux.petSections');
    for (const [id, key] of [['characters','ux.petCharacters'],['companion','ux.petCompanion'],['settings','ux.petSettings']]) {
      const button = document.createElement('button');
      button.type = 'button';
      button.dataset.petSection = id;
      button.setAttribute('data-i18n', key);
      button.textContent = t(key);
      button.onclick = () => { activeTab = id; renderTabs(); };
      tabs.appendChild(button);
    }
    box.querySelector('.pet-sheet-body').before(tabs);
    const list = box.querySelector('.apple-grouped-list');
    list.querySelectorAll('.apple-list-row').forEach(row => {
      row.dataset.petSectionContent = row.querySelector('#pet-gallery') ? 'characters' : row.querySelector('#pet-bubble-toggle') ? 'companion' : 'settings';
    });
    const roster = document.createElement('section');
    roster.id = 'pet-roster';
    roster.dataset.petSectionContent = 'characters';
    list.prepend(roster);
    const search = document.createElement('input');
    search.id = 'pet-roster-search'; search.type = 'search';
    search.dataset.petSectionContent = 'characters';
    search.placeholder = t('toolbar.search'); search.setAttribute('aria-label', t('toolbar.search'));
    search.oninput = () => { query = search.value.trim().toLocaleLowerCase(); if (list) list.scrollTop = 0; renderRoster(); };
    roster.before(search);

    const rendererRow = document.getElementById('pet-renderer-row') || document.getElementById('pet-renderer')?.closest('.apple-list-row');
    if (rendererRow) {
      rendererRow.dataset.petSectionContent = 'characters';
      search.before(rendererRow);
    }
    const rendererSelect = document.getElementById('pet-renderer');
    if (rendererSelect) {
      rendererSelect.addEventListener('change', () => {
        rendererSelect.dataset.userChanged = 'true';
        if (list) list.scrollTop = 0;
        renderRoster();
      });
    }

    const galleryRow = document.getElementById('pet-gallery-row') || document.getElementById('pet-gallery')?.closest('.apple-list-row');
    if (galleryRow) {
      galleryRow.dataset.petSectionContent = 'characters';
      search.after(galleryRow);
    }

    const feedback = document.createElement('p');
    feedback.id = 'pet-choice-feedback'; feedback.setAttribute('role', 'status');
    roster.after(feedback);

    const companion = document.createElement('section');
    companion.dataset.petSectionContent = 'companion';
    companion.className = 'pet-companion-controls';
    companion.innerHTML = `<h4 data-i18n="ux.petStyle">${t('ux.petStyle')}</h4><div class="pet-mode-buttons"><button id="pet-mode-social" type="button" data-i18n="ux.petSocial">${t('ux.petSocial')}</button><button id="pet-mode-quiet" type="button" data-i18n="ux.petQuiet">${t('ux.petQuiet')}</button></div><p data-i18n="ux.petQuietHint">${t('ux.petQuietHint')}</p><h4 data-i18n="ux.petActions">${t('ux.petActions')}</h4><div class="pet-action-buttons"><button id="pet-say-hello" type="button" data-i18n="ux.petHello">${t('ux.petHello')}</button><button id="pet-chat-open" type="button"><span data-i18n="ai.title">${t('ai.title')}</span> ↗</button></div>`;
    list.appendChild(companion);

    companion.querySelector('#pet-mode-social').onclick = () => quiet(false);
    companion.querySelector('#pet-mode-quiet').onclick = () => quiet(true);
    companion.querySelector('#pet-say-hello').onclick = () => { if (typeof handlePetInteractiveClick === 'function') handlePetInteractiveClick(); else window.showPetBubble?.(t('ux.petGreeting'), 3500, 2); };
    const chat = () => { window.closePetSettings?.(); window.openAiPanelWithPrompt?.(null, '', null); };
    companion.querySelector('#pet-chat-open').onclick = chat;
    const quickbar = document.querySelector('.pet-widget-quick-bar');
    for (const [id, key, icon, action] of [
      ['pet-quick-chat', 'ai.title', '<path d="M21 11.5a8.4 8.4 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.4 8.4 0 0 1-3.8-.9L3 21l1.9-5.7a8.4 8.4 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.4 8.4 0 0 1 3.8-.9H13a8.5 8.5 0 0 1 8 8z"/>', chat],
      ['pet-quick-quiet', 'ux.petQuiet', '<path d="M21 12.8A9 9 0 0 1 11.2 3 9 9 0 1 0 21 12.8z"/>', () => quiet(!window.petQuietMode)]
    ]) {
      if (!quickbar) break;
      const button = document.createElement('button');
      button.id = id; button.type = 'button'; button.className = 'pet-quick-btn';
      button.title = t(key); button.setAttribute('aria-label', t(key));
      button.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">${icon}</svg>`;
      button.onclick = action; quickbar.prepend(button);
    }
    const preview = box.querySelector('.pet-preview-stage');
    const enabledRow = document.getElementById('pet-enabled')?.closest('.apple-list-row');
    if (enabledRow) { delete enabledRow.dataset.petSectionContent; preview.appendChild(enabledRow); }
    const speech = document.createElement('p');
    speech.id = 'pet-interaction-preview'; speech.setAttribute('role', 'status'); preview.appendChild(speech);
    window.addEventListener('readmd:pet-message', e => {
      if (!document.getElementById('pet-settings-modal')?.classList.contains('hidden')) speech.textContent = e.detail.text;
    });
    const hint = document.createElement('p');
    hint.className = 'pet-workbench-preview-hint';
    hint.setAttribute('data-i18n', 'ux.petPreviewHint');
    hint.textContent = t('ux.petPreviewHint');
    preview.appendChild(hint);
    quiet(window.petQuietMode, false);
    void loadCatalog();
    renderTabs();
    renderRoster();

    // Local UI feedback only. No global keyboard hook, recording or background input collection.
    document.addEventListener('keydown', e => {
      if (e.repeat || window.petQuietMode || document.hidden || matchMedia('(prefers-reduced-motion: reduce)').matches || Date.now()-lastTap<500) return;
      const pet = document.getElementById('readmd-pet-widget');
      if (!pet || pet.classList.contains('hidden') || !e.target.matches('textarea,[contenteditable="true"],.cm-content')) return;
      lastTap = Date.now(); pet.classList.add('pet-input-react'); setTimeout(() => pet.classList.remove('pet-input-react'), 220);
    });
  }
  function quiet(value, persist = true) {
    window.petQuietMode = value;
    if (persist) { try { localStorage.setItem('readmd.pet.quiet', String(value)); } catch (_) {} }
    if (value && typeof hidePetBubble === 'function') hidePetBubble();
    document.getElementById('readmd-pet-widget')?.classList.toggle('pet-quiet', value);
    document.getElementById('pet-mode-social')?.setAttribute('aria-pressed', String(!value));
    document.getElementById('pet-mode-quiet')?.setAttribute('aria-pressed', String(value));
    document.getElementById('pet-quick-quiet')?.setAttribute('aria-pressed', String(value));
  }
  function renderTabs() {
    const box = document.getElementById('pet-settings-box');
    if (box) box.dataset.petTab = activeTab;
    const list = document.querySelector('#pet-settings-box .apple-grouped-list');
    if (list) list.scrollTop = 0;
    document.querySelectorAll('[data-pet-section]').forEach(btn => btn.setAttribute('aria-current', btn.dataset.petSection === activeTab ? 'page' : 'false'));
    document.querySelectorAll('[data-pet-section-content]').forEach(el => el.classList.toggle('pet-section-inactive', el.dataset.petSectionContent !== activeTab));
  }
  async function choose(renderer, slug) {
    if (choosing) return; choosing = true;
    const previousSlug = gallery.active;
    gallery.active = slug;
    if (typeof currentActivePetSlug !== 'undefined') currentActivePetSlug = slug;
    renderRoster();
    const feedback = document.getElementById('pet-choice-feedback');
    feedback.textContent = '';
    let changedGallery = false;
    try {
      if (renderer === 'hermes-sprite') {
        const result = await petGalleryRequest('/api/pets/active', { slug, confirm: true });
        if (!result.ok) throw new Error(result.error_code || result.code);
        changedGallery = true;
        await refreshPetGallery();
      }
      const rendererSelect = document.getElementById('pet-renderer');
      if (rendererSelect) {
        rendererSelect.value = renderer;
        delete rendererSelect.dataset.userChanged;
      }
      if (renderer === 'live2d') document.getElementById('pet-runtime').value = 'desktop';
      const result = await savePetSettings();
      if (!result?.ok) throw new Error(result?.code || 'pet_config_failed');
    } catch (error) {
      if (changedGallery) {
        gallery.active = previousSlug;
        if (typeof currentActivePetSlug !== 'undefined') currentActivePetSlug = previousSlug;
        await petGalleryRequest('/api/pets/active', { slug: previousSlug, confirm: true }).catch(() => null);
        await refreshPetGallery().catch(() => null);
      }
      feedback.textContent = petT('pet.configFailed', { code: error.message });
    }
    finally {
      choosing = false; renderRoster();
      Array.from(document.querySelectorAll('#pet-roster button')).find(button => button.dataset.slug === slug && button.dataset.renderer === renderer)?.focus({ preventScroll: true });
    }
  }
  function renderRoster() {
    const host = document.getElementById('pet-roster'); if (!host) return;
    const focused = host.contains(document.activeElement) ? document.activeElement.dataset : null;
    const focusedSlug = focused?.slug, focusedRenderer = focused?.renderer;
    host.setAttribute('aria-busy', String(choosing));
    host.replaceChildren();
    host.scrollTop = 0;
    const select = document.getElementById('pet-gallery'); if (select) select.disabled = choosing;

    const rendererSelect = document.getElementById('pet-renderer');
    const filterRenderer = rendererSelect?.value || (status?.preferences?.renderer === 'live2d' ? 'live2d' : 'hermes-sprite');

    const defaultPetName = t('pet.gallery.hermes') || '伴读使者';

    // Group 0: User's 3 created pets
    const customSlugs = ['mochi', 'moss', 'amber'];
    const customPets = [
      { slug: 'mochi', display_name: '糯米 / Mochi', renderer: 'hermes-sprite', groupPriority: 0 },
      { slug: 'moss', display_name: '苔苔 / Moss', renderer: 'hermes-sprite', groupPriority: 0 },
      { slug: 'amber', display_name: '琥珀 / Amber', renderer: 'hermes-sprite', groupPriority: 0 }
    ];

    // Group 1: 伴读使者
    const companionPet = { slug: '', display_name: defaultPetName, renderer: 'hermes-sprite', groupPriority: 1 };

    // Group 2: 73 loaded sprite pets from catalog
    const spriteCatalog = (catalogPets.length ? catalogPets : (gallery.catalog || [])).map(p => ({
      slug: p.slug,
      display_name: p.zh_name || p.display_name || p.name || p.slug,
      renderer: 'hermes-sprite',
      groupPriority: 2
    }));

    // Other installed pets from gallery (e.g. test custom-cat or user imports)
    const otherInstalled = (gallery.pets || [])
      .filter(p => !customSlugs.includes(p.slug) && p.slug !== '' && !spriteCatalog.some(c => c.slug === p.slug))
      .map(p => ({
        ...p,
        renderer: 'hermes-sprite',
        groupPriority: 2
      }));

    // Live2D pets
    const live2dPets = [
      { slug: 'arch-chan', display_name: 'Arch-Chan', renderer: 'live2d', groupPriority: 0 }
    ];

    const allItems = [...customPets, companionPet, ...spriteCatalog, ...otherInstalled, ...live2dPets];
    allItems.forEach((it, idx) => { it.originalIndex = idx; });

    // Filter by selected renderer:
    const filtered = allItems.filter(item => {
      if (item.renderer !== filterRenderer) return false;
      const label = (item.slug && petT('pet.preset.' + item.slug)) || item.display_name || item.slug;
      if (query && !`${label} ${item.slug} ${item.renderer}`.toLocaleLowerCase().includes(query)) return false;
      return true;
    });

    const favorites = getFavorites();
    filtered.sort((a, b) => {
      const favA = favorites.has(a.slug) ? 0 : 1;
      const favB = favorites.has(b.slug) ? 0 : 1;
      if (favA !== favB) return favA - favB;
      if (a.groupPriority !== b.groupPriority) return a.groupPriority - b.groupPriority;
      return a.originalIndex - b.originalIndex;
    });

    for (const item of filtered) {
      const label = (item.slug && petT('pet.preset.' + item.slug)) || item.display_name || item.slug;
      const live = item.renderer === 'live2d';
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'pet-character-card';
      button.disabled = choosing;
      const currentActive = status?.active_slug ?? gallery.active;
      const selected = live
        ? status?.preferences?.renderer === 'live2d'
        : status?.preferences?.renderer !== 'live2d' && item.slug === currentActive;
      button.setAttribute('aria-pressed', String(selected));
      button.dataset.renderer = item.renderer;
      button.dataset.slug = item.slug;

      const favBtn = document.createElement('span');
      favBtn.role = 'button';
      favBtn.tabIndex = 0;
      const isFav = favorites.has(item.slug);
      favBtn.className = 'pet-fav-btn' + (isFav ? ' is-favorite' : '');
      favBtn.setAttribute('aria-label', isFav ? '已收藏' : '收藏');
      favBtn.title = isFav ? '已收藏' : '收藏';
      favBtn.innerHTML = `<svg viewBox="0 0 24 24"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon></svg>`;
      const onFavClick = (e) => {
        e.stopPropagation();
        e.preventDefault();
        toggleFavorite(item.slug);
      };
      favBtn.onclick = onFavClick;
      favBtn.onkeydown = (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.stopPropagation();
          e.preventDefault();
          toggleFavorite(item.slug);
        }
      };
      button.appendChild(favBtn);

      const art = document.createElement('span');
      art.className = live ? 'pet-card-art live2d' : 'pet-card-art sprite';
      art.style.backgroundImage = live
        ? 'url("/assets/pet/arch-chan-avatar.png")'
        : item.slug
          ? `url("/api/pets/thumb?slug=${encodeURIComponent(item.slug)}")`
          : 'url("/assets/pet/hermes-sprite.png")';

      const name = document.createElement('strong');
      name.textContent = label;
      const kind = document.createElement('small');
      const kindKey = live ? 'ux.petLive2d' : 'ux.petSprite';
      kind.setAttribute('data-i18n', kindKey);
      kind.textContent = t(kindKey);

      button.append(art, name, kind);
      button.onclick = () => choose(item.renderer, item.slug);
      host.appendChild(button);

      if (!choosing && focusedSlug === item.slug && focusedRenderer === item.renderer) {
        button.focus({ preventScroll: true });
      }
    }
    if (!host.childElementCount) {
      const empty = document.createElement('p');
      empty.textContent = t('ux.noResults');
      host.appendChild(empty);
    }
  }

  window.addEventListener('readmd:pet-state', e => {
    status = e.detail;
    const rendererSelect = document.getElementById('pet-renderer');
    if (rendererSelect && status?.preferences?.renderer && !rendererSelect.dataset.userChanged) {
      rendererSelect.value = status.preferences.renderer;
    }
    renderRoster();
  });
  window.addEventListener('readmd:pet-gallery', e => {
    gallery = e.detail;
    if (Array.isArray(gallery.catalog) && gallery.catalog.length) {
      catalogPets = gallery.catalog;
    }
    renderRoster();
  });
  window.addEventListener('readmd:language-changed', () => { translateWorkbench(); renderRoster(); });
  window.addEventListener('readmd:pet-open-settings', () => {
    const box = document.getElementById('pet-settings-box');
    if (box) box.dataset.petTab = activeTab;
    const rendererSelect = document.getElementById('pet-renderer');
    if (rendererSelect && status?.preferences?.renderer) {
      rendererSelect.value = status.preferences.renderer;
      delete rendererSelect.dataset.userChanged;
    }
    translateWorkbench();
    renderRoster();
  });
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init); else init();
})();
