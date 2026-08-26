// SPDX-License-Identifier: Apache-2.0
// C5 -- the crease where provenance is folded away.
//
// Two halves, and they are not the same kind of thing. What is settled has happened
// and can be pointed at; what is held has not happened and must never be drawn
// wearing a receipt's face. They get different words, different marks, and different
// halves, and neither half is omitted when it is empty -- an absent half reads as
// "no such thing exists" when the truth is "nothing is in it yet" (req/04 C5).
//
// The requirement the tree this replaces stated but never checked: a fold is an
// aside, so removing the whole fold must leave the claims intact. That was written
// as "structural check" and no such check was ever written -- the shipped checks
// looked for the ids and never asked whether they sat inside the fold or outside it
// (req/04a C5, defect 1). `withoutFolds` is that check, and the test runs it.
//
// A second <details> elsewhere on a page is not this part. It carries a different
// payload and a different contract, so this one is stamped `data-kind="provenance"`
// and can be told apart by machine (req/04a C5, defect 2).

import { el, style } from './element.mjs';
import { glyph } from './glyph-sheet.mjs';
import { CONSUMED } from './tokens.mjs';

export const FOLD_MESSAGES = {
  SETTLED_EMPTY: 'nothing has settled here yet',
  HELD_EMPTY: 'nothing is being held here',
  NEEDS_SUMMARY: 'a fold states what it holds before it is opened',
  FOLDED: 'provenance folded',
};

export const FOLD_KIND = 'provenance';

/** The two halves, named once. The words differ because the things differ. */
export const HALVES = Object.freeze([
  { key: 'settled', label: 'settled', mark: ['structure', 'seal'], empty: FOLD_MESSAGES.SETTLED_EMPTY },
  { key: 'held', label: 'held', mark: ['standing', 'held'], empty: FOLD_MESSAGES.HELD_EMPTY },
]);

function line(item) {
  return el('div', {
    'data-role': 'entry',
    style: style({
      display: 'grid', 'grid-template-columns': 'minmax(0,10rem) minmax(0,1fr)', gap: '10px',
      padding: '3px 0', 'align-items': 'baseline',
    }),
  }, [
    el('span', { 'data-role': 'entry-name', style: style({ color: CONSUMED.attendant }) }, [String(item.name)]),
    el('span', { 'data-role': 'entry-value', style: style({ color: CONSUMED.ink, 'overflow-wrap': 'anywhere' }) }, [String(item.value)]),
  ]);
}

function half(spec, items, size) {
  const rows = Array.isArray(items) ? items : [];
  return el('section', {
    'data-role': 'half',
    'data-half': spec.key,
    'data-count': String(rows.length),
    style: style({ padding: '8px 0' }),
  }, [
    el('h4', {
      style: style({
        display: 'flex', 'align-items': 'center', gap: '6px', margin: '0 0 4px',
        // SS558 body-text floor: was CONSUMED.meta (12px), now CONSUMED.record (14px).
        color: CONSUMED.ink, 'font-family': CONSUMED.sans, 'font-size': CONSUMED.record,
        'line-height': CONSUMED.recordLine, 'font-weight': '600',
      }),
    }, [glyph(spec.mark[0], spec.mark[1], { size, label: spec.label }), el('span', {}, [spec.label])]),
    rows.length === 0
      ? el('p', {
        'data-role': 'empty',
        style: style({ margin: '0', color: CONSUMED.attendant, 'font-style': 'normal' }),
      }, [spec.empty])
      : el('div', { 'data-role': 'entries' }, rows.map(line)),
  ]);
}

/**
 * The fold. `summary` says what is inside before anyone opens it; a fold whose
 * handle does not say what it hides is a fold nobody opens.
 */
export function fold({ summary, settled = [], held = [], open = false, size = 12 } = {}) {
  if (typeof summary !== 'string' || summary.trim() === '') throw new Error(FOLD_MESSAGES.NEEDS_SUMMARY);
  return el('details', {
    'data-part': 'provenance-fold',
    'data-kind': FOLD_KIND,
    open: open ? 'open' : null,
    style: style({
      // SS558 body-text floor: was CONSUMED.meta (12px), now CONSUMED.record (14px).
      'font-family': CONSUMED.sans, 'font-size': CONSUMED.record, 'line-height': CONSUMED.recordLine,
      color: CONSUMED.ink, background: CONSUMED.page,
      'border-top': `1px solid ${CONSUMED.rule}`, padding: '6px 0',
    }),
  }, [
    el('summary', {
      // SS553: every interactive control is at least 36x36px, machine-checked by
      // tapTargets() in tools/shoot.mjs.
      style: style({
        cursor: 'default', color: CONSUMED.attendant, 'list-style': 'revert', display: 'flex', 'align-items': 'center', 'min-height': '36px', 'box-sizing': 'border-box',
      }),
    }, [summary]),
    el('div', { 'data-role': 'halves' }, HALVES.map((spec) => half(spec, spec.key === 'settled' ? settled : held, size))),
  ]);
}

const FOLD_BLOCK = /<details\b[^>]*data-kind="provenance"[^>]*>[\s\S]*?<\/details>/gi;

/**
 * The page with every provenance fold cut out of it. What remains has to still say
 * everything the page claims -- if a claim disappears with the fold, the claim was
 * living inside an aside.
 */
export function withoutFolds(html) {
  return String(html).replace(FOLD_BLOCK, '');
}
