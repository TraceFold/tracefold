// SPDX-License-Identifier: Apache-2.0
// The whole of what the shell holds, written on one line, and the digest of that line.
//
// One line, because a state that can only be compared by walking it can only be compared
// by the code that walks it, and then "the layout came back" is a claim about the walker.
// A line can be handed to something that was not there.
//
// The line is the state, not a picture of it: parse(serialise(s)) is s and
// serialise(parse(line)) is line, both asserted. Anything the line cannot carry is
// therefore not state -- which is how the viewpoint (focus, scroll, hover) is kept out by
// construction instead of by discipline.

import { leaf, split, isLeaf, RATIO_PLACES } from './tree.mjs';
import { digestOf, DIGEST_NAME } from './digest.mjs';

export const LINE_VERSION = 'gxw1';
export const THEMES = Object.freeze(['light', 'dark']);
export const DOCK_SIDES = Object.freeze(['left', 'right', 'bottom']);

/** Ids and names are drawn from one alphabet so a line can be split without escaping. */
export const NAME_PATTERN = /^[a-z0-9][a-z0-9_-]*$/;

const ratioText = (r) => r.toFixed(RATIO_PLACES);

function stageText(node) {
  if (isLeaf(node)) return `l[${node.tabs.join(',')}]@${node.active}`;
  const mark = node.axis === 'row' ? 'r' : 'c';
  const parts = node.kids.map((kid, i) => `${ratioText(node.ratios[i])}:${stageText(kid)}`);
  return `${mark}(${parts.join(',')})`;
}

const dockText = (side, dock) => `${side}:${dock.open ? '+' : '-'}${dock.size}[${dock.faces.join(',')}]@${dock.at}`;

const spaceText = (space) => [
  space.name,
  ...DOCK_SIDES.map((side) => dockText(side, space.docks[side])),
  stageText(space.stage),
].join('~');

/** @returns {string} the one line */
export function serialise(state) {
  return [LINE_VERSION, state.theme, String(state.space), ...state.spaces.map(spaceText)].join('|');
}

class Reader {
  constructor(text) { this.text = text; this.i = 0; }

  peek() { return this.text[this.i]; }

  take(ch) {
    if (this.text[this.i] !== ch) {
      throw new SyntaxError(`expected ${ch} at ${this.i}, found ${this.text[this.i] ?? 'end of line'}`);
    }
    this.i += 1;
  }

  until(stops) {
    const from = this.i;
    while (this.i < this.text.length && !stops.includes(this.text[this.i])) this.i += 1;
    return this.text.slice(from, this.i);
  }

  done() { return this.i >= this.text.length; }
}

function readIds(reader) {
  reader.take('[');
  const body = reader.until([']']);
  reader.take(']');
  if (body === '') return [];
  const ids = body.split(',');
  for (const id of ids) {
    if (!NAME_PATTERN.test(id)) throw new SyntaxError(`not a face id: ${id}`);
  }
  return ids;
}

function readStage(reader) {
  const head = reader.peek();
  if (head === 'l') {
    reader.take('l');
    const tabs = readIds(reader);
    reader.take('@');
    const active = Number(reader.until([',', ')', '~', '|']));
    if (!Number.isInteger(active) || active < 0) throw new SyntaxError('active tab must be a whole number');
    return leaf(tabs, active);
  }
  if (head !== 'r' && head !== 'c') throw new SyntaxError(`not a stage at ${reader.i}: ${head}`);
  const axis = head === 'r' ? 'row' : 'col';
  reader.i += 1;
  reader.take('(');
  const ratios = [];
  const kids = [];
  for (;;) {
    const ratio = Number(reader.until([':']));
    reader.take(':');
    if (!(ratio > 0)) throw new SyntaxError('a ratio must be positive');
    ratios.push(ratio);
    kids.push(readStage(reader));
    if (reader.peek() === ',') { reader.take(','); continue; }
    reader.take(')');
    break;
  }
  return split(axis, kids, ratios);
}

function readDock(reader, side) {
  const named = reader.until([':']);
  if (named !== side) throw new SyntaxError(`expected the ${side} dock, found ${named}`);
  reader.take(':');
  const open = reader.peek() === '+';
  if (!open && reader.peek() !== '-') throw new SyntaxError('a dock is + or -');
  reader.i += 1;
  const size = Number(reader.until(['[']));
  if (!Number.isInteger(size) || size < 0) throw new SyntaxError('a dock size is a whole number of px');
  const faces = readIds(reader);
  reader.take('@');
  const at = Number(reader.until(['~', '|']));
  if (!Number.isInteger(at) || at < 0) throw new SyntaxError('the face a dock is showing is an index');
  if (at > 0 && at >= faces.length) throw new SyntaxError('a dock is showing a face it does not hold');
  return Object.freeze({ open, size, faces: Object.freeze(faces), at });
}

function readSpace(text) {
  const reader = new Reader(text);
  const name = reader.until(['~']);
  if (!NAME_PATTERN.test(name)) throw new SyntaxError(`not a space name: ${name}`);
  const docks = {};
  for (const side of DOCK_SIDES) {
    reader.take('~');
    docks[side] = readDock(reader, side);
  }
  reader.take('~');
  const stage = readStage(reader);
  if (!reader.done()) throw new SyntaxError(`trailing text in a space at ${reader.i}`);
  return Object.freeze({ name, docks: Object.freeze(docks), stage });
}

/** @returns {object} the state the line stands for, frozen */
export function parse(line) {
  const parts = line.split('|');
  const [version, theme, active, ...spaces] = parts;
  if (version !== LINE_VERSION) throw new SyntaxError(`not a ${LINE_VERSION} line`);
  if (!THEMES.includes(theme)) throw new SyntaxError(`not a theme: ${theme}`);
  const space = Number(active);
  if (!Number.isInteger(space) || space < 0) throw new SyntaxError('the active space is an index');
  if (spaces.length === 0) throw new SyntaxError('a shell holds at least one space');
  if (space >= spaces.length) throw new SyntaxError('the active space is not among the spaces');
  return Object.freeze({ theme, space, spaces: Object.freeze(spaces.map(readSpace)) });
}

/** The value a receipt carries. The name of the algorithm travels with it. */
export const digestOfState = (state) => `${DIGEST_NAME}:${digestOf(serialise(state))}`;

/** For a strip a person reads, never for a comparison a machine makes. */
export const shortDigest = (digest) => digest.slice(digest.indexOf(':') + 1, digest.indexOf(':') + 13);
