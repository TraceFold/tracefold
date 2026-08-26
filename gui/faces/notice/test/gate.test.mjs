// SPDX-License-Identifier: Apache-2.0
// The gates, run against the shipped source, and then run against source written to
// break them. A gate that has never gone red is a gate nobody has seen work.

import test from 'node:test';
import assert from 'node:assert/strict';

import { CHECKS, checkSource, report, shippedSources } from '../tools/gate.mjs';
import { STATES } from '../tools/fixture.mjs';
import { DECLARATION } from '../declaration.mjs';
import { face } from '../notice.mjs';
import { representative } from './sample-notices.mjs';

/** Every state the gate is fired at, including the one no shipped page photographs:
 * a row whose menu is open. The overlay rule refuses an empty population, and a
 * screen nobody right-clicked is an empty population for it. */
function everyDrawnState() {
  return [
    ...Object.values(STATES).map((notices) => face.view(face.read(notices))),
    face.view(face.read(STATES.notice, [], { menu: { entry: '3', x: 40, y: 80 } })),
  ];
}

test('the shipped face passes every source gate', () => {
  const result = report({ trees: everyDrawnState() });
  const failing = result.checks.filter((c) => !c.holds);
  assert.deepEqual(failing.map((c) => `${c.id}: ${c.detail}`), []);
  assert.ok(result.checks.length >= 18, 'too few checks to call this a gate');
});

test('the tree checks are fired at every state the shipped pages draw, not only the one this file happened to build', () => {
  // The retrofit lane ran tools/gate.mjs from a command line for the first time and
  // the one-meaning-one-mark check went red on a state this file never drew (the
  // overflow page, the only one carrying a grouped run and therefore the only one
  // carrying a standing chip). A gate fired at one state is a gate that has been
  // tested on one state, so this fires it at all of them.
  const states = Object.values(STATES);
  assert.ok(states.length >= 3, `only ${states.length} states to draw`);
  const result = report({ trees: everyDrawnState() });
  assert.deepEqual(result.failing.map((c) => `${c.id}: ${c.detail}`), []);
});

test('the gate reads a non-empty set of shipped files', () => {
  const sources = shippedSources();
  assert.ok(sources.length >= 4);
  for (const source of sources) assert.ok(source.text.length > 100, `${source.file} is suspiciously small`);
});

const PLANTS = [
  ['no-network', "const r = await fetch('http://127.0.0.1:8787/v1/candidates');"],
  ['no-foreign-import', "import { createMembrane } from '../../membrane/src/index.mjs';"],
  ['no-verification', 'const ok = verify(record.digest, record.anchor);'],
  ['no-actor-named', "const body = { actor: { Human: { id: 'me' } } };"],
  ['no-colour-literal', "const ink = '#1a1a1a';"],
  ['no-borrowed-symbol', "const bullet = '●';"],
  ['nothing-out-of-flow', "const s = 'position:absolute;left:0';"],
  // The spelling this codebase actually uses, which the rule could not see until
  // 2026-08-25: a style object with a quoted value. `absolute` is never allowed, so
  // one line of it is red on its own.
  ['nothing-out-of-flow', "style({ position: 'absolute', left: 0 })"],
  ['nothing-out-of-flow', "style({ position: 'sticky', top: 0 })"],
  ['no-raw-transition', "style({ transition: 'background-color 120ms ease' })"],
  ['no-raw-transition', "const settle = '180ms';"],
  ['no-raw-corner', "style({ 'border-radius': '4px' })"],
  ['no-hand-picked-mark-size', "P.glyph('structure', 'hole', { size: 15 })"],
  ['no-method-literals-outside-the-declaration', "await port.fold('get_transformations');"],
  ['no-dynamic-code', 'const f = new Function("return 1");'],
  ['entries-are-not-edited', 'record.outcome = "tampered";'],
];

for (const [id, planted] of PLANTS) {
  test(`negative control: ${id} goes red on ${JSON.stringify(planted.slice(0, 40))}`, () => {
    const check = CHECKS.find((c) => c.id === id);
    assert.ok(check, `no such check: ${id}`);
    const result = checkSource(check, [{ file: 'planted.mjs', text: planted }]);
    assert.equal(result.holds, false, `${id} did not notice: ${planted}`);
  });
}

test('the one positioned node is an allowance with a number, not a licence: a second one goes red', () => {
  const check = CHECKS.find((c) => c.id === 'nothing-out-of-flow');
  const one = checkSource(check, [{ file: 'planted.mjs', text: "style({ position: 'fixed' })" }]);
  assert.equal(one.holds, true, 'the one overlay this face draws is refused');
  assert.match(one.detail, /1\/1 allowed/);
  const two = checkSource(check, [{ file: 'planted.mjs', text: "style({ position: 'fixed' })\nstyle({ position: 'fixed' })" }]);
  assert.equal(two.holds, false, 'two overlays passed, so the bound is not a bound');
  assert.match(two.detail, /2 of an allowed 1/);
});

test('negative control: a positioned node that is not the menu goes red on the drawn tree', () => {
  const tree = face.view(face.read(STATES.notice, [], { menu: { entry: '3', x: 10, y: 10 } }));
  tree.children.push({ tag: 'div', attrs: { 'data-role': 'entry', style: "position:'fixed';left:0" }, children: [] });
  const overlay = report({ trees: [tree] }).checks.find((c) => c.id === 'one-overlay-and-it-is-the-menu');
  assert.equal(overlay.holds, false, 'a second positioned node, on a row, was waved through');
});

test('the overlay rule refuses a population nobody right-clicked', () => {
  const trees = Object.values(STATES).map((notices) => face.view(face.read(notices)));
  const overlay = report({ trees }).checks.find((c) => c.id === 'one-overlay-and-it-is-the-menu');
  assert.equal(overlay.holds, false, 'a rule that passes when no menu was drawn has not been applied to anything');
  assert.match(overlay.detail, /empty population/);
});

test('negative control: an undeclared mark on screen goes red', () => {
  const state = face.read(representative());
  const tree = face.view(state);
  const planted = { tag: 'svg', attrs: { 'data-mark': 'weather/rain', 'data-means': 'weather.rain' }, children: [] };
  tree.children.push(planted);
  const result = report({ trees: [tree] });
  const marks = result.checks.find((c) => c.id === 'declared-marks-only');
  assert.equal(marks.holds, false);
});

test('negative control: one meaning with two marks goes red', () => {
  const state = face.read(representative());
  const tree = face.view(state);
  tree.children.push({ tag: 'svg', attrs: { 'data-mark': 'structure/hole', 'data-means': 'mark.undefined' }, children: [] });
  const result = report({ trees: [tree] });
  const single = result.checks.find((c) => c.id === 'one-meaning-one-mark');
  assert.equal(single.holds, false);
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
