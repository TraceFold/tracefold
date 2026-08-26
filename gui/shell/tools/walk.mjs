// SPDX-License-Identifier: Apache-2.0
// An arbitrary walk over the act registry, written once and used from both sides.
//
// The window's checks and the node tests take the same two hundred steps from the same
// seed. If the walk lived in each of them separately the two would drift, and the day
// they disagreed the disagreement would be about the walk rather than about the shell.
//
// The steps are drawn from what the state can actually take, not from a fixed script, so
// the sequence covers refusals and no-ops as well as moves -- and a refusal that moved
// the digest would be caught here rather than in a place where it looked deliberate.

/** A small deterministic source. The seed goes in the report; nothing here is random. */
export const seeded = (seed) => () => {
  let x = (seed += 0x6d2b79f5);
  x = Math.imul(x ^ (x >>> 15), x | 1);
  x ^= x + Math.imul(x ^ (x >>> 7), x | 61);
  return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
};

const SIDES = Object.freeze(['left', 'right', 'bottom']);

/**
 * @param {(verb: string, args: object) => object} act
 * @param {{faces: object[]}} read
 * @param {object} state
 * @param {() => number} random
 */
export function walkOnce(act, read, state, random) {
  const index = state.space;
  const space = state.spaces[index];
  const leaves = [];
  const splits = [];
  const walk = (node, path) => {
    if (node.k === 'l') { leaves.push({ node, path }); return; }
    splits.push({ node, path });
    node.kids.forEach((kid, i) => walk(kid, [...path, i]));
  };
  walk(space.stage, []);

  const pick = (list) => list[Math.floor(random() * list.length)];
  const someWhere = (list) => (list.length === 0 ? null : pick(list));
  const nothing = { outcome: 'unchanged', said: 'the walk had nothing of this kind to ask for' };

  const moves = [
    () => act('pane:divide', { index, path: pick(leaves).path, axis: random() < 0.5 ? 'row' : 'col' }),
    () => act('pane:drop', { index, path: pick(leaves).path }),
    () => {
      const target = someWhere(splits);
      if (!target) return nothing;
      return act('pane:ratio', { index, path: target.path, ratios: target.node.ratios.map(() => 0.2 + random()) });
    },
    () => {
      const face = someWhere(read.faces.filter((f) => f.place === 'stage'));
      return face ? act('tab:add', { index, path: pick(leaves).path, id: face.id }) : nothing;
    },
    () => {
      const here = pick(leaves);
      return act('tab:close', { index, path: here.path, at: Math.floor(random() * Math.max(1, here.node.tabs.length)) });
    },
    () => {
      const here = pick(leaves);
      return act('tab:go', { index, path: here.path, at: Math.floor(random() * Math.max(1, here.node.tabs.length)) });
    },
    () => {
      const from = pick(leaves);
      const to = pick(leaves);
      return act('tab:move', { index, from: from.path, at: Math.floor(random() * Math.max(1, from.node.tabs.length)), to: to.path });
    },
    () => {
      const side = pick(SIDES);
      return act('dock:open', { index, side, open: !space.docks[side].open });
    },
    () => act('dock:size', { index, side: pick(SIDES), size: 100 + Math.floor(random() * 260) }),
    () => {
      const side = pick(SIDES);
      return act('dock:go', { index, side, at: Math.floor(random() * Math.max(1, space.docks[side].faces.length)) });
    },
    () => {
      const side = pick(SIDES);
      const face = someWhere(read.faces.filter((f) => f.place === side));
      return face ? act('dock:add', { index, side, id: face.id }) : nothing;
    },
    () => {
      const side = pick(SIDES);
      return act('dock:drop', { index, side, at: Math.floor(random() * Math.max(1, space.docks[side].faces.length)) });
    },
    () => act('space:go', { index: Math.floor(random() * state.spaces.length) }),
    () => act('theme:set', { theme: random() < 0.5 ? 'light' : 'dark' }),
    () => act('space:rename', { index, name: `space-${Math.floor(random() * 6)}` }),
    () => act('space:add', { name: `space-${Math.floor(random() * 6)}` }),
    () => act('space:close', { index: Math.floor(random() * state.spaces.length) }),
    () => act('record:undo', {}),
  ];
  return pick(moves)();
}

export const WALK_SEED = 20260824;
export const WALK_STEPS = 200;
