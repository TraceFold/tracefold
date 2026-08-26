// SPDX-License-Identifier: Apache-2.0
// What the chrome offers on a right-click, asked without a window.
//
// The property that matters here is not "a menu appears". It is that every row either
// acts or says why it cannot, and that no row names a verb the act registry does not
// hold -- a menu item that reads well and does nothing is the defect r4 found on the
// faces, and this file is what stops it landing on the chrome instead.

import test from 'node:test';
import assert from 'node:assert/strict';

import { menuFor, MENU_TARGETS, MENU_SAID } from '../kernel/menu.mjs';
import { ACTS } from '../kernel/acts.mjs';

const CONTEXTS = Object.freeze({
  standing: { id: 'held', title: 'Held', index: 0, stands: false, region: 'nowhere', place: 'right', path: [], address: null },
  tab: { index: 0, path: [], at: 0, id: 'atlas', active: 1 },
  dock: { index: 0, side: 'right', at: 0, id: 'held', size: 320 },
  strip: { digest: 'abcd1234', suite: 'suite: 1040 tests / 0 failed' },
  sash: { index: 0, side: 'right', size: 320 },
});

test('every chrome surface named as a target answers with rows', () => {
  for (const target of MENU_TARGETS) {
    const rows = menuFor(target, CONTEXTS[target]);
    assert.ok(rows.length > 0, `${target} offers nothing`);
  }
  assert.throws(() => menuFor('nowhere-in-particular', {}), /no chrome menu/);
});

test('a row acts, or copies, or says why it cannot -- and never two of the three', () => {
  for (const target of MENU_TARGETS) {
    for (const row of menuFor(target, CONTEXTS[target])) {
      const offers = [row.act, row.copy, row.why].filter((held) => held !== null && held !== undefined);
      assert.equal(offers.length, 1, `${target}/${row.label} offers ${offers.length} things`);
      assert.ok(row.label.length > 3, `${target} has a row with no label`);
    }
  }
});

test('every verb a menu offers exists in the act registry', () => {
  for (const target of MENU_TARGETS) {
    for (const row of menuFor(target, CONTEXTS[target])) {
      if (!row.act) continue;
      assert.ok(row.act.verb in ACTS, `${target}/${row.label} offers "${row.act.verb}", which no act registers`);
    }
  }
});

test('a face that already stands somewhere is refused with the place it stands in', () => {
  const rows = menuFor('standing', { ...CONTEXTS.standing, stands: true, region: 'the right dock' });
  const place = rows.find((row) => row.label.startsWith('place'));
  assert.equal(place.act, null);
  assert.equal(place.why, MENU_SAID.alreadyStanding('Held', 'the right dock'));
  // The reason names the place. A refusal that says only "no" is a refusal a reader has
  // to guess at, which is the thing req/811 §8-7 is about.
  assert.match(place.why, /right dock/);
});

test('a dock already at a declared end refuses that end and offers the other', () => {
  const atLargest = menuFor('sash', { index: 0, side: 'right', size: 640 });
  const widen = atLargest.find((row) => row.label.startsWith('widen'));
  const narrow = atLargest.find((row) => row.label.startsWith('narrow'));
  assert.ok(widen.why || widen.act, 'widen neither acts nor says why');
  assert.ok(narrow.act, 'a dock at its largest can still be narrowed');
});

test('red-first: the tab menu refuses to send you to the tab you are already on', () => {
  const here = menuFor('tab', { ...CONTEXTS.tab, active: 0 });
  const go = here.find((row) => row.label === 'go to this tab');
  assert.equal(go.act, null);
  assert.match(go.why, /already/);
  // And offers it when you are not.
  const elsewhere = menuFor('tab', { ...CONTEXTS.tab, active: 1 });
  assert.equal(elsewhere.find((row) => row.label === 'go to this tab').act.verb, 'tab:go');
});

test('the strip refuses to copy a reading it has not got', () => {
  const bare = menuFor('strip', { digest: null, suite: null });
  for (const row of bare) {
    assert.equal(row.copy, null);
    assert.ok(row.why.length > 10, 'a refusal with a short reason is a refusal with no reason');
  }
});
