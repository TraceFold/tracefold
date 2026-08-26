// SPDX-License-Identifier: Apache-2.0
// What a right-click on the shell's own chrome offers. req/810 GAP-3.
//
// Owner #366 asked for right-click across the whole surface, not on part of it. r4 gave a
// menu to all six faces, and the shell chrome had none at all: the sidebar rows, the tabs,
// the dock heads, the strip and the sash were bare, so "全域" was six faces and nothing
// else. This is the other half.
//
// The model is here and the drawing is in the frame, for the reason `command.mjs` gives
// about itself: what a menu OFFERS is a question about state, and a question about state
// should be answerable by `node --test` without a window in the room.
//
// Every row is either something a hand may do, or something it may not do WITH THE REASON
// ATTACHED (req/811 §8-7). There is no third kind, and in particular there is no row that
// looks pressable and quietly does nothing -- which is the defect r4 recorded on the faces
// and the one this file exists not to repeat on the chrome.

import { DOCK_RULES } from './manifest.mjs';
import { commandFor } from './command.mjs';

/** The chrome surfaces a menu can be raised on. Enumerated, so a new one is a decision. */
export const MENU_TARGETS = Object.freeze(['standing', 'tab', 'dock', 'strip', 'sash']);

export const MENU_SAID = Object.freeze({
  alreadyStanding: (title, where) => `${title} already stands in ${where}; placing it again would change nothing`,
  noCommand: 'nothing stands here, so there is no view to reproduce',
  atLeast: (side, size) => `the ${side} dock is already at its smallest declared size (${size}px)`,
  atMost: (side, size) => `the ${side} dock is already at its largest declared size (${size}px)`,
});

/**
 * A row offers exactly one of three things, and never two: an ACT (a verb the registry
 * holds, which lands on the line and can be undone), a COPY (text for the clipboard, which
 * changes nothing and therefore is not an act), or nothing at all with a `why`.
 *
 * The act/copy split is not bookkeeping. Every verb named here must exist in `acts.mjs` --
 * a menu that offers `tab:solo` because it reads well is a menu with a row that does
 * nothing, which is precisely the defect this file was written to avoid on the chrome.
 */
const row = (label, said, { act = null, copy = null } = {}, why = null) => Object.freeze({ label, said, act, copy, why });

/**
 * @param {string} target one of MENU_TARGETS
 * @param {object} context what the frame knows about the thing that was right-clicked
 * @returns {{label:string, said:string, act:object|null, why:string|null}[]}
 */
export function menuFor(target, context = {}) {
  if (!MENU_TARGETS.includes(target)) throw new RangeError(`there is no chrome menu for "${target}"`);

  if (target === 'standing') {
    const { id, title, index, stands, region, path = [] } = context;
    return [
      stands
        // Not hidden, and not silently inert: the row is drawn with the reason it cannot
        // act, which is the whole of §8-7's contract.
        ? row(`place ${title}`, 'put this face where it is declared to live', {}, MENU_SAID.alreadyStanding(title, region))
        : row(`place ${title}`, 'put this face where it is declared to live', {
          act: context.place === 'stage'
            ? { verb: 'tab:add', args: { index, path, id } }
            : { verb: 'dock:add', args: { index, side: context.place, id } },
        }),
      row('copy the address of this face', 'take the gx line that reproduces this view',
        context.address ? { copy: context.address } : {},
        context.address ? null : MENU_SAID.noCommand),
    ];
  }

  if (target === 'tab') {
    const { index, path, at, id, active } = context;
    return [
      row('go to this tab', 'bring this face to the front of its pane',
        at === active ? {} : { act: { verb: 'tab:go', args: { index, path, at } } },
        at === active ? 'this tab is already the one in front' : null),
      row('close this tab', 'take this face off the stage', { act: { verb: 'tab:close', args: { index, path, at } } }),
      row('copy this address', 'take the gx line that reproduces this view', { copy: commandFor('stage', { index, path, at, id }) }),
    ];
  }

  if (target === 'dock') {
    const { index, side, at, id, size } = context;
    const rule = DOCK_RULES[side];
    return [
      row('take this face out of the dock', 'the dock keeps its other faces', { act: { verb: 'dock:drop', args: { index, side, at } } }),
      ...sizeRows(index, side, size, rule),
      row('copy this address', 'take the gx line that reproduces this view', { copy: commandFor('dock', { index, side, at, id }) }),
    ];
  }

  if (target === 'sash') {
    const { index, side, size } = context;
    return sizeRows(index, side, size, DOCK_RULES[side]);
  }

  // strip
  const { digest, suite } = context;
  return [
    row("copy this line's digest", 'the digest of what this window is showing',
      digest ? { copy: digest } : {},
      digest ? null : 'this window has not computed a digest yet'),
    row('copy the suite reading', 'what the last run measured',
      suite ? { copy: suite } : {},
      suite ? null : 'no suite reading has reached this window, so there is nothing to take'),
  ];
}

/** The two size presets a dock edge offers, each refusing itself at its own declared end. */
function sizeRows(index, side, size, rule) {
  return [
    row(`widen the ${side} dock`, 'to its largest declared size',
      size < rule.most ? { act: { verb: 'dock:size', args: { index, side, size: rule.most } } } : {},
      size < rule.most ? null : MENU_SAID.atMost(side, rule.most)),
    row(`narrow the ${side} dock`, 'to its smallest declared size',
      size > rule.least ? { act: { verb: 'dock:size', args: { index, side, size: rule.least } } } : {},
      size > rule.least ? null : MENU_SAID.atLeast(side, rule.least)),
  ];
}
