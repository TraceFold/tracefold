// SPDX-License-Identifier: Apache-2.0
// F-I -- whether a row's own escrowed inverse still exists, decided and not drawn.
//
// Decides; draws nothing. There is no document in this file, the same discipline
// parts/src/seal-claim.mjs holds for the seal claim -- a caller already familiar
// with claimOf() reads reversalOf() at a glance, because it returns the same
// shape: { state, why, mark }.
//
// req/768 section 1, family F-I: "Every committed row states, live, whether its
// escrowed inverse still exists and what pressing undo would restore." req/101
// section 6's own binding discipline -- the one this app already applies to
// MODO/JIN's enabled/disabled decision -- applies here unchanged: the answer is
// read off facts this window already has in hand, never by speculatively calling
// an undo route "to see what happens". post_transformations_id_undo is a write;
// calling it to probe would be exactly the self-favoring shortcut req/101 section
// 6 names and bans for the shell-track buttons, extended here to this face-track
// chip.
//
// membrane/wire-fields.json states its own hole in writing: "undo outcomes
// contribute no fields to this domain yet" -- the crate does not yet expose a
// field this window could read to say "the escrow for this row's inverse is still
// present". This module does not fabricate one. What it *can* say, honestly and
// from data this window already read in the same pass: a row a sibling in the
// same read names as its own predecessor (childOf) has demonstrably already been
// reversed. That is not the hole -- it is a different, positive fact, derivable
// with zero live calls, from records this window already holds in memory.
// Everything else renders the honest three-state absence rather than a guess
// (req/768 F-F, "a missing value renders one of exactly {measured-empty /
// not-observable-at-substrate / read-refused}, never a bare blank").

export const REVERSAL_MESSAGES = Object.freeze({
  reversed: (by) => `this row's own escrowed inverse was used${by ? `: row ${by} is on record as its reversal` : ', by a later row in this same read'}`,
  'not-observable': 'whether this row can still be undone right now is not observable from what this screen reads. The membrane does not yet expose a field for the escrowed inverse\'s own presence (membrane/wire-fields.json: "undo outcomes contribute no fields to this domain yet") -- this is a declared hole, not a computed answer, and this face will not call the undo route just to find out.',
  'not-committed': 'this has not happened yet, so there is no escrowed inverse to hold',
});

/** The three answers this module ever gives. Reversed is the one positive fact;
 * the other two are both honest absences, told apart because they come from
 * different reasons (nothing has happened yet, vs. the backend has not yet said). */
export const REVERSAL_STATES = Object.freeze(['reversed', 'not-observable', 'not-committed']);

const MARK_FOR = Object.freeze({
  reversed: ['standing', 'reversed'],
  'not-observable': ['standing', 'none'],
  'not-committed': ['standing', 'held'],
});

/**
 * Decide. `siblings` is whatever this window already read in the same pass --
 * never a second, live fetch. faces/ledger passes its own settled half's rows;
 * faces/graph passes one path-group's own rows; faces/held's rows are always
 * `lifecycle: 'held'` so they never reach the siblings scan at all; faces/receipt
 * has no list context whatsoever (a one-record read), so it always passes `[]`
 * and always reads not-observable -- honestly, not as a special case coded around,
 * simply because there is nothing in this function that could ever find a sibling
 * with none to look through.
 */
export function reversalOf(record, siblings = []) {
  if (!record || typeof record.id !== 'string' || record.id === '') {
    return { state: 'not-observable', by: null, why: REVERSAL_MESSAGES['not-observable'], mark: MARK_FOR['not-observable'] };
  }
  if (record.lifecycle === 'held') {
    return { state: 'not-committed', by: null, why: REVERSAL_MESSAGES['not-committed'], mark: MARK_FOR['not-committed'] };
  }
  const list = Array.isArray(siblings) ? siblings : [];
  const reverser = list.find((entry) => entry && entry !== record
    && typeof entry.childOf === 'string' && entry.childOf === record.id);
  if (reverser) {
    const by = typeof reverser.id === 'string' && reverser.id !== '' ? reverser.id : null;
    return { state: 'reversed', by, why: REVERSAL_MESSAGES.reversed(by), mark: MARK_FOR.reversed };
  }
  return { state: 'not-observable', by: null, why: REVERSAL_MESSAGES['not-observable'], mark: MARK_FOR['not-observable'] };
}
