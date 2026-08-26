// SPDX-License-Identifier: Apache-2.0
// The mount contract, and the count of how often it was used.
//
//   mount(host, port, notices) -> unmount
//
// Three arguments and a function back. The return value is required because a face that
// is only ever started has no moment at which it can stop: its listeners, its timers and
// its observers accumulate for as long as the window is open, and nothing in the shell is
// able to end them. So a face that answers with anything else is refused at mount time,
// loudly, rather than leaking quietly.
//
// The counters exist because "the shell only remounts on a face swap" is a claim about
// numbers. Switching a tab inside one pane must not unmount another pane's face; swapping
// the face in a pane must unmount exactly one. Both are read off `tally`.

export const MOUNT_ARITY = 3;

export const MOUNT_FAULT = Object.freeze({
  NO_UNMOUNT: 'no-unmount',
  THREW: 'threw',
  ABSENT: 'absent',
});

/**
 * The one guard behind W8: whether the face wanted at `key` differs from the one already
 * standing there. `render.mjs` calls this on every redraw, for every host, and touches
 * `mounted` only when the answer is true -- so a redraw caused by something that did not
 * change what a place shows (a tab switch in another pane, a focus move, a status line
 * update) costs one comparison per host and never reaches `raise`/`lower`. This is a
 * plain function, not a method on `Mounted`, so it can be read and tested without a DOM:
 * the thing W8 claims is "this comparison, and nothing else, decides", and a claim about
 * a comparison should be checkable by looking at the comparison.
 *
 * @param {Mounted} mounted
 * @param {string} key
 * @param {string|null|undefined} wantedId
 * @returns {boolean} true when `key` is about to change what it shows
 */
export function changing(mounted, key, wantedId) {
  return mounted.idAt(key) !== (wantedId ?? null);
}

export class Mounted {
  #live = new Map();

  #tally = { mounted: 0, unmounted: 0, refused: 0 };

  get tally() { return Object.freeze({ ...this.#tally }); }

  get live() { return Object.freeze([...this.#live.keys()]); }

  has(key) { return this.#live.has(key); }

  /**
   * @param {string} key the place, not the face: a pane path or a dock side
   * @param {{id: string, mount: Function, consumes: string[]}} face
   * @param {Element} host
   * @param {object} port the membrane's surface, already watched
   * @param {Array} notices
   */
  raise(key, face, host, port, notices) {
    if (this.#live.has(key)) this.lower(key);
    if (typeof face?.mount !== 'function') {
      this.#tally.refused += 1;
      throw new TypeError(`"${face?.id}" has no mount, so nothing can be raised at ${key}`);
    }
    if (face.mount.length > MOUNT_ARITY) {
      this.#tally.refused += 1;
      throw new TypeError(`"${face.id}" asks for ${face.mount.length} arguments; the contract hands ${MOUNT_ARITY} and adding a fourth would break every face at once`);
    }
    let unmount;
    try {
      unmount = face.mount(host, port, notices);
    } catch (error) {
      this.#tally.refused += 1;
      throw new Error(`"${face.id}" threw while mounting at ${key}: ${error.message}`, { cause: error });
    }
    if (typeof unmount !== 'function') {
      this.#tally.refused += 1;
      throw new TypeError(`"${face.id}" returned ${typeof unmount} instead of its unmount; a face that cannot be stopped cannot be mounted`);
    }
    this.#live.set(key, { id: face.id, unmount, host });
    this.#tally.mounted += 1;
    return unmount;
  }

  lower(key) {
    const held = this.#live.get(key);
    if (!held) return false;
    this.#live.delete(key);
    this.#tally.unmounted += 1;
    held.unmount();
    return true;
  }

  /** The id standing at a place, so the renderer can tell a tab switch from a face swap. */
  idAt(key) { return this.#live.get(key)?.id ?? null; }

  lowerAll() { for (const key of this.live) this.lower(key); }
}
