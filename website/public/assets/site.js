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

  const cinemaFilm = {
    frames: Array.from({ length: 124 }, () => null),
    ready: false,
    promise: null,
  };

  const loadCinemaFrame = (index) => new Promise((resolve) => {
    if (cinemaFilm.frames[index]) return resolve(cinemaFilm.frames[index]);
    const frame = new Image();
    frame.decoding = 'async';
    frame.onload = () => {
      cinemaFilm.frames[index] = frame;
      resolve(frame);
    };
    frame.onerror = () => resolve(null);
    frame.src = `/media/cinema-frames/frame-${String(index + 1).padStart(3, '0')}.webp`;
  });

  const ensureCinemaFilmFrames = () => {
    if (!cinemaFilm.promise) {
      cinemaFilm.promise = Promise.all(Array.from({ length: cinemaFilm.frames.length }, (_, index) => loadCinemaFrame(index)))
        .then(() => {
          cinemaFilm.ready = true;
          return cinemaFilm.frames;
        });
    }
    return cinemaFilm.promise;
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

      const count = clamp(Math.round((rect.width * rect.height) / 24000), 24, 72);
      particles = Array.from({ length: count }, () => ({
        x: Math.random() * rect.width,
        y: Math.random() * rect.height,
        vx: (Math.random() - 0.5) * 0.18,
        vy: (Math.random() - 0.5) * 0.18,
        radius: 0.8 + Math.random() * 1.2,
        alpha: 0.18 + Math.random() * 0.22,
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
          if (distance > 115) continue;

          context.strokeStyle = `rgba(0, 113, 227, ${0.08 * (1 - distance / 115)})`;
          context.lineWidth = 0.75;
          context.beginPath();
          context.moveTo(particle.x, particle.y);
          context.lineTo(other.x, other.y);
          context.stroke();
        }

        context.fillStyle = `rgba(0, 113, 227, ${particle.alpha})`;
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
        child.style.setProperty('--reveal-delay', `${Math.min(index * 60, 320)}ms`);
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
    }, { threshold: 0.15, rootMargin: '0px 0px -6% 0px' });

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
    let targetProgress = 0;
    let currentProgress = 0;
    let lastRenderedProgress = -1;
    let lastFrameIndex = -1;
    let filmFrames = sharedFilm.frames;

    const drawFilmFrame = (index, blendIndex = -1, blendWeight = 0) => {
      if (!filmCanvas) return;
      const frame = filmFrames[index];
      if (!frame || !frame.complete || !frame.naturalWidth) return;

      const rect = filmCanvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const targetW = Math.max(1, Math.round(rect.width * dpr));
      const targetH = Math.max(1, Math.round(rect.height * dpr));
      if (filmCanvas.width !== targetW || filmCanvas.height !== targetH) {
        filmCanvas.width = targetW;
        filmCanvas.height = targetH;
      }

      const ctx = filmCanvas.getContext('2d');
      const imgW = frame.naturalWidth;
      const imgH = frame.naturalHeight;
      const imgRatio = imgW / imgH;
      const canvasRatio = targetW / targetH;

      let renderW = targetW;
      let renderH = targetH;
      if (canvasRatio > imgRatio) {
        renderW = targetW;
        renderH = targetW / imgRatio;
      } else {
        renderH = targetH;
        renderW = targetH * imgRatio;
      }
      const x = (targetW - renderW) / 2;
      const y = (targetH - renderH) / 2;

      ctx.clearRect(0, 0, targetW, targetH);
      ctx.drawImage(frame, 0, 0, imgW, imgH, x, y, renderW, renderH);

      if (blendIndex >= 0 && blendWeight > 0.01) {
        const blendFrame = filmFrames[blendIndex];
        if (blendFrame && blendFrame.complete && blendFrame.naturalWidth) {
          ctx.globalAlpha = blendWeight;
          ctx.drawImage(blendFrame, 0, 0, imgW, imgH, x, y, renderW, renderH);
          ctx.globalAlpha = 1;
        }
      }

      lastFrameIndex = index;
      filmCanvas.dataset.frameIndex = String(index);
    };

    const syncFilm = (progress) => {
      if (!filmCanvas || !frameCount) return;
      const exactPos = progress * (frameCount - 1);
      const baseIndex = clamp(Math.floor(exactPos), 0, frameCount - 1);
      const nextIndex = clamp(baseIndex + 1, 0, frameCount - 1);
      const weight = exactPos - baseIndex;
      drawFilmFrame(baseIndex, nextIndex, weight);
      filmCanvas.dataset.syncProgress = progress.toFixed(4);
    };

    const loadFilmFrames = () => {
      if (!filmCanvas || !frameCount) return;
      ensureSharedFilmFrames().then(() => {
        filmFrames = sharedFilm.frames;
        syncFilm(currentProgress);
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
      targetProgress = linear <= 0.5
        ? 2 * linear * linear
        : 1 - ((-2 * linear + 2) ** 2) / 2;

      // Spring lerp smoothing for physical fluid motion
      currentProgress += (targetProgress - currentProgress) * 0.16;
      if (Math.abs(targetProgress - currentProgress) < 0.0005) {
        currentProgress = targetProgress;
      }

      if (Math.abs(currentProgress - lastRenderedProgress) >= 0.0008) {
        lastRenderedProgress = currentProgress;
        if (progressBar) progressBar.style.transform = `scaleX(${currentProgress})`;
        if (stage) {
          stage.style.transform = `scale(${(0.985 + currentProgress * 0.015).toFixed(4)})`;
        }
        ranges.forEach(({ element, start, end }) => {
          element.classList.toggle('is-active', currentProgress >= start && currentProgress <= end);
        });
      }

      syncFilm(currentProgress);
      journey.dataset.motionProgress = currentProgress.toFixed(4);
      return true;
    };

    const updateParallax = () => {
      const viewportCenter = window.innerHeight / 2;
      parallaxTargets.forEach((target) => {
        const rect = target.getBoundingClientRect();
        if (rect.bottom < -120 || rect.top > window.innerHeight + 120) return;
        const offset = (rect.top + rect.height / 2 - viewportCenter) / window.innerHeight;
        target.style.transform = `translateY(${(-offset * 18).toFixed(2)}px)`;
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
    const frameCount = cinemaFilm.frames.length;
    let canvasWidth = 1440;
    let canvasHeight = 900;
    let effectsFrame = 0;
    let visible = false;
    let targetProgress = 0;
    let currentProgress = 0;
    let lastPosition = -1;
    let velocity = 0;
    let lastTime = performance.now();
    let particles = [];
    let cinemaLoadStarted = false;

    const buildCanvas = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvasWidth = Math.max(1, Math.round(rect.width * dpr));
      canvasHeight = Math.max(1, Math.round(rect.height * dpr));
      canvas.width = canvasWidth;
      canvas.height = canvasHeight;

      particles = Array.from({ length: 48 }, () => ({
        x: Math.random() * canvasWidth,
        y: Math.random() * canvasHeight,
        radius: (0.7 + Math.random() * 1.5) * dpr,
        speed: (0.15 + Math.random() * 0.35) * dpr,
        alpha: 0.12 + Math.random() * 0.18,
      }));
    };

    const drawCover = (frame, scale = 1, offsetX = 0, offsetY = 0, alpha = 1) => {
      if (!frame || !frame.naturalWidth) return;
      const imgW = frame.naturalWidth;
      const imgH = frame.naturalHeight;
      const imgRatio = imgW / imgH; // 16 / 9 = 1.777778
      const screenRatio = canvasWidth / canvasHeight;

      let renderW, renderH;
      if (screenRatio > imgRatio) {
        // Screen is wider than 16:9 -> match width, crop height
        renderW = canvasWidth * scale;
        renderH = (canvasWidth / imgRatio) * scale;
      } else {
        // Screen is taller than 16:9 -> match height, crop width
        renderH = canvasHeight * scale;
        renderW = (canvasHeight * imgRatio) * scale;
      }

      const left = (canvasWidth - renderW) / 2 + offsetX;
      const top = (canvasHeight - renderH) / 2 + offsetY;

      context.globalAlpha = alpha;
      context.drawImage(frame, 0, 0, imgW, imgH, left, top, renderW, renderH);
      context.globalAlpha = 1;
    };

    // Smooth blended cinematic frames
    const drawBlendedFrame = (position, alpha, progress) => {
      const clamped = clamp(position, 0, frameCount - 1);
      const firstIndex = Math.floor(clamped);
      const secondIndex = Math.min(firstIndex + 1, frameCount - 1);
      const amount = clamped - firstIndex;
      const first = cinemaFilm.frames[firstIndex];
      const second = cinemaFilm.frames[secondIndex];

      drawCover(first, 1.0, 0, 0, alpha);
      if (second && amount > 0.01) {
        drawCover(second, 1.0, 0, 0, alpha * amount);
      }
    };

    // Clean subtle depth bloom (preserves contract without distortion)
    const drawSlices = (position, progress) => {
      if (Math.abs(velocity) < 0.02) return;
      const frame = cinemaFilm.frames[Math.floor(clamp(position, 0, frameCount - 1))];
      if (!frame) return;
      context.save();
      context.globalAlpha = Math.min(Math.abs(velocity) * 0.12, 0.05);
      drawCover(frame, 1.02, velocity * -10, 0, 1);
      context.restore();
    };

    // Subtle Apple-grade particle starfield
    const drawParticles = () => {
      for (const particle of particles) {
        particle.y -= particle.speed;
        if (particle.y < -4) particle.y = canvasHeight + 4;
        context.fillStyle = `rgba(144, 202, 249, ${particle.alpha})`;
        context.beginPath();
        context.arc(particle.x, particle.y, particle.radius, 0, Math.PI * 2);
        context.fill();
      }
    };

    const updateCapabilityCinema = (timestamp) => {
      const rect = section.getBoundingClientRect();
      const range = Math.max(rect.height - window.innerHeight, 1);
      const linear = clamp(-rect.top / range, 0, 1);
      targetProgress = linear;

      // Smooth lerp physics
      currentProgress += (targetProgress - currentProgress) * 0.18;
      if (Math.abs(targetProgress - currentProgress) < 0.0002) {
        currentProgress = targetProgress;
      }

      const now = timestamp || performance.now();
      const elapsed = Math.max(now - lastTime, 16);
      lastTime = now;
      if (lastPosition >= 0) {
        const instantVelocity = (currentProgress - lastPosition) / elapsed * 1000;
        velocity += (instantVelocity - velocity) * 0.12;
      }
      velocity = clamp(velocity, -0.3, 0.3);
      lastPosition = currentProgress;
      canvas.dataset.syncProgress = currentProgress.toFixed(4);

      // Uniformly distributed horizontal rail translation
      const railDistance = Math.max(track.scrollWidth - window.innerWidth + 64, 0);
      const railTranslate = -railDistance * currentProgress;
      track.style.transform = `translate3d(${railTranslate.toFixed(2)}px,0,0)`;
      if (progressBar) progressBar.style.transform = `scaleX(${currentProgress})`;

      const viewportCenter = window.innerWidth / 2;
      panels.forEach((panel) => {
        const panelRect = panel.getBoundingClientRect();
        const distance = Math.abs(panelRect.left + panelRect.width / 2 - viewportCenter) / window.innerWidth;
        const proximity = clamp(1 - distance * 1.5, 0, 1);
        panel.style.opacity = (0.35 + proximity * 0.65).toFixed(3);
        panel.style.transform = `translateY(${((1 - proximity) * 18).toFixed(1)}px) scale(${(0.96 + proximity * 0.04).toFixed(3)})`;
      });

      // Canvas rendering synchronized 1:1 across all 124 frames
      const position = currentProgress * (frameCount - 1);
      context.clearRect(0, 0, canvasWidth, canvasHeight);
      context.fillStyle = '#08090c';
      context.fillRect(0, 0, canvasWidth, canvasHeight);
      drawBlendedFrame(position, 0.92, currentProgress);
      drawSlices(position, currentProgress);
      drawParticles();
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

    const loadCinemaWhenVisible = () => {
      if (reducedMotion.matches || cinemaLoadStarted) return cinemaFilm.promise || Promise.resolve(cinemaFilm.frames);
      cinemaLoadStarted = true;
      return ensureCinemaFilmFrames().then(() => {
        buildCanvas();
        updateCapabilityCinema(performance.now());
        playCinema();
        return cinemaFilm.frames;
      });
    };

    buildCanvas();
    const resizeObserver = new ResizeObserver(() => {
      buildCanvas();
      updateCapabilityCinema(performance.now());
    });
    resizeObserver.observe(canvas);

    const visibilityObserver = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      if (visible) {
        loadCinemaWhenVisible().then(() => {
          if (visible && !reducedMotion.matches) {
            updateCapabilityCinema(performance.now());
            playCinema();
          }
        });
      }
    }, { rootMargin: '25% 0px' });
    visibilityObserver.observe(section);
    document.addEventListener('visibilitychange', playCinema);

    if (reducedMotion.matches) {
      if (progressBar) progressBar.style.display = 'none';
      track.style.transform = 'none';
      panels.forEach((panel) => {
        panel.style.opacity = '1';
        panel.style.transform = 'none';
      });
    }
  };

  /* Dual-Channel High-Speed Download Hub & Mirror Engine */
  const setupDownloadHub = () => {
    const mirrorEngines = {
      direct: (url) => url,
      ghfast: (url) => `https://ghfast.top/${url}`,
      ghproxy: (url) => `https://ghproxy.net/${url}`,
      gitmirror: (url) => `https://hub.gitmirror.com/${url}`,
      fastgit: (url) => `https://gh-proxy.com/${url}`,
    };

    let activeMirror = localStorage.getItem('readmd_download_mirror') || 'direct';

    const updateDownloadLinks = (mirrorKey, animate = true) => {
      activeMirror = mirrorKey;
      localStorage.setItem('readmd_download_mirror', mirrorKey);

      const targetElements = document.querySelectorAll('.platform-card a, .platform-card button');

      const applyLinks = () => {
        document.querySelectorAll('[data-mirror-url]').forEach((link) => {
          const original = link.dataset.mirrorUrl;
          const transform = mirrorEngines[mirrorKey] || mirrorEngines.direct;
          link.href = transform(original);
        });

        document.querySelectorAll('.mirror-tab').forEach((tab) => {
          tab.classList.toggle('is-active', tab.dataset.mirror === mirrorKey);
        });
      };

      if (animate && targetElements.length > 0) {
        targetElements.forEach((el) => {
          el.style.transition = 'opacity 0.18s cubic-bezier(0.25, 0.1, 0.25, 1), transform 0.18s cubic-bezier(0.25, 0.1, 0.25, 1)';
          el.style.opacity = '0.35';
          el.style.transform = 'scale(0.985)';
        });
        setTimeout(() => {
          applyLinks();
          targetElements.forEach((el) => {
            el.style.opacity = '1';
            el.style.transform = 'scale(1)';
          });
        }, 120);
      } else {
        applyLinks();
      }
    };

    document.querySelectorAll('.mirror-tab').forEach((tab) => {
      tab.addEventListener('click', () => {
        updateDownloadLinks(tab.dataset.mirror, true);
      });
    });

    // Detect user platform
    const ua = navigator.userAgent.toLowerCase();
    let detectedOS = 'windows';
    if (ua.includes('mac') || ua.includes('darwin')) detectedOS = 'macos';
    else if (ua.includes('linux') || ua.includes('x11') || ua.includes('kylin') || ua.includes('uos')) detectedOS = 'linux';

    document.querySelectorAll(`[data-platform="${detectedOS}"]`).forEach((card) => {
      card.classList.add('is-recommended');
    });

    // Initialize links with data-mirror-url if present
    document.querySelectorAll('a[href^="https://github.com/Natsummerance/readMD/releases/download/"]').forEach((link) => {
      if (!link.dataset.mirrorUrl) link.dataset.mirrorUrl = link.href;
    });

    updateDownloadLinks(activeMirror);

    // Dual-Layer Dynamic Version & Release Synchronizer
    const GITHUB_REPO = 'Natsummerance/readMD';
    const CACHE_KEY = 'readmd_release_cache_v2';
    const CACHE_TTL = 10 * 60 * 1000; // 10 minutes

    const applyVersionData = (versionTag, pureVersion) => {
      if (!versionTag) return;
      const cleanTag = versionTag.startsWith('v') ? versionTag : `v${versionTag}`;
      const cleanPure = pureVersion || cleanTag.replace(/^v/, '');

      document.querySelectorAll('.latest-version-badge, [data-version-slot]').forEach((el) => {
        el.textContent = cleanTag;
      });
      document.querySelectorAll('[data-pure-version]').forEach((el) => {
        el.textContent = cleanPure;
      });

      // Update SHA256SUMS.txt links
      document.querySelectorAll('a[href*="SHA256SUMS.txt"]').forEach((a) => {
        a.href = `https://github.com/${GITHUB_REPO}/releases/download/${cleanTag}/SHA256SUMS.txt`;
      });

      // Update all download URLs and mirror slots
      document.querySelectorAll('a[data-mirror-url]').forEach((link) => {
        let orig = link.dataset.mirrorUrl;
        if (orig.includes('/releases/download/')) {
          const oldTagMatch = orig.match(/\/releases\/download\/([^/]+)\//);
          if (oldTagMatch && oldTagMatch[1] && oldTagMatch[1] !== cleanTag) {
            const oldTag = oldTagMatch[1];
            const oldPure = oldTag.replace(/^v/, '');
            let updated = orig.replace(new RegExp(`/releases/download/${oldTag}/`, 'g'), `/releases/download/${cleanTag}/`);
            updated = updated.replace(new RegExp(`-${oldPure}\\.`, 'g'), `-${cleanPure}.`);
            link.dataset.mirrorUrl = updated;
          }
        }
      });

      updateDownloadLinks(activeMirror);
    };

    // 1. Try Cached GitHub Release
    let isApplied = false;
    try {
      const cached = sessionStorage.getItem(CACHE_KEY);
      if (cached) {
        const parsed = JSON.parse(cached);
        if (Date.now() - parsed.timestamp < CACHE_TTL && parsed.tag_name) {
          applyVersionData(parsed.tag_name, parsed.pure_version);
          isApplied = true;
        }
      }
    } catch (e) {}

    // 2. Fetch Latest from GitHub API (or fallback to /version.json)
    if (!isApplied) {
      fetch(`https://api.github.com/repos/${GITHUB_REPO}/releases/latest`, {
        headers: { 'Accept': 'application/vnd.github.v3+json' }
      })
        .then((res) => (res.ok ? res.json() : Promise.reject(new Error('Rate limited / network'))))
        .then((data) => {
          if (data && data.tag_name) {
            const tag = data.tag_name;
            const pure = tag.replace(/^v/, '');
            try {
              sessionStorage.setItem(CACHE_KEY, JSON.stringify({
                timestamp: Date.now(),
                tag_name: tag,
                pure_version: pure
              }));
            } catch (e) {}
            applyVersionData(tag, pure);
          }
        })
        .catch(() => {
          // Fallback to local build-time version.json
          fetch('/version.json')
            .then((res) => (res.ok ? res.json() : null))
            .then((vData) => {
              if (vData && vData.releaseTag) {
                applyVersionData(vData.releaseTag, vData.version);
              }
            })
            .catch(() => {});
        });
    }
  };

  /* Interactive MCP Configuration & 1-Click Multi-Harness Generator */
  const setupMcpGuide = () => {
    const mcpConfigs = {
      // --- IDEs & Desktop ---
      claude: {
        name: 'Claude Desktop',
        format: 'JSON',
        path: 'macOS: ~/Library/Application Support/Claude/claude_desktop_config.json | Windows: %APPDATA%\\Claude\\claude_desktop_config.json',
        rawPath: '~/Library/Application Support/Claude/claude_desktop_config.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      cursor: {
        name: 'Cursor IDE',
        format: 'JSON',
        path: '项目级: .cursor/mcp.json | 全局: Cursor Settings > Features > MCP',
        rawPath: '.cursor/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      trae: {
        name: 'Trae (字节跳动)',
        format: 'JSON',
        path: '~/.trae/mcp.json 或 Trae Settings > MCP',
        rawPath: '~/.trae/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      vscode: {
        name: 'VS Code (Cline/Roo)',
        format: 'JSON',
        path: 'VS Code Cline/Roo MCP Settings 或 .vscode/settings.json',
        rawPath: '.vscode/settings.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      zcode: {
        name: 'ZCode IDE',
        format: 'JSON',
        path: '~/.zcode/mcp.json 或 ZCode Settings > MCP Servers',
        rawPath: '~/.zcode/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      qoder: {
        name: 'Qoder (阿里/通用)',
        format: 'JSON',
        path: '~/.qoder/settings.json 或 qoder --mcp-config',
        rawPath: '~/.qoder/settings.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"],\n      "transport": "stdio"\n    }\n  }\n}`,
      },

      // --- Agent Harnesses & CLI ---
      codex: {
        name: 'Codex / ChatGPT',
        format: 'TOML',
        path: '~/.codex/config.toml (OpenAI Codex / ChatGPT Desktop)',
        rawPath: '~/.codex/config.toml',
        code: `[mcp_servers.readmd]\ncommand = "python"\nargs = ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]`,
      },
      antigravity: {
        name: 'Antigravity / Gemini',
        format: 'JSON',
        path: '~/.gemini/antigravity/mcp/readmd.json',
        rawPath: '~/.gemini/antigravity/mcp/readmd.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      opencode: {
        name: 'OpenCode CLI',
        format: 'JSONC',
        path: '~/.config/opencode/opencode.json 或项目根目录 opencode.json',
        rawPath: '~/.config/opencode/opencode.json',
        code: `{\n  "mcp": {\n    "readmd": {\n      "type": "local",\n      "enabled": true,\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      hermes: {
        name: 'Nous Hermes',
        format: 'YAML',
        path: '~/.hermes/config.yaml (Nous Hermes Agent Harness)',
        rawPath: '~/.hermes/config.yaml',
        code: `mcp_servers:\n  readmd:\n    command: "python"\n    args:\n      - "/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"`,
      },
      deepseek: {
        name: 'DeepSeek Harness',
        format: 'YAML',
        path: '~/.dsh/settings.yaml (DeepSeek Coding Agent Harness)',
        rawPath: '~/.dsh/settings.yaml',
        code: `mcpServers:\n  readmd:\n    command: "python"\n    args:\n      - "/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"`,
      },
      openclaw: {
        name: 'OpenClaw',
        format: 'JSON5',
        path: '~/.openclaw/openclaw.json (OpenClaw Agent Gateway)',
        rawPath: '~/.openclaw/openclaw.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      workbuddy: {
        name: 'WorkBuddy (腾讯)',
        format: 'JSON',
        path: '~/.workbuddy/mcp.json (腾讯云代码助手 / WorkBuddy)',
        rawPath: '~/.workbuddy/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      doubao: {
        name: '豆包 / Coze',
        format: 'JSON',
        path: '~/.coze/mcp.json 或 扣子/豆包 Studio 插件连接器',
        rawPath: '~/.coze/mcp.json',
        code: `{\n  "mcpServers": {\n    "readmd": {\n      "command": "python",\n      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]\n    }\n  }\n}`,
      },
      cli: {
        name: 'Terminal (uv / python)',
        format: 'BASH',
        path: '终端命令行直接启动 stdio 服务',
        rawPath: 'uv run /path/to/readmd/packages/mcp-server/readmd_mcp_server.py',
        code: `# 使用 uv 零配置快速运行\nuv run /path/to/readmd/packages/mcp-server/readmd_mcp_server.py\n\n# 或使用标准 Python 运行\npython /path/to/readmd/packages/mcp-server/readmd_mcp_server.py`,
      },
    };

    let currentTarget = 'claude';

    const updateMcpView = (target) => {
      currentTarget = target;
      const data = mcpConfigs[target] || mcpConfigs.claude;
      document.querySelectorAll('.mcp-tab').forEach((tab) => {
        tab.classList.toggle('is-active', tab.dataset.mcpTarget === target);
      });
      document.querySelectorAll('.mcp-config-path').forEach((el) => {
        el.textContent = data.path;
      });
      document.querySelectorAll('.mcp-format-badge').forEach((el) => {
        el.textContent = data.format;
      });
      document.querySelectorAll('.macos-agent-name').forEach((el) => {
        el.textContent = data.name;
      });
      document.querySelectorAll('.mcp-code-block code').forEach((el) => {
        el.style.opacity = '0.3';
        el.textContent = data.code;
        setTimeout(() => {
          el.style.opacity = '1';
        }, 60);
      });
    };

    document.querySelectorAll('.mcp-tab').forEach((tab) => {
      tab.addEventListener('click', () => {
        updateMcpView(tab.dataset.mcpTarget);
      });
    });

    // Apple Accordion Interactions
    document.querySelectorAll('.apple-accordion-item').forEach((item) => {
      const header = item.querySelector('.apple-accordion-header');
      if (!header) return;
      header.addEventListener('click', () => {
        const isOpen = item.classList.contains('is-open');
        const parentList = item.closest('.apple-accordion-list');
        if (parentList) {
          parentList.querySelectorAll('.apple-accordion-item').forEach((sibling) => {
            sibling.classList.remove('is-open');
          });
        }
        if (!isOpen) {
          item.classList.add('is-open');
        }
      });
    });

    const copyTextWithFallback = async (text, btn, successLabel) => {
      const textEl = btn.querySelector('.copy-text') || btn;
      const origText = textEl.textContent;
      try {
        await navigator.clipboard.writeText(text);
        textEl.textContent = successLabel || '✓ Copied!';
        btn.style.borderColor = 'var(--color-action)';
        setTimeout(() => {
          textEl.textContent = origText;
          btn.style.borderColor = '';
        }, 2000);
      } catch (e) {
        const textarea = document.createElement('textarea');
        textarea.value = text;
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
        textEl.textContent = successLabel || '✓ Copied!';
        setTimeout(() => {
          textEl.textContent = origText;
          btn.style.borderColor = '';
        }, 2000);
      }
    };

    document.querySelectorAll('.btn-copy-mcp').forEach((btn) => {
      btn.addEventListener('click', () => {
        const data = mcpConfigs[currentTarget] || mcpConfigs.claude;
        copyTextWithFallback(data.code, btn, '✓ Copied!');
      });
    });

    document.querySelectorAll('.btn-copy-path').forEach((btn) => {
      btn.addEventListener('click', () => {
        const data = mcpConfigs[currentTarget] || mcpConfigs.claude;
        copyTextWithFallback(data.rawPath || data.path, btn, '✓ Path Copied!');
      });
    });
  };

  const loadScript = (src) => new Promise((resolve, reject) => {
    const existing = document.querySelector(`script[src="${src}"]`);
    if (existing?.dataset.loaded === 'true') return resolve();
    if (existing) {
      existing.addEventListener('load', resolve, { once: true });
      existing.addEventListener('error', reject, { once: true });
      return;
    }
    const script = document.createElement('script');
    script.src = src;
    script.async = true;
    script.onload = () => {
      script.dataset.loaded = 'true';
      resolve();
    };
    script.onerror = reject;
    document.head.appendChild(script);
  });

  const startGsapMotion = async () => {
    if (reducedMotion.matches || !document.body) return;

    try {
      await loadScript('/assets/vendor/gsap/gsap.min.js');
      await Promise.all([
        loadScript('/assets/vendor/gsap/ScrollTrigger.min.js'),
        loadScript('/assets/vendor/gsap/ScrollToPlugin.min.js'),
        loadScript('/assets/vendor/gsap/Flip.min.js'),
      ]);
    } catch (_) {
      // CSS and native scrolling remain the resilient, zero-dependency fallback.
      return;
    }

    const { gsap, ScrollTrigger, ScrollToPlugin, Flip } = window;
    if (!gsap || !ScrollTrigger || !ScrollToPlugin) return;
    gsap.registerPlugin(ScrollTrigger, ScrollToPlugin);
    if (Flip) gsap.registerPlugin(Flip);

    const hero = document.querySelector('.hero');
    const motionMedia = gsap.matchMedia();
    if (hero) {
      const orbit = document.createElement('div');
      orbit.className = 'gsap-orbit';
      orbit.setAttribute('aria-hidden', 'true');
      orbit.innerHTML = '<i class="gsap-orbit-node"></i><i class="gsap-orbit-node"></i><i class="gsap-orbit-node"></i>';
      hero.prepend(orbit);

      const pointerLight = document.createElement('div');
      pointerLight.className = 'gsap-pointer-light';
      pointerLight.setAttribute('aria-hidden', 'true');
      hero.append(pointerLight);

      motionMedia.add('(min-width: 768px)', () => {
        const nodes = orbit.querySelectorAll('.gsap-orbit-node');
        const image = hero.querySelector('.product-frame img');
        const xTo = gsap.quickTo(pointerLight, 'x', { duration: 0.55, ease: 'power3.out' });
        const yTo = gsap.quickTo(pointerLight, 'y', { duration: 0.55, ease: 'power3.out' });
        const rotateXTo = image ? gsap.quickTo(image, 'rotationX', { duration: 0.55, ease: 'power3.out' }) : null;
        const rotateYTo = image ? gsap.quickTo(image, 'rotationY', { duration: 0.55, ease: 'power3.out' }) : null;

        gsap.set(pointerLight, { x: hero.clientWidth / 2 - 176, y: hero.clientHeight * 0.32 - 176, autoAlpha: 0.8 });
        gsap.to(nodes[0], { x: 36, y: 22, rotation: 12, duration: 8, ease: 'sine.inOut', repeat: -1, yoyo: true });
        gsap.to(nodes[1], { x: -28, y: 30, rotation: -16, duration: 7.2, ease: 'sine.inOut', repeat: -1, yoyo: true });
        gsap.to(nodes[2], { x: 18, y: -24, scale: 1.08, duration: 6.4, ease: 'sine.inOut', repeat: -1, yoyo: true });

        const onPointerMove = (event) => {
          const rect = hero.getBoundingClientRect();
          const horizontal = gsap.utils.clamp(-1, 1, ((event.clientX - rect.left) / rect.width) * 2 - 1);
          const vertical = gsap.utils.clamp(-1, 1, ((event.clientY - rect.top) / rect.height) * 2 - 1);
          xTo(event.clientX - rect.left - 176);
          yTo(event.clientY - rect.top - 176);
          rotateXTo?.(-vertical * 2.4);
          rotateYTo?.(horizontal * 3.2);
        };
        const onPointerLeave = () => {
          rotateXTo?.(0);
          rotateYTo?.(0);
          gsap.to(pointerLight, { autoAlpha: 0.45, duration: 0.35, overwrite: 'auto' });
        };
        const onPointerEnter = () => gsap.to(pointerLight, { autoAlpha: 0.85, duration: 0.3, overwrite: 'auto' });
        hero.addEventListener('pointermove', onPointerMove, { passive: true });
        hero.addEventListener('pointerleave', onPointerLeave);
        hero.addEventListener('pointerenter', onPointerEnter);

        return () => {
          hero.removeEventListener('pointermove', onPointerMove);
          hero.removeEventListener('pointerleave', onPointerLeave);
          hero.removeEventListener('pointerenter', onPointerEnter);
        };
      });

      const heroImage = hero.querySelector('.product-frame img');
      if (heroImage) {
        gsap.to(heroImage, {
          scale: 1.045,
          yPercent: -2,
          ease: 'none',
          scrollTrigger: {
            trigger: hero,
            start: 'top top',
            end: 'bottom top',
            scrub: 0.7,
          },
        });
      }
    }

    const context = gsap.context(() => {
      if (hero) {
        const intro = Array.from(hero.querySelectorAll('p, h1, .button-primary, .button-secondary, .product-frame'));
        if (intro.length) {
          gsap.timeline({ defaults: { ease: 'power3.out' } })
            .fromTo(intro, { autoAlpha: 0, y: 22 }, { autoAlpha: 1, y: 0, duration: 0.72, stagger: 0.07, clearProps: 'visibility' });
        }
      }

      const answerCards = gsap.utils.toArray('.answer-card');
      if (answerCards.length) {
        gsap.set(answerCards, { autoAlpha: 0, y: 18 });
        ScrollTrigger.batch(answerCards, {
          start: 'top 86%',
          once: true,
          interval: 0.08,
          onEnter: (batch) => gsap.to(batch, {
            autoAlpha: 1,
            y: 0,
            duration: 0.5,
            stagger: 0.07,
            ease: 'power2.out',
            clearProps: 'visibility,transform',
            overwrite: 'auto',
          }),
        });
      }

      const capabilityLinks = gsap.utils.toArray('.capability-panel .cinema-link');
      if (capabilityLinks.length) {
        ScrollTrigger.batch(capabilityLinks, {
          start: 'top 82%',
          once: true,
          onEnter: (batch) => gsap.fromTo(batch, { autoAlpha: 0, y: 10 }, {
            autoAlpha: 1,
            y: 0,
            duration: 0.42,
            stagger: 0.05,
            ease: 'power2.out',
            clearProps: 'visibility,transform',
          }),
        });
      }

      document.querySelectorAll('a[href^="#"], a[href^="/#"]').forEach((link) => {
        link.addEventListener('click', (event) => {
          if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
          const hash = new URL(link.href, window.location.href).hash;
          const target = hash ? document.querySelector(hash) : null;
          if (!target) return;
          event.preventDefault();
          history.pushState(null, '', hash);
          gsap.to(window, { duration: 0.72, scrollTo: { y: target, offsetY: 56 }, ease: 'power3.inOut', overwrite: 'auto' });
        });
      });

      if (Flip) {
        document.querySelectorAll('details').forEach((details) => {
          const summary = details.querySelector('summary');
          const menu = details.querySelector(':scope > div');
          if (!summary || !menu) return;
          summary.addEventListener('click', () => {
            if (details.open) return;
            const state = Flip.getState(menu);
            window.requestAnimationFrame(() => Flip.from(state, { duration: 0.24, ease: 'power2.out', absolute: true }));
          });
        });
      }

      ScrollTrigger.refresh();
    }, document.body);

    document.addEventListener('pagehide', () => {
      context.revert();
      motionMedia.revert();
    }, { once: true });
    reducedMotion.addEventListener('change', (event) => {
      if (event.matches) {
        context.revert();
        motionMedia.revert();
      }
    }, { once: true });
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
    setupDownloadHub();
    setupMcpGuide();
    startGsapMotion();
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initialize, { once: true });
  } else {
    initialize();
  }
})();
