// SPDX-License-Identifier: Apache-2.0
// C3 -- the verdict, shown as a shape.
//
// This part draws and does not decide (req/04 AC-P0). The distinction that matters
// is not "no lookups": it is that nothing here compares a record against a verdict
// word and takes a different route. A verdict arrives already decided, the sheet is
// asked for its drawing, and the drawing is placed. tools/boundary.mjs holds the
// count of verdict comparisons in drawing parts at zero, and fires red on a planted
// one.
//
// Colour is not how a verdict is told apart -- the frozen word and a distinct
// drawing both carry it, so the badge survives being read in one colour. Red is a
// property of one mark rather than a judgement about a record, which is what keeps
// the single-red rule (req/04 T-5) checkable: one site spends the deny colour.

import { el, style } from './element.mjs';
import { glyph, markOf, MIN_READABLE } from './glyph-sheet.mjs';
import { CONSUMED } from './tokens.mjs';

export const BADGE_MESSAGES = {
  UNKNOWN_VERDICT: 'a word arrived where a verdict was expected and this sheet does not hold it',
  DREW: 'verdict drawn',
};

/**
 * The word shown when the wire sends something that is not one of the three frozen
 * verdicts. This never falls back to a generic "undefined mark" -- a screen that says
 * only "undefined" has thrown away the one piece of evidence a reader could act on,
 * the word that actually arrived. The fallback names it instead: what showed up, in
 * the position a verdict was expected.
 */
export function unrecognisedWord(verdict) {
  const shown = verdict === null || verdict === undefined || verdict === ''
    ? '(nothing was sent)'
    : JSON.stringify(String(verdict));
  return `not a recognised verdict: ${shown}`;
}

/**
 * The meanings allowed to be red. Keyed by meaning rather than by verdict word, so
 * the rule reads "this mark is the red one" and not "this record is the bad one".
 */
export const RED_MEANINGS = Object.freeze(['verdict.deny']);

export function isRed(mark) {
  return RED_MEANINGS.includes(mark.means);
}

/**
 * Owner #340: the standings, and the two inks each of them owns.
 *
 * The header of this file used to say colour is not how a verdict is told apart, and
 * the half of that which was true is kept: the frozen word and a distinct drawing both
 * carry the verdict, so nothing here depends on colour and a screen read in one colour
 * loses no fact. What was not true is the conclusion drawn from it -- that the four
 * standings should therefore look the same. They did, and Owner #340's reading of the
 * result is the one this table answers: at arm's length a stranger could not tell an
 * admitted row from a denied one from a held one, because the only thing that differed
 * was a 14px stroke drawing.
 *
 * Keyed by meaning, exactly as RED_MEANINGS above is, and for the same reason: the
 * rule reads "this mark has this ink", never "this record is that kind of record". It
 * is a table read by key. Nothing here compares a verdict word to anything --
 * tools/boundary.mjs's verdictBranches gate holds that at zero across this file and
 * would fire on a comparison planted in it.
 *
 * `standing.held` is in the same table as the three verdicts on purpose. It is not a
 * verdict, and it is drawn in exactly the places a verdict is drawn (a held candidate's
 * chip, a box head's pill), so a reader who has learnt that this hue means held reads
 * it the same way wherever it appears.
 */
export const STANDING_INK = Object.freeze({
  'verdict.admit': Object.freeze({ ink: CONSUMED.admit, bed: CONSUMED.bedAdmit }),
  'verdict.deny': Object.freeze({ ink: CONSUMED.deny, bed: CONSUMED.bedDeny }),
  'verdict.escalate': Object.freeze({ ink: CONSUMED.escalate, bed: CONSUMED.bedEscalate }),
  'standing.held': Object.freeze({ ink: CONSUMED.held, bed: CONSUMED.bedHeld }),
});

/** The only place in this package that spends the deny colour. */
export function inkFor(mark) {
  return STANDING_INK[mark.means]?.ink ?? CONSUMED.ink;
}

/**
 * The bed a mark's own ink is measured against, or null for a mark that has no
 * standing of its own -- an effect, a structure mark, or a word the engine never
 * promised. Null is drawn as no bed at all rather than as a neutral one: a filled chip
 * says "this is one of the standings", and inventing a fill for something that is not
 * one would be the screen claiming a classification it does not have.
 */
export function bedFor(mark) {
  return STANDING_INK[mark.means]?.bed ?? null;
}

/** Every meaning that has been given a standing of its own, so a caller can count them. */
export const STANDING_MEANINGS = Object.freeze(Object.keys(STANDING_INK));

/**
 * The badge. `verdict` is the engine's own spelling and is printed unchanged -- the
 * frozen words are not rephrased and no friendlier synonym is invented for them.
 */
export function badge(verdict, { size = MIN_READABLE, showWord = true } = {}) {
  const mark = markOf('verdict', verdict);
  const word = mark.defined ? String(verdict) : unrecognisedWord(verdict);
  const bed = bedFor(mark);
  return el('span', {
    'data-part': 'verdict-badge',
    'data-verdict': mark.defined ? String(verdict) : 'undefined',
    'data-defined': String(mark.defined),
    'data-filled': String(bed !== null),
    // The word, always, not only when it is one the sheet does not hold.
    //
    // The word span below caps itself at 4.5rem and ellipsizes, and at this app's own
    // 720px viewport that was cutting `Admit`, `Deny` and `Escalate` -- the engine's own
    // answer, the single most load-bearing value on the screen -- with the full form
    // nowhere on the page. Every capture read `clipped=0` because the ellipsis sits on a
    // child span and the probe was looking at the cell. Found by the faces/ledger lane,
    // measured, and it had been true since the cap was added.
    title: mark.defined ? word : `${BADGE_MESSAGES.UNKNOWN_VERDICT}: ${JSON.stringify(verdict)}`,
    'data-full': word,
    style: style({
      display: 'inline-flex',
      'align-items': 'center',
      gap: '6px',
      // Owner #340: the chip is filled. A glyph and a word on the page background are
      // the shape this screen already had and the shape the Owner read as monotone;
      // a bed gives the mark an area rather than a stroke, which is what makes it
      // separable from across a room. The bed and the ink on it are a measured pair
      // (parts/src/tokens.mjs INK_ON_BED, held at or above the normal-text floor on
      // both pages by parts/test/tokens.test.mjs), so filling costs no legibility.
      //
      // A mark with no standing of its own gets no fill. It is not drawn on a neutral
      // bed either -- see bedFor().
      ...(bed === null ? {} : {
        background: bed,
        'border-radius': CONSUMED.radiusChip,
        padding: '1px 6px',
      }),
      // req/768 AC-4 retrofit round 2 found this: the row's grid cell that
      // hosts this badge (parts/src/receipt-row.mjs's `cell()`) is itself
      // `display:flex; overflow:hidden`, and this badge is that flex
      // container's one child. A flex item's automatic minimum width is its
      // own min-content size unless the item states otherwise -- and this
      // badge never did, so at a width narrow enough (the row-gutter frame
      // this round added shrinks every row's available width by the gutter's
      // own fixed budget) the badge's own layout box exceeded its cell's,
      // measured by tools/shoot.mjs's overlap probe even though the cell's
      // own overflow:hidden already clipped the paint. `min-width:0` is the
      // standard fix for this exact flexbox default (the same one
      // `parts/src/receipt-row.mjs`'s own `flex:1,min-width:0` pattern
      // already applies everywhere else in this row); it changes no visible
      // pixel, only the geometry a measurement tool reads.
      'min-width': '0',
      overflow: 'hidden',
      color: inkFor(mark),
      'font-family': CONSUMED.sans,
      // SS558: body text floor is 14px; a verdict word is primary data (the
      // engine's own answer), so it takes CONSUMED.record rather than the
      // 12px CONSUMED.meta this badge used before this lane.
      'font-size': CONSUMED.record,
      'line-height': CONSUMED.recordLine,
      'white-space': 'nowrap',
    }),
  }, [
    glyph('verdict', verdict, { size, label: word }),
    // The word truncates inside its own box rather than pushing past it: the frozen
    // three words are short enough that this never fires, but the fallback for a word
    // the wire did not promise (unrecognisedWord) can be long, and a badge is drawn
    // inside a fixed-width row cell (parts/src/receipt-row.mjs). Without its own
    // overflow limit the word's layout box extends past the cell into whatever sits
    // next to it -- a real overlap the picture shows even though the cell's own
    // `overflow:hidden` clips the paint (found the same way N-1 was: by measuring the
    // rendered page, not the DOM). The full word stays in reach in `title`, and for a
    // row this is drawn in, in the "verdict in full" note line underneath.
    showWord ? el('span', {
      'data-role': 'word',
      style: style({
        overflow: 'hidden', 'text-overflow': 'ellipsis', 'white-space': 'nowrap', 'max-width': '4.5rem', display: 'inline-block', 'vertical-align': 'bottom',
      }),
    }, [word]) : null,
  ]);
}

/**
 * The same chip, for a mark that is not a verdict.
 *
 * badge() is the verdict namespace's own entry point and stays exactly that. This is
 * for the two places Owner #340's pass needs the identical shape over a different
 * namespace: a held candidate's own standing, and the status pill on a box head
 * (parts/src/surface.mjs box()). Nothing about the drawing differs -- same table, same
 * measured bed, same rule that a mark with no standing of its own is drawn without a
 * fill -- so a reader who has learnt the hue reads it the same wherever it appears.
 *
 * The word is the caller's, not this module's. A namespace this package does not own
 * the vocabulary of (an effect verb, a face's own noun for a group) has no frozen
 * spelling here to print, and inventing one would be this part deciding.
 */
export function chip(namespace, key, { size = MIN_READABLE, word = null, said = null } = {}) {
  const mark = markOf(namespace, key);
  const bed = bedFor(mark);
  const shown = word === null ? String(key) : String(word);
  return el('span', {
    'data-part': 'standing-chip',
    'data-standing': mark.defined ? String(key) : 'undefined',
    // No `data-means` here, and the omission is load-bearing rather than tidy.
    //
    // Every face's own tools/gate.mjs runs a `one-meaning-one-mark` tree check keyed on
    // `data-means`: it walks the tree, and for each node carrying a meaning it reads the
    // `data-mark` beside it, so that one meaning can never be drawn by two different
    // marks. A wrapper carrying the meaning and no mark is visited before the glyph
    // inside it, and pairs the meaning with nothing -- which reds the gate on a screen
    // that is in fact drawing exactly one mark for it. Found by the faces/graph lane
    // with a probe rather than by reading, on a chip this module had shipped an hour
    // earlier; `badge()` never had the attribute and was never affected.
    //
    // The meaning is not lost: the glyph inside carries `data-mark` and `data-means`
    // together, which is the pairing the gate exists to check. An instrument that wants
    // the chip's meaning reads it from the mark, where the mark is.
    'data-filled': String(bed !== null),
    title: said,
    style: style({
      display: 'inline-flex',
      'align-items': 'center',
      gap: '5px',
      flex: 'none',
      'min-width': '0',
      overflow: 'hidden',
      color: inkFor(mark),
      'font-family': CONSUMED.sans,
      'font-size': CONSUMED.record,
      'line-height': CONSUMED.recordLine,
      'white-space': 'nowrap',
      ...(bed === null
        ? { border: `1px solid ${CONSUMED.rule}`, 'border-radius': CONSUMED.radiusChip, padding: '1px 6px' }
        : { background: bed, 'border-radius': CONSUMED.radiusChip, padding: '1px 6px' }),
    }),
  }, [
    glyph(namespace, key, { size, label: shown }),
    el('span', {
      'data-role': 'word',
      style: style({ overflow: 'hidden', 'text-overflow': 'ellipsis' }),
    }, [shown]),
  ]);
}
