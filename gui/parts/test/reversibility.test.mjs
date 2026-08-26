// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { reversalOf, REVERSAL_STATES, REVERSAL_MESSAGES } from '../src/reversibility.mjs';

const SETTLED = { id: 't-001', lifecycle: 'settled' };
const CHILD = { id: 't-002', lifecycle: 'settled', childOf: 't-001' };
const UNRELATED = { id: 't-003', lifecycle: 'settled', childOf: 't-099' };
const HELD = { id: 'c-001', lifecycle: 'held' };

test('the three states are exactly the ones this module ever answers with', () => {
  assert.deepEqual(REVERSAL_STATES, ['reversed', 'not-observable', 'not-committed']);
});

test('red-first: a held row is always not-committed, siblings or not -- nothing has happened yet', () => {
  const fact = reversalOf(HELD, [SETTLED, CHILD]);
  assert.equal(fact.state, 'not-committed');
  assert.equal(fact.by, null);
  assert.deepEqual(fact.mark, ['standing', 'held']);
  assert.equal(fact.why, REVERSAL_MESSAGES['not-committed']);
});

test('a settled row a sibling names as its predecessor (childOf) reads as reversed -- a derived fact, no live call', () => {
  const fact = reversalOf(SETTLED, [SETTLED, CHILD, UNRELATED]);
  assert.equal(fact.state, 'reversed');
  assert.equal(fact.by, 't-002');
  assert.deepEqual(fact.mark, ['standing', 'reversed']);
  assert.match(fact.why, /t-002/);
});

test('a settled row with no reversing sibling in the same read is not-observable, never guessed at as still-invertible', () => {
  const fact = reversalOf(SETTLED, [SETTLED, UNRELATED]);
  assert.equal(fact.state, 'not-observable');
  assert.equal(fact.by, null);
  assert.deepEqual(fact.mark, ['standing', 'none']);
  assert.match(fact.why, /membrane\/wire-fields\.json/);
});

test('an empty sibling list (faces/receipt has no list context at all) is always not-observable, honestly, not as a special case', () => {
  const fact = reversalOf(SETTLED, []);
  assert.equal(fact.state, 'not-observable');
  const noArg = reversalOf(SETTLED);
  assert.equal(noArg.state, 'not-observable');
});

test('a record with no identity cannot be looked for as anyone\'s predecessor -- not-observable, not a throw', () => {
  assert.equal(reversalOf(null, [CHILD]).state, 'not-observable');
  assert.equal(reversalOf({ lifecycle: 'settled' }, [CHILD]).state, 'not-observable');
});

test('a record never counts itself as its own reverser, even if childOf were somehow self-referential', () => {
  const selfChild = { id: 't-010', lifecycle: 'settled', childOf: 't-010' };
  const fact = reversalOf(selfChild, [selfChild]);
  assert.equal(fact.state, 'not-observable', 'the only candidate sibling is the record itself, which this function excludes');
});

test('red-first: no speculative call -- reversalOf takes no port/caller argument at all, so it cannot reach the wire', () => {
  // .length counts only parameters before the first default value (siblings = []
  // is defaulted), so the declared arity is 1 -- record, and nothing else. There
  // is no third parameter this function could accept a port or caller through.
  assert.equal(reversalOf.length, 1, 'the function signature itself has no room for a port; a caller cannot pass one in');
});
