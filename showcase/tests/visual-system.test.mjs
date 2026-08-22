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
    cover_hook: { formula_id: '#36', title: '写完就能讲', caption: 'Markdown 直接放映，不用重做 PPT。' },
    selected_shots: ['overview.reader', 'presentation.reveal'],
    card_plan: [
      { index: 1, file: 'xhs-01-cover.jpg', role: 'cover', shot_id: null },
      { index: 2, file: 'xhs-02-overview-reader.jpg', role: 'pure_ui_hero', shot_id: 'overview.reader', title: '软件完整主界面', caption: '打开文档就能看到完整排版、目录和公式渲染', ui_min_ratio: 0.7 },
      { index: 3, file: 'xhs-03-presentation-reveal.jpg', role: 'annotated_ui', shot_id: 'presentation.reveal', title: 'Reveal.js 演说模式放映', caption: '写完的 Markdown 能直接上台放映', ui_min_ratio: 0.55 },
      { index: 4, file: 'xhs-04-summary.jpg', role: 'summary', shot_id: null, title: '一条放映路', caption: '写作、修改和上台共用一份文件。', proof_points: ['同一份 MD', '真实排版', '直接放映'], ui_min_ratio: 0.3 },
    ],
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
  assert.match(html, /写完就能讲/);
  assert.match(html, /Markdown 直接放映，不用重做 PPT。/);
});

test('summary keeps three outcomes and feature cards stay image-led', () => {
  const design = loadDesignSystem();
  const summary = buildCardHtml({
    role: 'summary',
    title: '一条放映路',
    caption: '写作、修改和上台共用一份文件。',
    proof_points: ['同一份 MD', '真实排版', '直接放映'],
    shotId: 'overview.reader',
  }, 'data:image/png;base64,real', { design });
  assert.equal([...summary.matchAll(/<li/g)].length, 3);
  assert.match(summary, /一条放映路/);
  assert.match(summary, /直接放映/);
  const feature = buildCardHtml({ role: 'annotated_ui', title: '放映', caption: '一句话', shotId: 'overview.reader' }, 'data:image/png;base64,real', { design });
  assert.equal([...feature.matchAll(/<img\b/g)].length, 1);
  assert.doesNotMatch(feature, /class="annotation"/);
});

test('feature cards use the plan reader value instead of technical descriptions', () => {
  const design = loadDesignSystem();
  const html = buildCardHtml({
    role: 'annotated_ui',
    shotId: 'editor.code-chunk',
    title: '可执行代码块卡片',
    caption: '文档里的代码可以直接运行并保留输出',
  }, 'data:image/png;base64,real', { design });
  assert.match(html, /文档里的代码可以直接运行并保留输出/);
  assert.doesNotMatch(html, /CodeMirror/);
});
