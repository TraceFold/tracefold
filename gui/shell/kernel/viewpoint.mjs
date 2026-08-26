// SPDX-License-Identifier: Apache-2.0
// Where a person is looking. Not state.
//
// The distinction is kept by type, not by intention. The shell's state is whatever
// layout.serialise can write on a line; this object is not writable to that line and has
// no act in the registry, so undo cannot reach it even by mistake. The system this was
// derived from declared the same rule in a comment and then put the focus inside the
// snapshot it rewound, so going back a step moved the cursor. A rule that lives in a
// comment is enforced by whoever last read the comment.

export class Viewpoint {
  #focus = null;

  #scroll = new Map();

  #hover = null;

  get focus() { return this.#focus; }

  /** @param {string|null} where a pane path written as a string, or null */
  look(where) { this.#focus = where; return this.#focus; }

  hover(where) { this.#hover = where; return this.#hover; }

  get hovered() { return this.#hover; }

  keep(where, top) { this.#scroll.set(where, top); }

  kept(where) { return this.#scroll.get(where) ?? 0; }

  /** Deliberately not `toLine`. Nothing here belongs on the shell's line. */
  get reading() {
    return Object.freeze({ focus: this.#focus, hover: this.#hover, scrolls: this.#scroll.size });
  }
}

/** Said out loud so a reviewer can grep for it and a test can assert it. */
export const NOT_STATE = Object.freeze(['focus', 'scroll', 'hover']);
