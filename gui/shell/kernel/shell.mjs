// SPDX-License-Identifier: Apache-2.0
// Assembly. Everything the shell is, wired to everything else, and nothing named.
//
// There is no face id in this file, and there is no face id in any file under kernel/.
// The starting layout is derived from the declarations: the docks take the faces that
// declared them, in declaration order, up to the capacity the dock states; the stage takes
// the ones that declared the stage. Delete a face's folder, regenerate the manifest, and
// it is gone from the rail, the docks and the tabs without a line here changing. That
// sentence is the requirement (W1) and it is also the test.

import { readManifest, DOCK_RULES, Refused } from './manifest.mjs';
import { DOCK_SIDES } from './layout.mjs';
import { leaf } from './tree.mjs';
import { ShellState, OUTCOME } from './state.mjs';
import { Viewpoint } from './viewpoint.mjs';
import { Mounted } from './mount.mjs';
import { Dismiss, LAYERS } from './dismiss.mjs';
import { Frame } from './render.mjs';
import { match, VIEW_VERBS } from './keys.mjs';

export const SPACES = Object.freeze(['verify', 'inspect']);

/**
 * The membrane's surface, wrapped so that every call a face makes is written down. A face
 * is handed this and only this; it imports nothing, so there is no second way to the
 * network for it to take.
 */
export function watch(port, notices) {
  if (!port) return null;
  return new Proxy(port, {
    get(target, name) {
      const held = target[name];
      if (typeof held !== 'function') return held;
      return (...args) => {
        const seq = notices.length + 1;
        notices.push({ seq, at: Date.now(), through: 'shell', method: String(name), outcome: 'asked' });
        return held.apply(target, args);
      };
    },
  });
}

/**
 * How many stage tabs a space opens with (req/811 §8-6: default 1, 0 reachable).
 *
 * Named rather than written as a bare `1`, because the number is a ruling and the next
 * person to read this line should find the reason attached to it rather than a literal.
 */
export const STAGE_OPENS_WITH = 1;

/**
 * What each view verb does, by name. One row per `keys.mjs` VIEW_VERBS entry, so a verb
 * that is in the table and does nothing is a missing key here rather than a silent no-op
 * at a keystroke.
 */
const VIEW = Object.freeze({
  'palette:open': (frame) => frame?.openPalette(),
});

export function openingState(read) {
  const spaces = SPACES.map((name) => {
    const docks = Object.fromEntries(DOCK_SIDES.map((side) => {
      const faces = read.faces.filter((f) => f.place === side).slice(0, DOCK_RULES[side].capacity).map((f) => f.id);
      // req/884 (Owner: 標準で画面分割はなし). A dock opens SHUT, and it opens shut even
      // when it holds a face. `open: faces.length > 0` meant "a dock is open because it
      // has something in it", which is a statement about the manifest, not a decision
      // about what a first-time reader should meet. The window opened on three regions --
      // stage, right dock, bottom dock -- and the thing the reader came for got 62% of
      // the viewport (measured, req/884 section 6).
      //
      // What this is NOT: the faces stay in `faces`, the sizes stay, `dock:open` is
      // untouched, and every dock now has both a press on the window bar and a chord.
      // Nothing is removed; one default is changed, and the machinery that was there to
      // undo it is the same machinery, now actually reachable.
      return [side, Object.freeze({ open: false, size: DOCK_RULES[side].least, faces: Object.freeze(faces), at: 0 })];
    }));
    // One tab, not every stage face at once (req/811 §8-6).
    //
    // This window opened on four stage tabs plus two docked faces -- all six faces on
    // screen the instant it loaded, each in a quarter of the room it needed. The ruling is
    // that the stage opens on ONE, and the rest stay one press away in the standing column
    // beside it, where they are still listed with their region and their count. No face is
    // less reachable than it was; the one you are actually reading gets the whole pane.
    //
    // Zero is reachable too, and that matters more than it looks: the reference tool
    // performs a silent re-add when its stage empties, quietly putting a face back so the
    // window never looks bare. That is the product lying to avoid an awkward screen, and
    // it is exactly what the honest-absence doctrine here exists to forbid -- so closing
    // the last tab leaves an empty stage that says so (`#hold`'s `bare-said`).
    const onStage = read.faces.filter((f) => f.place === 'stage').map((f) => f.id).slice(0, STAGE_OPENS_WITH);
    return Object.freeze({ name, docks: Object.freeze(docks), stage: leaf(onStage, 0) });
  });
  return Object.freeze({ theme: 'light', space: 0, spaces: Object.freeze(spaces) });
}

/**
 * @param {object} options
 * @param {Element} options.root
 * @param {{faces: object[]}} options.manifest generated from the faces folder
 * @param {Map<string, {mount: Function}>} options.modules what each declared face actually is
 * @param {object} [options.port] the membrane's surface
 * @param {Array} [options.notices]
 */
export function createShell({ root, manifest, modules, port = null, notices = [], initial = null }) {
  const declared = readManifest(manifest);
  const bound = new Map(declared.faces.map((face) => [face.id, Object.freeze({
    ...face,
    mount: modules?.get(face.id)?.mount ?? null,
  })]));
  const read = Object.freeze({ faces: declared.faces, byId: bound });

  const view = new Viewpoint();
  const mounted = new Mounted();
  const dismiss = new Dismiss();
  const state = new ShellState(read, initial ?? openingState(read));
  const watched = watch(port, notices);

  let frame = null;
  const rows = [];

  // [B5] What a gesture is, decided once, here, rather than at each place that drags.
  //
  // A gesture is one press of the pointer to its release. That is the reader's unit --
  // "I moved the sash" is one thing they did -- and it is a fact the document already
  // knows, so nothing that drags has to remember to mint an id and no act name is
  // special-cased into a list that the next draggable thing will be missing from. The
  // listeners run in the capture phase so the id exists before any handler that might
  // perform an act, and the keyboard closes the gesture before dispatching: a release
  // that lands outside this window never arrives, and a key pressed afterwards must not
  // be folded into a drag that has visibly ended.
  const doc = root.ownerDocument;
  let gestures = 0;
  let gesture = null;
  const openGesture = () => { gestures += 1; gesture = gestures; };
  const closeGesture = () => { gesture = null; };
  doc.addEventListener('pointerdown', openGesture, true);
  doc.addEventListener('pointerup', closeGesture, true);
  doc.addEventListener('pointercancel', closeGesture, true);

  const act = (verb, args) => {
    const row = state.perform(verb, args, { gesture });
    rows.push(row);
    if (row.outcome === OUTCOME.REFUSED || row.outcome === OUTCOME.ELSEWHERE) {
      notices.push({ seq: notices.length + 1, at: Date.now(), through: 'shell', method: verb, outcome: row.outcome, said: row.said });
    }
    frame?.say(row);
    paint();
    return row;
  };

  const history = {
    undo: () => { const row = state.undo(); rows.push(row); frame?.say(row); paint(); return row; },
    redo: () => { const row = state.redo(); rows.push(row); frame?.say(row); paint(); return row; },
  };

  function paint() {
    frame?.draw(state.state, state.depth, state.digest);
  }

  // [B6] `history` used to stop at the keyboard: `mod+z` was the only way to reach
  // undo, and the strip drew its depth as three words of dead text. A capability the
  // window has and offers no control for is a capability most readers do not have.
  frame = new Frame({ root, read, mounted, port: watched, notices, act, history, viewpoint: view });
  paint();

  const onKey = (event) => {
    closeGesture();
    const hit = match(event);
    if (!hit) {
      if (event.key === 'Escape') dismiss.dismiss();
      return;
    }
    event.preventDefault();
    if (hit.verb === 'undo') { history.undo(); return; }
    if (hit.verb === 'redo') { history.redo(); return; }
    // A view verb changes what is shown and nothing about what is true, so it never
    // reaches `act` and never lands on the line (keys.mjs VIEW_VERBS).
    if (VIEW_VERBS.includes(hit.verb)) { VIEW[hit.verb]?.(frame); return; }
    act(hit.verb, keyArguments(hit, state.state, view));
  };
  root.ownerDocument.addEventListener('keydown', onKey);

  return {
    act,
    undo: history.undo,
    redo: history.redo,
    get state() { return state.state; },
    get line() { return state.line; },
    get digest() { return state.digest; },
    get receipt() { return state.receipt; },
    get depth() { return state.depth; },
    get rows() { return Object.freeze([...rows]); },
    read,
    mounted,
    dismiss,
    viewpoint: view,
    notices,
    frame,
    layers: LAYERS,
    stop() {
      root.ownerDocument.removeEventListener('keydown', onKey);
      doc.removeEventListener('pointerdown', openGesture, true);
      doc.removeEventListener('pointerup', closeGesture, true);
      doc.removeEventListener('pointercancel', closeGesture, true);
      mounted.lowerAll();
      root.replaceChildren();
    },
  };
}

/** The keyboard says which act; where it lands is the viewpoint's business, not the table's. */
function keyArguments(hit, state, view) {
  const index = state.space;
  const focus = typeof view.focus === 'string' && view.focus !== '' ? view.focus.split('.').map(Number) : [];
  switch (hit.verb) {
    case 'pane:divide':
      return { index, path: focus, axis: hit.chord.includes('shift') ? 'col' : 'row' };
    case 'pane:drop':
      return { index, path: focus };
    case 'dock:open': {
      // A table, not an else-branch (req/884). The previous form was
      // `endsWith('j') ? 'bottom' : 'left'`, which silently made every chord that was
      // not `j` mean `left` -- so when the right dock's chord was added it would have
      // opened the wrong dock, and before it was added the right dock was simply
      // unreachable. A lookup that can MISS is the difference: an unmapped chord now
      // returns nothing rather than guessing a side.
      const BY_CHORD = { 'mod+b': 'left', 'mod+shift+b': 'right', 'mod+j': 'bottom' };
      const side = BY_CHORD[hit.chord];
      if (!side) return {};
      return { index, side, open: !state.spaces[index].docks[side].open };
    }
    case 'space:go': {
      const step = hit.chord.includes('bracketright') ? 1 : -1;
      return { index: (index + step + state.spaces.length) % state.spaces.length };
    }
    case 'theme:set':
      return { theme: state.theme === 'light' ? 'dark' : 'light' };
    default:
      return {};
  }
}

export { Refused, OUTCOME };
