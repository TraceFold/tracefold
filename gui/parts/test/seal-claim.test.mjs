// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { basisOf, claimOf, portability, BASES, STATED_BASES, STANDINGS, PORTABLE_FIELDS, SEAL_MESSAGES } from '../src/seal-claim.mjs';

const VERIFIER = { name: 'gx-verify' };

test('a boolean does not seal a record, however loudly it is written', () => {
  for (const record of [
    { sealed: true },
    { sealed: true, verified: true },
    { sealed: true, basis: 'derived' },
    { sealed: true, basis: 'observed' },
  ]) {
    const claim = claimOf(record, { verifier: VERIFIER });
    assert.equal(claim.sealed, false, JSON.stringify(record));
    assert.equal(claim.standing, 'unsealed');
  }
});

test('an exact basis with nobody to have compared it is not sealed, and says which of the two is missing', () => {
  const claim = claimOf({ basis: 'exact' }, { verifier: null });
  assert.equal(claim.sealed, false);
  assert.equal(claim.why, SEAL_MESSAGES.NO_VERIFIER);
  assert.equal(claim.basis, 'exact');
});

test('a verifier with an inexact basis is not sealed either, and names the basis it got', () => {
  const claim = claimOf({ basis: 'derived' }, { verifier: VERIFIER });
  assert.equal(claim.sealed, false);
  assert.match(claim.why, /basis: derived/);
});

test('both together seal it, and only then', () => {
  const claim = claimOf({ basis: 'exact' }, { verifier: VERIFIER });
  assert.equal(claim.sealed, true);
  assert.equal(claim.standing, 'sealed');
  assert.equal(claim.why, SEAL_MESSAGES.SEALED);
});

test('the claim chooses the mark, so the drawing part has nothing left to decide', () => {
  assert.deepEqual(claimOf({ basis: 'exact' }, { verifier: VERIFIER }).mark, ['structure', 'seal']);
  assert.deepEqual(claimOf({ basis: 'exact' }, {}).mark, ['structure', 'unsealed']);
  for (const standing of STANDINGS) assert.equal(typeof standing, 'string');
});

test('a missing basis, an unknown word and a broken value are three different answers', () => {
  assert.equal(basisOf({}), 'unstated');
  assert.equal(basisOf({ basis: null }), 'unstated');
  assert.equal(basisOf({ basis: 'approximately' }), 'unknown');
  assert.equal(basisOf({ basis: {} }), 'malformed');
  assert.equal(basisOf({ basis: [] }), 'malformed');
  assert.equal(basisOf({ basis: 7 }), 'malformed');
  assert.equal(basisOf(null), 'malformed');
  assert.equal(basisOf('exact'), 'malformed');
  assert.equal(basisOf([{ basis: 'exact' }]), 'malformed');
  assert.equal(new Set([basisOf({}), basisOf({ basis: 'x' }), basisOf({ basis: {} })]).size, 3);
});

test('the stated bases are the only ones a record can claim, and all of them fail to seal but one', () => {
  for (const basis of STATED_BASES) {
    assert.equal(basisOf({ basis }), basis);
    assert.equal(claimOf({ basis }, { verifier: VERIFIER }).sealed, basis === 'exact');
  }
  assert.equal(BASES.length, STATED_BASES.length + 3);
});

test('a record that is not a record is refused by name', () => {
  const claim = claimOf(null, { verifier: VERIFIER });
  assert.equal(claim.sealed, false);
  assert.equal(claim.why, SEAL_MESSAGES.NOT_A_RECORD);
  assert.equal(claimOf('exact', { verifier: VERIFIER }).why, SEAL_MESSAGES.NOT_A_RECORD);
});

test('portability reports what is missing rather than a yes or a no', () => {
  const full = { digest: 'a1', algorithm: 'blake3', anchor: 'tile/44' };
  assert.deepEqual(portability(full), { portable: true, missing: [], why: 'everything needed to check this elsewhere is present' });
  const partial = portability({ digest: 'a1' });
  assert.equal(partial.portable, false);
  assert.deepEqual(partial.missing, ['algorithm', 'anchor']);
  assert.match(partial.why, /algorithm, anchor missing/);
  assert.deepEqual(portability(null).missing, [...PORTABLE_FIELDS]);
  assert.deepEqual(portability({ digest: '', algorithm: null, anchor: undefined }).missing, [...PORTABLE_FIELDS]);
});

test('sealed and portable are separate questions and neither answers the other', () => {
  const record = { basis: 'exact' };
  assert.equal(claimOf(record, { verifier: VERIFIER }).sealed, true);
  assert.equal(portability(record).portable, false);
});
