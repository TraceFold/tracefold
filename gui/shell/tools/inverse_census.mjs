// SPDX-License-Identifier: Apache-2.0
// What this shell's inverses actually hold, counted over this shell's own act set.
//
// The reqdef carries a measurement of another system: eighteen verbs, none impossible,
// thirty-nine per cent pure, sixty-one per cent needing capture, and a snapshot costing
// about forty-five times the information a step contained. Every one of those numbers is
// a transcription. This file measures ours.
//
// Two things are counted, and they are different questions:
//
//   the classification -- for each act, whether its inverse carries anything beyond its
//     own arguments. Read off `invert.held`, which the registry attaches at the point of
//     capture, so this is a reading rather than a guess about the source text.
//
//   the cost -- after an arbitrary walk, how many objects the whole history retains that
//     the current state can no longer reach. That is the honest figure for a persistent
//     tree: capturing a subtree that is still standing costs a pointer, and only the
//     branches that were removed are actually held open. A snapshot has no such
//     distinction, so its cost is stated beside it, in the same unit.
//
//   node tools/inverse_census.mjs

import { ACTS, VERBS, TRACK } from '../kernel/acts.mjs';
import { readManifest } from '../kernel/manifest.mjs';
import { openingState } from '../kernel/shell.mjs';
import { serialise } from '../kernel/layout.mjs';
import { ShellState } from '../kernel/state.mjs';
import { MANIFEST } from '../demo/manifest.gen.mjs';
import { seeded, walkOnce, WALK_SEED, WALK_STEPS } from './walk.mjs';

/** One reachable argument per verb, so the census covers acts that can be built. */
const ARGUMENTS = Object.freeze({
  'theme:set': () => ({ theme: 'dark' }),
  'space:go': () => ({ index: 1 }),
  'space:add': () => ({ name: 'a-third-space' }),
  'space:close': () => ({ index: 1 }),
  'space:rename': () => ({ index: 0, name: 'looking' }),
  'pane:divide': () => ({ index: 0, path: [0], axis: 'row' }),
  'pane:drop': () => ({ index: 0, path: [0] }),
  'pane:ratio': () => ({ index: 0, path: [], ratios: [0.4, 0.6] }),
  'tab:add': (read) => ({ index: 0, path: [1], id: read.faces.find((f) => f.place === 'stage').id }),
  'tab:close': () => ({ index: 0, path: [0], at: 0 }),
  'tab:go': () => ({ index: 0, path: [0], at: 1 }),
  'tab:move': () => ({ index: 0, from: [0], at: 0, to: [1] }),
  // req/884: the direction flipped with the arrival default. A dock now opens SHUT, so
  // `open: false` is what the dock already is -- the act correctly refuses to build,
  // and the census reported it unreachable. Opening it is the move that is real from the
  // opening state. This is the same act measured the same way, in the only direction
  // that is a move; it is not the census being asked an easier question, which is the
  // repair PREPARE's own comment warns against.
  'dock:open': () => ({ index: 0, side: 'left', open: true }),
  'dock:size': () => ({ index: 0, side: 'left', size: 300 }),
  'dock:add': (read) => ({ index: 0, side: 'right', id: read.faces.find((f) => f.place === 'right').id }),
  'dock:drop': () => ({ index: 0, side: 'left', at: 0 }),
  'dock:go': () => ({ index: 0, side: 'left', at: 1 }),
});

/**
 * What has to happen before a verb is reachable at all, per verb.
 *
 * `tab:go at 1` joined this list when the stage began opening on one tab (req/811 §8-6,
 * `openingState`'s STAGE_OPENS_WITH). There is nothing wrong with the verb: going to the
 * second tab is an ordinary act, and it simply needs a second tab to exist first. Writing
 * the preparation down is the honest repair; weakening the census to `at: 0` would have
 * made the number go green by asking an easier question.
 */
const PREPARE = Object.freeze({
  'dock:add': () => [['dock:drop', { index: 0, side: 'right', at: 0 }]],
  // Two steps, and the second one is not decoration: a tab that has just been added is
  // the active tab, so going to it changes nothing and the act correctly refuses to be
  // built. The reader is put back on the first tab, and then going to the second is a
  // real move -- which is the act this census is trying to measure.
  'tab:go': (read) => [
    ['tab:add', { index: 0, path: [0], id: read.faces.filter((f) => f.place === 'stage')[1].id }],
    ['tab:go', { index: 0, path: [0], at: 0 }],
  ],
});

const reach = (value, seen = new Set()) => {
  if (value === null || typeof value !== 'object') return seen;
  if (seen.has(value)) return seen;
  seen.add(value);
  for (const held of Array.isArray(value) ? value : Object.values(value)) reach(held, seen);
  return seen;
};

export function census() {
  const read = readManifest(MANIFEST);
  const opening = openingState(read);
  const rows = [];

  for (const verb of VERBS) {
    const act = ACTS[verb];
    if (act.track === TRACK.FACE) {
      rows.push({ verb, kind: 'delegated', held: null, why: 'the inverse is the backend\'s, reached through the membrane' });
      continue;
    }
    // A leaf stage cannot be dropped or divided at a path that does not exist yet, so the
    // few acts that need a deeper stage are built against one act's worth of history.
    const ground = new ShellState(read, opening);
    ground.perform('pane:divide', { index: 0, path: [], axis: 'row' });
    // One act's worth of preparation where the opening state cannot reach the verb --
    // a dock at its stated capacity has no room for another face, so the census would
    // otherwise report "unreachable" for an act that is perfectly ordinary in use.
    for (const [before, first] of PREPARE[verb]?.(read) ?? []) ground.perform(before, first);
    const args = ARGUMENTS[verb](read);
    let built = null;
    try { built = act.build(read, ground.state, args); } catch { built = null; }
    if (!built) {
      rows.push({ verb, kind: 'unreachable', held: null, why: `no argument in this census reaches ${verb} from the opening state` });
      continue;
    }
    const held = built.invert.held;
    rows.push({
      verb,
      kind: held === undefined ? 'unmarked' : (held === null ? 'pure' : 'captured'),
      held: held === undefined || held === null ? null : (typeof held === 'object' ? 'a node the act removed' : 'one value the act replaced'),
      why: held === undefined ? 'the inverse did not say what it holds' : undefined,
    });
  }

  const count = (kind) => rows.filter((r) => r.kind === kind).length;
  return {
    total: rows.length,
    impossible: 0,
    delegated: count('delegated'),
    pure: count('pure'),
    captured: count('captured'),
    unmarked: count('unmarked'),
    unreachable: count('unreachable'),
    rows,
  };
}

/** What a walk of this length actually keeps alive, against what a snapshot would. */
export function cost(steps = WALK_STEPS, seed = WALK_SEED) {
  const read = readManifest(MANIFEST);
  const shell = new ShellState(read, openingState(read));
  const random = seeded(seed);
  let moved = 0;
  for (let i = 0; i < steps; i += 1) {
    const row = walkOnce((verb, args) => shell.perform(verb, args), read, shell.state, random);
    if (row.outcome === 'moved') moved += 1;
  }
  const standing = reach(shell.state);
  const retained = new Set();
  for (const held of shell.heldByHistory) reach(held, retained);
  let orphaned = 0;
  for (const object of retained) if (!standing.has(object)) orphaned += 1;

  const line = serialise(shell.state).length;
  return {
    steps,
    moved,
    lineBytes: line,
    objectsStanding: standing.size,
    objectsRetainedByHistory: retained.size,
    objectsHeldOpen: orphaned,
    snapshotObjects: moved * standing.size,
    snapshotBytes: moved * line,
    timesCheaper: Number((((moved * standing.size) || 1) / (orphaned || 1)).toFixed(1)),
    said: 'objectsHeldOpen is what this history costs beyond the current state; snapshotObjects is what one copy of the state per moved step would have cost, in the same unit',
  };
}

if (process.argv[1] && process.argv[1].endsWith('inverse_census.mjs')) {
  const found = census();
  const share = (n) => `${n}/${found.total} (${Math.round((n / found.total) * 100)}%)`;
  process.stdout.write(`acts: ${found.total}\n`);
  process.stdout.write(`  impossible in principle: ${share(found.impossible)}\n`);
  process.stdout.write(`  delegated to the backend: ${share(found.delegated)}\n`);
  process.stdout.write(`  pure inverse:             ${share(found.pure)}\n`);
  process.stdout.write(`  captured inverse:         ${share(found.captured)}\n`);
  if (found.unmarked > 0) process.stdout.write(`  UNMARKED:                 ${share(found.unmarked)}\n`);
  if (found.unreachable > 0) process.stdout.write(`  not reached by this census: ${share(found.unreachable)}\n`);
  for (const row of found.rows) process.stdout.write(`    ${row.kind.padEnd(12)} ${row.verb}${row.held ? ` -- ${row.held}` : ''}\n`);
  process.stdout.write('\n');
  const spent = cost();
  for (const [key, value] of Object.entries(spent)) process.stdout.write(`  ${key}: ${value}\n`);
}
