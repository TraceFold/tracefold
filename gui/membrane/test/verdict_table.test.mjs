// SPDX-License-Identifier: Apache-2.0
// The verdict vocabulary against the crate that owns it (req/972 §1-E #11, R-972-9).
//
// Same shape as route_table.test.mjs on the same file's model: the crate is read
// once per run, not trusted from a comment, and a tree that cannot see the crate
// says so instead of pretending the comparison held.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { VERDICT_KINDS } from '../src/wire.mjs';
import {
  extractFromCrate,
  crateAvailable,
  crateLibPath,
  CRATE_ABSENT_REASON,
} from '../tools/verdict_table_from_crate.mjs';

if (!crateAvailable()) console.log(`      D1/D1-neg UNMEASURED -- ${CRATE_ABSENT_REASON}`);

test('D1 wire.mjs VERDICT_KINDS holds exactly the variants VerdictKind declares, in declaration order', {
  skip: !crateAvailable() && CRATE_ABSENT_REASON,
}, () => {
  const { verdict_kinds } = extractFromCrate();
  assert.deepEqual([...VERDICT_KINDS], verdict_kinds);
  console.log(`      D1 ${verdict_kinds.length} verdict kinds compared against ${crateLibPath()}`);
});

test('D1-neg the comparison notices a kind the wire vocabulary would have missed', {
  skip: !crateAvailable() && CRATE_ABSENT_REASON,
}, () => {
  const { verdict_kinds } = extractFromCrate();
  const planted = [...VERDICT_KINDS, 'Bogus'];
  assert.notDeepEqual(planted, verdict_kinds);
});
