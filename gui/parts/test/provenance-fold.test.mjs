// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { el, toHtml, textOf, findByAttr, find } from '../src/element.mjs';
import { fold, withoutFolds, HALVES, FOLD_KIND, FOLD_MESSAGES } from '../src/provenance-fold.mjs';

const SETTLED = [{ name: 'anchor', value: 'tile/44' }];
const HELD = [{ name: 'awaiting', value: 'an owner present in this session' }];

const claims = () => el('dl', { id: 'claims' }, [
  el('dt', { id: 'serial' }, ['serial']),
  el('dd', {}, ['A73E39']),
  el('dt', { id: 'false-when' }, ['false when']),
  el('dd', {}, ['the recomputed digest differs from the one recorded here']),
]);

test('a fold states what it holds before anyone opens it', () => {
  assert.throws(() => fold({ settled: SETTLED }), new RegExp(FOLD_MESSAGES.NEEDS_SUMMARY));
  assert.throws(() => fold({ summary: '   ', settled: SETTLED }), new RegExp(FOLD_MESSAGES.NEEDS_SUMMARY));
  const node = fold({ summary: 'where that came from', settled: SETTLED });
  assert.equal(textOf(node.children[0]), 'where that came from');
});

test('removing every fold leaves the claims standing, which is the check that was written and never written', () => {
  const page = toHtml(el('div', {}, [claims(), fold({ summary: 'where that came from', settled: SETTLED, held: HELD })]));
  const stripped = withoutFolds(page);
  assert.equal(stripped.includes('<details'), false);
  for (const id of ['claims', 'serial', 'false-when']) assert.ok(stripped.includes(`id="${id}"`), `${id} survives the fold being cut out`);
  assert.ok(stripped.includes('the recomputed digest differs'));
});

test('that check has teeth: a claim moved inside the fold disappears with it', () => {
  const wrong = toHtml(el('div', {}, [fold({ summary: 'where that came from', settled: [{ name: 'false when', value: 'the recomputed digest differs' }] })]));
  const stripped = withoutFolds(wrong);
  assert.equal(stripped.includes('the recomputed digest differs'), false);
});

test('both halves are drawn even when one of them is empty, and the empty one says which kind of empty', () => {
  const node = fold({ summary: 'where that came from', settled: SETTLED, held: [] });
  const halves = findByAttr(node, 'data-half');
  assert.deepEqual(halves.map((h) => h.attrs['data-half']), ['settled', 'held']);
  assert.equal(halves[1].attrs['data-count'], '0');
  const empty = findByAttr(halves[1], 'data-role', 'empty');
  assert.equal(empty.length, 1);
  assert.equal(textOf(empty[0]), FOLD_MESSAGES.HELD_EMPTY);
  assert.notEqual(FOLD_MESSAGES.HELD_EMPTY, FOLD_MESSAGES.SETTLED_EMPTY, 'the two emptinesses are not the same sentence');
});

test('an empty fold draws both halves empty rather than drawing nothing', () => {
  const node = fold({ summary: 'where that came from' });
  const empties = findByAttr(node, 'data-role', 'empty');
  assert.equal(empties.length, 2);
});

test('the halves keep different words and different marks, so what is held never wears what is settled', () => {
  const [settled, held] = HALVES;
  assert.notEqual(settled.label, held.label);
  assert.notDeepEqual(settled.mark, held.mark);
  assert.deepEqual(settled.mark, ['structure', 'seal']);
  assert.deepEqual(held.mark, ['standing', 'held']);
  const node = fold({ summary: 's', settled: SETTLED, held: HELD });
  const marks = find(node, (n) => n.attrs['data-mark']).map((n) => n.attrs['data-mark']);
  assert.deepEqual(marks, ['structure/seal', 'standing/held']);
});

test('entries land in the half they were given to, and nowhere else', () => {
  const node = fold({ summary: 's', settled: SETTLED, held: HELD });
  const [settledHalf, heldHalf] = findByAttr(node, 'data-half');
  assert.ok(textOf(settledHalf).includes('tile/44'));
  assert.equal(textOf(settledHalf).includes('awaiting'), false);
  assert.ok(textOf(heldHalf).includes('awaiting'));
  assert.equal(textOf(heldHalf).includes('tile/44'), false);
});

test('this fold is stamped, so a second details elsewhere on a page is not mistaken for it', () => {
  const node = fold({ summary: 's', settled: SETTLED });
  assert.equal(node.attrs['data-kind'], FOLD_KIND);
  assert.equal(node.attrs['data-part'], 'provenance-fold');
  const other = toHtml(el('details', {}, [el('summary', {}, ['something else']), el('p', {}, ['kept'])]));
  assert.ok(withoutFolds(other).includes('kept'), 'a details that is not this part is left alone');
});

test('open is a state of the element, not a second copy of the content', () => {
  assert.equal(fold({ summary: 's', open: true }).attrs.open, 'open');
  assert.equal(fold({ summary: 's', open: false }).attrs.open, undefined);
  assert.equal(textOf(fold({ summary: 's', settled: SETTLED, open: false })), textOf(fold({ summary: 's', settled: SETTLED, open: true })));
});

test('every glyph in a fold was asked for at a size', () => {
  for (const svg of find(fold({ summary: 's', settled: SETTLED, held: HELD, size: 12 }), (n) => n.tag === 'svg')) {
    assert.equal(svg.attrs.width, '12');
  }
});
