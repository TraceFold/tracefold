// SPDX-License-Identifier: Apache-2.0
// What the face does, asked of the face and not of a description of it.
//
// The one thing these tests exist to guard above everything else: this screen may
// never draw an edge whose far end this window did not actually read. Every other
// test here is either a property this face carries over from faces/ledger's/
// faces/receipt's shared discipline (fail-closed, no colour, every glyph sized) or
// the ordinary declaration/mark/gate checks every face in this tree holds.
//
// stub-port.mjs and dom-stand-in.mjs are read from faces/ledger/test/, not
// duplicated here -- the same precedent faces/notice, faces/held and faces/receipt
// already set (req/99 §5).

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createFace, face, mount, toRecord, GRAPH_MESSAGES,
} from '../graph.mjs';
import { DECLARATION } from '../declaration.mjs';
import { parts } from '../binding.mjs';
// The dash is read from the module that owns it rather than typed here, so a test
// asserting "this is drawn as unknown" cannot pass against a different dash.
import { STAT_DASH } from '../../../parts/src/surface.mjs';
import {
  standInHost, textOfHost, nodesOf, press, nodeWith,
} from '../../ledger/test/dom-stand-in.mjs';
import {
  stubPort, page, refused, failed, absent,
} from '../../ledger/test/stub-port.mjs';

const { el, toHtml, find, findByAttr, textOf } = parts.element;

/** A minimal, explicit transformation item -- no SAMPLE helper reused here, because
 * this face's own tests need multiple paths recurring against each other in
 * specific ways (a real chain, an edge leaving the window, a path collision), a
 * shape SAMPLE.transformation()'s single always-incrementing chain does not
 * produce. */
function item(id, sequence, path, prev, extra = {}) {
  return {
    id, sequence, prev, path, at: `2026-08-24T09:${String(sequence).padStart(2, '0')}:00Z`,
    actor: 'agent:packer', effect: 'write', verdict: 'Admit', digest: `d${String(sequence).padStart(3, '0')}`,
    ...extra,
  };
}

/**
 * The fixture population this whole file reasons about:
 *   /work/a.md -- 3 touches, a genuine chain, both edges resolvable in-window
 *   /work/b.md -- 1 touch: not a subject, counted under touchedOnce
 *   /work/c.md -- 2 touches: the first names a predecessor outside the window
 *                 (edge not drawn), the second resolves against the first (drawn)
 *   /work/d.md -- 2 touches: the first names t-002 (b.md) as predecessor -- a real
 *                 node this window read, but under a different path (edge not
 *                 drawn as a path collision), the second resolves normally
 *   /work/e.md -- 3 touches: t-011 -> t-012 is a clean adjacent link, but t-013
 *                 names t-011 (not t-012) as its predecessor -- a real node, same
 *                 path, so this face's own byId resolution draws it as a genuine
 *                 edge (t-013 chained under t-011, skipping t-012); the reused
 *                 parts/src/checkable.mjs chain claim, which only ever compares a
 *                 row against the one immediately before it in sequence, correctly
 *                 reads this as "does not hold" (t-013 does not name t-012). The
 *                 two readings disagreeing here is not a bug -- it is exactly the
 *                 distinction the claims section's own aside states.
 */
const ITEMS = Object.freeze([
  item('t-001', 1, '/work/a.md', null),
  item('t-002', 2, '/work/b.md', null),
  item('t-003', 3, '/work/a.md', 't-001'),
  item('t-004', 4, '/work/a.md', 't-003', { verdict: 'Deny', effect: 'delete' }),
  item('t-005', 5, '/work/c.md', 't-900'),
  item('t-006', 6, '/work/c.md', 't-005'),
  item('t-007', 7, '/work/d.md', 't-002'),
  item('t-008', 8, '/work/d.md', 't-007'),
  item('t-011', 11, '/work/e.md', null),
  item('t-012', 12, '/work/e.md', 't-011'),
  item('t-013', 13, '/work/e.md', 't-011'),
]);

function fullPort(overrides = {}) {
  return stubPort({
    get_transformations: page([...ITEMS]),
    ...overrides,
  }, { methods: DECLARATION.consumes });
}

async function draw(port) {
  const state = await face.read(port);
  return { state, tree: face.view(state), html: toHtml(face.view(state)) };
}

const sectionOf = (tree, name) => findByAttr(tree, 'data-section', name);
const pathGroupOf = (tree, path) => findByAttr(tree, 'data-path', path)[0] ?? null;
const pathGroups = (tree) => findByAttr(tree, 'data-section', 'path-group');

// -- mount --------------------------------------------------------------------

test('W2: mount returns a function, and unmount empties the host', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  assert.equal(typeof unmount, 'function');
  await unmount.ready;
  assert.ok(host.childNodes.length > 0, 'nothing was mounted');
  unmount();
  assert.equal(host.childNodes.length, 0);
  unmount();
  assert.equal(host.childNodes.length, 0, 'unmounting twice is not an error');
});

test('mount refuses a missing host or a missing port, and says which', () => {
  assert.throws(() => mount(null, fullPort(), []), new RegExp(GRAPH_MESSAGES.NO_HOST));
  assert.throws(() => mount(standInHost(), null, []), new RegExp(GRAPH_MESSAGES.NO_PORT));
});

test('mount draws something before the read answers', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  const early = textOfHost(host);
  assert.ok(early.includes(GRAPH_MESSAGES.READING));
  await unmount.ready;
  unmount();
});

// -- C-1 ------------------------------------------------------------------------

test('C-1: every method the face calls was declared', async () => {
  const port = fullPort();
  await face.read(port);
  const called = [...new Set(port.calls.map((c) => c.name))];
  const undeclared = called.filter((name) => !DECLARATION.consumes.includes(name));
  assert.deepEqual(undeclared, []);
  assert.ok(called.length > 0);
});

test('C-1 negative control: calling an undeclared method raises rather than reaching the port', async () => {
  const port = fullPort();
  const caller = face.callerFor(port);
  await assert.rejects(() => caller.invoke('get_healthz', {}), new RegExp(GRAPH_MESSAGES.UNDECLARED));
  await assert.rejects(() => caller.fold('get_healthz'), new RegExp(GRAPH_MESSAGES.UNDECLARED));
  assert.equal(port.calls.some((c) => c.name === 'get_healthz'), false, 'the call reached the port anyway');
});

// -- fail-closed ------------------------------------------------------------------

for (const [name, result] of [
  ['failed', failed()],
  ['refused', refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not list transformations', gx_code: 'UNAUTHORIZED' })],
  ['absent', absent({ name: 'get_transformations' })],
]) {
  test(`fail-closed: a ${name} list read draws the list as unread, never a silently empty graph`, async () => {
    const port = fullPort({ get_transformations: result });
    const { tree, html } = await draw(port);
    const listSection = sectionOf(tree, 'list')[0];
    assert.equal(listSection.attrs['data-state'], 'unread');
    assert.ok(textOf(listSection).includes(GRAPH_MESSAGES.LIST_UNREAD));
    assert.ok(textOf(listSection).includes(name), 'the outcome is not named on screen');
    assert.ok(html.includes('(not counted: the list was not read)'), 'a denominator was stated as zero when it was actually unmeasured');
  });
}

test('a genuinely empty list (answered, zero items) draws the no-subjects state honestly, not the unread state', async () => {
  const port = fullPort({ get_transformations: page([]) });
  const { tree, html } = await draw(port);
  const listSection = sectionOf(tree, 'list')[0];
  assert.equal(listSection.attrs['data-state'], 'no-subjects');
  assert.ok(html.includes(GRAPH_MESSAGES.NO_SUBJECTS));
  assert.ok(html.includes('0 -- none'), 'touchedOnce should read a real, measured zero here');
});

// -- toRecord: attest, verbatim, holes never dropped -----------------------------

test('toRecord: a member absent from an item becomes a named hole, not a silent omission', () => {
  const record = toRecord({ transformations: page([item('t-001', 1, '/work/a.md', null, { actor: undefined })]) });
  assert.ok(record.nodes[0].holes.actor?.includes(GRAPH_MESSAGES.MEMBER_ABSENT));
});

test('toRecord: a member that is not a scalar is a named hole, not a crash', () => {
  const record = toRecord({ transformations: page([item('t-001', 1, '/work/a.md', null, { path: { nested: true } })]) });
  assert.ok(record.nodes[0].holes.path?.includes(GRAPH_MESSAGES.MEMBER_NOT_SCALAR));
});

test('toRecord: an item with no usable id is not silently folded into a group -- it is counted as unidentifiable', () => {
  const record = toRecord({
    transformations: page([
      item('t-010', 1, '/work/z.md', null),
      { sequence: 2, path: '/work/z.md', prev: 't-010', at: '2026-08-24T09:02:00Z', actor: 'a', effect: 'write', verdict: 'Admit', digest: 'd002' },
    ]),
  });
  assert.equal(record.notDrawn.unidentifiable.count, 1);
  const group = record.groups.find((g) => g.path === '/work/z.md');
  assert.equal(group.touchCount, 2, 'the raw touch count is not reduced by an identity problem');
  assert.equal(group.rows.length, 1, 'the row that could not be identified is not drawn as though it were fine');
});

// -- the subject population: touched twice or more, and only that -----------------

test('a path touched exactly once is not a graph subject, and is counted, not silently dropped', async () => {
  const { state, tree } = await draw(fullPort());
  const record = toRecord(state);
  assert.equal(record.notDrawn.touchedOnce.count, 1);
  assert.deepEqual([...record.notDrawn.touchedOnce.paths], ['/work/b.md']);
  assert.equal(pathGroupOf(tree, '/work/b.md'), null, 'a once-touched path was drawn as a subject');
});

test('distinctPaths counts every path read, subjects and once-touched alike', async () => {
  const { state } = await draw(fullPort());
  const record = toRecord(state);
  assert.equal(record.distinctPaths, 5);
});

// -- edges: drawn only when both ends were actually read --------------------------

test('an edge whose predecessor this window read, under the same path, is drawn as a chained row', async () => {
  const { tree } = await draw(fullPort());
  const group = pathGroupOf(tree, '/work/a.md');
  assert.ok(group);
  const t003 = findByAttr(group, 'data-row', 't-003')[0];
  assert.equal(t003.attrs['data-child-of'], 't-001');
  const t004 = findByAttr(group, 'data-row', 't-004')[0];
  assert.equal(t004.attrs['data-child-of'], 't-003');
  const childMark = findByAttr(t003, 'data-mark', 'structure/child');
  assert.ok(childMark.length > 0, 'the child mark was not drawn for a resolvable edge');
});

test('an edge whose predecessor this window never read is declared not drawn, named, and counted', async () => {
  const { state, tree } = await draw(fullPort());
  const record = toRecord(state);
  const outside = record.notDrawn.edgesOutside.edges.find((e) => e.to === 't-005');
  assert.ok(outside, 'the t-900 -> t-005 edge was not counted as outside the window');
  assert.equal(outside.wantedPrev, 't-900');
  const group = pathGroupOf(tree, '/work/c.md');
  const annotation = findByAttr(group, 'data-to', 't-005')[0];
  assert.ok(annotation, 'the outside annotation was not drawn for t-005');
  assert.ok(textOf(annotation).includes('t-900'));
  const t005 = findByAttr(group, 'data-row', 't-005')[0];
  assert.equal(t005.attrs['data-child-of'], undefined, 'a row with an unresolved predecessor was drawn as though it were chained');
});

test('an edge whose predecessor exists but under a different path is a path collision, declared not drawn, never silently connected', async () => {
  const { state, tree } = await draw(fullPort());
  const record = toRecord(state);
  const outside = record.notDrawn.edgesOutside.edges.find((e) => e.to === 't-007');
  assert.ok(outside, 'the t-002 -> t-007 cross-path edge was not counted');
  assert.equal(outside.wantedPrev, 't-002');
  assert.ok(outside.why.includes('different path'));
  const group = pathGroupOf(tree, '/work/d.md');
  const t007 = findByAttr(group, 'data-row', 't-007')[0];
  assert.equal(t007.attrs['data-child-of'], undefined);
  const t008 = findByAttr(group, 'data-row', 't-008')[0];
  assert.equal(t008.attrs['data-child-of'], 't-007', 'the in-path edge from t-007 to t-008 should still be drawn');
});

test('edgesOutside counts exactly the two genuine anomalies planted, no more and no fewer', async () => {
  const { state } = await draw(fullPort());
  const record = toRecord(state);
  assert.equal(record.notDrawn.edgesOutside.count, 2);
});

// -- group ordering: most-touched-first, then alphabetical ------------------------

test('C-6: path groups are ordered by descending touch count, ties broken by path', async () => {
  const { tree } = await draw(fullPort());
  const groupEls = findByAttr(tree, 'data-section', 'path-group');
  const order = groupEls.map((g) => g.attrs['data-path']);
  assert.deepEqual(order, ['/work/a.md', '/work/e.md', '/work/c.md', '/work/d.md']);
});

test('within a group, touches are ordered by ascending sequence', async () => {
  const { tree } = await draw(fullPort());
  const group = pathGroupOf(tree, '/work/a.md');
  const rowIds = find(group, (n) => n.tag === 'div' && n.attrs['data-part'] === 'receipt-row').map((n) => n.attrs['data-row']);
  assert.deepEqual(rowIds, ['t-001', 't-003', 't-004']);
});

// -- claims section: parts/src/checkable.mjs reused per group ---------------------

test('the claims section states, per group, whether the chain claim holds -- a.md\'s clean chain holds', async () => {
  const { tree } = await draw(fullPort());
  const claims = sectionOf(tree, 'claims')[0];
  const aRow = findByAttr(claims, 'data-claim-path', '/work/a.md')[0];
  assert.equal(aRow.attrs['data-holds'], 'true');
});

test('the claims section names a group whose chain does not hold, and says why -- even though the row it names was drawn as chained', async () => {
  const { tree } = await draw(fullPort());
  const claims = sectionOf(tree, 'claims')[0];
  const eRow = findByAttr(claims, 'data-claim-path', '/work/e.md')[0];
  assert.equal(eRow.attrs['data-holds'], 'false');
  assert.ok(textOf(eRow).includes('t-013'), 'the claim does not name the row whose local link is broken');

  // The disagreement this test is about: t-013 IS drawn as a genuine edge (this
  // window read t-011, same path), even though the broader chain claim says the
  // group's chain does not hold (t-013 does not name t-012, its sequence-adjacent
  // neighbour). The two are independent readings over the same population.
  const group = pathGroupOf(tree, '/work/e.md');
  const t013 = findByAttr(group, 'data-row', 't-013')[0];
  assert.equal(t013.attrs['data-child-of'], 't-011');
});

// -- C-3: what is not drawn ---------------------------------------------------------

test('C-3: the members this face does not draw are named with reasons', async () => {
  const { tree } = await draw(fullPort());
  const words = textOf(sectionOf(tree, 'not-drawn')[0]);
  for (const entry of DECLARATION.undrawn) assert.ok(words.includes(entry.what), `${entry.what} is declared undrawn but not stated on screen`);
});

// -- order (screen-level "why") -----------------------------------------------------

test('C-6: the order reason is stated on screen', async () => {
  const { html } = await draw(fullPort());
  assert.ok(html.includes('fourth, after ledger'));
});

// -- marks, sizes, positioning --------------------------------------------------

test('C-4: every mark drawn was declared', async () => {
  const { tree } = await draw(fullPort());
  const declared = new Set(DECLARATION.marks.map((m) => m.mark));
  const attrOf = (name) => find(tree, (n) => name in n.attrs).map((n) => n.attrs[name]);
  const drawn = [...new Set(attrOf('data-mark'))];
  assert.ok(drawn.length > 0, 'no marks were drawn, so this proves nothing');
  for (const mark of drawn) assert.ok(declared.has(mark), `undeclared mark on screen: ${mark}`);
});

test('structure/outside is actually drawn on this fixture, not merely declared', async () => {
  const { html } = await draw(fullPort());
  assert.ok(html.includes('data-mark="structure/outside"'));
});

test('C-5: no meaning is carried by two different marks', async () => {
  const { tree } = await draw(fullPort());
  const seen = new Map();
  for (const node of find(tree, (n) => 'data-means' in n.attrs)) {
    const means = node.attrs['data-means'];
    const mark = node.attrs['data-mark'];
    if (seen.has(means)) assert.equal(seen.get(means), mark, `${means} is drawn with two marks`);
    seen.set(means, mark);
  }
  assert.ok(seen.size > 0);
});

test('AC-F3: every glyph on screen states its width and height', async () => {
  const { tree } = await draw(fullPort());
  const glyphs = find(tree, (n) => n.tag === 'svg' && 'data-mark' in n.attrs);
  assert.ok(glyphs.length > 0);
  for (const g of glyphs) {
    assert.match(g.attrs.width ?? '', /^\d+$/);
    assert.match(g.attrs.height ?? '', /^\d+$/);
    assert.match(g.attrs.style ?? '', /width:\d+px/);
  }
});

test('AC-F1: nothing in this face takes itself out of flow', async () => {
  const { tree } = await draw(fullPort());
  assert.deepEqual(parts.positionedNodes(tree), []);
});

test('N-4: a value long enough to be clipped in the row is on the page in full, and reaching it costs one press', async () => {
  // Owner directive #335 (3)/(4) moved the answer. `actor` is no longer one of the
  // columns this face scans on (parts/src/receipt-row.mjs SCAN_COLUMNS: with a detail
  // pane beside the list, eight columns starved the path column to zero pixels), so
  // it cannot be clipped in a row cell at all -- it is a fact of the chosen touch and
  // it is in the pane, in full.
  const longActor = item('t-201', 21, '/work/f.md', null, { actor: 'agent:the-very-long-one' });
  const longActor2 = item('t-202', 22, '/work/f.md', 't-201', { actor: 'agent:the-very-long-one' });
  const port = fullPort({ get_transformations: page([longActor, longActor2]) });
  const { tree } = await draw(port);
  assert.deepEqual(findByAttr(tree, 'data-cell', 'actor'), [], 'actor is still drawn in a fixed-width row cell');
  const state = await face.read(port);
  const opened = face.view({ ...state, selected: 't-201' });
  const pane = findByAttr(opened, 'data-part', 'detail-pane')[0];
  assert.equal(pane.attrs['data-subject'], 't-201');
  assert.ok(toHtml(pane).includes('agent:the-very-long-one'), 'the full actor string is not in the pane');
});

test('no colour is spelled out anywhere in what is drawn', async () => {
  const { html } = await draw(fullPort());
  assert.equal(/#[0-9a-fA-F]{3,8}\b/.test(html), false);
  assert.equal(/\brgba?\(/.test(html), false);
});

test('no borrowed symbol is drawn', async () => {
  const { html } = await draw(fullPort());
  for (const symbol of ['●', '◆', '◇', '◈', '▾', '▴', '★', '■', '⏺']) {
    assert.equal(html.includes(symbol), false, `borrowed symbol on screen: ${symbol}`);
  }
  // eslint-disable-next-line no-control-regex
  assert.equal(/[^\x00-\x7F]/.test(html), false, 'a non-ascii character reached the screen');
});

// -- rows are not edited -------------------------------------------------------------

test('rows are not edited: rendering the same state twice gives the same tree', async () => {
  const state = await face.read(fullPort());
  // Retrofit round 4 moved one thing on this screen out of the class of facts that
  // are the same on every paint: the runtime strip prints how long *this* paint took,
  // which is a measurement of the call and not a property of the record. It is held
  // out of the comparison by name -- only the two places it is written -- and
  // everything else on the screen is still compared byte for byte, which is what this
  // test was always for. Blanking the whole footer instead would have let a real
  // regression hide behind a measurement.
  const held = (html) => html
    .replace(/data-render-ms="[^"]*"/g, 'data-render-ms=""')
    .replace(/render [\d.]+ ms/g, 'render');
  const first = held(toHtml(face.view(state)));
  const second = held(toHtml(face.view(state)));
  assert.equal(first, second);
  assert.ok(first.includes('data-render-ms=""'), 'the measured figure is not where this test thinks it is');
});

test('the record toRecord() produces is immutable -- a node cannot be mutated after the fact', () => {
  const record = toRecord({ transformations: page([...ITEMS]) });
  assert.throws(() => { record.nodes[0].actor = 'someone-else'; });
  assert.throws(() => { record.notDrawn.touchedOnce.count = 999; });
});

// -- the parts are a seam ----------------------------------------------------------

test('the parts are injected: a face built on a stub draws the stub', async () => {
  const marker = 'this tree came from the stub';
  const stub = {
    ...parts,
    row: (record) => el('div', { 'data-part': 'receipt-row', 'data-row': record.id }, [marker]),
    receiptRow: (record) => el('div', { 'data-part': 'receipt-row', 'data-row': record.id }, [marker]),
    openableRow: (record) => el('div', { 'data-part': 'receipt-row', 'data-row': record.id }, [marker]),
    selectableRow: (record) => el('div', { 'data-part': 'receipt-row', 'data-row': record.id }, [marker]),
  };
  const other = createFace({ parts: stub });
  const state = await other.read(fullPort());
  const html = toHtml(other.view(state));
  assert.ok(html.includes(marker));
});

// -- SS657 retrofit (req/38 SS657 Owner #317/#318, idiom proven by faces/atlas) --

test('SS657 defect 4/5 cure: a single compact header line states the face name and the touched-twice-or-more denominator, before anything else', async () => {
  const { tree } = await draw(fullPort());
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  assert.ok(header);
  assert.ok(textOf(header).includes('graph'));
  assert.match(textOf(header), /\d+ of \d+ paths touched twice or more/);
  assert.equal(tree.children[0], header);
});

test('SS657 defect 2 cure, as Owner #348 (4) sharpens it: the controls are bordered, shut, and say how much is behind them rather than restating their own names', async () => {
  const { tree } = await draw(fullPort());
  const row = findByAttr(tree, 'data-role', 'control-row')[0];
  assert.ok(row);
  const controls = findByAttr(row, 'data-role', 'control');
  // Owner directive #335 (1): claims and omitted joined why and legend in this one
  // row; they were always-open bands of prose under the groups before it.
  assert.ok(controls.length >= 2);
  for (const control of controls) assert.equal(control.attrs['data-open'], 'false');
  const why = findByAttr(row, 'data-control', 'why')[0];
  const legend = findByAttr(row, 'data-control', 'legend')[0];
  assert.ok(why.attrs.style.includes('border'));
  assert.ok(legend.attrs.style.includes('border'));
  // What each of these used to carry beside its own name was a synonym of it --
  // `omitted -- what is not drawn`, `claims -- what you can check`. A count is the
  // thing a reader cannot work out from the label, and it is the same counted-
  // disclosure rule a row already holds (req/768 F-A, never a silent affordance).
  const behind = findByAttr(row, 'data-role', 'behind-count').map((n) => textOf(n));
  assert.deepEqual(behind, [
    `${DECLARATION.marks.length} marks`,
    `${pathGroups(tree).length} paths`,
    `${DECLARATION.undrawn.length} reasons`,
  ], 'the folded controls do not state what is behind them');
  // `why` is the one with nothing countable behind it, and its face states its label
  // alone rather than inventing a phrase to fill the space.
  const faceOf = (control) => textOf(control.children.find((n) => n.tag === 'summary'));
  assert.equal(faceOf(why).trim(), 'why');
  assert.equal(why.attrs['data-behind'], undefined);
  assert.equal(legend.attrs['data-behind'], String(DECLARATION.marks.length));
});

test('SS657 defect 1/3 cure: legend is a zero-inclusive counted table -- every declared mark gets a row, including ones this render drew zero of', async () => {
  const { tree } = await draw(fullPort());
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  const rows = findByAttr(legend, 'data-mark-entry');
  const declaredMarks = new Set(DECLARATION.marks.map((m) => m.mark));
  assert.equal(rows.length, declaredMarks.size);
  const controlRow = findByAttr(tree, 'data-role', 'control-row')[0];
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  const contentMarks = new Set();
  for (const child of tree.children) {
    if (child === controlRow || child === header) continue;
    for (const n of find(child, (x) => x.attrs && 'data-mark' in x.attrs)) contentMarks.add(n.attrs['data-mark']);
  }
  const zeroRows = rows.filter((r) => r.attrs['data-count'] === '0');
  assert.ok(zeroRows.length > 0);
  for (const r of zeroRows) assert.equal(contentMarks.has(r.attrs['data-mark-entry']), false);
});

test('Owner #348 (4): the undrawn census is on the screen once, with its reasons -- not twice in two folds a reader cannot tell apart', async () => {
  // What this replaces: the legend drew all seven declared undrawn entries with their
  // reasons, and the `omitted` control beside it drew all seven again. Both copies were
  // real and identical, which is worse than one, because a reader who finds the second
  // has no way to know it is not a different list. The census belongs to `omitted`,
  // which is the control whose name is the question it answers.
  const { tree } = await draw(fullPort());
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  const omitted = findByAttr(tree, 'data-control', 'omitted')[0];
  assert.ok(omitted, 'there is no omitted control for the census to live in');
  const drawnFor = (entry) => findByAttr(tree, 'data-omission', entry.what).length
    + findByAttr(tree, 'data-not-drawn', entry.what).length;
  for (const entry of DECLARATION.undrawn) {
    assert.equal(drawnFor(entry), 1, `the undrawn entry "${entry.what}" is on the screen ${drawnFor(entry)} times`);
    assert.ok(textOf(omitted).includes(entry.why), `omitted does not carry the reason for: ${entry.what}`);
  }
  assert.equal(findByAttr(legend, 'data-omission').length, 0, 'the legend is drawing the census a second time');
});

test('SS657 defect 1 cure, as Owner directive #335 (3) reshapes it: a touch is a keyboard-reachable control stating the count the pane will add', async () => {
  const { tree } = await draw(fullPort());
  const rows = findByAttr(tree, 'data-part', 'selectable-row');
  assert.ok(rows.length > 0, 'no touches were drawn, so this proves nothing');
  assert.deepEqual(findByAttr(tree, 'data-part', 'receipt-note'), [], 'a note is still drawn under a touch');
  for (const r of rows) {
    assert.equal(r.tag, 'button');
    assert.equal(r.attrs['data-selected'], 'false');
    const count = findByAttr(r, 'data-role', 'field-count')[0];
    assert.ok(count, 'the control states no count -- a silent affordance');
    assert.match(textOf(count), /\d+ fields/);
  }
  const pane = findByAttr(tree, 'data-part', 'detail-pane')[0];
  assert.ok(pane, 'there is no pane for a chosen touch to be stored in');
});

// -- retrofit round 2 (req/768 AC-6/AC-7, SS657 continued) --

test('AC-7 negative control: an ordinary chain edge (prev) is never read as a reversal -- t-001 has a chained child (t-003) but was never undone', async () => {
  // /work/a.md's own chain is t-001 -> t-003 -> t-004, an entirely ordinary
  // sequence of edits (no item in ITEMS sets undo_of). If this face's own
  // reversal chip mistakenly reused the chain-edge childOf for this decision,
  // t-001 would wrongly read as "reversed" the moment anything was ever
  // written after it -- this is the test that would have caught that bug.
  const { tree } = await draw(fullPort());
  const group = pathGroupOf(tree, '/work/a.md');
  const chips = findByAttr(group, 'data-part', 'reversal-chip');
  assert.equal(chips.length, 3, 'one chip per touch in this path');
  for (const chip of chips) {
    assert.equal(chip.attrs['data-state'], 'not-observable', 'no touch in this path was ever named as an undo target');
    assert.ok(textOf(chip).includes('unknown'));
  }
});

test('AC-7: a touch whose undo_of names an earlier touch on the same path reads that earlier touch\'s chip as reversed', async () => {
  const port = fullPort({
    get_transformations: page([
      ...ITEMS,
      item('t-014', 14, '/work/a.md', 't-004', { undo_of: 't-004', effect: 'undo' }),
    ]),
  });
  const { tree } = await draw(port);
  const group = pathGroupOf(tree, '/work/a.md');
  const chips = findByAttr(group, 'data-part', 'reversal-chip');
  const states = chips.map((c) => c.attrs['data-state']);
  assert.deepEqual(states, ['not-observable', 'not-observable', 'reversed', 'not-observable'], 't-001, t-003 unknown; t-004 reversed (by t-014); t-014 itself unknown (nothing reverses it)');
  const reversedChip = chips.find((c) => c.attrs['data-state'] === 'reversed');
  assert.match(reversedChip.attrs.title, /t-014/);
});

test('AC-7: a reversal on one path never leaks into another path\'s reversibility chips', async () => {
  const port = fullPort({
    get_transformations: page([
      ...ITEMS,
      item('t-014', 14, '/work/a.md', 't-004', { undo_of: 't-004', effect: 'undo' }),
    ]),
  });
  const { tree } = await draw(port);
  const cGroup = pathGroupOf(tree, '/work/c.md');
  const cChips = findByAttr(cGroup, 'data-part', 'reversal-chip');
  for (const chip of cChips) assert.equal(chip.attrs['data-state'], 'not-observable');
});

test('AC-7: the legend explains the reversibility chip\'s two reachable states once, not per touch and not twice', async () => {
  // AC-7's requirement is unchanged and still held; what changed under Owner #348 (4)
  // is which part of the legend holds it. The counted mark table already prints every
  // declared mark beside its own count and its own `from` sentence, and both chip
  // states are declared marks -- so the three prose lines underneath it, which restated
  // structure/child, structure/outside and the chip in longer words, were a second copy
  // of what the rows above them had just said. The table is the copy that survives,
  // because it is the one a reader can count against what is on the screen.
  const { tree } = await draw(fullPort());
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  for (const mark of ['standing/reversed', 'standing/none']) {
    const rows = findByAttr(legend, 'data-mark-entry', mark);
    assert.equal(rows.length, 1, `${mark} is explained ${rows.length} times in the legend`);
    const declared = DECLARATION.marks.find((m) => m.mark === mark);
    assert.ok(textOf(rows[0]).includes(declared.from), `${mark} is drawn without the reason the declaration gives it`);
  }
  assert.match(textOf(legend), /reversibility chip/);
});

test('AC-4: no acts, no gutter -- this face declares no commit/cancel/undo route', async () => {
  const { tree } = await draw(fullPort());
  assert.equal(findByAttr(tree, 'data-part', 'act-gutter').length, 0);
  assert.deepEqual(DECLARATION.acts, []);
});

// -- retrofit round 4 (Owner #340: not monotone / understandable at a glance / usable) --
//
// The five atoms this round asks of every face, asked of this one. Each assertion
// below was seen red against the build that preceded it, and each names a number
// this face already computes rather than a number written into the test by hand:
// the population these tests reason about is ITEMS above, whose shape is stated in
// its own comment (5 paths, 11 touches, 4 of them touched twice or more, 2 declared
// edges leaving the window, nothing reversed).

const bandOf = (tree) => findByAttr(tree, 'data-part', 'stat-band')[0] ?? null;
const segmentsOf = (tree) => findByAttr(bandOf(tree), 'data-role', 'segment');
const figuresOf = (tree) => segmentsOf(tree).map((s) => [s.attrs['data-noun'], textOf(findByAttr(s, 'data-role', 'figure')[0])]);

test('the band is the second thing on the screen, and every figure in it is a count this face already computed', async () => {
  const { state, tree } = await draw(fullPort());
  const record = toRecord(state);
  const band = bandOf(tree);
  assert.ok(band, 'there is no stat band');
  assert.equal(tree.children[1], band, 'the band does not sit immediately under the header line');
  const chained = record.groups.reduce((n, g) => n + g.rows.filter((r) => typeof r.childOf === 'string').length, 0);
  assert.deepEqual(figuresOf(tree), [
    ['touches', String(record.nodes.length)],
    ['linked', String(chained)],
    ['reversed', '0'],
    ['undrawn', String(record.notDrawn.edgesOutside.count)],
  ]);
  // Not a formula the test repeats back: the numbers this population actually has.
  // 11 touches; 6 of them name a predecessor this window read under the same path
  // (a.md twice, e.md twice, c.md once, d.md once); nothing is reversed; 2 declared
  // links leave the window (t-900, and t-002 under another path).
  assert.deepEqual(figuresOf(tree).map(([, n]) => n), ['11', '6', '0', '2']);
  // The two counts the header line already states are not repeated in the band.
  assert.equal(band.attrs['data-count'], '4');
});

test('the two figures that name a standing carry its mark, so the band is read as marks and not only as digits', async () => {
  const { tree } = await draw(fullPort());
  const band = bandOf(tree);
  const marks = findByAttr(band, 'data-mark').map((n) => n.attrs['data-mark']);
  assert.deepEqual(marks, ['structure/child', 'standing/reversed', 'structure/outside']);
  const declared = new Set(DECLARATION.marks.map((m) => m.mark));
  for (const mark of marks) assert.ok(declared.has(mark), `undeclared mark in the band: ${mark}`);
});

test('a count this window could not know is a dash in the band, never a zero', async () => {
  const port = fullPort({ get_transformations: refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not list transformations', gx_code: 'UNAUTHORIZED' }) });
  const { tree } = await draw(port);
  const segments = segmentsOf(tree);
  assert.equal(segments.length, 4, 'a segment was dropped rather than drawn as unknown');
  for (const segment of segments) {
    assert.equal(segment.attrs['data-value'], 'unread', `${segment.attrs['data-noun']} claims a number the list read never gave it`);
    assert.equal(textOf(findByAttr(segment, 'data-role', 'figure')[0]), STAT_DASH);
  }
});

test('an answered read of nothing is four measured zeros, which is a different fact from four dashes', async () => {
  const { tree } = await draw(fullPort({ get_transformations: page([]) }));
  assert.deepEqual(figuresOf(tree).map(([, n]) => n), ['0', '0', '0', '0']);
});

test('every path this screen draws is one bordered box whose head states the path, its own touch count and the standing of its latest touch', async () => {
  const { state, tree } = await draw(fullPort());
  const record = toRecord(state);
  const boxes = findByAttr(tree, 'data-part', 'box');
  assert.equal(boxes.length, record.groups.length, 'one box per drawn path, no more and no fewer');
  for (const group of record.groups) {
    const own = boxes.find((b) => b.attrs['data-box'] === group.path);
    assert.ok(own, `no box for ${group.path}`);
    assert.equal(own.attrs['data-count'], String(group.touchCount));
    assert.equal(own.attrs['data-noun'], 'touches');
    const head = findByAttr(own, 'data-role', 'box-head')[0];
    assert.ok(textOf(head).includes(group.path), 'the head does not name what the box holds');
    assert.ok(textOf(head).includes(`${group.touchCount} touches`), 'the head does not state its own count');
    const pill = findByAttr(head, 'data-part', 'verdict-badge')[0];
    const latest = group.rows[group.rows.length - 1];
    assert.equal(pill.attrs['data-verdict'], latest.verdict, 'the standing on the head is not the standing of the latest touch');
    assert.equal(pill.attrs['data-filled'], 'true', 'a standing drawn without its bed is a stroke, not an area');
  }
  // The population is not monotone: a.md's latest touch is a Deny and the other
  // three are Admits, so three boxes carry one hue and one carries another.
  const verdicts = boxes.map((b) => findByAttr(b, 'data-part', 'verdict-badge')[0].attrs['data-verdict']);
  assert.deepEqual(verdicts.filter((v) => v === 'Deny').length, 1);
});

test('a group with nothing in it keeps its border and says a number: 0 when it was read, a dash when it was not', async () => {
  const { tree: empty } = await draw(fullPort({ get_transformations: page([]) }));
  const emptyBox = findByAttr(empty, 'data-part', 'box')[0];
  assert.ok(emptyBox, 'the answered-empty state drew no box at all');
  assert.equal(emptyBox.attrs['data-count'], '0');

  const { tree: unread } = await draw(fullPort({ get_transformations: failed() }));
  const unreadBox = findByAttr(unread, 'data-part', 'box')[0];
  assert.ok(unreadBox, 'the unread state drew no box at all');
  assert.equal(unreadBox.attrs['data-count'], STAT_DASH);
});

test('the last thing on the screen is what this paint cost, measured on this call and not estimated', async () => {
  const { tree } = await draw(fullPort());
  const footer = tree.children[tree.children.length - 1];
  assert.equal(footer.attrs['data-part'], 'runtime-footer');
  const measured = Number(footer.attrs['data-render-ms']);
  assert.ok(Number.isFinite(measured) && measured > 0, `the render figure is not a measurement: ${footer.attrs['data-render-ms']}`);
  assert.match(textOf(footer), /render [\d.]+ ms/);
  assert.match(textOf(footer), /read one list of transformations/);
});

test('a paint that read nothing says so with a dash rather than naming a source it never reached', async () => {
  const { tree } = await draw(fullPort({ get_transformations: failed() }));
  const footer = tree.children[tree.children.length - 1];
  const read = findByAttr(footer, 'data-name', 'read')[0];
  assert.equal(textOf(read), `read ${STAT_DASH}`);
});

test('req/103 finding 2: a repaint cannot shut a fold the reader opened, because which folds are open is this window\'s own answer', async () => {
  // The mechanism that made this atom not apply is gone: this face installs a press
  // handler now, so a repaint is reachable and a native <details> would be reset to
  // shut by every one of them. The answer is carried in state instead, and the press on
  // the summary is taken here rather than left to the element -- two keepers of one
  // answer is how they come to disagree.
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  const legend = nodeWith(host, 'data-control', 'legend');
  assert.equal(legend.getAttribute('data-open'), 'false');
  const summary = legend.childNodes.find((n) => n.tag === 'summary');
  const { defaulted } = press(host, summary);
  assert.equal(defaulted, false, 'the element was left to toggle itself as well, so there are two answers');
  assert.equal(nodeWith(host, 'data-control', 'legend').getAttribute('data-open'), 'true');
  // The repaint a completely unrelated press causes must not take it back.
  press(host, nodeWith(host, 'data-select-row', 't-001'));
  assert.equal(nodeWith(host, 'data-control', 'legend').getAttribute('data-open'), 'true', 'a repaint shut a fold the reader opened');
  unmount();
});

test('this face bounds no height and scrolls nothing of its own -- the one bounded container on the screen is the shared pane', async () => {
  const { tree } = await draw(fullPort());
  const bounded = find(tree, (n) => typeof n.attrs?.style === 'string' && /(^|;)(max-height|overflow-x|overflow-y):/.test(n.attrs.style));
  assert.deepEqual(bounded.map((n) => n.attrs['data-part'] ?? n.tag), ['detail-pane']);
});

// -- retrofit round 5 (Owner #348 (2)(4)): a row that answers a press, and the menu the
// second mouse button opens on it ------------------------------------------------------
//
// The state these were written against: every row was a real <button> carrying
// aria-pressed and a field count, view() read state.selected, and no code path in the
// whole face ever produced a state carrying one. The rows were controls that did
// nothing, and the pane read "no row is open" for the entire life of a live window.

/**
 * A press of the second mouse button, and a key.
 *
 * dom-stand-in.mjs's own press() dispatches a click and nothing else; it is shared with
 * five other faces and this lane does not edit it. The two other kinds of event this
 * screen now listens for are delivered here instead, through the same listener list and
 * in the same shape a window uses -- a target, a type, and a preventDefault a handler
 * can call.
 */
function fire(host, type, target, extra = {}) {
  let defaulted = true;
  const event = {
    type, target, ...extra, preventDefault() { defaulted = false; },
  };
  for (const listener of [...host.listeners]) {
    if (listener.type === type) listener.handler(event);
  }
  return { defaulted };
}

const menusIn = (host) => nodesOf(host).filter((n) => n.attrs && 'data-face-menu' in n.attrs);
const cellIn = (host, id, key) => nodesOf(nodeWith(host, 'data-select-row', id))
  .find((n) => n.attrs && n.attrs['data-cell'] === key);

async function live(overrides = {}, faceUnder = null) {
  const host = standInHost();
  const unmount = (faceUnder ?? face).mount(host, fullPort(overrides), []);
  await unmount.ready;
  return { host, unmount };
}

test('a touch answers the first mouse button: it becomes the subject of the one pane, and pressing it again lets go', async () => {
  const { host, unmount } = await live();
  assert.equal(nodeWith(host, 'data-part', 'detail-pane').getAttribute('data-subject'), null, 'something was chosen before any press');
  press(host, nodeWith(host, 'data-select-row', 't-003'));
  const pane = nodeWith(host, 'data-part', 'detail-pane');
  assert.equal(pane.getAttribute('data-subject'), 't-003');
  assert.ok(Number(pane.getAttribute('data-count')) > 0, 'the pane names a subject and states nothing about it');
  assert.equal(nodeWith(host, 'data-select-row', 't-003').getAttribute('aria-pressed'), 'true');
  assert.equal(nodeWith(host, 'data-select-row', 't-001').getAttribute('aria-pressed'), 'false');
  press(host, nodeWith(host, 'data-select-row', 't-003'));
  assert.equal(nodeWith(host, 'data-part', 'detail-pane').getAttribute('data-subject'), null);
  unmount();
});

test('the pane holds the path, which is why a row inside a box no longer draws it -- and holds it once', async () => {
  const { host, unmount } = await live();
  const row = nodeWith(host, 'data-select-row', 't-003');
  const claimed = Number(row.getAttribute('data-fields'));
  press(host, nodeWith(host, 'data-select-row', 't-003'));
  const pane = nodeWith(host, 'data-part', 'detail-pane');
  assert.ok(textOfHost(pane).includes('/work/a.md'), 'the chosen touch does not say what it touched');
  // The pane opened with the group's path and then drew `path in full` under it with the
  // same string. One line now, and the count the row promised is exactly what arrived.
  const named = nodesOf(pane).filter((n) => n.attrs && 'data-name' in n.attrs).map((n) => n.getAttribute('data-name'));
  assert.deepEqual(named, [...new Set(named)], 'the pane names a field twice');
  assert.equal(named.filter((name) => name.includes('path')).length, 1);
  assert.equal(Number(pane.getAttribute('data-count')), claimed, 'the row promised a number of fields the pane did not add');
  // `in full` is a promise about a value the row drew short, and only two members are.
  assert.deepEqual(named.filter((name) => name.endsWith(' in full')), ['at in full', 'digest in full']);
  unmount();
});

test('Owner #348 (4): a path is stated once per box, not once per row', async () => {
  // The densest redundancy this screen had: the box head states the path, and every row
  // in that box drew the same string again in its own widest column -- four copies of
  // one value in a box of three touches, the longest string on the screen repeated the
  // most times.
  const { tree } = await draw(fullPort());
  const group = pathGroupOf(tree, '/work/a.md');
  assert.equal(group.attrs['data-touch-count'], '3');
  assert.equal(findByAttr(group, 'data-cell', 'path').length, 0, 'a row is still drawing the path its box head states');
  assert.equal(textOf(findByAttr(group, 'data-role', 'box-name')[0]), '/work/a.md');
  // Once for a reader, in the head. The two remaining copies are the attributes an
  // instrument reads the group by (data-path, data-box), which are not on the screen.
  const copies = toHtml(group).split('/work/a.md').length - 1;
  assert.equal(copies, 3, `the path is still written ${copies} times inside its own box`);
});

test('the second mouse button opens exactly one menu, on the touch it was pressed over', async () => {
  const { host, unmount } = await live();
  assert.equal(menusIn(host).length, 0);
  const { defaulted } = fire(host, 'contextmenu', cellIn(host, 't-001', 'at'));
  assert.equal(defaulted, false, 'the platform menu was left to open on top of this one');
  assert.equal(menusIn(host).length, 1);
  assert.equal(menusIn(host)[0].getAttribute('data-face-menu'), 't-001');
  // A second press does not stack a second menu: one touch is named, and naming another
  // replaces it.
  fire(host, 'contextmenu', cellIn(host, 't-003', 'effect'));
  assert.equal(menusIn(host).length, 1, 'two menus are on the screen at once');
  assert.equal(menusIn(host)[0].getAttribute('data-face-menu'), 't-003');
  unmount();
});

test('a press that is not over a touch is left to the browser: the platform menu is refused only where this face has something better', async () => {
  const { host, unmount } = await live();
  const { defaulted } = fire(host, 'contextmenu', nodeWith(host, 'data-role', 'face-header'));
  assert.equal(defaulted, true, 'the platform menu was refused over a place this face offers nothing');
  assert.equal(menusIn(host).length, 0);
  unmount();
});

test('the menu offers what the declaration offers and nothing else: this face declares no act, and says so rather than opening onto nothing', async () => {
  const { host, unmount } = await live();
  fire(host, 'contextmenu', cellIn(host, 't-001', 'at'));
  const menu = menusIn(host)[0];
  const items = nodesOf(menu).filter((n) => n.attrs && 'data-menu-item' in n.attrs);
  assert.deepEqual(DECLARATION.acts, [], 'this face grew an act, and the menu now has to draw it');
  assert.deepEqual(items.map((n) => n.getAttribute('data-menu-item')), ['copy-value', 'copy-identity']);
  const said = nodesOf(menu).find((n) => n.attrs && n.attrs['data-role'] === 'menu-empty');
  assert.ok(said, 'a menu with no act in it is indistinguishable from one whose acts failed to draw');
  assert.ok(textOfHost(said).includes(GRAPH_MESSAGES.MENU_NO_ACT));
  unmount();
});

test('an item the row does not send is drawn disabled with its reason, never absent', async () => {
  // A press that lands on the row but not on any cell has no value in mind, and a
  // "copy value" that guessed one would hand back something the hand was not over.
  const { host, unmount } = await live();
  fire(host, 'contextmenu', nodeWith(host, 'data-select-row', 't-001'));
  const item = nodeWith(host, 'data-menu-item', 'copy-value');
  assert.equal(item.getAttribute('data-sends'), 'false');
  assert.notEqual(item.getAttribute('disabled'), null, 'an unavailable item is drawn as an enabled control');
  assert.equal(item.getAttribute('title'), GRAPH_MESSAGES.MENU_NO_VALUE);
  assert.equal(item.getAttribute('data-value'), null);
  // The identity is always there to take, so the menu is never a row of dead controls.
  assert.equal(nodeWith(host, 'data-menu-item', 'copy-identity').getAttribute('data-sends'), 'true');
  unmount();
});

test('copy value takes what the face read, not what the row drew -- the time cell draws a declared cut', async () => {
  const { host, unmount } = await live();
  const cell = cellIn(host, 't-001', 'at');
  assert.equal(textOfHost(cell), '09:01:00', 'the row is not drawing the declared cut this test is about');
  fire(host, 'contextmenu', cell);
  const item = nodeWith(host, 'data-menu-item', 'copy-value');
  assert.equal(item.getAttribute('data-value'), '2026-08-24T09:01:00Z');
  assert.ok(textOfHost(item).includes('copy at'));
  unmount();
});

test('a copy states whether it worked, both ways', async () => {
  const taken = [];
  const withClipboard = createFace({ clipboard: { writeText: (v) => { taken.push(v); return Promise.resolve(); } } });
  const { host, unmount } = await live({}, withClipboard);
  fire(host, 'contextmenu', cellIn(host, 't-001', 'effect'));
  press(host, nodeWith(host, 'data-menu-item', 'copy-value'));
  await unmount.quiet();
  assert.deepEqual(taken, ['write']);
  const item = nodeWith(host, 'data-menu-item', 'copy-value');
  assert.equal(item.getAttribute('data-copied'), 'true');
  assert.equal(item.getAttribute('data-copy-failed'), null);
  assert.ok(textOfHost(item).includes(GRAPH_MESSAGES.COPY_DONE));
  unmount();

  // And the failing side, which is the one that matters: a window whose clipboard says
  // no must not draw a control that looks the same whether or not it did anything.
  const refusing = createFace({ clipboard: { writeText: () => Promise.reject(new Error('refused')) } });
  const second = await live({}, refusing);
  fire(second.host, 'contextmenu', cellIn(second.host, 't-001', 'effect'));
  press(second.host, nodeWith(second.host, 'data-menu-item', 'copy-value'));
  await second.unmount.quiet();
  const failed = nodeWith(second.host, 'data-menu-item', 'copy-value');
  assert.equal(failed.getAttribute('data-copy-failed'), 'true');
  assert.equal(failed.getAttribute('data-copied'), null);
  assert.ok(textOfHost(failed).includes(GRAPH_MESSAGES.COPY_FAILED));
  second.unmount();
});

test('the menu goes away on Escape, and on a press anywhere else, and never survives a repaint', async () => {
  const { host, unmount } = await live();
  fire(host, 'contextmenu', cellIn(host, 't-001', 'at'));
  assert.equal(menusIn(host).length, 1);
  const escaped = fire(host, 'keydown', nodeWith(host, 'data-menu-item', 'copy-identity'), { key: 'Escape' });
  assert.equal(escaped.defaulted, false);
  assert.equal(menusIn(host).length, 0, 'Escape left the menu on the screen');
  // A key that is not Escape is not a dismissal, and the handler must not claim it.
  fire(host, 'contextmenu', cellIn(host, 't-001', 'at'));
  const other = fire(host, 'keydown', nodeWith(host, 'data-menu-item', 'copy-identity'), { key: 'a' });
  assert.equal(other.defaulted, true);
  assert.equal(menusIn(host).length, 1);
  // A press inside the menu that lands on none of its controls is not a dismissal:
  // pressing the sentence about there being no act must not shut what a hand is reading.
  press(host, nodeWith(host, 'data-role', 'menu-empty'));
  assert.equal(menusIn(host).length, 1, 'pressing the menu itself put the menu away');
  // A press elsewhere on the face: the menu goes, and the press still does its own work
  // in the same paint rather than being swallowed by the dismissal.
  press(host, nodeWith(host, 'data-select-row', 't-003'));
  assert.equal(menusIn(host).length, 0, 'a press away from the menu left it on the screen');
  assert.equal(nodeWith(host, 'data-part', 'detail-pane').getAttribute('data-subject'), 't-003', 'the press that put the menu away was swallowed by it');
  unmount();
});

test('the menu is in flow, like everything else this face draws -- req/03 N-1 was an element that was not', async () => {
  const state = await face.read(fullPort());
  const tree = face.view({ ...state, menu: { row: 't-001', cell: 'at', item: null, outcome: null } });
  assert.equal(findByAttr(tree, 'data-face-menu').length, 1);
  assert.deepEqual(parts.positionedNodes(tree), [], 'the menu floats over the rows, which is the shape that produced N-1');
  // It hangs off the touch it belongs to, immediately under it, inside the same box.
  const group = pathGroupOf(tree, '/work/a.md');
  const block = findByAttr(group, 'data-role', 'row-block')[0];
  assert.equal(block.children[block.children.length - 1].attrs['data-face-menu'], 't-001');
});

test('unmounting takes every listener back off the host', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  assert.equal(host.listeners.length, 3, 'the press, the second-button press and the key are not all installed');
  unmount();
  assert.deepEqual(host.listeners, [], 'this face leaves a listener behind on a host it has left');
});
