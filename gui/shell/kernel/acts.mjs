// SPDX-License-Identifier: Apache-2.0
// The act registry: every way the shell's state is allowed to differ from itself.
//
// An act is a change together with its inverse. The inverse is a closure over what the
// change removed -- a subtree, a tab, an old ratio array -- and never over the state as a
// whole. That distinction is the whole design. Because the tree is persistent, the
// "captured" subtree is a reference to a node that already exists rather than a copy of
// one, so an inverse costs a pointer and a snapshot costs the layout again, every step.
//
// Two tracks, and they do not mix (ruling: glovrex req/38 §470):
//   shell -- only the shell's own state moves. The inverse is here, and undo is local.
//   face  -- the ledger's digest moves. The inverse belongs to the backend, is reached
//            through the membrane, and is *not* written here. A shell that kept its own
//            inverse for those would be offering a second way to undo a committed thing,
//            and the second way would not pass the gate the first one passes.
// Hence `invert: null` on the face track. Null here says "cannot hold one", never "not
// written yet": an act nobody has implemented is absent from this table entirely.

import { at, divide, drop, setRatios, leaf, replaceAt, isLeaf, MIN_RATIO } from './tree.mjs';
import { THEMES, DOCK_SIDES, NAME_PATTERN } from './layout.mjs';
import { placement, DOCK_RULES, Refused, REFUSAL } from './manifest.mjs';

export const TRACK = Object.freeze({ SHELL: 'shell', FACE: 'face' });

/** Which of the four layers the act performs which of the four operations on. */
export const LAYER = Object.freeze(['space', 'dock', 'pane', 'tab']);
export const OPERATION = Object.freeze(['create', 'read', 'update', 'delete']);

/**
 * One cell of that grid is deliberately empty, and this is the declaration of it rather
 * than the silence about it.
 *
 * "Reading a pane" means being at one -- which pane your next act lands in. That is where
 * a person is looking, and this shell keeps looking out of its state on purpose (W6): if
 * it were state it would be on the line, it would be in the digest, and going back a step
 * would move the cursor. So the pane layer's read lives in the viewpoint, where it cannot
 * be undone, and the four-operations rule is met across the shell rather than inside the
 * registry. Anything else here that looks like a gap is a gap.
 */
export const CRUD_ELSEWHERE = Object.freeze([
  Object.freeze({
    layer: 'pane',
    operation: 'read',
    where: 'kernel/viewpoint.mjs',
    said: 'which pane you are at is where you are looking, and looking is not state',
  }),
]);

const freeze = Object.freeze;

const withSpaces = (state, index, made) => freeze({
  ...state,
  spaces: freeze(state.spaces.map((s, i) => (i === index ? made : s))),
});

const withStage = (space, stage) => freeze({ ...space, stage });

const withDock = (space, side, dock) => freeze({
  ...space,
  docks: freeze({ ...space.docks, [side]: freeze(dock) }),
});

const spaceAt = (state, index) => {
  const space = state.spaces[index];
  if (!space) throw new Refused(REFUSAL.NOT_DECLARED_HERE, `there is no space ${index}`);
  return space;
};

/**
 * What an inverse carries, made readable from outside. Attaching it is not decoration:
 * it is the only way tools/inverse_census.mjs can say how much this design actually
 * retains without being told, and being told is what a snapshot's cost estimate was.
 */
const holding = (value, fn) => Object.assign(fn, { held: value });

/** Restore one node of one space's stage. Every stage inverse is built from this. */
const restoreStage = (index, anchor, node) => holding(node, (state) =>
  withSpaces(state, index, withStage(spaceAt(state, index), replaceAt(spaceAt(state, index).stage, anchor, node))));

const restoreDock = (index, side, dock) => holding(dock, (state) =>
  withSpaces(state, index, withDock(spaceAt(state, index), side, dock)));

function leafAt(space, path) {
  const node = at(space.stage, path);
  if (!isLeaf(node)) throw new Refused(REFUSAL.NOT_DECLARED_HERE, 'that place is a split, not a pane');
  return node;
}

/**
 * The table. `build` is given the manifest reading, the state it is about to change, and
 * the argument; it answers with `{apply, invert}` or refuses in words.
 */
export const ACTS = Object.freeze({
  'theme:set': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'space', operation: 'update',
    build: (read, state, { theme }) => {
      if (!THEMES.includes(theme)) throw new Refused(REFUSAL.BAD_PLACE, `"${theme}" is not a theme`);
      if (theme === state.theme) return null;
      const was = state.theme;
      return {
        apply: (s) => freeze({ ...s, theme }),
        invert: holding(was, (s) => freeze({ ...s, theme: was })),
      };
    },
  },

  'space:go': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'space', operation: 'read',
    build: (read, state, { index }) => {
      spaceAt(state, index);
      if (index === state.space) return null;
      const was = state.space;
      return {
        apply: (s) => freeze({ ...s, space: index }),
        invert: holding(was, (s) => freeze({ ...s, space: was })),
      };
    },
  },

  'space:add': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'space', operation: 'create',
    build: (read, state, { name }) => {
      if (!NAME_PATTERN.test(name ?? '')) throw new Refused(REFUSAL.BAD_ID, `"${name}" is not a space name`);
      if (state.spaces.some((s) => s.name === name)) throw new Refused(REFUSAL.DUPLICATE_ID, `a space is already called "${name}"`);
      const made = freeze({
        name,
        docks: freeze(Object.fromEntries(DOCK_SIDES.map((side) => [side, freeze({ open: false, size: DOCK_RULES[side].least, faces: freeze([]), at: 0 })]))),
        stage: leaf(),
      });
      const was = state.space;
      return {
        apply: (s) => freeze({ ...s, spaces: freeze([...s.spaces, made]), space: s.spaces.length }),
        invert: holding(was, (s) => freeze({ ...s, spaces: freeze(s.spaces.slice(0, -1)), space: was })),
      };
    },
  },

  'space:close': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'space', operation: 'delete',
    build: (read, state, { index }) => {
      const held = spaceAt(state, index);
      if (state.spaces.length === 1) throw new Refused(REFUSAL.NOT_DECLARED_HERE, 'the last space cannot be closed; a shell with no space has nowhere to refuse from');
      const was = state.space;
      return {
        apply: (s) => {
          const spaces = s.spaces.filter((_, i) => i !== index);
          return freeze({ ...s, spaces: freeze(spaces), space: Math.min(s.space, spaces.length - 1) });
        },
        invert: holding(held, (s) => {
          const spaces = [...s.spaces];
          spaces.splice(index, 0, held);
          return freeze({ ...s, spaces: freeze(spaces), space: was });
        }),
      };
    },
  },

  'space:rename': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'space', operation: 'update',
    build: (read, state, { index, name }) => {
      const space = spaceAt(state, index);
      if (!NAME_PATTERN.test(name ?? '')) throw new Refused(REFUSAL.BAD_ID, `"${name}" is not a space name`);
      if (name === space.name) return null;
      if (state.spaces.some((s, i) => i !== index && s.name === name)) throw new Refused(REFUSAL.DUPLICATE_ID, `a space is already called "${name}"`);
      const was = space.name;
      return {
        apply: (s) => withSpaces(s, index, freeze({ ...spaceAt(s, index), name })),
        invert: holding(was, (s) => withSpaces(s, index, freeze({ ...spaceAt(s, index), name: was }))),
      };
    },
  },

  'pane:divide': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'pane', operation: 'create',
    build: (read, state, { index, path, axis }) => {
      const space = spaceAt(state, index);
      leafAt(space, path);
      // The node that is about to stand where another one stands. Capturing it is
      // capturing a reference: the tree it belongs to is frozen and still whole.
      const parentPath = path.slice(0, -1);
      const parent = path.length > 0 ? at(space.stage, parentPath) : null;
      const anchor = parent && parent.axis === axis ? parentPath : path;
      const held = at(space.stage, anchor);
      return {
        apply: (s) => withSpaces(s, index, withStage(spaceAt(s, index), divide(spaceAt(s, index).stage, path, axis))),
        invert: restoreStage(index, anchor, held),
      };
    },
  },

  'pane:drop': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'pane', operation: 'delete',
    build: (read, state, { index, path }) => {
      const space = spaceAt(state, index);
      leafAt(space, path);
      if (path.length === 0) throw new Refused(REFUSAL.NOT_DECLARED_HERE, 'the only pane of a space cannot be dropped');
      const anchor = path.slice(0, -1);
      const held = at(space.stage, anchor);
      return {
        apply: (s) => withSpaces(s, index, withStage(spaceAt(s, index), drop(spaceAt(s, index).stage, path))),
        invert: restoreStage(index, anchor, held),
      };
    },
  },

  'pane:ratio': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'pane', operation: 'update',
    build: (read, state, { index, path, ratios }) => {
      const space = spaceAt(state, index);
      const node = at(space.stage, path);
      if (isLeaf(node)) throw new Refused(REFUSAL.NOT_DECLARED_HERE, 'a pane has no ratios; a split does');
      const held = node;
      const next = setRatios(space.stage, path, ratios);
      if (at(next, path).ratios.every((r, i) => r === held.ratios[i])) return null;
      return {
        apply: (s) => withSpaces(s, index, withStage(spaceAt(s, index), setRatios(spaceAt(s, index).stage, path, ratios))),
        invert: restoreStage(index, path, held),
      };
    },
  },

  'tab:add': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'tab', operation: 'create',
    build: (read, state, { index, path, id }) => {
      const space = spaceAt(state, index);
      const held = leafAt(space, path);
      const may = placement(read, id, 'stage');
      if (!may.ok) throw new Refused(may.code, may.said);
      if (held.tabs.includes(id)) return null;
      const made = leaf([...held.tabs, id], held.tabs.length);
      return {
        apply: restoreStage(index, path, made),
        invert: restoreStage(index, path, held),
      };
    },
  },

  'tab:close': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'tab', operation: 'delete',
    build: (read, state, { index, path, at: which }) => {
      const space = spaceAt(state, index);
      const held = leafAt(space, path);
      if (held.tabs[which] === undefined) throw new Refused(REFUSAL.NOT_A_FACE, `this pane has no tab ${which}`);
      const tabs = held.tabs.filter((_, i) => i !== which);
      const made = leaf(tabs, held.active > which ? held.active - 1 : held.active);
      return {
        apply: restoreStage(index, path, made),
        invert: restoreStage(index, path, held),
      };
    },
  },

  'tab:go': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'tab', operation: 'read',
    build: (read, state, { index, path, at: which }) => {
      const space = spaceAt(state, index);
      const held = leafAt(space, path);
      if (held.tabs[which] === undefined) throw new Refused(REFUSAL.NOT_A_FACE, `this pane has no tab ${which}`);
      if (held.active === which) return null;
      return {
        apply: restoreStage(index, path, leaf(held.tabs, which)),
        invert: restoreStage(index, path, held),
      };
    },
  },

  'tab:move': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'tab', operation: 'update',
    build: (read, state, { index, from, at: which, to }) => {
      const space = spaceAt(state, index);
      const source = leafAt(space, from);
      const target = leafAt(space, to);
      const id = source.tabs[which];
      if (id === undefined) throw new Refused(REFUSAL.NOT_A_FACE, `this pane has no tab ${which}`);
      if (from.length === to.length && from.every((v, i) => v === to[i])) return null;
      if (target.tabs.includes(id)) throw new Refused(REFUSAL.DUPLICATE_ID, `"${id}" already stands in that pane`);
      const madeSource = leaf(source.tabs.filter((_, i) => i !== which), source.active > which ? source.active - 1 : source.active);
      const madeTarget = leaf([...target.tabs, id], target.tabs.length);
      return {
        apply: (s) => {
          const one = restoreStage(index, from, madeSource)(s);
          return restoreStage(index, to, madeTarget)(one);
        },
        invert: holding({ source, target }, (s) => {
          const one = restoreStage(index, to, target)(s);
          return restoreStage(index, from, source)(one);
        }),
      };
    },
  },

  'dock:open': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'dock', operation: 'update',
    build: (read, state, { index, side, open }) => {
      const space = spaceAt(state, index);
      const held = space.docks[side];
      if (!held) throw new Refused(REFUSAL.BAD_PLACE, `there is no ${side} dock`);
      if (held.open === open) return null;
      return {
        apply: restoreDock(index, side, { ...held, open }),
        invert: restoreDock(index, side, held),
      };
    },
  },

  'dock:size': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'dock', operation: 'update',
    build: (read, state, { index, side, size }) => {
      const space = spaceAt(state, index);
      const held = space.docks[side];
      if (!held) throw new Refused(REFUSAL.BAD_PLACE, `there is no ${side} dock`);
      const rule = DOCK_RULES[side];
      if (!Number.isInteger(size) || size < rule.least || size > rule.most) {
        throw new Refused(REFUSAL.NOT_DECLARED_HERE, `the ${side} dock stands between ${rule.least} and ${rule.most} px, and ${size} was asked for`);
      }
      if (held.size === size) return null;
      return {
        apply: restoreDock(index, side, { ...held, size }),
        invert: restoreDock(index, side, held),
      };
    },
  },

  'dock:add': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'dock', operation: 'create',
    build: (read, state, { index, side, id }) => {
      const space = spaceAt(state, index);
      const held = space.docks[side];
      if (!held) throw new Refused(REFUSAL.BAD_PLACE, `there is no ${side} dock`);
      const may = placement(read, id, side);
      if (!may.ok) throw new Refused(may.code, may.said);
      if (held.faces.includes(id)) return null;
      if (held.faces.length + 1 > DOCK_RULES[side].capacity) {
        throw new Refused(REFUSAL.DOCK_OVER_CAPACITY, `the ${side} dock holds ${DOCK_RULES[side].capacity} faces and already holds ${held.faces.length}`);
      }
      return {
        apply: restoreDock(index, side, { ...held, faces: [...held.faces, id], open: true }),
        invert: restoreDock(index, side, held),
      };
    },
  },

  'dock:drop': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'dock', operation: 'delete',
    build: (read, state, { index, side, at: which }) => {
      const space = spaceAt(state, index);
      const held = space.docks[side];
      if (!held) throw new Refused(REFUSAL.BAD_PLACE, `there is no ${side} dock`);
      if (held.faces[which] === undefined) throw new Refused(REFUSAL.NOT_A_FACE, `the ${side} dock has no face ${which}`);
      const faces = held.faces.filter((_, i) => i !== which);
      return {
        apply: restoreDock(index, side, { ...held, faces, at: Math.max(0, Math.min(held.at, faces.length - 1)) }),
        invert: restoreDock(index, side, held),
      };
    },
  },

  'dock:go': {
    track: TRACK.SHELL, invert: 'captured-closure', layer: 'dock', operation: 'read',
    build: (read, state, { index, side, at: which }) => {
      const space = spaceAt(state, index);
      const held = space.docks[side];
      if (!held) throw new Refused(REFUSAL.BAD_PLACE, `there is no ${side} dock`);
      if (held.faces[which] === undefined) throw new Refused(REFUSAL.NOT_A_FACE, `the ${side} dock has no face ${which}`);
      if (held.at === which) return null;
      return {
        apply: restoreDock(index, side, { ...held, at: which }),
        invert: restoreDock(index, side, held),
      };
    },
  },

  // ---- the face track. One entry, and it is here to be refused by undo, not performed
  // by it: the shell offers the verb so that "the inverse is elsewhere" is a thing the
  // shell can say, rather than a thing a reader has to know.
  'record:undo': {
    track: TRACK.FACE,
    // Stated, not omitted. "null" is the declaration that this act cannot hold an
    // inverse here; an act whose inverse merely has not been written yet is not in this
    // table at all, so the two can never be read as the same thing.
    invert: null,
    layer: 'tab', operation: 'update',
    delegate: 'post_transformations_id_undo',
    said: 'undoing a committed transformation moves the ledger, so its inverse is the backend\'s undo, reached through the membrane; the shell does not keep a second one',
    build: () => { throw new Refused(REFUSAL.NOT_DECLARED_HERE, 'a face-track act is performed through the membrane, not through the shell history'); },
  },
});

export const VERBS = Object.freeze(Object.keys(ACTS));

/**
 * The table checks itself on load. Not because a gate would not catch it -- tools/gates
 * does -- but because a table that can be loaded while malformed will be, by whatever
 * imports it before the gate runs.
 */
for (const [verb, act] of Object.entries(ACTS)) {
  if (!('invert' in act)) throw new Error(`${verb} declares no invert`);
  if (act.track === TRACK.FACE && act.invert !== null) {
    throw new Error(`${verb} is on the face track and may not hold its own inverse`);
  }
  if (act.track === TRACK.SHELL && act.invert !== 'captured-closure') {
    throw new Error(`${verb} is on the shell track and must hold a captured inverse`);
  }
  if (act.track === TRACK.FACE && typeof act.delegate !== 'string') {
    throw new Error(`${verb} is on the face track and must name the membrane method its inverse lives behind`);
  }
  if (!LAYER.includes(act.layer) || !OPERATION.includes(act.operation)) {
    throw new Error(`${verb} does not state which layer and operation it is`);
  }
}

export { Refused, REFUSAL, MIN_RATIO };
