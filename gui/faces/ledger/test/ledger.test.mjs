// SPDX-License-Identifier: Apache-2.0
// What the face does, asked of the face and not of a description of it.
//
// The two things these tests are really guarding: a list that could not be read must
// never be drawn as a list with nothing in it, and a row that has been written must
// never be edited afterwards. Everything else here is in service of those two.

import test from 'node:test';
import assert from 'node:assert/strict';

import { createFace, face, mount, LEDGER_MESSAGES } from '../ledger.mjs';
import { DECLARATION } from '../declaration.mjs';
import { parts } from '../binding.mjs';
import {
  standInHost, attrValues, textOfHost, nodesOf, press, nodeWith, rightPress, strike, pressAway,
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

const sectionOf = (tree, name) => findByAttr(tree, 'data-section', name)[0] ?? null;

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

test('C-2: a declared method the face withholds is never sent, and says why on screen', async () => {
  const port = fullPort();
  const { html } = await draw(port);
  for (const entry of DECLARATION.withheld) {
    assert.equal(port.calls.some((c) => c.name === entry.method), false);
    assert.ok(html.includes(entry.why), `the reason for withholding ${entry.method} is not on screen`);
  }
  assert.ok(html.includes('disabled'), 'a withheld act is drawn dimmed, not removed');
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
    const settled = sectionOf(tree, 'settled');
    assert.equal(settled.attrs['data-state'], 'unread');
    assert.ok(textOf(settled).includes(LEDGER_MESSAGES.UNREAD));
    assert.equal(html.includes(LEDGER_MESSAGES.EMPTY_SETTLED), false, 'an unread list was given the empty list\'s words');
    assert.equal(findByAttr(settled, 'data-part', 'receipt-row').length, 0);
    assert.ok(textOf(settled).includes(name), 'the outcome is not named on screen');
  });
}

test('an answered read with no items says so in different words from an unread one', async () => {
  const port = fullPort({ get_transformations: page([]) });
  const { tree } = await draw(port);
  const settled = sectionOf(tree, 'settled');
  assert.equal(settled.attrs['data-state'], 'empty');
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

test('C-3: the walk\'s denominator is on screen, not just the row count', async () => {
  const port = fullPort({ get_transformations: page(settledItems, { pages: 7, stopped: true }) });
  const { tree } = await draw(port);
  const notDrawn = sectionOf(tree, 'not-drawn');
  const words = textOf(notDrawn);
  assert.ok(words.includes('7'), 'the number of requests the walk took is not stated');
  assert.ok(words.includes(LEDGER_MESSAGES.TRUNCATED));
});

test('C-3: rows the order dropped are counted and the reason for each is named', async () => {
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1), { sequence: 2, at: 'x' }, 'not a record']),
  });
  const { tree } = await draw(port);
  const words = textOf(sectionOf(tree, 'not-drawn'));
  assert.ok(words.includes('no-identity'));
  assert.ok(words.includes('not-a-record'));
  assert.ok(words.includes('1 of 3'), 'the drawn-of-received count is not stated');
});

test('C-3: the members this face does not draw are named with reasons', async () => {
  const { tree } = await draw(fullPort());
  const words = textOf(sectionOf(tree, 'not-drawn'));
  for (const entry of DECLARATION.undrawn) assert.ok(words.includes(entry.what), `${entry.what} is declared undrawn but not stated on screen`);
});

// -- order ---------------------------------------------------------------------

test('C-6: the order is stated on screen with its reason', async () => {
  const { tree } = await draw(fullPort());
  const settled = sectionOf(tree, 'settled');
  assert.ok(textOf(settled).includes(DECLARATION.rows.order));
  assert.ok(textOf(settled).includes(DECLARATION.rows.order_reason));
});

test('an order whose assumption breaks is substituted, and the substitution is stated', async () => {
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1), SAMPLE.transformation(2, { sequence: undefined })]),
  });
  const { tree } = await draw(port);
  assert.ok(textOf(sectionOf(tree, 'settled')).includes('as-recorded'));
});

// -- rows are not edited --------------------------------------------------------

test('undo appends a child row and leaves the row it undoes byte-identical', async () => {
  const before = [SAMPLE.transformation(1), SAMPLE.transformation(2)];
  const after = [...before, SAMPLE.transformation(3, { undo_of: 't-002', effect: 'undo' })];
  let reads = 0;
  const port = fullPort({
    get_transformations: () => page(reads++ === 0 ? before : after),
  });

  const first = await face.read(port);
  const firstHtml = toHtml(face.view(first));
  const rowHtml = (html, id) => {
    const at = html.indexOf(`data-row="${id}"`);
    assert.notEqual(at, -1, `row ${id} is not drawn`);
    return html.slice(at, html.indexOf('</div>', at));
  };

  const next = await face.act(port, first, { act: 'undo', id: 't-002' });
  const secondHtml = toHtml(face.view(next));

  assert.equal(rowHtml(secondHtml, 't-002'), rowHtml(firstHtml, 't-002'), 'the undone row was rewritten');
  assert.ok(secondHtml.includes('data-child-of="t-002"'), 'no child row was appended');
  assert.ok(firstHtml.includes('data-row="t-001"') && secondHtml.includes('data-row="t-001"'), 'a row disappeared');
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

test('a held row does not wear a receipt\'s face', async () => {
  const { tree } = await draw(fullPort());
  const held = sectionOf(tree, 'held');
  const rows = findByAttr(held, 'data-part', 'receipt-row');
  assert.equal(rows.length, heldItems.length);
  for (const row of rows) {
    const seal = findByAttr(row, 'data-cell', 'seal')[0];
    assert.equal(seal.attrs['data-state'], 'hole', 'a held row was given a seal cell');
    assert.ok((seal.attrs.title ?? '').includes(LEDGER_MESSAGES.NOTHING_TO_SEAL));
  }
});

test('nothing is drawn as sealed while no verifier is present', async () => {
  const { html } = await draw(fullPort());
  assert.equal(html.includes('"sealed"'), false);
  assert.equal(/aria-label="sealed"/.test(html), false);
  assert.ok(html.includes(LEDGER_MESSAGES.NO_VERIFIER_HERE));
});

test('the engine\'s consistency word is carried up and is not called verification', async () => {
  const { tree } = await draw(fullPort());
  const words = textOf(sectionOf(tree, 'consistency'));
  assert.ok(words.includes('consistent'));
  assert.ok(words.includes('true'));
  assert.ok(words.includes(LEDGER_MESSAGES.NOT_VERIFICATION));
});

test('claims that do not hold are shown, not filtered out', async () => {
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1), SAMPLE.transformation(3)]),
  });
  const { tree } = await draw(port);
  const claims = sectionOf(tree, 'claims');
  const verdicts = attrOf(claims, 'data-holds');
  assert.ok(verdicts.includes('false'), 'every claim held, which means nothing was really checked');
  assert.ok(verdicts.includes('true'));
});

function attrOf(tree, name) {
  return find(tree, (n) => name in n.attrs).map((n) => n.attrs[name]);
}

test('the pane says nothing twice under one name (Owner directive #335, 3: the note moved, the property did not)', async () => {
  const state = await face.read(fullPort());
  const first = state.settled.items[0];
  const tree = face.view({ ...state, selected: first.id });
  const pane = findByAttr(tree, 'data-part', 'detail-pane')[0];
  assert.ok(pane, 'no detail pane was drawn');
  assert.equal(pane.attrs['data-subject'], first.id, 'the pane describes the chosen row and no other');
  const names = findByAttr(pane, 'data-role', 'pane-line').map((line) => line.attrs['data-name']);
  assert.ok(names.length > 0, 'the pane drew no facts, so this proves nothing');
  assert.equal(new Set(names).size, names.length, `one name used twice in the pane: ${names.join(', ')}`);
});

test('no row draws a note under itself any more: the list keeps its geometry whatever is chosen', async () => {
  const state = await face.read(fullPort());
  const first = state.settled.items[0];
  const shut = face.view(state);
  const open = face.view({ ...state, selected: first.id });
  assert.deepEqual(findByAttr(shut, 'data-part', 'receipt-note'), [], 'a note is drawn under a row');
  assert.deepEqual(findByAttr(open, 'data-part', 'receipt-note'), [], 'choosing a row drew a note under it');
  const rowsOf = (t) => findByAttr(t, 'data-part', 'selectable-row').map((r) => r.attrs['data-select-row']);
  assert.deepEqual(rowsOf(open), rowsOf(shut), 'choosing a row changed the list');
  const chosen = findByAttr(open, 'data-part', 'selectable-row').filter((r) => r.attrs['data-selected'] === 'true');
  assert.equal(chosen.length, 1, 'exactly one row is the subject of the pane');
});

test('a value too long for its column is drawn in full on the row itself, because the row wraps rather than cutting it', async () => {
  const long = '/work/notes/very/long/path/that/has/to/be/clipped/rather/than/wrapped.md';
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1, { path: long })]),
  });
  const { tree, html } = await draw(port);
  const block = findByAttr(tree, 'data-role', 'row-block')[0];
  assert.equal(block.attrs['data-open-because'], 'clip-risk', 'the row still states that the grid cannot hold it');
  // Owner directive #335 (3): with no note under the row, the cure for req/03 N-4 is
  // geometry -- the two columns whose length nothing declares wrap instead of
  // clipping, and a wrapping cell cannot lose data.
  const cell = findByAttr(tree, 'data-cell', 'path').find((c) => textOf(c) === long);
  assert.ok(cell, 'the whole value is not in the row cell');
  assert.match(cell.attrs.style, /white-space:normal/, 'the path cell clips instead of wrapping, so the value is lost');
  assert.ok(html.includes(long), 'the whole value is nowhere on the page');
  // The row itself carries a short label, not the full explanation -- repeating that
  // sentence on every clipped row is exactly the boilerplate SS528/RC-2 named. The
  // full sentence lives once, in the legend (req/09 SS528, req/97 RC-2).
  assert.equal(textOf(block).includes(LEDGER_MESSAGES.CLIP_RISK), false, 'the boilerplate sentence no longer repeats on the row');
  // SS657 retrofit: legend is now a bordered controlToggle() control (data-role
  // "control", data-control "legend"), not a full-width data-section band.
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  assert.ok(textOf(legend).includes(LEDGER_MESSAGES.CLIP_RISK), 'the full sentence lives once, in the legend');
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

test('no borrowed symbol is drawn', async () => {
  const { html } = await draw(fullPort());
  for (const symbol of ['●', '◆', '◇', '◈', '▾', '▴', '★', '■', '⏺']) {
    assert.equal(html.includes(symbol), false, `borrowed symbol on screen: ${symbol}`);
  }
  // eslint-disable-next-line no-control-regex
  assert.equal(/[^\x00-\x7F]/.test(html), false, 'a non-ascii character reached the screen');
});

// -- the parts are a seam -------------------------------------------------------

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

// -- SS657 retrofit (req/38 SS657 Owner #317/#318, idiom proven by faces/atlas) --

test('SS657 defect 4/5 cure: a single compact header line states the face name and both denominators, before anything else', async () => {
  const { tree } = await draw(fullPort());
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  assert.ok(header, 'a face-header line is drawn');
  assert.ok(textOf(header).includes('ledger'));
  assert.match(textOf(header), /\d+ of \d+ settled, \d+ of \d+ held/);
  // it is the first thing in the frame, before the control row and every section.
  assert.equal(tree.children[0], header);
});

test('SS657 defect 2 cure: why/legend are bordered, self-evident controls sitting in one row, each with a plain-language hint', async () => {
  const { tree } = await draw(fullPort());
  const row = findByAttr(tree, 'data-role', 'control-row')[0];
  assert.ok(row, 'a control-row is drawn');
  const controls = findByAttr(row, 'data-role', 'control');
  // Owner directive #335 (1): everything explanatory on this screen is behind a
  // click, and every click is in this one row -- claims, consistency and omitted
  // were three always-open bands of prose below the rows before it.
  assert.ok(controls.length >= 2, 'why and legend sit side by side in the same row');
  assert.deepEqual(
    controls.map((c) => c.attrs['data-control']),
    ['why', 'legend', 'claims', 'consistency', 'omitted', 'where from'],
  );
  for (const control of controls) {
    assert.equal(control.attrs['data-open'], 'false', `${control.attrs['data-control']} is not collapsed by default`);
    assert.ok(control.attrs.style.includes('border'), `${control.attrs['data-control']} is a bare word, not a control`);
  }
  const why = findByAttr(row, 'data-control', 'why')[0];
  const legend = findByAttr(row, 'data-control', 'legend')[0];
  assert.ok(why.attrs.style.includes('border'), 'the why control is bordered, not a bare word');
  assert.ok(legend.attrs.style.includes('border'), 'the legend control is bordered, not a bare word');
  // req/822_c7 (Owner #387/#388 冗長文字全掃): the hint no longer draws as its own
  // visible span beside the label -- it rides the control's own summary as a
  // title (a hover) and a data-hint attribute.
  const summaryOf = (control) => find(control, (n) => n.tag === 'summary')[0];
  assert.equal(summaryOf(why).attrs['data-hint'], 'about this screen', 'why carries a plain-language hint on its summary');
  assert.equal(summaryOf(legend).attrs['data-hint'], 'symbols and counts', 'legend carries a plain-language hint on its summary');
  assert.doesNotMatch(textOf(why), /about this screen/, 'the hint is still drawn as visible text');
  assert.doesNotMatch(textOf(legend), /symbols and counts/, 'the hint is still drawn as visible text');
  // collapsed by default -- neither control forces itself open.
  assert.equal(why.attrs['data-open'], 'false');
  assert.equal(legend.attrs['data-open'], 'false');
});

test('SS657 defect 1/3 cure: legend is a zero-inclusive counted table -- every declared mark gets a row, including ones this render drew zero of', async () => {
  const { tree } = await draw(fullPort());
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  const rows = findByAttr(legend, 'data-mark-entry');
  const declaredMarks = new Set(DECLARATION.marks.map((m) => m.mark));
  assert.equal(rows.length, declaredMarks.size, 'every declared mark has a legend row, not only the ones this render happened to draw');
  // The count is a census of this screen's own content (rows/claims/sections),
  // deliberately excluding the header and the why/legend controls' own chrome
  // glyphs (fold-shut/fold-open on the controls themselves would otherwise make
  // the legend partly a census of itself). Content marks only, for this check.
  const controlRow = findByAttr(tree, 'data-role', 'control-row')[0];
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  const contentMarks = new Set();
  for (const child of tree.children) {
    if (child === controlRow || child === header) continue;
    for (const n of find(child, (x) => x.attrs && 'data-mark' in x.attrs)) contentMarks.add(n.attrs['data-mark']);
  }
  const zeroRows = rows.filter((r) => r.attrs['data-count'] === '0');
  assert.ok(zeroRows.length > 0, 'at least one declared mark was not drawn on this state, and still has a row');
  for (const r of zeroRows) assert.equal(contentMarks.has(r.attrs['data-mark-entry']), false);
  // every content mark that IS drawn reports a positive, live count.
  for (const mark of contentMarks) {
    const own = rows.find((r) => r.attrs['data-mark-entry'] === mark);
    assert.ok(own, `${mark} was drawn but has no legend row`);
    assert.ok(Number(own.attrs['data-count']) > 0);
  }
});

test('the legend also carries a not-drawn row (with its reason) for this face\'s own declared unreached set', async () => {
  const { tree } = await draw(fullPort());
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  const notDrawn = findByAttr(legend, 'data-not-drawn');
  assert.equal(notDrawn.length, DECLARATION.undrawn.length);
  assert.ok(notDrawn.length > 0);
  for (const entry of DECLARATION.undrawn) {
    const own = notDrawn.find((n) => n.attrs['data-not-drawn'] === entry.what);
    assert.ok(own, `no legend row for undrawn entry: ${entry.what}`);
    assert.ok(textOf(own).includes(entry.why));
  }
});

test('SS657 defect 1 cure, as Owner directive #335 (3) reshapes it: a row is a keyboard-reachable control stating the count the pane will add', async () => {
  const { tree } = await draw(fullPort());
  const rows = findByAttr(tree, 'data-part', 'selectable-row');
  assert.ok(rows.length > 0, 'no rows were drawn, so this proves nothing');
  for (const row of rows) {
    assert.equal(row.tag, 'button', 'a control a keyboard cannot reach is not a control');
    assert.ok(row.attrs['data-select-row'], 'the control does not name the row it opens');
    assert.equal(row.attrs['aria-pressed'], 'false');
    const count = findByAttr(row, 'data-role', 'field-count')[0];
    assert.ok(count, 'the control states no count -- a silent affordance');
    assert.match(textOf(count), /\d+ fields/);
    assert.ok(Number(row.attrs['data-fields']) > 0);
  }
});

test('choosing a row is a decision this window makes and never a request: the same read, a different subject', async () => {
  const state = await face.read(fullPort());
  const first = state.settled.items[0];
  const tree = face.view({ ...state, selected: first.id });
  const chosen = findByAttr(tree, 'data-part', 'selectable-row').find((r) => r.attrs['data-select-row'] === first.id);
  assert.equal(chosen.attrs['aria-pressed'], 'true');
  assert.equal(chosen.attrs['data-selected'], 'true');
  const pane = findByAttr(tree, 'data-part', 'detail-pane')[0];
  assert.equal(pane.attrs['data-subject'], first.id);
  assert.ok(Number(pane.attrs['data-count']) > 0, 'the pane names a subject and states nothing about it');
});

test('with nothing chosen the pane says so, and says how to fill it', async () => {
  const { tree } = await draw(fullPort());
  const pane = findByAttr(tree, 'data-part', 'detail-pane')[0];
  assert.equal(pane.attrs['data-subject'] ?? null, null);
  assert.equal(pane.attrs['data-count'], '0');
  assert.ok(findByAttr(pane, 'data-role', 'pane-empty')[0], 'an empty pane with no word in it is a blank box');
});

// -- retrofit round 2 (req/768 AC-4/AC-6/AC-7, SS657 continued) --

test('AC-4: a held row\'s acts sit in a fixed-width right gutter, never a full-width strip drawn underneath the row', async () => {
  const { tree } = await draw(fullPort());
  const strips = findByAttr(tree, 'data-role', 'acts');
  assert.equal(strips.length, 0, 'the old below-row strip this face drew before this round is gone');
  const gutters = findByAttr(tree, 'data-part', 'act-gutter');
  assert.ok(gutters.length > 0, 'at least one row-level gutter is drawn');
  const heldGutter = gutters.find((g) => g.attrs['data-target'] === 'c-004');
  assert.ok(heldGutter, 'the held row for c-004 carries its own gutter');
  assert.equal(heldGutter.attrs['data-count'], '3', 'commit, cancel, escalate -- every act this half declares');
  const frame = findByAttr(tree, 'data-part', 'row-gutter-frame').find((f) => find(f, (n) => n.attrs['data-target'] === 'c-004' && n.tag === 'button').length > 0);
  assert.ok(frame, 'the row and its gutter are drawn as siblings in one frame');
});

test('AC-4: a withheld act still draws a visibly-disabled slot with its reason on the button itself, never blank space', async () => {
  const { tree } = await draw(fullPort());
  const gutters = findByAttr(tree, 'data-part', 'act-gutter');
  const heldGutter = gutters.find((g) => g.attrs['data-target'] === 'c-004');
  const escalate = find(heldGutter, (n) => n.tag === 'button' && n.attrs['data-act'] === 'escalate')[0];
  assert.ok(escalate, 'escalate is drawn, not omitted');
  assert.equal(escalate.attrs.disabled, '');
  assert.match(escalate.attrs.title, /Declared, offered, and dimmed/);
});

test('AC-4: a settled row\'s single undo act is a one-button gutter beside that row', async () => {
  const { tree } = await draw(fullPort());
  const gutters = findByAttr(tree, 'data-part', 'act-gutter');
  const settledGutter = gutters.find((g) => g.attrs['data-target'] === 't-001');
  assert.ok(settledGutter, 'the settled row for t-001 carries its own gutter');
  assert.equal(settledGutter.attrs['data-count'], '1', 'only undo is offered on a settled row');
  const button = find(settledGutter, (n) => n.tag === 'button')[0];
  assert.equal(button.attrs['data-act'], 'undo');
  assert.equal(button.attrs.disabled, undefined, 'undo sends, so it is not disabled');
});

test('AC-7: a settled row a later row names as its predecessor reads its reversibility chip as reversed', async () => {
  const port = fullPort({
    get_transformations: page([
      SAMPLE.transformation(1),
      SAMPLE.transformation(2, { undo_of: 't-001', effect: 'undo' }),
    ]),
  });
  const { tree } = await draw(port);
  const chips = findByAttr(tree, 'data-part', 'reversal-chip');
  const reversedRowChip = chips.find((c) => c.attrs['data-state'] === 'reversed');
  assert.ok(reversedRowChip, 'a reversed-state chip is drawn');
  assert.match(reversedRowChip.attrs.title, /t-002/, 'the full reason names the reversing row, reachable on hover');
  assert.equal(find(reversedRowChip, (n) => n.tag === 'svg')[0].attrs['data-mark'], 'standing/reversed');
});

test('AC-7: a settled row with no reversing sibling in this same read is honestly unknown, never guessed as still-invertible', async () => {
  const { tree } = await draw(fullPort());
  const chips = findByAttr(tree, 'data-part', 'reversal-chip');
  const t003 = chips.find((c) => textOf(c).includes('unknown'));
  assert.ok(t003, 'a not-observable chip is drawn for a settled row nothing reverses');
  assert.match(t003.attrs.title, /membrane\/wire-fields\.json/, 'the honest reason names the declared backend hole, not a fabricated status');
});

test('AC-7: a held row\'s reversibility chip is always n/a -- nothing has happened yet, so there is nothing to invert', async () => {
  const { tree } = await draw(fullPort());
  // 'data-half' is also carried by parts/src/provenance-fold.mjs's own unrelated
  // settled/held legend rows -- 'data-role', 'row-block' scopes this to the rows
  // this face itself draws, the same disambiguation the render source needs.
  const held = findByAttr(tree, 'data-role', 'row-block').filter((n) => n.attrs['data-half'] === 'held');
  assert.ok(held.length > 0, 'at least one held row-block was drawn');
  for (const row of held) {
    const chip = findByAttr(row, 'data-part', 'reversal-chip')[0];
    assert.ok(chip, 'every held row carries a chip');
    assert.equal(chip.attrs['data-state'], 'not-committed');
    assert.ok(textOf(chip).includes('n/a'));
  }
});

test('AC-7: the legend explains the reversibility chip\'s three states once, not per row', async () => {
  const { tree } = await draw(fullPort());
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  assert.match(textOf(legend), /undo availability chip/);
  assert.match(textOf(legend), /reversed/);
  assert.match(textOf(legend), /unknown/);
  assert.match(textOf(legend), /n\/a/);
});

test('C-4 continued: standing/reversed and standing/none are declared marks, reachable and not fabricated', () => {
  const marks = new Set(DECLARATION.marks.map((m) => m.mark));
  assert.ok(marks.has('standing/reversed'));
  assert.ok(marks.has('standing/none'));
});

// -- closed by default (req/97 gap-list item gap 1) ---------------------------------------

test('every row of an ordinary read is drawn shut: an ISO timestamp in the time column is a declared cut, not a clip', async () => {
  const { tree } = await draw(fullPort());
  const blocks = findByAttr(tree, 'data-role', 'row-block');
  assert.ok(blocks.length > 0, 'no rows were drawn, so this proves nothing');
  const marked = blocks.filter((b) => b.attrs['data-open-because'] !== null && b.attrs['data-open-because'] !== undefined);
  assert.deepEqual(marked.map((b) => b.attrs['data-open-because']), [], 'a row was marked as one the grid cannot hold when it holds it fine');
  const rows = findByAttr(tree, 'data-part', 'selectable-row');
  assert.ok(rows.length > 0, 'the rows carry no control at all');
  for (const r of rows) assert.equal(r.attrs['data-selected'], 'false', 'a row made itself the subject of the pane');
});

test('negative control: a genuinely over-budget value still opens its own row, and only that row', async () => {
  const long = '/work/notes/very/long/path/that/has/to/be/clipped/rather/than/wrapped.md';
  const port = fullPort({
    get_transformations: page([SAMPLE.transformation(1), SAMPLE.transformation(2, { path: long })]),
  });
  const { tree } = await draw(port);
  const settled = sectionOf(tree, 'settled');
  const blocks = findByAttr(settled, 'data-role', 'row-block');
  assert.equal(blocks.length, 2);
  const reasons = blocks.map((b) => b.attrs['data-open-because'] ?? null);
  assert.deepEqual(reasons.filter(Boolean), ['clip-risk'], 'exactly one row had a reason to open');
});

// -- Owner #340: the band, the boxes, the standings, the footer ----------------------
//
// The three words this face is judged against are that it not be monotone, that it be
// understandable at a glance, and that it be usable. The first two are what these
// assertions are about: a figure a reader lands on before a word, four standings that
// do not share an ink, and two named groups with their own counts and their own edges.

const bandOf = (tree) => findByAttr(tree, 'data-part', 'stat-band')[0] ?? null;
const segmentsOf = (tree) => findByAttr(bandOf(tree), 'data-role', 'segment');
const figureOf = (segments, noun) => segments.find((s) => s.attrs['data-noun'] === noun);
const boxOf = (tree, name) => findByAttr(tree, 'data-box', name)[0] ?? null;

test('a band at the head states the size and the shape of this screen before a word is read', async () => {
  const { tree } = await draw(fullPort());
  const band = bandOf(tree);
  assert.ok(band, 'no band is drawn, so the screen opens with prose again');
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  assert.equal(tree.children[0], header);
  assert.equal(tree.children[1], band, 'the band does not sit immediately after the header line');
  const segments = segmentsOf(tree);
  assert.deepEqual(
    segments.map((s) => s.attrs['data-noun']),
    ['settled', 'admit', 'deny', 'escalate', 'held'],
  );
  // every figure is counted from the state this face read, and not one of them is typed
  // by hand: three settled rows, all Admit, and two held candidates.
  assert.equal(figureOf(segments, 'settled').attrs['data-value'], String(settledItems.length));
  assert.equal(figureOf(segments, 'admit').attrs['data-value'], '3');
  assert.equal(figureOf(segments, 'held').attrs['data-value'], String(heldItems.length));
});

test('a verdict this read holds none of still gets its figure, and the figure is a zero', async () => {
  const { tree } = await draw(fullPort());
  const segments = segmentsOf(tree);
  for (const noun of ['deny', 'escalate']) {
    const segment = figureOf(segments, noun);
    assert.ok(segment, `${noun} was dropped from the band because this read had none`);
    assert.equal(segment.attrs['data-value'], '0');
    assert.ok(textOf(segment).includes('0'), `${noun} states no figure at all`);
  }
});

test('a half that could not be read draws a dash in the band, never a zero', async () => {
  const { tree } = await draw(fullPort({ get_transformations: failed() }));
  const segments = segmentsOf(tree);
  for (const noun of ['settled', 'admit', 'deny', 'escalate']) {
    const segment = figureOf(segments, noun);
    assert.equal(segment.attrs['data-value'], 'unread', `${noun} claimed a count off a list that never arrived`);
    assert.ok(textOf(segment).includes(parts.statDash));
  }
  // the half that did answer still states its own figure: one unread list does not
  // blank the screen.
  assert.equal(figureOf(segments, 'held').attrs['data-value'], String(heldItems.length));
});

test('no two standings in the band are drawn in one ink, which is the whole of the complaint', async () => {
  const { tree } = await draw(fullPort());
  const segments = segmentsOf(tree);
  const inks = new Set();
  for (const noun of ['admit', 'deny', 'escalate', 'held']) {
    const segment = figureOf(segments, noun);
    const figure = findByAttr(segment, 'data-role', 'figure')[0];
    const ink = /color:([^;"]+)/.exec(figure.attrs.style ?? '');
    assert.ok(ink, `${noun} carries no ink of its own`);
    inks.add(ink[1]);
    const mark = find(segment, (n) => n.tag === 'svg' && 'data-mark' in n.attrs)[0];
    assert.ok(mark, `${noun} carries no mark`);
  }
  assert.equal(inks.size, 4, `two standings share an ink: ${[...inks].join(', ')}`);
});

test('the band is counted by the legend, so no mark is on screen without a live count behind it', async () => {
  const { tree } = await draw(fullPort());
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  const counted = (mark) => Number(findByAttr(legend, 'data-mark-entry', mark)[0].attrs['data-count']);
  // this read holds no denied row anywhere, so the only Deny mark drawn is the band's.
  assert.equal(figureOf(segmentsOf(tree), 'deny').attrs['data-value'], '0');
  assert.ok(counted('verdict/Deny') > 0, 'the band drew a mark the legend does not know about');
});

test('each half is a box with its own name, its own count and its own edge', async () => {
  const { tree } = await draw(fullPort());
  const boxes = findByAttr(tree, 'data-part', 'box');
  assert.deepEqual(boxes.map((b) => b.attrs['data-box']), ['settled', 'held']);
  assert.equal(boxOf(tree, 'settled').attrs['data-count'], String(settledItems.length));
  assert.equal(boxOf(tree, 'held').attrs['data-count'], String(heldItems.length));
  // the two halves are not counted in one word: a candidate is not a record.
  assert.ok(textOf(findByAttr(boxOf(tree, 'settled'), 'data-role', 'box-head')[0]).includes('3 records'));
  assert.ok(textOf(findByAttr(boxOf(tree, 'held'), 'data-role', 'box-head')[0]).includes('2 candidates'));
  // and the rows are still inside the box that names them.
  assert.equal(findByAttr(boxOf(tree, 'held'), 'data-part', 'receipt-row').length, heldItems.length);
});

test('what this window has sent is a group too, so it is drawn as one', async () => {
  const port = fullPort();
  const state = await face.read(port);
  const after = await face.act(port, state, { act: 'commit', id: 'c-004' });
  const tree = face.view(after);
  const acts = boxOf(tree, 'acts');
  assert.ok(acts, 'the act log is a bare heading with lines under it');
  assert.equal(acts.attrs['data-count'], '1');
  // one of a thing takes the singular: a head that said "1 entries" would be a screen
  // nobody read before shipping.
  assert.ok(textOf(findByAttr(acts, 'data-role', 'box-head')[0]).includes('1 entry'));
  assert.deepEqual(findByAttr(tree, 'data-part', 'box').map((b) => b.attrs['data-box']), ['settled', 'held', 'acts']);
});

test('the held box wears its standing and the settled box wears none: nothing here has been checked', async () => {
  const { tree } = await draw(fullPort());
  const heldHead = findByAttr(boxOf(tree, 'held'), 'data-role', 'box-head')[0];
  const pill = findByAttr(heldHead, 'data-part', 'standing-chip')[0];
  assert.ok(pill, 'the held box states no standing');
  assert.equal(pill.attrs['data-standing'], 'held');
  assert.equal(pill.attrs['data-filled'], 'true', 'a standing with an ink of its own is drawn on its own bed');
  const settledHead = findByAttr(boxOf(tree, 'settled'), 'data-role', 'box-head')[0];
  assert.deepEqual(findByAttr(settledHead, 'data-part', 'standing-chip'), [], 'the settled box was given a standing it has not got');
});

test('an empty half keeps its box and says nought; an unread one says neither nought nor a number', async () => {
  const empty = await draw(fullPort({ get_candidates: page([]) }));
  const emptyBox = boxOf(empty.tree, 'held');
  assert.ok(emptyBox, 'an empty half lost its box');
  assert.equal(emptyBox.attrs['data-count'], '0');
  assert.ok(textOf(emptyBox).includes(LEDGER_MESSAGES.EMPTY_HELD));

  const unread = await draw(fullPort({ get_candidates: failed() }));
  const unreadBox = boxOf(unread.tree, 'held');
  assert.equal(unreadBox.attrs['data-count'], parts.statDash);
  assert.ok(textOf(unreadBox).includes(LEDGER_MESSAGES.UNREAD));
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
// Everything below is about presses rather than about trees. The three defects the
// interaction audit found against a real browser are all in the handler: two of them
// are invisible to any test that reads a rendered tree, because the tree is correct
// both before and after -- what is wrong is what the handler did in between.

const actButton = (host, act, id) => nodesOf(host).find((n) => n.tag === 'button'
  && n.attrs?.['data-act'] === act && n.attrs?.['data-target'] === id) ?? null;

test('req/103 finding 1: two presses of one act button in the same tick record two acts, and lose neither', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  // Both presses land before the first has answered, which is what a fast double
  // click is: the second event is dispatched against the screen the first one has
  // not repainted yet.
  press(host, actButton(host, 'commit', 'c-004'));
  press(host, actButton(host, 'commit', 'c-004'));
  await unmount.quiet();
  assert.equal(port.calls.filter((c) => c.name === 'post_candidates_id_commit').length, 2, 'one press never reached the port');
  const log = nodeWith(host, 'data-role', 'act-log');
  assert.ok(log, 'nothing was written down at all');
  assert.equal(log.attrs['data-count'], '2', 'an act was sent and the record of it was overwritten by the other');
  unmount();
});

test('req/103 finding 1: two presses on two different rows in the same tick both survive', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  press(host, actButton(host, 'commit', 'c-004'));
  press(host, actButton(host, 'cancel', 'c-005'));
  await unmount.quiet();
  const log = nodeWith(host, 'data-role', 'act-log');
  assert.equal(log.attrs['data-count'], '2');
  assert.match(textOfHost(host), /commit c-004/);
  assert.match(textOfHost(host), /cancel c-005/);
  unmount();
});

test('req/103 finding 3: an act that has been sent says so on its own button until it is answered', async () => {
  const host = standInHost();
  let answer = null;
  const waiting = new Promise((resolve) => { answer = resolve; });
  const port = fullPort({ post_candidates_id_commit: () => waiting });
  const unmount = mount(host, port, []);
  await unmount.ready;
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

test('req/103 finding 2: a panel a reader opened is still open after they choose a row', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  const control = (name) => nodeWith(host, 'data-control', name);
  assert.equal(control('why').attrs['data-open'], 'false');
  press(host, control('why').childNodes[0]);
  assert.equal(control('why').attrs['data-open'], 'true', 'pressing the control did not open it');
  press(host, nodeWith(host, 'data-select-row', 't-001'));
  assert.equal(control('why').attrs['data-open'], 'true', 'choosing a row shut a panel the reader had opened');
  assert.equal(control('legend').attrs['data-open'], 'false', 'opening one control opened another');
  press(host, control('why').childNodes[0]);
  assert.equal(control('why').attrs['data-open'], 'false', 'the control could be opened and never shut');
  unmount();
});

test('req/103 finding 2: an act does not shut what a reader opened, and does not forget which row they were reading', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  press(host, nodeWith(host, 'data-control', 'legend').childNodes[0]);
  press(host, nodeWith(host, 'data-select-row', 't-001'));
  press(host, actButton(host, 'commit', 'c-004'));
  await unmount.quiet();
  assert.equal(nodeWith(host, 'data-control', 'legend').attrs['data-open'], 'true', 'an act shut the panel the reader was reading');
  const pane = nodeWith(host, 'data-part', 'detail-pane');
  assert.equal(pane.attrs['data-subject'], 't-001', 'an act threw away the row the reader had open');
  unmount();
});

test('a caller that states where its state came from is taken at its word', async () => {
  const state = await face.read(fullPort());
  const said = 'a stand-in, not an engine';
  const tree = face.view({ ...state, source: said });
  const read = findByAttr(tree, 'data-name', 'read')[0];
  assert.ok(textOf(read).includes(said), 'the stated source was overwritten by a word the face chose');
});

// -- the other button (Owner #348 (2)) -----------------------------------------------
//
// A second control surface on a row is a chance to say something different from the
// first one, and everything below is about that not happening. The menu is drawn from
// the same declaration the gutter is, an act pressed on it goes into the same queue an
// act pressed in the gutter goes into, and neither of those is a convention somebody
// has to keep -- they are one function and one code path, and these are what say so.

const menuOf = (host) => nodeWith(host, 'data-role', 'row-menu');
const menusOf = (host) => nodesOf(host).filter((n) => n.attrs?.['data-role'] === 'row-menu');
const gutterActs = (host, id) => nodesOf(host).filter((n) => n.tag === 'button'
  && n.attrs?.['data-act'] && n.attrs?.['data-target'] === id
  && n.parentNode?.attrs?.['data-role'] !== 'row-menu');
const menuActs = (host) => nodesOf(menuOf(host) ?? { childNodes: [] })
  .filter((n) => n.attrs?.['data-menu-item'] === 'act');
const menuCopy = (host) => nodesOf(menuOf(host) ?? { childNodes: [] })
  .find((n) => n.attrs?.['data-menu-item'] === 'copy') ?? null;
const cellIn = (host, id, key) => nodesOf(host)
  .find((n) => n.attrs?.['data-cell'] === key && n.closest('[data-row]')?.attrs?.['data-row'] === id) ?? null;
const availability = (nodes) => nodes.map((n) => `${n.attrs['data-act']}:${n.attrs.disabled === '' ? 'off' : 'on'}`);

test('a right-click on a row offers exactly the acts that row gutter offers, in the same order and with the same availability', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  const { defaulted } = rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  assert.equal(defaulted, false, 'the browser own menu was left underneath this one');
  const menu = menuOf(host);
  assert.ok(menu, 'no menu was opened');
  assert.equal(menu.attrs['data-menu-row'], 'c-004');
  const gutter = gutterActs(host, 'c-004');
  assert.ok(gutter.length > 0, 'this row has no gutter acts, so this proves nothing');
  assert.deepEqual(
    availability(menuActs(host)),
    availability(gutter),
    'the menu and the gutter disagree about what this row offers',
  );
  unmount();
});

test('a right-click on an act reaches the same row as a right-click on the line', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightPress(host, actButton(host, 'cancel', 'c-005'));
  assert.equal(menuOf(host).attrs['data-menu-row'], 'c-005');
  unmount();
});

test('a withheld act is in the menu, disabled, carrying the same reason it carries in the gutter', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  const escalate = menuActs(host).find((n) => n.attrs['data-act'] === 'escalate');
  assert.ok(escalate, 'a withheld act was dropped from the menu, which reads as never offered');
  assert.equal(escalate.attrs.disabled, '');
  const declared = DECLARATION.acts.find((a) => a.act === 'escalate');
  assert.equal(escalate.attrs.title, declared.why, 'the menu invented its own reason');
  unmount();
});

test('an act in flight is dimmed in both surfaces at once, because there is one answer to what is offered', async () => {
  const host = standInHost();
  let answer = null;
  const waiting = new Promise((resolve) => { answer = resolve; });
  const unmount = mount(host, fullPort({ post_candidates_id_commit: () => waiting }), []);
  await unmount.ready;
  press(host, actButton(host, 'commit', 'c-004'));
  await null;
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  const commit = menuActs(host).find((n) => n.attrs['data-act'] === 'commit');
  assert.equal(commit.attrs.disabled, '', 'the menu offered an act the gutter had already dimmed');
  assert.equal(commit.attrs.title, LEDGER_MESSAGES.IN_FLIGHT);
  answer({ outcome: 'answered', status: 202, body: {} });
  await unmount.quiet();
  unmount();
});

test('a menu act and a gutter act pressed in the same tick are two acts and two entries: the menu is on the one queue', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  const fromMenu = menuActs(host).find((n) => n.attrs['data-act'] === 'commit');
  assert.ok(fromMenu, 'no commit item in the menu');
  // Both land before either has answered, which is the shape req/103 finding 1
  // measured losing one of two acts on.
  press(host, fromMenu);
  press(host, actButton(host, 'cancel', 'c-005'));
  await unmount.quiet();
  assert.equal(port.calls.filter((c) => c.name === 'post_candidates_id_commit').length, 1);
  assert.equal(port.calls.filter((c) => c.name === 'post_candidates_id_cancel').length, 1);
  const log = nodeWith(host, 'data-role', 'act-log');
  assert.equal(log.attrs['data-count'], '2', 'one of the two acts left no record of itself');
  unmount();
});

test('a second right-click replaces the menu rather than stacking one on top of another', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  assert.equal(menusOf(host).length, 1);
  rightPress(host, nodeWith(host, 'data-select-row', 'c-005'));
  assert.equal(menusOf(host).length, 1, 'two menus are open at once');
  assert.equal(menuOf(host).attrs['data-menu-row'], 'c-005', 'the second press opened a menu on the wrong row');
  rightPress(host, nodeWith(host, 'data-select-row', 'c-005'));
  assert.equal(menusOf(host).length, 0, 'the same row twice did not shut it');
  unmount();
});

test('a repaint cannot leave a menu behind: the menu is state, and an act repaints from state', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  press(host, menuActs(host).find((n) => n.attrs['data-act'] === 'commit'));
  await unmount.quiet();
  assert.equal(menusOf(host).length, 0, 'the menu survived the act it fired');
  // a fresh read of the server does not throw away one a reader still has open either.
  const state = await face.read(fullPort(), { menu: { id: 'c-005', cell: null } });
  assert.deepEqual(state.menu, { id: 'c-005', cell: null }, 'a fresh read threw away a window decision');
  unmount();
});

test('Escape dismisses the menu, from a key struck anywhere in the document', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  assert.equal(menusOf(host).length, 1);
  strike(host.ownerDocument, 'a');
  assert.equal(menusOf(host).length, 1, 'any key at all dismissed it');
  strike(host.ownerDocument, 'Escape');
  assert.equal(menusOf(host).length, 0, 'Escape did nothing');
  unmount();
});

test('a press away dismisses the menu, inside this face and outside it', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  // inside the face: choosing another row shuts the menu AND chooses that row, so the
  // press is not spent on the dismissal.
  press(host, nodeWith(host, 'data-select-row', 't-001'));
  assert.equal(menusOf(host).length, 0);
  assert.equal(nodeWith(host, 'data-part', 'detail-pane').attrs['data-subject'], 't-001');
  // outside the face: a press on the shell around it.
  rightPress(host, nodeWith(host, 'data-select-row', 'c-004'));
  assert.equal(menusOf(host).length, 1);
  pressAway(host.ownerDocument, { tag: 'div', attrs: {}, parentNode: null });
  assert.equal(menusOf(host).length, 0, 'a press outside this face left its menu open');
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
    rightPress(host, cellIn(host, 't-001', 'path'));
    const copy = menuCopy(host);
    press(host, actButton(host, 'commit', 'c-004'));
    press(host, copy);
    await unmount.quiet();
    // the act is written down and the copy happened: neither write to state was lost
    // under the other.
    assert.equal(nodeWith(host, 'data-role', 'act-log').attrs['data-count'], '1');
    assert.deepEqual(written, [settledItems[0].path]);
    assert.equal(nodeWith(host, 'data-role', 'copy-report').getAttribute('data-copied'), 'path');
    unmount();
  } finally {
    Object.defineProperty(globalThis, 'navigator', held);
  }
});
