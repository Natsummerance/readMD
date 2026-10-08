/* Native browser motion. Existing videos, no frame archives or animation runtime. */
(() => {
  'use strict';
  const root = document.documentElement;
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  const finePointer = matchMedia('(hover: hover) and (pointer: fine)');
  const clamp = (v, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, v));
  const items = new Set(), visible = new Set();
  const selector = 'main h1,main h2,main h3,main p,main article,main pre,main details,main button,main summary,main input,main select,main a,main li,main figure,nav a,main .demo-card,main .product-frame,main .platform-card,main .apple-platform-card,main .button-primary,main .button-secondary,main .apple-pill-primary,main .apple-pill-secondary,footer';
  let raf = 0, last = 0, progress = 0, pageProgress = 0;
  const journey = document.querySelector('[data-readmd-cinema]');
  const stage = journey?.querySelector('.journey-stage');
  const captions = [...(journey?.querySelectorAll('.journey-caption') || [])];
  const poster = stage?.querySelector('img');
  const clips = captions.map(c => ({video: c.dataset.video, webm: c.dataset.webm, poster: c.dataset.poster}));
  let journeyTop = 0, journeyRange = 1, journeyVisible = false, active = -1;
  const slots = [];

  const wake = () => { if (!raf && !document.hidden && !reduced.matches) raf = requestAnimationFrame(tick); };
  const measure = () => {
    if (journey) {
      journeyTop = journey.getBoundingClientRect().top + scrollY;
      journeyRange = Math.max(1, journey.offsetHeight - innerHeight + 56);
    }
    wake();
  };
  const observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        visible.add(entry.target); entry.target.classList.add('motion-seen');
      } else visible.delete(entry.target);
    }
    wake();
  }, {rootMargin: '0px 0px 40px', threshold: 0.08});
  function enroll(scope) {
    const found = [...(scope.matches?.(selector) ? [scope] : []), ...scope.querySelectorAll(selector)];
    for (const node of found) {
      if (items.has(node) || node.closest('[data-readmd-cinema],dialog,[role="dialog"]')) continue;
      // Observe every component; only visible components enter frame work.
      items.add(node); node.classList.add('motion-item');
      node.style.setProperty('--motion-delay', (items.size % 4) * 45 + 'ms');
      observer.observe(node);
    }
  }

  function present(slot) {
    const video = slot.video;
    if (!slot.visible || video.readyState < 2 || video.seeking) return;
    video.classList.add('cinema-decoded');
    stage.dataset.mediaReady = 'true';
    stage.dataset.presentedTime = video.currentTime.toFixed(3);
  }
  function seek(slot) {
    const video = slot.video;
    if (reduced.matches || document.hidden || !slot.visible || video.readyState < 1 || video.seeking) return;
    const target = clamp(slot.target, 0, Math.max(0, video.duration - 0.045));
    if (Math.abs(video.currentTime - target) > 0.018) {
      try { video.currentTime = target; } catch { /* Poster remains usable. */ }
    } else present(slot);
  }
  function createSlot() {
    const video = document.createElement('video');
    video.muted = true; video.playsInline = true; video.preload = 'metadata';
    video.setAttribute('aria-hidden', 'true'); video.tabIndex = -1;
    const slot = {video, index: -1, target: 0, visible: false};
    const decoded = () => {
      if (!slot.visible) return;
      present(slot);
      seek(slot);
    };
    video.addEventListener('loadedmetadata', () => { seek(slot); wake(); });
    video.addEventListener('loadeddata', decoded);
    video.addEventListener('seeked', decoded);
    video.addEventListener('error', () => {
      video.classList.remove('cinema-decoded');
      if (slot.visible) stage.dataset.mediaReady = 'poster';
    });
    stage.append(video); slots.push(slot); return slot;
  }
  function loadSlot(slot, index) {
    if (slot.index === index) return;
    slot.index = index; slot.target = 0;
    slot.video.classList.remove('cinema-decoded');
    slot.video.removeAttribute('src');
    slot.video.replaceChildren(...[[clips[index].webm,'video/webm'],[clips[index].video,'video/mp4']]
      .filter(([src]) => src).map(([src,type]) => {
        const source = document.createElement('source'); source.src = src; source.type = type; return source;
      }));
    slot.video.load();
  }
  function updateCinema(p) {
    if (!journey || !journeyVisible || reduced.matches || !clips.length) return;
    const scaled = clamp(p) * clips.length;
    const index = Math.min(clips.length - 1, Math.floor(scaled));
    const local = clamp(scaled - index);
    let slot = slots.find(s => s.index === index);
    if (!slot) {
      slot = slots.find(s => !s.visible) || (slots.length < 2 ? createSlot() : slots[0]);
      loadSlot(slot, index);
    }
    if (active !== index) {
      active = index;
      stage.dataset.clipIndex = String(index); stage.dataset.mediaReady = 'poster';
      poster.src = clips[index].poster;
      poster.alt = captions[index].querySelector('strong').textContent + ' · ReadMD';
      for (let n = 0; n < captions.length; n++) {
        captions[n].classList.toggle('is-active', n === index);
        captions[n].inert = n !== index;
        captions[n].setAttribute('aria-hidden', String(n !== index));
      }
    }
    for (const other of slots) {
      other.visible = other === slot;
      other.video.classList.toggle('cinema-visible', other.visible);
    }
    const video = slot.video;
    slot.target = Number.isFinite(video.duration) ? local * Math.max(0, video.duration - 0.045) : 0;
    stage.dataset.targetTime = slot.target.toFixed(3);
    seek(slot);
    // Warm only the adjacent clip; never decode six simultaneous videos.
    if (index + 1 < clips.length && local > 0.65 && !slots.some(s => s.index === index + 1)) {
      const next = slots.find(s => s !== slot) || createSlot(); loadSlot(next, index + 1);
    }
    const focus = Math.sin(local * Math.PI);
    stage.style.setProperty('--cinema-scale', (0.965 + focus * 0.035).toFixed(4));
    stage.style.setProperty('--cinema-rotate', ((0.5 - local) * 1.8).toFixed(3) + 'deg');
    stage.style.setProperty('--cinema-lift', (-focus * 12).toFixed(2) + 'px');
    journey.style.setProperty('--cinema-progress', p.toFixed(5));
    journey.style.setProperty('--cinema-hue', (205 + p * 65).toFixed(2));
    journey.dataset.motionProgress = p.toFixed(5);
  }
  function tick(time) {
    raf = 0;
    if (document.hidden || reduced.matches) return;
    const dt = Math.min(48, last ? time - last : 16.7); last = time;
    const target = journey ? clamp((scrollY - journeyTop + 56) / journeyRange) : 0;
    const mix = 1 - Math.exp(-dt / 90);
    progress += (target - progress) * mix;
    const targetPage = clamp(scrollY / Math.max(1, document.documentElement.scrollHeight - innerHeight));
    pageProgress += (targetPage - pageProgress) * mix;
    // Read layout together before setting compositor-only properties.
    const measurements = [...visible].map(node => [node, node.getBoundingClientRect()]);
    root.style.setProperty('--motion-page-progress', pageProgress.toFixed(5));
    for (const [node, rect] of measurements) {
      if (node.matches('.product-frame,.demo-card,.apple-platform-card')) {
        const depth = clamp((innerHeight / 2 - rect.top - rect.height / 2) / innerHeight, -0.5, 0.5);
        node.style.setProperty('--motion-parallax', (depth * 18).toFixed(2) + 'px');
      }
    }
    updateCinema(progress);
    if (Math.abs(target - progress) > 0.00015 || Math.abs(targetPage - pageProgress) > 0.00015) wake();
  }
  const mediaObserver = journey && new IntersectionObserver(entries => {
    journeyVisible = entries.some(e => e.isIntersecting);
    if (journeyVisible) { measure(); updateCinema(progress); }
    wake();
  }, {rootMargin: '400px 0px'});
  mediaObserver?.observe(journey);
  enroll(document);
  new MutationObserver(records => {
    for (const record of records) for (const node of record.addedNodes) {
      if (node.nodeType === 1 && !node.closest('[data-readmd-cinema]')) enroll(node);
    }
  }).observe(document.querySelector('main') || document.body, {childList: true, subtree: true});

  document.addEventListener('pointermove', event => {
    if (!finePointer.matches || reduced.matches) return;
    const card = event.target.closest?.('.demo-card,.platform-card,.apple-platform-card,main article');
    if (!card) return;
    const rect = card.getBoundingClientRect();
    card.classList.add('motion-interactive');
    card.style.setProperty('--motion-pointer-x', clamp((event.clientX - rect.left) / rect.width) * 100 + '%');
    card.style.setProperty('--motion-pointer-y', clamp((event.clientY - rect.top) / rect.height) * 100 + '%');
  }, {passive: true});
  addEventListener('scroll', wake, {passive: true});
  addEventListener('resize', measure, {passive: true});
  new ResizeObserver(measure).observe(document.querySelector('main') || document.body);
  document.addEventListener('visibilitychange', () => {
    root.classList.toggle('motion-paused', document.hidden);
    if (document.hidden) { cancelAnimationFrame(raf); raf = 0; last = 0; }
    else wake();
  });
  const preference = () => {
    root.classList.toggle('motion-ready', !reduced.matches);
    for (const caption of captions) {
      caption.inert = !reduced.matches && !caption.classList.contains('is-active');
      caption.setAttribute('aria-hidden', String(caption.inert));
    }
    if (reduced.matches) { cancelAnimationFrame(raf); raf = 0; }
    measure();
  };
  reduced.addEventListener('change', preference);
  preference();
})();
