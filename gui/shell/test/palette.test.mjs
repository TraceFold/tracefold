// SPDX-License-Identifier: Apache-2.0
// The palette's grammar and its results, with no window in sight.
//
// req/811 §8-5 ruled one invoked palette with a faceted grammar, and ruled that a result
// is an ADDRESS rather than a place on a screen. The second half is the one that must be
// held by a test, because it is the half that is easy to lose: a search that lands you
// somewhere feels finished, and only a check notices that it landed you somewhere
// unrepeatable.

import test from 'node:test';
import assert from 'node:assert/strict';

import { parseQuery, corpusOf, search, FACETS, PALETTE_SAID } from '../kernel/palette.mjs';
import { readManifest } from '../kernel/manifest.mjs';
import { openingState } from '../kernel/shell.mjs';
import { MANIFEST } from '../app/manifest.gen.mjs';

const read = readManifest(MANIFEST);
const state = openingState(read);
const corpus = corpusOf({ ...read, byId: new Map(read.faces.map((f) => [f.id, f])) }, state);

test('a query names its axis, and an axis this palette does not have is said', () => {
  assert.deepEqual(parseQuery('box:right ledger').facets, { box: 'right' });
  assert.deepEqual(parseQuery('box:right ledger').words, ['ledger']);
  assert.deepEqual(parseQuery('bx:right').unknown, ['bx']);
  const said = search('bx:right', corpus);
  assert.equal(said.rows.length, 0);
  assert.match(said.said, /there is no bx: axis/);
  // Not silently searched for as literal text: that is how a typo becomes "no results"
  // and a reader concludes the thing they wanted does not exist.
  assert.notEqual(said.said, PALETTE_SAID.none('bx:right'));
});

test('the three axes are the three the ruling fixed, and each says what it means here', () => {
  assert.deepEqual(Object.keys(FACETS).sort(), ['box', 'ext', 'phase']);
  for (const means of Object.values(FACETS)) assert.equal(typeof means, 'string');
});

test('every corpus row that can be reached carries the gx line that reproduces it', () => {
  const reachable = corpus.filter((row) => row.land !== null);
  assert.ok(reachable.length > 0, 'nothing is reachable, so this test is measuring an empty set');
  for (const row of reachable) {
    assert.match(row.address, /^gx (tab|dock):go /, `${row.id} has no address`);
    assert.match(row.address, /--index \d+/);
  }
});

test('a face standing nowhere is listed, with the reason it has no address', () => {
  const nowhere = corpus.filter((row) => row.box === 'nowhere');
  assert.ok(nowhere.length > 0, 'the stage opens on one tab, so some faces stand nowhere');
  for (const row of nowhere) {
    assert.equal(row.address, null);
    assert.equal(row.land, null);
    assert.match(row.why, /stands nowhere/);
  }
});

test('the box axis finds a docked face the tab strip cannot carry', () => {
  const found = search('box:right', corpus);
  assert.ok(found.rows.length > 0);
  for (const row of found.rows) assert.equal(row.box, 'right');
  assert.match(found.rows[0].address, /^gx dock:go .*--side right/);
});

test('an empty query searches for nothing and says so, rather than listing everything', () => {
  const found = search('   ', corpus);
  assert.equal(found.rows.length, 0);
  assert.equal(found.said, PALETTE_SAID.empty);
});

test('red-first: a query that matches nothing is a stated absence, not an empty screen', () => {
  const found = search('no-such-face-anywhere', corpus);
  assert.equal(found.rows.length, 0);
  assert.match(found.said, /nothing here matches/);
});
