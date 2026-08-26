// SPDX-License-Identifier: Apache-2.0
// The stage: a tree of splits and leaves, and nothing that changes one in place.
//
// Every function here answers with a new tree. That is not a taste in immutability, it
// is what makes an inverse constructible at all: an operation that overwrote the node it
// edited would have to be told what the node used to be, and the only honest way to be
// told is to have kept it -- which is the whole state, which is the cost we refuse.
// Keeping the removed branch alone is 75 bytes of the thing that changed; keeping the
// state is thousands of bytes of the things that did not.
//
// A path is an array of child indices from the root. It names a place without naming a
// face, which is why the shell can move a pane it cannot identify.

/** Ratios are stored already rounded, so serialise(parse(x)) can be the identity. */
export const RATIO_PLACES = 4;
const SCALE = 10 ** RATIO_PLACES;

/** The smallest share a pane may be pushed to before the shell refuses instead of vanishing it. */
export const MIN_RATIO = 0.05;

export function normalise(ratios) {
  const total = ratios.reduce((a, b) => a + b, 0);
  if (!(total > 0)) throw new RangeError('ratios must sum to something positive');
  const scaled = ratios.map((r) => Math.round((r / total) * SCALE));
  const drift = SCALE - scaled.reduce((a, b) => a + b, 0);
  scaled[scaled.length - 1] += drift;
  return Object.freeze(scaled.map((v) => v / SCALE));
}

export const leaf = (tabs = [], active = 0) => Object.freeze({
  k: 'l',
  tabs: Object.freeze([...tabs]),
  active: tabs.length === 0 ? 0 : Math.min(Math.max(0, active), tabs.length - 1),
});

export const split = (axis, kids, ratios) => {
  if (axis !== 'row' && axis !== 'col') throw new TypeError(`axis must be row or col, not ${axis}`);
  if (kids.length < 2) throw new RangeError('a split holds two children or more');
  return Object.freeze({
    k: 's',
    axis,
    kids: Object.freeze([...kids]),
    ratios: normalise(ratios ?? kids.map(() => 1)),
  });
};

export const isLeaf = (n) => n.k === 'l';

export function at(node, path) {
  let here = node;
  for (const step of path) {
    if (isLeaf(here)) throw new RangeError(`path runs past a leaf at step ${step}`);
    here = here.kids[step];
    if (here === undefined) throw new RangeError(`no child ${step}`);
  }
  return here;
}

/** Replace one node by another and rebuild only the spine above it. */
export function replaceAt(node, path, made) {
  if (path.length === 0) return made;
  const [step, ...rest] = path;
  if (isLeaf(node)) throw new RangeError('cannot descend into a leaf');
  const kids = [...node.kids];
  kids[step] = replaceAt(kids[step], rest, made);
  return Object.freeze({ ...node, kids: Object.freeze(kids) });
}

export function leafPaths(node, path = []) {
  if (isLeaf(node)) return [Object.freeze([...path])];
  return node.kids.flatMap((kid, i) => leafPaths(kid, [...path, i]));
}

export function splitPaths(node, path = []) {
  if (isLeaf(node)) return [];
  return [Object.freeze([...path]), ...node.kids.flatMap((kid, i) => splitPaths(kid, [...path, i]))];
}

/**
 * Divide a leaf. A sibling is inserted when the parent already runs along this axis,
 * which is what keeps three panes in a row from becoming a row holding a row.
 * The tree is rebuilt, never mutated, so `undo` only has to remember the node that stood
 * where the new one now stands.
 */
export function divide(root, path, axis, made = leaf()) {
  const target = at(root, path);
  if (!isLeaf(target)) throw new RangeError('only a leaf can be divided');
  if (path.length > 0) {
    const parentPath = path.slice(0, -1);
    const index = path[path.length - 1];
    const parent = at(root, parentPath);
    if (parent.axis === axis) {
      const kids = [...parent.kids];
      kids.splice(index + 1, 0, made);
      const share = parent.ratios[index] / 2;
      const ratios = [...parent.ratios];
      ratios[index] = share;
      ratios.splice(index + 1, 0, share);
      return replaceAt(root, parentPath, split(axis, kids, ratios));
    }
  }
  return replaceAt(root, path, split(axis, [target, made], [1, 1]));
}

/**
 * Remove a leaf. A split left holding one child is replaced *by that child* rather than
 * rewritten to look like it: a rewrite loses the child's identity, and every path anyone
 * still holds into it becomes a path to a stranger.
 */
export function drop(root, path) {
  if (path.length === 0) throw new RangeError('the last pane cannot be dropped');
  const parentPath = path.slice(0, -1);
  const index = path[path.length - 1];
  const parent = at(root, parentPath);
  const kids = [...parent.kids];
  const ratios = [...parent.ratios];
  kids.splice(index, 1);
  ratios.splice(index, 1);
  const made = kids.length === 1 ? kids[0] : split(parent.axis, kids, ratios);
  return replaceAt(root, parentPath, made);
}

export function setRatios(root, path, ratios) {
  const target = at(root, path);
  if (isLeaf(target)) throw new RangeError('a leaf has no ratios');
  if (ratios.length !== target.kids.length) throw new RangeError('one ratio per child');
  const made = normalise(ratios);
  if (made.some((r) => r < MIN_RATIO)) {
    throw new RangeError(`a pane may not be pushed below ${MIN_RATIO} of its split`);
  }
  return replaceAt(root, path, split(target.axis, target.kids, made));
}

export const withTabs = (node, tabs, active) => {
  if (!isLeaf(node)) throw new RangeError('tabs live on leaves');
  return leaf(tabs, active);
};

export function countLeaves(node) {
  return isLeaf(node) ? 1 : node.kids.reduce((n, kid) => n + countLeaves(kid), 0);
}

export const samePath = (a, b) => a.length === b.length && a.every((v, i) => v === b[i]);
