// SPDX-License-Identifier: Apache-2.0
// The keyboard table, and the only place in this app that knows what an input method is.
//
// A face never sees a chord and never asks whether text is mid-composition. Five separate
// files each writing their own `isComposing` check is a thing that happens by itself; it
// happened in the system this shell was derived from, and every one of those files also
// carried a comment saying the shell handled it. So the check lives here, once, and the
// gate counts occurrences elsewhere and expects zero.
//
// Every chord names a verb from the act registry. A chord for a verb that does not exist
// is refused when this module loads, so the table cannot drift away from the acts while
// still looking authoritative.

import { ACTS } from './acts.mjs';

/** `mod` is Ctrl on Windows and Linux, Meta on macOS; the table does not care which. */
export const KEYMAP = Object.freeze([
  { chord: 'mod+z', verb: 'undo', said: 'undo the last shell act' },
  { chord: 'mod+shift+z', verb: 'redo', said: 'redo the last undone act' },
  { chord: 'mod+backslash', verb: 'pane:divide', said: 'divide the focused pane across' },
  { chord: 'mod+shift+backslash', verb: 'pane:divide', said: 'divide the focused pane down' },
  { chord: 'mod+w', verb: 'pane:drop', said: 'drop the focused pane' },
  { chord: 'mod+b', verb: 'dock:open', said: 'open or close the left dock' },
  { chord: 'mod+j', verb: 'dock:open', said: 'open or close the bottom dock' },
  // req/884. There were two chords for three docks, and `keyArguments` chose the side
  // with `endsWith('j') ? 'bottom' : 'left'` -- so `right` was not decided against, it
  // was simply never reached by an else-branch. It held a real face (Held) the whole
  // time. Adding the chord matters more now than it did: the docks open shut by default,
  // so a side with no way to open it is a face with no way back.
  { chord: 'mod+shift+b', verb: 'dock:open', said: 'open or close the right dock' },
  { chord: 'mod+bracketright', verb: 'space:go', said: 'go to the next space' },
  { chord: 'mod+bracketleft', verb: 'space:go', said: 'go to the previous space' },
  { chord: 'mod+k', verb: 'theme:set', said: 'change the theme' },
  { chord: 'mod+p', verb: 'palette:open', said: 'find a view by where it stands' },
]);

/** Verbs the shell owns that are not acts in the registry: history is its own road. */
export const HISTORY_VERBS = Object.freeze(['undo', 'redo']);

/**
 * Verbs that change what is SHOWN and nothing about what is true (req/811 §8-5).
 *
 * The palette is one of these, for the reason `Frame#sideToggle` already gives about the
 * collapse control: opening a way to look for something changes no face, no placement and
 * no count, so putting it on the line would mean undo walking back through openings and
 * closings on its way to a real change. Enumerated here rather than let through as an
 * exception, so the gate below still holds -- a chord may name an act, a history verb or
 * a view verb, and there is no fourth kind that goes unchecked.
 */
export const VIEW_VERBS = Object.freeze(['palette:open']);

for (const entry of KEYMAP) {
  const known = HISTORY_VERBS.includes(entry.verb) || VIEW_VERBS.includes(entry.verb) || entry.verb in ACTS;
  if (!known) {
    throw new Error(`the keyboard table names "${entry.verb}", which the act registry does not hold`);
  }
}

const NAMED = Object.freeze({
  backslash: 'Backslash', bracketleft: 'BracketLeft', bracketright: 'BracketRight',
});

function chordOf(event) {
  const parts = [];
  if (event.ctrlKey || event.metaKey) parts.push('mod');
  if (event.shiftKey) parts.push('shift');
  if (event.altKey) parts.push('alt');
  const code = event.code ?? '';
  const named = Object.entries(NAMED).find(([, value]) => value === code);
  parts.push(named ? named[0] : (event.key ?? '').toLowerCase());
  return parts.join('+');
}

/**
 * The one composition guard. While an input method is assembling a character, the browser
 * still delivers keydown, and a shell that acts on it steals the person's word.
 */
export const composing = (event) => event.isComposing === true || event.keyCode === 229;

/**
 * @param {KeyboardEvent} event
 * @returns {{chord: string, verb: string, said: string}|null}
 */
export function match(event) {
  if (composing(event)) return null;
  const chord = chordOf(event);
  return KEYMAP.find((entry) => entry.chord === chord) ?? null;
}

export const chordFor = chordOf;
