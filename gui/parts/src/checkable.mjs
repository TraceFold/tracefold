// SPDX-License-Identifier: Apache-2.0
// M3 -- the things a reader can check for themselves, including the ones that do not
// hold.
//
// Decides; draws nothing.
//
// A page that lists only its passing claims is not being checked, it is being
// advertised, so every claim is returned with its verdict attached and a caller
// cannot ask for the passing ones only.
//
// Two claims are stronger here than in the tree this replaces, and both for the same
// reason: a claim whose sentence says more than its test does is a lie with a green
// tick on it (req/04a M3).
//
// The gap claim used to say the gaps "are exactly the withheld records" and then
// compare two lengths. Two of one and two of the other satisfied it whichever
// records they were, so a withholding that dropped the wrong record passed. Here the
// two sets of sequence numbers are compared as sets, and the claim names the ones
// that differ.
//
// And nothing checked that a sequence number appeared once. Sequence numbers are
// what the gap claim counts and what the chain claim walks, and a repeat folds two
// records into one silently in both. That is now its own claim, and it is stated as
// a precondition of the other two rather than left implied.

export const CHECKABLE_MESSAGES = {
  NO_RECORDS: 'no records were given, so there is nothing to check',
  CHECKED: 'claims evaluated',
};

const HEX = /^[0-9a-f]+$/i;
const SERIAL_LENGTH = 6;

const listOf = (values) => (values.length === 0 ? 'none' : values.join(', '));

/**
 * Every claim, evaluated. The shape is the same whether it holds or not: an id, the
 * sentence in full, the verdict, and enough detail to redo the check by hand.
 */
export function checkable(records, withheld = []) {
  const rows = Array.isArray(records) ? records : [];
  const held = Array.isArray(withheld) ? withheld : [];
  const claims = [];

  const digests = rows.map((r) => r?.digest);
  const uncuttable = digests
    .map((d, i) => ({ d, id: rows[i]?.id ?? `#${i}` }))
    .filter(({ d }) => typeof d !== 'string' || !HEX.test(d) || d.length < SERIAL_LENGTH);
  claims.push({
    id: 'serial-can-be-cut',
    claim: `Every record carries a hexadecimal digest of at least ${SERIAL_LENGTH} characters, so the serial shown beside it can be cut from it by hand.`,
    holds: rows.length > 0 && uncuttable.length === 0,
    detail: rows.length === 0
      ? CHECKABLE_MESSAGES.NO_RECORDS
      : `${rows.length - uncuttable.length} of ${rows.length} digests can be cut; cannot: ${listOf(uncuttable.map((u) => u.id))}`,
  });

  const ids = rows.map((r) => r?.id).filter((id) => id !== undefined && id !== null);
  const repeatedIds = ids.filter((id, i) => ids.indexOf(id) !== i);
  claims.push({
    id: 'identities-appear-once',
    claim: 'No identity appears on two records, so pointing at one record cannot mean two.',
    holds: ids.length === rows.length && repeatedIds.length === 0,
    detail: `${new Set(ids).size} distinct identities across ${rows.length} records; repeated: ${listOf([...new Set(repeatedIds)])}`,
  });

  const numbers = rows.map((r) => r?.n).filter((n) => Number.isInteger(n));
  const repeatedNumbers = [...new Set(numbers.filter((n, i) => numbers.indexOf(n) !== i))];
  const numbersAreSound = numbers.length === rows.length && repeatedNumbers.length === 0;
  claims.push({
    id: 'sequence-appears-once',
    claim: 'Every record carries a whole sequence number and no number appears twice. The two claims below count and walk these numbers, so neither means anything unless this one holds.',
    holds: numbersAreSound,
    detail: `${numbers.length} of ${rows.length} records numbered; repeated: ${listOf(repeatedNumbers.map(String))}`,
  });

  const present = new Set(numbers);
  const span = numbers.length > 0 ? { lo: Math.min(...numbers), hi: Math.max(...numbers) } : null;
  const gaps = [];
  if (span) for (let n = span.lo; n <= span.hi; n += 1) if (!present.has(n)) gaps.push(n);
  const heldNumbers = held.map((h) => (Number.isInteger(h) ? h : h?.n)).filter((n) => Number.isInteger(n));
  const gapsNotHeld = gaps.filter((n) => !heldNumbers.includes(n));
  const heldNotGaps = heldNumbers.filter((n) => !gaps.includes(n));
  claims.push({
    id: 'gaps-are-the-withheld-records',
    claim: 'Each break in the sequence is a record named as withheld, and each record named as withheld is a break. Not the same number of them: the same ones.',
    holds: numbersAreSound && heldNumbers.length === held.length && gapsNotHeld.length === 0 && heldNotGaps.length === 0,
    detail: `gaps at ${listOf(gaps.map(String))}; withheld at ${listOf(heldNumbers.map(String))}; unexplained gaps: ${listOf(gapsNotHeld.map(String))}; withheld that are not gaps: ${listOf(heldNotGaps.map(String))}${heldNumbers.length === held.length ? '' : `; ${held.length - heldNumbers.length} withheld entries carry no sequence number`}`,
  });

  const bySequence = rows.filter((r) => Number.isInteger(r?.n)).sort((a, b) => a.n - b.n);
  const brokenLinks = [];
  for (let i = 1; i < bySequence.length; i += 1) {
    const previous = bySequence[i - 1];
    const current = bySequence[i];
    if (current.prev !== previous.id) brokenLinks.push(`${current.id} names ${JSON.stringify(current.prev ?? null)} where ${previous.id} stands`);
  }
  claims.push({
    id: 'prev-names-the-record-before',
    claim: 'Each record names the identity of the record before it. This links the records; it does not seal them. The name is an identity and not a digest, so following the chain proves the order was written this way and proves nothing about whether the contents changed afterwards.',
    holds: numbersAreSound && brokenLinks.length === 0,
    detail: brokenLinks.length === 0
      ? `${Math.max(bySequence.length - 1, 0)} links, each naming the record before it`
      : `broken: ${listOf(brokenLinks)}`,
  });

  return claims;
}

/** One line per claim, for a caller that wants to print them without deciding anything. */
export function checkableLines(claims) {
  return claims.map((c) => `${c.holds ? 'holds' : 'does not hold'}: ${c.claim} (${c.detail})`);
}

/** The ones that do not hold. Provided so a caller can find them, never to hide them. */
export function failing(claims) {
  return claims.filter((c) => !c.holds);
}
