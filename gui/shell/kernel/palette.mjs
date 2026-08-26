// SPDX-License-Identifier: Apache-2.0
// Search, as addresses rather than as scroll positions. req/811 §8-5.
//
// The ruling this implements has two halves and the second one is the interesting one.
//
// First: no permanent search bar. A bar spends chrome characters continuously for a
// capability used intermittently, which fails the density axis on every frame where
// nobody is searching. An invoked palette costs nothing at rest.
//
// Second, and this is where we pass the reference tool rather than copy it: **a result is
// an ADDRESS**. Every hit carries the `gx` line that reproduces it, because every view in
// this application is already addressable and `kernel/command.mjs` already writes those
// lines. The reference tool cannot do this at any level of effort -- its views are not
// addressable, so a result there can only ever be a place on a screen that stops meaning
// anything the moment the screen changes. This is an existing capability composed with
// search, not a new mechanism, which is also why this module is thirty lines of matching
// and no machinery.
//
// The grammar is faceted so a query names its axis instead of guessing at it. The three
// facet names are fixed by the ruling; what each one means HERE is written down below
// rather than assumed, because a facet whose meaning is folded into the reader's head is
// a facet that means something slightly different to every reader.
//
// This module holds no DOM and no clipboard. Pure in, pure out, like `command.mjs`.

import { isLeaf } from './tree.mjs';
import { DOCK_SIDES } from './layout.mjs';
import { commandFor } from './command.mjs';

/**
 * The three axes, and what each names in this application.
 *
 * `box:`   the container a face stands in -- stage, left, right, bottom, or nowhere.
 *          The standing column's own vocabulary (req/811 §4-1), so the palette and the
 *          sidebar are asking one question in one language.
 * `phase:` the space, because the spaces in this shell ARE the phases of the work and
 *          are named for them (`verify`, `inspect`) -- req/811 §4 candidate C records
 *          that the pipeline verb is already the rail-top axis at the right coarseness.
 * `ext:`   the face itself: in this application a face is the unit that gets installed,
 *          declared and drawn, so the extension axis is the face id.
 */
export const FACETS = Object.freeze({
  box: 'where a face stands: stage, left, right, bottom, nowhere',
  phase: 'which space: this shell names its spaces for the phase of work they carry',
  ext: 'which face, by the id it is installed under',
});

export const PALETTE_SAID = Object.freeze({
  empty: 'nothing has been typed, so nothing has been searched for',
  none: (query) => `nothing here matches ${query}`,
  facetUnknown: (name) => `there is no ${name}: axis; this palette knows ${Object.keys(FACETS).join(', ')}`,
});

/**
 * Split a query into its facets and its loose words.
 * `box:right ledger` -> { facets: { box: 'right' }, words: ['ledger'], unknown: [] }
 */
export function parseQuery(query) {
  const facets = {};
  const words = [];
  const unknown = [];
  for (const token of String(query ?? '').trim().split(/\s+/).filter(Boolean)) {
    const hit = /^([a-z]+):(.*)$/i.exec(token);
    if (!hit) { words.push(token.toLowerCase()); continue; }
    const [, name, value] = hit;
    const axis = name.toLowerCase();
    // An axis this palette does not have is said, never quietly treated as a word --
    // silently searching for the literal text "box:rihgt" is how a typo becomes "no
    // results" and a person concludes the thing they wanted is not there.
    if (!(axis in FACETS)) { unknown.push(axis); continue; }
    facets[axis] = value.toLowerCase();
  }
  return { facets, words, unknown };
}

/**
 * Everything this window can be asked about, as rows, each carrying its own address.
 *
 * Built from the state rather than from a list somebody maintains: a face that is placed
 * somewhere appears at that placement, and a face that is placed nowhere appears as
 * itself with no address, which is a truthful row and not a hidden one.
 *
 * @param {object} read   the manifest read
 * @param {object} state  the shell state
 */
export function corpusOf(read, state) {
  const rows = [];
  state.spaces.forEach((space, index) => {
    for (const side of DOCK_SIDES) {
      space.docks[side].faces.forEach((id, at) => {
        const face = read.byId?.get(id);
        rows.push({
          id,
          title: face?.title ?? id,
          box: side,
          phase: space.name,
          spaceIndex: index,
          address: commandFor('dock', { index, side, at, id }),
          land: { verb: 'dock:go', args: { index, side, at } },
        });
      });
    }
    const walk = (node, path) => {
      if (isLeaf(node)) {
        node.tabs.forEach((id, at) => {
          const face = read.byId?.get(id);
          rows.push({
            id,
            title: face?.title ?? id,
            box: 'stage',
            phase: space.name,
            spaceIndex: index,
            address: commandFor('stage', { index, path, at, id }),
            land: { verb: 'tab:go', args: { index, path, at } },
          });
        });
        return;
      }
      node.kids.forEach((kid, at) => walk(kid, [...path, at]));
    };
    walk(space.stage, []);
  });

  // Faces that stand nowhere in any space. Listed, because a palette that only knows
  // what is already on screen is a palette that cannot help you reach anything.
  const placed = new Set(rows.map((row) => row.id));
  for (const face of read.faces) {
    if (placed.has(face.id)) continue;
    rows.push({
      id: face.id,
      title: face.title,
      box: 'nowhere',
      phase: null,
      spaceIndex: null,
      address: null,
      land: null,
      why: 'this face stands nowhere, so there is no view to reproduce; place it first',
    });
  }
  return rows;
}

/**
 * @returns {{rows: object[], unknown: string[], said: string|null}}
 */
export function search(query, corpus) {
  const { facets, words, unknown } = parseQuery(query);
  if (unknown.length > 0) {
    return { rows: [], unknown, said: PALETTE_SAID.facetUnknown(unknown[0]) };
  }
  if (Object.keys(facets).length === 0 && words.length === 0) {
    return { rows: [], unknown, said: PALETTE_SAID.empty };
  }
  const rows = corpus.filter((row) => {
    for (const [axis, value] of Object.entries(facets)) {
      const held = row[axis];
      if (held === null || held === undefined) return false;
      if (!String(held).toLowerCase().startsWith(value)) return false;
    }
    return words.every((word) => `${row.id} ${row.title} ${row.box} ${row.phase ?? ''}`.toLowerCase().includes(word));
  });
  return { rows, unknown, said: rows.length === 0 ? PALETTE_SAID.none(String(query).trim()) : null };
}
