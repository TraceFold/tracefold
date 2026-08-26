// SPDX-License-Identifier: Apache-2.0
// W13 -- "the ACs' population is not the empty set of changed files": every AC row in
// req/02_SHELL_WLAYER.md §4 must quantify over something that is non-empty on the static
// tree, or its gate holds for the wrong reason (nothing to fail it).
//
// Red-first: `nonEmpty()` is exercised first against a deliberately empty population
// (what a row looks like once its subject is deleted -- the exact failure mode this AC
// exists to catch: a gate whose "over" produces an array of length zero) and required to
// throw; then against all 15 real rows, and required to pass 15 times.

import test from 'node:test';
import assert from 'node:assert/strict';

import { ROWS, populationCounts } from '../tools/ac_population.mjs';

function nonEmpty(row, count) {
  assert.ok(count > 0, `${row.ac} ("${row.quantifies}") has an empty population on the static tree`);
}

test('red-first: nonEmpty() rejects a row whose population is empty', () => {
  assert.throws(() => nonEmpty({ ac: 'W-broken', quantifies: 'a deleted subject' }, 0), /empty population/);
  assert.doesNotThrow(() => nonEmpty({ ac: 'W-ok', quantifies: 'anything' }, 1));
});

test('all 15 AC rows are present and named W1..W15 once each', () => {
  assert.equal(ROWS.length, 15, `req/02 §4 has 15 AC rows; this harness has ${ROWS.length}`);
  const names = ROWS.map((r) => r.ac);
  assert.deepEqual(names, Array.from({ length: 15 }, (_, i) => `W${i + 1}`), 'the rows are not W1..W15 in order');
  assert.equal(new Set(names).size, 15, 'an AC row is duplicated');
});

test('every AC row\'s population is non-empty on the static tree, and the counts are reported', () => {
  const counts = populationCounts();
  for (const row of counts) nonEmpty(row, row.count);
  process.stdout.write(`# W13 AC populations:\n${counts.map((r) => `#   ${r.ac.padEnd(4)} n=${r.count}`).join('\n')}\n`);
});
