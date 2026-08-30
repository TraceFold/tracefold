// SPDX-License-Identifier: Apache-2.0
// The four tables the shell holds, and the census of inverses over this project's own act
// set -- measured here rather than carried over from the measurement of another system's
// eighteen verbs, which was a transcription and not a reading.

import test from 'node:test';
import assert from 'node:assert/strict';

import { ACTS, VERBS, TRACK, LAYER, OPERATION, CRUD_ELSEWHERE } from '../kernel/acts.mjs';
import { KEYMAP, HISTORY_VERBS, VIEW_VERBS, composing, match } from '../kernel/keys.mjs';
import { SLOTS, SLOT_NAMES, FILLS } from '../kernel/slots.mjs';
import { LAYERS, LAYER_NUMBERS, Dismiss } from '../kernel/dismiss.mjs';
import { census, cost } from '../tools/inverse_census.mjs';
import { ROUTES } from '../demo/routes.gen.mjs';

test('every act declares an invert, and the declaration matches its track', () => {
  assert.ok(VERBS.length >= 15, `only ${VERBS.length} acts are registered`);
  for (const verb of VERBS) {
    const act = ACTS[verb];
    assert.ok('invert' in act, `${verb} has no invert field`);
    if (act.track === TRACK.SHELL) assert.equal(act.invert, 'captured-closure', verb);
    else assert.equal(act.invert, null, verb);
  }
});

test('a face-track act names a membrane method that exists, and keeps no inverse here', () => {
  const names = new Set(ROUTES.map((r) => r.name));
  const delegated = VERBS.filter((v) => ACTS[v].track === TRACK.FACE);
  assert.ok(delegated.length >= 1, 'no act is on the face track, so the two-track ruling is untested');
  for (const verb of delegated) {
    const act = ACTS[verb];
    assert.ok(names.has(act.delegate), `${verb} delegates to "${act.delegate}", which the membrane does not serve`);
    assert.equal(act.invert, null, `${verb} would give the shell a second way to undo a committed thing`);
    assert.match(act.said, /membrane/);
  }
});

test('the shell names the engine\'s "verify" step, not only its "undo"', () => {
  // glovrex req/822_c6 §3 measured this live against a real engine: a commit sent
  // for a candidate the engine had not yet verified came back refused with
  // INVALID_STATE ("Candidateはverify前"). Before this test, `record:undo` was the
  // only face-track act this table held, so a reader of ACTS alone had no way to
  // learn that a step comes before commit, let alone which one -- the shell's own
  // vocabulary had no word for it, which is the defect this test reproduces and
  // this file's next act closes. The control that presses it belongs to the face
  // that draws the held screen (a separate build, out of this file's reach); what
  // this table can hold is the verb itself: on the face track, naming the real
  // membrane method, with no inverse kept here -- the same three facts it already
  // states for `record:undo`'s own delegate.
  assert.ok('candidate:verify' in ACTS, 'the shell has no word for the engine\'s verify step, so nothing reading this table can learn it comes before commit');
  const act = ACTS['candidate:verify'];
  assert.equal(act.track, TRACK.FACE, 'verify moves the engine\'s own state, not the shell\'s layout');
  assert.equal(act.invert, null, 'verify would give the shell a second way to hold what is the engine\'s decision');
  assert.equal(act.delegate, 'post_candidates_id_verify');
  const names = new Set(ROUTES.map((r) => r.name));
  assert.ok(names.has(act.delegate), `candidate:verify delegates to "${act.delegate}", which the membrane does not serve`);
});

test('the census of inverses over this act set, measured not transcribed', () => {
  const found = census();
  assert.equal(found.total, VERBS.length);
  assert.equal(found.impossible, 0, 'an act whose inverse cannot exist at all would break the design, not the count');
  assert.equal(found.unmarked, 0, `${found.unmarked} inverses do not say what they hold, so the census is guessing about them`);
  assert.equal(found.unreachable, 0, 'the census could not build every act, so its shares are over a smaller set than it claims');
  assert.equal(found.delegated + found.pure + found.captured, found.total);
  assert.ok(found.captured > 0, 'nothing captures anything, so the closure design is doing no work');
  process.stdout.write(`# inverse census: ${JSON.stringify({ ...found, rows: undefined })}\n`);
});

test('history costs less than one copy of the state per step, by count', () => {
  const spent = cost();
  assert.ok(spent.moved > 60, `only ${spent.moved} steps moved anything`);
  assert.ok(spent.objectsHeldOpen < spent.snapshotObjects, 'the inverses retain more than snapshots would have');
  assert.ok(spent.timesCheaper > 2, `only ${spent.timesCheaper} times cheaper than snapshots; the design is not paying for itself`);
  process.stdout.write(`# history cost: ${JSON.stringify({ ...spent, said: undefined })}\n`);
});

test('all four layers take all four operations, or say where the operation lives', () => {
  const grid = new Map();
  for (const verb of VERBS) {
    const act = ACTS[verb];
    const cell = `${act.layer}/${act.operation}`;
    grid.set(cell, (grid.get(cell) ?? 0) + 1);
  }
  const declared = new Set(CRUD_ELSEWHERE.map((e) => `${e.layer}/${e.operation}`));
  const missing = [];
  for (const layer of LAYER) {
    for (const operation of OPERATION) {
      const cell = `${layer}/${operation}`;
      if ((grid.get(cell) ?? 0) >= 1) continue;
      if (declared.has(cell)) continue;
      missing.push(cell);
    }
  }
  assert.deepEqual(missing, [], `no act and no declaration for: ${missing.join(', ')}`);
  for (const away of CRUD_ELSEWHERE) {
    assert.match(away.where, /^kernel\/[a-z]+\.mjs$/, `${away.layer}/${away.operation} does not say where it lives`);
    assert.ok(away.said.length > 20, `${away.layer}/${away.operation} gives no reason`);
    assert.ok(!grid.has(`${away.layer}/${away.operation}`), `${away.layer}/${away.operation} is declared elsewhere and also registered here`);
  }
  assert.ok(CRUD_ELSEWHERE.length <= 1, `${CRUD_ELSEWHERE.length} cells are declared away; the grid is becoming a list of excuses`);
});

test('the keyboard table names only verbs that exist', () => {
  assert.equal(HISTORY_VERBS.length, 2);
  // Three kinds of verb, all enumerated: an act, a history verb, or a view verb -- one
  // that changes what is shown and nothing about what is true, so it never lands on the
  // line (req/811 §8-5's palette is the first). There is no fourth kind, and the list is
  // asserted small so the exception cannot quietly become the rule.
  assert.ok(VIEW_VERBS.length <= 3, `${VIEW_VERBS.length} verbs claim to change only what is shown`);
  for (const entry of KEYMAP) {
    const known = entry.verb in ACTS || HISTORY_VERBS.includes(entry.verb) || VIEW_VERBS.includes(entry.verb);
    assert.ok(known, `the keyboard names "${entry.verb}"`);
    assert.ok(entry.said.length > 4, `${entry.chord} has no stated meaning`);
  }
  const chords = KEYMAP.map((e) => e.chord);
  assert.equal(new Set(chords).size, chords.length, 'a chord is bound twice');
});

test('a chord arriving mid-composition is not a chord', () => {
  const event = { ctrlKey: true, shiftKey: false, altKey: false, key: 'z', code: 'KeyZ', isComposing: true };
  assert.equal(composing(event), true);
  assert.equal(match(event), null);
  assert.equal(match({ ...event, isComposing: false }).verb, 'undo');
  assert.equal(match({ ...event, isComposing: false, keyCode: 229 }), null, 'the older composition signal is ignored');
});

test('the slot registry says where each place is filled from', () => {
  assert.equal(new Set(SLOT_NAMES).size, SLOT_NAMES.length);
  for (const s of SLOTS) assert.ok(FILLS.includes(s.fills));
  assert.ok(SLOT_NAMES.includes('rail'));
  assert.ok(SLOT_NAMES.includes('stage'));
});

test('dismiss layers are numbered here, once each, and answered in order', () => {
  assert.deepEqual(LAYER_NUMBERS, [...LAYER_NUMBERS].sort((a, b) => a - b));
  const dismiss = new Dismiss();
  const order = [];
  const release = dismiss.hold(LAYERS.OVERLAY, () => { order.push('overlay'); return true; });
  dismiss.hold(LAYERS.MENU, () => { order.push('menu'); return true; });
  assert.throws(() => dismiss.hold(LAYERS.MENU, () => true), /already held/);
  assert.throws(() => dismiss.hold(99, () => true), /not a dismiss layer/);
  assert.equal(dismiss.dismiss(), LAYERS.MENU);
  assert.deepEqual(order, ['menu'], 'more than the shallowest layer was dismissed');
  release();
  assert.deepEqual(dismiss.held, [LAYERS.MENU]);
});
