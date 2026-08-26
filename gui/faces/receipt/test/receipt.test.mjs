// SPDX-License-Identifier: Apache-2.0
// What the face does, asked of the face and not of a description of it.
//
// The one thing these tests exist to guard above everything else: this screen may
// never draw "sealed" because a payload said so. Every other test here is either a
// property this face carries over from faces/ledger's/faces/held's shared
// discipline (fail-closed, no colour, every glyph sized) or the ordinary
// declaration/mark/gate checks every face in this tree holds.
//
// stub-port.mjs and dom-stand-in.mjs are read from faces/ledger/test/, not
// duplicated here -- the same precedent faces/notice and faces/held already set
// (req/99 §5).

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createFace, face, mount, toRecord, digestAgreement, census, RECEIPT_MESSAGES,
} from '../receipt.mjs';
import { DECLARATION } from '../declaration.mjs';
import { parts } from '../binding.mjs';
import { standInHost, textOfHost, nodesOf } from '../../ledger/test/dom-stand-in.mjs';
import {
  stubPort, answered, refused, failed, absent, SAMPLE,
} from '../../ledger/test/stub-port.mjs';

const { el, toHtml, find, findByAttr, textOf } = parts.element;

const DELTA_ID = 't-001';
const delta = (extra = {}) => SAMPLE.transformation(1, extra);
const receiptBody = (extra = {}) => ({
  digest: delta().digest, algorithm: 'sha256', anchor: 'https://example.test/anchor/t-001', basis: 'exact', ...extra,
});

function fullPort(overrides = {}) {
  return stubPort({
    get_transformations_id: answered(delta()),
    get_receipts_tid: answered(receiptBody()),
    ...overrides,
  }, { methods: DECLARATION.consumes });
}

async function draw(port, id = DELTA_ID) {
  const state = await face.read(port, id);
  return { state, tree: face.view(state), html: toHtml(face.view(state)) };
}

const sectionOf = (tree, name) => findByAttr(tree, 'data-section', name)[0] ?? null;
const attrOf = (tree, name) => find(tree, (n) => name in n.attrs).map((n) => n.attrs[name]);

// -- mount --------------------------------------------------------------------

test('W2: mount returns a function, and unmount empties the host', async () => {
  const host = standInHost();
  host.setAttribute('data-receipt-id', DELTA_ID);
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
  assert.throws(() => mount(null, fullPort(), []), new RegExp(RECEIPT_MESSAGES.NO_HOST));
  assert.throws(() => mount(standInHost(), null, []), new RegExp(RECEIPT_MESSAGES.NO_PORT));
});

test('mount draws something before the read answers', async () => {
  const host = standInHost();
  host.setAttribute('data-receipt-id', DELTA_ID);
  const unmount = mount(host, fullPort(), []);
  const early = textOfHost(host);
  assert.ok(early.includes(RECEIPT_MESSAGES.READING));
  await unmount.ready;
  unmount();
});

test('mount with no data-receipt-id on the host draws the no-id state, not a silent blank screen', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  const text = textOfHost(host);
  assert.ok(text.includes(RECEIPT_MESSAGES.NO_ID));
  unmount();
});

// -- C-1 ------------------------------------------------------------------------

test('C-1: every method the face calls was declared', async () => {
  const port = fullPort();
  await face.read(port, DELTA_ID);
  const called = [...new Set(port.calls.map((c) => c.name))];
  const undeclared = called.filter((name) => !DECLARATION.consumes.includes(name));
  assert.deepEqual(undeclared, []);
  assert.ok(called.length > 0);
});

test('C-1 negative control: calling an undeclared method raises rather than reaching the port', async () => {
  const port = fullPort();
  const caller = face.callerFor(port);
  await assert.rejects(() => caller.invoke('get_healthz', {}), new RegExp(RECEIPT_MESSAGES.UNDECLARED));
  assert.equal(port.calls.some((c) => c.name === 'get_healthz'), false, 'the call reached the port anyway');
});

test('a caller cannot name the actor: this face performs no writes, so there is nothing to guard there directly, but no call ever carries one', async () => {
  const port = fullPort();
  await face.read(port, DELTA_ID);
  for (const call of port.calls) {
    assert.equal(Object.prototype.hasOwnProperty.call(call.input?.params ?? {}, 'actor'), false);
  }
});

// -- fail-closed, two independent reads ------------------------------------------

for (const [name, result] of [
  ['failed', failed()],
  ['refused', refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not read this transformation', gx_code: 'UNAUTHORIZED' })],
  ['absent', absent({ name: 'get_transformations_id' })],
]) {
  test(`fail-closed: a ${name} delta read draws the delta as unread, and still attempts the receipt read`, async () => {
    const port = fullPort({ get_transformations_id: result });
    const { tree } = await draw(port);
    const deltaSection = sectionOf(tree, 'delta');
    assert.equal(deltaSection.attrs['data-state'], 'unread');
    assert.ok(textOf(deltaSection).includes(RECEIPT_MESSAGES.DELTA_UNREAD));
    assert.ok(textOf(deltaSection).includes(name), 'the outcome is not named on screen');
    assert.equal(port.calls.some((c) => c.name === 'get_receipts_tid'), true, 'the receipt read was skipped because the delta read failed');
  });
}

for (const [name, result] of [
  ['failed', failed()],
  ['refused', refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not read this receipt', gx_code: 'UNAUTHORIZED' })],
  ['absent', absent({ name: 'get_receipts_tid' })],
]) {
  test(`fail-closed: a ${name} receipt read draws address/bytes/verify as holes, never as absent-looking-empty`, async () => {
    const port = fullPort({ get_receipts_tid: result });
    const { html } = await draw(port);
    assert.ok(html.includes(RECEIPT_MESSAGES.MEMBER_UNREAD), 'an unread receipt member was not stated as unread');
    assert.equal(html.includes(RECEIPT_MESSAGES.ADDRESS_MISSING) || html.includes(RECEIPT_MESSAGES.MEMBER_UNREAD), true);
  });
}

// -- the record is attest: verbatim, holes never dropped -------------------------

test('toRecord: every delta member absent from the body becomes a named hole, not a silent omission', () => {
  const record = toRecord({ id: DELTA_ID, delta: answered({ ...delta(), actor: undefined }), receipt: answered(receiptBody()) });
  assert.ok(record.holes.actor?.includes(RECEIPT_MESSAGES.MEMBER_ABSENT));
  assert.equal(record.actor, undefined);
});

test('toRecord: an unread delta turns every delta member into a named "could not be read" hole', () => {
  const record = toRecord({ id: DELTA_ID, delta: failed(), receipt: answered(receiptBody()) });
  for (const key of ['at', 'actor', 'effect', 'verdict', 'path', 'digest']) {
    assert.ok(record.holes[key]?.includes(RECEIPT_MESSAGES.MEMBER_UNREAD), `${key} was not stated as unread`);
  }
  assert.equal(record.deltaOutcome, 'failed');
});

test('toRecord: a receipt member that is not a scalar is a named hole, not a crash and not a silent drop', () => {
  const record = toRecord({ id: DELTA_ID, delta: answered(delta()), receipt: answered({ ...receiptBody(), anchor: { nested: true } }) });
  assert.ok(record.holes.receipt_anchor?.includes(RECEIPT_MESSAGES.MEMBER_NOT_SCALAR));
  assert.equal(record.anchor, null);
});

// -- digest agreement (this face's own decide-step addition) --------------------

test('digestAgreement holds when the receipt\'s digest is the identical string to the delta\'s', () => {
  const record = toRecord({ id: DELTA_ID, delta: answered(delta()), receipt: answered(receiptBody()) });
  const claim = digestAgreement(record);
  assert.equal(claim.holds, true);
  assert.ok(claim.detail.includes(record.digest));
});

test('digestAgreement does not hold when the two digests disagree, and names both values', () => {
  const record = toRecord({ id: DELTA_ID, delta: answered(delta()), receipt: answered(receiptBody({ digest: 'deadbeef0000' })) });
  const claim = digestAgreement(record);
  assert.equal(claim.holds, false);
  assert.ok(claim.detail.includes(record.digest));
  assert.ok(claim.detail.includes('deadbeef0000'));
});

test('digestAgreement cannot hold when either digest is missing, and says which is missing', () => {
  const record = toRecord({ id: DELTA_ID, delta: answered({ ...delta(), digest: undefined }), receipt: answered(receiptBody()) });
  const claim = digestAgreement(record);
  assert.equal(claim.holds, false);
  assert.ok(claim.detail.includes('delta digest missing'));
});

test('the verify section draws the digest-agreement claim, and shows a genuine disagreement, not a planted one', async () => {
  const port = fullPort({ get_receipts_tid: answered(receiptBody({ digest: 'deadbeef0000' })) });
  const { tree } = await draw(port);
  const verify = sectionOf(tree, 'verify');
  const claimNode = findByAttr(verify, 'data-claim', 'receipt-digest-agrees-with-delta')[0];
  assert.ok(claimNode);
  assert.equal(claimNode.attrs['data-holds'], 'false');
  assert.ok(textOf(claimNode).includes('deadbeef0000'));
});

// -- never a boolean sealed claim (glovrex/req/405 SS5, the H-class defence) -----

test('a mischievous sealed:true in the wire payload changes nothing: no verifier means unsealed, always', async () => {
  const port = fullPort({ get_receipts_tid: answered({ ...receiptBody(), sealed: true }) });
  const { html, tree } = await draw(port);
  assert.equal(html.includes('"sealed"'), false, 'the payload\'s own boolean reached the drawn attribute space');
  const verify = sectionOf(tree, 'verify');
  // The standing was drawn as the words "seal claim: unsealed" until the r5 pass took
  // that label off (the box head already wears the standing as a pill). It is now an
  // attribute on the line, which is what tools/gate.mjs reads too -- a stricter thing
  // to assert than a substring, because a reworded sentence changes an attribute check
  // from green to red rather than from green to silent.
  const claimLine = findByAttr(verify, 'data-role', 'seal-claim')[0];
  assert.equal(claimLine.attrs['data-standing'], 'unsealed');
  assert.ok(textOf(claimLine).includes('no verifier is present'), 'the reason is still drawn, in words, on the line');
  const sealCell = findByAttr(tree, 'data-cell', 'seal')[0];
  const sealGlyph = findByAttr(sealCell, 'data-mark', 'structure/seal');
  assert.equal(sealGlyph.length, 0, 'a sealed mark was drawn with no verifier present');
});

test('the seal claim\'s standing and its drawn mark always agree, because both come from the one claimOf() call', async () => {
  const { tree } = await draw(fullPort());
  const sealCell = findByAttr(tree, 'data-cell', 'seal')[0];
  const glyph = find(sealCell, (n) => n.tag === 'svg')[0];
  assert.equal(glyph.attrs['data-mark'], 'structure/unsealed', 'no verifier is ever wired in this environment, so this screen never draws structure/seal');
});

// -- portability (b)/(c) ----------------------------------------------------------

test('when the receipt carries digest/algorithm/anchor, portability says this receipt is checkable elsewhere', async () => {
  const { tree } = await draw(fullPort());
  const verify = sectionOf(tree, 'verify');
  assert.ok(textOf(verify).includes('everything needed to check this elsewhere is present'));
});

test('when the anchor is missing, portability names exactly what is missing, and the address section says so honestly', async () => {
  const { tree, html } = await draw(fullPort({ get_receipts_tid: answered({ ...receiptBody(), anchor: undefined }) }));
  const verify = sectionOf(tree, 'verify');
  assert.ok(textOf(verify).includes('anchor'));
  assert.ok(html.includes(RECEIPT_MESSAGES.ADDRESS_MISSING) || /receipt_anchor/.test(html) === false);
});

// -- (b) bytes: the serial part's own two sentences travel with it ----------------

test('the bytes section never emits the cut digest without its cut sentence and its not-a-proof sentence', async () => {
  const { html } = await draw(fullPort());
  assert.ok(/the first 6 of \d+ hexadecimal characters/.test(html), 'the cut sentence is missing');
  assert.ok(html.includes('a match here is a hint and not a proof'), 'the not-a-proof caveat is missing');
});

// -- C-3: what is not drawn ---------------------------------------------------------

test('C-3: the members this face does not draw are named with reasons', async () => {
  const { tree } = await draw(fullPort());
  const words = textOf(sectionOf(tree, 'not-drawn'));
  for (const entry of DECLARATION.undrawn) assert.ok(words.includes(entry.what), `${entry.what} is declared undrawn but not stated on screen`);
});

// -- order --------------------------------------------------------------------------

test('C-6: the order reason is stated on screen', async () => {
  const { html } = await draw(fullPort());
  assert.ok(html.includes('third, after ledger'));
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
  };
  const other = createFace({ parts: stub });
  const state = await other.read(fullPort(), DELTA_ID);
  const html = toHtml(other.view(state));
  assert.ok(html.includes(marker));
});

/**
 * Two calls on one state draw the same screen apart from one field, and the exception
 * is named rather than tolerated: the footer's render figure is a measurement of the
 * call that produced it, so two calls that measured the same number would mean the
 * clock was not being read. Everything else is compared byte for byte.
 */
const MEASURED = /(data-render-ms="[^"]*")|(render [\d.]+ ms)/g;
const withoutTheMeasurement = (html) => html.replace(MEASURED, 'measured');

test('rendering the same state twice gives the same tree, apart from the figure that was measured', async () => {
  const state = await face.read(fullPort(), DELTA_ID);
  assert.equal(withoutTheMeasurement(toHtml(face.view(state))), withoutTheMeasurement(toHtml(face.view(state))));
});

// -- SS657 retrofit (req/38 SS657 Owner #317/#318, idiom proven by faces/atlas) --

test('SS657 defect 4/5 cure: a single compact header line states the face name and the delta/receipt outcomes, before anything else', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  assert.ok(header);
  assert.ok(textOf(header).includes('receipt'));
  assert.match(textOf(header), /delta \w+, receipt \w+/);
  // The header now carries the delta's own id as well, which is the string a reader
  // needs to come back to this receipt, and is the cell the menu's copy entry matters
  // most on. It sits between the face's name and the two outcomes.
  assert.ok(textOf(header).includes(DELTA_ID), 'the screen never said which delta it was about');
  // Still first on the screen, one wrapper deeper: a menu is appended to a block, and
  // appending one into the header's own flex row would make it a third item in that row.
  const first = tree.children[0];
  assert.equal(first.attrs['data-copy-anchor'], 'subject');
  assert.equal(first.children[0], header);
});

test('SS657 defect 2 cure: why/legend are bordered, self-evident controls sitting in one row, each with a plain-language hint', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const row = findByAttr(tree, 'data-role', 'control-row')[0];
  assert.ok(row);
  const controls = findByAttr(row, 'data-role', 'control');
  assert.equal(controls.length, 2);
  const why = findByAttr(row, 'data-control', 'why')[0];
  const legend = findByAttr(row, 'data-control', 'legend')[0];
  assert.ok(why.attrs.style.includes('border'));
  assert.ok(legend.attrs.style.includes('border'));
  // req/822_c7 (Owner #387/#388 冗長文字全掃): the hint no longer draws as its own
  // visible span beside the label -- it rides the control's own summary as a
  // title (a hover) and a data-hint attribute.
  const summaryOf = (control) => find(control, (n) => n.tag === 'summary')[0];
  assert.equal(summaryOf(why).attrs['data-hint'], 'about this screen');
  assert.equal(summaryOf(legend).attrs['data-hint'], 'symbols and counts');
  assert.doesNotMatch(textOf(why), /about this screen/, 'the hint is still drawn as visible text');
  assert.doesNotMatch(textOf(legend), /symbols and counts/, 'the hint is still drawn as visible text');
  assert.equal(why.attrs['data-open'], 'false');
  assert.equal(legend.attrs['data-open'], 'false');
});

test('SS657 defect 1/3 cure: legend is a zero-inclusive counted table -- every declared mark gets a row, including ones this render drew zero of', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
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

test('the legend also carries a not-drawn row (with its reason) for this face\'s own declared unreached set', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  const notDrawn = findByAttr(legend, 'data-not-drawn');
  assert.equal(notDrawn.length, DECLARATION.undrawn.length);
  for (const entry of DECLARATION.undrawn) {
    const own = notDrawn.find((n) => n.attrs['data-not-drawn'] === entry.what);
    assert.ok(own, `no legend row for undrawn entry: ${entry.what}`);
  }
});

test('SS657 defect 1 cure: the delta row, when it has a note, is a native, user-openable disclosure that states the count of what it withholds', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const disclosures = findByAttr(tree, 'data-part', 'receipt-row-disclosure');
  assert.ok(disclosures.length > 0, 'the delta this fixture reads has at least one field it withholds');
  for (const d of disclosures) {
    assert.equal(d.tag, 'details');
    const badge = findByAttr(d, 'data-role', 'withheld-count')[0];
    assert.ok(badge);
    assert.match(textOf(badge), /\d+ more field/);
  }
});

// -- retrofit round 2 (req/768 AC-6/AC-7, SS657 continued) --

test('AC-7: this screen always reads the reversibility chip as unknown -- one delta with no sibling list, so reversed can never be observed here', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const chip = findByAttr(tree, 'data-part', 'reversal-chip')[0];
  assert.ok(chip, 'the delta row carries a reversibility chip');
  assert.equal(chip.attrs['data-state'], 'not-observable');
  assert.ok(textOf(chip).includes('unknown'));
  assert.equal(find(chip, (n) => n.tag === 'svg')[0].attrs['data-mark'], 'standing/none');
});

test('AC-7: the honest reason is reachable on the chip itself, and the legend explains it once, not per row', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const chip = findByAttr(tree, 'data-part', 'reversal-chip')[0];
  assert.match(chip.attrs.title, /membrane\/wire-fields\.json/);
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  assert.match(textOf(legend), /undo availability chip/);
});

test('AC-4: no acts, no gutter -- this face declares no commit/cancel/undo route, so there is nothing a gutter could ever hold', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  assert.equal(findByAttr(tree, 'data-part', 'act-gutter').length, 0);
  assert.equal(findByAttr(tree, 'data-part', 'row-gutter-frame').length, 0);
  assert.deepEqual(DECLARATION.acts, [], 'this face has never had an ACTS list to draw a gutter from');
});

// -- retrofit r4: the screen states its own size before it states a word ------------

const bandOf = (tree) => findByAttr(tree, 'data-part', 'stat-band')[0];
const segmentsOf = (tree) => findByAttr(bandOf(tree), 'data-role', 'segment');
const boxesOf = (tree) => findByAttr(tree, 'data-part', 'box');
const boxNamed = (tree, name) => boxesOf(tree).find((b) => b.attrs['data-box'] === name);

test('r4: the band is the second thing on the screen -- after the one header line, before the controls', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const band = bandOf(tree);
  assert.ok(band, 'this face draws no band, so a viewer reads prose before they know the size of anything');
  assert.equal(tree.children[1], band);
  assert.equal(tree.children[2], findByAttr(tree, 'data-role', 'control-row')[0]);
});

test('r4: every band segment states a noun beside its number, and there are between three and five of them', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const segments = segmentsOf(tree);
  assert.ok(segments.length >= 3 && segments.length <= 5, `${segments.length} segments`);
  for (const segment of segments) assert.ok((segment.attrs['data-noun'] ?? '').trim().length > 0);
});

test('r4: every figure in the band is counted from the record this window read, not written by hand', async () => {
  const state = await face.read(fullPort(), DELTA_ID);
  const record = toRecord(state);
  const counted = census(record);
  const drawn = Object.fromEntries(segmentsOf(face.view(state)).map((s) => [s.attrs['data-noun'], s.attrs['data-value']]));
  assert.equal(drawn[`of ${counted.looked} fields`], String(counted.held));
  assert.equal(drawn.missing, String(counted.missing));
  assert.equal(drawn[`of ${parts.portableFields.length} to confirm`], String(parts.portableFields.length));
  assert.equal(drawn.delta, '1');
});

test('r4: held fields plus declared holes is every field this face looks for, on a read that answered and on one that did not', async () => {
  for (const port of [fullPort(), fullPort({ get_receipts_tid: failed() }), fullPort({ get_transformations_id: failed() })]) {
    const counted = census(toRecord(await face.read(port, DELTA_ID)));
    assert.ok(counted.looked > 0);
    assert.equal(counted.held + counted.missing, counted.looked, 'a member was neither kept nor named as a hole');
  }
});

test('r4: a delta this window could not read draws a dash in the band, never a zero', async () => {
  const { tree } = await draw(fullPort({ get_transformations_id: failed() }), DELTA_ID);
  const segment = segmentsOf(tree).find((s) => s.attrs['data-noun'] === 'delta');
  assert.ok(segment);
  assert.equal(segment.attrs['data-value'], 'unread');
  assert.ok(textOf(segment).includes('--'), 'an unread count was drawn as something other than a dash');
});

test('r4: the band carries the standing, with the hue and the mark the rest of the app spends on it', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const segment = segmentsOf(tree).find((s) => s.attrs['data-noun'] === 'delta');
  const mark = find(segment, (n) => n.tag === 'svg')[0];
  assert.ok(mark, 'the standing segment carries no mark');
  assert.equal(mark.attrs['data-mark'], 'verdict/Admit');
  const figure = findByAttr(segment, 'data-role', 'figure')[0];
  assert.match(figure.attrs.style ?? '', /color:/, 'the figure does not carry the standing\'s own ink');
});

test('r4: every group on this screen is a box, and every box head states its own count', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const boxes = boxesOf(tree);
  assert.equal(boxes.length, 5, 'the five groups this screen draws are not five boxes');
  for (const box of boxes) {
    assert.match(box.attrs['data-count'] ?? '', /^(\d+|--)$/, `${box.attrs['data-box']} states no count`);
    assert.ok(findByAttr(box, 'data-role', 'box-name')[0]);
  }
  assert.equal(boxNamed(tree, 'omitted').attrs['data-count'], String(DECLARATION.undrawn.length));
});

test('r4: the verify box head counts the claims that hold, and the count moves when one stops holding', async () => {
  const good = await draw(fullPort(), DELTA_ID);
  const bad = await draw(fullPort({ get_receipts_tid: answered(receiptBody({ digest: 'deadbeef0000' })) }), DELTA_ID);
  const held = (drawn) => Number(boxesOf(drawn.tree).find((b) => b.attrs['data-box'].startsWith('(c)')).attrs['data-count']);
  assert.ok(held(good) > 0);
  assert.equal(held(good) - held(bad), 1, 'a claim stopped holding and the head did not move');
});

test('r4: the delta box carries the standing as a filled pill in its head, so the verdict is legible without reading the row', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const head = findByAttr(boxNamed(tree, 'the delta'), 'data-role', 'box-head')[0];
  const pill = findByAttr(head, 'data-part', 'verdict-badge')[0];
  assert.ok(pill, 'the delta box head carries no standing');
  assert.equal(pill.attrs['data-filled'], 'true', 'the standing is drawn as a stroke rather than as an area');
  assert.ok(textOf(pill).includes('Admit'));
});

test('r4: an unread delta gets no pill at all -- a head with no standing rather than a neutral one', async () => {
  const { tree } = await draw(fullPort({ get_transformations_id: failed() }), DELTA_ID);
  const head = findByAttr(boxNamed(tree, 'the delta'), 'data-role', 'box-head')[0];
  assert.equal(findByAttr(head, 'data-part', 'verdict-badge').length, 0);
  assert.equal(findByAttr(head, 'data-part', 'standing-chip').length, 0);
});

test('r4: the verify box head states the seal standing beside the count, so a screen where every check holds still says it is not sealed', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const box = boxesOf(tree).find((b) => b.attrs['data-box'].startsWith('(c)'));
  const pill = findByAttr(findByAttr(box, 'data-role', 'box-head')[0], 'data-part', 'standing-chip')[0];
  assert.ok(pill, 'the verify box head states no standing');
  assert.equal(textOf(pill).includes('unsealed'), true);
  assert.equal(box.attrs['data-count'], String(findByAttr(box, 'data-holds', 'true').length));
});

test('r4: no section head carries an explanatory sentence any more, and all three are still reachable', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const words = textOf(tree);
  const reachable = find(tree, (n) => typeof n.attrs.title === 'string').map((n) => n.attrs.title).join(' | ');
  for (const sentence of [RECEIPT_MESSAGES.WHY_ADDRESS, RECEIPT_MESSAGES.WHY_BYTES, RECEIPT_MESSAGES.NOT_A_PROOF_HEADING]) {
    assert.equal(words.includes(sentence), false, `still drawn above the facts: ${sentence}`);
    assert.ok(reachable.includes(sentence), `no longer reachable at all: ${sentence}`);
  }
});

test('r4: each declared omission states its reason exactly once in what is drawn', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const words = textOf(tree);
  for (const entry of DECLARATION.undrawn) {
    assert.equal(words.split(entry.why).length - 1, 1, `the reason for "${entry.what}" is drawn more than once`);
  }
});

test('r4: the last node on the screen is the runtime footer, and its render figure was measured', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const footer = tree.children[tree.children.length - 1];
  assert.equal(footer.attrs['data-part'], 'runtime-footer');
  const ms = Number(footer.attrs['data-render-ms']);
  assert.ok(Number.isFinite(ms) && ms > 0, `the render figure is not a measurement: ${footer.attrs['data-render-ms']}`);
  assert.match(textOf(footer), /read 2 of 2 answers/);
});

test('r4: the footer says how many of the two reads answered, and says one when one did not', async () => {
  const { tree } = await draw(fullPort({ get_receipts_tid: failed() }), DELTA_ID);
  const footer = tree.children[tree.children.length - 1];
  assert.match(textOf(footer), /read 1 of 2 answers/);
});

// -- r5: the menu (Owner #348 (2)) ----------------------------------------------
//
// This face declares no acts, so the menu it offers is the copy entry and nothing
// else -- and on this screen that is the entry that matters, because the whole point
// of a receipt is that somebody takes the digest, the anchor and the id away and
// checks them somewhere that is not this window.
//
// The five properties #348 (2) asks to be pinned are each a test below: it opens, it
// acts and says whether the act worked, Escape dismisses it, a click away dismisses
// it, and a second right-click does not stack two.

/**
 * An event delivered the way a window delivers one, to the listeners mount() put on
 * the host. The shared stand-in's own press() only speaks click, and it is not this
 * lane's file to extend.
 */
function deliver(host, type, target, extra = {}) {
  let defaulted = true;
  const event = {
    type, target, preventDefault() { defaulted = false; }, ...extra,
  };
  for (const listener of [...host.listeners]) {
    if (listener.type === type) listener.handler(event);
  }
  return { defaulted };
}

const inHost = (root, predicate) => nodesOf(root).filter((n) => n.attrs && predicate(n));
const menusIn = (host) => inHost(host, (n) => n.attrs['data-part'] === 'menu');
const cellIn = (host, key) => inHost(host, (n) => n.attrs['data-cell'] === key)[0] ?? null;
const entryIn = (host) => inHost(host, (n) => n.attrs['data-entry'] === 'copy-value')[0] ?? null;

/** A host whose window has a clipboard that records what it was handed. */
function hostWithClipboard({ refuse = false } = {}) {
  const host = standInHost();
  const taken = [];
  host.ownerDocument.defaultView = {
    navigator: {
      clipboard: {
        writeText(value) {
          taken.push(value);
          return refuse ? Promise.reject(new Error('refused')) : Promise.resolve();
        },
      },
    },
  };
  host.setAttribute('data-receipt-id', DELTA_ID);
  return { host, taken };
}

async function mounted(options = {}, port = fullPort()) {
  const { host, taken } = hostWithClipboard(options);
  const unmount = mount(host, port, []);
  await unmount.ready;
  return { host, taken, unmount };
}

test('r5: a right-click on a data cell opens exactly one menu, offering the value it holds', async () => {
  const { host, unmount } = await mounted();
  const cell = inHost(host, (n) => n.attrs['data-role'] === 'kv-value' && n.attrs['data-copy'])[0];
  assert.ok(cell, 'no key/value line on this screen offers anything to take');
  const { defaulted } = deliver(host, 'contextmenu', cell);
  assert.equal(defaulted, false, 'the window own menu was left to open over ours');
  const menus = menusIn(host);
  assert.equal(menus.length, 1);
  assert.ok(textOfHost(menus[0]).includes(RECEIPT_MESSAGES.MENU_COPY));
  unmount();
});

test('r5: a second right-click does not stack two menus', async () => {
  const { host, unmount } = await mounted();
  deliver(host, 'contextmenu', cellIn(host, 'path'));
  deliver(host, 'contextmenu', cellIn(host, 'path'));
  deliver(host, 'contextmenu', cellIn(host, 'actor'));
  assert.equal(menusIn(host).length, 1, 'a menu was left behind by the one that replaced it');
  unmount();
});

test('r5: Escape dismisses the menu', async () => {
  const { host, unmount } = await mounted();
  deliver(host, 'contextmenu', cellIn(host, 'path'));
  assert.equal(menusIn(host).length, 1);
  deliver(host, 'keydown', host, { key: 'Escape' });
  assert.equal(menusIn(host).length, 0);
  unmount();
});

test('r5: a click anywhere that is not an entry dismisses the menu', async () => {
  const { host, unmount } = await mounted();
  deliver(host, 'contextmenu', cellIn(host, 'path'));
  assert.equal(menusIn(host).length, 1);
  deliver(host, 'click', inHost(host, (n) => n.attrs['data-part'] === 'stat-band')[0]);
  assert.equal(menusIn(host).length, 0);
  unmount();
});

test('r5: pressing copy hands the whole value over and says it worked', async () => {
  const { host, taken, unmount } = await mounted();
  const cell = cellIn(host, 'fingerprint');
  deliver(host, 'contextmenu', cell);
  const entry = entryIn(host);
  deliver(host, 'click', entry);
  await Promise.resolve();
  await Promise.resolve();
  // The fingerprint column draws six characters; what leaves the screen is the digest.
  assert.deepEqual(taken, [delta().digest]);
  assert.ok(taken[0].length > 6, 'the whole value is no longer than the drawn cut, so this test proves nothing');
  assert.equal(entry.getAttribute('data-copied'), 'true');
  assert.equal(entry.getAttribute('data-copy-said'), RECEIPT_MESSAGES.MENU_COPIED);
  // Drawn, not only recorded: an outcome that lives in an attribute is a control that
  // looks the same whether or not it did anything.
  assert.ok(textOfHost(entry).includes(RECEIPT_MESSAGES.MENU_COPIED), 'the entry does not say on its face that it worked');
  unmount();
});

test('r5: a clipboard that refuses is reported, not pretended past', async () => {
  const { host, unmount } = await mounted({ refuse: true });
  deliver(host, 'contextmenu', cellIn(host, 'path'));
  const entry = entryIn(host);
  deliver(host, 'click', entry);
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(entry.getAttribute('data-copied'), 'false');
  assert.equal(entry.getAttribute('data-copy-failed'), 'true');
  assert.equal(entry.getAttribute('data-copy-said'), RECEIPT_MESSAGES.MENU_REFUSED);
  assert.ok(textOfHost(entry).includes(RECEIPT_MESSAGES.MENU_REFUSED), 'the entry does not say on its face that it failed');
  unmount();
});

test('r5: a window with no clipboard at all is reported the same way', async () => {
  const host = standInHost();
  host.setAttribute('data-receipt-id', DELTA_ID);
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  deliver(host, 'contextmenu', cellIn(host, 'path'));
  const entry = entryIn(host);
  deliver(host, 'click', entry);
  assert.equal(entry.getAttribute('data-copy-failed'), 'true');
  assert.equal(entry.getAttribute('data-copy-said'), RECEIPT_MESSAGES.MENU_NO_CLIPBOARD);
  unmount();
});

test('r5: a cell that drew a declared hole offers the entry disabled, with the hole own reason', async () => {
  const { host, unmount } = await mounted({}, fullPort({
    get_transformations_id: answered(delta({ path: undefined })),
  }));
  const hole = inHost(host, (n) => n.attrs['data-cell'] === 'path' && n.attrs['data-state'] === 'hole')[0];
  assert.ok(hole, 'the row drew no hole, so this test is measuring nothing');
  deliver(host, 'contextmenu', hole);
  const entry = entryIn(host);
  assert.equal(entry.getAttribute('data-enabled'), 'false');
  assert.equal(entry.getAttribute('disabled'), '');
  assert.ok(textOfHost(entry).length > RECEIPT_MESSAGES.MENU_COPY.length, 'a disabled entry with no reason drawn on it');
  deliver(host, 'click', entry);
  assert.equal(entry.getAttribute('data-copied'), null, 'a disabled entry copied something');
  unmount();
});

test('r5: a right-click on something that is not a cell leaves the window its own menu', async () => {
  const { host, unmount } = await mounted();
  const footer = inHost(host, (n) => n.attrs['data-part'] === 'runtime-footer')[0];
  const { defaulted } = deliver(host, 'contextmenu', footer);
  assert.equal(defaulted, true, 'this face took a menu away where it had nothing to offer');
  assert.equal(menusIn(host).length, 0);
  unmount();
});

test('r5: unmounting with a menu open leaves nothing behind and stops listening', async () => {
  const { host, unmount } = await mounted();
  deliver(host, 'contextmenu', cellIn(host, 'path'));
  assert.equal(menusIn(host).length, 1);
  unmount();
  assert.equal(host.childNodes.length, 0);
  assert.equal(host.listeners.length, 0, 'the face is gone and its listeners are still on the host');
});

test('r5: the id is on the screen and is a cell the menu can hand over', async () => {
  const { host, taken, unmount } = await mounted();
  const subject = inHost(host, (n) => n.attrs['data-role'] === 'subject')[0];
  assert.ok(subject, 'the screen still does not say which delta it is about');
  assert.equal(subject.getAttribute('data-copy'), DELTA_ID);
  deliver(host, 'contextmenu', subject);
  deliver(host, 'click', entryIn(host));
  await Promise.resolve();
  assert.deepEqual(taken, [DELTA_ID]);
  unmount();
});

test('r5: both legend tables are drawn on one column grid, so they cannot line up differently', async () => {
  const { tree } = await draw(fullPort(), DELTA_ID);
  const grids = [
    ...find(tree, (n) => 'data-mark-entry' in n.attrs),
    ...find(tree, (n) => 'data-not-drawn' in n.attrs),
  ].map((n) => /grid-template-columns:([^;]+)/.exec(n.attrs.style)?.[1]);
  assert.ok(grids.length >= 16, `too few legend rows to compare: ${grids.length}`);
  assert.equal(new Set(grids).size, 1, `the two legend tables declare ${new Set(grids).size} different column grids`);
  // The width itself was chosen by measuring a real renderer, not by counting
  // characters -- record/interaction-pass.json's "legend:measure the mark column" act
  // is the instrument, and it reported two names overflowing at the previous value.
  // What this asserts is the thing a unit test can actually know: that every mark name
  // this declaration holds is drawn on one line and shares one track with the other
  // table. A name long enough to need a wider column fails in that reading, not here.
  const longest = Math.max(...DECLARATION.marks.map((m) => m.mark.length));
  assert.ok(longest <= 19, `a mark name grew past the width the renderer was measured at: ${longest}`);
});

test('r5: every cell that offers a value offers the record own value, never a drawn shortening', async () => {
  const { tree, state } = await draw(fullPort(), DELTA_ID);
  const record = toRecord(state);
  const offered = find(tree, (n) => typeof n.attrs['data-copy'] === 'string');
  assert.ok(offered.length >= 9, `too few cells offer anything: ${offered.length}`);
  const held = new Set([
    record.id, record.at, record.actor, record.effect, record.verdict,
    record.path, record.digest, record.receiptDigest, record.algorithm, record.anchor,
  ].filter((v) => typeof v === 'string'));
  for (const node of offered) {
    assert.ok(held.has(node.attrs['data-copy']), `a cell offers something no member of the record holds: ${node.attrs['data-copy']}`);
  }
});
