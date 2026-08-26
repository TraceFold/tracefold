// SPDX-License-Identifier: Apache-2.0
// The gates, run against the shipped source, and then run against source written to
// break them. A gate that has never gone red is a gate nobody has seen work.

import test from 'node:test';
import assert from 'node:assert/strict';

import { CHECKS, checkSource, report, shippedSources } from '../tools/gate.mjs';
import { DECLARATION } from '../declaration.mjs';
import { face } from '../held.mjs';
import { stubPort, page, SAMPLE } from '../../ledger/test/stub-port.mjs';

const port = () => stubPort({
  get_candidates: page([SAMPLE.candidate(1), SAMPLE.candidate(2)]),
}, { methods: DECLARATION.consumes });

test('the shipped face passes every source gate', async () => {
  const state = await face.read(port());
  // Two states, because two of the tree rules are about what a whole screen holds
  // when a reader has opened a menu on it, and both refuse to pass on a population
  // where nobody ever right-clicked anything.
  const chosen = state.held.items[0].id;
  const trees = [
    face.view(state),
    face.view({
      ...state,
      selected: chosen,
      menu: {
        at: `row:${chosen}`, subject: chosen, value: '/work/contract.pdf', copy: null,
      },
    }),
  ];
  const result = report({ trees });
  const failing = result.checks.filter((c) => !c.holds);
  assert.deepEqual(failing.map((c) => `${c.id}: ${c.detail}`), []);
  assert.ok(result.checks.length >= 12, 'too few checks to call this a gate');
});

test('the gate reads a non-empty set of shipped files', () => {
  const sources = shippedSources();
  assert.ok(sources.length >= 4);
  for (const source of sources) assert.ok(source.text.length > 200, `${source.file} is suspiciously small`);
});

const PLANTS = [
  ['no-network', "const r = await fetch('http://127.0.0.1:8787/v1/candidates');"],
  ['no-foreign-import', "import { createMembrane } from '../../membrane/src/index.mjs';"],
  ['no-verification', 'const ok = verify(record.digest, record.anchor);'],
  ['no-actor-named', "const body = { actor: { Human: { id: 'me' } } };"],
  ['no-colour-literal', "const ink = '#1a1a1a';"],
  ['no-borrowed-symbol', "const bullet = '●';"],
  ['nothing-out-of-flow', "const s = 'position:absolute;left:0';"],
  ['no-method-literals-outside-the-declaration', "await port.fold('get_candidates');"],
  ['no-dynamic-code', 'const f = new Function("return 1");'],
  ['rows-are-not-edited', 'record.verdict = "Admit";'],
  ['no-unconditional-seal', "el('span', { 'data-cell': 'seal', 'data-state': 'value' }, [])"],
  ['no-face-motion', "style: style({ transition: 'opacity 120ms ease' })"],
  ['no-face-motion', "const settle = { duration: 180ms };"],
  ['no-raw-corner', "style: style({ 'border-radius': '4px' })"],
  ['no-raw-weight', "style: style({ 'font-weight': '700' })"],
  ['no-mid-word-break', "style: style({ 'overflow-wrap': 'anywhere' })"],
];

for (const [id, planted] of PLANTS) {
  test(`negative control: ${id} goes red on planted source -- ${planted.slice(0, 34)}`, () => {
    const check = CHECKS.find((c) => c.id === id);
    assert.ok(check, `no such check: ${id}`);
    const result = checkSource(check, [{ file: 'planted.mjs', text: planted }]);
    assert.equal(result.holds, false, `${id} did not notice: ${planted}`);
  });
}

test('negative control: an undeclared mark on screen goes red', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  const planted = { tag: 'svg', attrs: { 'data-mark': 'weather/rain', 'data-means': 'weather.rain' }, children: [] };
  tree.children.push(planted);
  const result = report({ trees: [tree] });
  const marks = result.checks.find((c) => c.id === 'declared-marks-only');
  assert.equal(marks.holds, false);
});

test('negative control: one meaning with two marks goes red', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  // 'standing.held' is the meaning every row on this screen already draws
  // (lifecycleCell in receipt-row.mjs, since every held record carries
  // lifecycle:'held') -- planting a second mark for that same meaning is what
  // makes this a real collision rather than a lone, uncontested entry.
  tree.children.push({ tag: 'svg', attrs: { 'data-mark': 'structure/hole', 'data-means': 'standing.held' }, children: [] });
  const result = report({ trees: [tree] });
  const single = result.checks.find((c) => c.id === 'one-meaning-one-mark');
  assert.equal(single.holds, false);
});

test('negative control: a seal cell drawn as a value (not a hole) goes red', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  tree.children.push({ tag: 'span', attrs: { 'data-cell': 'seal', 'data-state': 'value' }, children: [] });
  const result = report({ trees: [tree] });
  const sealGate = result.checks.find((c) => c.id === 'seal-column-is-always-a-hole');
  assert.equal(sealGate.holds, false, 'the seal-is-always-a-hole gate did not notice a planted sealed cell');
});

test('the seal gate refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const sealGate = result.checks.find((c) => c.id === 'seal-column-is-always-a-hole');
  assert.equal(sealGate.holds, false, 'a rule that passes when nothing was drawn is not a rule');
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

// -- Owner #348 (2): the two rules about a whole screen holding a menu -------------

const withMenu = async (menu) => {
  const state = await face.read(port());
  return face.view({ ...state, selected: menu.subject, menu });
};

const MENU = Object.freeze({
  at: 'row:c-001', subject: 'c-001', value: '/work/contract.pdf', copy: null,
});

test('negative control: a screen carrying two menus goes red', async () => {
  const tree = await withMenu(MENU);
  const drawn = tree.children.find((child) => child.attrs && child.attrs['data-menu']) ?? null;
  // A second menu node planted at the top level is exactly the shape a handler that
  // appended rather than replaced would produce.
  tree.children.push({ tag: 'div', attrs: { 'data-part': 'row-menu', 'data-menu': 'row:c-002' }, children: [] });
  assert.equal(drawn ?? null, null, 'the menu is drawn inside the row it belongs to, not at the top of the screen');
  const result = report({ trees: [tree] });
  const single = result.checks.find((c) => c.id === 'one-menu-at-most');
  assert.equal(single.holds, false, 'two menus on one screen was not noticed');
});

test('the one-menu rule refuses to pass on a screen nobody right-clicked', async () => {
  const state = await face.read(port());
  const result = report({ trees: [face.view(state)] });
  const single = result.checks.find((c) => c.id === 'one-menu-at-most');
  assert.equal(single.holds, false, 'a rule that passes when no menu was drawn is not a rule');
});

test('negative control: a menu offering an act this face never declared goes red', async () => {
  const tree = await withMenu(MENU);
  tree.children.push({
    tag: 'button',
    attrs: {
      'data-role': 'menu-act', 'data-act': 'purge', 'data-state': 'open', 'data-target': 'c-001',
    },
    children: [],
  });
  const result = report({ trees: [tree] });
  const offers = result.checks.find((c) => c.id === 'the-menu-offers-what-the-row-offers');
  assert.equal(offers.holds, false, 'an invented verb in the menu was not noticed');
});

test('negative control: a menu offer whose gate is shut but which still names a row goes red', async () => {
  const tree = await withMenu(MENU);
  tree.children.push({
    tag: 'button',
    attrs: {
      'data-role': 'menu-act', 'data-act': 'undo', 'data-state': 'shut', 'data-target': 'c-001',
    },
    children: [],
  });
  const result = report({ trees: [tree] });
  const offers = result.checks.find((c) => c.id === 'the-menu-offers-what-the-row-offers');
  assert.equal(offers.holds, false, 'a dead offer one stray press from sending was not noticed');
});
