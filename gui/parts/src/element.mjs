// SPDX-License-Identifier: Apache-2.0
// The substrate every drawing part is built on: a part returns a tree, it does not
// touch a document.
//
// Why a tree and not DOM calls. The two defects this rebuild exists to not repeat
// (req/03 N-1, N-2) were both invisible to the unit tests that guarded them, because
// what the tests could reach was a DOM built by a fake and what shipped was a DOM
// built by a browser. Here a part decides a tree, `toHtml` serialises that same tree
// into the fixture a real browser draws, and `render` mounts that same tree into a
// live document. There is one description and three consumers, so a test that reads
// the tree is reading the thing that was photographed -- no stand-in document exists
// anywhere in this package.

export const ELEMENT_MESSAGES = {
  TAG_REQUIRED: 'an element needs a tag name',
  BAD_CHILD: 'a child is an element node, a text node, or nothing',
  RENDER_NEEDS_DOCUMENT: 'render needs a document that can create elements',
};

const SVG_NS = 'http://www.w3.org/2000/svg';

// Tags that carry no children in HTML serialisation. Everything inside <svg> is
// serialised self-closing instead, which the HTML parser accepts in foreign content.
// `wbr` joins the set for the path cell: it is the one element that offers a browser a
// place it MAY break, without putting a character into the value. A path has no break
// opportunity any browser recognises, so `overflow-wrap` broke it wherever the box ran
// out -- mid-token, in the middle of a filename. Marking the breaks explicitly is the
// only cure that does not corrupt the value, which rules out injecting a zero-width
// space: that would end up in what a reader copies.
const VOID_TAGS = new Set(['br', 'hr', 'img', 'input', 'link', 'meta', 'wbr']);
const SVG_TAGS = new Set([
  'svg', 'symbol', 'path', 'use', 'desc', 'title', 'g', 'rect', 'circle', 'line', 'metadata',
]);

/** An element node. `attrs` values of null/undefined are dropped, never printed. */
export function el(tag, attrs = {}, children = []) {
  if (typeof tag !== 'string' || tag.length === 0) throw new Error(ELEMENT_MESSAGES.TAG_REQUIRED);
  const kept = {};
  for (const [name, value] of Object.entries(attrs)) {
    if (value === null || value === undefined || value === false) continue;
    kept[name] = value === true ? '' : String(value);
  }
  const list = (Array.isArray(children) ? children : [children]).filter((c) => c !== null && c !== undefined && c !== false);
  for (const child of list) {
    const ok = typeof child === 'string' || (child && (typeof child.tag === 'string' || typeof child.text === 'string'));
    if (!ok) throw new Error(ELEMENT_MESSAGES.BAD_CHILD);
  }
  return { tag, attrs: kept, children: list.map((c) => (typeof c === 'string' ? text(c) : c)) };
}

export function text(value) {
  return { text: String(value) };
}

/** A style attribute assembled from pairs, so no part writes a semicolon by hand. */
export function style(pairs) {
  return Object.entries(pairs)
    .filter(([, v]) => v !== null && v !== undefined)
    .map(([k, v]) => `${k}:${v}`)
    .join(';');
}

export function isText(node) {
  return Boolean(node) && typeof node.text === 'string';
}

/** Depth-first, parents before children. */
export function walk(node, visit, parent = null) {
  visit(node, parent);
  if (isText(node)) return;
  for (const child of node.children) walk(child, visit, node);
}

/** Every string a person could read, in document order. */
export function textOf(node) {
  let out = '';
  walk(node, (n) => { if (isText(n)) out += n.text; });
  return out;
}

export function find(node, predicate) {
  const hits = [];
  walk(node, (n) => { if (!isText(n) && predicate(n)) hits.push(n); });
  return hits;
}

export function findByAttr(node, name, value) {
  return find(node, (n) => (value === undefined ? name in n.attrs : n.attrs[name] === value));
}

const ESCAPE_TEXT = { '&': '&amp;', '<': '&lt;', '>': '&gt;' };
const ESCAPE_ATTR = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' };

const escapeText = (s) => s.replace(/[&<>]/g, (c) => ESCAPE_TEXT[c]);
const escapeAttr = (s) => s.replace(/[&<>"]/g, (c) => ESCAPE_ATTR[c]);

export function toHtml(node) {
  if (isText(node)) return escapeText(node.text);
  const attrs = Object.entries(node.attrs).map(([k, v]) => ` ${k}="${escapeAttr(v)}"`).join('');
  const inner = node.children.map(toHtml).join('');
  if (VOID_TAGS.has(node.tag)) return `<${node.tag}${attrs}>`;
  if (SVG_TAGS.has(node.tag) && inner === '') return `<${node.tag}${attrs}/>`;
  return `<${node.tag}${attrs}>${inner}</${node.tag}>`;
}

/**
 * Mount the same tree into a live document. The namespace is decided by the tag, so
 * a caller cannot forget it -- the unsized glyph of N-2 came from an svg built by
 * hand in a second place, and there is no second place here.
 */
export function render(doc, node) {
  if (!doc || typeof doc.createElement !== 'function') throw new Error(ELEMENT_MESSAGES.RENDER_NEEDS_DOCUMENT);
  if (isText(node)) return doc.createTextNode(node.text);
  const svg = SVG_TAGS.has(node.tag);
  const element = svg ? doc.createElementNS(SVG_NS, node.tag) : doc.createElement(node.tag);
  for (const [name, value] of Object.entries(node.attrs)) element.setAttribute(name, value);
  for (const child of node.children) element.appendChild(render(doc, child));
  return element;
}

/**
 * The one reading a tree cannot hold: whether what was mounted is actually cutting
 * a value off, asked of the renderer after it has laid the tree out.
 *
 * It lives here, in the door, for the reason every other document reach in this
 * package lives here (parts/test/boundary.test.mjs holds the population of modules
 * that may touch a document at two): a drawing part decides a tree and knows
 * nothing about a document, and this is not a drawing decision -- it is the same
 * "put this in front of a real renderer and ask it" move `render` already is, one
 * step later in the same sequence.
 *
 * It is deliberately generic and deliberately closure-free. Generic, because
 * "a disclosure whose own summary is overflowing should be open" is not knowledge
 * about receipts -- the caller names its own fold and cell selectors. Closure-free,
 * because a face's fixture writer serialises this function into a static page with
 * Function.prototype.toString() rather than keeping a hand-written copy of it, and
 * a hand-written copy is the thing that drifts.
 *
 * It only ever opens. A reader who shut a row by hand is never overruled by it,
 * and running it twice is running it once.
 */
export function openWhereClipped(root, spec) {
  if (!root || typeof root.querySelectorAll !== 'function') return 0;
  const foldSelector = spec && spec.fold;
  const cellSelector = (spec && spec.cell) || '[data-cell]';
  const because = (spec && spec.because) || 'measured-clip';
  if (!foldSelector) return 0;
  let opened = 0;
  for (const fold of root.querySelectorAll(foldSelector)) {
    if (fold.open) continue;
    const summary = fold.querySelector('summary');
    if (!summary) continue;
    let cut = false;
    for (const cell of summary.querySelectorAll(cellSelector)) {
      if ((cell.textContent || '').trim() === '') continue;
      if (cell.scrollWidth > cell.clientWidth + 1) { cut = true; break; }
    }
    if (!cut) continue;
    fold.open = true;
    fold.setAttribute('data-open', 'true');
    fold.setAttribute('data-open-because', because);
    opened += 1;
  }
  return opened;
}

/**
 * A node a document is to carry exactly once, put there once.
 *
 * glyph-sheet.mjs's installSheet() has done this for the sprite since it was written;
 * parts/src/surface.mjs now needs the same thing for its rule set, and the alternative
 * was a third module in this package reaching a document. That population is held at
 * two by parts/test/boundary.test.mjs on purpose -- "deciding parts may not reach a
 * document, drawing parts may", with the two that do named so the permission cannot
 * spread unnoticed -- so the reach lives here, in the door, and the caller passes a
 * tree and an id.
 *
 * Idempotent per document by that id: a second face mounted into the same page finds
 * what the first one left and installs nothing.
 */
export function installOnce(doc, render, node, id, { into = 'body' } = {}) {
  if (!doc || typeof doc.getElementById !== 'function') return { installed: false, why: ELEMENT_MESSAGES.RENDER_NEEDS_DOCUMENT, node: null };
  const standing = doc.getElementById(id);
  if (standing) return { installed: false, why: 'already in this document', node: standing };
  const made = render(doc, node);
  const host = into === 'head' ? (doc.head ?? doc.body) : doc.body;
  if (!host) return { installed: false, why: ELEMENT_MESSAGES.RENDER_NEEDS_DOCUMENT, node: null };
  host.appendChild(made);
  return { installed: true, why: 'installed', node: made };
}
