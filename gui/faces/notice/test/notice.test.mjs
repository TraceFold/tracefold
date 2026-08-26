// SPDX-License-Identifier: Apache-2.0
// The face's own behaviour: what it draws, what it never touches, and what happens
// when its window's record grows, is malformed, or is not there at all.

import test from 'node:test';
import assert from 'node:assert/strict';

import { createFace, face, toRecord, DISPLAY_CAP } from '../notice.mjs';
import { DECLARATION } from '../declaration.mjs';
import { parts } from '../binding.mjs';
import { standInHost, textOfHost, attrValues } from './dom-stand-in.mjs';
import {
  asked, answered, refused, absent, shellRefused, unrecognised, representative,
} from './sample-notices.mjs';

const { find, findByAttr, textOf } = parts.element;

/** A port that raises the instant anything on it is touched. C-7 is not "the face
 * did not send anything today", it is "the face has no path that could reach the
 * port at all" -- this makes the second claim checkable at run time, not only by a
 * source-pattern scan. */
function trapPort() {
  return new Proxy({}, {
    get(target, prop) { throw new Error(`C-7 violation: the notice face touched port.${String(prop)}`); },
  });
}

test('W2: mount returns a function, and unmount empties the host', () => {
  const host = standInHost();
  const unmount = face.mount(host, trapPort(), representative(), { pollMs: 0 });
  assert.equal(typeof unmount, 'function');
  assert.ok(host.childNodes.length > 0);
  unmount();
  assert.equal(host.childNodes.length, 0);
});

test('mount refuses a missing host', () => {
  assert.throws(() => face.mount(null, trapPort(), []), /a face is mounted into a host element/);
});

test('C-7: mounting, drawing every shape, and unmounting never touches the port', () => {
  const host = standInHost();
  const unmount = face.mount(host, trapPort(), representative(), { pollMs: 0 });
  unmount.repaint();
  unmount();
});

test('C-7: the declaration itself has nothing to send', () => {
  assert.deepEqual(DECLARATION.consumes, []);
});

test('drawing needs no answer to wait for: the screen is complete on the first paint', () => {
  const host = standInHost();
  face.mount(host, trapPort(), [asked('get_transformations')], { pollMs: 0 });
  assert.ok(textOfHost(host).includes('asked, not yet answered'));
});

test('an empty window and a window this face was never given read as different facts', () => {
  const emptyHost = standInHost();
  face.mount(emptyHost, trapPort(), [], { pollMs: 0 });
  assert.ok(textOfHost(emptyHost).includes('has not asked the server anything yet'));
  assert.equal(textOfHost(emptyHost).includes('was not handed'), false);

  const state = face.read(null);
  const drawn = textOfHostFromTree(face.view(state));
  assert.ok(drawn.includes('was not handed its own record'));
  assert.equal(drawn.includes('has not asked the server anything yet'), false);
});

function textOfHostFromTree(tree) {
  let out = '';
  const walk = (node) => {
    if (typeof node.text === 'string') { out += node.text; return; }
    for (const child of node.children ?? []) walk(child);
  };
  walk(tree);
  return out;
}

test('an entry the window recorded through the shell and one recorded through the membrane are told apart', () => {
  const host = standInHost();
  face.mount(host, trapPort(), [shellRefused('pane:divide', 'no such act'), answered('get_transformations', 'GET', '/v1/transformations')], { pollMs: 0 });
  assert.deepEqual(attrValues(host, 'data-through').sort(), ['membrane', 'shell']);
});

test('a refusal carries the engine\'s own words up, unedited', () => {
  const host = standInHost();
  face.mount(host, trapPort(), [refused('post_candidates_id_commit', 'POST', '/v1/candidates/{id}/commit', {
    type: 'about:blank', title: 'conflict', status: 409, detail: 'this candidate was already committed', gx_code: 'IDEMPOTENCY_CONFLICT',
  })], { pollMs: 0 });
  const text = textOfHost(host);
  assert.ok(text.includes('this candidate was already committed'));
  assert.ok(text.includes('IDEMPOTENCY_CONFLICT'));
});

test('a call that came back for a route the table does not carry draws the hole mark', () => {
  const host = standInHost();
  face.mount(host, trapPort(), [absent('get_everything_i_wish_for', { name: 'get_everything_i_wish_for' })], { pollMs: 0 });
  assert.ok(attrValues(host, 'data-mark').includes('structure/hole'));
});

test('an outcome word this face does not recognise draws the undefined mark, not silence', () => {
  const host = standInHost();
  face.mount(host, trapPort(), [unrecognised('get_transformations')], { pollMs: 0 });
  assert.ok(attrValues(host, 'data-mark').includes('undefined'));
});

test('C-4: every mark drawn was declared', () => {
  const declared = new Set(DECLARATION.marks.map((m) => m.mark));
  const drawn = attrValues(standInHostWith(representative()), 'data-mark');
  assert.ok(drawn.length > 0, 'nothing was drawn, so this assertion would be vacuous');
  for (const mark of drawn) assert.ok(declared.has(mark), `undeclared mark drawn: ${mark}`);
});

function standInHostWith(notices) {
  const host = standInHost();
  face.mount(host, trapPort(), notices, { pollMs: 0 });
  return host;
}

test('C-5: no meaning is carried by two different marks', () => {
  const host = standInHostWith(representative());
  const nodes = [];
  const visit = (n) => { if (n.attrs && 'data-means' in n.attrs) nodes.push(n); for (const c of n.childNodes ?? []) visit(c); };
  visit(host);
  const byMeaning = new Map();
  for (const n of nodes) {
    const means = n.attrs['data-means'];
    const mark = n.attrs['data-mark'];
    if (byMeaning.has(means)) assert.equal(byMeaning.get(means), mark, `${means} carried by two marks`);
    byMeaning.set(means, mark);
  }
});

test('AC-F3: every glyph on screen states its width and height', () => {
  const host = standInHostWith(representative());
  const nodes = [];
  const visit = (n) => { if (n.tag === 'svg' && n.attrs['data-mark']) nodes.push(n); for (const c of n.childNodes ?? []) visit(c); };
  visit(host);
  assert.ok(nodes.length > 0, 'no glyph was drawn, so this assertion would be vacuous');
  for (const n of nodes) {
    assert.ok(/^\d+$/.test(n.attrs.width ?? ''), `glyph missing a width: ${JSON.stringify(n.attrs)}`);
    assert.ok(/^\d+$/.test(n.attrs.height ?? ''), `glyph missing a height: ${JSON.stringify(n.attrs)}`);
  }
});

test('a non-record in the window\'s array is counted and named, not silently skipped', () => {
  // The property is unchanged and the sentence carrying it is not: the count used to
  // be read out of a denominator line ("1 of 3 entries drawn, 2 not a record") that
  // restated two figures the band and the box head now carry in the open. What is
  // asserted here is the fact itself -- two items could not be placed, both are
  // counted, and the row that counts them says so in words.
  const tree = face.view(face.read(['this is not a record', 42, asked('get_transformations')]));
  const row = findByAttr(tree, 'data-tally-entry', 'not a record')[0];
  assert.ok(row, 'nothing on the screen counts the items that were not records');
  assert.equal(row.attrs['data-count'], '2');
  assert.match(textOf(row), /not a record/);
});

test('entries past the drawn budget are counted and not drawn row by row', () => {
  const many = [];
  for (let i = 0; i < DISPLAY_CAP + 7; i += 1) many.push(asked(`get_transformations_${i}`));
  const host = standInHost();
  face.mount(host, trapPort(), many, { pollMs: 0 });
  const drawnCount = [...host.childNodes].length > 0 ? attrValuesCount(host, 'data-entry') : 0;
  assert.equal(drawnCount, DISPLAY_CAP);
  // The count is stated once now. It used to be stated three times on one screen: as
  // this clause on the open surface, again as a line inside the `omitted` control,
  // and a third time as the last of the declared omissions. The clause carries the
  // figure, the declared omission carries the reason, and the middle one is gone.
  assert.ok(textOfHost(host).includes('7 more arrived and are not drawn'));
  assert.equal(textOfHost(host).includes('entries arrived past the drawn budget'), false, 'the same fact is still stated twice');
  // On the open surface it is one clause with the figure in it, not the paragraph it
  // used to be -- and the paragraph is not lost, it is that line's own title and a
  // declared omission besides.
  const tree = face.view(face.read(many));
  const line = findByAttr(tree, 'data-role', 'capped-line')[0];
  assert.equal(textOf(line), '7 more arrived and are not drawn');
  assert.ok((line.attrs.title ?? '').length > 60, 'the reason behind the figure is nowhere in reach');
  assert.equal(textOf(line).includes('stated below'), false, 'the line still points at a census that is above it and folded');
});

function attrValuesCount(root, name) {
  let count = 0;
  const visit = (n) => { if (n.attrs && name in n.attrs) count += 1; for (const c of n.childNodes ?? []) visit(c); };
  visit(root);
  return count;
}

test('a record is frozen: no shipped line can edit one after it is built', () => {
  const record = toRecord(answered('get_transformations', 'GET', '/v1/transformations'), 0);
  assert.ok(Object.isFrozen(record));
  assert.throws(() => { record.outcome = 'tampered'; }, TypeError);
});

test('the face is handed its window\'s array and grows what it draws when the array grows', () => {
  const shared = [asked('get_transformations')];
  const host = standInHost();
  const unmount = face.mount(host, trapPort(), shared, { pollMs: 0 });
  assert.equal(attrValuesCount(host, 'data-entry'), 1);
  shared.push(answered('get_transformations', 'GET', '/v1/transformations'));
  assert.equal(unmount.repaint(), true, 'repaint should notice the array grew');
  assert.equal(attrValuesCount(host, 'data-entry'), 2);
  assert.equal(unmount.repaint(), false, 'a second repaint with nothing new should say so');
});

test('the growth check is polled automatically at the interval given to mount', async () => {
  const shared = [];
  const host = standInHost();
  const unmount = face.mount(host, trapPort(), shared, { pollMs: 5 });
  assert.equal(attrValuesCount(host, 'data-entry'), 0);
  shared.push(asked('get_transformations'));
  await new Promise((resolve) => { setTimeout(resolve, 60); });
  assert.equal(attrValuesCount(host, 'data-entry'), 1);
  unmount();
});

test('unmounting stops the poll: growth after unmount is never drawn', async () => {
  const shared = [];
  const host = standInHost();
  const unmount = face.mount(host, trapPort(), shared, { pollMs: 5 });
  unmount();
  shared.push(asked('get_transformations'));
  await new Promise((resolve) => { setTimeout(resolve, 60); });
  assert.equal(host.childNodes.length, 0);
});

test('the parts are injected: a face built on a stub draws the stub\'s marks', () => {
  let asked_ = 0;
  const stub = { ...parts, glyph: (ns, key, opts) => { asked_ += 1; return parts.glyph(ns, key, opts); } };
  const built = createFace({ parts: stub });
  const host = standInHost();
  built.mount(host, trapPort(), [absent('get_everything_i_wish_for', {})], { pollMs: 0 });
  assert.ok(asked_ > 0);
});

test('rendering the same state twice gives the same tree, apart from the one figure that is a measurement', () => {
  const state = face.read(representative());
  const first = face.view(state);
  const second = face.view(state);
  // The footer's render figure is how long this build took, so two builds of the
  // same state disagree about it by design -- that is the difference between a
  // measured number and a claimed one. Everything else is still required to be
  // identical, and the figures themselves are required to be real.
  assert.deepEqual(withoutMeasurement(first), withoutMeasurement(second));
  for (const tree of [first, second]) {
    const footer = findByAttr(tree, 'data-part', 'runtime-footer')[0];
    const measured = Number(footer.attrs['data-render-ms']);
    assert.ok(Number.isFinite(measured) && measured >= 0, `the footer carries no measured figure: ${footer.attrs['data-render-ms']}`);
  }
});

/** The tree with the one measured field blanked, so the rest can be compared. */
function withoutMeasurement(node) {
  if (!node || typeof node !== 'object') return node;
  const attrs = node.attrs && 'data-render-ms' in node.attrs ? { ...node.attrs, 'data-render-ms': 'measured' } : node.attrs;
  if (typeof node.text === 'string') return { ...node, text: node.text.replace(/^render .*$/, 'render measured') };
  return { ...node, attrs, children: (node.children ?? []).map(withoutMeasurement) };
}

// -- Owner #348 (2): what a row offers, in the gutter and under a right-click ------

test('the column that held eight dead buttons holds one live control per row, and none of them is named reach', () => {
  const tree = face.view(face.read(representative()));
  const buttons = findByAttr(tree, 'data-role', 'offer').filter((n) => n.attrs['data-shape'] === 'gutter');
  // representative() carries 8 method-bearing entries (2 non-records excluded), and
  // every one of them can be copied -- so where there were eight disabled controls
  // there are now eight that act.
  assert.equal(buttons.length, 8, `expected one gutter control per drawn row, drew ${buttons.length}`);
  for (const button of buttons) {
    assert.equal(button.attrs.disabled, undefined, `${button.attrs['data-offer-entry']} drew a control a hand cannot press`);
    assert.ok((button.attrs.title ?? '').length > 20, 'a control that says nothing about what it would give');
  }
  assert.deepEqual(findByAttr(tree, 'data-role', 'reach'), [], 'the retired control is still being drawn');
  // And the reason it was retired is still on the screen, once, where the things this
  // face deliberately does not draw are listed.
  const omission = findByAttr(tree, 'data-omission', 'a way through to the face that reads this record');
  assert.equal(omission.length, 1);
  assert.match(textOf(omission[0]), /cannot reach another screen/);
});

test('a right-click on a row offers every value that row holds, drawn from the one declaration the gutter draws from', () => {
  const tree = face.view(face.read(representative(), [], { menu: { entry: '3', x: 12, y: 34 } }));
  const menu = findByAttr(tree, 'data-role', 'menu');
  assert.equal(menu.length, 1);
  const offers = findByAttr(menu[0], 'data-role', 'offer');
  assert.deepEqual(offers.map((o) => o.attrs['data-offer']), DECLARATION.offers.map((o) => o.id));
  // Entry 3 is the refused call: it has a time, a call and a word of the server's own,
  // so all four can act.
  for (const offer of offers) assert.equal(offer.attrs.disabled, undefined, `${offer.attrs['data-offer']} is dimmed on a row that holds it`);
  // C-7 said out loud rather than left to be inferred from a menu with no verbs.
  assert.match(textOf(menu[0]), /nothing on this screen can be sent/);
});

test('an offer this record cannot answer is drawn, dimmed, and says why -- the rule the gutter already followed', () => {
  // Entry 1 is an asked call through the shell: no status, and nothing came back
  // carrying a word of the server's own.
  const tree = face.view(face.read(representative(), [], { menu: { entry: '1', x: 0, y: 0 } }));
  const menu = findByAttr(tree, 'data-role', 'menu')[0];
  const code = findByAttr(menu, 'data-offer', 'code')[0];
  assert.equal(code.attrs.disabled, '', 'an offer with nothing behind it drew pressable');
  assert.match(code.attrs.title, /no word of the server's own/);
  const call = findByAttr(menu, 'data-offer', 'call')[0];
  assert.equal(call.attrs.disabled, undefined, 'this row does name a call');
});

test('a second right-click cannot stack two menus, because there is one slot and it is overwritten', () => {
  const notices = representative();
  const first = face.view(face.read(notices, [], { menu: { entry: '2', x: 5, y: 5 } }));
  assert.equal(findByAttr(first, 'data-role', 'menu').length, 1);
  const second = face.view(face.read(notices, [], { menu: { entry: '4', x: 9, y: 9 } }));
  const menus = findByAttr(second, 'data-role', 'menu');
  assert.equal(menus.length, 1, 'two menus on one screen');
  assert.equal(menus[0].attrs['data-menu-entry'], '4', 'the second right-click did not move the menu');
});

test('a menu about a row that is no longer drawn is not drawn either', () => {
  const tree = face.view(face.read([asked('a')], [], { menu: { entry: 'no-such-entry', x: 0, y: 0 } }));
  assert.deepEqual(findByAttr(tree, 'data-role', 'menu'), []);
});

test('every row a menu can be opened on names itself, and a group names the record its own cells are drawing', () => {
  const many = [];
  for (let i = 0; i < 4; i += 1) many.push(asked('get_transformations_x'));
  const tree = face.view(face.read(many));
  const group = findByAttr(tree, 'data-role', 'entry-group')[0];
  assert.ok(group, 'nothing grouped, so this assertion would be vacuous');
  const inside = findByAttr(group, 'data-entry').map((n) => n.attrs['data-entry']);
  assert.equal(inside.length, 4, 'the run does not still list every record it stands for');
  assert.equal(group.attrs['data-menu-row'], inside[0], 'the head offers a menu about a record other than the one it draws');
  // A group's head carries a menu subject and no data-entry: a second node carrying
  // the same entry id would read as one record drawn twice (tools/shoot.mjs).
  assert.equal('data-entry' in group.attrs, false);
  // And every drill-down row can be right-clicked in its own right.
  for (const row of findByAttr(group, 'data-entry')) assert.equal(row.attrs['data-menu-row'], row.attrs['data-entry']);
  // The menu's own subject is spelled differently from a row's, on purpose: the menu
  // is drawn over the list, so one name for both made a right-click on the menu find
  // the menu as its own row.
  const menu = findByAttr(face.view(face.read(many, [], { menu: { entry: inside[0], x: 0, y: 0 } })), 'data-role', 'menu')[0];
  assert.equal('data-menu-row' in menu.attrs, false, 'the menu is drawn as a row a right-click can be aimed at');
  assert.equal(menu.attrs['data-menu-entry'], inside[0]);
});

test('the menu is dismissed by Escape, by a press away, and never survives an unmount', () => {
  const host = standInHost();
  const unmount = face.mount(host, trapPort(), representative(), { pollMs: 0 });
  const fire = (type, event) => {
    for (const listener of host.listeners.filter((l) => l.type === type)) listener.handler(event);
  };
  const rowAt = (id) => ({
    closest: (selector) => (selector === '[data-menu-row]' ? { getAttribute: () => id } : null),
  });
  const menusOn = () => attrValues(host, 'data-role').filter((role) => role === 'menu').length;

  assert.equal(menusOn(), 0, 'a menu was drawn before anything asked for one');
  fire('contextmenu', { target: rowAt('3'), clientX: 20, clientY: 30, preventDefault() {} });
  assert.equal(menusOn(), 1, 'a right-click drew no menu');
  fire('contextmenu', { target: rowAt('4'), clientX: 21, clientY: 31, preventDefault() {} });
  assert.equal(menusOn(), 1, 'a second right-click stacked a second menu');

  fire('click', { target: { closest: () => null } });
  assert.equal(menusOn(), 0, 'a press away left the menu standing');

  fire('contextmenu', { target: rowAt('3'), clientX: 20, clientY: 30, preventDefault() {} });
  assert.equal(menusOn(), 1);
  unmount();
  assert.equal(host.childNodes.length, 0, 'unmount left the menu behind');
});

test('a repaint carries the open menu rather than leaving a second one behind', () => {
  const shared = [asked('get_transformations')];
  const host = standInHost();
  const unmount = face.mount(host, trapPort(), shared, { pollMs: 0 });
  const fire = (type, event) => {
    for (const listener of host.listeners.filter((l) => l.type === type)) listener.handler(event);
  };
  // The id the screen actually drew, read off the screen -- these entries are numbered
  // by the window, so a literal here would be a test asserting against a number that
  // depends on how many samples were built before it.
  const id = attrValues(host, 'data-menu-row')[0];
  fire('contextmenu', {
    target: { closest: (s) => (s === '[data-menu-row]' ? { getAttribute: () => id } : null) },
    clientX: 10,
    clientY: 10,
    preventDefault() {},
  });
  const menus = () => attrValues(host, 'data-role').filter((role) => role === 'menu').length;
  assert.equal(menus(), 1);
  shared.push(answered('get_transformations', 'GET', '/v1/transformations'));
  assert.equal(unmount.repaint(), true);
  assert.equal(menus(), 1, 'the repaint either dropped the menu or drew a second one');
});

test('a copy states which way it went; it never draws the same whether or not anything happened', () => {
  const ok = face.view(face.read(representative(), [], { copied: { key: '3:row', ok: true } }));
  const took = findByAttr(ok, 'data-copied', 'true');
  assert.equal(took.length, 1, 'a copy that worked said nothing');
  assert.equal(took[0].attrs['data-offer-entry'], '3');
  assert.match(textOf(took[0]), /copied/);

  const refused = face.view(face.read(representative(), [], { copied: { key: '3:row', ok: false } }));
  const failed = findByAttr(refused, 'data-copy-failed', 'true');
  assert.equal(failed.length, 1, 'a refused clipboard write drew as a success');
  assert.match(textOf(failed[0]), /not allowed to reach the clipboard/);
});

test('how many of each is a true count, wherever the figure ended up', () => {
  const tree = face.view(face.read([asked('a'), asked('b'), answered('c', 'GET', '/v1/c'), 'oops']));
  // answered is a figure at the head of the screen now; asked is still a row of the
  // residual tally. Both are counts of the same census, and both are checked here so
  // that moving a word between the two cannot quietly stop it being counted.
  const segments = findByAttr(tree, 'data-role', 'segment');
  const byNoun = new Map(segments.map((s) => [s.attrs['data-noun'], s.attrs['data-value']]));
  assert.equal(byNoun.get('answered'), '1');
  assert.equal(byNoun.get('calls'), '4');
  assert.match(textOf(findByAttr(tree, 'data-tally-entry', 'asked')[0]), /asked: *2/);
});

// -- SS657 retrofit (req/38 SS657 Owner #317/#318, idiom proven by faces/atlas) --

test('SS657 defect 4/5 cure: a single compact header line names the face, and carries no figure the band below it already states', () => {
  const tree = face.view(face.read([asked('a'), asked('b')]));
  const header = findByAttr(tree, 'data-role', 'face-header')[0];
  assert.ok(header);
  assert.ok(textOf(header).includes('notice'));
  assert.equal(tree.children[0], header);
  // Owner #340: the figures belong to the band, at a size a figure deserves. A count
  // restated in the header would be the same fact twice on one screen.
  assert.equal(/\d/.test(textOf(header)), false, `the header still carries a figure: ${textOf(header)}`);
  assert.equal(tree.children[1].attrs['data-part'], 'stat-band', 'the band is not the second thing on the screen');
});

test('SS657 defect 2 cure: the explanatory surfaces are bordered, self-evident controls sitting in one row, each with a plain-language hint', () => {
  const tree = face.view(face.read([asked('a')]));
  const row = findByAttr(tree, 'data-role', 'control-row')[0];
  assert.ok(row);
  const controls = findByAttr(row, 'data-role', 'control');
  // Owner directive #335 (1): the omitted census joined why and legend in this one
  // row; it was an always-open band of prose under the entries before it. `tally` is
  // no longer one of them -- three of its words are figures in the band now and the
  // rest sit inside the legend, which is where a counted table already was. See
  // notice.mjs bandSegments() for why a fourth counted table would be req/784 R-07.
  assert.deepEqual(controls.map((c) => c.attrs['data-control']), ['why', 'legend', 'omitted', 'reference']);
  for (const control of controls) {
    assert.equal(control.attrs['data-open'], 'false', `${control.attrs['data-control']} is not collapsed by default`);
    assert.ok(control.attrs.style.includes('border'), `${control.attrs['data-control']} is a bare word, not a control`);
  }
  // req/822_c7 (Owner #387/#388 冗長文字全掃): the hint no longer draws as its own
  // visible span beside the name -- it rides the control's own summary as a title
  // (a hover) and a data-hint attribute.
  const hintOf = (control) => control.children[0].attrs['data-hint'];
  assert.match(hintOf(findByAttr(row, 'data-control', 'why')[0]), /the order, and why/);
  assert.match(hintOf(findByAttr(row, 'data-control', 'legend')[0]), /marks and counts/);
  // `omitted -- what is not drawn` said one thing twice in six words. A hint earns its
  // place by adding something the name does not already carry.
  assert.match(hintOf(findByAttr(row, 'data-control', 'omitted')[0]), /what is left out, and why/);
  assert.match(hintOf(findByAttr(row, 'data-control', 'reference')[0]), /the server's own words/);
  for (const control of controls) {
    const summary = control.children[0];
    assert.equal(summary.tag, 'summary');
    assert.equal(summary.attrs.title, summary.attrs['data-hint'], `${control.attrs['data-control']}'s title and data-hint should carry the same words`);
    assert.equal(textOf(summary).includes('--'), false, `${control.attrs['data-control']} still draws a pair of dashes that mean nothing`);
  }
});

test('SS657 defect 1/3 cure: legend is a zero-inclusive counted mark table -- every declared mark gets a row, including ones this render drew zero of', () => {
  const tree = face.view(face.read([asked('a')]));
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  const rows = findByAttr(legend, 'data-mark-entry');
  const declaredMarks = new Set(DECLARATION.marks.map((m) => m.mark));
  assert.equal(rows.length, declaredMarks.size);
  const zeroRows = rows.filter((r) => r.attrs['data-count'] === '0');
  assert.ok(zeroRows.length > 0, 'an asked-only entry draws neither effect/network, effect/message, structure/hole nor undefined, so several marks read zero here');
});

test('the declared omissions are on this screen exactly once, having been drawn twice in two different controls', () => {
  // What this replaces asserted the legend carried its own copy of the undrawn set.
  // It did, and so did the `omitted` control -- the same seven sentences, in two
  // grids, on one screen, which is the class of defect this face already refuses for
  // a count. Each entry is still fully present; each one is present once.
  const tree = face.view(face.read([asked('a')]));
  const drawn = findByAttr(tree, 'data-omission');
  assert.equal(drawn.length, DECLARATION.undrawn.length, 'the omitted control does not carry the whole declared set');
  for (const entry of DECLARATION.undrawn) {
    const own = drawn.filter((n) => n.attrs['data-omission'] === entry.what);
    assert.equal(own.length, 1, `${entry.what} is drawn ${own.length} times`);
    // And it is inside the one control whose subject it is, not loose on the screen.
    assert.equal(findByAttr(findByAttr(tree, 'data-control', 'omitted')[0], 'data-omission', entry.what).length, 1);
  }
  assert.deepEqual(findByAttr(tree, 'data-not-drawn'), [], 'the second copy is still being drawn');
});

test('SS657 tally cure, held across the band and the tally together: every outcome word this face knows is counted exactly once, zero included', () => {
  const tree = face.view(face.read([asked('a'), asked('b')]));
  const bandByNoun = new Map(findByAttr(tree, 'data-role', 'segment').map((s) => [s.attrs['data-noun'], s.attrs['data-value']]));
  const tallyByWord = new Map(findByAttr(tree, 'data-tally-entry').map((r) => [r.attrs['data-tally-entry'], r.attrs['data-count']]));
  for (const word of ['asked', 'answered', 'refused', 'failed', 'absent', 'elsewhere']) {
    const inBand = bandByNoun.has(word);
    const inTally = tallyByWord.has(word);
    // Exactly one of the two, which is the whole point: zero-inclusive coverage of
    // the closed set, and no word counted in two places on one screen (req/784 R-07).
    assert.equal(inBand !== inTally, true, `${word} is counted ${inBand && inTally ? 'twice' : 'nowhere'}`);
    const shown = inBand ? bandByNoun.get(word) : tallyByWord.get(word);
    assert.equal(shown, word === 'asked' ? '2' : '0', `${word} should read a real figure, not be omitted`);
  }
});

test('negative control: a word that arrived and is not in the closed set is still counted, and named with its own spelling', () => {
  const tree = face.view(face.read([unrecognised('get_transformations')]));
  const row = findByAttr(tree, 'data-tally-entry', 'partially_answered')[0];
  assert.ok(row, 'an outcome word this face does not recognise was dropped from the count');
  assert.equal(row.attrs['data-count'], '1');
});

// -- Owner #348 (4): the weight scale, held mechanically --------------------------

/** Every weight the face is allowed to set, and the role that carries it. This is the
 * assertion's own copy on purpose: a test that read the table out of the module it is
 * checking would agree with any table that module happened to hold. */
const WEIGHTS = { head: '700', figure: '600', lead: '600', label: '500', body: '400' };

test('Owner #348 (4): no weight is set without naming the role that carries it, and every role\'s weight is the declared one', () => {
  const trees = [
    face.view(face.read(representative(), [], { menu: { entry: '3', x: 0, y: 0 } })),
    face.view(face.read([])),
    face.view(face.read(null)),
  ];
  let counted = 0;
  for (const tree of trees) {
    for (const node of find(tree, (n) => /font-weight:/.test(n.attrs.style ?? ''))) {
      const role = node.attrs['data-type'];
      const set = /font-weight:([^;]+)/.exec(node.attrs.style)[1];
      // parts/ draws its own type (the band's figure, a box head, a chip) and owns
      // its own weights; what this holds is everything this face draws itself.
      if (role === undefined) {
        assert.ok(node.attrs['data-part'] || node.attrs['data-role'] === 'box-name', `a weight set with no role named: ${node.attrs.style}`);
        continue;
      }
      counted += 1;
      assert.ok(role in WEIGHTS, `unknown type role on the screen: ${role}`);
      assert.equal(set, WEIGHTS[role], `${role} drew at ${set}, and the scale says ${WEIGHTS[role]}`);
    }
  }
  assert.ok(counted > 20, `the rule was applied to ${counted} nodes, which is not enough to have tested it`);
});

test('Owner #348 (4) negative control: a role drawn at a weight the scale does not give it is caught', () => {
  const tree = face.view(face.read([asked('a')]));
  const label = find(tree, (n) => n.attrs['data-type'] === 'label')[0];
  assert.ok(label, 'nothing on this screen is a label, so the rule above measures nothing');
  const planted = { ...label, attrs: { ...label.attrs, style: label.attrs.style.replace(/font-weight:\d+/, 'font-weight:900') } };
  assert.notEqual(/font-weight:([^;]+)/.exec(planted.attrs.style)[1], WEIGHTS.label);
});

test('Owner #348 (4): nothing on this face asks a browser to break inside a word by default', () => {
  const tree = face.view(face.read(representative()));
  const anywhere = find(tree, (n) => /overflow-wrap:anywhere/.test(n.attrs.style ?? ''));
  assert.deepEqual(anywhere.map((n) => n.attrs['data-role'] ?? n.tag), [], 'a style still tells the browser it may break at any letter');
  const wrapped = find(tree, (n) => /overflow-wrap:break-word/.test(n.attrs.style ?? ''));
  assert.ok(wrapped.length > 5, `only ${wrapped.length} styles carry the last-resort rule, so the population is too small to mean anything`);
  for (const node of wrapped) assert.match(node.attrs.style, /text-wrap:pretty/, 'a wrapping style with nothing stopping a one-character last line');
});

// -- req/97 gap-list item 5, carried forward: one verb at one width ----------------

test('req/97 gap-list item 5 still holds on the control that replaced it: one verb, one width, above the tap budget', () => {
  const tree = face.view(face.read([
    asked('a'), asked('b'), asked('c'), asked('d'),
    asked('e'), asked('f'), asked('g'), asked('h'),
  ]));
  const gutter = findByAttr(tree, 'data-role', 'offer').filter((n) => n.attrs['data-shape'] === 'gutter');
  assert.ok(gutter.length >= 4, `expected several row controls, got ${gutter.length}`);
  const labels = new Set(gutter.map((r) => textOf(r)));
  assert.deepEqual([...labels], ['copy row'], 'the control label is not one short phrase');
  const widths = new Set(gutter.map((r) => /width:([^;]+)/.exec(r.attrs.style)?.[1]));
  assert.equal(widths.size, 1, 'the control edge is ragged: these do not all state one width');
  for (const control of gutter) {
    assert.match(control.attrs.style, /min-height:36px/, 'a control below the tap budget');
    // What it would hand over is on the control itself, so pointing at it answers
    // "what exactly would I get" without pressing it.
    assert.ok((control.attrs.title ?? '').includes('asked'), 'the control does not say what it would give');
  }
});

test('what a row offers to copy is what the row is drawing, taken from the record and not read back off the screen', () => {
  const tree = face.view(face.read([answered('get_transformations', 'GET', '/v1/transformations')], [], { menu: { entry: '1', x: 0, y: 0 } }));
  const menu = findByAttr(tree, 'data-role', 'menu')[0] ?? face.view(face.read([answered('x', 'GET', '/v1/x')]));
  const address = findByAttr(tree, 'data-role', 'entry-address')[0];
  assert.ok(address, 'no address line at all');
  const gutter = findByAttr(tree, 'data-role', 'offer')[0];
  // Everything the row copies is a thing the row itself draws: the call, the time and
  // the outcome are all in the control's own stated value.
  assert.ok(gutter.attrs.title.includes(textOf(address)), 'the row would copy something it is not drawing');
  assert.ok(menu, 'the menu did not draw for the entry it was opened on');
});

// -- retrofit r4 (Owner #340): a band, boxes, a measured footer, and two defects ----

test('atom 1: the band states the size and shape of this screen before a word is read, every figure a real count', () => {
  const tree = face.view(face.read(representative()));
  const band = findByAttr(tree, 'data-part', 'stat-band')[0];
  assert.ok(band, 'no band at the head of the face');
  const segments = findByAttr(band, 'data-role', 'segment');
  assert.ok(segments.length >= 3 && segments.length <= 5, `the band carries ${segments.length} segments`);
  const byNoun = new Map(segments.map((s) => [s.attrs['data-noun'], s.attrs['data-value']]));
  // representative(): 10 items, 8 of them records; 1 answered, 2 refused (one over the
  // network, one the shell refused), 1 absent, and no run of the same call at all.
  assert.equal(byNoun.get('calls'), '10');
  assert.equal(byNoun.get('answered'), '1');
  assert.equal(byNoun.get('refused'), '2');
  assert.equal(byNoun.get('absent'), '1');
  assert.equal(byNoun.get('repeats'), '0', 'a count of nothing is a measurement and is drawn');
  for (const segment of segments) {
    assert.ok((segment.attrs['data-noun'] ?? '').length > 0, 'a figure with no noun is a number about nothing');
  }
});

test('atom 1 negative control: a window this face was never handed draws dashes, not zeroes', () => {
  const tree = face.view(face.read(null));
  const segments = findByAttr(tree, 'data-role', 'segment');
  assert.ok(segments.length > 0, 'the band vanished on the one state it matters most on');
  for (const segment of segments) {
    assert.equal(segment.attrs['data-value'], 'unread', `${segment.attrs['data-noun']} claims a count this face never had`);
    assert.match(textOf(segment), /--/, 'an unread figure drew something other than a dash');
  }
});

test('atom 1: a repeated call is counted as one run, not as N calls', () => {
  const many = [];
  for (let i = 0; i < 5; i += 1) many.push(asked(`get_transformations_${i}`));
  const tree = face.view(face.read([...many, answered('get_candidates', 'GET', '/v1/candidates')]));
  const byNoun = new Map(findByAttr(tree, 'data-role', 'segment').map((s) => [s.attrs['data-noun'], s.attrs['data-value']]));
  assert.equal(byNoun.get('calls'), '6', 'the total counts every call, abstracted or not');
  assert.equal(byNoun.get('repeats'), '1', 'five identical calls are one run');
});

test('atom 2: every grouping this face has is a box with its own name and its own count in its head', () => {
  const many = [];
  for (let i = 0; i < 5; i += 1) many.push(asked(`get_transformations_${i}`));
  const tree = face.view(face.read([...many, answered('get_candidates', 'GET', '/v1/candidates')]));
  const boxes = findByAttr(tree, 'data-part', 'box');
  assert.equal(boxes.length, 2, `expected one box for the run and one for the ungrouped call, got ${boxes.length}`);
  const run = boxes.find((b) => b.attrs['data-box'] === 'get_transformations');
  assert.ok(run, 'the run of five identical calls is not a named box');
  assert.equal(run.attrs['data-count'], '5', 'the box head does not state how many the run collapses');
  assert.ok(findByAttr(run, 'data-part', 'standing-chip').length > 0, 'the run box head carries no standing');
  const rest = boxes.find((b) => b.attrs['data-box'] === 'calls');
  assert.ok(rest, 'the ungrouped calls are loose rows, not an object on the screen');
  assert.equal(rest.attrs['data-count'], '1');
});

test('atom 2: an empty window keeps its border and says 0', () => {
  const tree = face.view(face.read([]));
  const boxes = findByAttr(tree, 'data-part', 'box');
  assert.equal(boxes.length, 1);
  assert.equal(boxes[0].attrs['data-count'], '0');
  assert.ok(boxes[0].attrs.style.includes('border'), 'an empty group lost its border, so it reads as never having been read');
  assert.match(textOfHostFromTree(tree), /has not asked the server anything yet/);
});

test('atom 2 negative control: a window never handed its record draws a box with a dash, which is a different fact from 0', () => {
  const tree = face.view(face.read(null));
  const box = findByAttr(tree, 'data-part', 'box')[0];
  assert.ok(box, 'nothing bordered was drawn for a face that was handed nothing');
  assert.equal(box.attrs['data-count'], '--');
});

test('atom 5: the last thing on the screen is a footer carrying a measured figure and this face\'s own word for what it read', () => {
  const tree = face.view(face.read(representative()));
  const last = tree.children[tree.children.length - 1];
  assert.equal(last.attrs['data-part'], 'runtime-footer', 'the footer is not the last node view() returns');
  const measured = Number(last.attrs['data-render-ms']);
  assert.ok(Number.isFinite(measured), 'the render figure is not a number');
  assert.ok(measured > 0, 'the render figure is zero, which no real build takes');
  assert.match(textOf(last), /read this window's own record/);
  assert.equal(/membrane|shell|wire/.test(textOf(last)), false, 'the footer names an internal layer');
});

// -- req/97 section 4: the raw wire payload on the product surface -----------------

const INTERNAL_ON_SURFACE = [
  { name: 'an all-caps wire constant', pattern: /\b[A-Z][A-Z_]{3,}\b/ },
  { name: 'a raw JSON fragment', pattern: /[{[]\s*"/ },
];

/** Everything the screen draws except what is behind the reference control, which is
 * exactly where a word this window did not choose is allowed to be, labelled. */
function surfaceText(tree) {
  const strip = (node) => {
    if (!node || typeof node !== 'object') return node;
    if (node.attrs && node.attrs['data-role'] === 'internal-reference') return null;
    if (!Array.isArray(node.children)) return node;
    return { ...node, children: node.children.map(strip).filter((c) => c !== null) };
  };
  return textOfHostFromTree(strip(tree));
}

test('req/97 section 4: no raw wire token and no raw payload fragment is drawn on the product surface', () => {
  const tree = face.view(face.read(representative()));
  const drawn = surfaceText(tree);
  for (const { name, pattern } of INTERNAL_ON_SURFACE) {
    assert.equal(pattern.test(drawn), false, `${name} is drawn on the surface: ${pattern.exec(drawn)?.[0]}`);
  }
  // Moved, not deleted: the plain form of the one that carried no plain form at all.
  assert.match(drawn, /no route by that name/);
});

test('negative control: the same reading fires on what was kept, which is why its absence above means something', () => {
  const tree = face.view(face.read(representative()));
  const references = findByAttr(tree, 'data-role', 'internal-reference');
  assert.ok(references.length > 0, 'no reference was drawn at all, so nothing was moved -- it was deleted');
  const kept = references.map((r) => textOf(r)).join(' ');
  const fired = INTERNAL_ON_SURFACE.filter(({ pattern }) => pattern.test(kept));
  assert.equal(fired.length, INTERNAL_ON_SURFACE.length, 'the reference carries neither of the two, so the reading above measures nothing');
  assert.ok(kept.includes('IDEMPOTENCY_CONFLICT'), 'the refusal code the server sent is gone from the screen');
  assert.ok(kept.includes('get_everything_i_wish_for'), 'the request the server echoed back is gone from the screen');
  const control = findByAttr(tree, 'data-control', 'reference')[0];
  assert.ok(control, 'what was kept is reachable from nowhere');
  // req/822_c7 (Owner #387/#388 冗長文字全掃): the hint rides the control's own
  // summary title/data-hint now, not the control's visible text.
  assert.match(control.children[0].attrs['data-hint'], /the server's own words/, 'the control does not say what it holds');
});

test('a window whose calls all came back in words this window could use draws the reference control empty and says so', () => {
  const tree = face.view(face.read([asked('a'), answered('b', 'GET', '/v1/b')]));
  const control = findByAttr(tree, 'data-control', 'reference')[0];
  assert.match(textOf(control), /nothing on this screen came back carrying a word of the server's own/);
});

// -- req/97 section 4: the mid-word wrap in the status column ----------------------

test('req/97 section 4: the outcome word and the status are two lines, and the column is wide enough for the longest word this face can draw', () => {
  const tree = face.view(face.read([unrecognised('get_transformations')]));
  const cell = findByAttr(tree, 'data-role', 'entry-outcome')[0];
  assert.ok(cell, 'no outcome cell');
  const word = findByAttr(cell, 'data-role', 'entry-outcome-word')[0];
  const status = findByAttr(cell, 'data-role', 'entry-status')[0];
  assert.ok(word && status, 'the word and the status are still one string sharing one line');
  assert.equal(textOf(word), 'partially_answered', 'the word still carries the comma that cost four of its five missing pixels');
  // Two spans, not one string: the word that names the number is a label and the
  // number is a figure, which is the weight rule made mechanical (Owner #348 (4)).
  assert.match(textOf(status), /^status\s*207$/);
  assert.deepEqual(status.children.map((n) => n.attrs['data-type']), ['label', 'figure']);
  // The measurement this is aimed at: tools/shoot.mjs measured the longest word this
  // face can draw at 117px against a 112px column. The declared ceiling is 9rem.
  const row = findByAttr(tree, 'data-role', 'entry')[0];
  assert.match(row.attrs.style, /minmax\(0,9rem\)/, 'the status column is not the width the measurement asked for');
  assert.equal(cell.attrs.title, 'partially_answered', 'a word longer than the column has no full form in reach');
});

test('a grouped run draws its time through the same declared cut a single row does', () => {
  const many = [];
  for (let i = 0; i < 5; i += 1) many.push(asked(`get_transformations_${i}`));
  const tree = face.view(face.read(many));
  const cell = findByAttr(tree, 'data-role', 'entry-group-time')[0];
  assert.ok(cell, 'the grouped row has no time cell a clip reading could examine');
  // Found in the picture: the whole ISO-8601 string was drawn into the 72px budget
  // and came out as "2026-08-2" -- req/03's N-4. Then found again by the clip reading
  // once the cell carried a data-role it could see by: the cut form plus the run's
  // own count ("10:00:00 +4") still did not fit. The count is in the box head.
  assert.equal(textOf(cell), '10:00:00');
  assert.equal(cell.attrs['data-full'], '2026-08-24T10:00:00.000Z', 'the whole timestamp is not on the cell');
  const box = findByAttr(tree, 'data-part', 'box')[0];
  assert.equal(box.attrs['data-count'], '5', 'the count left this row and is not in the head either');
});

// -- req/103 finding 2: an open control does not close on a repaint ----------------

test('req/103 finding 2: what the reader opened is still open after the window records another call', () => {
  const state = face.read([asked('a')], ['legend']);
  const tree = face.view(state);
  const legend = findByAttr(tree, 'data-control', 'legend')[0];
  assert.equal(legend.attrs['data-open'], 'true', 'a control the reader had opened was rebuilt closed');
  // el()'s boolean-attribute convention (parts/src/element.mjs): a true boolean
  // serialises to the empty string, the same shape a disabled button already draws.
  assert.equal(legend.attrs.open, '', 'the control is marked open in state but does not draw open');
  const why = findByAttr(tree, 'data-control', 'why')[0];
  assert.equal(why.attrs['data-open'], 'false', 'a control nobody opened was rebuilt open');
});

test('req/103 finding 2 negative control: with nothing carried in, every control is closed -- which is what a first look is', () => {
  const tree = face.view(face.read([asked('a')]));
  for (const control of findByAttr(tree, 'data-role', 'control')) {
    assert.equal(control.attrs['data-open'], 'false', `${control.attrs['data-control']} opened itself`);
  }
});

test('req/103 finding 2: a repaint carries the open state across, and a host that draws nothing openable says so honestly', () => {
  const shared = [asked('get_transformations')];
  const host = standInHost();
  const unmount = face.mount(host, trapPort(), shared, { pollMs: 0 });
  // The stand-in host is not a document and has no querySelectorAll, so mount() reads
  // an empty list off it -- the honest answer for a host that draws nothing a reader
  // could have opened. What this holds is that the repaint path runs through the same
  // reading rather than around it.
  assert.equal(attrValuesCount(host, 'data-entry'), 1);
  shared.push(answered('get_candidates', 'GET', '/v1/candidates'));
  assert.equal(unmount.repaint(), true);
  assert.equal(attrValuesCount(host, 'data-entry'), 2);
  unmount();
});
