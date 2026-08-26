// SPDX-License-Identifier: Apache-2.0
// The declaration, checked against the two things it can be wrong about: the server
// it names methods on, and the files it names tests in -- plus this face's own
// declared-not-wired slot/address fields, checked against what the rest of this
// tree actually does today.

import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { DECLARATION } from '../declaration.mjs';
import { tableRows } from '../../../membrane/src/index.mjs';
import { DECLARATION as LEDGER } from '../../ledger/declaration.mjs';
import { DECLARATION as HELD } from '../../held/declaration.mjs';
import { DECLARATION as RECEIPT } from '../../receipt/declaration.mjs';
import { DECLARATION as GRAPH } from '../../graph/declaration.mjs';

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

test('there is no reference row for this face -- ui_proto never had an atlas/main face, so this declaration has no reference count to be honestly fewer than', () => {
  assert.deepEqual([...DECLARATION.consumes], ['get_transformations']);
  assert.equal(DECLARATION.consumes.length, 1);
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
  assert.equal(DECLARATION.rows.reports_denominator, true);
});

test('C-3: chain edges and lens modes are both named as never-drawn, unconditionally, with the face that does draw them (or the reason none does) stated', () => {
  const edges = DECLARATION.undrawn.find((e) => e.what.includes('chain edge'));
  const lens = DECLARATION.undrawn.find((e) => e.what.includes('graph or timeline lens'));
  const switcher = DECLARATION.undrawn.find((e) => e.what.includes('link to another face'));
  assert.ok(edges, 'the declaration does not name "chain edge" as an omission');
  assert.ok(edges.why.includes('graph'), 'the chain-edge omission does not point at the face that does draw edges');
  assert.ok(lens, 'the declaration does not name the graph/timeline lens modes as an omission');
  assert.ok(switcher, 'the declaration does not name the face-switcher as an omission');
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

test('one mark on this face is new and belongs to no other face: structure/subject', () => {
  const mark = DECLARATION.marks.find((m) => m.means === 'structure.subject');
  assert.ok(mark, 'structure.subject is not declared');
  assert.equal(mark.mark, 'structure/subject');
});

test('two marks on this face are reused by content for the first time: structure/fold-shut and structure/fold-open', () => {
  const shut = DECLARATION.marks.find((m) => m.means === 'structure.fold.shut');
  const open = DECLARATION.marks.find((m) => m.means === 'structure.fold.open');
  assert.ok(shut, 'structure.fold.shut is not declared');
  assert.ok(open, 'structure.fold.open is not declared');
});

test('C-5: atlas\'s marks agree with faces/ledger\'s, faces/held\'s, faces/receipt\'s and faces/graph\'s marks for every meaning shared', () => {
  const byMeaning = new Map([...LEDGER.marks, ...HELD.marks, ...RECEIPT.marks, ...GRAPH.marks].map((m) => [m.means, m.mark]));
  for (const mark of DECLARATION.marks) {
    if (byMeaning.has(mark.means)) assert.equal(mark.mark, byMeaning.get(mark.means), `${mark.means}: atlas says ${mark.mark}, elsewhere says ${byMeaning.get(mark.means)}`);
  }
});

test('C-6: the position in build order is declared, and so is the reason it is not the same claim as default_slot', () => {
  assert.equal(typeof DECLARATION.order.position, 'number');
  assert.equal(DECLARATION.order.position, 6, 'atlas is F-6, the sixth face built');
  assert.ok(DECLARATION.order.reason.length > 20);
  assert.equal(DECLARATION.rows.order, 'by-sequence');
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
  assert.equal(DECLARATION.id, 'atlas');
});

// -- req/08 SS2-3/SS2-4's declared-not-wired slot/address fields, checked honestly --

test('default_slot is declared as "primary", and the gap between declaring it and any shell reading it is stated in the same breath', () => {
  assert.equal(DECLARATION.default_slot, 'primary');
  assert.equal(typeof DECLARATION.slot_wiring, 'string');
  assert.ok(DECLARATION.slot_wiring.length > 20);
});

test('no other face in this tree declares default_slot: "primary" -- if two did, app req/08 AC-S1\'s "exactly one" rule would already be broken and this test would catch it before any shell-side gate exists to', () => {
  const others = [LEDGER, HELD, RECEIPT, GRAPH];
  for (const other of others) assert.notEqual(other.default_slot, 'primary', `${other.id} also declares default_slot: "primary"`);
});

test('emits and handles are declared empty, and each carries a reason honestly stating why', () => {
  assert.deepEqual([...DECLARATION.emits], []);
  assert.deepEqual([...DECLARATION.handles], []);
  assert.equal(typeof DECLARATION.emits_reason, 'string');
  assert.ok(DECLARATION.emits_reason.length > 20);
  assert.equal(typeof DECLARATION.handles_reason, 'string');
  assert.ok(DECLARATION.handles_reason.length > 20);
});

test('the handles_reason\'s premise is checked directly: no other face in this tree declares an emits field at all today', () => {
  const others = [LEDGER, HELD, RECEIPT, GRAPH];
  for (const other of others) assert.equal(other.emits, undefined, `${other.id} already declares emits -- HANDLES_REASON's premise is stale`);
});
