// SPDX-License-Identifier: Apache-2.0
// The declaration, checked against the two things it can be wrong about: the C-7
// property it exists to hold, and the files it names as its tests.

import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { DECLARATION } from '../declaration.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FACE_DIR = path.resolve(HERE, '..');

test('C-7: this face declares no method of the server, and says so by name', () => {
  assert.deepEqual(DECLARATION.consumes, []);
  assert.deepEqual(DECLARATION.sends, []);
  assert.deepEqual(DECLARATION.withheld, []);
  assert.equal(typeof DECLARATION.silent_face_note, 'string');
  assert.ok(DECLARATION.silent_face_note.length > 10);
});

test('C-7 negative control: a non-empty consumes list is a thing this test would catch', () => {
  const rigged = { ...DECLARATION, consumes: ['get_transformations'] };
  assert.notDeepEqual(rigged.consumes, []);
});

test('C-1/C-2: an empty declaration has nothing to send and nothing to withhold, which is the whole of what C-1/C-2 ask of a face with no methods', () => {
  assert.equal(DECLARATION.consumes.length, 0);
  assert.equal(DECLARATION.sends.length, 0);
  assert.equal(DECLARATION.withheld.length, 0);
});

test('C-3: the face declares what it does not draw, with a reason for each', () => {
  assert.ok(DECLARATION.undrawn.length >= 5, 'too few declared omissions to be a real declaration');
  for (const entry of DECLARATION.undrawn) {
    assert.equal(typeof entry.what, 'string');
    assert.ok(typeof entry.why === 'string' && entry.why.length > 20, `${entry.what} is undrawn without a reason`);
  }
  assert.equal(DECLARATION.rows.draws, true);
  assert.equal(DECLARATION.rows.reports_denominator, true);
});

test('C-4/C-5: marks are declared, and one meaning never carries two marks', () => {
  assert.ok(DECLARATION.marks.length >= 2);
  const byMeaning = new Map();
  for (const mark of DECLARATION.marks) {
    assert.equal(typeof mark.mark, 'string');
    assert.equal(typeof mark.means, 'string');
    const seen = byMeaning.get(mark.means);
    assert.equal(seen, undefined, `two marks for ${mark.means}: ${seen} and ${mark.mark}`);
    byMeaning.set(mark.means, mark.mark);
  }
});

test('C-6: the position in the rail is declared, and so is the reason for it', () => {
  assert.equal(typeof DECLARATION.order.position, 'number');
  assert.ok(DECLARATION.order.reason.length > 20, 'a position without a reason is a lexical accident with a number on it');
  assert.ok(DECLARATION.rows.order.length > 0);
  assert.ok(DECLARATION.rows.order_reason.length > 20);
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

test('Owner #348 (2): what a row offers is declared once, and an offer is not an act', () => {
  // ACTS names a method of the server and stays empty (C-7). OFFERS names a value
  // already on this screen -- the gutter draws one member, the right-click menu draws
  // all of them, and neither invents one.
  assert.deepEqual(DECLARATION.acts, []);
  assert.ok(DECLARATION.offers.length >= 3, 'too few offers for a menu to be worth opening');
  const ids = DECLARATION.offers.map((offer) => offer.id);
  assert.equal(new Set(ids).size, ids.length, 'two offers under one id');
  for (const offer of DECLARATION.offers) {
    assert.ok(offer.label.length > 0 && offer.menu.length > offer.label.length, `${offer.id}: the menu line is not the fuller of the two`);
    assert.ok(offer.of.length > 20, `${offer.id} does not say what it would hand over`);
    // An offer that a record can fail to answer says what it will tell the reader
    // then. The two every method-bearing record can always answer do not need one.
    if (offer.why !== undefined) assert.ok(offer.why.length > 20, `${offer.id} has a reason too short to be one`);
  }
  assert.equal(DECLARATION.offers.filter((offer) => offer.gutter === true).length, 1, 'the row gutter holds exactly one control');
});

test('Owner #348 (2): the retired control is still accounted for, as a declared omission', () => {
  const entry = DECLARATION.undrawn.find((undrawn) => undrawn.what === 'a way through to the face that reads this record');
  assert.ok(entry, 'the reason a reader cannot jump to another screen is nowhere');
  assert.ok(entry.why.length > 100, 'the reason was shortened into something that no longer explains itself');
});

test('no mark describes itself to a reader in vocabulary only this codebase holds', () => {
  // These sentences are drawn on the screen, in the legend. Two of them ended in a
  // parenthesis naming a directive number, two function names and another face's
  // directory.
  for (const mark of DECLARATION.marks) {
    assert.equal(/\(\)|SS\d|req\/|faces\//.test(mark.from), false, `${mark.mark} explains itself with something only this codebase can look up: ${mark.from}`);
  }
});

test('the face states one question and does not grow a second', () => {
  assert.equal(typeof DECLARATION.question, 'string');
  assert.equal(DECLARATION.question.split('?').length <= 2, true);
  assert.equal(DECLARATION.id, 'notice');
});
