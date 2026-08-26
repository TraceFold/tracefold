// SPDX-License-Identifier: Apache-2.0
// M1 -- what has to be true before this window says "sealed".
//
// Decides; draws nothing. There is no document in this file and the boundary test
// keeps it that way.
//
// A boolean cannot seal a record. `sealed: true` arriving in a payload is a claim
// made by whoever wrote the payload, and repeating it back is not verification -- so
// the flag is not consulted at all here. Two things are: the basis has to be the one
// basis that means the value was compared rather than inferred, and somebody has to
// be present who could have done the comparing. Neither alone is enough, and the
// absence of a verifier is the case that matters, because drawing an unchecked
// record wearing a checked record's face is the worst thing this product can do.
//
// The basis is reported in six words, not four. The tree this replaces folded "the
// field was absent" and "the field was rubbish" into one answer, which was safe as
// long as nothing else ever read the basis and stops being safe the moment something
// does (req/04a M1). They are told apart here while it is cheap.

export const SEAL_MESSAGES = {
  NOT_A_RECORD: 'not a record',
  NO_VERIFIER: 'no verifier is present, so nothing here has been checked by anyone',
  BASIS_NOT_EXACT: 'the basis is not an exact comparison, so this is at best a resemblance',
  SEALED: 'compared exactly, by a verifier that is present',
};

/** Bases a record is allowed to state. */
export const STATED_BASES = Object.freeze(['exact', 'derived', 'observed']);

/**
 * Every answer `basisOf` can give. The last three are not bases a record states --
 * they are the three different ways a record can fail to state one, and they are
 * different: nobody wrote it, somebody wrote a word we do not hold, somebody wrote
 * something that is not a word at all.
 */
export const BASES = Object.freeze([...STATED_BASES, 'unstated', 'unknown', 'malformed']);

export const STANDINGS = Object.freeze(['sealed', 'unsealed']);

export function basisOf(record) {
  if (record === null || typeof record !== 'object' || Array.isArray(record)) return 'malformed';
  const basis = record.basis;
  if (basis === undefined || basis === null) return 'unstated';
  if (typeof basis !== 'string') return 'malformed';
  return STATED_BASES.includes(basis) ? basis : 'unknown';
}

/**
 * The claim, and the mark that goes with it. The mark is chosen here so that the
 * drawing part has nothing left to decide -- it places what it is handed.
 */
export function claimOf(record, { verifier = null } = {}) {
  const basis = basisOf(record);
  const unsealed = (why) => ({
    sealed: false, standing: 'unsealed', basis, why, mark: ['structure', 'unsealed'],
  });
  if (basis === 'malformed' && (record === null || typeof record !== 'object')) return unsealed(SEAL_MESSAGES.NOT_A_RECORD);
  if (basis !== 'exact') return unsealed(`${SEAL_MESSAGES.BASIS_NOT_EXACT} (basis: ${basis})`);
  if (!verifier) return unsealed(SEAL_MESSAGES.NO_VERIFIER);
  return { sealed: true, standing: 'sealed', basis, why: SEAL_MESSAGES.SEALED, mark: ['structure', 'seal'] };
}

/**
 * What a third party would need in hand to check this without asking us. Reported as
 * what is missing rather than as a yes or no, because "not portable" is not useful
 * to anyone and "the anchor is missing" is.
 */
export const PORTABLE_FIELDS = Object.freeze(['digest', 'algorithm', 'anchor']);

export function portability(record) {
  if (record === null || typeof record !== 'object') {
    return { portable: false, missing: [...PORTABLE_FIELDS], why: SEAL_MESSAGES.NOT_A_RECORD };
  }
  const missing = PORTABLE_FIELDS.filter((field) => {
    const value = record[field];
    return value === undefined || value === null || value === '';
  });
  return {
    portable: missing.length === 0,
    missing,
    why: missing.length === 0
      ? 'everything needed to check this elsewhere is present'
      : `cannot be checked away from here: ${missing.join(', ')} missing`,
  };
}
