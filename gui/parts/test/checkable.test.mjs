// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { checkable, checkableLines, failing, CHECKABLE_MESSAGES } from '../src/checkable.mjs';

const SOUND = [
  { id: 'r-01', n: 1, digest: 'a73e393d8a97b86c', prev: null },
  { id: 'r-02', n: 2, digest: 'b1c2d3e4f5a6b7c8', prev: 'r-01' },
  { id: 'r-04', n: 4, digest: 'ffee0011aabb2233', prev: 'r-02' },
];
const WITHHELD = [{ id: 'r-03', n: 3 }];

const claimById = (claims, id) => claims.find((c) => c.id === id);

test('every claim comes back with its verdict, and there is no way to ask for the passing ones only', () => {
  const claims = checkable(SOUND, WITHHELD);
  assert.equal(claims.length, 5);
  for (const claim of claims) {
    assert.equal(typeof claim.holds, 'boolean');
    assert.ok(claim.claim.length > 30, claim.id);
    assert.ok(claim.detail.length > 0, claim.id);
  }
});

test('a sound set holds every claim, so a failure below means something', () => {
  const claims = checkable(SOUND, WITHHELD);
  assert.deepEqual(failing(claims), []);
});

test('the gap claim compares which records are missing, not how many', () => {
  // The defect being closed. Under a count comparison this passes: one gap, one
  // withheld record. The gap is at 3 and the record named as withheld is 9, so the
  // claim as written -- these gaps are these records -- is false.
  const claims = checkable(SOUND, [{ id: 'r-09', n: 9 }]);
  const gaps = claimById(claims, 'gaps-are-the-withheld-records');
  assert.equal(gaps.holds, false);
  assert.match(gaps.detail, /unexplained gaps: 3/);
  assert.match(gaps.detail, /withheld that are not gaps: 9/);
  assert.match(gaps.claim, /Not the same number of them: the same ones/);
});

test('an unexplained gap is caught even when nothing at all was withheld', () => {
  const gaps = claimById(checkable(SOUND, []), 'gaps-are-the-withheld-records');
  assert.equal(gaps.holds, false);
  assert.match(gaps.detail, /gaps at 3/);
});

test('a withheld entry carrying no sequence number is counted as unusable, not as a match', () => {
  const gaps = claimById(checkable(SOUND, [{ id: 'r-03' }]), 'gaps-are-the-withheld-records');
  assert.equal(gaps.holds, false);
  assert.match(gaps.detail, /1 withheld entries carry no sequence number/);
});

test('a repeated sequence number is its own claim, because two others are built on it', () => {
  const doubled = [
    { id: 'r-01', n: 1, digest: 'a73e39', prev: null },
    { id: 'r-02', n: 1, digest: 'b1c2d3', prev: 'r-01' },
  ];
  const claims = checkable(doubled, []);
  const sequence = claimById(claims, 'sequence-appears-once');
  assert.equal(sequence.holds, false);
  assert.match(sequence.detail, /repeated: 1/);
  assert.equal(claimById(claims, 'gaps-are-the-withheld-records').holds, false, 'a claim resting on it cannot hold either');
  assert.equal(claimById(claims, 'prev-names-the-record-before').holds, false);
  assert.match(sequence.claim, /neither means anything unless this one holds/);
});

test('a record with no sequence number at all fails the same claim', () => {
  const claims = checkable([{ id: 'a', n: 1, digest: 'aabbcc', prev: null }, { id: 'b', digest: 'ddeeff', prev: 'a' }], []);
  assert.equal(claimById(claims, 'sequence-appears-once').holds, false);
  assert.match(claimById(claims, 'sequence-appears-once').detail, /1 of 2 records numbered/);
});

test('a repeated identity is caught, and a distinct one is not', () => {
  const claims = checkable([{ id: 'a', n: 1, digest: 'aabbcc' }, { id: 'a', n: 2, digest: 'ddeeff' }], []);
  const ids = claimById(claims, 'identities-appear-once');
  assert.equal(ids.holds, false);
  assert.match(ids.detail, /repeated: a/);
  assert.equal(claimById(checkable(SOUND, WITHHELD), 'identities-appear-once').holds, true);
});

test('a digest that no serial could be cut from is named', () => {
  const claims = checkable([{ id: 'a', n: 1, digest: 'zz' }, { id: 'b', n: 2, digest: 'aabbccdd', prev: 'a' }], []);
  const cut = claimById(claims, 'serial-can-be-cut');
  assert.equal(cut.holds, false);
  assert.match(cut.detail, /cannot: a/);
  assert.match(cut.detail, /1 of 2 digests can be cut/);
});

test('the chain claim says in its own words that it is not a seal', () => {
  const chain = claimById(checkable(SOUND, WITHHELD), 'prev-names-the-record-before');
  assert.match(chain.claim, /does not seal them/);
  assert.match(chain.claim, /identity and not a digest/);
  assert.equal(chain.holds, true);
});

test('a broken link is named with both ends', () => {
  const broken = [
    { id: 'r-01', n: 1, digest: 'aabbcc', prev: null },
    { id: 'r-02', n: 2, digest: 'ddeeff', prev: 'r-99' },
  ];
  const chain = claimById(checkable(broken, []), 'prev-names-the-record-before');
  assert.equal(chain.holds, false);
  assert.match(chain.detail, /r-02 names "r-99" where r-01 stands/);
});

test('no records is answered as no records, not as everything holding', () => {
  const claims = checkable([], []);
  assert.equal(claimById(claims, 'serial-can-be-cut').holds, false);
  assert.equal(claimById(claims, 'serial-can-be-cut').detail, CHECKABLE_MESSAGES.NO_RECORDS);
  assert.equal(checkable(null, null).length, 5);
});

test('the lines a caller prints carry the verdict, not only the sentence', () => {
  const lines = checkableLines(checkable(SOUND, []));
  assert.equal(lines.length, 5);
  assert.ok(lines.some((l) => l.startsWith('does not hold')));
  assert.ok(lines.every((l) => /^(holds|does not hold): /.test(l)));
});
