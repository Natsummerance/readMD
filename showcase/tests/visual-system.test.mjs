import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { loadDesignSystem, buildCardHtml, planCards } = require('../compose_lib.cjs');

test('design system locks one evidence-paper accent and readable type scale', () => {
  const design = loadDesignSystem();
  assert.equal(design.schema_version, 1);
  assert.equal(design.palette.accent, '#d6482c');
  assert.notEqual(design.palette.background, '#0e1630');
  assert.ok(design.type.display.size >= 72);
  assert.ok(design.type.body.size >= 24);
  assert.match(design.signature, /proof/i);
});

test('cover carries a real UI strip and rejects the generic feature grid', () => {
  const story = {
    release: 'v1.2.3',
    angle: 'ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映',
    primary_shot: 'presentation.reveal',
    selected_shots: ['overview.reader', 'presentation.reveal'],
    shots: [
      { id: 'overview.reader', name: '软件完整主界面', role: 'pure_ui_hero' },
      { id: 'presentation.reveal', name: 'Reveal.js 演说模式放映', role: 'annotated_ui' },
    ],
  };
  const cards = planCards(story);
  assert.equal(cards[0].shotId, 'overview.reader');
  const html = buildCardHtml(cards[0], 'data:image/png;base64,real', { design: loadDesignSystem() });
  assert.match(html, /class="proof-strip"/);
  assert.doesNotMatch(html, /class="grid"/);
});

test('summary keeps three outcomes and feature cards stay image-led', () => {
  const design = loadDesignSystem();
  const summary = buildCardHtml({ role: 'summary', title: '总结', caption: '', shotId: 'overview.reader' }, 'data:image/png;base64,real', { design });
  assert.equal([...summary.matchAll(/<li/g)].length, 3);
  const feature = buildCardHtml({ role: 'annotated_ui', title: '放映', caption: '一句话', shotId: 'overview.reader' }, 'data:image/png;base64,real', { design });
  assert.equal([...feature.matchAll(/<img\b/g)].length, 1);
  assert.doesNotMatch(feature, /class="annotation"/);
});
