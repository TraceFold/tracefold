// SPDX-License-Identifier: Apache-2.0
// The declaration, checked against the two things it can be wrong about: the server
// it names methods on, and the files it names tests in.

import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { DECLARATION } from '../declaration.mjs';
import { tableRows } from '../../../membrane/src/index.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FACE_DIR = path.resolve(HERE, '..');

test('C-1: every declared method is a method the membrane actually carries', () => {
  const carried = new Set(tableRows().map((row) => row.name));
  const unknown = DECLARATION.consumes.filter((name) => !carried.has(name));
  assert.deepEqual(unknown, [], `declared but not on the wire: ${unknown.join(', ')}`);
});

test('C-1 negative control: a name the membrane does not carry is detectable', () => {
  const carried = new Set(tableRows().map((row) => row.name));
  assert.equal(carried.has('get_everything_i_wish_for'), false);
});

test('the reference tree\'s declared method count is honestly deviated from, and the deviation is stated', () => {
  // req/03 §3-1 declares 2 for this face's reference row (transformations /
  // subscribe). "subscribe" has no membrane equivalent (get_stream is the nearest
  // real route and is declared undrawn instead), the same honest deviation
  // faces/held already states for the identical reference word -- so this
  // declaration matches 1 of the reference's 2, not silently 2.
  assert.equal(DECLARATION.consumes.length, 1);
  assert.deepEqual([...DECLARATION.consumes], ['get_transformations']);
});

test('C-2: this face sends no acts and withholds nothing -- a read-only screen states that structurally', () => {
  assert.deepEqual(DECLARATION.acts, []);
  assert.deepEqual(DECLARATION.withheld, []);
  assert.deepEqual([...DECLARATION.sends], ['get_transformations']);
  for (const name of DECLARATION.sends) assert.ok(DECLARATION.consumes.includes(name));
});

test('C-3: the face declares what it does not draw, with a reason for each', () => {
  assert.ok(DECLARATION.undrawn.length >= 3, 'too few declared omissions to be a real declaration');
  for (const entry of DECLARATION.undrawn) {
    assert.equal(typeof entry.what, 'string');
    assert.ok(typeof entry.why === 'string' && entry.why.length > 20, `${entry.what} is undrawn without a reason`);
  }
  assert.equal(DECLARATION.rows.draws, true);
  assert.equal(DECLARATION.rows.reports_denominator, true, 'a graph screen must state its denominators, not only its subjects');
});

test('C-3: a path touched exactly once, and an edge leaving the window, are both named as never-drawn, unconditionally', () => {
  const touchedOnce = DECLARATION.undrawn.find((e) => e.what.includes('touched exactly once'));
  const edgeOutside = DECLARATION.undrawn.find((e) => e.what.includes('predecessor this window did not read'));
  assert.ok(touchedOnce, 'the declaration does not name "touched once is not a subject" as an omission');
  assert.ok(edgeOutside, 'the declaration does not name "edge leaving the window" as an omission');
});

test('C-4/C-5: marks are declared, and one meaning never carries two marks', () => {
  assert.ok(DECLARATION.marks.length >= 5);
  const byMeaning = new Map();
  for (const mark of DECLARATION.marks) {
    assert.equal(typeof mark.mark, 'string');
    assert.equal(typeof mark.means, 'string');
    const seen = byMeaning.get(mark.means);
    assert.equal(seen, undefined, `two marks for ${mark.means}: ${seen} and ${mark.mark}`);
    byMeaning.set(mark.means, mark.mark);
  }
});

test('the marks drawn are honestly derived, not copied from the reference\'s count', () => {
  // 9 data marks (unchanged, honestly fewer than the reference's 11) + 2 chrome
  // marks (structure/fold-shut, structure/fold-open) the SS657 retrofit lane
  // (round 1) added for the why/legend controls' own fold glyph -- see
  // faces/receipt's identical situation and identical test-pin reasoning.
  // Retrofit round 2 (req/768 AC-7) adds two more: standing/reversed and
  // standing/none, the reversibility chip's two reachable states on this
  // screen (every touch this face draws has lifecycle 'settled', so the third
  // state, not-committed/standing/held, is unreachable here and not declared).
  assert.equal(DECLARATION.marks.length, 13);
  const dataMarks = DECLARATION.marks.filter((m) => !['structure/fold-shut', 'structure/fold-open', 'standing/reversed', 'standing/none'].includes(m.mark));
  assert.equal(dataMarks.length, 9, 'the data marks this screen draws about the graph itself are still 9, fewer than the reference\'s 11');
});

test('one mark on this face is new and belongs to no other face: structure/outside', () => {
  const mark = DECLARATION.marks.find((m) => m.means === 'structure.outside');
  assert.ok(mark, 'structure.outside is not declared');
  assert.equal(mark.mark, 'structure/outside');
});

test('C-5: graph\'s marks agree with faces/ledger\'s, faces/held\'s and faces/receipt\'s marks for every meaning shared', async () => {
  const { DECLARATION: LEDGER } = await import('../../ledger/declaration.mjs');
  const { DECLARATION: HELD } = await import('../../held/declaration.mjs');
  const { DECLARATION: RECEIPT } = await import('../../receipt/declaration.mjs');
  const byMeaning = new Map([...LEDGER.marks, ...HELD.marks, ...RECEIPT.marks].map((m) => [m.means, m.mark]));
  for (const mark of DECLARATION.marks) {
    if (byMeaning.has(mark.means)) assert.equal(mark.mark, byMeaning.get(mark.means), `${mark.means}: graph says ${mark.mark}, elsewhere says ${byMeaning.get(mark.means)}`);
  }
});

test('C-6: the position in the rail is declared, and so is the reason for it', () => {
  assert.equal(typeof DECLARATION.order.position, 'number');
  assert.equal(DECLARATION.order.position, 4, 'graph sits directly after receipt (position 3)');
  assert.ok(DECLARATION.order.reason.length > 20, 'a position without a reason is a lexical accident with a number on it');
  assert.equal(DECLARATION.rows.order, 'by-sequence');
  assert.ok(DECLARATION.rows.order_reason.length > 20);
  assert.equal(DECLARATION.rows.groups_order, 'most-touched-first');
  assert.ok(DECLARATION.rows.groups_order_reason.length > 20);
});

test('C-8: every test the declaration names exists', () => {
  for (const named of DECLARATION.tests) {
    assert.ok(fs.existsSync(path.join(FACE_DIR, named)), `declared test is missing: ${named}`);
  }
  assert.ok(DECLARATION.tests.length >= 3);
});

test('C-8 negative control: a named test that does not exist is caught', () => {
  assert.equal(fs.existsSync(path.join(FACE_DIR, 'test/there-is-no-such-test.mjs')), false);
});

test('C-7 is not this face: the face with no methods is named, not assumed', () => {
  assert.ok(DECLARATION.consumes.length > 0);
  assert.equal(typeof DECLARATION.silent_face, 'string');
  assert.ok(DECLARATION.silent_face.length > 10);
});

test('the face states one question and does not grow a second', () => {
  assert.equal(typeof DECLARATION.question, 'string');
  assert.equal(DECLARATION.question.split('?').length <= 2, true);
  assert.equal(DECLARATION.id, 'graph');
});
