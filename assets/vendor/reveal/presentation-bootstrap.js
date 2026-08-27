(function () {
  'use strict';

  window.deck = new Reveal({
    width: 1080,
    height: 720,
    margin: 0.06,
    minScale: 0.2,
    maxScale: 2.0,
    controls: true,
    progress: true,
    center: true,
    hash: false,
    transition: document.body.dataset.transition || 'slide',
    slideNumber: 'c/t',
    katex: {
      local: '/assets/vendor/katex',
      version: '0.16.8',
    },
    plugins: [RevealMarkdown, RevealHighlight, RevealNotes, RevealMath.KaTeX],
  });
  window.deck.initialize();

  window.addEventListener('message', function (event) {
    if (!event.data || typeof event.data !== 'object') return;
    var data = event.data;
    if (data.type === 'set-theme' && data.theme) {
      var themeLink = document.getElementById('theme');
      if (themeLink) themeLink.href = '/assets/vendor/reveal/theme/' + data.theme + '.css';
    } else if (data.type === 'set-transition' && data.transition && window.deck.configure) {
      window.deck.configure({ transition: data.transition });
    } else if (data.type === 'set-font-size' && data.size) {
      document.documentElement.style.setProperty('--reveal-base-font-size', data.size + 'px');
    } else if (data.type === 'toggle-overview' && window.deck.toggleOverview) {
      window.deck.toggleOverview();
    }
  });
})();
