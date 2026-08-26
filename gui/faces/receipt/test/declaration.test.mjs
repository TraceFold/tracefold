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

test('the reference tree\'s declared method count is met exactly, not deviated from', () => {
  // req/03 §3-1 declares 2 for this face's reference row (transformations / receipt).
  // Unlike faces/held (5 of 6), this declaration matches the reference's count -- a
  // match is still a claim that needs a test, so it is asserted here rather than
  // left implicit.
  assert.equal(DECLARATION.consumes.length, 2);
  assert.deepEqual([...DECLARATION.consumes].sort(), ['get_receipts_tid', 'get_transformations_id']);
});

test('C-2: this face sends no acts and withholds nothing -- a read-only screen states that structurally', () => {
  assert.deepEqual(DECLARATION.acts, []);
  assert.deepEqual(DECLARATION.withheld, []);
  assert.deepEqual([...DECLARATION.sends].sort(), ['get_receipts_tid', 'get_transformations_id']);
  for (const name of DECLARATION.sends) assert.ok(DECLARATION.consumes.includes(name));
});

test('C-3: the face declares what it does not draw, with a reason for each', () => {
  assert.ok(DECLARATION.undrawn.length >= 3, 'too few declared omissions to be a real declaration');
  for (const entry of DECLARATION.undrawn) {
    assert.equal(typeof entry.what, 'string');
    assert.ok(typeof entry.why === 'string' && entry.why.length > 20, `${entry.what} is undrawn without a reason`);
  }
  assert.equal(DECLARATION.rows.draws, true);
  assert.equal(DECLARATION.rows.reports_denominator, false, 'a single-record screen states a claim count, not a row count');
});

test('C-3: an unverified seal is named as never-drawn, unconditionally', () => {
  const sealEntry = DECLARATION.undrawn.find((e) => e.what.includes('verified seal'));
  assert.ok(sealEntry, 'the declaration does not name "no sealed:true without a verifier" as an omission');
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
  // 9 data marks (unchanged, honestly fewer than the reference's 11 -- see the
  // face header's own account) + 2 chrome marks (structure/fold-shut,
  // structure/fold-open) the SS657 retrofit lane (round 1) added for the
  // why/legend controls' own fold glyph, which numerically equalled the
  // reference's 11 by coincidence, not because anything was copied. Retrofit
  // round 2 (req/768 AC-7) adds one more: standing/none, the reversibility
  // chip's always-not-observable mark on this screen -- 12 now, breaking the
  // coincidence, which is itself evidence nothing here is chasing a target
  // number. This test pins the honest derivation, not a number to hit.
  assert.equal(DECLARATION.marks.length, 12);
  const dataMarks = DECLARATION.marks.filter((m) => !['structure/fold-shut', 'structure/fold-open', 'standing/none'].includes(m.mark));
  assert.equal(dataMarks.length, 9, 'the data marks this screen draws about the delta itself are still 9, fewer than the reference\'s 11');
});

test('C-5: receipt\'s marks agree with faces/ledger\'s and faces/held\'s marks for every meaning shared', async () => {
  const { DECLARATION: LEDGER } = await import('../../ledger/declaration.mjs');
  const { DECLARATION: HELD } = await import('../../held/declaration.mjs');
  const byMeaning = new Map([...LEDGER.marks, ...HELD.marks].map((m) => [m.means, m.mark]));
  for (const mark of DECLARATION.marks) {
    if (byMeaning.has(mark.means)) assert.equal(mark.mark, byMeaning.get(mark.means), `${mark.means}: receipt says ${mark.mark}, elsewhere says ${byMeaning.get(mark.means)}`);
  }
});

test('C-6: the position in the rail is declared, and so is the reason for it', () => {
  assert.equal(typeof DECLARATION.order.position, 'number');
  assert.equal(DECLARATION.order.position, 3, 'receipt sits directly after held (position 2)');
  assert.ok(DECLARATION.order.reason.length > 20, 'a position without a reason is a lexical accident with a number on it');
  assert.equal(DECLARATION.rows.order, 'single-record');
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

test('C-7 is not this face: the face with no methods is named, not assumed', () => {
  assert.ok(DECLARATION.consumes.length > 0);
  assert.equal(typeof DECLARATION.silent_face, 'string');
  assert.ok(DECLARATION.silent_face.length > 10);
});

test('the face states one question and does not grow a second', () => {
  assert.equal(typeof DECLARATION.question, 'string');
  assert.equal(DECLARATION.question.split('?').length <= 2, true);
  assert.equal(DECLARATION.id, 'receipt');
});
