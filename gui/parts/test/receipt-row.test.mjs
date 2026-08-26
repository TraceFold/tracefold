// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { el, toHtml, textOf, findByAttr, find } from '../src/element.mjs';
import {
  row, note, receiptRow, openableRow, positionedNodes, COLUMNS, ROW_MESSAGES, GUTTER_WIDTH, rowWithGutter,
  drawnAt, drawnTextFor,
} from '../src/receipt-row.mjs';
import { claimOf } from '../src/seal-claim.mjs';
import { reversalOf } from '../src/reversibility.mjs';
import { CONSUMED } from '../src/tokens.mjs';

const RECORD = {
  id: 'r-01', n: 1, at: '09:14:02', actor: 'agent/packer', effect: 'wrote',
  path: 'src/index.mjs', verdict: 'Admit', basis: 'exact',
};
const VERIFIER = { name: 'gx-verify' };
const CLAIM = claimOf(RECORD, { verifier: VERIFIER });

test('a row without an identity is refused, because it could not be pointed at afterwards', () => {
  assert.throws(() => row({ at: '09:00:00' }), new RegExp(ROW_MESSAGES.NEEDS_ID));
  assert.throws(() => row({ id: '' }), new RegExp(ROW_MESSAGES.NEEDS_ID));
  assert.throws(() => row(null), new RegExp(ROW_MESSAGES.NEEDS_ID));
});

test('nothing in a row or its note is taken out of flow, which is what N-1 needed to happen', () => {
  const group = receiptRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'a'.repeat(400) }], open: true });
  assert.deepEqual(positionedNodes(group), []);
});

test('the out-of-flow gate refuses a planted absolute, so its silence means something', () => {
  const planted = el('div', { style: 'position:absolute;top:0' }, ['note']);
  assert.equal(positionedNodes(planted).length, 1);
  assert.equal(positionedNodes(el('div', { style: 'position:fixed' })).length, 1);
  assert.equal(positionedNodes(el('div', { style: 'position:static' })).length, 0);
});

test('the note carries its own opaque background, so a stacking mistake still leaves one text readable', () => {
  const block = note([{ name: 'why', value: 'because' }]);
  assert.ok(block.attrs.style.includes(`background:${CONSUMED.page}`));
  assert.equal(block.attrs.style.includes('position'), false);
  assert.match(block.attrs.style, /white-space:normal/, 'the note is the one thing allowed to wrap');
});

test('the note follows its row as a sibling, so rows below it move down', () => {
  const group = receiptRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'because' }], open: true });
  assert.equal(group.children.length, 2);
  assert.equal(group.children[0].attrs['data-part'], 'receipt-row');
  assert.equal(group.children[1].attrs['data-part'], 'receipt-note');
  assert.equal(group.attrs['data-open'], 'true');
});

test('a shut note is absent rather than present and hidden', () => {
  const group = receiptRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'because' }], open: false });
  assert.equal(group.children.length, 1);
  assert.equal(group.attrs['data-open'], 'false');
});

test('the row keeps one pitch and clips instead of growing', () => {
  const long = { ...RECORD, path: 'build/'.repeat(60) };
  const node = row(long, { claim: CLAIM });
  assert.ok(node.attrs.style.includes(`height:${CONSUMED.pitch}`));
  assert.match(node.attrs.style, /overflow:hidden/);
  const pathCell = findByAttr(node, 'data-cell', 'path')[0];
  assert.match(pathCell.attrs.style, /text-overflow:ellipsis/);
  assert.match(pathCell.attrs.style, /white-space:nowrap/);
});

test('the eight columns are declared once and every one of them appears', () => {
  const node = row(RECORD, { claim: CLAIM });
  const cells = COLUMNS.map((c) => findByAttr(node, 'data-cell', c.key));
  for (const [i, hit] of cells.entries()) assert.equal(hit.length, 1, `column ${COLUMNS[i].key}`);
  assert.equal(COLUMNS.length, 8);
  assert.deepEqual(COLUMNS.map((c) => c.key), ['lifecycle', 'at', 'actor', 'effect', 'verdict', 'fingerprint', 'seal', 'path']);
  for (const column of COLUMNS) assert.ok(column.role.length > 2, `${column.key} states what it is for`);
});

test('composition A: the kind of change carries a glyph next to its word, not the word alone', () => {
  const node = row(RECORD, { claim: CLAIM });
  const effectCellNode = findByAttr(node, 'data-cell', 'effect')[0];
  assert.equal(find(effectCellNode, (n) => n.tag === 'svg').length, 1);
  assert.ok(textOf(effectCellNode).includes('wrote'));
});

test('composition A: a held row carries a lifecycle mark where a settled row carries none', () => {
  const held = row({ ...RECORD, lifecycle: 'held' }, { claim: CLAIM });
  const settled = row({ ...RECORD, lifecycle: 'settled' }, { claim: CLAIM });
  assert.equal(find(held, (n) => n.attrs['data-mark'] === 'standing/held').length, 1);
  assert.equal(find(settled, (n) => n.attrs['data-mark'] === 'standing/held').length, 0);
});

test('composition A: the fingerprint cell carries the digest this delta left, cut and named as a cut', () => {
  const node = row({ ...RECORD, digest: 'a1b2c3d4e5f60718' }, { claim: CLAIM });
  const fp = findByAttr(node, 'data-cell', 'fingerprint')[0];
  assert.equal(textOf(fp), 'A1B2C3');
  assert.match(fp.attrs.title, /first 6 of 16 hexadecimal characters/);
});

test('a cell with no value and a cell declared missing are different things on the screen', () => {
  const withHole = { ...RECORD, actor: undefined, holes: { effect: 'the issuer did not record it' } };
  const node = row(withHole, { claim: CLAIM });
  assert.equal(findByAttr(node, 'data-cell', 'actor')[0].attrs['data-state'], 'slot');
  const hole = findByAttr(node, 'data-cell', 'effect')[0];
  assert.equal(hole.attrs['data-state'], 'hole');
  assert.match(hole.attrs.title, /the issuer did not record it/);
  assert.equal(find(hole, (n) => n.attrs['data-mark'] === 'structure/hole').length, 1);
});

test('a row given no seal claim says so, rather than drawing an unsealed record as plain', () => {
  const node = row(RECORD, {});
  const seal = findByAttr(node, 'data-cell', 'seal')[0];
  assert.equal(seal.attrs['data-state'], 'hole');
  assert.match(seal.attrs.title, new RegExp(ROW_MESSAGES.NO_CLAIM));
});

test('the seal cell places the mark the claim chose and makes no choice of its own', () => {
  const sealed = row(RECORD, { claim: claimOf(RECORD, { verifier: VERIFIER }) });
  const unsealed = row(RECORD, { claim: claimOf({ ...RECORD, basis: 'derived' }, { verifier: VERIFIER }) });
  assert.equal(find(sealed, (n) => n.attrs['data-mark'] === 'structure/seal').length, 1);
  assert.equal(find(unsealed, (n) => n.attrs['data-mark'] === 'structure/unsealed').length, 1);
});

test('undo is a row written under another, and the first row is not touched', () => {
  const parent = toHtml(row(RECORD, { claim: CLAIM }));
  const child = row({ ...RECORD, id: 'r-05', childOf: 'r-01', at: '09:15:31' }, { claim: CLAIM });
  assert.equal(toHtml(row(RECORD, { claim: CLAIM })), parent, 'building the child changed nothing about the parent');
  assert.equal(child.attrs['data-child-of'], 'r-01');
  assert.equal(find(child, (n) => n.attrs['data-mark'] === 'structure/child').length, 1);
  assert.equal(row(RECORD, { claim: CLAIM }).attrs['data-child-of'], undefined);
});

test('every glyph inside a row was asked for at a size', () => {
  const group = receiptRow({ ...RECORD, childOf: 'r-00', holes: { effect: 'unrecorded' } }, { claim: CLAIM, note: [{ name: 'a', value: 'b' }], open: true });
  const svgs = find(group, (n) => n.tag === 'svg');
  assert.ok(svgs.length >= 4);
  for (const svg of svgs) {
    assert.match(svg.attrs.width ?? '', /^\d+$/);
    assert.match(svg.attrs.style, /width:\d+px/);
  }
});

test('no colour is spelled anywhere in a drawn row', () => {
  const html = toHtml(receiptRow(RECORD, { claim: CLAIM, note: [{ name: 'a', value: 'b' }], open: true }));
  assert.equal(/#[0-9a-fA-F]{3,8}\b/.test(html.replace(/href="#[^"]*"/g, '')), false);
  assert.equal(/rgba?\(/.test(html), false);
});

test('the note prints what it was given and does not summarise it away', () => {
  const long = 'the packer asked to remove a build artifact that the standing rule holds for review';
  const block = note([{ name: 'why', value: long }], { summary: 'held by a standing rule' });
  assert.ok(textOf(block).includes(long));
  assert.ok(textOf(block).includes('held by a standing rule'));
  assert.equal(block.attrs['data-count'], '1');
});

// -- openableRow: SS657 retrofit (req/768 F-A, collapsed-by-default + counted
// disclosure) -- a second, additive export. receiptRow() above is unchanged and
// still used wherever a caller wants the old, non-interactive shape (faces/atlas's
// own per-touch rows nested inside its own bespoke subject <details>).

test('openableRow wraps a row with a note in a native <details>, so a reader can open or close it by hand', () => {
  const group = openableRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'because' }], open: false });
  assert.equal(group.tag, 'details');
  assert.equal(group.attrs['data-open'], 'false');
  assert.equal(group.attrs.open, undefined, 'closed by default means no open attribute is written');
  assert.equal(find(group, (n) => n.tag === 'summary').length, 1);
});

test('openableRow opened by construction carries the open attribute, and the row line is still findable inside the summary', () => {
  const group = openableRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'because' }], open: true });
  assert.equal(group.attrs.open, '', 'a boolean HTML attribute serialises as the empty string, the same convention every other <details open> in this tree uses');
  assert.equal(findByAttr(group, 'data-part', 'receipt-row').length, 1, 'the row line lives inside the summary, always visible');
  assert.equal(findByAttr(group, 'data-part', 'receipt-note').length, 1);
});

test('openableRow with nothing to disclose draws no details wrapper at all -- there is nothing to open', () => {
  const group = openableRow(RECORD, { claim: CLAIM, note: [], open: false });
  assert.equal(group.tag, 'div');
  assert.equal(group.attrs['data-withholds'], '0');
  assert.equal(find(group, (n) => n.tag === 'details').length, 0);
});

test('openableRow states the count of what it withholds on the control itself -- never a silent chevron', () => {
  const group = openableRow(RECORD, { claim: CLAIM, note: [{ name: 'a', value: '1' }, { name: 'b', value: '2' }, { name: 'c', value: '3' }], open: false });
  assert.equal(group.attrs['data-withholds'], '3');
  const badge = findByAttr(group, 'data-role', 'withheld-count')[0];
  assert.ok(badge, 'a withheld-count node is drawn in the summary');
  assert.match(textOf(badge), /3 more field/);
});

test('openableRow keeps nothing out of flow, the same N-1 guard receiptRow holds', () => {
  const group = openableRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'a'.repeat(400) }], open: true });
  assert.deepEqual(positionedNodes(group), []);
});

// -- retrofit round 2 (req/768 AC-4/AC-6/AC-7): the reversibility chip and the
// act gutter, both additive and both opt-in. atlas's own receiptRow() call sites
// never pass `reversal` or `acts` -- the two red-first pins directly below hold
// that at zero: a caller that asks for neither gets back exactly the tree
// openableRow already produced before this round existed.

test('red-first: openableRow with neither reversal nor acts renders byte-identical to the pre-retrofit-2 shape', () => {
  const before = toHtml(openableRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'because' }], open: true }));
  const after = toHtml(openableRow(RECORD, {
    claim: CLAIM, note: [{ name: 'why', value: 'because' }], open: true, reversal: null, acts: null,
  }));
  assert.equal(after, before);
});

test('red-first: the no-note branch is also byte-identical when neither option is passed', () => {
  const before = toHtml(openableRow(RECORD, { claim: CLAIM, note: [], open: false }));
  const after = toHtml(openableRow(RECORD, { claim: CLAIM, note: [], open: false, reversal: null, acts: null }));
  assert.equal(after, before);
});

test('AC-7: a reversal fact draws a self-evident chip -- glyph and word together, never a bare glyph', () => {
  const fact = reversalOf({ id: 't-001', lifecycle: 'settled' }, [{ id: 't-002', lifecycle: 'settled', childOf: 't-001' }]);
  const group = openableRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'because' }], open: false, reversal: fact });
  const chip = findByAttr(group, 'data-part', 'reversal-chip')[0];
  assert.ok(chip, 'a reversal chip is drawn');
  assert.equal(chip.attrs['data-state'], 'reversed');
  assert.equal(find(chip, (n) => n.tag === 'svg').length, 1, 'a glyph');
  assert.ok(textOf(chip).includes('reversed'), 'and a word beside it');
  assert.match(chip.attrs.title, /t-002/, 'the full honest reason names the reversing row, reachable on hover');
});

test('AC-7: the three states each draw their own declared mark, never the same mark for two meanings', () => {
  const reversed = reversalOf({ id: 'a', lifecycle: 'settled' }, [{ id: 'b', lifecycle: 'settled', childOf: 'a' }]);
  const notObservable = reversalOf({ id: 'a', lifecycle: 'settled' }, []);
  const notCommitted = reversalOf({ id: 'a', lifecycle: 'held' }, []);
  const marks = [reversed, notObservable, notCommitted].map((fact) => {
    const group = openableRow(RECORD, { claim: CLAIM, note: [], open: false, reversal: fact });
    const svg = find(group, (n) => n.tag === 'svg' && typeof n.attrs['data-mark'] === 'string' && n.attrs['data-mark'].startsWith('standing/'))[0];
    return svg?.attrs['data-mark'];
  });
  assert.deepEqual(marks, ['standing/reversed', 'standing/none', 'standing/held']);
  assert.equal(new Set(marks).size, 3, 'three distinct facts, three distinct marks');
});

test('AC-4: a row with acts offered draws a fixed-width right gutter, never a full-width strip underneath', () => {
  const acts = [{ act: 'commit', label: 'commit', sends: true }, { act: 'cancel', label: 'cancel', sends: true }];
  const frame = openableRow(RECORD, { claim: CLAIM, note: [], open: false, acts });
  assert.equal(frame.attrs['data-part'], 'row-gutter-frame');
  const gutter = findByAttr(frame, 'data-part', 'act-gutter')[0];
  assert.ok(gutter, 'a gutter is drawn');
  assert.match(gutter.attrs.style, new RegExp(`width:${GUTTER_WIDTH}`));
  assert.equal(gutter.attrs['data-count'], '2');
  const buttons = find(gutter, (n) => n.tag === 'button');
  assert.equal(buttons.length, 2);
});

test('AC-4: an unavailable act still draws a visibly-disabled slot with its reason, never blank space', () => {
  const acts = [{ act: 'escalate', label: 'escalate', sends: false, why: 'the body was never read' }];
  const frame = openableRow(RECORD, { claim: CLAIM, note: [], open: false, acts });
  const button = find(frame, (n) => n.tag === 'button')[0];
  assert.equal(button.attrs.disabled, '', 'a boolean HTML attribute serialises as the empty string, the same convention <details open> uses');
  assert.equal(button.attrs.title, 'the body was never read');
});

test('AC-4: no acts offered draws no gutter at all -- rowWithGutter is a no-op on a null gutter', () => {
  const withoutActs = openableRow(RECORD, { claim: CLAIM, note: [], open: false, acts: [] });
  assert.notEqual(withoutActs.attrs['data-part'], 'row-gutter-frame');
  assert.equal(rowWithGutter(row(RECORD, { claim: CLAIM }), null).attrs['data-part'], 'receipt-row');
});

test('the gutter and the chip together keep nothing out of flow', () => {
  const acts = [{ act: 'commit', label: 'commit', sends: true }];
  const fact = reversalOf({ id: 't-001', lifecycle: 'settled' }, []);
  const group = openableRow(RECORD, { claim: CLAIM, note: [{ name: 'why', value: 'a'.repeat(400) }], open: true, reversal: fact, acts });
  assert.deepEqual(positionedNodes(group), []);
});

// -- the declared cut in the `at` column (req/97 gap-list item gap 1) ---------------------

test('the time column draws a declared cut of an ISO-8601 timestamp, and keeps the whole of it on the cell', () => {
  const node = row({ ...RECORD, at: '2026-08-24T09:01:00Z' }, { claim: CLAIM });
  const atNode = findByAttr(node, 'data-cell', 'at')[0];
  assert.equal(textOf(atNode), '09:01:00', 'the cell draws the time of day');
  assert.equal(atNode.attrs['data-cut'], 'true');
  assert.equal(atNode.attrs['data-full'], '2026-08-24T09:01:00Z', 'the whole timestamp is still on the cell');
  assert.ok(atNode.attrs.title.includes('2026-08-24T09:01:00Z'), 'and reachable by pointing at it');
  assert.ok(atNode.attrs.title.includes(ROW_MESSAGES.AT_FORM), 'named as a declared form, not left to be guessed at');
});

test('drawnTextFor answers with what the row will draw, which is the only thing a width budget can be measured against', () => {
  assert.equal(drawnTextFor('at', '2026-08-24T09:01:00Z'), '09:01:00');
  assert.equal(drawnAt('2026-08-24T09:01Z'), '09:01', 'a timestamp without seconds keeps the form it arrived in');
  for (const key of ['actor', 'effect', 'verdict', 'path']) {
    assert.equal(drawnTextFor(key, 'a'.repeat(40)), 'a'.repeat(40), `${key} is drawn whole, so its budget is measured whole`);
  }
});

test('negative control: a value the cut cannot read is handed back whole, so a real overflow is still a real overflow', () => {
  // The cure for the forced-open row must not become a blanket excuse. Anything
  // that is not an ISO-8601 timestamp passes through untouched and therefore still
  // measures long, which is what keeps a face's clip predicate able to fire at all.
  const odd = 'yesterday, late afternoon, about tea time';
  assert.equal(drawnAt(odd), odd);
  assert.equal(drawnTextFor('at', odd).length, odd.length);
  const node = row({ ...RECORD, at: odd }, { claim: CLAIM });
  const atNode = findByAttr(node, 'data-cell', 'at')[0];
  assert.equal(atNode.attrs['data-cut'], 'false', 'nothing was cut, and the cell says so');
  assert.equal(textOf(atNode), odd);
});
