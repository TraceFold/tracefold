// SPDX-License-Identifier: Apache-2.0
// The shared surface layer: the rules a face cannot carry inline, and the three
// containers Owner #340's pass added on top of the pane this module already owned.
//
// This file is new. `parts/src/surface.mjs` shipped in retrofit r3 with no test of its
// own -- every claim about it was a claim about the faces that used it -- so the
// existing exports are pinned here alongside the new ones rather than the new ones
// being tested against an untested floor.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { el, textOf, findByAttr, find } from '../src/element.mjs';
import {
  SURFACE_CSS, SURFACE_ID, surfaceStyle, detailFrame, detailPane, PANE_MESSAGES,
  statBand, box, runtimeFooter, figureFor, STAT_DASH, SURFACE_MESSAGES, FOOTER_MESSAGES,
} from '../src/surface.mjs';
import { chip } from '../src/verdict-badge.mjs';
import { row, note, selectableRow, openableRow } from '../src/receipt-row.mjs';
import { glyph } from '../src/glyph-sheet.mjs';
import { BUILD } from '../generated/build.generated.mjs';

const seg = (noun, count, extra = {}) => ({ noun, count, ...extra });

// ---- the rules ------------------------------------------------------------------------

test('the rule set spells no colour of its own and is installed under one id', () => {
  assert.equal(surfaceStyle().attrs.id, SURFACE_ID);
  assert.deepEqual(SURFACE_CSS.match(/#[0-9a-fA-F]{3,8}\b|\brgba?\s*\(|\bhsla?\s*\(/g), null);
  assert.match(SURFACE_CSS, /scrollbar-width:thin/);
  // req/884 (Owner #423追記1): the scrollbar's geometry was written out here AND in
  // shell/kernel/shell.css, which is why two surfaces of one window could disagree about
  // it. Both now read the roster, so this pins the TOKEN rather than the number -- an
  // assertion on `8px` would have to be edited every time the value is retuned, which is
  // the same coupling the token exists to remove.
  assert.match(SURFACE_CSS, /::-webkit-scrollbar\{width:var\(--scrollbar-w\)/);
});

test('Owner #340 (5): every interactive part of a row states a pointer, a hover and a focus ring', () => {
  // Named individually rather than as one "the css mentions hover" assertion, because
  // each of these answers a specific control that gave no sign it was one.
  // req/884: a pointer is now stated as a token, not as the CSS keyword. The assertion
  // moves with it -- what it is testing ("this control says it is one") is unchanged, and
  // it additionally proves the control reads the one roster, which is what the gate at
  // tools/protocol_gate.mjs enforces from the outside.
  assert.match(SURFACE_CSS, /\[data-part="selectable-row"\][^{]*\{cursor:var\(--cursor-act\)\}|cursor:var\(--cursor-act\)/);
  assert.match(SURFACE_CSS, /\[data-part="selectable-row"\]:hover\{background:/);
  assert.match(SURFACE_CSS, /\[data-part="selectable-row"\]\[data-selected="true"\]\{background:var\(--act-bed\)/);
  // Matched on the rule, not on its exact selector list. These grew a second selector
  // when the role hook was added, and an assertion pinned to one spelling would have
  // failed on a change that made the rule reach MORE of what it is meant to reach.
  assert.match(SURFACE_CSS, /act-gutter"\] button:not\(\[disabled\]\):hover[^{]*\{background:var\(--act-bed\)\}/);
  assert.match(SURFACE_CSS, /act-gutter"\] button:not\(\[disabled\]\):active[^{]*\{/);
  assert.match(SURFACE_CSS, /act-gutter"\] button\[disabled\][^{]*\{cursor:var\(--cursor-refuse\)\}/);
  // And the role hook itself: any face's own act surface is on the same route, which is
  // what stops six right-click menus each spelling the accent inline.
  for (const rule of [/\[data-role="act"\]\{cursor:var\(--cursor-act\)\}/, /\[data-role="act"\]:not\(\[disabled\]\)[^{]*\{color:var\(--act\)/]) {
    assert.match(SURFACE_CSS, rule);
  }
  assert.match(SURFACE_CSS, /summary:hover\{background:/);
  assert.match(SURFACE_CSS, /:focus-visible\{outline:var\(--focus-w\) solid var\(--focus-ink\)/);
});

test('red-first: nothing this package draws spells inline what the rule set is meant to own', () => {
  // The defect this pins shipped, and it made six of the seven operability declarations
  // above inert on arrival: parts/src/receipt-row.mjs wrote `cursor`, `color`,
  // `background` and `border` as inline styles, and an inline declaration outranks a
  // stylesheet. Measured in a real window by the faces/ledger lane -- a live act button
  // drew the plain ink and a default cursor while these rules said accent and pointer --
  // and independently by the faces/atlas lane, which built a `no-inline-cursor` gate for
  // its own source on the same day. Two lanes finding one shape is what promoted it from
  // a slip to a rule.
  //
  // Two scopes, because the rule set claims two different amounts.
  //
  // `cursor` it claims everywhere: no part of this package spells one, the way no part
  // spells a colour. That is the whole of what an inline cursor was doing wrong.
  //
  // Ground and edge it claims only on the two parts it draws states for -- the
  // selectable row and the act gutter's buttons. Elsewhere a background is a legitimate
  // decision of the part that draws it: `row()` states `transparent` deliberately (see
  // the test below), and a filled verdict chip's bed IS the standing and belongs to
  // parts/src/verdict-badge.mjs's measured table, not to a hover rule. Scoping this to
  // the interactive parts is the difference between a gate and a blanket ban that would
  // have to be argued with every time it fired.
  const OWNED_EVERYWHERE = ['cursor'];
  const OWNED_ON_CONTROLS = ['background', 'border-color', 'border-left-color', 'outline'];
  const isControl = (n) => ['selectable-row', 'act-gutter'].includes(n.attrs?.['data-part'])
    || n.tag === 'button' || n.tag === 'summary';
  const spelled = (node) => find(node, (n) => typeof n.attrs?.style === 'string')
    .flatMap((n) => [...OWNED_EVERYWHERE, ...(isControl(n) ? OWNED_ON_CONTROLS : [])]
      .filter((prop) => new RegExp(`(^|;)\\s*${prop}\\s*:`).test(n.attrs.style))
      .map((prop) => `${n.attrs['data-part'] ?? n.tag}: ${prop}`));

  const planted = el('button', { 'data-part': 'act-gutter', style: 'cursor:default;background:var(--bg)' }, []);
  assert.deepEqual(
    spelled(planted).sort(),
    ['act-gutter: background', 'act-gutter: cursor'],
    'the predicate detects the shape that shipped',
  );
  // And it does not fire on the two legitimate grounds, so a pass here means something.
  assert.deepEqual(spelled(el('div', { 'data-part': 'receipt-note', style: 'background:var(--bg)' }, [])), []);

  const record = { id: 'r1', at: '2026-08-25T10:00:00Z', effect: 'write', verdict: 'Admit', path: '/a' };
  const acts = [
    { act: 'commit', label: 'commit', sends: true },
    { act: 'cancel', label: 'cancel', sends: false, why: 'not while it is held' },
  ];
  for (const [what, node] of [
    ['a selectable row with acts', selectableRow(record, { acts, fields: 3, selected: false })],
    ['a selected row', selectableRow(record, { acts, fields: 3, selected: true })],
    ['an openable row', openableRow(record, { note: [{ name: 'a', value: 'b' }], acts })],
    ['a plain row', row(record)],
  ]) {
    assert.deepEqual(spelled(node), [], `${what} spells a property the rule set owns`);
  }
});

test('a row draws no ground of its own, so the ground the rule set draws is visible', () => {
  // The other half of the same defect: the row line is a child that filled the whole of
  // the button, and it was opaque -- so the hover and selected grounds were painted over
  // before a reader could see either. The note keeps its own ground on purpose; that one
  // is the surviving half of the N-1 cure.
  const record = { id: 'r1', at: '2026-08-25T10:00:00Z', effect: 'write', verdict: 'Admit', path: '/a' };
  assert.match(row(record).attrs.style, /background:transparent/);
  assert.match(note([{ name: 'a', value: 'b' }]).attrs.style, /background:var\(--bg\)/);
});

test('the rule set states a ground and an edge for the parts that gave theirs up', () => {
  // Removing an inline declaration is only half a fix; if nothing states it instead, the
  // element inherits whatever is behind it. These are the base rules the modifiers above
  // override, and their absence would be a silent regression rather than a visible one.
  assert.match(SURFACE_CSS, /\[data-part="selectable-row"\]\{background:var\(--bg\);border-left-color:transparent\}/);
  assert.match(SURFACE_CSS, /act-gutter"\] button[^{]*\{background:var\(--bg\);border:1px solid var\(--line\);color:var\(--ink-2\)\}/);
});

test('the accent ink marks only what a hand may do, and never a standing', () => {
  // The rule the whole colour pass rests on: a coloured area is either a standing or an
  // invitation, and there is no third meaning. Every --act site in these rules is a
  // cursor, a hover, a press, a selection or a focus ring.
  const actLines = SURFACE_CSS.split('\n').filter((line) => /var\(--act(-bed)?\)/.test(line));
  assert.ok(actLines.length >= 4, 'the accent is actually spent');
  for (const line of actLines) {
    assert.match(line, /selectable-row|act-gutter|focus-visible/, line);
  }
});

test('the base text size in these rules is at the stated floor, including the small label', () => {
  // req/38 SS693 flagged .gx-label at 12px against tokens.css's own 14px body floor and
  // left it to this lane. Ruling taken here: the floor is stated as a hard requirement
  // and a label under a figure is read, so it takes the record size like everything
  // else. The figure above it is what carries the size hierarchy.
  assert.match(SURFACE_CSS, /\.gx-label\{[^}]*font-size:var\(--t-record\)/);
  assert.equal(/\.gx-label\{[^}]*font-size:12px/.test(SURFACE_CSS), false);
});

// ---- the pane (pre-existing, pinned here for the first time) ---------------------------

test('the detail pane is the one container in this package that scrolls, and it is bounded', () => {
  const pane = detailPane({});
  assert.match(pane.attrs.style, /max-height:520px/);
  assert.match(pane.attrs.style, /overflow-y:auto/);
  assert.equal(pane.attrs['data-count'], '0');
  assert.equal(textOf(pane).includes(PANE_MESSAGES.NOTHING), true, 'an empty pane says so');
});

test('the list keeps its own geometry whatever the pane holds', () => {
  const frame = detailFrame(el('div', {}, []), detailPane({ subject: 'r1', lines: [{ name: 'a', value: 'b' }] }));
  assert.equal(frame.attrs['data-part'], 'list-with-detail');
  assert.match(frame.attrs.style, /flex-wrap:wrap/);
  assert.equal(findByAttr(frame, 'data-part', 'detail-pane').length, 1, 'exactly one pane');
});

// ---- the stat band ---------------------------------------------------------------------

test('red-first: a number with no noun beside it is refused, in every shape the omission takes', () => {
  for (const bad of [{ count: 3 }, { count: 3, noun: '' }, { count: 3, noun: '   ' }, { count: 3, noun: 7 }, null]) {
    assert.throws(() => statBand([bad]), (error) => {
      assert.ok(error.message.startsWith(SURFACE_MESSAGES.NO_NOUN), error.message);
      return true;
    }, JSON.stringify(bad));
  }
});

test('a band draws equal columns, one figure and one noun each', () => {
  const band = statBand([seg('records', 12), seg('held', 3), seg('inverses held', 0)]);
  assert.equal(band.attrs['data-count'], '3');
  assert.match(band.attrs.style, /grid-template-columns:repeat\(3, ?minmax\(0,1fr\)\)/);
  const cells = findByAttr(band, 'data-role', 'segment');
  assert.equal(cells.length, 3);
  assert.deepEqual(cells.map((c) => c.attrs['data-noun']), ['records', 'held', 'inverses held']);
  assert.deepEqual(cells.map((c) => textOf(findByAttr(c, 'data-role', 'figure')[0])), ['12', '3', '0']);
  assert.deepEqual(cells.map((c) => textOf(findByAttr(c, 'data-role', 'noun')[0])), ['records', 'held', 'inverses held']);
});

test('zero is drawn and an unread count is a dash, because they are different facts', () => {
  const band = statBand([seg('measured empty', 0), seg('not read', null)]);
  const cells = findByAttr(band, 'data-role', 'segment');
  assert.equal(textOf(findByAttr(cells[0], 'data-role', 'figure')[0]), '0');
  assert.equal(cells[0].attrs['data-value'], '0');
  assert.equal(textOf(findByAttr(cells[1], 'data-role', 'figure')[0]), STAT_DASH);
  assert.equal(cells[1].attrs['data-value'], 'unread');
  assert.notEqual(STAT_DASH, '0');
});

test('a band places the mark and the tone it is handed and chooses neither', () => {
  const band = statBand([{
    noun: 'denied', count: 2, mark: glyph('verdict', 'Deny', { size: 16, label: 'denied' }), tone: 'var(--deny)',
  }]);
  assert.equal(find(band, (n) => n.tag === 'svg').length, 1);
  assert.match(findByAttr(band, 'data-role', 'figure')[0].attrs.style, /color:var\(--deny\)/);
  // A segment with no tone takes the figure class's own ink and states no colour.
  const plain = statBand([seg('records', 1)]);
  assert.equal(/color:/.test(findByAttr(plain, 'data-role', 'figure')[0].attrs.style ?? ''), false);
});

// ---- the box ----------------------------------------------------------------------------

test('red-first: a box without a name or without a count is refused', () => {
  assert.throws(() => box({ count: 0 }), (e) => e.message.startsWith(SURFACE_MESSAGES.NO_NAME));
  assert.throws(() => box({ name: '  ', count: 0 }), (e) => e.message.startsWith(SURFACE_MESSAGES.NO_NAME));
  assert.throws(() => box({ name: 'settled' }), (e) => e.message.startsWith(SURFACE_MESSAGES.NO_COUNT));
});

test('a box states its own count in its own head, and an empty box keeps its border', () => {
  const empty = box({ name: 'held', count: 0, noun: 'candidates' });
  assert.equal(empty.attrs['data-count'], '0');
  assert.match(textOf(findByAttr(empty, 'data-role', 'box-count')[0]), /^0 candidates$/);
  assert.match(empty.attrs.style, /border:1px solid var\(--line\)/, 'an empty group does not vanish');
  const unread = box({ name: 'attached', count: null, noun: 'repositories' });
  assert.equal(unread.attrs['data-count'], STAT_DASH);
  assert.match(textOf(findByAttr(unread, 'data-role', 'box-count')[0]), /^-- repositories$/);
});

test('a box places the standing pill it is handed and decides no standing', () => {
  const held = box({
    name: 'held', count: 3, noun: 'candidates', pill: chip('standing', 'held', { word: 'held' }),
  });
  const pill = findByAttr(held, 'data-part', 'standing-chip');
  assert.equal(pill.length, 1);
  assert.equal(pill[0].attrs['data-standing'], 'held');
  // The meaning is read off the mark, where the mark is -- see the red-first case in
  // parts/test/verdict-badge.test.mjs for why it is not on the wrapper.
  assert.equal(findByAttr(pill[0], 'data-means', 'standing.held').length, 1);
  assert.equal(findByAttr(box({ name: 'settled', count: 1 }), 'data-part', 'standing-chip').length, 0);
});

test('a box does not scroll: it grows to what is in it', () => {
  const held = box({ name: 'settled', count: 2, children: [el('div', {}, ['a'])] });
  assert.equal(/overflow/.test(held.attrs.style), false);
  assert.equal(/max-height/.test(held.attrs.style), false);
  assert.equal(textOf(held).includes('a'), true, 'the children are in it');
});

// ---- the footer --------------------------------------------------------------------------

test('the footer states four fields and draws a dash for anything it did not measure', () => {
  const bare = runtimeFooter({ build: {} });
  const fields = findByAttr(bare, 'data-role', 'footer-field');
  assert.deepEqual(fields.map((f) => f.attrs['data-name']), ['render', 'read', 'suite', 'build']);
  for (const field of fields) assert.match(textOf(field), /--$/, field.attrs['data-name']);
  for (const field of fields) assert.equal(/ 0$/.test(textOf(field)), false, 'never a fabricated zero');
  assert.equal(fields[0].attrs.title, FOOTER_MESSAGES.UNREAD);
});

test('a measured render cost is printed as measured, at a precision that cannot round it away', () => {
  const shot = runtimeFooter({ renderMs: 3.14159, source: 'fixture', build: {} });
  const fields = findByAttr(shot, 'data-role', 'footer-field');
  assert.equal(textOf(fields[0]), 'render 3.1 ms');
  assert.equal(shot.attrs['data-render-ms'], '3.14159', 'the unrounded reading survives for an instrument');
  assert.equal(textOf(fields[1]), 'read fixture');
  // Zero is a measurement and is printed, unlike the dash above -- and printed bare, so
  // it does not read as something that was rounded.
  assert.equal(textOf(findByAttr(runtimeFooter({ renderMs: 0, build: {} }), 'data-role', 'footer-field')[0]), 'render 0 ms');
});

test('red-first: no non-zero reading is ever drawn as a zero', () => {
  // The defect this pins shipped: toFixed(1) turned every sub-0.05ms paint into
  // "render 0.0 ms", on a footer whose own doc comment forbids a fabricated zero. These
  // are real magnitudes -- the faces on this tree build their trees in tens of
  // microseconds.
  const rounding = (ms) => `${ms.toFixed(1)} ms`;
  for (const ms of [0.0001, 0.006, 0.032, 0.049]) {
    assert.equal(rounding(ms), '0.0 ms', `the old form drew ${ms} as a zero`);
    assert.equal(figureFor(ms) === '0', false, `${ms} must not be drawn as a zero`);
    assert.equal(Number(figureFor(ms)) > 0, true, ms);
  }
  assert.equal(figureFor(0.032), '0.032');
  assert.equal(figureFor(0.6), '0.6');
  assert.equal(figureFor(0.9), '0.9');
  assert.equal(figureFor(7.3), '7.3');
  assert.equal(figureFor(1), '1.0');
  assert.equal(figureFor(0), '0', 'an exact zero is a measurement and reads as one');
  assert.equal(textOf(findByAttr(runtimeFooter({ renderMs: 0.032, build: {} }), 'data-role', 'footer-field')[0]), 'render 0.032 ms');
});

test('the build fields come from the generated stamp, and an uncommitted tree says so', () => {
  const real = runtimeFooter({ renderMs: 1, source: 'fixture' });
  const fields = findByAttr(real, 'data-role', 'footer-field');
  assert.equal(real.attrs['data-build'], BUILD.commit);
  assert.match(textOf(fields[2]), new RegExp(`^suite ${BUILD.suiteTests} tests / ${BUILD.suiteFailed} failed$`));
  assert.match(textOf(fields[3]), new RegExp(`^build ${BUILD.commit}`));
  assert.equal(textOf(fields[3]).includes('+changes'), Boolean(BUILD.dirty));
  const clean = runtimeFooter({ build: { commit: 'abc1234', dirty: false, at: 'then' } });
  assert.equal(textOf(findByAttr(clean, 'data-role', 'footer-field')[3]), 'build abc1234');
  const dirty = runtimeFooter({ build: { commit: 'abc1234', dirty: true, at: 'then' } });
  assert.equal(textOf(findByAttr(dirty, 'data-role', 'footer-field')[3]), 'build abc1234 +changes');
  assert.equal(findByAttr(dirty, 'data-role', 'footer-field')[3].attrs.title, FOOTER_MESSAGES.DIRTY);
});

test('the footer is one line high at the stated floor, so it costs a strip and not a section', () => {
  const foot = runtimeFooter({ build: {} });
  assert.match(foot.attrs.style, /min-height:20px/);
  assert.match(foot.attrs.style, /font-size:var\(--t-record\)/);
  assert.equal(/max-height|overflow/.test(foot.attrs.style), false);
});
