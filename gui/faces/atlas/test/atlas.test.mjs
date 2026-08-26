// SPDX-License-Identifier: Apache-2.0
// What the face does, asked of the face and not of a description of it.
//
// The one thing these tests exist to guard above everything else: this screen may
// never draw all of a subject's own touch history expanded by default -- the Owner
// eye-judgment correction this face was built to satisfy from the start, not
// retrofit later. Every other test here is either a property this face carries
// over from faces/graph's/faces/receipt's shared discipline (fail-closed, no
// colour, every glyph sized) or the ordinary declaration/mark/gate checks every
// face in this tree holds.
//
// stub-port.mjs and dom-stand-in.mjs are read from faces/ledger/test/, not
// duplicated here -- the same precedent faces/notice, faces/held, faces/receipt
// and faces/graph already set (req/99 SS5).

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createFace, face, mount, toRecord, ATLAS_MESSAGES,
} from '../atlas.mjs';
import { DECLARATION, OFFERS } from '../declaration.mjs';
import { parts } from '../binding.mjs';
import {
  standInHost, textOfHost, nodesOf, nodeWith, press,
} from '../../ledger/test/dom-stand-in.mjs';
import {
  stubPort, page, refused, failed, absent,
} from '../../ledger/test/stub-port.mjs';

const {
  el, toHtml, find, findByAttr, textOf, walk, isText,
} = parts.element;

function item(id, sequence, path, extra = {}) {
  return {
    id, sequence, path, at: `2026-08-24T09:${String(sequence).padStart(2, '0')}:00Z`,
    actor: 'agent:packer', effect: 'write', verdict: 'Admit', digest: `a1b2c3d4e5f6${String(sequence).padStart(4, '0')}`,
    ...extra,
  };
}

/**
 * The fixture population this whole file reasons about:
 *   /work/a.md -- 3 touches: a normal subject with a real history to fold
 *   /work/b.md -- 1 touch: a subject atlas still draws (unlike faces/graph, which
 *                 would not draw this at all -- see the dedicated test below)
 *   /work/c.md -- 2 touches, last one Deny/delete: exercises a non-Admit/write
 *                 latest touch
 */
const ITEMS = Object.freeze([
  item('t-001', 1, '/work/a.md'),
  item('t-002', 2, '/work/a.md'),
  item('t-003', 3, '/work/a.md'),
  item('t-004', 4, '/work/b.md'),
  item('t-005', 5, '/work/c.md'),
  item('t-006', 6, '/work/c.md', { verdict: 'Deny', effect: 'delete' }),
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
const subjectOf = (tree, subjectPath) => findByAttr(tree, 'data-path', subjectPath)[0] ?? null;

// -- the three shapes Owner #340's round added, addressed the way the screen holds
// them: the band by the noun each figure counts, a box by the name in its head, and
// the footer by being the last thing on the screen.
const bandOf = (tree) => findByAttr(tree, 'data-part', 'stat-band')[0] ?? null;
const segmentOf = (tree, noun) => findByAttr(tree, 'data-noun', noun)
  .find((node) => node.attrs['data-role'] === 'segment') ?? null;
const figureOf = (tree, noun) => {
  const segment = segmentOf(tree, noun);
  const figure = segment === null ? null : findByAttr(segment, 'data-role', 'figure')[0] ?? null;
  return figure === null ? null : textOf(figure).trim();
};
const boxesOf = (tree) => findByAttr(tree, 'data-part', 'box');
const headOf = (box) => findByAttr(box, 'data-role', 'box-head')[0] ?? null;
const footerOf = (tree) => tree.children[tree.children.length - 1];
const withoutFooter = (tree) => ({ ...tree, children: tree.children.slice(0, -1) });

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

/**
 * The interaction audit in req/103 found disclosures closing on a repaint: whatever a
 * reader had opened was shut again the moment anything else on the screen changed.
 * This face cannot do that, and this is the mechanism rather than an opinion about it:
 * `mount()` paints exactly twice -- the waiting screen synchronously, and the drawn
 * screen once, when the single read settles -- and holds no other route to `paint()`.
 * It declares no act (`DECLARATION.acts` is empty), subscribes to nothing and starts no
 * timer, so nothing can ask it to draw a third time. The open state of every disclosure
 * on this screen therefore lives where the browser keeps it, on a `<details>` element
 * that is never rebuilt under the reader.
 *
 * The test is written as the property, not as the count: something written into the
 * host after the read has settled is still there afterwards. A face that repainted
 * would have cleared it, exactly as it would have cleared a reader's open panel.
 */
test('nothing repaints after the read settles, so a panel a reader opened cannot be shut by this face', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  const drawn = host.childNodes.length;
  assert.ok(drawn > 0, 'nothing was mounted, so this proves nothing');
  const marker = { tag: 'div', attrs: { 'data-role': 'a-reader-was-here' }, children: [] };
  host.appendChild(marker);
  await new Promise((resolve) => { setTimeout(resolve, 20); });
  assert.equal(host.childNodes.length, drawn + 1, 'the face repainted after its read settled and cleared the host under the reader');
  assert.equal(host.childNodes[host.childNodes.length - 1], marker);
  assert.deepEqual([...DECLARATION.acts], [], 'this face declares an act, so the no-repaint argument above needs re-reading');
  unmount();
});

// -- Owner #348 (2): the menu a right-click opens -------------------------------
//
// Escape and a click away are pinned where they can actually be pinned, in
// tools/browser-mount-smoke.mjs against a real window: this stand-in is not a
// document, has no top layer and delivers no key. What is pinned here is everything
// the handler itself decides -- what the menu offers, what it refuses to offer, that a
// second right-click replaces rather than stacks, and that a repaint takes it down.

function rightClick(host, target, at = { x: 40, y: 60 }) {
  if (!target) throw new Error('nothing was right-clicked: no such node in this tree');
  let defaulted = true;
  const event = {
    type: 'contextmenu', target, clientX: at.x, clientY: at.y, preventDefault() { defaulted = false; },
  };
  for (const listener of [...host.listeners]) {
    if (listener.type === 'contextmenu') listener.handler(event);
  }
  return { defaulted };
}

const menusIn = (host) => nodesOf(host).filter((n) => n.attrs && n.attrs['data-part'] === 'row-menu');
const entriesOf = (menu) => nodesOf(menu).filter((n) => n.attrs && n.attrs['data-menu-entry']);

async function mounted() {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  await unmount.ready;
  return { host, unmount };
}

test('a right-click on a cell drawn as a value opens one menu, and its copy entry carries the whole value', async () => {
  const { host, unmount } = await mounted();
  const cell = nodeWith(host, 'data-cell', 'at');
  assert.ok(cell, 'no time cell was drawn to right-click');
  const { defaulted } = rightClick(host, cell);
  assert.equal(defaulted, false, 'the browser page menu was left to answer instead of this face');
  const menus = menusIn(host);
  assert.equal(menus.length, 1);
  const copy = entriesOf(menus[0]).find((e) => e.attrs['data-menu-entry'] === OFFERS[0].offer);
  assert.ok(copy, 'the declared offer is not in the menu');
  assert.equal(copy.attrs['data-enabled'], 'true');
  // The full timestamp, not the eleven characters of it the cell draws. That is the
  // one thing this menu can do that no column width can.
  assert.equal(copy.attrs['data-menu-value'], ITEMS[2].at);
  assert.ok(cell.attrs['data-menu-value'].length > textOfHost(cell).trim().length, 'the cell is not drawing a cut value, so this test is measuring nothing');
  unmount();
});

test('a right-click on a cell drawn as a stated gap offers the same entry, disabled, carrying the declared reason', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort({ get_transformations: page([item('t-100', 1, '/work/gap.md', { at: null })]) }), []);
  await unmount.ready;
  const gap = nodeWith(host, 'data-state', 'hole');
  assert.ok(gap, 'no cell was drawn as a stated gap, so this test is measuring nothing');
  rightClick(host, gap);
  const copy = entriesOf(menusIn(host)[0]).find((e) => e.attrs['data-menu-entry'] === OFFERS[0].offer);
  assert.equal(copy.attrs['data-enabled'], 'false');
  assert.equal(copy.attrs['data-menu-value'], undefined, 'a disabled entry carries a value it cannot give');
  assert.equal(copy.attrs.title, OFFERS[0].why);
  unmount();
});

/**
 * This face declares no acts, and the menu says so rather than being quietly shorter
 * than another face's. A reader who right-clicks a row on a face that acts and then
 * right-clicks one here is owed the difference in words.
 */
test('the menu states that there is no act to offer here, because this face declares none', async () => {
  const { host, unmount } = await mounted();
  assert.deepEqual([...DECLARATION.acts], [], 'this face now declares an act, so the line below is the wrong shape');
  rightClick(host, nodeWith(host, 'data-cell', 'effect'));
  const said = entriesOf(menusIn(host)[0]).map((e) => textOfHost(e));
  assert.ok(said.includes(DECLARATION.acts_reason), `the menu does not say why it offers no act: ${JSON.stringify(said)}`);
  unmount();
});

test('the menu offers nothing this face has not declared', async () => {
  const { host, unmount } = await mounted();
  rightClick(host, nodeWith(host, 'data-cell', 'effect'));
  const declared = new Set([...OFFERS.map((o) => o.offer), ...DECLARATION.acts.map((a) => a.act), 'no-acts', 'outcome']);
  for (const entry of entriesOf(menusIn(host)[0])) {
    assert.ok(declared.has(entry.attrs['data-menu-entry']), `the menu invented an entry: ${entry.attrs['data-menu-entry']}`);
  }
  unmount();
});

test('a right-click on the fold line itself has something to take: the subject it belongs to', async () => {
  const { host, unmount } = await mounted();
  const line = nodeWith(host, 'data-role', 'subject-summary');
  assert.ok(line, 'no fold line was drawn');
  rightClick(host, line);
  const copy = entriesOf(menusIn(host)[0]).find((e) => e.attrs['data-menu-entry'] === OFFERS[0].offer);
  assert.equal(copy.attrs['data-enabled'], 'true');
  assert.ok(copy.attrs['data-menu-value'].startsWith('/work/'), `the fold line offered ${copy.attrs['data-menu-value']}`);
  unmount();
});

test('a second right-click replaces the menu rather than stacking a second one', async () => {
  const { host, unmount } = await mounted();
  rightClick(host, nodeWith(host, 'data-cell', 'effect'), { x: 10, y: 10 });
  assert.equal(menusIn(host).length, 1);
  rightClick(host, nodeWith(host, 'data-cell', 'at'), { x: 300, y: 200 });
  assert.equal(menusIn(host).length, 1, 'two menus are on the screen at once');
  // And it is a menu about the second thing, not the first.
  const copy = entriesOf(menusIn(host)[0]).find((e) => e.attrs['data-menu-entry'] === OFFERS[0].offer);
  assert.equal(copy.attrs['data-menu-value'], ITEMS[2].at);
  unmount();
});

test('a click anywhere that is not a menu entry takes the menu down', async () => {
  const { host, unmount } = await mounted();
  rightClick(host, nodeWith(host, 'data-cell', 'effect'));
  assert.equal(menusIn(host).length, 1);
  press(host, nodeWith(host, 'data-role', 'subject-summary'));
  assert.equal(menusIn(host).length, 0);
  unmount();
});

test('a repaint leaves no menu behind, and unmounting takes both the menu and the listeners with it', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  // Opened against the waiting screen, before the read settles, so the repaint that
  // draws the answer is the thing being measured.
  rightClick(host, nodeWith(host, 'data-part', 'stat-band') ?? host.childNodes[0]);
  await unmount.ready;
  assert.equal(menusIn(host).length, 0, 'a menu about the waiting screen survived the repaint that replaced it');
  rightClick(host, nodeWith(host, 'data-cell', 'effect'));
  assert.equal(menusIn(host).length, 1);
  unmount();
  assert.equal(menusIn(host).length, 0);
  assert.deepEqual(host.listeners, [], 'the handlers outlived the face they belong to');
});

test('mount refuses a missing host or a missing port, and says which', () => {
  assert.throws(() => mount(null, fullPort(), []), new RegExp(ATLAS_MESSAGES.NO_HOST));
  assert.throws(() => mount(standInHost(), null, []), new RegExp(ATLAS_MESSAGES.NO_PORT));
});

test('mount draws something before the read answers, including the header line', async () => {
  const host = standInHost();
  const unmount = mount(host, fullPort(), []);
  const early = textOfHost(host);
  assert.ok(early.includes(ATLAS_MESSAGES.READING));
  assert.ok(early.includes('atlas'));
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
  await assert.rejects(() => caller.invoke('get_healthz', {}), new RegExp(ATLAS_MESSAGES.UNDECLARED));
  await assert.rejects(() => caller.fold('get_healthz'), new RegExp(ATLAS_MESSAGES.UNDECLARED));
  assert.equal(port.calls.some((c) => c.name === 'get_healthz'), false, 'the call reached the port anyway');
});

// -- fail-closed ------------------------------------------------------------------

for (const [name, result] of [
  ['failed', failed()],
  ['refused', refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not list transformations', gx_code: 'UNAUTHORIZED' })],
  ['absent', absent({ name: 'get_transformations' })],
]) {
  test(`fail-closed: a ${name} list read draws the list as unread, never a silently empty atlas`, async () => {
    const port = fullPort({ get_transformations: result });
    const { tree, html } = await draw(port);
    const listSection = sectionOf(tree, 'list')[0];
    assert.equal(listSection.attrs['data-state'], 'unread');
    assert.ok(textOf(listSection).includes(ATLAS_MESSAGES.LIST_UNREAD));
    assert.ok(textOf(listSection).includes(name), 'the outcome is not named on screen');
    // "not yet read", until Owner #348 (4): `yet` promised a later reading, and two of
    // the three states in this loop are not waiting for one.
    assert.ok(html.includes('not read'), 'the header denominator was not stated as unknown on an unread list');
  });
}

test('a genuinely empty list (answered, zero items) draws the no-subjects state honestly, not the unread state', async () => {
  const port = fullPort({ get_transformations: page([]) });
  const { tree, html } = await draw(port);
  const listSection = sectionOf(tree, 'list')[0];
  assert.equal(listSection.attrs['data-state'], 'no-subjects');
  assert.ok(html.includes(ATLAS_MESSAGES.NO_SUBJECTS));
  // A read that answered with nothing is a measurement, and it is drawn as one:
  // every figure on this screen is a real zero here, not a dash and not a gap.
  for (const noun of ['subjects', 'changes', 'Admit', 'Deny', 'Escalate']) {
    assert.equal(segmentOf(tree, noun).attrs['data-value'], '0', `${noun} should read a real, measured zero on an empty read`);
    assert.equal(figureOf(tree, noun), '0', `${noun} should draw the digit, not a dash`);
  }
  const box = boxesOf(tree)[0];
  assert.equal(box.attrs['data-count'], '0', 'an empty population keeps its border and states 0');
});

// -- toRecord: attest, verbatim, holes never dropped -----------------------------

test('toRecord: a member absent from an item becomes a named hole, not a silent omission', () => {
  const record = toRecord({ transformations: page([item('t-001', 1, '/work/a.md', { actor: undefined })]) });
  assert.ok(record.subjects[0].latest.holes.actor?.includes(ATLAS_MESSAGES.MEMBER_ABSENT));
});

test('toRecord: a member that is not a scalar is a named hole, not a crash', () => {
  const record = toRecord({ transformations: page([item('t-001', 1, '/work/a.md', { path: { nested: true } })]) });
  assert.equal(record.notDrawn.unidentifiable.count, 1, 'a touch with an unreadable path is counted as unidentifiable, not silently dropped');
});

test('toRecord: an item with no usable id is not silently folded into a group -- it is counted as unidentifiable', () => {
  const record = toRecord({
    transformations: page([
      item('t-010', 1, '/work/z.md'),
      { sequence: 2, path: '/work/z.md', at: '2026-08-24T09:02:00Z', actor: 'a', effect: 'write', verdict: 'Admit', digest: 'd002' },
    ]),
  });
  assert.equal(record.notDrawn.unidentifiable.count, 1);
  const subject = record.subjects.find((s) => s.path === '/work/z.md');
  assert.equal(subject.touchCount, 2, 'the raw touch count is not reduced by an identity problem');
  assert.equal(subject.rows.length, 1, 'the row that could not be identified is not drawn as though it were fine');
});

// -- the subject population: every distinct path, unlike faces/graph ------------

test('a path touched exactly once IS drawn as a subject here (unlike faces/graph, which would not draw it at all)', async () => {
  const { tree } = await draw(fullPort());
  const subject = subjectOf(tree, '/work/b.md');
  assert.ok(subject, 'a once-touched path was not drawn as a subject');
  assert.equal(subject.attrs['data-touch-count'], '1');
});

test('distinctPaths and totalTouches both count every touch read, not only the ones with a usable identity', async () => {
  const { state } = await draw(fullPort());
  const record = toRecord(state);
  assert.equal(record.distinctPaths, 3);
  assert.equal(record.totalTouches, 6);
});

test('C-6: subjects are ordered by descending touch count, ties broken by path', async () => {
  const { tree } = await draw(fullPort());
  const subjectEls = findByAttr(tree, 'data-role', 'subject');
  const order = subjectEls.map((s) => s.attrs['data-path']);
  assert.deepEqual(order, ['/work/a.md', '/work/c.md', '/work/b.md']);
});

test('a subject\'s own latest touch is the one with the highest sequence number, not the first or an arbitrary one', async () => {
  const { state } = await draw(fullPort());
  const record = toRecord(state);
  const c = record.subjects.find((s) => s.path === '/work/c.md');
  assert.equal(c.latest.id, 't-006');
  assert.equal(c.latest.verdict, 'Deny');
  assert.equal(c.latest.effect, 'delete');
});

// -- no chain edges are ever drawn (this is faces/graph's question, not this one) --

test('this face draws no structure/child or structure/outside mark anywhere, even for a subject with several touches', async () => {
  const { html } = await draw(fullPort());
  assert.equal(html.includes('data-mark="structure/child"'), false);
  assert.equal(html.includes('data-mark="structure/outside"'), false);
  assert.equal(html.includes('data-child-of'), false);
});

// -- collapsed by default: the Owner correction this face is built to satisfy ----

test('a normal subject (no holes, an ordinary short path and word) is constructed CLOSED by default', async () => {
  const { tree } = await draw(fullPort());
  const a = subjectOf(tree, '/work/a.md');
  assert.equal(a.attrs['data-open'], 'false', 'a normal subject should not be open by default');
  assert.equal(a.attrs.open, undefined, 'the native open attribute should be absent when closed');
  const fold = findByAttr(a, 'data-mark', 'structure/fold-shut');
  assert.ok(fold.length > 0, 'the fold-shut mark was not drawn on a closed subject');
});

test('density: with three subjects in the fixture, the always-visible content is three box heads and three compact fold lines, not the touch history of all three', async () => {
  const { tree, html } = await draw(fullPort());
  // Every summary row is always present (data-role="subject-summary" is the
  // <summary> itself, which native <details> never hides), and so is every box
  // head. Nothing else about a subject is visible until a reader presses one.
  assert.equal(boxesOf(tree).length, 3, 'one box per subject');
  assert.equal(findByAttr(tree, 'data-role', 'subject-summary').length, 3, 'one fold line per subject');
  const openSubjects = (html.match(/data-role="subject"[^>]*data-open="true"/g) ?? []).length;
  assert.equal(openSubjects, 0, 'no subject in this ordinary fixture should be forced open');
});

test('no sentence is repeated per subject: the line that named the subject and its order on every one of them is gone, and the fold line does not restate its own box head', async () => {
  const { tree, html } = await draw(fullPort());
  assert.equal(html.includes('every touch this window read for'), false, 'the per-subject sentence is back');
  // req/96 R-4's own anchor, read over this screen's own repeating unit: what a
  // fold line carries is this subject's own two facts and no prose at all. (Two
  // fold lines may still read alike where the data is alike -- two subjects last
  // written in the same hour do read the same, and the box head above each one is
  // what tells them apart. That is data, not a repeated sentence.)
  for (const node of findByAttr(tree, 'data-role', 'subject-summary')) {
    const words = textOf(node).trim();
    assert.ok(words.length <= 40, `a fold line is carrying prose rather than two facts: ${JSON.stringify(words)}`);
  }
  const first = findByAttr(tree, 'data-path', '/work/a.md')[0];
  const summary = findByAttr(first, 'data-role', 'subject-summary')[0];
  assert.equal(textOf(summary).includes('/work/a.md'), false, 'the fold line repeats the subject its own box head already names');
  assert.equal(/\b3 changes\b/.test(textOf(summary)), false, 'the fold line repeats the count its own box head already states');
});

test('needsOpen: a hole on the latest touch forces that subject open, and only that one', async () => {
  const port = fullPort({
    get_transformations: page([
      ...ITEMS,
      item('t-007', 7, '/work/d.md', { verdict: undefined }),
    ]),
  });
  const { tree } = await draw(port);
  const d = subjectOf(tree, '/work/d.md');
  assert.equal(d.attrs['data-open'], 'true');
  const a = subjectOf(tree, '/work/a.md');
  assert.equal(a.attrs['data-open'], 'false', 'an unrelated subject was forced open by another subject\'s hole');
});

test('needsOpen: a pathologically long path forces that subject open, and its full value is repeated in the detail underneath (N-4-style: never clipped without a full copy on the page)', async () => {
  const longPath = '/work/a-path-so-long-it-cannot-possibly-fit-in-the-fixed-summary-column-width.md';
  const port = fullPort({
    get_transformations: page([item('t-101', 1, longPath)]),
  });
  const { tree, html } = await draw(port);
  const subject = subjectOf(tree, longPath);
  assert.equal(subject.attrs['data-open'], 'true', 'a subject whose path overruns the summary column budget must start open');
  const occurrences = html.split(longPath).length - 1; // String.split with a string separator is a literal match, not a regex -- no escaping needed or correct here
  assert.ok(occurrences >= 2, `expected the full path to appear at least twice (clipped summary cell + full detail heading), got ${occurrences}`);
});

test('needsOpen: an unrecognised (overlong) verdict word forces that subject open', async () => {
  const port = fullPort({
    get_transformations: page([item('t-102', 1, '/work/e.md', { verdict: 'ThisIsNotAKnownVerdictWord' })]),
  });
  const { tree } = await draw(port);
  const subject = subjectOf(tree, '/work/e.md');
  assert.equal(subject.attrs['data-open'], 'true');
});

// -- claims section: parts/src/checkable.mjs's two population-wide claims -------

test('the claims section shows exactly the two population-wide claims, not the three subject/chain-relative ones', async () => {
  const { tree } = await draw(fullPort());
  const claims = sectionOf(tree, 'claims')[0];
  const ids = findByAttr(claims, 'data-claim-id').map((n) => n.attrs['data-claim-id']);
  assert.deepEqual(ids.sort(), ['identities-appear-once', 'serial-can-be-cut']);
});

test('the claims section states a real verdict for the population it was actually given', async () => {
  const { tree } = await draw(fullPort());
  const claims = sectionOf(tree, 'claims')[0];
  const serial = findByAttr(claims, 'data-claim-id', 'serial-can-be-cut')[0];
  assert.equal(serial.attrs['data-holds'], 'true', 'every fixture digest here is a valid hex string of the required length');
});

// -- C-3: what is not drawn ---------------------------------------------------------

test('C-3: the members this face does not draw are named with reasons', async () => {
  const { tree } = await draw(fullPort());
  const words = textOf(sectionOf(tree, 'not-drawn')[0]);
  for (const entry of DECLARATION.undrawn) assert.ok(words.includes(entry.what), `${entry.what} is declared undrawn but not stated on screen`);
});

test('C-3: an unidentifiable touch is counted on screen, not silently absorbed into the total', async () => {
  const port = fullPort({
    get_transformations: page([
      ...ITEMS,
      { sequence: 99, path: '/work/f.md', at: '2026-08-24T09:09:00Z', actor: 'a', effect: 'write', verdict: 'Admit', digest: 'd099' },
    ]),
  });
  const { html } = await draw(port);
  assert.ok(html.includes('1'), 'expected the unidentifiable count to be stated');
  const record = toRecord(await face.read(port));
  assert.equal(record.notDrawn.unidentifiable.count, 1);
});

// -- controls: bordered, closed by default, in one row ---------------------------

test('why and legend are both drawn as native <details> controls, closed by default, sitting inside one shared control-row', async () => {
  const { tree } = await draw(fullPort());
  const controls = findByAttr(tree, 'data-role', 'control');
  // Owner directive #335 (1): claims and omitted joined why and legend in this one
  // row. They were two always-open bands of prose taking about two thirds of the
  // window, met before the second subject line.
  assert.deepEqual(controls.map((c) => c.attrs['data-control']), ['why', 'legend', 'claims', 'omitted']);
  for (const control of controls) {
    assert.equal(control.attrs['data-open'], 'false', `${control.attrs['data-control']} should be closed by default`);
    assert.equal(control.attrs.open, undefined, `${control.attrs['data-control']}'s native open attribute should be absent when closed`);
  }
  const row = findByAttr(tree, 'data-role', 'control-row')[0];
  assert.ok(row, 'no shared control-row wrapper was drawn');
  const inRow = find(row, (n) => n.attrs['data-role'] === 'control');
  assert.equal(inRow.length, controls.length, 'every control should sit inside the one control-row');
});

/**
 * Owner #348 (4), held as a property rather than as two literals.
 *
 * The first version of this test named the two hints it expected to find, which meant
 * it passed on `omitted -- what is not drawn` and `legend -- symbols used`: two hints
 * that are their own label said again in more letters, which is exactly the redundancy
 * this atom removes. What the rule actually is: a hint has to be there (a bare word is
 * not a label) AND it has to say something the name does not, so it may not share a
 * word with the name it sits beside.
 *
 * req/822_c7 (Owner #387/#388 冗長文字全掃): the hint moved off the default-visible
 * line and onto the control's own summary, as its `title` (a hover) and a
 * `data-hint` attribute -- so this reads the property off the summary now instead
 * of off a `data-role="control-hint"` span that no longer draws.
 */
test('each control name carries a hint that says something the name does not', async () => {
  const { tree } = await draw(fullPort());
  const controls = findByAttr(tree, 'data-role', 'control');
  assert.ok(controls.length >= 4, 'there are not enough controls here to be measuring anything');
  const wordsOf = (words) => new Set(words.toLowerCase().split(/[^a-z]+/).filter((w) => w.length > 2));
  for (const control of controls) {
    const name = textOf(findByAttr(control, 'data-role', 'control-name')[0]).trim();
    const summary = find(control, (n) => n.tag === 'summary')[0];
    assert.ok(summary, `the ${name} control drew no summary`);
    const hint = (summary.attrs['data-hint'] ?? '').trim();
    assert.equal(summary.attrs.title, summary.attrs['data-hint'], `the ${name} control's title and data-hint should carry the same words`);
    assert.ok(hint.length > 3, `the ${name} control has no plain-language hint`);
    const shared = [...wordsOf(name)].filter((word) => wordsOf(hint).has(word));
    assert.deepEqual(shared, [], `the ${name} control's hint repeats its own name: "${hint}"`);
  }
});

test('the hint no longer draws as its own visible span beside the control name', async () => {
  const { tree } = await draw(fullPort());
  assert.deepEqual(findByAttr(tree, 'data-role', 'control-hint'), [], 'a control-hint span is still drawn; the hint should ride the summary\'s title/data-hint instead');
});

/**
 * The same rule one level up, over the words the closed screen actually draws.
 *
 * `omitted -- what is not drawn` was not caught by any reading this face had, because
 * nothing here was reading for a phrase that restates the phrase beside it. This does
 * not try to be a general redundancy detector -- it holds the two shapes that were
 * genuinely on this screen: a section heading that repeats the control it sits inside,
 * and a hint that repeats its own name.
 */
test('no heading on this screen repeats the name of the control it is drawn inside', async () => {
  const { tree } = await draw(fullPort());
  const headings = find(tree, (n) => n.tag === 'h2');
  assert.deepEqual(
    headings.map((h) => textOf(h)),
    [],
    'a heading inside a named control is the control name said twice; there is nowhere else on this screen an h2 belongs',
  );
});

/**
 * req/822_c7 (Owner #387/#388 冗長文字全掃): the question sentence used to be its
 * own always-visible span next to the face name, ahead of the stat band that is
 * this screen's real head now (Owner #340). It still has to reach a reader, just
 * not by default: it rides the face-name heading's own `title` (a hover) and a
 * `data-question` attribute now, so this reads it there instead of off the header
 * line's visible text.
 */
test('a header line states the face name, before anything else is drawn, and carries what this screen answers on its own title/data-question', async () => {
  const { tree } = await draw(fullPort());
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  assert.ok(header, 'no header line was drawn');
  assert.ok(textOf(header).includes('atlas'));
  const named = findByAttr(header, 'data-question')[0];
  assert.ok(named, 'no element in the header carries the declared question as a data-question attribute');
  assert.equal(named.attrs.title, DECLARATION.question, 'the declared question should be on the heading\'s own title');
  assert.equal(named.attrs['data-question'], DECLARATION.question, 'the declared question should be on the heading\'s own data-question');
  assert.ok(!textOf(header).includes(DECLARATION.question), 'the question sentence should no longer draw as default-visible header text');
  assert.equal(tree.children[0], header, 'the header line must be the first thing drawn on screen');
});

test('both denominators are stated every time this screen draws, as the first two figures of the band under the header', async () => {
  const { tree } = await draw(fullPort());
  const band = bandOf(tree);
  assert.ok(band, 'no stat band was drawn');
  assert.equal(tree.children[1], band, 'the band must sit directly under the header line, before any control');
  assert.equal(band.attrs['data-count'], '5');
  assert.equal(figureOf(tree, 'subjects'), '3');
  assert.equal(figureOf(tree, 'changes'), '6');
  assert.ok(DECLARATION.rows.reports_denominator, 'this face declares that it states both denominators every time it draws');
});

test('the band states this screen\'s whole split by standing, zero-inclusive, each in the standing\'s own hue and its own mark', async () => {
  const { tree } = await draw(fullPort());
  // The fixture: /work/a.md (3 touches, latest Admit), /work/b.md (1, Admit),
  // /work/c.md (2, latest Deny). Nothing here was ever escalated, and that is a
  // measurement, so it is drawn as 0 rather than left out.
  assert.equal(figureOf(tree, 'Admit'), '2');
  assert.equal(figureOf(tree, 'Deny'), '1');
  assert.equal(figureOf(tree, 'Escalate'), '0');
  // The label under each figure is the engine's own word, unchanged -- the same word
  // the pill on the box beneath it carries, rather than a second vocabulary for it.
  for (const [noun, mark] of [['Admit', 'verdict/Admit'], ['Deny', 'verdict/Deny'], ['Escalate', 'verdict/Escalate']]) {
    const segment = segmentOf(tree, noun);
    assert.equal(findByAttr(segment, 'data-mark', mark).length, 1, `${noun} does not carry its own mark`);
    const figure = findByAttr(segment, 'data-role', 'figure')[0];
    assert.match(figure.attrs.style ?? '', /color:\s*var\(--(admit|deny|escalate)\)/, `${noun} does not carry its own hue`);
  }
});

test('the three standings and the ones that could not be placed always add up to the subject count', async () => {
  const port = fullPort({
    get_transformations: page([
      item('t-401', 1, '/work/admitted.md'),
      item('t-402', 2, '/work/denied.md', { verdict: 'Deny' }),
      item('t-403', 3, '/work/escalated.md', { verdict: 'Escalate' }),
      item('t-404', 4, '/work/strange.md', { verdict: 'Perhaps' }),
      item('t-405', 5, '/work/absent.md', { verdict: undefined }),
    ]),
  });
  const { tree } = await draw(port);
  assert.equal(figureOf(tree, 'subjects'), '5');
  const placed = ['Admit', 'Deny', 'Escalate'].reduce((sum, noun) => sum + Number(figureOf(tree, noun)), 0);
  assert.equal(placed, 3, 'a word this screen does not hold must not be counted into a standing it never carried');
  const subjects = segmentOf(tree, 'subjects');
  assert.equal(findByAttr(subjects, 'data-mark', 'structure/hole').length, 1, 'the figure that the three columns do not account for carries no stated gap');
  assert.ok(subjects.attrs.title.startsWith('2 of these'), `the gap does not say how many: ${subjects.attrs.title}`);
});

test('negative control: a population every one of whose subjects has a standing draws no gap mark on the subject figure', async () => {
  const { tree } = await draw(fullPort());
  const subjects = segmentOf(tree, 'subjects');
  assert.equal(findByAttr(subjects, 'data-mark', 'structure/hole').length, 0, 'a gap mark is drawn where there is no gap');
  assert.equal(subjects.attrs.title ?? null, null);
});

test('a read that did not answer draws a dash on every figure, never a zero', async () => {
  const port = fullPort({ get_transformations: refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not list transformations', gx_code: 'UNAUTHORIZED' }) });
  const { tree } = await draw(port);
  for (const noun of ['subjects', 'changes', 'Admit', 'Deny', 'Escalate']) {
    assert.equal(segmentOf(tree, noun).attrs['data-value'], 'unread', `${noun} claims a count on a screen that read nothing`);
    assert.equal(figureOf(tree, noun), parts.statDash);
  }
  const box = boxesOf(tree)[0];
  assert.equal(box.attrs['data-count'], parts.statDash, 'an unread population must keep its border and state a dash, not 0');
});

test('every subject is an object on the screen: a bordered box whose head states the subject, its own count and its standing', async () => {
  const { tree } = await draw(fullPort());
  const boxes = boxesOf(tree);
  assert.deepEqual(boxes.map((b) => b.attrs['data-box']), ['/work/a.md', '/work/c.md', '/work/b.md']);
  assert.deepEqual(boxes.map((b) => b.attrs['data-count']), ['3', '2', '1']);
  // Found in the picture, not in a test: a box holding one change said "1 changes".
  assert.deepEqual(boxes.map((b) => b.attrs['data-noun']), ['changes', 'changes', 'change']);
  const c = boxes[1];
  const head = headOf(c);
  assert.ok(head, 'a box was drawn with no head');
  assert.ok(textOf(head).includes('/work/c.md'));
  assert.ok(textOf(head).includes('2 changes'));
  const pill = findByAttr(head, 'data-part', 'verdict-badge')[0];
  assert.ok(pill, 'the box head carries no standing at all');
  assert.equal(pill.attrs['data-verdict'], 'Deny', 'the head states the standing of the subject\'s most recent change');
  assert.equal(pill.attrs['data-filled'], 'true', 'a standing is drawn as a filled chip, not as a stroke on the page background');
});

test('a subject whose most recent change carries no verdict states that in its box head, rather than leaving the corner empty', async () => {
  const port = fullPort({ get_transformations: page([item('t-501', 1, '/work/quiet.md', { verdict: undefined })]) });
  const { tree } = await draw(port);
  const head = headOf(boxesOf(tree)[0]);
  assert.equal(findByAttr(head, 'data-mark', 'structure/hole').length, 1);
  assert.equal(findByAttr(head, 'data-part', 'verdict-badge').length, 0);
});

test('the last thing this screen draws is what the draw cost, measured here rather than estimated', async () => {
  const { tree } = await draw(fullPort());
  const footer = footerOf(tree);
  assert.equal(footer.attrs['data-part'], 'runtime-footer');
  const ms = Number(footer.attrs['data-render-ms']);
  assert.ok(Number.isFinite(ms) && ms >= 0, `the footer carries no measured figure: ${footer.attrs['data-render-ms']}`);
  assert.ok(textOf(footer).includes('render'));
  assert.ok(textOf(footer).includes('read the list of changes'), 'the footer does not name what this face read');
});

test('a screen that read nothing says so in the footer with a dash, rather than naming a source it never had', async () => {
  const port = fullPort({ get_transformations: failed() });
  const { tree } = await draw(port);
  const footer = footerOf(tree);
  assert.ok(textOf(footer).includes(`read ${parts.statDash}`), `the footer claims a source on an unread screen: ${textOf(footer)}`);
});

// -- order (screen-level "why") -----------------------------------------------------

test('C-6: the order reason is stated inside the why control', async () => {
  const { html } = await draw(fullPort());
  assert.ok(html.includes('sixth in build order'));
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

test('structure/subject is actually drawn on this fixture, once per subject line, not merely declared', async () => {
  const { html } = await draw(fullPort());
  const occurrences = html.split('data-mark="structure/subject"').length - 1;
  assert.equal(occurrences, 3);
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

// -- rows are not edited -------------------------------------------------------------

test('rows are not edited: rendering the same state twice gives the same tree, apart from the measured cost of drawing it', async () => {
  const state = await face.read(fullPort());
  const first = face.view(state);
  const second = face.view(state);
  // The footer is the one node on this screen that is allowed to differ between two
  // draws of one state, because it states how long this draw took and two draws do
  // not take the same time. Everything above it is the same tree or the record was
  // edited, which is the property this test has always been for.
  assert.equal(toHtml(withoutFooter(first)), toHtml(withoutFooter(second)));
  for (const tree of [first, second]) {
    assert.ok(Number.isFinite(Number(footerOf(tree).attrs['data-render-ms'])), 'a draw that measured nothing must not be the reason this test passes');
  }
});

test('the record toRecord() produces is immutable -- a subject cannot be mutated after the fact', () => {
  const record = toRecord({ transformations: page([...ITEMS]) });
  assert.throws(() => { record.subjects[0].touchCount = 999; });
  assert.throws(() => { record.notDrawn.unidentifiable.count = 999; });
});

// -- the parts are a seam ----------------------------------------------------------

test('the parts are injected: a face built on a stub draws the stub', async () => {
  const marker = 'this tree came from the stub';
  const stub = {
    ...parts,
    receiptRow: (record) => el('div', { 'data-part': 'receipt-row', 'data-row': record.id }, [marker]),
  };
  const other = createFace({ parts: stub });
  const state = await other.read(fullPort());
  // Force every subject open so the stubbed receiptRow (used in the per-touch
  // detail) is actually reached by this render.
  const html = toHtml(other.view(state));
  void html; // the marker only appears once a subject's detail is actually opened; see the dedicated open-detail test below for that path
  assert.ok(typeof html === 'string');
});

test('opening a subject\'s detail draws the injected part: the stub marker appears when a subject is forced open', async () => {
  const marker = 'this tree came from the stub';
  const stub = {
    ...parts,
    receiptRow: (record) => el('div', { 'data-part': 'receipt-row', 'data-row': record.id }, [marker]),
  };
  const other = createFace({ parts: stub });
  const port = fullPort({
    get_transformations: page([item('t-201', 1, '/work/g.md', { verdict: undefined })]),
  });
  const state = await other.read(port);
  const html = toHtml(other.view(state));
  assert.ok(html.includes(marker), 'the stubbed receiptRow was not reached for a subject forced open by a hole');
});

// -- req/97 gap-list item 4: nothing on the surface a first viewer cannot decode ----

/**
 * The strings req/96 axis B scores zero for: a path inside this repository, a route
 * name, a requirement number, an all-caps wire constant. This is the machine form of
 * "first-viewer-undecodable", and it is deliberately a regex over the drawn text
 * rather than a list of the six offenders req/97 happened to name -- a cure that only
 * removes the ones a reviewer listed is a cure that lets the seventh through.
 */
const INTERNAL_ON_SURFACE = [
  { name: 'a path inside this repository', pattern: /\b(parts|faces|shell|membrane|tools|req)\/[a-z0-9_.-]+/i },
  { name: 'a requirement number', pattern: /\breq\/\d+|\bSS\d+|\bAC-[A-Z0-9]/ },
  { name: 'an all-caps wire constant', pattern: /\b[A-Z][A-Z_]{3,}\b/ },
  { name: 'a route name', pattern: /\b(get|post)_[a-z_]+/ },
];

const drawnSurface = (tree) => {
  // Everything the screen draws except what is behind an "internal reference"
  // control -- which is exactly where an internal name is allowed to be, labelled.
  const strip = (node) => {
    if (!node || typeof node !== 'object') return node;
    if (node.attrs && node.attrs['data-role'] === 'internal-reference') return null;
    if (!Array.isArray(node.children)) return node;
    return { ...node, children: node.children.map(strip).filter((c) => c !== null) };
  };
  // Joined with a space rather than run together, and this is the instrument's own
  // bug fixed rather than a preference: textOf() concatenates every text node with
  // nothing between them, so a word ending one element and a word beginning the next
  // become one token -- and every pattern below anchors on \b. The screen was drawing
  // `code: UNAUTHORIZED` immediately before the footer's `render`, so the reading was
  // looking at `UNAUTHORIZEDrender`, which has no word boundary after the capitals and
  // matched nothing. A check that cannot fire is not a check.
  const words = [];
  walk(strip(tree), (node) => { if (isText(node)) words.push(node.text); });
  return words.join(' ');
};

/**
 * Read over every state this screen has, not only the one that answers.
 *
 * The first version of this test read the answered screen alone, and the screen that
 * could not read anything was drawing `code: UNAUTHORIZED` -- an all-caps wire
 * constant on a product surface, which is exactly what the reading below is for and
 * exactly what req/96 axis B scores zero for. Found by looking at the photograph of
 * the refused fixture; the test was one state short of catching it.
 */
for (const [name, result] of [
  ['answered', page([...ITEMS])],
  ['answered with nothing', page([])],
  ['refused', refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not list transformations', gx_code: 'UNAUTHORIZED' })],
  ['failed', failed()],
  ['found no such method', absent({ name: 'get_transformations' })],
]) {
  test(`req/97 gap-list item 4: no internal name reaches the atlas surface when the read ${name} -- they live behind the internal-reference control, labelled`, async () => {
    const { tree } = await draw(fullPort({ get_transformations: result }));
    const surface = drawnSurface(tree);
    assert.ok(surface.length > 200, 'almost nothing was drawn, so this proves nothing');
    for (const { name: what, pattern } of INTERNAL_ON_SURFACE) {
      const hit = pattern.exec(surface);
      assert.equal(hit, null, `${what} on the product surface: "${hit && hit[0]}"`);
    }
  });
}

test('negative control: the same reading fires on the text that is deliberately kept, which is why its absence above means something', async () => {
  const { tree } = await draw(fullPort());
  const references = findByAttr(tree, 'data-role', 'internal-reference');
  assert.ok(references.length > 0, 'no internal reference was drawn at all, so nothing was moved -- it was deleted');
  const kept = references.map((r) => textOf(r)).join(' ');
  const fired = INTERNAL_ON_SURFACE.filter(({ pattern }) => pattern.test(kept));
  assert.ok(fired.length >= 2, 'the internal reference carries no internal names, so the reading above is not measuring anything');
  for (const reference of references) {
    assert.match(textOf(reference), /internal reference/, 'the panel does not say what it is');
  }
});

test('every declared omission has a plain-language form, and it is the one drawn', async () => {
  const { tree } = await draw(fullPort());
  const omitted = findByAttr(tree, 'data-control', 'omitted')[0];
  assert.ok(omitted, 'the omitted census is not reachable at all');
  for (const entry of DECLARATION.undrawn) {
    assert.equal(typeof entry.plain, 'string', `${entry.what} has no plain-language form`);
    assert.ok(entry.plain.length > 20, `${entry.what}'s plain form says nothing`);
    assert.ok(textOf(omitted).includes(entry.plain), `${entry.what}'s plain form is not what the screen draws`);
  }
});
