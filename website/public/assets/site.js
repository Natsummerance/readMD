'use strict';

(() => {
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
  const clamp = (value, min, max) => Math.min(Math.max(value, min), max);

  const sharedFilm = {
    frames: Array.from({ length: 57 }, () => null),
    ready: false,
    promise: null,
  };

  const loadSharedFrame = (index) => new Promise((resolve) => {
    if (sharedFilm.frames[index]) return resolve(sharedFilm.frames[index]);
    const frame = new Image();
    frame.decoding = 'async';
    frame.onload = () => {
      sharedFilm.frames[index] = frame;
      resolve(frame);
    };
    frame.onerror = () => resolve(null);
    frame.src = `/media/journey-frames/frame-${String(index + 1).padStart(3, '0')}.webp`;
  });

  const ensureSharedFilmFrames = () => {
    if (!sharedFilm.promise) {
      sharedFilm.promise = Promise.all(Array.from({ length: sharedFilm.frames.length }, (_, index) => loadSharedFrame(index)))
        .then(() => {
          sharedFilm.ready = true;
          return sharedFilm.frames;
        });
    }
    return sharedFilm.promise;
  };

  const startParticles = (canvas) => {
    const context = canvas.getContext('2d', { alpha: true });
    if (!context) return;

    let particles = [];
    let animationId = 0;
    let visible = false;
    let disposed = false;

    const build = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
      canvas.width = Math.max(1, Math.round(rect.width * dpr));
      canvas.height = Math.max(1, Math.round(rect.height * dpr));
      context.setTransform(dpr, 0, 0, dpr, 0, 0);

      const count = clamp(Math.round((rect.width * rect.height) / 28000), 22, 68);
      particles = Array.from({ length: count }, () => ({
        x: Math.random() * rect.width,
        y: Math.random() * rect.height,
        vx: (Math.random() - 0.5) * 0.16,
        vy: (Math.random() - 0.5) * 0.16,
        radius: 0.7 + Math.random() * 1.1,
        alpha: 0.16 + Math.random() * 0.18,
      }));
    };

    const draw = () => {
      const rect = canvas.getBoundingClientRect();
      context.clearRect(0, 0, rect.width, rect.height);

      for (let i = 0; i < particles.length; i += 1) {
        const particle = particles[i];
        particle.x = (particle.x + particle.vx + rect.width) % rect.width;
        particle.y = (particle.y + particle.vy + rect.height) % rect.height;

        for (let j = i + 1; j < particles.length; j += 1) {
          const other = particles[j];
          const dx = particle.x - other.x;
          const dy = particle.y - other.y;
          const distance = Math.hypot(dx, dy);
          if (distance > 108) continue;

          context.strokeStyle = `rgba(10, 114, 232, ${0.07 * (1 - distance / 108)})`;
          context.lineWidth = 0.7;
          context.beginPath();
          context.moveTo(particle.x, particle.y);
          context.lineTo(other.x, other.y);
          context.stroke();
        }

        context.fillStyle = `rgba(10, 114, 232, ${particle.alpha})`;
        context.beginPath();
        context.arc(particle.x, particle.y, particle.radius, 0, Math.PI * 2);
        context.fill();
      }
    };

    const render = () => {
      if (disposed || !visible || document.hidden) {
        animationId = 0;
        return;
      }
      draw();
      animationId = window.requestAnimationFrame(render);
    };

    const play = () => {
      if (!animationId && visible && !document.hidden) {
        animationId = window.requestAnimationFrame(render);
      }
    };

    const resizeObserver = new ResizeObserver(() => {
      build();
      play();
    });
    resizeObserver.observe(canvas);

    const visibilityObserver = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      play();
    }, { threshold: 0 });
    visibilityObserver.observe(canvas);

    document.addEventListener('visibilitychange', play);
    reducedMotion.addEventListener('change', () => {
      disposed = true;
      if (animationId) window.cancelAnimationFrame(animationId);
      resizeObserver.disconnect();
      visibilityObserver.disconnect();
    }, { once: true });

    build();
  };

  const prepareReveals = () => {
    document.querySelectorAll('[data-reveal-group]').forEach((group) => {
      Array.from(group.children).forEach((child, index) => {
        child.classList.add('reveal');
        child.style.setProperty('--reveal-delay', `${Math.min(index * 65, 350)}ms`);
      });
    });

    const targets = document.querySelectorAll('.reveal');
    if (!targets.length) return;

    if (reducedMotion.matches) {
      targets.forEach((target) => target.classList.add('is-visible'));
      return;
    }

    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add('is-visible');
        observer.unobserve(entry.target);
      });
    }, { threshold: 0.16, rootMargin: '0px 0px -8% 0px' });

    targets.forEach((target) => observer.observe(target));
  };

  const startScrollEffects = () => {
    const journey = document.querySelector('[data-journey]');
    const progressBar = journey ? journey.querySelector('.journey-progress span') : null;
    const stage = journey ? journey.querySelector('.journey-stage') : null;
    const captions = journey ? Array.from(journey.querySelectorAll('.journey-caption')) : [];
    const filmCanvas = document.getElementById('journey-film');
    const frameCount = Number(filmCanvas?.dataset.frameCount || 0);
    const parallaxTargets = Array.from(document.querySelectorAll('.product-parallax'));
    const regions = Array.from(document.querySelectorAll('[data-motion-region]'));
    let activeRegions = new Set(regions.slice(0, 1));
    let effectsFrame = 0;
    let lastProgress = -1;
    let lastFrameIndex = -1;
    let filmFrames = sharedFilm.frames;

    const drawFilmFrame = (index) => {
      if (!filmCanvas || index === lastFrameIndex) return;
      const frame = filmFrames[index];
      if (!frame || !frame.complete || !frame.naturalWidth) return;
      filmCanvas.width = frame.naturalWidth;
      filmCanvas.height = frame.naturalHeight;
      filmCanvas.getContext('2d').drawImage(frame, 0, 0);
      lastFrameIndex = index;
      filmCanvas.dataset.frameIndex = String(index);
    };

    const syncFilm = (progress) => {
      if (!filmCanvas || !frameCount) return;
      drawFilmFrame(clamp(Math.round(progress * (frameCount - 1)), 0, frameCount - 1));
      filmCanvas.dataset.syncProgress = progress.toFixed(4);
    };

    const loadFilmFrames = () => {
    if (!filmCanvas || !frameCount) return;
    ensureSharedFilmFrames().then(() => {
      filmFrames = sharedFilm.frames;
        drawFilmFrame(Math.max(lastFrameIndex, 0));
        filmCanvas.dataset.framesReady = 'true';
      });
    };

    const ranges = captions.map((caption) => {
      const [start, end] = (caption.dataset.range || '0,1').split(',').map(Number);
      return { element: caption, start, end };
    });

    const updateJourney = () => {
      if (!journey) return;
      const rect = journey.getBoundingClientRect();
      const range = Math.max(rect.height - window.innerHeight, 1);
      const linear = clamp(-rect.top / range, 0, 1);
      const progress = linear <= 0.5
        ? 2 * linear * linear
        : 1 - ((-2 * linear + 2) ** 2) / 2;

      if (Math.abs(progress - lastProgress) >= 0.0012) {
        lastProgress = progress;
        if (progressBar) progressBar.style.transform = `scaleX(${progress})`;
        if (stage) {
          stage.style.transform = `perspective(1300px) rotateX(${((0.5 - progress) * 3).toFixed(3)}deg) scale(${(0.975 + progress * 0.025).toFixed(4)})`;
        }
        ranges.forEach(({ element, start, end }) => {
          element.classList.toggle('is-active', progress >= start && progress <= end);
        });
      }

      syncFilm(progress);
      journey.dataset.motionProgress = progress.toFixed(4);
      return true;
    };

    const updateParallax = () => {
      const viewportCenter = window.innerHeight / 2;
      parallaxTargets.forEach((target) => {
        const rect = target.getBoundingClientRect();
        if (rect.bottom < -120 || rect.top > window.innerHeight + 120) return;
        const offset = (rect.top + rect.height / 2 - viewportCenter) / window.innerHeight;
        target.style.transform = `translateY(${(-offset * 20).toFixed(2)}px)`;
      });
    };

    const renderEffects = () => {
      if (!activeRegions.size || document.hidden) {
        effectsFrame = 0;
        return;
      }
      updateJourney();
      updateParallax();
      effectsFrame = window.requestAnimationFrame(renderEffects);
    };

    const playEffects = () => {
      if (!effectsFrame && activeRegions.size && !document.hidden && !reducedMotion.matches) {
        effectsFrame = window.requestAnimationFrame(renderEffects);
      }
    };

    if (reducedMotion.matches) {
      ranges.forEach(({ element }) => element.classList.remove('is-active'));
      if (ranges[0]) ranges[0].element.classList.add('is-active');
      return;
    }

    const regionObserver = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) activeRegions.add(entry.target);
        else activeRegions.delete(entry.target);
      });
      playEffects();
    }, { rootMargin: '20% 0px' });

    regions.forEach((region) => regionObserver.observe(region));
    document.addEventListener('visibilitychange', playEffects);
    loadFilmFrames();
    updateJourney();
    playEffects();
  };

  const startCapabilityCinema = () => {
    const section = document.querySelector('[data-capability-cinema]');
    const canvas = document.querySelector('.capability-canvas');
    const track = document.querySelector('[data-capability-track]');
    const progressBar = document.querySelector('.capability-progress span');
    const panels = Array.from(document.querySelectorAll('.capability-panel'));
    if (!section || !canvas || !track) return;

    const context = canvas.getContext('2d', { alpha: false });
    const frameCount = sharedFilm.frames.length;
    let canvasWidth = 1280;
    let canvasHeight = 720;
    let effectsFrame = 0;
    let visible = false;
    let lastProgress = -1;
    let lastPosition = -1;
    let velocity = 0;
    let lastTime = performance.now();
    let particles = [];

    const buildCanvas = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
      canvasWidth = Math.max(960, Math.round(rect.width * dpr));
      canvasHeight = Math.max(540, Math.round(rect.height * dpr));
      canvas.width = canvasWidth;
      canvas.height = canvasHeight;
      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      particles = Array.from({ length: 42 }, () => ({
        x: Math.random() * rect.width,
        y: Math.random() * rect.height,
        radius: 0.6 + Math.random() * 1.5,
        speed: 0.15 + Math.random() * 0.55,
        alpha: 0.08 + Math.random() * 0.14,
      }));
    };

    const drawCover = (frame, scale = 1, offsetX = 0, offsetY = 0, alpha = 1) => {
      if (!frame || !frame.naturalWidth) return;
      const sourceRatio = frame.naturalWidth / frame.naturalHeight;
      const targetRatio = canvasWidth / canvasHeight;
      let width = canvasWidth * scale;
      let height = canvasHeight * scale;
      if (sourceRatio > targetRatio) width = height * sourceRatio;
      else height = width / sourceRatio;
      const left = (canvasWidth - width) / 2 + offsetX;
      const top = (canvasHeight - height) / 2 + offsetY;
      context.globalAlpha = alpha;
      context.drawImage(frame, left, top, width, height);
      context.globalAlpha = 1;
    };

    const drawBlendedFrame = (position, alpha, progress) => {
      const clamped = clamp(position, 0, frameCount - 1);
      const firstIndex = Math.floor(clamped);
      const secondIndex = Math.min(firstIndex + 1, frameCount - 1);
      const amount = clamped - firstIndex;
      const first = sharedFilm.frames[firstIndex];
      const second = sharedFilm.frames[secondIndex];
      drawCover(first, 1.16 + progress * 0.12, 0, 0, alpha);
      if (second && amount > 0.02) {
        context.globalCompositeOperation = 'lighter';
        drawCover(second, 1.18 + progress * 0.12, Math.sin(progress * Math.PI * 4) * 12, Math.cos(progress * Math.PI * 3) * 8, alpha * 0.28 * amount);
        context.globalCompositeOperation = 'source-over';
      }
    };

    const drawSlices = (position, progress) => {
      const bands = 8;
      const bandHeight = canvasHeight / bands;
      for (let index = 0; index < bands; index += 1) {
        if ((index + Math.round(progress * 12)) % 3 !== 0) continue;
        const neighborPosition = clamp(position + (index % 2 === 0 ? -0.035 : 0.04), 0, frameCount - 1);
        const frame = sharedFilm.frames[Math.floor(neighborPosition)];
        if (!frame) continue;
        const sourceY = (index / bands) * frame.naturalHeight;
        const sourceHeight = frame.naturalHeight / bands;
        const offset = Math.sin(index * 1.7 + progress * 11) * (10 + velocity * 90);
        context.globalAlpha = 0.22;
        context.drawImage(
          frame,
          0,
          sourceY,
          frame.naturalWidth,
          sourceHeight,
          offset,
          index * bandHeight,
          canvasWidth,
          bandHeight,
        );
        context.globalAlpha = 1;
      }
    };

    const drawParticles = () => {
      for (const particle of particles) {
        particle.y -= particle.speed;
        if (particle.y < -4) particle.y = canvas.clientHeight + 4;
        context.fillStyle = `rgba(157, 202, 255, ${particle.alpha})`;
        context.beginPath();
        context.arc(particle.x, particle.y, particle.radius, 0, Math.PI * 2);
        context.fill();
      }
    };

    const updateCapabilityCinema = (timestamp) => {
      const rect = section.getBoundingClientRect();
      const range = Math.max(rect.height - window.innerHeight, 1);
      const linear = clamp(-rect.top / range, 0, 1);
      const progress = linear <= 0.45
        ? linear / 0.45 * 0.24
        : 0.24 + ((linear - 0.45) / 0.55) * 0.76;

      const now = timestamp || performance.now();
      const elapsed = Math.max(now - lastTime, 16);
      lastTime = now;
      if (lastPosition >= 0) {
        const instantVelocity = (progress - lastPosition) / elapsed * 1000;
        velocity += (instantVelocity - velocity) * 0.12;
      }
      velocity = clamp(velocity, -0.35, 0.35);
      lastPosition = progress;
      canvas.dataset.syncProgress = progress.toFixed(4);

      const position = progress * (frameCount - 1);
      context.clearRect(0, 0, canvasWidth, canvasHeight);
      context.fillStyle = '#050609';
      context.fillRect(0, 0, canvasWidth, canvasHeight);
      if (!sharedFilm.ready) return true;

      drawBlendedFrame(position, 0.82, progress);
      drawSlices(position, progress);
      context.globalCompositeOperation = 'lighter';
      drawCover(sharedFilm.frames[Math.floor(clamp(position + velocity * 7, 0, frameCount - 1))], 1.2, velocity * -70, velocity * -35, Math.min(Math.abs(velocity) * 1.3, 0.2));
      context.globalCompositeOperation = 'source-over';
      drawParticles();

      const railDistance = Math.max(track.scrollWidth - window.innerWidth + 48, 0);
      const railEase = progress <= 0.35 ? 0 : (progress - 0.35) / 0.65;
      track.style.transform = `translate3d(${-railDistance * (railEase ** 2 * (3 - 2 * railEase))}px,0,0)`;
      if (progressBar) progressBar.style.transform = `scaleX(${progress})`;

      const viewportCenter = window.innerWidth / 2;
      panels.forEach((panel) => {
        const panelRect = panel.getBoundingClientRect();
        const distance = Math.abs(panelRect.left + panelRect.width / 2 - viewportCenter) / window.innerWidth;
        const proximity = clamp(1 - distance * 1.7, 0, 1);
        panel.style.opacity = (0.34 + proximity * 0.66).toFixed(3);
        panel.style.transform = `translateY(${(1 - proximity) * 26}px) scale(${0.94 + proximity * 0.06})`;
      });
      return true;
    };

    const renderCinema = (timestamp) => {
      if (!visible || document.hidden) {
        effectsFrame = 0;
        return;
      }
      updateCapabilityCinema(timestamp || performance.now());
      effectsFrame = window.requestAnimationFrame(renderCinema);
    };

    const playCinema = () => {
      if (!effectsFrame && visible && !document.hidden && !reducedMotion.matches) {
        effectsFrame = window.requestAnimationFrame(renderCinema);
      }
    };

    buildCanvas();
    const resizeObserver = new ResizeObserver(() => {
      buildCanvas();
      updateCapabilityCinema(performance.now());
    });
    resizeObserver.observe(canvas);

    const visibilityObserver = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      playCinema();
    }, { rootMargin: '12% 0px' });
    visibilityObserver.observe(section);
    document.addEventListener('visibilitychange', playCinema);

    ensureSharedFilmFrames().then(() => {
      buildCanvas();
      updateCapabilityCinema(performance.now());
    });

    if (reducedMotion.matches) {
      if (progressBar) progressBar.style.display = 'none';
      track.style.transform = 'none';
      panels.forEach((panel) => {
        panel.style.opacity = '1';
        panel.style.transform = 'none';
      });
    }
  };

  const initialize = () => {
    const navShell = document.querySelector('.nav-shell');
    const hero = document.querySelector('.hero');
    if (navShell && hero) {
      const navObserver = new IntersectionObserver(([entry]) => {
        navShell.classList.toggle('is-scrolled', !entry.isIntersecting);
      }, { threshold: 0.72 });
      navObserver.observe(hero);
    }

    document.querySelectorAll('.particle-field').forEach(startParticles);
    prepareReveals();
    startScrollEffects();
    startCapabilityCinema();
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initialize, { once: true });
  } else {
    initialize();
  }
})();
