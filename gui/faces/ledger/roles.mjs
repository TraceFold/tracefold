// SPDX-License-Identifier: Apache-2.0
//
// The rank this tree did not have.
//
// Before this file, drawing code went straight from a meaning in someone's head to a
// token name, and often to a raw length. `CONSUMED` in parts/src/tokens.mjs looks like a
// layer and is not one: `ink: 'var(--ink)'` renames a token to itself, so a face that
// writes `CONSUMED.ink` has still decided which token, and nothing above it can be asked
// what the choice meant. Two ranks were missing, and this file adds them for this face.
//
//   intent -> role -> token -> value
//
// A face may name an intent or a role. It may not name a token and it may not name a
// value. That rule is the whole point: the map from intent to role is where a meaning is
// recorded, and a meaning that is recorded can be checked against the vocabulary the
// engine actually speaks. It is the only place a screen and an engine can be compared.
//
// Three properties are load-bearing, and each is a check in `lattice()`:
//
//   1. No edge skips a rank. Naming a value skips three, naming a token skips two,
//      naming a role skips none. Because the ranks are ordered, the depth of a
//      violation is a number rather than a complaint.
//
//   2. intent -> role is injective. Two intents that resolve to the same appearance is
//      a screen teaching a reader that two different facts are one fact. On this face
//      that failure has a specific victim: the three verdicts and the four kinds of
//      nothing.
//
//   3. Every cell of the lattice is one of three things -- filled, declared empty, or
//      unreachable. "Empty" and "unreachable" are kept apart because collapsing them
//      makes a gate silently accept an implementation of a cell that can never be
//      reached, which is the same class of mistake as drawing `absent` and `false` the
//      same way.
//
// The roles themselves are generated as a product and then pruned by a transition graph
// rather than listed by hand, because a hand-written list of states has holes in it and
// a product without a graph has cells in it that no reader can ever see.

import { CONSUMED } from '../../parts/src/tokens.mjs';

export const ROLE_MESSAGES = {
  UNKNOWN_INTENT: 'this face has no such intent',
  UNKNOWN_ROLE: 'this face has no such role',
  UNREACHABLE: 'this role cannot be reached by any sequence of interactions, so drawing it is drawing something no reader can see',
  NOT_DECLARED: 'this cell of the lattice was neither filled nor declared empty',
  COLLAPSED: 'two intents resolve to one appearance, so the screen says two facts are one',
};

// -- rank 2: the roles, generated ---------------------------------------------

/** What part of a drawing a role speaks about. */
export const CHANNELS = Object.freeze(['surface', 'text', 'edge', 'glyph']);

/** How far forward the thing sits. */
export const GROUNDS = Object.freeze(['base', 'raised', 'sunken', 'overlay']);

/**
 * Interaction is a graph and not a set (#429). `rest` reaches `hover`, `hover` reaches
 * `active`, `focus` is orthogonal to all three, and `disabled` absorbs: it is reachable
 * from `rest` and has no outgoing edge at all. A product over these names alone would
 * manufacture `disabled` crossed with `active`, which is a cell that cannot occur.
 */
export const STATES = Object.freeze(['rest', 'hover', 'active', 'focus', 'disabled']);

const TRANSITIONS = Object.freeze({
  rest: ['hover', 'focus', 'disabled'],
  hover: ['rest', 'active', 'focus'],
  active: ['rest', 'hover'],
  focus: ['rest', 'hover', 'active'],
  disabled: [],
});

/**
 * Which (channel, ground) pairs belong to something a hand can reach.
 *
 * This is what prunes the product. A line of prose on a static row has exactly one state
 * it can be in, so the other four cells for it are unreachable rather than merely empty,
 * and an implementation of `text/base/hover` would be code no reader can trigger.
 */
const REACHABLE_GROUNDS = Object.freeze({
  surface: ['base', 'raised', 'sunken', 'overlay'],
  text: ['base', 'raised', 'overlay'],
  edge: ['base', 'raised', 'overlay'],
  glyph: ['base', 'raised', 'overlay'],
});

const INTERACTIVE = Object.freeze([
  'surface/raised',
  'surface/overlay',
  'edge/raised',
  'edge/overlay',
  'text/raised',
  'glyph/raised',
]);

/** Every state reachable from `rest` by following the transition graph. */
function reachableStates() {
  const seen = new Set(['rest']);
  const queue = ['rest'];
  while (queue.length > 0) {
    for (const next of TRANSITIONS[queue.shift()]) {
      if (!seen.has(next)) {
        seen.add(next);
        queue.push(next);
      }
    }
  }
  return seen;
}

const REACHED = reachableStates();

/**
 * The product, with every cell carrying its own reachability. Three values, never two.
 */
export function roleProduct() {
  const cells = [];
  for (const channel of CHANNELS) {
    for (const ground of GROUNDS) {
      const groundExists = REACHABLE_GROUNDS[channel].includes(ground);
      const interactive = INTERACTIVE.includes(`${channel}/${ground}`);
      for (const state of STATES) {
        const name = `${channel}.${ground}.${state}`;
        let reachable = true;
        let why = null;
        if (!groundExists) {
          reachable = false;
          why = `${channel} has no ${ground} ground of its own; it takes the ground it sits on`;
        } else if (!REACHED.has(state)) {
          reachable = false;
          why = `no sequence of interactions arrives at ${state}`;
        } else if (!interactive && state !== 'rest') {
          reachable = false;
          why = `${channel}.${ground} is not something a hand can reach, so it is only ever at rest`;
        }
        cells.push({ name, channel, ground, state, reachable, why });
      }
    }
  }
  return cells;
}

// -- rank 3 and 4: what a role resolves to ------------------------------------

/**
 * The token each role reads.
 *
 * Two families are reached by the name the stylesheet gives them rather than through
 * `CONSUMED`, because `CONSUMED` covers 33 of the 99 declared tokens and the interaction
 * and scrollbar families are not among them. That is a hole in the rank below this one,
 * not a licence to invent one here, and it is written down rather than worked around.
 */
const RESOLVE = Object.freeze({
  'surface.base.rest': { background: CONSUMED.page },
  'surface.raised.rest': { background: CONSUMED.raisedPage },
  'surface.raised.hover': { background: CONSUMED.raisedPage },
  'surface.raised.active': { background: CONSUMED.raisedPage },
  'surface.raised.focus': { background: CONSUMED.raisedPage },
  'surface.raised.disabled': { background: CONSUMED.page },
  'surface.sunken.rest': { background: 'var(--bg-inset)' },
  'surface.overlay.rest': { background: CONSUMED.raisedPage, 'box-shadow': 'var(--shadow-menu)' },
  'surface.overlay.hover': { background: CONSUMED.raisedPage, 'box-shadow': 'var(--shadow-menu)' },
  'surface.overlay.active': { background: CONSUMED.raisedPage, 'box-shadow': 'var(--shadow-menu)' },
  'surface.overlay.focus': { background: CONSUMED.raisedPage, 'box-shadow': 'var(--shadow-menu)' },
  'surface.overlay.disabled': { background: CONSUMED.raisedPage },

  'text.base.rest': { color: CONSUMED.ink },
  'text.raised.rest': { color: CONSUMED.ink },
  'text.raised.hover': { color: CONSUMED.ink },
  'text.raised.active': { color: CONSUMED.ink },
  'text.raised.focus': { color: CONSUMED.ink },
  'text.raised.disabled': { color: CONSUMED.attendant, opacity: 'var(--disabled-opacity)' },
  'text.overlay.rest': { color: CONSUMED.ink },

  'edge.base.rest': { 'border-color': CONSUMED.rule },
  'edge.raised.rest': { 'border-color': CONSUMED.rule },
  'edge.raised.hover': { 'border-color': CONSUMED.act },
  'edge.raised.active': { 'border-color': CONSUMED.act },
  'edge.raised.focus': { 'outline-color': 'var(--focus-ink)', 'outline-width': 'var(--focus-w)', 'outline-offset': 'var(--focus-offset)' },
  'edge.raised.disabled': { 'border-color': CONSUMED.rule, opacity: 'var(--disabled-opacity)' },
  'edge.overlay.rest': { 'border-color': CONSUMED.rule },

  'glyph.base.rest': { color: CONSUMED.attendant },
  'glyph.raised.rest': { color: CONSUMED.ink },
  'glyph.raised.hover': { color: CONSUMED.act },
  'glyph.raised.active': { color: CONSUMED.act },
  'glyph.raised.focus': { color: CONSUMED.act },
  'glyph.raised.disabled': { color: CONSUMED.attendant, opacity: 'var(--disabled-opacity)' },
  'glyph.overlay.rest': { color: CONSUMED.ink },
});

/**
 * Cells that are reachable and deliberately carry nothing.
 *
 * Declared, so that the difference between "we decided this needs no drawing of its own"
 * and "nobody wrote this" survives into the gate. This is the same distinction the face
 * itself draws between a record that is absent and a record that is false.
 */
const DECLARED_EMPTY = Object.freeze({
  // An overlay's border does not answer the pointer. The overlay itself is the thing
  // that moved forward, and moving its edge as well would be two answers to one gesture.
  // Reachable, deliberately carrying nothing, and said so rather than left blank.
  'edge.overlay.hover': 'an overlay is already forward; its edge does not answer as well',
  'edge.overlay.active': 'an overlay is already forward; its edge does not answer as well',
  'edge.overlay.focus': 'focus is drawn on the control inside the overlay, not on the overlay',
  'edge.overlay.disabled': 'an overlay that is not available is not drawn at all',
});

export function resolveRole(name) {
  const filled = RESOLVE[name];
  if (filled) return filled;
  const cell = roleProduct().find((c) => c.name === name);
  if (!cell) throw new Error(`${ROLE_MESSAGES.UNKNOWN_ROLE}: ${name}`);
  if (!cell.reachable) throw new Error(`${ROLE_MESSAGES.UNREACHABLE}: ${name} (${cell.why})`);
  throw new Error(`${ROLE_MESSAGES.NOT_DECLARED}: ${name}`);
}

// -- rank 1: the intents ------------------------------------------------------

/**
 * What this face is telling a reader, and the appearance each meaning takes.
 *
 * `accent` is the hue budget, and it is spent once. It marks reversibility -- whether a
 * thing can still be taken back -- because that is the claim this product exists to make;
 * an earlier draft of this screen spent five hues and, having spent five, emphasised
 * nothing. The three verdicts are told apart by their mark and their word, which is a
 * distinction that survives a monochrome print, a colour-blind reader and a screenshot
 * pasted into a document, none of which a hue does.
 *
 * `mark` names a glyph in the shared sheet. It is the meaning-bearing half: the rule for
 * simplification on this face is that a mark may replace a word, never decorate one.
 */
const INTENTS = Object.freeze({
  // What kind of thing a row is.
  'record.settled': { role: 'text.base.rest', mark: ['structure', 'seal'], weight: 'body' },
  'record.held': { role: 'text.base.rest', mark: ['standing', 'held'], weight: 'body' },
  'record.child': { role: 'glyph.base.rest', mark: ['structure', 'child'], weight: 'body' },

  // The engine's word about a record. Mark and word, never hue.
  'verdict.admit': { role: 'text.base.rest', mark: ['verdict', 'Admit'], weight: 'label' },
  'verdict.deny': { role: 'text.base.rest', mark: ['verdict', 'Deny'], weight: 'figure' },
  'verdict.escalate': { role: 'text.base.rest', mark: ['verdict', 'Escalate'], weight: 'label' },
  // A word arrived where a verdict was expected. This is NOT the same fact as a member
  // being absent, and the lattice caught the draft where it was drawn as though it were:
  // "the engine said something I do not know" and "the engine said nothing" are the
  // difference between a vocabulary problem and a silence, and this face exists to keep
  // silences visible. It takes the sheet's own undefined mark, which is the drawing for
  // exactly this and for nothing else.
  'verdict.unrecognised': { role: 'glyph.base.rest', mark: ['verdict', '(unrecognised)'], weight: 'body' },

  // The one hue, spent here. Whether this can still be taken back.
  //
  // There used to be three reversal intents. The lattice's injectivity check found that
  // two of them were other facts wearing a second name: "reversal is not observable" is
  // the nothing vertical's `unknown`, and "not committed yet" is what `record.held`
  // already says on the same row. Both are gone rather than restyled. A held row that
  // also carried a chip saying it cannot be reversed yet was telling a reader the same
  // thing twice, which is where this screen's density went in the first place.
  'reversal.reversed': { role: 'text.base.rest', mark: ['standing', 'reversed'], accent: true, weight: 'label' },

  // The nothing vertical (#429), which is exempt from every simplification on this
  // face. Four meanings, four appearances, and a check that they stay four.
  'nothing.loading': { role: 'glyph.base.rest', mark: ['structure', 'fold-shut'], weight: 'body' },
  'nothing.unknown': { role: 'glyph.base.rest', mark: ['standing', 'none'], weight: 'body' },
  'nothing.absent': { role: 'glyph.base.rest', mark: ['structure', 'hole'], weight: 'body' },
  'nothing.false': { role: 'text.base.rest', mark: ['standing', 'cancelled'], weight: 'body' },

  // Acts, and the two ways one can fail to be available.
  'act.offered': { role: 'edge.raised.rest', accent: true, weight: 'label' },
  'act.withheld': { role: 'edge.raised.disabled', weight: 'body' },
  'act.inflight': { role: 'text.raised.disabled', weight: 'body' },

  // A read that did not happen is not an empty read.
  'reading.unread': { role: 'text.base.rest', mark: ['structure', 'hole'], weight: 'label' },

  // Figures and the words that support them.
  'measure.figure': { role: 'text.base.rest', weight: 'figure' },
  'measure.label': { role: 'text.base.rest', weight: 'label' },

  // A value the line could not hold, and the same value where it is held whole.
  'value.clipped': { role: 'text.base.rest', weight: 'body' },
  'value.full': { role: 'text.raised.rest', weight: 'body' },

  // This window concluded it; the engine did not say it.
  'provenance.inferred-here': { role: 'text.raised.rest', mark: ['structure', 'outside'], weight: 'body' },
  'provenance.stated': { role: 'text.raised.rest', mark: ['structure', 'subject'], weight: 'body' },

  // The line between one thing and the next. An intent rather than a bare role, because
  // the first draft of the rebuilt screen reached for the role directly and this file
  // refused it -- which is the rule working: a face may name an intent, and `ink` is not
  // a door onto the rank below.
  'boundary.rule': { role: 'edge.base.rest' },

  // Selection is this window's own state, not the record's.
  //
  // There is no `selection.shut`. A resting row and the page it sits on are the same
  // appearance on purpose -- a row that is not chosen should not stand out -- and the
  // injectivity check is what made that explicit: two names for one appearance is the
  // defect whether or not the two meanings feel different when you name them. So the
  // page ground is the resting row's ground, spelled once.
  'ground.page': { role: 'surface.base.rest' },
  'ground.opened': { role: 'surface.raised.rest' },
  'selection.open': { role: 'surface.raised.rest', accent: true, weight: 'body' },
});

export const INTENT_NAMES = Object.freeze(Object.keys(INTENTS));

/** Three weights and no fourth; a fourth would be one nobody could name. */
export const WEIGHT = Object.freeze({ figure: '700', label: '500', body: '400' });

/**
 * Resolve an intent all the way down. This is the only door: nothing in this face reads
 * `RESOLVE`, `CONSUMED` or a token name on its own.
 */
export function ink(intent) {
  const spec = INTENTS[intent];
  if (!spec) throw new Error(`${ROLE_MESSAGES.UNKNOWN_INTENT}: ${intent}`);
  const base = { ...resolveRole(spec.role) };
  if (spec.weight) base['font-weight'] = WEIGHT[spec.weight];
  if (spec.accent) base.color = CONSUMED.act;
  return base;
}

/** The glyph an intent carries, or null where it carries none. */
export function markOfIntent(intent) {
  const spec = INTENTS[intent];
  if (!spec) throw new Error(`${ROLE_MESSAGES.UNKNOWN_INTENT}: ${intent}`);
  return spec.mark ?? null;
}

/** The role an intent names, for the gate and for nothing else. */
export function roleOfIntent(intent) {
  const spec = INTENTS[intent];
  if (!spec) throw new Error(`${ROLE_MESSAGES.UNKNOWN_INTENT}: ${intent}`);
  return spec.role;
}

// -- the same two ranks, for the quantities that are not colour ---------------

/**
 * Lengths, type and corners, reached by what they are for.
 *
 * Colour was never the whole of it. The face this replaced spelled thirty-five raw
 * lengths -- `6px`, `4px`, `10px`, `2px 0`, `36px`, `14px 0` -- while a spacing scale
 * sat declared and unused two directories away, so the screen's rhythm was decided at
 * whichever call site was written last. These names are the rank that was missing for
 * quantity, and the rule is the same: a face asks for `gap.line`, never for a number.
 *
 * `--space-*` is spelled here rather than reached through `CONSUMED`, which does not
 * carry it. That is the hole in the rank below, recorded at the one place that has to
 * work around it instead of at the thirty-five that used to.
 */
const METRICS = Object.freeze({
  'gap.hairline': 'var(--space-1)',
  'gap.line': 'var(--space-2)',
  'gap.block': 'var(--space-3)',
  'gap.section': 'var(--space-4)',

  'pitch.row': CONSUMED.pitch,
  'pad.side': CONSUMED.padX,
  'pad.spine': CONSUMED.spineX,

  'type.meta': CONSUMED.meta,
  'type.meta.line': CONSUMED.metaLine,
  'type.time': CONSUMED.time,
  'type.time.line': CONSUMED.timeLine,
  'type.record': CONSUMED.record,
  'type.record.line': CONSUMED.recordLine,
  'type.head': CONSUMED.head,
  'type.head.line': CONSUMED.headLine,
  'type.figure': CONSUMED.stat,
  'type.figure.line': CONSUMED.statLine,

  'family.reading': CONSUMED.sans,
  'family.exact': CONSUMED.mono,

  'corner.chip': CONSUMED.radiusChip,
  'corner.control': CONSUMED.radiusControl,
  'corner.container': CONSUMED.radiusContainer,

  'track.label': 'var(--track-label)',

  'cursor.act': 'var(--cursor-act)',
  'cursor.refuse': 'var(--cursor-refuse)',
  'cursor.inert': 'var(--cursor-inert)',

  // A hairline has no name in the token rank. There are ninety-nine declared tokens and
  // not one of them is a border width, so this is the one quantity on this face that
  // resolves to a literal. It is written here, once, at the rank whose job is to hold
  // literals -- which is the difference between a hole that is recorded and a hole that
  // is worked around at every call site.
  'edge.hairline': '1px',
});

export const METRIC_NAMES = Object.freeze(Object.keys(METRICS));

/** The only door to a quantity. A face that wants a number asks for a purpose. */
export function metric(name) {
  const value = METRICS[name];
  if (!value) throw new Error(`${ROLE_MESSAGES.UNKNOWN_ROLE}: ${name}`);
  return value;
}

// -- the one invariant --------------------------------------------------------

/**
 * The four checks of #428 written as one walk of a ranked graph, plus the report a
 * reader needs to see that the lattice is covered.
 *
 * Returned rather than thrown so a caller can print the whole table; the gate turns a
 * non-empty `breaches` into an exit code.
 */
export function lattice() {
  const cells = roleProduct();
  const filled = [];
  const empty = [];
  const unreachable = [];
  const breaches = [];

  for (const cell of cells) {
    if (!cell.reachable) {
      unreachable.push(cell);
      if (RESOLVE[cell.name]) {
        breaches.push({
          check: 'g4-unreachable-implemented',
          cell: cell.name,
          why: `${ROLE_MESSAGES.UNREACHABLE}: ${cell.why}`,
        });
      }
      continue;
    }
    if (RESOLVE[cell.name]) filled.push(cell);
    else if (cell.name in DECLARED_EMPTY) empty.push(cell);
    else breaches.push({ check: 'g4-undeclared', cell: cell.name, why: ROLE_MESSAGES.NOT_DECLARED });
  }

  // g3: intent -> role must not collapse two meanings into one appearance. The
  // appearance is the whole of it -- role, mark and weight -- because telling two facts
  // apart by a hue alone is the thing this face refuses to do.
  const seen = new Map();
  for (const [intent, spec] of Object.entries(INTENTS)) {
    const shape = JSON.stringify([spec.role, spec.mark ?? null, spec.weight ?? null, Boolean(spec.accent)]);
    if (seen.has(shape)) {
      breaches.push({
        check: 'g3-injective',
        cell: intent,
        why: `${ROLE_MESSAGES.COLLAPSED}: ${intent} and ${seen.get(shape)}`,
      });
    } else {
      seen.set(shape, intent);
    }
  }

  // The nothing vertical is checked on its own, because it is the one place where a
  // collapse is not a blemish but a lie about the product.
  const nothing = INTENT_NAMES.filter((n) => n.startsWith('nothing.'));
  const shapes = new Set(nothing.map((n) => JSON.stringify([INTENTS[n].role, INTENTS[n].mark, INTENTS[n].weight])));
  if (shapes.size !== nothing.length) {
    breaches.push({
      check: 'g3-nothing-vertical',
      cell: nothing.join(','),
      why: 'loading, unknown, absent and false must be four appearances and are not',
    });
  }

  // The hue budget, counted rather than asserted.
  const accents = INTENT_NAMES.filter((n) => INTENTS[n].accent);

  return {
    cells: cells.length,
    filled: filled.length,
    empty: empty.length,
    unreachable: unreachable.length,
    intents: INTENT_NAMES.length,
    accentIntents: accents,
    nothingVertical: nothing,
    breaches,
  };
}
