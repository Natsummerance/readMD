import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { planCards } = require('../compose_lib.cjs');

test('card plan produces ordered semantic filenames', () => {
  const cards = planCards({
    selected_shots: ['overview.reader', 'presentation.reveal'],
    card_plan: [
      { index: 1, file: 'xhs-01-cover.jpg', role: 'cover', shot_id: null },
      { index: 2, file: 'xhs-02-overview-reader.jpg', role: 'pure_ui_hero', shot_id: 'overview.reader', title: '软件完整主界面', caption: '打开文档就能看到完整排版、目录和公式渲染', ui_min_ratio: 0.7 },
      { index: 3, file: 'xhs-03-presentation-reveal.jpg', role: 'annotated_ui', shot_id: 'presentation.reveal', title: 'Reveal.js 演说模式放映', caption: '写完的 Markdown 能直接上台放映', ui_min_ratio: 0.55 },
      { index: 4, file: 'xhs-04-summary.jpg', role: 'summary', shot_id: null, title: '一条放映路', caption: '写作、修改和上台共用一份文件。', proof_points: ['同一份 MD', '真实排版', '直接放映'], ui_min_ratio: 0.3 },
    ],
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
    card_plan: [
      { index: 1, file: 'xhs-01-cover.jpg', role: 'cover', shot_id: null },
      { index: 2, file: 'xhs-02-overview-reader.jpg', role: 'pure_ui_hero', shot_id: 'overview.reader', title: '软件完整主界面', caption: '打开文档就能看到完整排版、目录和公式渲染', ui_min_ratio: 0.7 },
      { index: 3, file: 'xhs-03-editor-diagram-picker.jpg', role: 'annotated_ui', shot_id: 'editor.diagram-picker', title: '科学图表选择器', caption: '科研图表从语法记忆变成面板选择', ui_min_ratio: 0.55 },
      { index: 4, file: 'xhs-04-summary.jpg', role: 'summary', shot_id: null, title: '图随文稿走', caption: '科研图表留在可维护的 Markdown 里。', proof_points: ['面板选图', '源码可改', '渲染留档'], ui_min_ratio: 0.3 },
    ],
    shots: [
      { id: 'overview.reader', name: '主界面' },
      { id: 'editor.diagram-picker', name: '图表' },
    ],
    cover_hook: { formula_id: '#36', title: '图表直接选', caption: '科研图从面板进入 Markdown，不背语法。' },
  });
  assert.equal(cards[0].title, '图表直接选');
  assert.equal(cards[0].caption, '科研图从面板进入 Markdown，不背语法。');
  assert.equal(cards[2].caption, '科研图表从语法记忆变成面板选择');
  assert.equal(cards[3].title, '图随文稿走');
  assert.deepEqual(cards[3].proof_points, ['面板选图', '源码可改', '渲染留档']);
  assert.throws(() => planCards({ selected_shots: ['overview.reader'] }), /cover_hook is missing/);
  assert.throws(() => planCards({
    selected_shots: ['overview.reader'],
    cover_hook: { formula_id: '#36', title: '同屏改稿', caption: '改完立刻看到排版，不用切窗口。' },
    shots: [{ id: 'overview.editor' }],
  }), /card_plan is missing/);
});
