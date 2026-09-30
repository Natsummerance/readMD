import test from 'node:test';
import assert from 'node:assert/strict';
import { analyse } from '../check-wiring.mjs';

test('unbound control is reported, bound and hooked ones are not', () => {
  const html = '<button id="a">A</button><button id="b">B</button><input id="dead" type="file"><button data-action="x">X</button>';
  const p = analyse(html, ["$('a').onclick=1; document.getElementById(\"b\")"]);
  assert.deepEqual(p.map(x => x.kind + ':' + x.id), ['unbound:dead']);
});

test('icon-only buttons need an accessible name', () => {
  const html = '<button id="i1"><svg><path/></svg></button><button id="i2" aria-label="Close"><svg/></button><button id="i3" data-i18n-aria="k"><svg></svg></button>';
  const p = analyse(html, ["'i1' 'i2' 'i3'"]);
  assert.deepEqual(p.map(x => x.kind + ':' + x.id), ['unnamed-icon-button:i1']);
});

test('dangling aria-labelledby is reported', () => {
  const html = '<div role="dialog" aria-labelledby="t1"></div><div role="dialog" aria-labelledby="t2"></div><h3 id="t1">x</h3>';
  const p = analyse(html, ['']);
  assert.deepEqual(p.map(x => x.kind + ':' + x.id), ['dangling-aria-labelledby:t2']);
});
