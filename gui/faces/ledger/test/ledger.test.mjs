// SPDX-License-Identifier: Apache-2.0
// What the face does, asked of the face and not of a description of it.
//
// The two things these tests are really guarding: a list that could not be read must
// never be drawn as a list with nothing in it, and a row that has been written must
// never be edited afterwards. Everything else here is in service of those two.
//
// req/108 -- this suite re-derived against the req/893 rebuild.
//
// req/893 rebuilt the drawing from zero (the row is the control, the line never wraps,
// acts live inside the open row, one entrance, one hue) and left this suite standing
// against the screen it destroyed: 64 of 116 tests red, six of them asserting the act
// gutter and one asserting the wrap by name -- "the suite was not merely blind to the
// two worst defects; it specified them" (req/893 D-5). Under the standing rule that a
// test may change only when a named, dated ruling judged the asserted behaviour itself
// defective, every rewritten test below carries its ruling: req/893's own sections
// (S0 defects 1-5, AC-1..AC-8, D-2, D-5, D-6, D-8) are that ruling, dated 2026-08-26.
// Where the asserted behaviour survives the rebuild, only the locators changed and the
// assertion is the same fact read off the new tree. Where the behaviour itself was the
// defect (gutter, wrap, standing panel, hue-coded standings, per-row unknown chips,
// the row menu as a second act surface), the test now asserts the ruling's replacement.
// The five copy tests at the end are deliberately NOT rewritten and stand red: req/893
// D-8 names copy "a real capability and its loss is a regression, not a simplification",
// and no ruling has judged that capability defective -- so the red is the honest record
// of the regression until the reimplement-copy lane (req/893 S8) lands. Ledger of this
// re-derivation, family by family: req/108.

import test from 'node:test';
import assert from 'node:assert/strict';

import { createFace, face, mount, LEDGER_MESSAGES } from '../ledger.mjs';
import { DECLARATION } from '../declaration.mjs';
import { parts } from '../binding.mjs';
import { lattice, ink, markOfIntent } from '../roles.mjs';
import {
  standInHost, textOfHost, nodesOf, press, nodeWith, rightPress, strike, pressAway,
} from './dom-stand-in.mjs';
import {
  stubPort, page, answered, refused, failed, absent, SAMPLE,
} from './stub-port.mjs';

const { el, toHtml, find, findByAttr, textOf } = parts.element;

const settledItems = [SAMPLE.transformation(1), SAMPLE.transformation(2), SAMPLE.transformation(3)];
const heldItems = [SAMPLE.candidate(4), SAMPLE.candidate(5)];

function fullPort(overrides = {}) {
  return stubPort({
    get_transformations: page(settledItems),
    get_candidates: page(heldItems),
    get_ledger_consistency: answered({ consistent: true, checked_from: 1, checked_to: 3 }),
    post_candidates_id_commit: answered({ id: 'c-004' }, 202),
    post_candidates_id_cancel: answered({ id: 'c-005' }, 202),
    post_transformations_id_undo: answered({ id: 't-004' }, 202),
    ...overrides,
  }, { methods: DECLARATION.consumes });
}

async function draw(port) {
  const state = await face.read(port);
  return { state, tree: face.view(state), html: toHtml(face.view(state)) };
}

// The new tree's own addresses (req/893: data-section/data-state/receipt-row are gone;
// a half is a named section, a row is a button, the entrance is one <details>).
const halfOf = (tree, key) => findByAttr(tree, 'data-part', 'half').find((h) => h.attrs['data-half'] === key) ?? null;
const headOf = (half) => findByAttr(half, 'data-part', 'half-head')[0] ?? null;
const rowsOf = (tree) => findByAttr(tree, 'data-part', 'ledger-row');
const noteOf = (tree, key) => find(tree, (n) => n.attrs && n.attrs['data-note'] === key)[0] ?? null;
const entranceOf = (tree) => findByAttr(tree, 'data-part', 'ledger-aside')[0] ?? null;
const figureOf = (tree, noun) => findByAttr(tree, 'data-role', 'figure').find((f) => f.attrs['data-noun'] === noun) ?? null;
const actButtonsIn = (tree) => find(tree, (n) => n.tag === 'button' && n.attrs && n.attrs['data-act']);

// -- mount ------------------------------------------------------------------

test('W2: mount returns a function, and unmount empties the host', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  assert.equal(typeof unmount, 'function');
  await unmount.ready;
  assert.ok(host.childNodes.length > 0, 'nothing was mounted');
  unmount();
  assert.equal(host.childNodes.length, 0);
  unmount();
  assert.equal(host.childNodes.length, 0, 'unmounting twice is not an error');
});

test('mount refuses a missing host or a missing port, and says which', () => {
  assert.throws(() => mount(null, fullPort(), []), new RegExp(LEDGER_MESSAGES.NO_HOST));
  assert.throws(() => mount(standInHost(), null, []), new RegExp(LEDGER_MESSAGES.NO_PORT));
});

test('mount draws something before the reads answer, and does not call it a ledger', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  const early = textOfHost(host);
  assert.ok(early.includes(LEDGER_MESSAGES.READING));
  assert.equal(early.includes(LEDGER_MESSAGES.EMPTY_SETTLED), false);
  await unmount.ready;
  unmount();
});

test('the face is handed its notices and draws none of them', async () => {
  const host = standInHost();
  const notices = [{ seq: 1, method: 'get_transformations', outcome: 'answered' }];
  const unmount = mount(host, fullPort(), notices);
  await unmount.ready;
  assert.equal(textOfHost(host).includes('get_transformations'), false);
  unmount();
});

// -- C-1: the declaration bounds the calls -----------------------------------

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
  await assert.rejects(() => caller.invoke('get_healthz', {}), new RegExp(LEDGER_MESSAGES.UNDECLARED));
  assert.equal(port.calls.some((c) => c.name === 'get_healthz'), false, 'the call reached the port anyway');
});

test('C-2: a declared method the face withholds is never sent, and says why in the opened row (req/893 AC-1: acts live inside the open row, so the reason lives there too)', async () => {
  const port = fullPort();
  const state = await face.read(port);
  // req/893 killer assumption: at rest the screen holds zero act controls, so the
  // withheld reason cannot be "on screen" at rest -- it is on the withheld act's own
  // dimmed button, inside the opened held row, which is the one place acts exist.
  assert.equal(actButtonsIn(face.view(state)).length, 0, 'an act control is standing on the screen at rest');
  const opened = toHtml(face.view({ ...state, selected: 'c-004' }));
  for (const entry of DECLARATION.withheld) {
    assert.equal(port.calls.some((c) => c.name === entry.method), false);
    assert.ok(opened.includes(entry.why), `the reason for withholding ${entry.method} is not on the opened row`);
  }
  assert.ok(opened.includes('disabled'), 'a withheld act is drawn dimmed, not removed');
});

test('the face never names an actor: identity is the membrane\'s to attach', async () => {
  const port = fullPort();
  const { state } = await draw(port);
  await face.act(port, state, { act: 'commit', id: 'c-004' });
  for (const call of port.calls) {
    assert.equal(Object.prototype.hasOwnProperty.call(call.input ?? {}, 'actor'), false);
    assert.equal(Object.prototype.hasOwnProperty.call(call.input?.body ?? {}, 'actor'), false);
  }
});

// -- fail-closed --------------------------------------------------------------

for (const [name, result] of [
  ['failed', failed()],
  ['refused', refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not read the ledger', gx_code: 'UNAUTHORIZED' })],
  ['absent', absent({ name: 'get_transformations' })],
]) {
  test(`fail-closed: a ${name} read is not drawn as an empty ledger`, async () => {
    const port = fullPort({ get_transformations: result });
    const { tree, html } = await draw(port);
    const settled = halfOf(tree, 'settled');
    assert.ok(findByAttr(settled, 'data-part', 'unread')[0], 'an unread half draws no unread notice (req/893: the state moved from an attribute to a named part)');
    assert.ok(textOf(settled).includes(LEDGER_MESSAGES.UNREAD));
    assert.equal(html.includes(LEDGER_MESSAGES.EMPTY_SETTLED), false, 'an unread list was given the empty list\'s words');
    assert.equal(findByAttr(settled, 'data-part', 'ledger-row').length, 0);
    assert.ok(textOf(settled).includes(name), 'the outcome is not named on screen');
  });
}

test('an answered read with no items says so in different words from an unread one', async () => {
  const port = fullPort({ get_transformations: page([]) });
  const { tree } = await draw(port);
  const settled = halfOf(tree, 'settled');
  assert.ok(findByAttr(settled, 'data-part', 'empty')[0], 'an empty half draws no empty notice');
  assert.ok(textOf(settled).includes(LEDGER_MESSAGES.EMPTY_SETTLED));
  assert.equal(textOf(settled).includes(LEDGER_MESSAGES.UNREAD), false);
});

test('a refusal carries the engine\'s own words up, unedited', async () => {
  const problem = { type: 'about:blank', title: 'conflict', status: 409, detail: 'the ledger moved under the walk', gx_code: 'IDEMPOTENCY_CONFLICT' };
  const { html } = await draw(fullPort({ get_candidates: refused(problem) }));
  assert.ok(html.includes('IDEMPOTENCY_CONFLICT'));
  assert.ok(html.includes(problem.detail));
});

// -- C-3: the denominator ------------------------------------------------------
// req/893 AC-8: the denominator moved from a not-drawn band to the half's own head
// line; the facts it states -- drawn of received, requests, whether the walk stopped --
// are the same facts. The undrawn members moved behind the one entrance ('omitted').

test('C-3: the walk\'s denominator is on screen, not just the row count', async () => {
  const port = fullPort({ get_transformations: page(settledItems, { pages: 7, stopped: true }) });
  const { tree } = await draw(port);
  const words = textOf(headOf(halfOf(tree, 'settled')));
  assert.ok(words.includes('7 requests'), 'the number of requests the walk took is not stated');
  assert.ok(words.includes(LEDGER_MESSAGES.TRUNCATED));
});

test('C-3: rows the order dropped are counted and the reason for each is named', async () => {
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1), { sequence: 2, at: 'x' }, 'not a record']),
  });
  const { tree } = await draw(port);
  const settled = halfOf(tree, 'settled');
  const dropped = findByAttr(settled, 'data-part', 'dropped').map((d) => textOf(d));
  assert.ok(dropped.some((w) => w.includes('no-identity')));
  assert.ok(dropped.some((w) => w.includes('not-a-record')));
  assert.ok(textOf(headOf(settled)).includes('1 of 3'), 'the drawn-of-received count is not stated');
});

test('C-3: the members this face does not draw are named with reasons, behind the entrance', async () => {
  const { tree } = await draw(fullPort());
  const words = textOf(noteOf(tree, 'omitted'));
  for (const entry of DECLARATION.undrawn) {
    assert.ok(words.includes(entry.what), `${entry.what} is declared undrawn but not stated on screen`);
    assert.ok(words.includes(entry.why), `${entry.what} is named without its reason`);
  }
});

// -- order ---------------------------------------------------------------------

test('C-6: the order\'s reason is stated, behind the one entrance (req/893 AC-5: explanation lives behind one door)', async () => {
  const { tree } = await draw(fullPort());
  assert.ok(textOf(noteOf(tree, 'order')).includes(DECLARATION.rows.order_reason));
});

test('an order whose assumption breaks is substituted, and the substitution is stated', async () => {
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1), SAMPLE.transformation(2, { sequence: undefined })]),
  });
  const { tree } = await draw(port);
  assert.ok(textOf(noteOf(tree, 'order')).includes('the recorded order was kept'), 'the substitution is not stated');
});

// -- rows are not edited --------------------------------------------------------

test('undo appends a child row and leaves the row it undoes unedited: what changes on it is exactly the reversal reading (req/893 AC-7)', async () => {
  const before = [SAMPLE.transformation(1), SAMPLE.transformation(2)];
  const after = [...before, SAMPLE.transformation(3, { undo_of: 't-002', effect: 'undo' })];
  let reads = 0;
  const port = fullPort({
    get_transformations: () => page(reads++ === 0 ? before : after),
  });

  const first = await face.read(port);
  const firstTree = face.view(first);
  const next = await face.act(port, first, { act: 'undo', id: 't-002' });
  const secondTree = face.view(next);

  // no row disappeared, and the child arrived wearing the declared child mark.
  const ids = (t) => rowsOf(t).map((r) => r.attrs['data-row']);
  assert.ok(ids(firstTree).includes('t-001') && ids(secondTree).includes('t-001'), 'a row disappeared');
  const child = rowsOf(secondTree).find((r) => r.attrs['data-row'] === 't-003');
  assert.ok(child, 'no child row was appended');
  assert.ok(find(child, (n) => n.attrs && n.attrs['data-mark'] === 'structure/child')[0], 'the child row does not say it was written under an earlier row');

  // the undone row's facts -- time, effect, verdict, path -- are byte-identical. The
  // one cell that changes is the standing cell, which gains the reversed reading:
  // this window's own conclusion (req/893 D-6), not an edit of the record.
  const cells = (t, id) => rowsOf(t).find((r) => r.attrs['data-row'] === id).children.map((c) => textOf(c));
  const was = cells(firstTree, 't-002');
  const is = cells(secondTree, 't-002');
  assert.deepEqual([is[1], is[2], is[3], is[5]], [was[1], was[2], was[3], was[5]], 'the undone row was rewritten');
  assert.equal(was[4].includes('reversed'), false, 'the row read as reversed before anything reversed it');
  assert.ok(is[4].includes('reversed'), 'the reversal this read proves is not on the line');
});

test('an act that is refused is drawn, not swallowed', async () => {
  const port = fullPort({
    post_candidates_id_commit: refused({ type: 'about:blank', title: 'gone', status: 410, detail: 'this candidate was already cancelled', gx_code: 'VALIDATION_ERROR' }),
  });
  const state = await face.read(port);
  const next = await face.act(port, state, { act: 'commit', id: 'c-004' });
  const html = toHtml(face.view(next));
  assert.ok(html.includes('this candidate was already cancelled'));
  assert.ok(html.includes('VALIDATION_ERROR'));
});

test('an act that throws inside the membrane is drawn as a failure, not as silence', async () => {
  const port = fullPort();
  port.post_candidates_id_cancel = () => { throw new TypeError('this route requires an actor and the membrane was built without one'); };
  const state = await face.read(port);
  const next = await face.act(port, state, { act: 'cancel', id: 'c-005' });
  const html = toHtml(face.view(next));
  assert.ok(html.includes('requires an actor'));
});

// -- held is not settled ---------------------------------------------------------

test('a held row does not wear a receipt\'s face (req/893: the seal cell is gone; the absence of anything to check is stated in the opened row)', async () => {
  const state = await face.read(fullPort());
  assert.equal(find(face.view(state), (n) => n.attrs && n.attrs['data-mark'] === 'structure/seal').length, 0, 'something on this screen is drawn sealed');
  const opened = face.view({ ...state, selected: 'c-004' });
  const seal = findByAttr(opened, 'data-name', 'seal')[0];
  assert.ok(seal, 'the opened held row does not state the seal hole');
  assert.ok(textOf(seal).includes(LEDGER_MESSAGES.NOTHING_TO_SEAL));
});

test('nothing is drawn as sealed while no verifier is present', async () => {
  const { html } = await draw(fullPort());
  assert.equal(html.includes('"sealed"'), false);
  assert.equal(/aria-label="sealed"/.test(html), false);
  assert.ok(html.includes(LEDGER_MESSAGES.NO_VERIFIER_HERE));
});

test('the engine\'s consistency word is carried up and is not called verification', async () => {
  const { tree } = await draw(fullPort());
  const words = textOf(noteOf(tree, 'consistency'));
  assert.ok(words.includes('consistent'));
  assert.ok(words.includes('true'));
  assert.ok(words.includes(LEDGER_MESSAGES.NOT_VERIFICATION));
});

test('claims that do not hold are shown, not filtered out', async () => {
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1), SAMPLE.transformation(3)]),
  });
  const { tree } = await draw(port);
  const names = findByAttr(noteOf(tree, 'claims'), 'data-role', 'detail-line').map((l) => l.attrs['data-name']);
  assert.ok(names.includes('does not hold'), 'every claim held, which means nothing was really checked');
  assert.ok(names.includes('holds'));
});

function attrOf(tree, name) {
  return find(tree, (n) => name in n.attrs).map((n) => n.attrs[name]);
}

test('the opened row says nothing twice under one name (req/893 AC-3: the pane became the opened row; Owner directive #335, 3 still holds there)', async () => {
  const state = await face.read(fullPort());
  const first = state.settled.items[0];
  const tree = face.view({ ...state, selected: first.id });
  const detail = findByAttr(tree, 'data-part', 'row-detail')[0];
  assert.ok(detail, 'no opened row was drawn');
  assert.equal(detail.attrs['data-detail-for'], first.id, 'the opened row describes the chosen row and no other');
  const names = findByAttr(detail, 'data-role', 'detail-line').map((line) => line.attrs['data-name']);
  assert.ok(names.length > 0, 'the opened row drew no facts, so this proves nothing');
  assert.equal(new Set(names).size, names.length, `one name used twice in the opened row: ${names.join(', ')}`);
});

test('choosing a row does not change the list: the same rows, in the same order, exactly one of them open (req/893 AC-2)', async () => {
  const state = await face.read(fullPort());
  const first = state.settled.items[0];
  const shut = face.view(state);
  const open = face.view({ ...state, selected: first.id });
  const ids = (t) => rowsOf(t).map((r) => r.attrs['data-row']);
  assert.deepEqual(ids(open), ids(shut), 'choosing a row changed the list');
  assert.equal(findByAttr(shut, 'data-part', 'row-detail').length, 0, 'a detail is drawn under a row nobody chose');
  assert.equal(findByAttr(open, 'data-part', 'row-detail').length, 1, 'choosing a row opened more than its own detail');
  const chosen = rowsOf(open).filter((r) => r.attrs['data-open'] === 'true');
  assert.equal(chosen.length, 1, 'exactly one row is open');
});

test('req/893 D-2/AC-2, reversing the wrap this suite used to require by name: a long value is clipped on the line and carried whole in three reachable places', async () => {
  const long = '/work/notes/very/long/path/that/has/to/be/clipped/rather/than/wrapped.md';
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1, { path: long })]),
  });
  const { tree, html } = await draw(port);
  const row = rowsOf(tree)[0];
  assert.equal(row.attrs['data-clipped'], 'true', 'the row does not state that it cut a value');
  const pathCell = row.children[5];
  assert.match(pathCell.attrs.style ?? '', /white-space:\s*nowrap/, 'the path cell wraps, which is the defect req/893 S0 (2) names');
  assert.ok((pathCell.attrs.title ?? '').includes(long), 'the whole value does not travel with the cut one');
  assert.ok((pathCell.attrs.title ?? '').includes(LEDGER_MESSAGES.CLIP_ONE_POLICY), 'the line does not say which policy cut it');
  assert.ok(html.includes(long), 'the whole value is nowhere on the page');
  const cut = noteOf(tree, 'cut on this line');
  assert.ok(cut && textOf(cut).includes(long), 'the cut value is not written out whole behind the entrance');
  const state = await face.read(port);
  const opened = face.view({ ...state, selected: 't-001' });
  assert.ok(textOf(findByAttr(opened, 'data-name', 'path')[0]).includes(long), 'the opened row does not carry the whole value');
});

// -- marks, sizes, positioning ------------------------------------------------

test('C-4: every mark drawn was declared', async () => {
  const { tree } = await draw(fullPort());
  const declared = new Set(DECLARATION.marks.map((m) => m.mark));
  const drawn = [...new Set(attrOf(tree, 'data-mark'))];
  assert.ok(drawn.length > 0, 'no marks were drawn, so this proves nothing');
  for (const mark of drawn) assert.ok(declared.has(mark), `undeclared mark on screen: ${mark}`);
});

test('C-5: no meaning is carried by two different marks', async () => {
  const { tree } = await draw(fullPort());
  const seen = new Map();
  // Drawings only. A node that names a meaning and carries no mark of its own is a
  // container around the drawing that does (the standing pill on a box head is one),
  // and counting its absent mark as a second mark says one drawing is two.
  for (const node of find(tree, (n) => 'data-means' in n.attrs && n.attrs['data-mark'])) {
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

test('no colour is spelled out anywhere in what is drawn', async () => {
  const { html } = await draw(fullPort());
  assert.equal(/#[0-9a-fA-F]{3,8}\b/.test(html), false);
  assert.equal(/\brgba?\(/.test(html), false);
});

test('no borrowed symbol is drawn, and the only non-ascii on the page are the two cut marks the policy names (req/893: a declared cut ends in an ellipsis; a denominator\'s parts are joined by an interpunct)', async () => {
  const long = '/work/notes/very/long/path/that/has/to/be/clipped/rather/than/wrapped.md';
  const { html } = await draw(fullPort({
    get_transformations: page([SAMPLE.transformation(1, { path: long })]),
  }));
  for (const symbol of ['●', '◆', '◇', '◈', '▾', '▴', '★', '■', '⏺']) {
    assert.equal(html.includes(symbol), false, `borrowed symbol on screen: ${symbol}`);
  }
  // eslint-disable-next-line no-control-regex
  const nonAscii = [...new Set(html.match(/[^\x00-\x7F]/g) ?? [])];
  assert.deepEqual(nonAscii.filter((c) => c !== '…' && c !== '·'), [], `a non-ascii character outside the declared pair reached the screen: ${nonAscii.join(' ')}`);
  assert.ok(nonAscii.includes('…'), 'the declared cut never drew its mark, so the allowance above checked nothing');
});

// -- the parts are a seam -------------------------------------------------------

test('the parts are injected: a face built on a stub draws the stub', async () => {
  const marker = 'this glyph came from the stub';
  const stub = {
    ...parts,
    // req/893: the face no longer draws rows through a row part -- the seam it
    // reaches through for every mark is the glyph, so the glyph is what proves the
    // seam. A face that imported its parts directly would draw the real one.
    glyph: () => el('span', { 'data-part': 'stub-glyph' }, [marker]),
  };
  const other = createFace({ parts: stub });
  const state = await other.read(fullPort());
  const html = toHtml(other.view(state));
  assert.ok(html.includes(marker));
});

/**
 * The one field on this screen that is a reading of the moment it was taken, blanked so
 * the rest of the tree can be compared for what it is: a function of the state. Two
 * renders of one state that differed anywhere else would be a face with a memory.
 */
const withoutTheMeasurement = (html) => html
  .replace(/data-render-ms="[^"]*"/g, 'data-render-ms="(a measurement)"')
  .replace(/render [\d.]+ ms/g, 'render (a measurement)');

test('rendering the same state twice gives the same tree, apart from the field that is a measurement', async () => {
  const state = await face.read(fullPort());
  const first = toHtml(face.view(state));
  const second = toHtml(face.view(state));
  assert.equal(withoutTheMeasurement(first), withoutTheMeasurement(second));
  // and the blanking cannot be hiding an absent figure: both renders carried a real one.
  for (const html of [first, second]) {
    const found = /data-render-ms="([\d.]+)"/.exec(html);
    assert.ok(found, 'no measured render figure was drawn, so the comparison above blanked nothing');
    assert.ok(Number(found[1]) > 0);
  }
});

// -- the head and the one entrance (req/893 S0 defect 5, AC-5; was the SS657 retrofit) --

test('req/893, was SS657 defect 4/5: the head is one line -- the face name, its question, the figures -- and each half states its own denominator', async () => {
  const { tree } = await draw(fullPort());
  const head = findByAttr(tree, 'data-part', 'ledger-head')[0];
  assert.ok(head, 'a head line is drawn');
  assert.equal(tree.children[0], head, 'the head is not the first thing in the frame');
  assert.ok(textOf(head).includes('ledger'));
  assert.ok(textOf(head).includes(DECLARATION.question));
  assert.deepEqual(
    findByAttr(head, 'data-role', 'figure').map((f) => f.attrs['data-noun']),
    ['settled', 'admit', 'deny', 'escalate', 'held'],
  );
  for (const key of ['settled', 'held']) {
    assert.match(textOf(headOf(halfOf(tree, key))), /\d+ of \d+/, `${key} states no denominator on its own line`);
  }
});

test('req/893 AC-5, was SS657 defect 2: one entrance, one line, carrying the count of what is behind it', async () => {
  const { tree } = await draw(fullPort());
  const entrance = entranceOf(tree);
  assert.ok(entrance, 'no entrance is drawn');
  assert.equal(findByAttr(tree, 'data-part', 'ledger-aside').length, 1, 'more than one entrance -- six doors was the defect');
  assert.equal(tree.children[1], entrance, 'the entrance does not sit between the head and the data');
  assert.equal(entrance.attrs['data-open'], 'false', 'the entrance forces itself open');
  const count = Number(entrance.attrs['data-count']);
  assert.ok(count > 0);
  const noteKeys = find(entrance, (n) => n.attrs && n.attrs['data-note']).map((n) => n.attrs['data-note']);
  assert.equal(noteKeys.length, count, 'the count on the door is not the count of what is behind it');
  // everything the six disclosures said is still here, behind the one door.
  for (const key of ['why', 'order', 'legend', 'claims', 'consistency', 'omitted', 'where from']) {
    assert.ok(noteKeys.includes(key), `the ${key} note did not survive the collapse to one entrance`);
  }
  const summary = find(entrance, (n) => n.tag === 'summary')[0];
  assert.ok(textOf(summary).includes('about this screen'));
  assert.ok(textOf(summary).includes(String(count)), 'the entrance carries no figure, so it is a door with a label on it');
});

test('the legend is a zero-inclusive counted table -- every declared mark gets a line, including ones this render drew none of (req/768 F-B, kept through req/893)', async () => {
  const { tree } = await draw(fullPort());
  const lines = findByAttr(noteOf(tree, 'legend'), 'data-role', 'detail-line');
  const declared = new Set(DECLARATION.marks.map((m) => m.mark));
  assert.equal(lines.length, declared.size, 'every declared mark has a legend line, not only the ones this render happened to draw');
  const countOf = new Map(lines.map((l) => [l.attrs['data-name'], Number(textOf(l.children[1]))]));
  assert.equal(countOf.get('verdict/Admit'), 3, 'the legend count is not a live census of this screen');
  assert.ok([...countOf.values()].some((v) => v === 0), 'no declared mark was undrawn on this state, so the zero rows checked nothing');
  // every content mark actually drawn in the halves reports a positive, live count.
  const contentMarks = new Set();
  for (const half of findByAttr(tree, 'data-part', 'half')) {
    for (const n of find(half, (x) => x.attrs && 'data-mark' in x.attrs)) contentMarks.add(n.attrs['data-mark']);
  }
  assert.ok(contentMarks.size > 0, 'the halves drew no marks, so this proves nothing');
  for (const mark of contentMarks) {
    assert.ok(countOf.get(mark) > 0, `${mark} was drawn but the legend counts none of it`);
  }
});

test('was "the legend also carries a not-drawn row": every undrawn declaration is exactly one line behind the entrance, none silently dropped', async () => {
  const { tree } = await draw(fullPort());
  const lines = findByAttr(noteOf(tree, 'omitted'), 'data-role', 'detail-line');
  assert.equal(lines.length, DECLARATION.undrawn.length, 'the omitted note does not carry one line per undrawn declaration');
  assert.ok(lines.length > 0);
  assert.deepEqual(
    lines.map((l) => l.attrs['data-name']).sort(),
    DECLARATION.undrawn.map((u) => u.what).sort(),
  );
});

test('a mark this read holds none of is an honest zero in the legend, never a dropped line (req/893: zero and absent are different facts)', async () => {
  const { tree } = await draw(fullPort());
  // this read holds no denied row anywhere, and the figure says 0 -- the legend line
  // for the Deny mark must exist and agree, not vanish.
  assert.equal(figureOf(tree, 'deny').attrs['data-value'], '0');
  const line = findByAttr(noteOf(tree, 'legend'), 'data-name', 'verdict/Deny')[0];
  assert.ok(line, 'the legend dropped a declared mark for being zero');
  assert.equal(Number(textOf(line.children[1])), 0);
});

test('req/893: the rows carry no constant dressed as a figure -- the member count is gone by ruling, and a row is a keyboard-reachable control', async () => {
  const { tree } = await draw(fullPort());
  const rows = rowsOf(tree);
  assert.ok(rows.length > 0, 'no rows were drawn, so this proves nothing');
  for (const row of rows) {
    assert.equal(row.tag, 'button', 'a control a keyboard cannot reach is not a control');
    assert.ok(row.attrs['data-select-row'], 'the control does not name the row it opens');
    assert.equal(row.attrs['aria-expanded'], 'false');
  }
  // "N fields" read the same on every row of a half: a constant wearing a figure's
  // clothes (req/893 UNDRAWN, 'a count of members on the line'). It must not return.
  assert.equal(findByAttr(tree, 'data-role', 'field-count').length, 0, 'the constant member count is back on the rows');
});

test('choosing a row is a decision this window makes and never a request: the same read, a different subject', async () => {
  const port = fullPort();
  const state = await face.read(port);
  const callsBefore = port.calls.length;
  const first = state.settled.items[0];
  const tree = face.view({ ...state, selected: first.id });
  assert.equal(port.calls.length, callsBefore, 'drawing a selection reached the port');
  const chosen = rowsOf(tree).find((r) => r.attrs['data-select-row'] === first.id);
  assert.equal(chosen.attrs['aria-expanded'], 'true');
  assert.equal(chosen.attrs['data-open'], 'true');
  const detail = findByAttr(tree, 'data-part', 'row-detail')[0];
  assert.equal(detail.attrs['data-detail-for'], first.id);
  assert.ok(findByAttr(detail, 'data-role', 'detail-line').length > 0, 'the opened row states nothing about its subject');
});

test('req/893 AC-3: with nothing chosen there is no pane at all -- a panel whose only content is that it has no content is furniture', async () => {
  const { tree, html } = await draw(fullPort());
  assert.equal(findByAttr(tree, 'data-part', 'row-detail').length, 0);
  assert.equal(findByAttr(tree, 'data-part', 'detail-pane').length, 0, 'the standing pane this rebuild removed is back');
  assert.equal(html.includes('no row is open'), false, 'the screen spends words saying nothing is open');
});

// -- retrofit round 2, re-derived: acts inside the row (req/893 AC-1, was AC-4's gutter) --

test('req/893 AC-1, was AC-4\'s gutter: zero act controls at rest; an open held row offers the three its half declares', async () => {
  const state = await face.read(fullPort());
  const rest = face.view(state);
  assert.equal(actButtonsIn(rest).length, 0, 'an act control is standing on the screen at rest');
  assert.equal(findByAttr(rest, 'data-part', 'act-gutter').length, 0, 'the gutter this rebuild removed is back');
  const opened = face.view({ ...state, selected: 'c-004' });
  const bar = findByAttr(opened, 'data-part', 'act-bar')[0];
  assert.ok(bar, 'the opened held row offers no acts at all');
  assert.equal(bar.attrs['data-count'], '3', 'commit, cancel, escalate -- every act this half declares');
  assert.deepEqual(
    find(bar, (n) => n.tag === 'button').map((b) => b.attrs['data-act']),
    ['commit', 'cancel', 'escalate'],
  );
});

test('req/893 AC-1: a withheld act still draws a visibly-disabled slot with its reason on the button itself, never blank space', async () => {
  const state = await face.read(fullPort());
  const opened = face.view({ ...state, selected: 'c-004' });
  const escalate = find(opened, (n) => n.tag === 'button' && n.attrs['data-act'] === 'escalate')[0];
  assert.ok(escalate, 'escalate is drawn, not omitted');
  assert.equal(escalate.attrs.disabled, '');
  assert.match(escalate.attrs.title, /Declared, offered, and dimmed/);
});

test('req/893 AC-1: an open settled row offers exactly its one undo act, live', async () => {
  const state = await face.read(fullPort());
  const opened = face.view({ ...state, selected: 't-001' });
  const bar = findByAttr(opened, 'data-part', 'act-bar')[0];
  assert.ok(bar, 'the opened settled row offers no acts');
  assert.equal(bar.attrs['data-count'], '1', 'only undo is offered on a settled row');
  const button = find(bar, (n) => n.tag === 'button')[0];
  assert.equal(button.attrs['data-act'], 'undo');
  assert.equal(button.attrs.disabled, undefined, 'undo sends, so it is not disabled');
});

test('AC-7: a settled row a later row names as its predecessor wears the reversed reading on the line, with the reverser named on hover', async () => {
  const port = fullPort({
    get_transformations: page([
      SAMPLE.transformation(1),
      SAMPLE.transformation(2, { undo_of: 't-001', effect: 'undo' }),
    ]),
  });
  const { tree } = await draw(port);
  const cell = find(tree, (n) => n.attrs && n.attrs['data-standing'] === 'reversed')[0];
  assert.ok(cell, 'no reversed reading is drawn');
  assert.match(cell.attrs.title ?? '', /t-002/, 'the full reason names the reversing row, reachable on hover');
  assert.ok(find(cell, (n) => n.attrs && n.attrs['data-mark'] === 'standing/reversed')[0], 'the reversed reading carries no mark');
});

test('AC-7 + req/893 D-6: what is not observable is not chipped on every row -- it is stated once, in the opened row, named as this window\'s own conclusion', async () => {
  const state = await face.read(fullPort());
  // req/893 UNDRAWN, 'a reversibility chip on every settled row': "unknown" on most
  // rows is a column repeating that nothing was found out. The chip must not return.
  assert.equal(find(face.view(state), (n) => n.attrs && n.attrs['data-mark'] === 'standing/none').length, 0, 'an unknown chip is standing on the list again');
  const opened = face.view({ ...state, selected: 't-003' });
  const line = findByAttr(opened, 'data-name', 'reversal')[0];
  assert.ok(line, 'the opened row does not state the reversal question at all');
  assert.equal(line.attrs['data-provenance'], 'inferred-here');
  assert.match(textOf(line), /membrane\/wire-fields\.json/, 'the honest reason names the declared backend hole, not a fabricated status');
  assert.ok(textOf(line).includes(LEDGER_MESSAGES.INFERRED_HERE), 'the window\'s own conclusion is not labelled as the window\'s');
  assert.ok(find(line, (n) => n.attrs && n.attrs['data-mark'] === 'structure/outside')[0], 'the inferred-here line carries no mark');
});

test('AC-7: a held row carries no reversal reading at rest, and its opened row says why -- nothing has happened yet', async () => {
  const state = await face.read(fullPort());
  const held = halfOf(face.view(state), 'held');
  for (const cell of find(held, (n) => n.attrs && 'data-standing' in n.attrs)) {
    assert.equal(cell.attrs['data-standing'], 'nothing to say', 'a held row wears a standing it has not got');
  }
  const opened = face.view({ ...state, selected: 'c-004' });
  const line = findByAttr(opened, 'data-name', 'reversal')[0];
  assert.match(textOf(line), /no escrowed inverse to hold/);
});

test('AC-7, was the legend\'s three states: the chip\'s removal is itself declared with its reason, behind the entrance', async () => {
  const { tree } = await draw(fullPort());
  const words = textOf(noteOf(tree, 'omitted'));
  assert.ok(words.includes('a reversibility chip on every settled row'), 'the removed chip is not declared as removed');
  assert.match(words, /unknown/, 'the reason does not name what the chip used to repeat');
});

test('C-4 continued: standing/reversed and standing/none are declared marks, reachable and not fabricated', () => {
  const marks = new Set(DECLARATION.marks.map((m) => m.mark));
  assert.ok(marks.has('standing/reversed'));
  assert.ok(marks.has('standing/none'));
});

// -- closed by default (req/97 gap-list item gap 1; req/893 AC-2: no auto-open) ------------

test('every row of an ordinary read is drawn shut, and the time column is a declared cut whose whole member is in the opened row', async () => {
  const state = await face.read(fullPort());
  const tree = face.view(state);
  const rows = rowsOf(tree);
  assert.ok(rows.length > 0, 'no rows were drawn, so this proves nothing');
  for (const r of rows) assert.equal(r.attrs['data-open'], 'false', 'a row made itself the subject');
  const atCell = rows.find((r) => r.attrs['data-row'] === 't-001').children[1];
  assert.notEqual(textOf(atCell), settledItems[0].at, 'the time cell is not cut here, so this proves nothing');
  const opened = face.view({ ...state, selected: 't-001' });
  assert.ok(textOf(findByAttr(opened, 'data-name', 'at')[0]).includes(settledItems[0].at), 'the whole timestamp is not in the opened row');
});

test('negative control: a genuinely over-budget value marks its own row as clipped, and opens nothing (req/893: the whole value travels with the cut one)', async () => {
  const long = '/work/notes/very/long/path/that/has/to/be/clipped/rather/than/wrapped.md';
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1), SAMPLE.transformation(2, { path: long })]),
  });
  const { tree } = await draw(port);
  const rows = findByAttr(halfOf(tree, 'settled'), 'data-part', 'ledger-row');
  assert.equal(rows.length, 2);
  assert.deepEqual(rows.map((r) => r.attrs['data-clipped']), ['false', 'true'], 'exactly one row had a value to cut');
  for (const r of rows) {
    assert.equal(r.attrs['data-open'], 'false', 'a clipped row auto-opened, which is the reflow req/893 exists to remove');
  }
});

// -- the head figures (req/893; was Owner #340's band, boxes, standings) ----------
//
// The facts survive the redesign: a figure a reader lands on before a word, a zero
// that is a zero and a dash that is an unread, and two named groups with their own
// counts. What changed by ruling is the chrome -- tiles, boxes and hue-coded
// standings collapsed into one head line and two half sections (req/893 S0 4 and 5).

test('the head figures state the size and the shape of this screen before a word is read', async () => {
  const { tree } = await draw(fullPort());
  const nouns = findByAttr(tree, 'data-role', 'figure').map((f) => f.attrs['data-noun']);
  assert.deepEqual(nouns, ['settled', 'admit', 'deny', 'escalate', 'held']);
  // every figure is counted from the state this face read, and not one of them is typed
  // by hand: three settled rows, all Admit, and two held candidates.
  assert.equal(figureOf(tree, 'settled').attrs['data-value'], String(settledItems.length));
  assert.equal(figureOf(tree, 'admit').attrs['data-value'], '3');
  assert.equal(figureOf(tree, 'held').attrs['data-value'], String(heldItems.length));
});

test('a verdict this read holds none of still gets its figure, and the figure is a zero', async () => {
  const { tree } = await draw(fullPort());
  for (const noun of ['deny', 'escalate']) {
    const figure = figureOf(tree, noun);
    assert.ok(figure, `${noun} was dropped from the head because this read had none`);
    assert.equal(figure.attrs['data-value'], '0');
    assert.ok(textOf(figure).includes('0'), `${noun} states no figure at all`);
  }
});

test('a half that could not be read draws a dash in the head, never a zero', async () => {
  const { tree } = await draw(fullPort({ get_transformations: failed() }));
  for (const noun of ['settled', 'admit', 'deny', 'escalate']) {
    const figure = figureOf(tree, noun);
    assert.equal(figure.attrs['data-value'] ?? null, null, `${noun} claimed a count off a list that never arrived`);
    assert.ok(textOf(figure).includes(parts.statDash));
  }
  // the half that did answer still states its own figure: one unread list does not
  // blank the screen.
  assert.equal(figureOf(tree, 'held').attrs['data-value'], String(heldItems.length));
});

test('req/893 AC-4, reversing the four-ink rule this suite used to assert: the hue budget is one, spent on reversibility, and the verdicts are told apart by mark and weight', () => {
  const report = lattice();
  assert.deepEqual(report.breaches, [], 'the lattice holds a breach');
  assert.ok(report.accentIntents.length > 0, 'nothing carries the accent, so the budget bought nothing');
  const accentInks = new Set(report.accentIntents.map((intent) => ink(intent).color));
  assert.equal(accentInks.size, 1, `the accent resolves to ${accentInks.size} inks: ${[...accentInks].join(', ')}`);
  const verdicts = ['verdict.admit', 'verdict.deny', 'verdict.escalate'];
  assert.equal(new Set(verdicts.map((v) => ink(v).color)).size, 1, 'a verdict is told apart by hue, which a monochrome print loses');
  assert.equal(new Set(verdicts.map((v) => JSON.stringify(markOfIntent(v)))).size, 3, 'two verdicts share a mark');
  assert.equal(new Set(verdicts.map((v) => ink(v)['font-weight'])).size > 1, true, 'the verdicts do not differ in weight either');
});

test('each half is a named group with its own count in its own words: a candidate is not a record', async () => {
  const { tree } = await draw(fullPort());
  const halves = findByAttr(tree, 'data-part', 'half');
  assert.deepEqual(halves.map((h) => h.attrs['data-half']), ['settled', 'held']);
  assert.ok(textOf(headOf(halfOf(tree, 'settled'))).includes('3 of 3 records'));
  assert.ok(textOf(headOf(halfOf(tree, 'held'))).includes('2 of 2 candidates'));
  // and the rows are still inside the half that names them.
  assert.equal(findByAttr(halfOf(tree, 'held'), 'data-part', 'ledger-row').length, heldItems.length);
});

test('what this window has sent is a group too, so it is drawn as one', async () => {
  const port = fullPort();
  const state = await face.read(port);
  const after = await face.act(port, state, { act: 'commit', id: 'c-004' });
  const tree = face.view(after);
  const log = findByAttr(tree, 'data-part', 'act-log')[0];
  assert.ok(log, 'the act log is not drawn as its own group');
  assert.equal(log.attrs['data-count'], '1');
  assert.equal(findByAttr(log, 'data-role', 'detail-line')[0].attrs['data-name'], 'commit c-004');
  // it sits after the halves and before the strip at the foot.
  assert.equal(tree.children[tree.children.length - 2], log, 'the act log is not the last group before the footer');
});

test('was "the held box wears its standing and the settled box wears none": no half wears a standing chip, and the settled half is never marked held (req/893 S0 defect 4: the pill was chrome)', async () => {
  const { tree } = await draw(fullPort());
  // The standing pill on a box head was chrome repeated per group; the halves now
  // wear their own words (records / candidates) and nothing else. What must never
  // return: a settled row or half wearing the held mark, or any half wearing a chip.
  assert.equal(findByAttr(tree, 'data-part', 'standing-chip').length, 0, 'a standing chip is back on a group head');
  assert.equal(find(halfOf(tree, 'settled'), (n) => n.attrs && n.attrs['data-mark'] === 'standing/held').length, 0, 'the settled half was given a standing it has not got');
});

test('an empty half keeps its group and says nought; an unread one says neither nought nor a number', async () => {
  const empty = await draw(fullPort({ get_candidates: page([]) }));
  const emptyHalf = halfOf(empty.tree, 'held');
  assert.ok(emptyHalf, 'an empty half lost its group');
  assert.ok(textOf(headOf(emptyHalf)).includes('0 of 0 candidates'));
  assert.ok(textOf(emptyHalf).includes(LEDGER_MESSAGES.EMPTY_HELD));

  const unread = await draw(fullPort({ get_candidates: failed() }));
  const unreadHalf = halfOf(unread.tree, 'held');
  assert.ok(textOf(headOf(unreadHalf)).includes(LEDGER_MESSAGES.UNREAD_DENOMINATOR), 'an unread half stated a denominator it has not got');
  assert.ok(textOf(unreadHalf).includes(LEDGER_MESSAGES.UNREAD));
  assert.ok(textOf(figureOf(unread.tree, 'held')).includes(parts.statDash));
});

test('the last thing on the screen is what this screen cost, and the cost is measured', async () => {
  const { tree } = await draw(fullPort());
  const footer = tree.children[tree.children.length - 1];
  assert.equal(footer.attrs['data-part'], 'runtime-footer', 'the footer is not the last node this face returns');
  const measured = Number(footer.attrs['data-render-ms']);
  assert.ok(Number.isFinite(measured) && measured > 0, `the render figure is not a measurement: ${footer.attrs['data-render-ms']}`);
  assert.deepEqual(
    findByAttr(footer, 'data-role', 'footer-field').map((f) => f.attrs['data-name']),
    ['render', 'read', 'suite', 'build'],
  );
  const read = findByAttr(footer, 'data-name', 'read')[0];
  assert.equal(textOf(read).includes(parts.statDash), false, 'the face drew a dash for a read it actually performed');
});

test('a screen that read nothing says so in the footer rather than naming a source it never reached', async () => {
  const port = fullPort({
    get_transformations: failed(),
    get_candidates: failed(),
    get_ledger_consistency: absent({ name: 'get_ledger_consistency' }),
  });
  const { tree } = await draw(port);
  const footer = tree.children[tree.children.length - 1];
  const read = findByAttr(footer, 'data-name', 'read')[0];
  assert.ok(textOf(read).includes(parts.statDash), 'a face that read nothing named a source anyway');
});

// -- what a hand does to this window (req/103, findings 1 to 3) ----------------------
//
// Everything below is about presses rather than about trees, and every behaviour it
// asserts survived req/893 unchanged: one queue, in-flight said on the button, and a
// window that does not throw away what a reader decided. What changed by ruling is
// where the buttons are -- an act button exists only inside an open row (AC-1), so
// every press of an act is preceded by the press that opens its row.

const actButton = (host, act, id) => nodesOf(host).find((n) => n.tag === 'button'
  && n.attrs?.['data-act'] === act && n.attrs?.['data-target'] === id) ?? null;
const openRow = (host, id) => press(host, nodeWith(host, 'data-select-row', id));

test('req/103 finding 1: two presses of one act button in the same tick record two acts, and lose neither', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  openRow(host, 'c-004');
  // Both presses land before the first has answered, which is what a fast double
  // click is: the second event is dispatched against the screen the first one has
  // not repainted yet.
  press(host, actButton(host, 'commit', 'c-004'));
  press(host, actButton(host, 'commit', 'c-004'));
  await unmount.quiet();
  assert.equal(port.calls.filter((c) => c.name === 'post_candidates_id_commit').length, 2, 'one press never reached the port');
  const log = nodeWith(host, 'data-part', 'act-log');
  assert.ok(log, 'nothing was written down at all');
  assert.equal(log.attrs['data-count'], '2', 'an act was sent and the record of it was overwritten by the other');
  unmount();
});

test('req/103 finding 1: two different acts pressed in the same tick both survive, and in order', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  openRow(host, 'c-004');
  press(host, actButton(host, 'commit', 'c-004'));
  press(host, actButton(host, 'cancel', 'c-004'));
  await unmount.quiet();
  const log = nodeWith(host, 'data-part', 'act-log');
  assert.equal(log.attrs['data-count'], '2');
  assert.match(textOfHost(host), /commit c-004/);
  assert.match(textOfHost(host), /cancel c-004/);
  unmount();
});

test('req/103 finding 3: an act that has been sent says so on its own button until it is answered', async () => {
  const host = standInHost();
  let answer = null;
  const waiting = new Promise((resolve) => { answer = resolve; });
  const port = fullPort({ post_candidates_id_commit: () => waiting });
  const unmount = mount(host, port, []);
  await unmount.ready;
  openRow(host, 'c-004');
  assert.equal(actButton(host, 'commit', 'c-004').attrs.disabled, undefined, 'nothing is in flight yet');
  press(host, actButton(host, 'commit', 'c-004'));
  await null;
  const sending = actButton(host, 'commit', 'c-004');
  assert.equal(sending.attrs.disabled, '', 'the button stayed live while its own act was in flight');
  assert.match(sending.attrs.title ?? '', /waiting/, 'a dimmed control that does not say why is a control that reads as broken');
  // the other acts on the same row are not disabled by somebody else's act.
  assert.equal(actButton(host, 'cancel', 'c-004').attrs.disabled, undefined);
  answer({ outcome: 'answered', status: 202, body: { id: 'c-004' } });
  await unmount.quiet();
  assert.equal(actButton(host, 'commit', 'c-004').attrs.disabled, undefined, 'the button never came back');
  unmount();
});

test('req/103 finding 2: the entrance a reader opened is still open after they choose a row', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  const entrance = () => nodeWith(host, 'data-peripheral', 'about this screen');
  assert.equal(entrance().attrs['data-open'], 'false');
  press(host, entrance().childNodes[0]);
  assert.equal(entrance().attrs['data-open'], 'true', 'pressing the entrance did not open it');
  press(host, nodeWith(host, 'data-select-row', 't-001'));
  assert.equal(entrance().attrs['data-open'], 'true', 'choosing a row shut the entrance the reader had opened');
  press(host, entrance().childNodes[0]);
  assert.equal(entrance().attrs['data-open'], 'false', 'the entrance could be opened and never shut');
  unmount();
});

test('req/103 finding 2: an act does not shut what a reader opened, and does not forget which row they were reading', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  press(host, nodeWith(host, 'data-peripheral', 'about this screen').childNodes[0]);
  openRow(host, 't-001');
  press(host, actButton(host, 'undo', 't-001'));
  await unmount.quiet();
  assert.equal(nodeWith(host, 'data-peripheral', 'about this screen').attrs['data-open'], 'true', 'an act shut the entrance the reader was reading');
  const detail = nodeWith(host, 'data-part', 'row-detail');
  assert.ok(detail, 'an act shut the row the reader had open');
  assert.equal(detail.attrs['data-detail-for'], 't-001', 'an act threw away the row the reader had open');
  unmount();
});

test('a caller that states where its state came from is taken at its word', async () => {
  const state = await face.read(fullPort());
  const said = 'a stand-in, not an engine';
  const tree = face.view({ ...state, source: said });
  const read = findByAttr(tree, 'data-name', 'read')[0];
  assert.ok(textOf(read).includes(said), 'the stated source was overwritten by a word the face chose');
});

// -- the other button, judged (req/893 D-8) -----------------------------------------
//
// The row menu was a second place a row's acts appeared -- the same chrome-per-row
// defect as the gutter, in a second coat -- and req/893 D-8 rules its removal as an
// act surface correct. What this block asserts now is the ruling itself: acts appear
// in exactly one place, a right-click opens nothing and sends nothing, and no repaint
// or key can resurrect a surface that does not exist. The wiring for a menu decision
// still exists in the window's state (read() carries `menu` across, and the handlers
// still stand); until the reimplement-copy lane gives it a drawing, these tests hold
// that no drawing appears.

const menusIn = (host) => nodesOf(host).filter((n) => n.attrs?.['data-role'] === 'row-menu');

test('req/893 D-8, was the menu/gutter parity test: acts appear in exactly one place, and a right-click opens no second surface', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  assert.equal(menusIn(host).length, 0, 'a row menu was opened -- a second act surface is back');
  openRow(host, 'c-004');
  const offers = nodesOf(host).filter((n) => n.tag === 'button'
    && n.attrs?.['data-act'] === 'commit' && n.attrs?.['data-target'] === 'c-004');
  assert.equal(offers.length, 1, 'one act is offered on more than one surface');
  unmount();
});

test('req/893 D-8, was the right-click-on-an-act test: a right-click on an act button sends nothing and opens nothing', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  openRow(host, 'c-004');
  const sent = port.calls.length;
  rightPress(host, actButton(host, 'commit', 'c-004'));
  await unmount.quiet();
  assert.equal(menusIn(host).length, 0);
  assert.equal(port.calls.length, sent, 'a right-click reached the port');
  unmount();
});

test('req/893 D-8, was the withheld-in-menu test: the one surface carries the declaration\'s own reason, verbatim', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  openRow(host, 'c-004');
  const escalate = actButton(host, 'escalate', 'c-004');
  assert.ok(escalate, 'a withheld act was dropped from the row, which reads as never offered');
  assert.equal(escalate.attrs.disabled, '');
  const declared = DECLARATION.acts.find((a) => a.act === 'escalate');
  assert.equal(escalate.attrs.title, declared.why, 'the surface invented its own reason');
  unmount();
});

test('req/893 D-8, was the dimmed-in-both-surfaces test: an act in flight is dimmed on the one surface that offers it, and it is offered exactly once', async () => {
  const host = standInHost();
  let answer = null;
  const waiting = new Promise((resolve) => { answer = resolve; });
  const unmount = mount(host, fullPort({ post_candidates_id_commit: () => waiting }), []);
  await unmount.ready;
  openRow(host, 'c-004');
  press(host, actButton(host, 'commit', 'c-004'));
  await null;
  const offers = nodesOf(host).filter((n) => n.tag === 'button'
    && n.attrs?.['data-act'] === 'commit' && n.attrs?.['data-target'] === 'c-004');
  assert.equal(offers.length, 1, 'the in-flight act is offered on more than one surface');
  assert.equal(offers[0].attrs.disabled, '', 'the surface offered an act that is already in flight');
  assert.equal(offers[0].attrs.title, LEDGER_MESSAGES.IN_FLIGHT);
  answer({ outcome: 'answered', status: 202, body: {} });
  await unmount.quiet();
  unmount();
});

test('req/893 D-8, was the menu-queue test: the acts of one row land on the one queue, in the order they were pressed', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  openRow(host, 'c-004');
  press(host, actButton(host, 'commit', 'c-004'));
  press(host, actButton(host, 'cancel', 'c-004'));
  await unmount.quiet();
  assert.equal(port.calls.filter((c) => c.name === 'post_candidates_id_commit').length, 1);
  assert.equal(port.calls.filter((c) => c.name === 'post_candidates_id_cancel').length, 1);
  const log = nodeWith(host, 'data-part', 'act-log');
  assert.equal(log.attrs['data-count'], '2', 'one of the two acts left no record of itself');
  const entries = nodesOf(log).filter((n) => n.attrs?.['data-role'] === 'detail-line').map((n) => n.attrs['data-name']);
  assert.deepEqual(entries, ['commit c-004', 'cancel c-004'], 'the acts were recorded out of the order they were asked in');
  unmount();
});

test('req/893 D-8, was the menu-stacking test: repeated right-clicks accumulate nothing and change nothing', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  const before = withoutTheMeasurement(textOfHost(host));
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  rightPress(host, nodeWith(host, 'data-select-row', 'c-005'));
  assert.equal(menusIn(host).length, 0, 'a menu accumulated');
  assert.equal(withoutTheMeasurement(textOfHost(host)), before, 'a right-click changed what the screen says');
  unmount();
});

test('req/893 D-8, was the repaint-leaves-a-menu test: no repaint draws a menu, and a fresh read still does not throw away a window decision', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  openRow(host, 'c-004');
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  press(host, actButton(host, 'commit', 'c-004'));
  await unmount.quiet();
  assert.equal(menusIn(host).length, 0, 'a repaint drew a menu');
  // the state contract of req/103 finding 2 outlives the drawing: what this window
  // decided is not the server's to overwrite on a fresh read.
  const state = await face.read(fullPort(), { menu: { id: 'c-005', cell: null } });
  assert.deepEqual(state.menu, { id: 'c-005', cell: null }, 'a fresh read threw away a window decision');
  unmount();
});

test('req/893 D-8, was the Escape test: Escape has no second surface to dismiss, and dismisses nothing else', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  openRow(host, 't-001');
  const before = withoutTheMeasurement(textOfHost(host));
  strike(host.ownerDocument, 'Escape');
  assert.equal(withoutTheMeasurement(textOfHost(host)), before, 'Escape changed a screen with nothing to dismiss');
  assert.ok(nodeWith(host, 'data-part', 'row-detail'), 'Escape shut the row the reader had open');
  unmount();
});

test('req/893 D-8, was the press-away test: a press outside this face dismisses nothing it needs to, and a press inside spends itself on what it hit', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  openRow(host, 'c-004');
  pressAway(host.ownerDocument, { tag: 'div', attrs: {}, parentNode: null });
  assert.ok(nodeWith(host, 'data-part', 'row-detail'), 'a press outside this face shut the row the reader had open');
  // choosing another row is one press: it moves the subject, it is not spent on any
  // dismissal (the same fact the old press-away test asserted through the menu).
  press(host, nodeWith(host, 'data-select-row', 't-001'));
  assert.equal(nodeWith(host, 'data-part', 'row-detail').attrs['data-detail-for'], 't-001');
  unmount();
});

test('unmounting takes this face handlers off the document it does not own', async () => {
  const host = standInHost();
  const doc = host.ownerDocument;
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  assert.ok(doc.listeners.length > 0, 'nothing was put on the document, so this proves nothing');
  unmount();
  assert.deepEqual(doc.listeners, [], 'a face that was taken down is still answering presses');
});

// -- copy, standing red on purpose (req/893 D-8; NOT re-derived) ---------------------
//
// req/893 D-8: "copy was a real capability and its loss is a regression, not a
// simplification." No named ruling has judged the capability itself defective, so
// under the test-change rule these five may not be rewritten into passing shapes --
// their red is the standing record of the regression, and it clears only when the
// reimplement-copy lane (req/893 S8, open TODO) gives the rebuilt screen a copy
// affordance and a copy report again. The five reach copy through the retired row
// menu because that is where the capability lived; the lane that reimplements copy
// re-derives them against wherever it puts the affordance. Ledger: req/108.
//
// req/109 (the reimplement-copy lane, foreseen above): the affordance exists again --
// a right-press on a data cell draws a copy-only strip under that row, and the last
// copy's outcome is a copy-report near the head. Four of the five held as written.
// The fifth needed the one re-derivation the paragraph above authorises: its act
// press reached a commit button standing at rest, which is the gutter req/893
// (S0 defect 1, AC-1) removed, so that test now opens the row first and nothing
// else about it moved. Ledger of the reimplementation: req/109.

const menuOf = (host) => nodeWith(host, 'data-role', 'row-menu');
const menuCopy = (host) => nodesOf(menuOf(host) ?? { childNodes: [] })
  .find((n) => n.attrs?.['data-menu-item'] === 'copy') ?? null;
const cellIn = (host, id, key) => nodesOf(host)
  .find((n) => n.attrs?.['data-cell'] === key && n.closest('[data-row]')?.attrs?.['data-row'] === id) ?? null;

test('copy value names the whole member and never the drawn text of the cell', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  const cell = cellIn(host, 't-001', 'at');
  assert.ok(cell, 'no time cell to point at');
  rightPress(host, cell);
  const copy = menuCopy(host);
  assert.ok(copy, 'a data cell offered no copy item');
  assert.equal(copy.attrs['data-copy-from'], 'at');
  assert.equal(copy.attrs['data-target'], 't-001');
  // the cell draws a declared cut of the timestamp; what the item names is the member,
  // which is the whole of it.
  assert.notEqual(textOfHost(cell), settledItems[0].at, 'the cell is not cut here, so this proves nothing');
  unmount();
});

test('a cell whose member is a declared hole offers the copy item disabled, with the hole own reason', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort({
    get_transformations: page([SAMPLE.transformation(1, { path: undefined })]),
  }), []);
  await unmount.ready;
  rightPress(host, cellIn(host, 't-001', 'path'));
  const copy = menuCopy(host);
  assert.ok(copy, 'a cell with a hole in it offered nothing at all, which reads as no such cell');
  assert.equal(copy.attrs.disabled, '');
  assert.equal(copy.attrs.title, LEDGER_MESSAGES.MEMBER_ABSENT);
  unmount();
});

test('a copy with no clipboard to write to says so, rather than looking exactly like one that worked', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightPress(host, cellIn(host, 't-001', 'path'));
  press(host, menuCopy(host));
  await unmount.quiet();
  const report = nodeWith(host, 'data-role', 'copy-report');
  assert.ok(report, 'nothing on the screen says whether the copy happened');
  assert.equal(report.getAttribute('data-copy-failed'), 'path');
  assert.equal(report.getAttribute('data-copied'), null);
  assert.match(textOfHost(report), /nothing was copied/);
  unmount();
});

test('a copy that reaches a clipboard puts the whole value on it and says it did', async () => {
  const written = [];
  const held = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
  Object.defineProperty(globalThis, 'navigator', {
    value: { clipboard: { writeText: async (text) => { written.push(text); } } },
    configurable: true,
  });
  try {
    const host = standInHost();
    const unmount = mount(host, fullPort(), []);
    await unmount.ready;
    // The time column, on purpose: it is the one cell on this face that draws less
    // than it is about, so it is the one that says whether the clipboard is being
    // handed the member or the drawing of it.
    const cell = cellIn(host, 't-001', 'at');
    assert.notEqual(textOfHost(cell), settledItems[0].at, 'this cell is not cut, so it proves nothing');
    rightPress(host, cell);
    press(host, menuCopy(host));
    await unmount.quiet();
    assert.deepEqual(written, [settledItems[0].at], 'the clipboard was handed the drawn text, not the value');
    const report = nodeWith(host, 'data-role', 'copy-report');
    assert.equal(report.getAttribute('data-copied'), 'at');
    assert.equal(report.getAttribute('data-copy-failed'), null);
    unmount();
  } finally {
    Object.defineProperty(globalThis, 'navigator', held);
  }
});

test('a copy waits behind an act rather than racing it, because there is one queue', async () => {
  const written = [];
  const held = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
  Object.defineProperty(globalThis, 'navigator', {
    value: { clipboard: { writeText: async (text) => { written.push(text); } } },
    configurable: true,
  });
  try {
    const host = standInHost();
    const port = fullPort();
    const unmount = mount(host, port, []);
    await unmount.ready;
    // req/109, under the test-change rule: this line is the one re-derivation in the
    // five. As written for the old screen, the act press reached a commit button
    // standing at rest in the gutter -- the exact behaviour req/893 (2026-08-26,
    // S0 defect 1, AC-1) judged defective, and the re-derivation of these five
    // against wherever the affordance lives is this lane's assignment (req/108 §4).
    // Acts now exist only inside the opened row (the same transformation every F3
    // interaction test took), so the row is opened first. Every assertion below is
    // untouched: the capability -- a copy waits behind an act on the one queue, and
    // neither write to state is lost -- is the ruling-protected part.
    openRow(host, 'c-004');
    rightPress(host, cellIn(host, 't-001', 'path'));
    const copy = menuCopy(host);
    press(host, actButton(host, 'commit', 'c-004'));
    press(host, copy);
    await unmount.quiet();
    // the act is written down and the copy happened: neither write to state was lost
    // under the other.
    assert.equal(nodeWith(host, 'data-part', 'act-log').attrs['data-count'], '1');
    assert.deepEqual(written, [settledItems[0].path]);
    assert.equal(nodeWith(host, 'data-role', 'copy-report').getAttribute('data-copied'), 'path');
    unmount();
  } finally {
    Object.defineProperty(globalThis, 'navigator', held);
  }
});
