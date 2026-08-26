// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { el, text, style, toHtml, walk, textOf, find, findByAttr, isText, render, ELEMENT_MESSAGES } from '../src/element.mjs';

test('a tag is required, because an element without one cannot be drawn or found', () => {
  assert.throws(() => el(''), new RegExp(ELEMENT_MESSAGES.TAG_REQUIRED));
  assert.throws(() => el(null), new RegExp(ELEMENT_MESSAGES.TAG_REQUIRED));
});

test('attributes that are absent are dropped rather than printed empty', () => {
  const node = el('span', { title: null, hidden: undefined, 'data-x': 'kept', 'data-off': false });
  assert.deepEqual(Object.keys(node.attrs), ['data-x']);
  assert.equal(toHtml(node), '<span data-x="kept"></span>');
});

test('a boolean attribute set true prints as the empty attribute html expects', () => {
  assert.equal(toHtml(el('details', { open: true })), '<details open=""></details>');
});

test('text and attribute values are escaped, so a path cannot close a tag', () => {
  const node = el('span', { title: 'a "quoted" <thing> & more' }, ['</span><script>x</script>']);
  const html = toHtml(node);
  assert.equal(html.includes('<script>'), false);
  assert.equal(html.includes('&quot;'), true);
  assert.equal(html.includes('&lt;/span&gt;'), true);
});

test('svg children serialise self-closing, which the html parser accepts in foreign content', () => {
  assert.equal(toHtml(el('use', { href: '#gx-verdict-Admit' })), '<use href="#gx-verdict-Admit"/>');
  assert.equal(toHtml(el('br')), '<br>');
});

test('a child that is neither an element nor text is refused rather than skipped', () => {
  assert.throws(() => el('div', {}, [42]), new RegExp(ELEMENT_MESSAGES.BAD_CHILD));
  assert.throws(() => el('div', {}, [{ nope: true }]), new RegExp(ELEMENT_MESSAGES.BAD_CHILD));
});

test('nothing is not a child: null and undefined drop out, they do not become "null"', () => {
  const node = el('div', {}, ['a', null, undefined, false, 'b']);
  assert.equal(node.children.length, 2);
  assert.equal(textOf(node), 'ab');
});

test('style pairs assemble without a caller writing punctuation', () => {
  assert.equal(style({ color: 'var(--ink)', gap: null, height: 'var(--row)' }), 'color:var(--ink);height:var(--row)');
});

test('walk reaches every node, parents before children', () => {
  const tree = el('a', {}, [el('b', {}, ['x']), el('c', {}, [el('d', {}, ['y'])])]);
  const seen = [];
  walk(tree, (n) => seen.push(isText(n) ? `#${n.text}` : n.tag));
  assert.deepEqual(seen, ['a', 'b', '#x', 'c', 'd', '#y']);
});

test('find and findByAttr answer over the whole tree', () => {
  const tree = el('div', {}, [el('span', { 'data-role': 'word' }, ['Deny']), el('span', {}, ['x'])]);
  assert.equal(find(tree, (n) => n.tag === 'span').length, 2);
  assert.equal(findByAttr(tree, 'data-role', 'word').length, 1);
  assert.equal(findByAttr(tree, 'data-role').length, 1);
});

test('render refuses a document that cannot make elements, instead of failing halfway down a tree', () => {
  assert.throws(() => render(null, el('div')), new RegExp(ELEMENT_MESSAGES.RENDER_NEEDS_DOCUMENT));
  assert.throws(() => render({}, el('div')), new RegExp(ELEMENT_MESSAGES.RENDER_NEEDS_DOCUMENT));
});

test('text() coerces once and keeps what it was given', () => {
  assert.deepEqual(text(7), { text: '7' });
  assert.equal(textOf(el('p', {}, [text('a'), 'b'])), 'ab');
});
