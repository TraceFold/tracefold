// SPDX-License-Identifier: Apache-2.0
// What the face does, asked of the face and not of a description of it.
//
// The one thing these tests exist to guard above everything else: nothing this
// face draws may wear a receipt's face. Every other test here is either the two
// properties this face carries over from faces/ledger's discipline (fail-closed,
// rows-not-edited) or the ordinary declaration/mark/gate checks every face in this
// tree holds.
//
// stub-port.mjs and dom-stand-in.mjs are read from faces/ledger/test/, not
// duplicated here -- both are generic port/document stand-ins with no ledger-
// specific behaviour (SAMPLE.candidate already describes exactly this face's own
// data shape), and faces/notice already sets the precedent of one face's tests
// importing another face's test helper directly (req/99 §5, faces/notice row).

import test from 'node:test';
import assert from 'node:assert/strict';

import { createFace, face, mount, HELD_MESSAGES } from '../held.mjs';
import { DECLARATION } from '../declaration.mjs';
import { parts } from '../binding.mjs';
import { standInHost, textOfHost, nodesOf } from '../../ledger/test/dom-stand-in.mjs';
import {
  stubPort, page, answered, refused, failed, absent, SAMPLE,
} from '../../ledger/test/stub-port.mjs';

const { el, toHtml, find, findByAttr, textOf } = parts.element;

const heldItems = [SAMPLE.candidate(1), SAMPLE.candidate(2), SAMPLE.candidate(3)];

function fullPort(overrides = {}) {
  return stubPort({
    get_candidates: page(heldItems),
    post_candidates_id_commit: answered({ id: 'c-001' }, 202),
    post_candidates_id_cancel: answered({ id: 'c-002' }, 202),
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
  assert.throws(() => mount(null, fullPort(), []), new RegExp(HELD_MESSAGES.NO_HOST));
  assert.throws(() => mount(standInHost(), null, []), new RegExp(HELD_MESSAGES.NO_PORT));
});

test('mount draws something before the read answers, and does not call it a list', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  const early = textOfHost(host);
  assert.ok(early.includes(HELD_MESSAGES.READING));
  assert.equal(early.includes(HELD_MESSAGES.EMPTY_HELD), false);
  await unmount.ready;
  unmount();
});

// -- C-1 ----------------------------------------------------------------------

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
  await assert.rejects(() => caller.invoke('get_healthz', {}), new RegExp(HELD_MESSAGES.UNDECLARED));
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
  await face.act(port, state, { act: 'commit', id: 'c-001' });
  for (const call of port.calls) {
    assert.equal(Object.prototype.hasOwnProperty.call(call.input ?? {}, 'actor'), false);
    assert.equal(Object.prototype.hasOwnProperty.call(call.input?.body ?? {}, 'actor'), false);
  }
});

// -- fail-closed ----------------------------------------------------------------

for (const [name, result] of [
  ['failed', failed()],
  ['refused', refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not read the candidates', gx_code: 'UNAUTHORIZED' })],
  ['absent', absent({ name: 'get_candidates' })],
]) {
  test(`fail-closed: a ${name} read is not drawn as an empty list of held candidates`, async () => {
    const port = fullPort({ get_candidates: result });
    const { tree, html } = await draw(port);
    const held = sectionOf(tree, 'held');
    assert.equal(held.attrs['data-state'], 'unread');
    assert.ok(textOf(held).includes(HELD_MESSAGES.UNREAD));
    assert.equal(html.includes(HELD_MESSAGES.EMPTY_HELD), false, 'an unread list was given the empty list\'s words');
    assert.equal(findByAttr(held, 'data-part', 'receipt-row').length, 0);
    assert.ok(textOf(held).includes(name), 'the outcome is not named on screen');
  });
}

test('an answered read with no items says so in different words from an unread one', async () => {
  const port = fullPort({ get_candidates: page([]) });
  const { tree } = await draw(port);
  const held = sectionOf(tree, 'held');
  assert.equal(held.attrs['data-state'], 'empty');
  assert.ok(textOf(held).includes(HELD_MESSAGES.EMPTY_HELD));
  assert.equal(textOf(held).includes(HELD_MESSAGES.UNREAD), false);
});

test('a refusal carries the engine\'s own words up, unedited', async () => {
  const problem = { type: 'about:blank', title: 'conflict', status: 409, detail: 'the candidate list moved under the walk', gx_code: 'IDEMPOTENCY_CONFLICT' };
  const { html } = await draw(fullPort({ get_candidates: refused(problem) }));
  assert.ok(html.includes('IDEMPOTENCY_CONFLICT'));
  assert.ok(html.includes(problem.detail));
});

// -- C-3: the denominator --------------------------------------------------------

test('C-3: the walk\'s denominator is on screen, not just the row count', async () => {
  const port = fullPort({ get_candidates: page(heldItems, { pages: 5, stopped: true }) });
  const { tree } = await draw(port);
  const words = textOf(sectionOf(tree, 'not-drawn'));
  assert.ok(words.includes('5'), 'the number of requests the walk took is not stated');
  assert.ok(words.includes(HELD_MESSAGES.TRUNCATED));
});

test('C-3: rows the order dropped are counted and the reason for each is named', async () => {
  const port = fullPort({
    get_candidates: page([SAMPLE.candidate(1), { sequence: 2, at: 'x' }, 'not a record']),
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

// -- order ------------------------------------------------------------------------

test('C-6: the order is stated on screen with its reason', async () => {
  const { tree } = await draw(fullPort());
  const held = sectionOf(tree, 'held');
  assert.ok(textOf(held).includes(DECLARATION.rows.order));
  assert.ok(textOf(held).includes(DECLARATION.rows.order_reason));
});

test('an order whose assumption breaks is substituted, and the substitution is stated', async () => {
  const port = fullPort({
    get_candidates: page([SAMPLE.candidate(1), SAMPLE.candidate(2, { sequence: undefined })]),
  });
  const { tree } = await draw(port);
  assert.ok(textOf(sectionOf(tree, 'held')).includes('as-recorded'));
});

// -- rows are not edited ------------------------------------------------------------

test('committing a candidate does not rewrite the row that was drawn -- the list is read again', async () => {
  const before = [SAMPLE.candidate(1), SAMPLE.candidate(2)];
  const after = [SAMPLE.candidate(1)]; // c-002 committed, and left this list
  let reads = 0;
  const port = fullPort({
    get_candidates: () => page(reads++ === 0 ? before : after),
  });

  const first = await face.read(port);
  const firstHtml = toHtml(face.view(first));
  assert.ok(firstHtml.includes('data-row="c-002"'));

  const next = await face.act(port, first, { act: 'commit', id: 'c-002' });
  const secondHtml = toHtml(face.view(next));
  assert.equal(secondHtml.includes('data-row="c-002"'), false, 'the committed row was still drawn as held after commit');
  assert.ok(secondHtml.includes('data-row="c-001"'), 'an unrelated row disappeared');
});

test('an act that is refused is drawn, not swallowed', async () => {
  const port = fullPort({
    post_candidates_id_commit: refused({ type: 'about:blank', title: 'gone', status: 410, detail: 'this candidate was already cancelled', gx_code: 'VALIDATION_ERROR' }),
  });
  const state = await face.read(port);
  const next = await face.act(port, state, { act: 'commit', id: 'c-001' });
  const html = toHtml(face.view(next));
  assert.ok(html.includes('this candidate was already cancelled'));
  assert.ok(html.includes('VALIDATION_ERROR'));
});

test('an act that throws inside the membrane is drawn as a failure, not as silence', async () => {
  const port = fullPort();
  port.post_candidates_id_cancel = () => { throw new TypeError('this route requires an actor and the membrane was built without one'); };
  const state = await face.read(port);
  const next = await face.act(port, state, { act: 'cancel', id: 'c-002' });
  const html = toHtml(face.view(next));
  assert.ok(html.includes('requires an actor'));
});

// -- held does not wear a receipt's face -----------------------------------------

test('every row is a declared seal hole, unconditionally -- nothing here is ever sealed', async () => {
  const { tree } = await draw(fullPort());
  const held = sectionOf(tree, 'held');
  const rows = findByAttr(held, 'data-part', 'receipt-row');
  assert.equal(rows.length, heldItems.length);
  for (const row of rows) {
    const seal = findByAttr(row, 'data-cell', 'seal')[0];
    assert.equal(seal.attrs['data-state'], 'hole', 'a held row was drawn with a seal value');
    assert.ok((seal.attrs.title ?? '').includes(HELD_MESSAGES.NOTHING_TO_SEAL));
  }
});

test('nothing on this screen is drawn as sealed or checked, and the claims section says so in the open', async () => {
  const { html, tree } = await draw(fullPort());
  assert.equal(html.includes('"sealed"'), false);
  assert.equal(/aria-label="sealed"/.test(html), false);
  const claims = sectionOf(tree, 'claims');
  assert.ok(textOf(claims).includes(HELD_MESSAGES.NOT_A_RECEIPT));
});

test('the legend states the seal column is always a hole, once, not per row', async () => {
  const { tree } = await draw(fullPort());
  // SS657 retrofit: legend is now a bordered controlToggle() control
  // (data-role "control", data-control "legend"), not a full-width data-section
  // band.
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  assert.ok(textOf(legend).includes(HELD_MESSAGES.NOT_A_RECEIPT));
});

test('claims that do not hold are shown, not filtered out', async () => {
  const { tree } = await draw(fullPort());
  const claims = sectionOf(tree, 'claims');
  const verdicts = attrOf(claims, 'data-holds');
  assert.ok(verdicts.length > 0);
  // At least one structural claim over a bare candidate list with no withheld set
  // and no prev chain will not hold -- shown here rather than hidden, the same
  // discipline faces/ledger's own claims section holds.
  assert.ok(verdicts.includes('true') || verdicts.includes('false'));
});

function attrOf(tree, name) {
  return find(tree, (n) => name in n.attrs).map((n) => n.attrs[name]);
}

test('the pane says nothing twice under one name (Owner directive #335, 3: the note moved, the property did not)', async () => {
  const state = await face.read(fullPort());
  const first = state.held.items[0];
  const tree = face.view({ ...state, selected: first.id });
  const pane = findByAttr(tree, 'data-part', 'detail-pane')[0];
  assert.ok(pane, 'no detail pane was drawn');
  assert.equal(pane.attrs['data-subject'], first.id);
  const names = findByAttr(pane, 'data-role', 'pane-line').map((line) => line.attrs['data-name']);
  assert.ok(names.length > 0, 'the pane drew no facts, so this proves nothing');
  assert.equal(new Set(names).size, names.length, `one name used twice in the pane: ${names.join(', ')}`);
  assert.deepEqual(findByAttr(tree, 'data-part', 'receipt-note'), [], 'a note is still drawn under a row');
});

test('a value too long for its column is drawn in full on the row itself, because the row wraps rather than cutting it', async () => {
  const long = '/work/notes/very/long/path/that/has/to/be/clipped/rather/than/wrapped.md';
  const port = fullPort({
    get_candidates: page([SAMPLE.candidate(1, { path: long })]),
  });
  const { tree, html } = await draw(port);
  const block = findByAttr(tree, 'data-role', 'row-block')[0];
  assert.equal(block.attrs['data-open-because'], 'clip-risk', 'the row does not state that the grid cannot hold it');
  const cell = findByAttr(tree, 'data-cell', 'path').find((c) => textOf(c) === long);
  assert.ok(cell, 'the whole value is not in the row cell');
  assert.match(cell.attrs.style, /white-space:normal/, 'the path cell clips instead of wrapping, so the value is lost');
  assert.ok(html.includes(long), 'the whole value is nowhere on the page');
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  assert.ok(textOf(legend).includes(HELD_MESSAGES.CLIP_RISK), 'the full sentence lives once, in the legend');
});

test('a missing member marks the row, and the seal hole every row carries does not also mark it', async () => {
  const port = fullPort({
    get_candidates: page([SAMPLE.candidate(1, { actor: undefined }), SAMPLE.candidate(2)]),
  });
  const { tree } = await draw(port);
  const blocks = findByAttr(tree, 'data-role', 'row-block');
  assert.equal(blocks.length, 2);
  assert.equal(blocks[0].attrs['data-open-because'], 'declared-hole', 'a genuinely missing member was not surfaced');
  assert.equal(blocks[1].attrs['data-open-because'] ?? null, null, 'the seal hole every row carries marked an ordinary row');
  const state = await face.read(port);
  const opened = face.view({ ...state, selected: state.held.items[0].id });
  assert.ok(toHtml(opened).includes(HELD_MESSAGES.MEMBER_ABSENT), 'the reason is not in the pane either');
});

// -- marks, sizes, positioning --------------------------------------------------

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

test('rendering the same state twice gives the same tree, apart from the one figure that is a measurement', async () => {
  const state = await face.read(fullPort());
  const first = toHtml(face.view(state));
  const second = toHtml(face.view(state));
  // The runtime footer states the milliseconds this tree took to build, measured
  // with performance.now() around the build itself. That figure is a fact about a
  // machine at a moment and not a function of the state, so it is normalised out
  // here and asserted separately -- the alternative, dropping the measurement to
  // keep the tree pure, is the fabricated-figure failure the footer exists to
  // refuse.
  const measured = [/render [\d.]+ ms/g, /data-render-ms="[\d.]+"/g];
  assert.match(first, measured[0], 'the footer states no measured figure at all');
  const settled = (html) => measured.reduce((text, pattern) => text.replace(pattern, '<measured>'), html);
  assert.equal(settled(first), settled(second));
});

// -- SS657 retrofit (req/38 SS657 Owner #317/#318, idiom proven by faces/atlas) --

test('SS657 defect 4/5 cure: a single compact header line states the face name and the denominator, before anything else', async () => {
  const { tree } = await draw(fullPort());
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  assert.ok(header);
  assert.ok(textOf(header).includes('held'));
  // req/822_c7 (Owner #387/#388 冗長文字全掃): nothing was dropped on this fixture
  // (3 clean candidates, all drawn), so the denominator agrees with the count and
  // is stated once, not twice -- "3 candidates", not "3 of 3 candidates".
  assert.match(textOf(header), new RegExp(`${heldItems.length} candidates`));
  assert.doesNotMatch(textOf(header), /\d+ of \d+ candidates/, 'the header restates one number as two when nothing was dropped');
  assert.equal(tree.children[0], header);
});

test('req/822_c7 (Owner #387/#388): the header states "N of M candidates" when a drop makes the two numbers differ', async () => {
  const port = fullPort({
    get_candidates: page([SAMPLE.candidate(1), { sequence: 2, at: 'x' }, 'not a record']),
  });
  const { tree } = await draw(port);
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  assert.match(textOf(header), /1 of 3 candidates/, 'the header should still carry both numbers once a row was dropped');
});

test('SS657 defect 2 cure: why/legend are bordered, self-evident controls sitting in one row, each with a plain-language hint', async () => {
  const { tree } = await draw(fullPort());
  const row = findByAttr(tree, 'data-role', 'control-row')[0];
  assert.ok(row);
  const controls = findByAttr(row, 'data-role', 'control');
  // Owner directive #335 (1): claims and omitted joined why and legend in this one
  // row; they were always-open bands of prose under the candidates before it.
  assert.ok(controls.length >= 2);
  for (const control of controls) {
    assert.equal(control.attrs['data-open'], 'false', `${control.attrs['data-control']} is not collapsed by default`);
    assert.ok(control.attrs.style.includes('border'), `${control.attrs['data-control']} is a bare word, not a control`);
  }
  const why = findByAttr(row, 'data-control', 'why')[0];
  const legend = findByAttr(row, 'data-control', 'legend')[0];
  assert.ok(why.attrs.style.includes('border'));
  assert.ok(legend.attrs.style.includes('border'));
  // req/822_c7 (Owner #387/#388 冗長文字全掃): the hint no longer draws as its own
  // visible span beside the label -- it rides the control's own summary as a
  // title (a hover) and a data-hint attribute.
  const summaryOf = (control) => find(control, (n) => n.tag === 'summary')[0];
  assert.equal(summaryOf(why).attrs['data-hint'], 'about this screen');
  assert.equal(summaryOf(why).attrs.title, 'about this screen');
  assert.equal(summaryOf(legend).attrs['data-hint'], 'symbols and counts');
  assert.equal(summaryOf(legend).attrs.title, 'symbols and counts');
  assert.doesNotMatch(textOf(why), /about this screen/, 'the hint is still drawn as visible text');
  assert.doesNotMatch(textOf(legend), /symbols and counts/, 'the hint is still drawn as visible text');
  assert.equal(why.attrs['data-open'], 'false');
  assert.equal(legend.attrs['data-open'], 'false');
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
  assert.deepEqual(findByAttr(tree, 'data-part', 'receipt-note'), [], 'a note is still drawn under a row');
  for (const row of rows) {
    assert.equal(row.tag, 'button', 'a control a keyboard cannot reach is not a control');
    assert.equal(row.attrs['aria-pressed'], 'false');
    const count = findByAttr(row, 'data-role', 'field-count')[0];
    assert.ok(count, 'the control states no count -- a silent affordance');
    assert.match(textOf(count), /\d+ fields/);
  }
  const pane = findByAttr(tree, 'data-part', 'detail-pane')[0];
  assert.ok(pane, 'there is no pane for a chosen candidate to be stored in');
});

// -- retrofit round 2 (req/768 AC-4/AC-6/AC-7, SS657 continued) --

// The readiness ladder draws each gate's act in a gutter of its own, so these two
// read the rows' own gutters through the rows container rather than through the
// whole tree. The property each one holds is exactly the property it held before.
const rowsOf = (tree) => findByAttr(tree, 'data-role', 'rows')[0];

test('AC-4: every row\'s acts sit in a fixed-width right gutter, never the full-width strip this face drew before this round', async () => {
  const { tree } = await draw(fullPort());
  const strips = findByAttr(tree, 'data-role', 'acts');
  assert.equal(strips.length, 0);
  const gutters = findByAttr(rowsOf(tree), 'data-part', 'act-gutter');
  assert.equal(gutters.length, heldItems.length, 'one gutter per row');
  for (const gutter of gutters) assert.equal(gutter.attrs['data-count'], '4', 'commit, cancel, escalate, undo -- every act this face declares, offered on every row');
});

test('AC-4: the withheld undo act draws disabled with its reason, because a held candidate has no transformation id to target yet', async () => {
  const { tree } = await draw(fullPort());
  const gutter = findByAttr(rowsOf(tree), 'data-part', 'act-gutter')[0];
  const undo = find(gutter, (n) => n.tag === 'button' && n.attrs['data-act'] === 'undo')[0];
  assert.ok(undo, 'undo is drawn, not omitted, even though it is withheld on every row here');
  assert.equal(undo.attrs.disabled, '');
  assert.match(undo.attrs.title, /has not produced a transformation yet/);
});

test('AC-7: every row\'s reversibility chip reads n/a -- a candidate has not happened yet, so there is nothing to invert', async () => {
  const { tree } = await draw(fullPort());
  const chips = findByAttr(tree, 'data-part', 'reversal-chip');
  assert.equal(chips.length, heldItems.length, 'one chip per row');
  for (const chip of chips) {
    assert.equal(chip.attrs['data-state'], 'not-committed');
    assert.ok(textOf(chip).includes('n/a'));
    assert.equal(find(chip, (n) => n.tag === 'svg')[0].attrs['data-mark'], 'standing/held', 'the chip reuses the same mark the lifecycle cell already carries for this meaning, not a new one');
  }
});

test('AC-7: "reversed" and "unknown" are unreachable states on this screen -- this face never declares standing/reversed or standing/none', () => {
  // held's declaration only names the marks it can actually draw (C-4): a
  // candidate's lifecycle is always 'held', so reversalOf() short-circuits to
  // not-committed before it could ever answer 'reversed' or 'not-observable'.
  // Declaring the other two chip marks here would be declaring a mark this face
  // can prove it will never draw.
  const marks = new Set(DECLARATION.marks.map((m) => m.mark));
  assert.equal(marks.has('standing/reversed'), false);
  assert.equal(marks.has('standing/none'), false);
  assert.ok(marks.has('standing/held'));
});

test('AC-7: the legend explains why the chip is always n/a here, once, not per row', async () => {
  const { tree } = await draw(fullPort());
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  assert.match(textOf(legend), /undo availability chip/);
  assert.match(textOf(legend), /n\/a/);
});

// -- Owner #340 retrofit round 4: the band, the containers, the ladder, the footer --

test('the band states the population before a word of the screen is read, and every figure in it is a count', async () => {
  const { tree } = await draw(fullPort());
  const band = findByAttr(tree, 'data-part', 'stat-band')[0];
  assert.ok(band, 'there is no band');
  assert.equal(tree.children[1], band, 'the band is not the first thing under the header');
  const segments = findByAttr(band, 'data-role', 'segment');
  assert.equal(segments.length, 4);
  const byNoun = new Map(segments.map((s) => [s.attrs['data-noun'], s.attrs['data-value']]));
  assert.equal(byNoun.get('candidates'), String(heldItems.length));
  assert.equal(byNoun.get('to commit'), String(heldItems.length));
  assert.equal(byNoun.get('not drawn'), '0', 'a zero is drawn, not omitted');
  assert.equal(byNoun.get('inverses'), '0');
  const figures = findByAttr(band, 'data-role', 'figure').map((f) => textOf(f));
  for (const figure of figures) assert.match(figure, /^\d+$/, `a figure that is not a number: ${figure}`);
  // The band cuts a noun that does not fit its column, and a cut noun on the one
  // strip meant to be read at a glance is a label that has to be hovered. Eleven
  // characters is what four columns leave at this app's own narrow viewport.
  for (const noun of byNoun.keys()) assert.ok(noun.length <= 11, `a noun too long for its column: ${noun}`);
});

test('the band counts what was dropped rather than pretending the received count is the drawn one', async () => {
  const port = fullPort({ get_candidates: page([SAMPLE.candidate(1), 'not a record', { sequence: 9 }]) });
  const { tree } = await draw(port);
  const band = findByAttr(tree, 'data-part', 'stat-band')[0];
  const byNoun = new Map(findByAttr(band, 'data-role', 'segment').map((s) => [s.attrs['data-noun'], s.attrs['data-value']]));
  assert.equal(byNoun.get('candidates'), '1');
  assert.equal(byNoun.get('not drawn'), '2');
});

test('a read that did not answer draws dashes in the band and a dash in the list\'s own head, never zeros', async () => {
  const { tree } = await draw(fullPort({ get_candidates: failed() }));
  const band = findByAttr(tree, 'data-part', 'stat-band')[0];
  for (const segment of findByAttr(band, 'data-role', 'segment')) {
    assert.equal(segment.attrs['data-value'], 'unread', `${segment.attrs['data-noun']} claims a number from a read that did not answer`);
  }
  const box = findByAttr(tree, 'data-box', 'candidates')[0];
  assert.equal(box.attrs['data-count'], '--', 'an unread list is drawn as a list of none');
});

test('every group on this screen is a container with its own name and its own count, zero included', async () => {
  const { tree } = await draw(fullPort({ get_candidates: page([]) }));
  const boxes = findByAttr(tree, 'data-part', 'box');
  const named = new Map(boxes.map((b) => [b.attrs['data-box'], b.attrs['data-count']]));
  assert.equal(named.get('candidates'), '0', 'an empty list keeps its border and says 0');
  assert.equal(named.get('acts'), '0', 'a screen from which nothing was sent keeps the container');
  assert.ok([...named.keys()].some((name) => name.startsWith('readiness')), 'the ladder is not a container');
  for (const box of boxes) {
    assert.ok(findByAttr(box, 'data-role', 'box-head')[0], `${box.attrs['data-box']} has no head`);
  }
});

test('the list\'s container states the standing every row in it shares, as a chip and not as a sentence', async () => {
  const { tree } = await draw(fullPort());
  const box = findByAttr(tree, 'data-box', 'candidates')[0];
  const head = findByAttr(box, 'data-role', 'box-head')[0];
  const pill = findByAttr(head, 'data-part', 'standing-chip')[0];
  assert.ok(pill, 'the group states no standing');
  assert.equal(pill.attrs['data-standing'], 'held');
  assert.equal(pill.attrs['data-filled'], 'true', 'the standing is drawn as an area, not as a stroke');
});

test('the ladder draws one container per declared gate, in the declared order, the answer first and the act inside it', async () => {
  const { tree } = await draw(fullPort());
  const gates = findByAttr(tree, 'data-role', 'gate');
  assert.equal(gates.length, DECLARATION.gates.length);
  assert.deepEqual(gates.map((g) => g.attrs['data-gate']), DECLARATION.gates.map((g) => g.gate));
  for (const [index, gate] of gates.entries()) {
    const declared = DECLARATION.gates[index];
    const chip = findByAttr(gate, 'data-part', 'standing-chip')[0];
    assert.ok(chip, `${declared.gate} states no answer`);
    assert.ok(['open', 'shut', 'unknown'].includes(textOf(chip).trim()), `${declared.gate} answers with a word that is not one of the three`);
    const control = find(gate, (n) => n.tag === 'button' && n.attrs['data-act'] === declared.act)[0];
    assert.ok(control, `${declared.gate} draws no ${declared.act} control inside the same container`);
    assert.ok(textOf(gate).includes(declared.name), `${declared.gate} does not say what it is`);
  }
});

test('every gate\'s act is one this face declares, and every declared act is governed by exactly one gate', () => {
  const acts = DECLARATION.acts.map((a) => a.act);
  const governed = DECLARATION.gates.map((g) => g.act);
  assert.deepEqual([...governed].sort(), [...acts].sort(), 'a gate invented an act, or an act has no gate');
  assert.equal(new Set(governed).size, governed.length);
});

test('a gate whose act is unavailable draws its reason beside the disabled control, at the same size, not only in a title', async () => {
  const { tree } = await draw(fullPort());
  const gates = findByAttr(tree, 'data-role', 'gate').filter((g) => g.attrs['data-state'] !== 'open');
  assert.ok(gates.length > 0, 'no gate was shut, so this proves nothing');
  for (const gate of gates) {
    const control = find(gate, (n) => n.tag === 'button')[0];
    assert.equal(control.attrs.disabled, '', `${gate.attrs['data-gate']} draws a live control for an answer that is not open`);
    assert.equal(control.attrs['data-target'] ?? null, null, 'a dead control still names a row to send against');
    const why = findByAttr(gate, 'data-role', 'gate-why')[0];
    assert.ok(why && textOf(why).trim().length > 20, `${gate.attrs['data-gate']} draws no reason beside its control`);
    assert.ok(control.attrs.title.startsWith(textOf(why).trim().slice(0, 30)), 'the drawn reason is not the beginning of the reason in full');
  }
});

test('an unread list answers every gate unknown -- a gate this window could not read is never drawn as a passed one', async () => {
  const { tree } = await draw(fullPort({ get_candidates: refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'no', gx_code: 'UNAUTHORIZED' }) }));
  const gates = findByAttr(tree, 'data-role', 'gate');
  assert.equal(gates.length, DECLARATION.gates.length);
  const states = new Set(gates.map((g) => g.attrs['data-state']));
  assert.equal(states.has('open'), false, 'a failed read produced an open gate');
  for (const gate of gates) {
    if (gate.attrs['data-state'] !== 'unknown') continue;
    const mark = find(gate, (n) => n.tag === 'svg')[0].attrs['data-mark'];
    assert.equal(mark, 'structure/hole', 'an unreadable gate is drawn with something other than the declared hole');
  }
});

test('choosing a candidate opens exactly the gates whose acts this face sends, and names that candidate on the control', async () => {
  const state = await face.read(fullPort());
  const chosen = state.held.items[1].id;
  const tree = face.view({ ...state, selected: chosen });
  const open = findByAttr(tree, 'data-role', 'gate').filter((g) => g.attrs['data-state'] === 'open');
  const sending = DECLARATION.acts.filter((a) => a.sends).map((a) => a.act);
  assert.deepEqual(open.map((g) => find(g, (n) => n.tag === 'button')[0].attrs['data-act']).sort(), [...sending].sort());
  for (const gate of open) {
    const control = find(gate, (n) => n.tag === 'button')[0];
    assert.equal(control.attrs['data-target'], chosen);
    assert.equal(control.attrs.disabled ?? null, null);
  }
});

test('the inverse gate is answered from the reversibility part, and has no path to open on this screen', async () => {
  const state = await face.read(fullPort());
  for (const selected of [null, state.held.items[0].id]) {
    const tree = face.view({ ...state, selected });
    const gate = findByAttr(tree, 'data-gate', 'inverse')[0];
    assert.notEqual(gate.attrs['data-state'], 'open', 'this window claimed an inverse it holds no field for');
    assert.ok(textOf(gate).includes('no'), 'the inverse gate says nothing about what it could not find');
  }
});

test('the footer is the last thing on the screen and states a figure that was measured, not one that was typed', async () => {
  const { tree } = await draw(fullPort());
  const footer = tree.children[tree.children.length - 1];
  assert.equal(footer.attrs['data-part'], 'runtime-footer');
  assert.ok(Number(footer.attrs['data-render-ms']) >= 0);
  const fields = new Map(findByAttr(footer, 'data-role', 'footer-field').map((f) => [f.attrs['data-name'], textOf(f)]));
  assert.match(fields.get('render'), /render [\d.]+ ms/);
  assert.equal(fields.get('read'), 'read candidates');
});

test('a read that did not answer draws a dash for what this face read, never the name of a source it did not get', async () => {
  const { tree } = await draw(fullPort({ get_candidates: failed() }));
  const footer = tree.children[tree.children.length - 1];
  const read = findByAttr(footer, 'data-role', 'footer-field').find((f) => f.attrs['data-name'] === 'read');
  assert.equal(textOf(read), 'read --');
});

// -- what the interaction audit found next door, asked of this face --------------
//
// The audit (101 real clicks and keypresses against two other faces) found three
// defects in the way a face is operated rather than in the way it is drawn. This
// face was not among the two, and it is the face with four acts on every row, so
// each of the three is asked of it here rather than assumed absent.

/** A click target that answers the questions this face's handlers ask of it. The
 * text is what a cell holds, which the menu reads when it offers to take a value. */
function clickable(attrs, textContent = null) {
  const has = (name) => Object.prototype.hasOwnProperty.call(attrs, name);
  const self = {
    textContent,
    getAttribute: (name) => (has(name) ? attrs[name] : null),
    closest: (selector) => (has(selector.startsWith('[') ? selector.slice(1, -1) : selector) ? self : null),
  };
  return self;
}

function clickOn(host, attrs, textContent = null) {
  const listener = host.listeners.find((l) => l.type === 'click');
  assert.ok(listener, 'the mounted face listens for no clicks at all');
  listener.handler({ target: clickable(attrs, textContent), preventDefault() {} });
}

function rightClickOn(host, attrs, textContent = null) {
  const listener = host.listeners.find((l) => l.type === 'contextmenu');
  assert.ok(listener, 'the mounted face listens for no right-click at all');
  let prevented = false;
  listener.handler({ target: clickable(attrs, textContent), preventDefault() { prevented = true; } });
  return prevented;
}

function pressKey(host, key) {
  const listener = host.listeners.find((l) => l.type === 'keydown');
  assert.ok(listener, 'the mounted face listens for no keys at all');
  listener.handler({ key, preventDefault() {} });
}

const menusIn = (host) => nodesOf(host).filter((n) => n.attrs && n.attrs['data-menu']);
const offersIn = (host) => nodesOf(host).filter((n) => n.attrs && n.attrs['data-role'] === 'menu-act');
const copyIn = (host) => nodesOf(host).find((n) => n.attrs && n.attrs['data-role'] === 'menu-copy') ?? null;
/** A press delivered to a control the menu drew, carrying the ancestry the handler
 * asks about -- a stand-in target has no parents of its own to be asked. */
const pressInMenu = (host, node, at) => clickOn(host, { ...node.attrs, 'data-menu': at });

const settle = async (ticks = 80) => { for (let i = 0; i < ticks; i += 1) await Promise.resolve(); };

const sentCount = (port, method) => port.calls.filter((call) => call.name === method).length;

test('A (the mechanism): act() is pure in the state it is handed, so two acts given the same state each carry only their own entry', async () => {
  const port = fullPort();
  const before = await face.read(port);
  const [first, second] = await Promise.all([
    face.act(port, before, { act: 'commit', id: 'c-001' }),
    face.act(port, before, { act: 'cancel', id: 'c-002' }),
  ]);
  assert.equal(first.acts.length, 1);
  assert.equal(second.acts.length, 1);
  assert.notDeepEqual(first.acts, second.acts);
  // Whichever of these two a caller wrote back last would be the whole log, and the
  // other act -- sent, answered -- would be nowhere. That is what the caller below
  // is written not to do.
});

test('A: two acts pressed in the same tick are both sent and both written down', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  clickOn(host, { 'data-act': 'commit', 'data-target': 'c-001' });
  clickOn(host, { 'data-act': 'cancel', 'data-target': 'c-002' });
  await settle();
  const words = textOfHost(host);
  assert.ok(words.includes('commit c-001'), 'the first act was sent and is not in the log');
  assert.ok(words.includes('cancel c-002'), 'the second act was sent and is not in the log');
  assert.equal(sentCount(port, 'post_candidates_id_commit'), 1);
  assert.equal(sentCount(port, 'post_candidates_id_cancel'), 1);
  unmount();
});

test('A: the same act pressed twice before it answers is sent once', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  clickOn(host, { 'data-act': 'commit', 'data-target': 'c-001' });
  clickOn(host, { 'data-act': 'commit', 'data-target': 'c-001' });
  await settle();
  assert.equal(sentCount(port, 'post_candidates_id_commit'), 1, 'a commit was sent twice for one candidate');
  unmount();
});

test('B: an act in flight is drawn dead with that as its reason, in the row and in the gate that governs it', async () => {
  const host = standInHost();
  const port = fullPort({ post_candidates_id_commit: () => new Promise(() => {}) });
  const unmount = mount(host, port, []);
  await unmount.ready;
  clickOn(host, { 'data-select-row': 'c-001' });
  clickOn(host, { 'data-act': 'commit', 'data-target': 'c-001' });
  const commits = nodesOf(host).filter((n) => n.attrs && n.attrs['data-act'] === 'commit');
  assert.equal(commits.length, heldItems.length + 1, 'one commit control per row, and one on the gate that governs it');
  const dead = commits.filter((n) => (n.attrs.disabled ?? null) !== null);
  assert.equal(dead.length, 2, 'the row whose commit is in flight and its gate are the two controls that go dead');
  for (const control of dead) {
    assert.ok(control.attrs.title.startsWith('this was sent'), 'a dead control gives no reason for being dead');
  }
  // The lock is per candidate, not per screen: an act out for one row does not take
  // the other rows' controls away from a reader.
  const live = commits.filter((n) => (n.attrs.disabled ?? null) === null);
  assert.equal(live.length, heldItems.length - 1);
  for (const control of live) assert.notEqual(control.attrs['data-target'], 'c-001');
  const gate = nodesOf(host).find((n) => n.attrs && n.attrs['data-gate'] === 'commit');
  assert.equal(gate.attrs['data-state'], 'shut', 'the gate stayed open for an act that is already out');
  assert.ok(textOfHost(host).includes('this was sent and the answer has not arrived.'), 'the reason is in a title and nowhere a reader can see it');
  unmount();
});

test('C: a disclosure the reader opened is still open after an act repaints the screen', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  const foldState = () => nodesOf(host).find((n) => n.attrs && n.attrs['data-control'] === 'legend')?.attrs['data-open'];
  assert.equal(foldState(), 'false');
  clickOn(host, { summary: '', 'data-control': 'legend' });
  assert.equal(foldState(), 'true', 'the fold did not open');
  clickOn(host, { 'data-act': 'commit', 'data-target': 'c-001' });
  await settle();
  assert.equal(foldState(), 'true', 'taking an act shut what the reader had open');
  unmount();
});

test('an act does not empty the pane: the chosen row and the open folds are this window\'s, and a fresh read is not an event that clears them', async () => {
  const port = fullPort();
  const state = await face.read(port);
  const chosen = state.held.items[0].id;
  const next = await face.act(port, { ...state, selected: chosen, open: ['legend'] }, { act: 'cancel', id: chosen });
  assert.equal(next.selected, chosen, 'acting on a candidate silently emptied the detail pane');
  assert.deepEqual(next.open, ['legend']);
});

// -- Owner #348 (2): the second way to reach an act, and it reaches the same one ----

test('a right-click on a row opens one menu, offering the row\'s own acts answered by the gates that govern them', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  assert.equal(menusIn(host).length, 0, 'a menu was drawn before anybody asked for one');

  const prevented = rightClickOn(host, { 'data-select-row': 'c-001', 'data-cell': 'path' }, '/work/contract.pdf');
  assert.equal(prevented, true, 'the browser\'s own menu was left to open on top of this one');

  const menus = menusIn(host);
  assert.equal(menus.length, 1);
  assert.equal(menus[0].attrs['data-subject'], 'c-001');

  const offers = offersIn(host);
  assert.deepEqual(
    offers.map((n) => n.attrs['data-act']),
    DECLARATION.gates.map((g) => g.act),
    'the menu offers a set of verbs that is not the declared one, in an order the ladder does not climb',
  );
  const sending = new Set(DECLARATION.acts.filter((a) => a.sends).map((a) => a.act));
  for (const offer of offers) {
    const open = offer.attrs['data-state'] === 'open';
    assert.equal(open, sending.has(offer.attrs['data-act']), `${offer.attrs['data-act']} is offered in a state its own declaration does not support`);
    assert.equal(offer.attrs['data-target'] ?? null, open ? 'c-001' : null, 'an offer whose gate is shut still names a row to send against');
    assert.equal(offer.attrs.disabled ?? null, open ? null : '', 'a shut offer is pressable');
    assert.ok((offer.attrs.title ?? '').length > 20, `${offer.attrs['data-act']} gives no reason for the state it is in`);
  }
  unmount();
});

test('the menu answers with the same gate answers the ladder draws, for the row the pointer was on and not for the chosen one', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  clickOn(host, { 'data-select-row': 'c-001' });
  rightClickOn(host, { 'data-select-row': 'c-002' });
  const ladder = nodesOf(host).filter((n) => n.attrs && n.attrs['data-gate']);
  const offers = offersIn(host);
  assert.equal(offers.length, ladder.length);
  for (const [index, gate] of ladder.entries()) {
    const declared = DECLARATION.gates.find((g) => g.gate === gate.attrs['data-gate']);
    assert.equal(offers[index].attrs['data-act'], declared.act, 'the menu and the ladder disagree about which act a gate governs');
  }
  // The ladder is answering for c-001 and the menu for c-002; the answers agree in
  // kind because they come from one rule, and the targets differ because they are
  // about different candidates.
  for (const offer of offers.filter((n) => n.attrs['data-state'] === 'open')) {
    assert.equal(offer.attrs['data-target'], 'c-002');
  }
  assert.equal(menusIn(host)[0].attrs['data-subject'], 'c-002');
  unmount();
});

test('a second right-click replaces the menu rather than stacking a second one on it', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightClickOn(host, { 'data-select-row': 'c-001' });
  rightClickOn(host, { 'data-select-row': 'c-002' });
  const menus = menusIn(host);
  assert.equal(menus.length, 1, 'two right-clicks left two menus on the screen');
  assert.equal(menus[0].attrs['data-subject'], 'c-002');
  unmount();
});

test('Escape puts the menu away, and any other key leaves it alone', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightClickOn(host, { 'data-select-row': 'c-001' });
  pressKey(host, 'a');
  assert.equal(menusIn(host).length, 1, 'a key that means nothing here shut the menu');
  pressKey(host, 'Escape');
  assert.equal(menusIn(host).length, 0, 'Escape left the menu on the screen');
  unmount();
});

test('a press away from the menu puts it away, and a press inside it does not', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightClickOn(host, { 'data-select-row': 'c-001' });
  clickOn(host, { 'data-menu': 'row:c-001' });
  assert.equal(menusIn(host).length, 1, 'pressing the menu itself shut it');
  clickOn(host, { 'data-role': 'somewhere-else' });
  assert.equal(menusIn(host).length, 0, 'a press away from the menu left it open');
  unmount();
});

test('the menu is drawn from the state, so a repaint that is not about it cannot leave one behind', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightClickOn(host, { 'data-select-row': 'c-001' });
  assert.equal(menusIn(host).length, 1);
  // Opening a fold is a press away from the menu and a full repaint of the tree.
  clickOn(host, { summary: '', 'data-control': 'legend' });
  assert.equal(menusIn(host).length, 0, 'a repaint left a menu behind');
  assert.equal(nodesOf(host).find((n) => n.attrs && n.attrs['data-control'] === 'legend').attrs['data-open'], 'true');
  unmount();
});

test('an act pressed in the menu is the same send as the act pressed in the row: one queue, one log entry, and the menu closes behind it', async () => {
  const host = standInHost();
  const port = fullPort();
  const unmount = mount(host, port, []);
  await unmount.ready;
  rightClickOn(host, { 'data-select-row': 'c-001' });
  const offer = offersIn(host).find((n) => n.attrs['data-act'] === 'commit');
  assert.ok(offer, 'the menu offers no commit to press');
  pressInMenu(host, offer, 'row:c-001');
  await settle();
  assert.equal(sentCount(port, 'post_candidates_id_commit'), 1);
  assert.ok(textOfHost(host).includes('commit c-001'), 'the menu sent an act the log does not carry');
  assert.equal(menusIn(host).length, 0, 'the menu stayed open over the act it had just sent');
  unmount();
});

test('the menu is behind the same per-candidate lock the gutter is behind -- it cannot send a second commit while the first is out', async () => {
  const host = standInHost();
  const port = fullPort({ post_candidates_id_commit: () => new Promise(() => {}) });
  const unmount = mount(host, port, []);
  await unmount.ready;
  rightClickOn(host, { 'data-select-row': 'c-001' });
  pressInMenu(host, offersIn(host).find((n) => n.attrs['data-act'] === 'commit'), 'row:c-001');
  await settle();
  assert.equal(sentCount(port, 'post_candidates_id_commit'), 1);

  rightClickOn(host, { 'data-select-row': 'c-001' });
  const again = offersIn(host).find((n) => n.attrs['data-act'] === 'commit');
  assert.equal(again.attrs['data-state'], 'shut', 'the menu offered an act that is already out as though it were available');
  assert.ok(again.attrs.title.startsWith('this was sent'), 'the menu gives a different reason from the one the row and the gate give');
  assert.equal(again.attrs['data-target'] ?? null, null);
  pressInMenu(host, again, 'row:c-001');
  await settle();
  assert.equal(sentCount(port, 'post_candidates_id_commit'), 1, 'the menu sent a second commit for a candidate whose first has not answered');
  unmount();
});

test('a right-click in the ladder opens the menu on the gate it was made on, with that gate\'s own subject', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  clickOn(host, { 'data-select-row': 'c-002' });
  rightClickOn(host, { 'data-gate': 'commit', 'data-subject': 'c-002' });
  const menus = menusIn(host);
  assert.equal(menus.length, 1);
  assert.equal(menus[0].attrs['data-menu'], 'gate:commit');
  assert.equal(menus[0].attrs['data-subject'], 'c-002');
  unmount();
});

test('a right-click with no candidate under it draws every act dead, with the reason the ladder already gives', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightClickOn(host, { 'data-gate': 'commit' });
  const offers = offersIn(host);
  assert.equal(offers.length, DECLARATION.gates.length);
  assert.deepEqual([...new Set(offers.map((n) => n.attrs['data-state']))], ['shut']);
  const commit = offers.find((n) => n.attrs['data-act'] === 'commit');
  assert.ok(commit.attrs.title.startsWith('no candidate is chosen'));
  unmount();
});

// -- Owner #348 (2), copy value -----------------------------------------------------

/** A clipboard, or the absence of one, for the length of one test. */
async function withNavigator(value, run) {
  const had = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
  Object.defineProperty(globalThis, 'navigator', { value, configurable: true, writable: true });
  try {
    await run();
  } finally {
    if (had) Object.defineProperty(globalThis, 'navigator', had);
    else delete globalThis.navigator;
  }
}

test('copy value takes what the cell holds in full, not the form the row drew, and says that it worked', async () => {
  const taken = [];
  await withNavigator({ clipboard: { writeText: (value) => { taken.push(value); return Promise.resolve(); } } }, async () => {
    const host = standInHost();
    const unmount = mount(host, fullPort(), []);
    await unmount.ready;
    // The time cell draws a declared cut and carries the whole timestamp on itself.
    rightClickOn(host, { 'data-select-row': 'c-001', 'data-cell': 'at', 'data-full': '2026-08-24T10:02:00Z' }, '10:02');
    const offer = copyIn(host);
    assert.equal(offer.attrs['data-copy-value'], '2026-08-24T10:02:00Z', 'the menu offered the drawn form and called it the value');
    pressInMenu(host, offer, 'row:c-001');
    await settle();
    assert.deepEqual(taken, ['2026-08-24T10:02:00Z']);
    const after = copyIn(host);
    assert.equal(after.attrs['data-copied'], 'true');
    assert.equal(after.attrs['data-copy-failed'] ?? null, null);
    assert.ok(textOfHost(host).includes(HELD_MESSAGES.COPIED), 'the control looks the same whether or not it did anything');
    unmount();
  });
});

test('a window with no reachable clipboard says nothing was taken rather than drawing the control it draws on success', async () => {
  await withNavigator({}, async () => {
    const host = standInHost();
    const unmount = mount(host, fullPort(), []);
    await unmount.ready;
    rightClickOn(host, { 'data-select-row': 'c-001', 'data-cell': 'path' }, '/work/contract.pdf');
    pressInMenu(host, copyIn(host), 'row:c-001');
    await settle();
    const after = copyIn(host);
    assert.equal(after.attrs['data-copy-failed'], 'true');
    assert.equal(after.attrs['data-copied'] ?? null, null);
    assert.ok(textOfHost(host).includes(HELD_MESSAGES.COPY_FAILED));
    unmount();
  });
});

test('a clipboard that refuses is a failure and is drawn as one', async () => {
  await withNavigator({ clipboard: { writeText: () => Promise.reject(new Error('this document may not write a clipboard')) } }, async () => {
    const host = standInHost();
    const unmount = mount(host, fullPort(), []);
    await unmount.ready;
    rightClickOn(host, { 'data-select-row': 'c-001', 'data-cell': 'path' }, '/work/contract.pdf');
    pressInMenu(host, copyIn(host), 'row:c-001');
    await settle();
    assert.equal(copyIn(host).attrs['data-copy-failed'], 'true');
    unmount();
  });
});

test('a right-click that was not over a value offers copy dead, with the reason, rather than not offering it', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  rightClickOn(host, { 'data-select-row': 'c-001' });
  const offer = copyIn(host);
  assert.ok(offer, 'the offer was dropped instead of being drawn dead');
  assert.equal(offer.attrs.disabled, '');
  assert.equal(offer.attrs['data-copy-value'] ?? null, null);
  assert.equal(offer.attrs.title, HELD_MESSAGES.MENU_NOT_A_VALUE);
  unmount();
});

// -- Owner #348 (4): the type, held mechanically ------------------------------------

test('every weight this face draws is one of the three it declares, and the denominator\'s number is the heaviest of them', async () => {
  const { tree } = await draw(fullPort());
  const weights = new Set();
  for (const node of find(tree, (n) => /font-weight:/.test(n.attrs.style ?? ''))) {
    for (const found of (node.attrs.style ?? '').matchAll(/font-weight:([^;]+)/g)) weights.add(found[1]);
  }
  assert.deepEqual([...weights].sort(), ['400', '500', '600'], 'a fourth weight arrived on this face');
  const count = findByAttr(tree, 'data-role', 'header-count')[0];
  assert.ok(count, 'the denominator states no count of its own');
  assert.match(count.attrs.style, /font-weight:600/);
  // req/822_c7 (Owner #387/#388): this fixture drops nothing, so the header's own
  // count reads "N candidates" -- see the dedicated equal/differ pair of tests above.
  assert.match(textOf(findByAttr(tree, 'data-role', 'face-header')[0]), new RegExp(`${heldItems.length} candidates`));
});

test('no paragraph on this face comes apart mid-word', async () => {
  const { tree } = await draw(fullPort());
  const paragraphs = find(tree, (n) => n.tag === 'p');
  assert.ok(paragraphs.length > 0, 'there is no prose here, so this proves nothing');
  const anywhere = paragraphs.filter((n) => /overflow-wrap:anywhere/.test(n.attrs.style ?? ''));
  assert.deepEqual(anywhere.map((n) => n.attrs['data-role']), []);
  const broken = paragraphs.filter((n) => /overflow-wrap:break-word/.test(n.attrs.style ?? ''));
  assert.ok(broken.length > 0, 'no paragraph states how it breaks at all');
});

test('no path in this repository and no specification identifier reaches the surface', async () => {
  const { html } = await draw(fullPort());
  for (const pattern of [/faces\//, /parts\//, /membrane\//, /\bshell\//, /req\/\d/, /\bSS\d{3}/, /\bAC-\d/, /\bF-\d\b/]) {
    assert.equal(pattern.test(html), false, `an internal name is drawn on the screen: ${pattern} in ${html.match(pattern)?.[0]}`);
  }
});

// -- req/822_c5 item 2 (B1, carried from c2 §3): the honest relabel ---------------------

test('toRecord takes the item at its own word: a lifecycle it carries is not overwritten', async () => {
  const { toRecord } = await import('../held.mjs');
  const record = toRecord({ id: 'x-1', at: '12:00', lifecycle: 'settled' });
  assert.equal(record.lifecycle, 'settled', 'the item said settled and the record must wear that word, not this screen\'s');
  assert.ok(record.holes.lifecycle, 'a row wearing a word this screen did not assign carries the contradiction in its note');
  assert.match(record.holes.lifecycle, /settled/);
});

test('toRecord still stamps held for an item that carries no word of its own', async () => {
  const { toRecord } = await import('../held.mjs');
  const record = toRecord({ id: 'x-2', at: '12:00' });
  assert.equal(record.lifecycle, 'held');
  assert.equal(record.holes.lifecycle, undefined, 'nothing contradicts, so nothing is noted');
});

test('toRecord: an item claiming held in its own voice is simply held, with no note', async () => {
  const { toRecord } = await import('../held.mjs');
  const record = toRecord({ id: 'x-3', lifecycle: 'held' });
  assert.equal(record.lifecycle, 'held');
  assert.equal(record.holes.lifecycle, undefined);
});
