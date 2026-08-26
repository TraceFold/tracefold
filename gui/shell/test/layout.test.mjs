// SPDX-License-Identifier: Apache-2.0
// The line and the state are the same thing said twice. Both directions are asserted,
// because only one direction is a lossy encoding that looks fine until something is
// restored from it.

import test from 'node:test';
import assert from 'node:assert/strict';

import { serialise, parse, digestOfState, shortDigest, LINE_VERSION, DOCK_SIDES } from '../kernel/layout.mjs';
import { leaf, split } from '../kernel/tree.mjs';
import { readManifest } from '../kernel/manifest.mjs';
import { openingState } from '../kernel/shell.mjs';
import { MANIFEST } from '../demo/manifest.gen.mjs';
import { seeded, walkOnce } from '../tools/walk.mjs';
import { ShellState } from '../kernel/state.mjs';

const dock = (open, size, faces, at = 0) => Object.freeze({ open, size, faces: Object.freeze(faces), at });

const made = Object.freeze({
  theme: 'light',
  space: 1,
  spaces: Object.freeze([
    Object.freeze({
      name: 'verify',
      docks: Object.freeze({ left: dock(true, 240, ['probe-a']), right: dock(false, 280, []), bottom: dock(true, 180, ['floor-a']) }),
      stage: split('row', [leaf(['sheet-a', 'sheet-b'], 1), split('col', [leaf(['sheet-c']), leaf()])], [2, 1]),
    }),
    Object.freeze({
      name: 'inspect',
      docks: Object.freeze({ left: dock(false, 200, []), right: dock(false, 220, []), bottom: dock(false, 120, []) }),
      stage: leaf(),
    }),
  ]),
});

test('a state writes one line and the line reads back the same state', () => {
  const line = serialise(made);
  assert.ok(line.startsWith(`${LINE_VERSION}|light|1|`));
  assert.equal(line.split('\n').length, 1);
  assert.equal(serialise(parse(line)), line, 'line -> state -> line is not the identity');
  const back = parse(line);
  assert.equal(back.spaces.length, 2);
  assert.equal(back.spaces[0].stage.kids[0].active, 1);
  assert.deepEqual(back.spaces[0].docks.left.faces, ['probe-a']);
  for (const side of DOCK_SIDES) assert.ok(back.spaces[0].docks[side]);
});

test('the line survives two hundred arbitrary acts, every step of the way', () => {
  const read = readManifest(MANIFEST);
  const shell = new ShellState(read, openingState(read));
  const random = seeded(7);
  for (let i = 0; i < 200; i += 1) {
    walkOnce((verb, args) => shell.perform(verb, args), read, shell.state, random);
    const line = shell.line;
    assert.equal(serialise(parse(line)), line, `step ${i} does not round-trip`);
  }
});

test('a malformed line is refused, not guessed at', () => {
  for (const bad of [
    'gxw0|light|0|verify~left:+240[]~right:-280[]~bottom:-180[]~l[]@0',
    'gxw1|sepia|0|verify~left:+240[]~right:-280[]~bottom:-180[]~l[]@0',
    'gxw1|light|4|verify~left:+240[]~right:-280[]~bottom:-180[]~l[]@0',
    'gxw1|light|0|verify~left:+240[]~bottom:-180[]~l[]@0',
    'gxw1|light|0|Verify~left:+240[]~right:-280[]~bottom:-180[]~l[]@0',
    'gxw1|light|0|verify~left:+240[]~right:-280[]~bottom:-180[]~x[]@0',
  ]) {
    assert.throws(() => parse(bad), SyntaxError, bad);
  }
});

test('the digest names its algorithm and moves when one bit of the line does', () => {
  const value = digestOfState(made);
  assert.match(value, /^blake3:[0-9a-f]{64}$/);
  assert.equal(shortDigest(value).length, 12);
  const shifted = parse(serialise(made).replace('|light|', '|dark|'));
  assert.notEqual(digestOfState(shifted), value);
});
