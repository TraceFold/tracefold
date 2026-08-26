// SPDX-License-Identifier: Apache-2.0
// The one road. Every difference between one state and the next passes through
// `#commit`, and `#commit` is reached from exactly one assignment in this file.
//
// It is a private field rather than a convention because a convention is a count of the
// places people remembered. `#current` cannot be written from another module -- the
// language refuses, not the reviewer -- so "how many functions change the state" and
// "how many go through the act path" are the same number by construction, and the gate
// that counts them (tools/gates.mjs, W4) is checking that this file did not grow a
// second assignment rather than checking that nobody was careless somewhere else.
//
// History holds inverses, not states. Nothing here ever copies the state.

import { ACTS, TRACK, Refused, REFUSAL } from './acts.mjs';
import { digestOfState, serialise } from './layout.mjs';

/** Kept, and said. A bounded history that does not say it is bounded is a lost history. */
export const HISTORY_DEPTH = 512;

export const OUTCOME = Object.freeze({
  MOVED: 'moved',
  UNCHANGED: 'unchanged',
  REFUSED: 'refused',
  ELSEWHERE: 'elsewhere',
});

/**
 * Two steps of one gesture, made into one step.
 *
 * [B5] A sash drag is not one act; it is one act per pointermove, and a fourteen-move
 * drag measured fourteen entries into the history -- fourteen presses of undo to reverse
 * one thing the reader did once. The cure is not a special case for sashes and not a
 * timer: an act is told which gesture it belongs to, and two acts that belong to the same
 * one are the same entry.
 *
 * The pair's "before" is the FIRST act's before and its "after" is the latest, so the
 * entry stands for the whole run. Its inverse undoes the later half first and the earlier
 * half second, which is the only order that arrives back where the gesture began; its
 * apply runs them the other way round for redo. What the pair *holds* is what the first
 * act captured, because that is the value undo puts back -- and because
 * tools/inverse_census.mjs reads exactly that to say what history retains, a merge that
 * dropped it would have made the census quietly wrong rather than loudly.
 */
function coalesce(first, next) {
  const invert = (state) => first.invert(next.invert(state));
  invert.held = first.invert?.held;
  const apply = (state) => next.apply(first.apply(state));
  return { verb: first.verb, gesture: first.gesture, invert, apply, before: first.before, after: next.after };
}

export class ShellState {
  #current;

  #past = [];

  #ahead = [];

  #receipt = [];

  #dropped = 0;

  #read;

  constructor(read, initial) {
    this.#read = read;
    this.#commit(initial, null);
  }

  /** The only assignment. tools/gates.mjs counts it and expects to find one. */
  #commit(next, row) {
    this.#current = next;
    if (row) {
      this.#receipt.push(Object.freeze({ seq: this.#receipt.length + 1, ...row }));
    }
  }

  get state() { return this.#current; }

  get line() { return serialise(this.#current); }

  get digest() { return digestOfState(this.#current); }

  get receipt() { return Object.freeze([...this.#receipt]); }

  get depth() { return { past: this.#past.length, ahead: this.#ahead.length, dropped: this.#dropped }; }

  /** What history is carrying, so a claim about its cost can be measured, not asserted. */
  get carried() { return this.#past.map((e) => e.verb); }

  /** The captured half of each inverse, for the same reason. */
  get heldByHistory() { return this.#past.map((e) => e.invert?.held ?? null); }

  /**
   * The one place an outcome is built -- and the one place a reason is required.
   *
   * req/811 §8-7 named this as the single best thing to take from the reference tool, and
   * then improved on it. That tool attaches a `why` string to every disabled item, which
   * is good, and does it at the widget: three independent strings that can drift apart,
   * and do. Here the reason is required on the outcome ITSELF, so a refusal cannot be
   * constructed without one, and the menu, the tooltip and the status strip are three
   * renderings of one value rather than three values that agree today.
   *
   * That is also the mechanical cure for the class of defect §8-2b belongs to: a state and
   * its explanation drifting apart. They cannot drift if there is only one of them.
   *
   * MOVED carries no reason on purpose. "It worked" is not a reason, and demanding a
   * sentence there is how a codebase fills up with `said: 'ok'`.
   */
  #record(verb, before, after, outcome, said) {
    const mustSay = outcome === OUTCOME.REFUSED || outcome === OUTCOME.UNCHANGED || outcome === OUTCOME.ELSEWHERE;
    if (mustSay && !(typeof said === 'string' && said.trim().length > 0)) {
      throw new TypeError(`${verb} answered ${outcome} with no reason; a refusal carries its why or it is not a refusal`);
    }
    return Object.freeze({ verb, before, after, outcome, said: said ?? null });
  }

  /**
   * @returns {{outcome: string, verb: string, before: string, after: string, said: string|null}}
   *   Refusals are answers, not exceptions: a placement the shell will not make has to be
   *   distinguishable from one it made, and both from one that changed nothing.
   */
  perform(verb, args = {}, { gesture = null } = {}) {
    const entry = ACTS[verb];
    const before = this.digest;
    if (!entry) {
      return this.#record(verb, before, before, OUTCOME.REFUSED, `there is no act called "${verb}"`);
    }
    if (entry.track === TRACK.FACE) {
      return this.#record(verb, before, before, OUTCOME.ELSEWHERE, entry.said);
    }

    let built;
    try {
      built = entry.build(this.#read, this.#current, args);
    } catch (error) {
      if (error instanceof Refused) return this.#record(verb, before, before, OUTCOME.REFUSED, error.message);
      throw error;
    }
    if (built === null) {
      return this.#record(verb, before, before, OUTCOME.UNCHANGED, 'this act would leave the shell as it stands');
    }

    const next = built.apply(this.#current);
    const after = digestOfState(next);
    const row = this.#record(verb, before, after, OUTCOME.MOVED);
    this.#remember({ verb, gesture, invert: built.invert, apply: built.apply, before, after });
    this.#ahead.length = 0;
    this.#commit(next, row);
    return row;
  }

  /**
   * The one place history grows -- and the one place it does not.
   *
   * A step naming no gesture is always its own entry, so nothing about the acts a person
   * performs one at a time changes. A step naming a gesture the step behind it already
   * names folds into that one instead, and the history is a step shorter than the number
   * of acts by exactly the number of acts a single gesture produced. The receipt is left
   * alone on purpose: it is the record of what was performed, and coalescing that would
   * be losing evidence rather than saving a reader keystrokes.
   */
  #remember(step) {
    const behind = this.#past[this.#past.length - 1];
    if (step.gesture !== null && step.gesture !== undefined && behind && behind.gesture === step.gesture) {
      this.#past[this.#past.length - 1] = coalesce(behind, step);
      return;
    }
    this.#past.push(step);
    if (this.#past.length > HISTORY_DEPTH) {
      this.#past.shift();
      this.#dropped += 1;
    }
  }

  /** Undo is itself an act: it is recorded, and it passes the same road. */
  undo() {
    const before = this.digest;
    const entry = this.#past.pop();
    if (!entry) return this.#record('undo', before, before, OUTCOME.UNCHANGED, 'there is nothing behind this state');
    const next = entry.invert(this.#current);
    const after = digestOfState(next);
    const row = this.#record(`undo(${entry.verb})`, before, after, OUTCOME.MOVED);
    this.#ahead.push(entry);
    this.#commit(next, row);
    return row;
  }

  redo() {
    const before = this.digest;
    const entry = this.#ahead.pop();
    if (!entry) return this.#record('redo', before, before, OUTCOME.UNCHANGED, 'there is nothing ahead of this state');
    const next = entry.apply(this.#current);
    const after = digestOfState(next);
    const row = this.#record(`redo(${entry.verb})`, before, after, OUTCOME.MOVED);
    this.#past.push(entry);
    this.#commit(next, row);
    return row;
  }

  /** The shell holds no store. This is a value going out, not a database being read. */
  toLine() { return this.line; }
}

export { REFUSAL };
