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

test('the reference tree\'s sixth consumed method ("subscribe") is honestly absent, not silently matched', () => {
  // req/03 §3-1 declares 6 for this face's reference row; this declaration carries
  // 5. The gap is stated here as a fact about the declaration, not left to be
  // noticed by someone counting -- and it must not become 6 by inventing a method
  // name that fails the membrane check above.
  assert.equal(DECLARATION.consumes.length, 5);
  assert.equal(DECLARATION.consumes.includes('subscribe'), false);
});

test('C-2: the declaration is an upper bound -- what is declared and withheld is stated, not hidden', () => {
  const withheldNames = DECLARATION.withheld.map((w) => w.method);
  for (const name of withheldNames) {
    assert.ok(DECLARATION.consumes.includes(name), `${name} is withheld but not declared`);
  }
  assert.equal(DECLARATION.withheld.length, 2, 'held declares two withholdings: escalate (unread body) and undo (nothing to target)');
  for (const entry of DECLARATION.withheld) {
    assert.ok(typeof entry.why === 'string' && entry.why.length > 20, `${entry.method} is withheld without a reason`);
  }
  const sent = new Set(DECLARATION.sends);
  for (const name of withheldNames) assert.equal(sent.has(name), false);
  for (const name of DECLARATION.sends) assert.ok(DECLARATION.consumes.includes(name));
});

test('C-2: undo is declared reachable but is not the same withholding reason as escalate', () => {
  const undo = DECLARATION.withheld.find((w) => w.act === 'undo');
  const escalate = DECLARATION.withheld.find((w) => w.act === 'escalate');
  assert.ok(undo && escalate);
  assert.notEqual(undo.why, escalate.why, 'reachable-but-not-offered and body-unread are different facts and must read differently');
});

test('the gates are declared, one per act, and no gate invents a verb', () => {
  const acts = new Map(DECLARATION.acts.map((a) => [a.act, a]));
  assert.equal(DECLARATION.gates.length, DECLARATION.acts.length);
  const governed = new Set();
  for (const gate of DECLARATION.gates) {
    assert.equal(typeof gate.gate, 'string');
    assert.ok(gate.name.length > 0, `${gate.gate} has no name to draw`);
    assert.ok(acts.has(gate.act), `${gate.gate} names an act this face does not declare: ${gate.act}`);
    assert.equal(governed.has(gate.act), false, `${gate.act} is governed by two gates`);
    governed.add(gate.act);
    assert.ok(gate.asks.length > 20, `${gate.gate} does not state what it asks`);
    assert.ok(Array.isArray(gate.reads) && gate.reads.length > 0, `${gate.gate} does not state what its answer is read from`);
    for (const read of gate.reads) assert.ok(typeof read === 'string' && read.length > 0);
  }
});

test('the terminal act is the last gate, because the ladder is climbed and not scanned', () => {
  const last = DECLARATION.gates[DECLARATION.gates.length - 1];
  assert.equal(last.act, 'commit');
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

test('C-3: a sealed record is named as never-drawn, unconditionally', () => {
  const sealEntry = DECLARATION.undrawn.find((e) => e.what.includes('sealed'));
  assert.ok(sealEntry, 'the declaration does not name "nothing here is ever sealed" as an omission');
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

test('C-5: held\'s marks agree with faces/ledger\'s marks for every meaning both faces draw', async () => {
  // The two faces share receipt-row.mjs and glyph-sheet.mjs. If held invented a
  // second mark for a meaning ledger already owns, C-5 would be violated across
  // the app even though each face's own declaration test passes in isolation.
  const { DECLARATION: LEDGER } = await import('../../ledger/declaration.mjs');
  const byMeaning = new Map(LEDGER.marks.map((m) => [m.means, m.mark]));
  for (const mark of DECLARATION.marks) {
    if (byMeaning.has(mark.means)) assert.equal(mark.mark, byMeaning.get(mark.means), `${mark.means}: held says ${mark.mark}, ledger says ${byMeaning.get(mark.means)}`);
  }
});

test('C-6: the position in the rail is declared, and so is the reason for it', () => {
  assert.equal(typeof DECLARATION.order.position, 'number');
  assert.equal(DECLARATION.order.position, 2, 'held sits directly after ledger (position 1)');
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

test('C-7 is not this face: the face with no methods is named, not assumed', () => {
  assert.ok(DECLARATION.consumes.length > 0);
  assert.equal(typeof DECLARATION.silent_face, 'string');
  assert.ok(DECLARATION.silent_face.length > 10);
});

test('the face states one question and does not grow a second', () => {
  assert.equal(typeof DECLARATION.question, 'string');
  assert.equal(DECLARATION.question.split('?').length <= 2, true);
  assert.equal(DECLARATION.id, 'held');
});
