// SPDX-License-Identifier: Apache-2.0
// The slot registry: the places the shell has, listed once.
//
// A slot is a place, and a place knows nothing about what stands in it. This is the older
// principle the derived-from system wrote down, argued for, and then abandoned on the
// same day by hand-writing a strip of face names into its rail. The principle was right;
// what was missing was something that would notice its withdrawal. That is what the gate
// on this table is for -- a principle can be quietly reversed, an exit code cannot.

import { DOCK_SIDES } from './layout.mjs';

export const SLOTS = Object.freeze([
  { name: 'rail', fills: 'declaration', said: 'the strip of faces that may be summoned' },
  ...DOCK_SIDES.map((side) => ({ name: `dock-${side}`, fills: 'state', said: `the ${side} dock` })),
  { name: 'stage', fills: 'state', said: 'the split tree' },
  { name: 'strip', fills: 'shell', said: 'the line that carries the digest and the standing of the shell' },
]);

export const SLOT_NAMES = Object.freeze(SLOTS.map((s) => s.name));

/** Where a slot's contents come from. Nothing here comes from a list held in the shell. */
export const FILLS = Object.freeze(['declaration', 'state', 'shell']);

for (const slot of SLOTS) {
  if (!FILLS.includes(slot.fills)) throw new Error(`slot ${slot.name} does not say where it is filled from`);
}

export const slot = (name) => SLOTS.find((s) => s.name === name) ?? null;
