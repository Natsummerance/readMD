import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { planCards } = require('../compose_lib.cjs');

test('card plan produces ordered semantic filenames', () => {
  const cards = planCards({
    selected_shots: ['overview.reader', 'presentation.reveal'],
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
