'use strict';

(() => {
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
  const clamp = (value, min, max) => Math.min(Math.max(value, min), max);

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
    let filmFrames = [];
    let lastFrameIndex = -1;

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
      const requests = Array.from({ length: frameCount }, (_, index) => new Promise((resolve) => {
        const frame = new Image();
        frame.decoding = 'async';
        frame.onload = () => resolve(frame);
        frame.onerror = () => resolve(frame);
        frame.src = `/media/journey-frames/frame-${String(index + 1).padStart(3, '0')}.webp`;
      }));
      Promise.all(requests).then((loaded) => {
        filmFrames = loaded;
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
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initialize, { once: true });
  } else {
    initialize();
  }
})();
