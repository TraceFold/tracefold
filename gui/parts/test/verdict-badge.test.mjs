// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { textOf, findByAttr, find } from '../src/element.mjs';
import {
  badge, chip, isRed, inkFor, bedFor, RED_MEANINGS, STANDING_INK, STANDING_MEANINGS,
  BADGE_MESSAGES, unrecognisedWord,
} from '../src/verdict-badge.mjs';
import { markOf, MARKS } from '../src/glyph-sheet.mjs';
import { CONSUMED } from '../src/tokens.mjs';

const WORDS = Object.keys(MARKS.verdict);

test('the frozen word is printed as the engine spells it, with nothing friendlier put in its place', () => {
  for (const word of WORDS) {
    const node = badge(word);
    assert.equal(textOf(node), word);
    assert.equal(node.attrs['data-verdict'], word);
  }
});

test('each verdict has its own shape, so the badge survives being read in one colour', () => {
  const marks = WORDS.map((w) => markOf('verdict', w));
  assert.equal(new Set(marks.map((m) => m.id)).size, WORDS.length);
  assert.equal(new Set(marks.map((m) => m.means)).size, WORDS.length);
  const drawings = marks.map((m) => JSON.stringify(m.strokes));
  assert.equal(new Set(drawings).size, WORDS.length, 'two verdicts must not be drawn the same');
});

test('with the word withheld the shape still identifies the verdict', () => {
  const shapes = WORDS.map((w) => findByAttr(badge(w, { showWord: false }), 'data-mark')[0].attrs['data-mark']);
  assert.deepEqual(shapes, WORDS.map((w) => `verdict/${w}`));
  assert.equal(textOf(badge('Deny', { showWord: false })), '');
});

test('one meaning is red, and it is the mark that is red rather than the record', () => {
  assert.deepEqual(RED_MEANINGS, ['verdict.deny']);
  assert.equal(isRed(markOf('verdict', 'Deny')), true);
  assert.equal(isRed(markOf('verdict', 'Admit')), false);
  assert.equal(isRed(markOf('standing', 'cancelled')), false, 'a cancelled record is not a refusal');
  assert.equal(inkFor(markOf('verdict', 'Deny')), CONSUMED.deny);
  // Superseded by Owner #340 (2026-08-25): this line read CONSUMED.ink until the pass
  // that gave each standing its own hue. What the rule above still holds is the part
  // that matters -- red means refusal and nothing else -- and the two assertions below
  // are the ones that keep it true now that three other hues exist beside it.
  assert.equal(inkFor(markOf('verdict', 'Escalate')), CONSUMED.escalate);
  assert.equal(isRed(markOf('verdict', 'Escalate')), false);
  assert.equal(inkFor(markOf('effect', 'write')), CONSUMED.ink, 'an effect is not a standing and takes the plain ink');
});

test('each standing is a different pair of inks, and nothing outside the table gets one', () => {
  assert.deepEqual(STANDING_MEANINGS, ['verdict.admit', 'verdict.deny', 'verdict.escalate', 'standing.held']);
  const inks = STANDING_MEANINGS.map((m) => STANDING_INK[m].ink);
  const beds = STANDING_MEANINGS.map((m) => STANDING_INK[m].bed);
  assert.equal(new Set(inks).size, inks.length, 'two standings must not share an ink');
  assert.equal(new Set(beds).size, beds.length, 'two standings must not share a bed');
  // The accent belongs to "you may press this" and to nothing that is a standing.
  assert.equal(inks.includes(CONSUMED.act), false);
  assert.equal(beds.includes(CONSUMED.bedAct), false);
  // A mark with no standing of its own is drawn without a fill rather than on a
  // neutral one -- a filled chip is a claim of classification.
  assert.equal(bedFor(markOf('standing', 'cancelled')), null);
  assert.equal(bedFor(markOf('effect', 'undo')), null);
  assert.equal(bedFor(markOf('verdict', 'approved')), null, 'a word the engine never said is not given a standing');
});

test('a verdict chip is filled, and one the engine does not know is not', () => {
  for (const word of WORDS) {
    const node = badge(word);
    assert.equal(node.attrs['data-filled'], 'true', word);
    assert.match(node.attrs.style, /background:var\(--(admit|deny|escalate)-bed\)/, word);
  }
  const unknown = badge('approved');
  assert.equal(unknown.attrs['data-filled'], 'false');
  assert.equal(/background:/.test(unknown.attrs.style), false, 'no bed is invented for a word that is not a standing');
});

test('red-first: a meaning never sits on a node that draws no mark', () => {
  // The defect this pins shipped and was caught by a lane's probe, not by reading. Every
  // face's own tools/gate.mjs runs a `one-meaning-one-mark` tree check keyed on
  // `data-means`: for each node carrying a meaning it reads the `data-mark` beside it. A
  // wrapper carrying the meaning and no mark is visited first and pairs that meaning with
  // nothing, so the gate goes red on a screen that is drawing exactly one mark for it.
  //
  // The check below is the gate's own predicate, re-implemented here over the node tree
  // so this package fails before a face does.
  const pairs = (node) => find(node, (n) => n.attrs?.['data-means'] !== undefined && n.attrs['data-means'] !== null)
    .map((n) => ({ means: n.attrs['data-means'], mark: n.attrs['data-mark'] ?? null }));
  const broken = { attrs: { 'data-means': 'standing.held' }, tag: 'span', children: [] };
  assert.deepEqual(pairs(broken), [{ means: 'standing.held', mark: null }], 'the predicate detects the shape');

  for (const node of [
    chip('standing', 'held', { word: 'held' }),
    chip('verdict', 'Admit', { word: 'admitted' }),
    chip('effect', 'write', { word: 'wrote' }),
    badge('Deny'),
    badge('approved'),
  ]) {
    const orphans = pairs(node).filter((p) => p.mark === null);
    assert.deepEqual(orphans, [], `a meaning with no mark beside it: ${JSON.stringify(orphans)}`);
  }
});

test('the chip draws the same shape over a namespace that is not verdict', () => {
  const held = chip('standing', 'held', { word: 'held', said: 'this has not happened yet' });
  assert.equal(held.attrs['data-part'], 'standing-chip');
  assert.equal(held.attrs['data-standing'], 'held');
  assert.equal(held.attrs['data-filled'], 'true');
  assert.match(held.attrs.style, /background:var\(--held-bed\)/);
  assert.match(held.attrs.style, /color:var\(--held\)/);
  assert.equal(textOf(held), 'held');
  assert.equal(held.attrs.title, 'this has not happened yet');
  // A standing with no ink of its own keeps an edge instead, so it is still an object
  // on the screen rather than loose text.
  const cancelled = chip('standing', 'cancelled', { word: 'cancelled' });
  assert.equal(cancelled.attrs['data-filled'], 'false');
  assert.match(cancelled.attrs.style, /border:1px solid var\(--line\)/);
  assert.equal(find(cancelled, (n) => n.tag === 'svg').length, 1, 'the mark is still drawn');
  // The word is the caller's; nothing here invents one for a namespace it does not own.
  assert.equal(textOf(chip('effect', 'write', { word: 'wrote a file' })), 'wrote a file');
  assert.equal(textOf(chip('effect', 'write')), 'write');
});

test('red is spent on exactly one badge out of the three', () => {
  const reds = WORDS.filter((w) => badge(w).attrs.style.includes(CONSUMED.deny));
  assert.deepEqual(reds, ['Deny']);
});

test('a word the engine does not say is drawn as unknown, with the word that arrived kept in reach', () => {
  const node = badge('approved');
  assert.equal(node.attrs['data-verdict'], 'undefined');
  assert.equal(node.attrs['data-defined'], 'false');
  assert.equal(textOf(node), unrecognisedWord('approved'));
  assert.equal(/^undefined mark$/i.test(textOf(node)), false, 'a generic "undefined mark" throws away the word that arrived');
  assert.match(textOf(node), /"approved"/, 'the fallback names what actually arrived');
  assert.match(node.attrs.title, new RegExp(BADGE_MESSAGES.UNKNOWN_VERDICT));
  assert.match(node.attrs.title, /"approved"/);
});

test('an absent verdict is drawn as absent rather than as an empty box nobody notices', () => {
  for (const absent of [null, undefined, '']) {
    const node = badge(absent);
    assert.equal(node.attrs['data-defined'], 'false');
    assert.equal(find(node, (n) => n.tag === 'svg').length, 1, 'something is still drawn');
  }
});

test('every glyph the badge draws carries a size', () => {
  for (const size of [12, 14, 22]) {
    const svg = find(badge('Admit', { size }), (n) => n.tag === 'svg')[0];
    assert.equal(svg.attrs.width, String(size));
  }
});

// req/768 AC-4 retrofit round 2 found this: the badge sits as the one child of a
// flex cell (parts/src/receipt-row.mjs's `cell()`) whose available width shrank
// once faces/ledger and faces/held started drawing a fixed-width act gutter
// beside every row (tools/shoot.mjs measured a real overlap at the app's own
// 720px narrow viewport before this fix). A flex item's automatic minimum width
// is its content size unless it says otherwise -- this pins that it now does.
test('the badge states min-width:0 and overflow:hidden on its own box, so a narrow flex parent can shrink it below its content size', () => {
  const node = badge('Escalate');
  assert.match(node.attrs.style, /min-width:0/);
  assert.match(node.attrs.style, /overflow:hidden/);
});
