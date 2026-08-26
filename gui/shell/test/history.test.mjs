// SPDX-License-Identifier: Apache-2.0
// Two hundred arbitrary acts, undone to the beginning and redone to the end, with the
// digest as the only witness at either edge.
//
// This is the requirement the whole design exists to meet, so it is also the one that has
// to be attacked: a walk that only ever adds panes proves that adding a pane is
// reversible and nothing else. The walk therefore draws from every verb the registry
// holds, including the ones that remove things, and counts what it actually moved.

import test from 'node:test';
import assert from 'node:assert/strict';

import { ShellState, OUTCOME, HISTORY_DEPTH } from '../kernel/state.mjs';
import { readManifest } from '../kernel/manifest.mjs';
import { openingState } from '../kernel/shell.mjs';
import { MANIFEST } from '../demo/manifest.gen.mjs';
import { seeded, walkOnce, WALK_SEED, WALK_STEPS } from '../tools/walk.mjs';

const opened = () => {
  const read = readManifest(MANIFEST);
  return { read, shell: new ShellState(read, openingState(read)) };
};

test('two hundred acts undo to the opening digest and redo to the last one', () => {
  const { read, shell } = opened();
  const start = shell.digest;
  const random = seeded(WALK_SEED);
  const seen = new Map();
  let moved = 0;
  for (let i = 0; i < WALK_STEPS; i += 1) {
    const row = walkOnce((verb, args) => shell.perform(verb, args), read, shell.state, random);
    seen.set(row.outcome, (seen.get(row.outcome) ?? 0) + 1);
    if (row.outcome === OUTCOME.MOVED) moved += 1;
  }
  const end = shell.digest;
  assert.ok(moved > 60, `only ${moved} of ${WALK_STEPS} acts moved the shell; the walk is not exercising it`);
  assert.ok(seen.get(OUTCOME.REFUSED) > 0, 'the walk never hit a refusal, so refusals are untested here');
  assert.notEqual(end, start);

  for (let i = 0; i < moved; i += 1) shell.undo();
  assert.equal(shell.digest, start, 'undoing every act did not arrive at the opening state');
  assert.equal(shell.depth.past, 0);

  for (let i = 0; i < moved; i += 1) shell.redo();
  assert.equal(shell.digest, end, 'redoing every act did not arrive where the walk left off');
  assert.equal(shell.depth.ahead, 0);
});

test('every step is individually reversible, not just the sequence as a whole', () => {
  const { read, shell } = opened();
  const random = seeded(11);
  for (let i = 0; i < 120; i += 1) {
    const before = shell.digest;
    const row = walkOnce((verb, args) => shell.perform(verb, args), read, shell.state, random);
    if (row.outcome !== OUTCOME.MOVED) {
      assert.equal(shell.digest, before, `a ${row.outcome} act moved the shell at step ${i}`);
      continue;
    }
    shell.undo();
    assert.equal(shell.digest, before, `step ${i} (${row.verb}) did not undo cleanly`);
    shell.redo();
    assert.equal(shell.digest, row.after, `step ${i} (${row.verb}) did not redo cleanly`);
  }
});

test('the receipt carries one row per act, each with the two digests', () => {
  const { read, shell } = opened();
  const first = shell.perform('pane:divide', { index: 0, path: [], axis: 'row' });
  const second = shell.undo();
  const rows = shell.receipt;
  assert.equal(rows.length, 2);
  assert.equal(rows[0].verb, 'pane:divide');
  assert.equal(rows[0].before, first.before);
  assert.equal(rows[0].after, first.after);
  assert.equal(rows[1].verb, 'undo(pane:divide)');
  assert.equal(rows[1].after, second.after);
  assert.equal(rows[1].after, rows[0].before, 'undo did not return to the digest before the act');
  for (const row of rows) {
    assert.match(row.before, /^blake3:/);
    assert.match(row.after, /^blake3:/);
  }
});

test('history carries inverses, never states', () => {
  const { read, shell } = opened();
  const random = seeded(3);
  for (let i = 0; i < 40; i += 1) walkOnce((verb, args) => shell.perform(verb, args), read, shell.state, random);
  for (const verb of shell.carried) assert.equal(typeof verb, 'string');
  assert.ok(Object.isFrozen(shell.state), 'the state is not frozen, so something could edit it in place');
  assert.throws(() => { shell.state.theme = 'dark'; }, TypeError);
  assert.ok(HISTORY_DEPTH > WALK_STEPS, 'the walk is longer than the history it is testing');
  assert.equal(shell.depth.dropped, 0);
});

test('a refusal is an answer with a reason, and it does not move anything', () => {
  const { shell } = opened();
  const before = shell.digest;
  const row = shell.perform('pane:drop', { index: 0, path: [] });
  assert.equal(row.outcome, OUTCOME.REFUSED);
  assert.match(row.said, /only pane/);
  assert.equal(shell.digest, before);

  const nowhere = shell.perform('there:is:no:such:verb', {});
  assert.equal(nowhere.outcome, OUTCOME.REFUSED);
  assert.match(nowhere.said, /no act called/);

  const elsewhere = shell.perform('record:undo', {});
  assert.equal(elsewhere.outcome, OUTCOME.ELSEWHERE);
  assert.match(elsewhere.said, /membrane/);
  assert.equal(shell.digest, before);
  assert.equal(shell.depth.past, 0, 'a delegated act was pushed onto the shell\'s own history');
});

// [B5] One gesture is one entry.
//
// Measured through CDP against the live window before this existed: a fourteen-move drag
// of the right dock's sash left fourteen entries behind it, because the sash performs one
// act per pointermove and every act was its own step. The count is not fourteen in
// principle -- it is however many times the pointer moved -- so the assertion below is on
// the shape (one gesture, one entry, and undo arrives where the gesture began) rather
// than on any particular number of moves.
const GESTURE = 'one-drag';

test('acts carrying one gesture collapse to one history entry, and undo returns to where it began', () => {
  const { shell } = opened();
  const start = shell.digest;
  const sizes = [230, 240, 250, 260, 270, 280, 290];
  for (const size of sizes) {
    const row = shell.perform('dock:size', { index: 0, side: 'right', size }, { gesture: GESTURE });
    assert.equal(row.outcome, OUTCOME.MOVED, `sizing to ${size} did not move the shell`);
  }
  assert.equal(shell.depth.past, 1, `${sizes.length} acts of one gesture left ${shell.depth.past} entries behind`);
  assert.equal(shell.receipt.length, sizes.length, 'the receipt lost acts; only the history coalesces');
  assert.notEqual(shell.digest, start);

  shell.undo();
  assert.equal(shell.digest, start, 'one undo did not return to the state the gesture began from');
  assert.equal(shell.depth.past, 0);

  shell.redo();
  assert.equal(shell.depth.past, 1);
  // req/884: the sigil is `+` when the dock is open and `-` when it is shut, and this
  // test is about gesture COALESCING, not about the arrival default. It asserted `+290`
  // and so began failing the moment docks opened shut -- an assertion pinned to a fact
  // it was not written to check. It pins the size, which is what "arrived at the size it
  // ended on" actually means, and now says nothing about whether the dock is open.
  assert.match(shell.line, /right:[-+]290\[/, 'redoing the gesture did not arrive at the size it ended on');
});

test('a second gesture is a second entry, and an act naming no gesture is always its own', () => {
  const { shell } = opened();
  shell.perform('dock:size', { index: 0, side: 'right', size: 230 }, { gesture: 'first' });
  shell.perform('dock:size', { index: 0, side: 'right', size: 240 }, { gesture: 'first' });
  assert.equal(shell.depth.past, 1);
  shell.perform('dock:size', { index: 0, side: 'right', size: 250 }, { gesture: 'second' });
  assert.equal(shell.depth.past, 2, 'a new gesture folded into the one before it');
  shell.perform('dock:size', { index: 0, side: 'right', size: 260 });
  shell.perform('dock:size', { index: 0, side: 'right', size: 270 });
  assert.equal(shell.depth.past, 4, 'acts naming no gesture were coalesced');
  assert.deepEqual(shell.carried, ['dock:size', 'dock:size', 'dock:size', 'dock:size']);
});

test('a coalesced entry still holds what its first act captured, so the census can read it', () => {
  const { shell } = opened();
  const alone = new ShellState(readManifest(MANIFEST), openingState(readManifest(MANIFEST)));
  alone.perform('dock:size', { index: 0, side: 'right', size: 230 });
  const held = alone.heldByHistory[0];
  shell.perform('dock:size', { index: 0, side: 'right', size: 230 }, { gesture: GESTURE });
  shell.perform('dock:size', { index: 0, side: 'right', size: 300 }, { gesture: GESTURE });
  assert.deepEqual(shell.heldByHistory[0], held, 'the merged inverse holds the later act\'s capture, not the first\'s');
});

test('undoing past the beginning says so rather than throwing or looping', () => {
  const { shell } = opened();
  const row = shell.undo();
  assert.equal(row.outcome, OUTCOME.UNCHANGED);
  assert.match(row.said, /nothing behind/);
  assert.equal(shell.redo().outcome, OUTCOME.UNCHANGED);
});
