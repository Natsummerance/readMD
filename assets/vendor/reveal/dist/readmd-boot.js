/* ReadMD Reveal.js 启动器：配置经 JSON <script> 注入，代码本体同源加载以符合 CSP。 */
(function () {
  'use strict';
  var cfgEl = document.getElementById('readmd-reveal-config');
  var cfg = {};
  try { cfg = JSON.parse(cfgEl ? cfgEl.textContent : '{}') || {}; } catch (e) { cfg = {}; }

  var plugins = [];
  if (window.RevealMarkdown) plugins.push(window.RevealMarkdown);
  if (window.RevealHighlight) plugins.push(window.RevealHighlight);
  if (window.RevealNotes) plugins.push(window.RevealNotes);
  var katexPlugin = (!cfg.standalone && window.RevealMath && window.RevealMath.KaTeX) || null;
  if (katexPlugin) plugins.push(katexPlugin);

  var init = {
    width: 1080,
    height: 720,
    margin: 0.06,
    minScale: 0.2,
    maxScale: 2.0,
    controls: true,
    progress: true,
    center: false,
    /* srcdoc iframe 内无真实 URL，hash 路由会抛 replaceState 异常；仅导出单文件启用 */
    hash: !!cfg.standalone,
    transition: cfg.transition || 'slide',
    slideNumber: 'c/t',
    plugins: plugins
  };
  if (katexPlugin && cfg.katexLocal) init.katex = { local: cfg.katexLocal };

  window.deck = new Reveal(init);
  deck.initialize();

  if (cfg.standalone && typeof renderMathInElement === 'function') {
    deck.on('ready', function () {
      renderMathInElement(deck.getSlidesElement(), {
        delimiters: [
          { left: '$$', right: '$$', display: true },
          { left: '$', right: '$', display: false },
          { left: '\\(', right: '\\)', display: false },
          { left: '\\[', right: '\\]', display: true }
        ],
        ignoredTags: ['script', 'noscript', 'style', 'textarea', 'pre']
      });
    });
  }

  /* 监听 iframe 内鼠标移动并同步给父窗口，用于实现禅模式顶部工具栏与右下角翻页按钮的接近感应 */
  window.addEventListener('mousemove', function (e) {
    if (window.parent && window.parent !== window) {
      window.parent.postMessage({
        type: 'pres-mousemove',
        clientX: e.clientX,
        clientY: e.clientY,
        innerWidth: window.innerWidth,
        innerHeight: window.innerHeight
      }, '*');
    }
  }, { passive: true });

  window.addEventListener('keydown', function (e) {
    if (e.key === 'F11') {
      e.preventDefault();
      if (window.parent && window.parent !== window) {
        window.parent.postMessage({ type: 'pres-toggle-fullscreen' }, '*');
      }
    }
  });

  window.addEventListener('message', function (event) {
    if (!event.data || typeof event.data !== 'object') return;
    var data = event.data;
    if (data.type === 'set-theme' && data.theme && /^[a-z0-9-]+$/i.test(data.theme)) {
      var next = (cfg.themeBase || 'assets/vendor/reveal/dist/theme/') + data.theme + '.css';
      var link = document.getElementById('theme');
      if (!link) {
        link = document.createElement('link');
        link.rel = 'stylesheet';
        link.id = 'theme';
        document.head.appendChild(link);
      }
      link.href = next;
    } else if (data.type === 'set-transition' && data.transition) {
      if (window.deck && typeof window.deck.configure === 'function') {
        window.deck.configure({ transition: data.transition });
      }
    } else if (data.type === 'set-font-size' && data.size) {
      document.documentElement.style.setProperty('--reveal-base-font-size', data.size + 'px');
    } else if (data.type === 'toggle-overview') {
      if (window.deck && typeof window.deck.toggleOverview === 'function') {
        window.deck.toggleOverview();
      }
    } else if (data.type === 'set-zen-controls') {
      if (data.showControls) {
        document.body.classList.remove('reveal-zen-controls-hidden');
      } else {
        document.body.classList.add('reveal-zen-controls-hidden');
      }
    }
  });
})();
