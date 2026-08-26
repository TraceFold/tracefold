// SPDX-License-Identifier: Apache-2.0
// Owner #340: the sidebar carries state, not links -- so what a count slot says has to
// be exactly what a face is displaying, and has to say nothing at all when no face is
// displaying anything.
//
// The reach into a document (four querySelector calls) is not what is worth testing.
// What is worth testing is the decision made about a reading that is missing, empty or
// explicitly unread, which is why `censusOf` has no document in it -- the same split
// `changing()` was pulled out of render.mjs for under W8, and for the same reason.
//
// Red-first: one shared body is run first against a census that reports a zero where it
// has nothing (the obvious wrong implementation, and the one a reader of the sidebar
// could not tell from a real measured zero), and is required to throw there before the
// real one is trusted.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  censusOf, countText, COUNT_UNREAD, COUNT_SOURCE, COUNT_SAID, COUNT_STATE,
} from '../kernel/render.mjs';

/** What this must not be: a census that fills a gap with a number. */
const zeroFilling = (readings) => {
  const found = new Map();
  for (const reading of readings ?? []) {
    if (!reading?.id) continue;
    found.set(reading.id, { value: String(reading.value ?? 0), noun: reading.noun || 'records' });
  }
  return found;
};

const reading = (id, value, noun = null) => ({ id, value, noun });

/**
 * The claim, as one body: a face that drew a number is counted; a face that is standing
 * but stated no number, or stated it as unread, is NOT counted; and what an uncounted
 * slot says is the dash and never a digit.
 */
function assertOnlyDrawnNumbersBecomeCounts(census) {
  const found = census([
    reading('a', '12', 'records'),
    reading('b', '0', 'held'),
    reading('c', null),
    reading('d', COUNT_SOURCE.unread),
    reading('e', ''),
  ]);
  assert.equal(countText(found.get('a')), '12');
  // A measured zero is a count and is drawn.
  assert.equal(countText(found.get('b')), '0');
  // Three shapes of "nothing was read", all of which must reach the dash.
  assert.equal(countText(found.get('c')), COUNT_UNREAD);
  assert.equal(countText(found.get('d')), COUNT_UNREAD);
  assert.equal(countText(found.get('e')), COUNT_UNREAD);
  // And a face nobody mounted at all.
  assert.equal(countText(found.get('never-mounted')), COUNT_UNREAD);
}

test('red-first: a census that fills a gap with a zero fails the claim', () => {
  assert.throws(() => assertOnlyDrawnNumbersBecomeCounts(zeroFilling));
});

test('only a number a face is actually drawing becomes a count; everything else is a dash', () => {
  assertOnlyDrawnNumbersBecomeCounts(censusOf);
});

test('the dash and the zero are different strings, because they are different facts', () => {
  assert.notEqual(COUNT_UNREAD, '0');
  assert.equal(/\d/.test(COUNT_UNREAD), false, 'nothing about the dash may look like a measurement');
});

test('a reading with no face id is not a reading', () => {
  // A host can be standing with nothing in it, or with something that is not a face.
  assert.equal(censusOf([reading(null, '9'), reading(undefined, '9'), reading('', '9')]).size, 0);
  assert.equal(censusOf([]).size, 0);
  assert.equal(censusOf(null).size, 0);
  assert.equal(censusOf(undefined).size, 0);
});

test('the noun travels with the number, and falls back to a stated default rather than to nothing', () => {
  assert.equal(censusOf([reading('a', '3', 'candidates')]).get('a').noun, 'candidates');
  assert.equal(censusOf([reading('a', '3', null)]).get('a').noun, COUNT_SOURCE.defaultNoun);
  assert.equal(censusOf([reading('a', '3', '')]).get('a').noun, COUNT_SOURCE.defaultNoun);
});

test('what a slot says about itself distinguishes a reading from the absence of one', () => {
  const found = censusOf([reading('a', '12', 'records')]).get('a');
  assert.match(COUNT_SAID.read(found), /^12 records,/);
  assert.match(COUNT_SAID.unplaced, /not standing anywhere/);
  assert.equal(/\b0\b/.test(COUNT_SAID.unplaced), false, 'the reason a slot is empty does not mention a number');
  assert.equal(/\b0\b/.test(COUNT_SAID.standing), false);
});

/**
 * req/811 §8-2b, the most serious finding in that document, pinned so it cannot return.
 *
 * There were two sentences for three states. `--` is produced whenever the census found
 * no value, and it was explained as "this face is not standing anywhere in this space" --
 * but a face that IS standing and has simply not reported yet also has no census value,
 * and with the membrane unbound that was every face, permanently. So five of six sidebar
 * items asserted `on` (meaning: stands somewhere) while the tooltip on the same button
 * said the face stood nowhere, and `aria-pressed="true"` broadcast the false half of that
 * pair to assistive technology. The window stated a proposition and its negation about one
 * face at one instant, six times, on first paint.
 */
test('a face that stands here and has not reported is not told it stands nowhere', () => {
  assert.notEqual(COUNT_SAID.standing, COUNT_SAID.unplaced);
  assert.equal(/not standing anywhere/.test(COUNT_SAID.standing), false,
    'the standing-but-silent state must not borrow the unplaced sentence: that is the contradiction');
  assert.match(COUNT_SAID.standing, /standing here/);
  // Three states, three distinct sentences, and none of them is reachable from another.
  const said = [COUNT_SAID.standing, COUNT_SAID.unplaced, COUNT_SAID.read({ value: '1', noun: 'calls' })];
  assert.equal(new Set(said).size, 3);
});

test('red-first: the three states are named, so a slot cannot be in an unnamed one', () => {
  assert.deepEqual(Object.values(COUNT_STATE).sort(), ['read', 'standing', 'unplaced']);
  for (const which of Object.values(COUNT_STATE)) {
    if (which === COUNT_STATE.READ) continue;
    assert.equal(typeof COUNT_SAID[which], 'string', `${which} has no sentence`);
  }
});

test('the last reading of an id wins, so two hosts drawing one face cannot disagree in the sidebar', () => {
  // A face can stand in two places at once (a dock and a stage tab). Both draw the same
  // population from the same read, so either is correct -- what must not happen is two
  // slots for one face showing two numbers, which is req/784 R-07's defect class.
  const found = censusOf([reading('a', '12', 'records'), reading('a', '12', 'records')]);
  assert.equal(found.size, 1);
  assert.equal(found.get('a').value, '12');
});

test('the selectors the reaching half uses are declared here, not written twice', () => {
  // If these ever drift from what parts/src/surface.mjs's statBand() draws, the sidebar
  // silently goes all-dashes -- which looks like an honest state and is not one. Named
  // so a change to the band is a change that has to pass through this file.
  assert.equal(COUNT_SOURCE.band, '[data-part="stat-band"] [data-role="segment"]');
  assert.equal(COUNT_SOURCE.value, 'data-value');
  assert.equal(COUNT_SOURCE.noun, 'data-noun');
  assert.equal(COUNT_SOURCE.unread, 'unread');
});
