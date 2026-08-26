// SPDX-License-Identifier: Apache-2.0
// The stage's tree, and the property the whole inverse design rests on: nothing here
// changes a node that already exists.

import test from 'node:test';
import assert from 'node:assert/strict';

import { leaf, split, divide, drop, setRatios, at, replaceAt, leafPaths, splitPaths, normalise, isLeaf, countLeaves, MIN_RATIO } from '../kernel/tree.mjs';

test('a node is frozen, and dividing does not touch the node it divided', () => {
  const root = leaf(['a', 'b'], 1);
  assert.ok(Object.isFrozen(root));
  assert.ok(Object.isFrozen(root.tabs));
  const after = divide(root, [], 'row');
  assert.equal(root.tabs.length, 2);
  assert.equal(root.active, 1);
  assert.equal(after.k, 's');
  assert.equal(after.kids[0], root, 'the original leaf is carried, not rebuilt');
});

test('a sibling joins an existing split along the same axis instead of nesting', () => {
  const root = split('row', [leaf(['a']), leaf(['b'])]);
  const after = divide(root, [0], 'row');
  assert.equal(after.kids.length, 3);
  assert.equal(countLeaves(after), 3);
  const nested = divide(root, [0], 'col');
  assert.equal(nested.kids.length, 2);
  assert.equal(nested.kids[0].k, 's');
  assert.equal(nested.kids[0].axis, 'col');
});

test('dropping to one child replaces the split by that child, identity intact', () => {
  const kept = leaf(['a']);
  const root = split('row', [kept, leaf(['b'])]);
  const after = drop(root, [1]);
  assert.equal(after, kept, 'the surviving child is the same node, not a copy of it');
});

test('ratios are normalised, rounded, and refuse to crush a pane out of sight', () => {
  const ratios = normalise([1, 1, 1]);
  assert.equal(ratios.reduce((a, b) => a + b, 0), 1);
  for (const r of ratios) assert.equal(r, Number(r.toFixed(4)));
  const root = split('row', [leaf(), leaf()]);
  assert.throws(() => setRatios(root, [], [0.001, 0.999]), /below/);
  assert.ok(MIN_RATIO > 0);
});

test('paths name places, and replaceAt rebuilds only the spine', () => {
  const deep = split('row', [leaf(['a']), split('col', [leaf(['b']), leaf(['c'])])]);
  assert.deepEqual(leafPaths(deep), [[0], [1, 0], [1, 1]]);
  assert.deepEqual(splitPaths(deep), [[], [1]]);
  const untouched = at(deep, [0]);
  const after = replaceAt(deep, [1, 1], leaf(['d']));
  assert.equal(at(after, [0]), untouched);
  assert.equal(at(after, [1, 1]).tabs[0], 'd');
  assert.ok(isLeaf(at(after, [1, 1])));
});

test('a path that runs past a leaf is refused rather than answered', () => {
  assert.throws(() => at(leaf(['a']), [0]), /past a leaf/);
  assert.throws(() => split('row', [leaf()]), /two children/);
  assert.throws(() => split('diagonal', [leaf(), leaf()]), /row or col/);
});
