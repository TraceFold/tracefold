// SPDX-License-Identifier: Apache-2.0
// The gates, run against the shipped source, and then run against source written to
// break them. A gate that has never gone red is a gate nobody has seen work.

import test from 'node:test';
import assert from 'node:assert/strict';

import { CHECKS, checkSource, report, shippedSources } from '../tools/gate.mjs';
import { DECLARATION } from '../declaration.mjs';
import { face } from '../receipt.mjs';
import { stubPort, answered, SAMPLE } from '../../ledger/test/stub-port.mjs';

const DELTA_ID = 't-001';
const delta = () => SAMPLE.transformation(1);
const receiptBody = (extra = {}) => ({
  digest: delta().digest, algorithm: 'sha256', anchor: 'https://example.test/anchor/t-001', basis: 'exact', ...extra,
});

const port = () => stubPort({
  get_transformations_id: answered(delta()),
  get_receipts_tid: answered(receiptBody()),
}, { methods: DECLARATION.consumes });

test('the shipped face passes every source gate', async () => {
  const state = await face.read(port(), DELTA_ID);
  const result = report({ trees: [face.view(state)] });
  const failing = result.checks.filter((c) => !c.holds);
  assert.deepEqual(failing.map((c) => `${c.id}: ${c.detail}`), []);
  assert.ok(result.checks.length >= 13, 'too few checks to call this a gate');
});

test('the gate reads a non-empty set of shipped files', () => {
  const sources = shippedSources();
  assert.ok(sources.length >= 4);
  for (const source of sources) assert.ok(source.text.length > 200, `${source.file} is suspiciously small`);
});

const PLANTS = [
  ['no-network', "const r = await fetch('http://127.0.0.1:8787/v1/transformations');"],
  ['no-foreign-import', "import { createMembrane } from '../../membrane/src/index.mjs';"],
  ['no-verification', 'const ok = verify(record.digest, record.anchor);'],
  ['no-actor-named', "const body = { actor: { Human: { id: 'me' } } };"],
  ['no-colour-literal', "const ink = '#1a1a1a';"],
  ['no-borrowed-symbol', "const bullet = '●';"],
  ['nothing-out-of-flow', "const s = 'position:absolute;left:0';"],
  ['no-method-literals-outside-the-declaration', "await port.get_transformations_id('get_transformations_id');"],
  ['no-dynamic-code', 'const f = new Function("return 1");'],
  ['rows-are-not-edited', 'record.verdict = "Admit";'],
  ['no-boolean-sealed-claim', 'const shown = receiptBody.sealed ? "sealed" : "unsealed";'],
  // Owner #348 (5). The first two of these went red against this face's own shipped
  // source before the r5 pass touched it -- `git show 3bae9b7:faces/receipt/receipt.mjs`
  // carries `'border-radius': '4px'` and spells font-weight three times -- and the
  // plants keep them red once the real source is clean.
  ['no-raw-corner', "const s = style({ 'border-radius': '4px' });"],
  ['no-raw-motion', "const s = style({ transition: 'background 120ms ease' });"],
];

for (const [id, planted] of PLANTS) {
  test(`negative control: ${id} goes red on planted source`, () => {
    const check = CHECKS.find((c) => c.id === id);
    assert.ok(check, `no such check: ${id}`);
    const result = checkSource(check, [{ file: 'planted.mjs', text: planted }]);
    assert.equal(result.holds, false, `${id} did not notice: ${planted}`);
  });
}

test('negative control: a second font-weight goes red, and the shipped one does not', () => {
  const check = CHECKS.find((c) => c.id === 'weight-is-spelled-once');
  const once = checkSource(check, [{ file: 'planted.mjs', text: "const weight = (role) => ({ 'font-weight': WEIGHTS[role] });" }]);
  assert.equal(once.holds, true, 'the one place a face is allowed to spell it must pass');
  const twice = checkSource(check, [{
    file: 'planted.mjs',
    text: "const weight = (role) => ({ 'font-weight': WEIGHTS[role] });\nconst head = style({ 'font-weight': '600' });",
  }]);
  assert.equal(twice.holds, false, 'a weight typed at a call site is exactly the drift this counts');
});

test('the corner and motion gates do not fire on the tokens they exist to force', () => {
  const corner = CHECKS.find((c) => c.id === 'no-raw-corner');
  assert.equal(checkSource(corner, [{ file: 'planted.mjs', text: "style({ 'border-radius': T.radiusControl })" }]).holds, true);
  const motion = CHECKS.find((c) => c.id === 'no-raw-motion');
  assert.equal(checkSource(motion, [{ file: 'planted.mjs', text: "el('button', { class: 'gx-move' }, [])" }]).holds, true);
});

test('the sealed-claim check does not fire on the legitimate claim.sealed read', () => {
  const check = CHECKS.find((c) => c.id === 'no-boolean-sealed-claim');
  const result = checkSource(check, [{ file: 'planted.mjs', text: "const shown = claim.sealed ? 'sealed' : 'unsealed';" }]);
  assert.equal(result.holds, true, 'reading claimOf()\'s own decided answer is exactly what render is supposed to do');
});

test('negative control: an undeclared mark on screen goes red', async () => {
  const state = await face.read(port(), DELTA_ID);
  const tree = face.view(state);
  const planted = { tag: 'svg', attrs: { 'data-mark': 'weather/rain', 'data-means': 'weather.rain' }, children: [] };
  tree.children.push(planted);
  const result = report({ trees: [tree] });
  const marks = result.checks.find((c) => c.id === 'declared-marks-only');
  assert.equal(marks.holds, false);
});

test('negative control: one meaning with two marks goes red', async () => {
  const state = await face.read(port(), DELTA_ID);
  const tree = face.view(state);
  // 'verdict.admit' is the meaning the delta row already draws (this sample's
  // verdict is Admit) -- planting a second mark for that same meaning is what makes
  // this a real collision rather than a lone, uncontested entry.
  tree.children.push({ tag: 'svg', attrs: { 'data-mark': 'structure/hole', 'data-means': 'verdict.admit' }, children: [] });
  const result = report({ trees: [tree] });
  const single = result.checks.find((c) => c.id === 'one-meaning-one-mark');
  assert.equal(single.holds, false);
});

test('negative control: the seal row and the claim line disagreeing goes red', async () => {
  const state = await face.read(port(), DELTA_ID);
  const tree = face.view(state);
  // The shipped tree agrees (both read off the one claimOf() call): plant a second,
  // contradicting seal-cell glyph so the two readings actually disagree.
  tree.children.push({
    tag: 'span',
    attrs: { 'data-cell': 'seal' },
    children: [{ tag: 'svg', attrs: { 'data-mark': 'structure/seal', 'data-means': 'structure.seal' }, children: [] }],
  });
  const result = report({ trees: [tree] });
  const sealGate = result.checks.find((c) => c.id === 'seal-claim-mark-matches-standing');
  // The shipped claim in this fixture is unsealed (no verifier present); a planted
  // structure/seal cell now disagrees with the printed "seal claim: unsealed" line.
  assert.equal(sealGate.holds, false, 'the seal-claim-mark-matches-standing gate did not notice a planted disagreement');
});

test('the seal-claim-mark-matches-standing gate refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const sealGate = result.checks.find((c) => c.id === 'seal-claim-mark-matches-standing');
  assert.equal(sealGate.holds, false, 'a rule that passes when nothing was drawn is not a rule');
});

test('negative control: a sentence that breaks mid-word goes red', async () => {
  const state = await face.read(port(), DELTA_ID);
  const tree = face.view(state);
  tree.children.push({
    tag: 'p',
    attrs: { 'data-role': 'planted', style: 'color:red;overflow-wrap:anywhere' },
    children: [{ text: 'a sentence has spaces to break at and must not be split down the middle of a word' }],
  });
  const result = report({ trees: [tree] });
  const breaks = result.checks.find((c) => c.id === 'only-opaque-values-break-mid-word');
  assert.equal(breaks.holds, false, 'a prose node reaching for anywhere is the whole of what this rule forbids');
});

test('the break rule refuses to pass on an empty population, and its exception cannot go stale', () => {
  const empty = report({ trees: [] }).checks.find((c) => c.id === 'only-opaque-values-break-mid-word');
  assert.equal(empty.holds, false, 'a rule that passes when nothing was drawn is not a rule');
  // The named exception is a defect in a file this lane may not edit. When that file is
  // fixed the exception stops matching anything, and this gate says so rather than
  // carrying an excuse for a defect that is gone.
  const opaqueOnly = {
    tag: 'div',
    attrs: { 'data-text': 'opaque', style: 'overflow-wrap:anywhere' },
    children: [{ text: 'a1b2c3d4e5f60001' }],
  };
  const stale = report({ trees: [opaqueOnly] }).checks.find((c) => c.id === 'only-opaque-values-break-mid-word');
  assert.equal(stale.holds, false, 'an exception nothing matches any more is a stale excuse');
  assert.match(stale.detail, /stale exceptions: receipt-note/);
});

test('negative control: a copy offering the drawn shortening rather than the whole value goes red', async () => {
  const state = await face.read(port(), DELTA_ID);
  const tree = face.view(state);
  // The fingerprint cell draws six characters and hands over the whole digest. A cell
  // that claims to be a shortening while offering no more than it draws is the trap
  // this rule exists for: somebody takes six characters of a digest away and believes
  // they have the digest.
  tree.children.push({
    tag: 'span',
    attrs: { 'data-cell': 'fingerprint', 'data-state': 'value', 'data-copy': 'A1B2C3', 'data-copy-whole': 'true' },
    children: [{ text: 'A1B2C3' }],
  });
  const result = report({ trees: [tree] });
  const copy = result.checks.find((c) => c.id === 'copy-hands-over-the-whole-value');
  assert.equal(copy.holds, false, 'the copy gate did not notice a shortening dressed as a whole value');
});

test('the copy rule refuses to pass on an empty population', () => {
  const copy = report({ trees: [] }).checks.find((c) => c.id === 'copy-hands-over-the-whole-value');
  assert.equal(copy.holds, false, 'a rule that passes when nothing was drawn is not a rule');
});

test('negative control: a named test that is gone goes red', () => {
  const result = report({ trees: [], declaration: { ...DECLARATION, tests: [...DECLARATION.tests, 'test/vanished.mjs'] } });
  const named = result.checks.find((c) => c.id === 'named-tests-exist');
  assert.equal(named.holds, false);
});

test('the mark check refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const marks = result.checks.find((c) => c.id === 'declared-marks-only');
  assert.equal(marks.holds, false, 'a rule that passes when nothing was drawn is not a rule');
});
