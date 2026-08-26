// SPDX-License-Identifier: Apache-2.0
// C6 -- a shortened digest, and the two sentences that must travel with it.
//
// The defect this replaces is worth stating plainly, because it is the kind that
// survives review. The page that shipped said "six characters out of sixty-four"
// while the digest it was describing was sixteen characters long, and a few lines
// above it the same page said so (req/04a C6, defect 1). Sixty-four was a string
// somebody typed. The check that guarded the sentence looked for the phrase "not a
// proof" and found it, so it passed.
//
// Here the denominator is read off the digest. There is no literal to go stale, and
// the test asserts the rendered text carries the digest's real length.
//
// The second defect was that a serial could travel alone: the share card printed six
// characters with neither the cut nor the caveat, and nothing checked it because the
// check knew only about the page (req/04a C6, defect 2). So `serial` has no mode
// that emits the characters by themselves. Getting the number out of this part
// without its two sentences means calling `serialOf`, which returns a string and
// draws nothing.

import { el, style } from './element.mjs';
import { CONSUMED } from './tokens.mjs';

export const SERIAL_MESSAGES = {
  NOT_HEX: 'a digest is hexadecimal; this is not',
  TOO_SHORT: 'the digest is shorter than the cut asked for',
  BAD_TAKE: 'a cut takes a whole number of characters, one or more',
  CUT: 'digest cut',
};

export const DEFAULT_TAKE = 6;

const HEX = /^[0-9a-f]+$/i;

/** Why this digest cannot be cut, or null if it can. */
export function refusalFor(digest, take = DEFAULT_TAKE) {
  if (!Number.isInteger(take) || take < 1) return SERIAL_MESSAGES.BAD_TAKE;
  if (typeof digest !== 'string' || !HEX.test(digest)) return SERIAL_MESSAGES.NOT_HEX;
  if (digest.length < take) return SERIAL_MESSAGES.TOO_SHORT;
  return null;
}

/** The characters alone. Draws nothing, so it cannot put a bare serial on a screen. */
export function serialOf(digest, { take = DEFAULT_TAKE } = {}) {
  return refusalFor(digest, take) ? null : digest.slice(0, take).toUpperCase();
}

/** How it was cut, said in full, so a reader can redo the cut by hand. */
export function cutOf(digest, { take = DEFAULT_TAKE } = {}) {
  const refusal = refusalFor(digest, take);
  if (refusal) return refusal;
  return `the first ${take} of ${digest.length} hexadecimal characters, in upper case`;
}

/**
 * Why it is not evidence. Both numbers come from the digest in hand: a shorter digest
 * makes this sentence say something different, which is the whole point.
 */
export function notAProof(digest, { take = DEFAULT_TAKE } = {}) {
  const refusal = refusalFor(digest, take);
  if (refusal) return refusal;
  return `${take} characters out of ${digest.length}: two different digests can begin the same way, so a match here is a hint and not a proof. Compare the whole digest.`;
}

/**
 * The three together. There is no argument that removes the sentences, and the
 * unusable case is drawn as a refusal rather than as an empty space.
 */
export function serial(digest, { take = DEFAULT_TAKE, size = 12 } = {}) {
  const refusal = refusalFor(digest, take);
  const cut = serialOf(digest, { take });
  return el('span', {
    'data-part': 'serial',
    'data-usable': String(refusal === null),
    style: style({
      display: 'inline-flex', 'align-items': 'baseline', gap: '8px',
      // SS558 body-text floor: was CONSUMED.meta (12px), now CONSUMED.record (14px).
      color: CONSUMED.ink, 'font-size': CONSUMED.record, 'line-height': CONSUMED.recordLine,
      'font-family': CONSUMED.sans,
    }),
  }, [
    el('span', {
      'data-role': 'cut-value',
      style: style({ 'font-family': CONSUMED.mono, 'font-size': `${size}px`, 'letter-spacing': '0.08em' }),
    }, [cut ?? 'no serial']),
    el('span', {
      'data-role': 'how-cut',
      style: style({ color: CONSUMED.attendant }),
    }, [refusal ?? cutOf(digest, { take })]),
    el('span', {
      'data-role': 'not-a-proof',
      style: style({ color: CONSUMED.attendant }),
    }, [refusal ? 'nothing was cut, so there is nothing here to be mistaken for proof' : notAProof(digest, { take })]),
  ]);
}
