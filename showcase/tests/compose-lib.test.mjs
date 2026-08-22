import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { planCards } = require('../compose_lib.cjs');

test('card plan produces ordered semantic filenames', () => {
  const cards = planCards({
    selected_shots: ['overview.reader', 'presentation.reveal'],
    cover_hook: { formula_id: '#36', title: '写完就能讲', caption: 'Markdown 直接放映，不用重做 PPT。' },
    shots: [
      { id: 'overview.reader', name: '主界面' },
      { id: 'presentation.reveal', name: '放映' },
    ],
  });
  assert.deepEqual(cards.map((card) => card.file), [
    'xhs-01-cover.jpg',
    'xhs-02-overview-reader.jpg',
    'xhs-03-presentation-reveal.jpg',
    'xhs-04-summary.jpg',
  ]);
  assert.equal(cards[1].role, 'pure_ui_hero');
  assert.equal(cards[1].uiMinRatio, 0.7);
});

test('cover plan follows the selected release mechanism', () => {
  const cards = planCards({
    primary_shot: 'editor.diagram-picker',
    selected_shots: ['overview.reader', 'editor.diagram-picker'],
    shots: [
      { id: 'overview.reader', name: '主界面' },
      { id: 'editor.diagram-picker', name: '图表' },
    ],
    cover_hook: { formula_id: '#36', title: '图表直接选', caption: '科研图从面板进入 Markdown，不背语法。' },
  });
  assert.equal(cards[0].title, '图表直接选');
  assert.equal(cards[0].caption, '科研图从面板进入 Markdown，不背语法。');
  assert.throws(() => planCards({ selected_shots: ['overview.reader'] }), /cover_hook is missing/);
});
