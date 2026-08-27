import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const {
  buildCardHtml,
  listPosterStyles,
  loadDesignSystem,
  loadDesignSystemForStyle,
  resolveDesignSystem,
} = require('../compose_lib.cjs');

const featureCard = {
  role: 'annotated_ui',
  shotId: 'presentation.reveal',
  title: '写完就能讲',
  caption: '同一份 Markdown 可以直接放映',
};

const dualEvidenceCard = {
  ...featureCard,
  secondaryShotId: 'editor.code-chunk',
};

test('poster registry preserves evidence-paper and exposes four expansions', () => {
  assert.deepEqual(listPosterStyles(), [
    'evidence-paper',
    'minimal-zine',
    'morandi-cinematic',
    'photo-abstract',
    'photo-relic',
  ]);
});

test('every expansion keeps the feed type, palette, and canvas contract', () => {
  const styles = listPosterStyles().map((style) => loadDesignSystemForStyle(style));
  const accents = new Set(styles.map((design) => design.palette.accent));
  assert.equal(accents.size, styles.length);
  for (const design of styles) {
    assert.equal(design.layout.canvas.join('x'), '1080x1440');
    assert.ok(design.type.display.size >= 96);
    assert.ok(design.type.body.size >= 24);
    assert.ok(design.type.utility.size >= 22);
    assert.match(design.signature, /proof|authentic|running/i);
  }
});

test('story can select a style while omitted stories remain evidence-paper', () => {
  assert.equal(resolveDesignSystem({}).name, 'evidence-paper');
  assert.equal(resolveDesignSystem({}, 'photo-relic').template, 'photo-relic');
  assert.equal(resolveDesignSystem({ poster_style: 'morandi-cinematic' }).name, 'morandi-cinematic');
  assert.throws(() => resolveDesignSystem({ poster_style: 'fake-poster' }), /Unknown poster style/);
});

test('default markup stays stable and expansions declare distinct templates', () => {
  const source = 'data:image/png;base64,real';
  const fallback = buildCardHtml(featureCard, source, { design: loadDesignSystem() });
  assert.match(fallback, /<main class="poster annotated_ui evidence-paper">/);
  assert.match(fallback, /data-poster-template="evidence-paper"/);

  const expectations = {
    'minimal-zine': /class="poster annotated_ui minimal-zine"/,
    'photo-relic': /\.photo-relic \.evidence\{border-width:3px/,
    'morandi-cinematic': /\.morandi-cinematic \.evidence\{border-width:8px/,
    'photo-abstract': /class="poster annotated_ui photo-abstract"/,
  };
  for (const [style, pattern] of Object.entries(expectations)) {
    const html = buildCardHtml(featureCard, source, { design: loadDesignSystemForStyle(style) });
    assert.match(html, pattern, style);
    assert.match(html, /真实运行画面/, style);
    assert.match(html, /object-fit:contain/, style);
    assert.doesNotMatch(html, /object-fit:cover/, style);
  }
});

test('landscape feature cards pair two different authentic captures', () => {
  const html = buildCardHtml(
    dualEvidenceCard,
    '',
    {
      design: loadDesignSystemForStyle('minimal-zine'),
      sources: {
        'presentation.reveal': 'data:image/png;base64,primary',
        'editor.code-chunk': 'data:image/png;base64,secondary',
      },
    },
  );
  assert.equal([...html.matchAll(/<img\b/g)].length, 2);
  assert.match(html, /主画面 · 真实运行/);
  assert.match(html, /关联工作流 · 真实运行/);
  assert.match(html, /src="data:image\/png;base64,primary"/);
  assert.match(html, /src="data:image\/png;base64,secondary"/);
});
