// SPDX-License-Identifier: Apache-2.0
// The dismiss table. Every layer number in the shell is written on this page and nowhere
// else, which is the point: numbers scattered through the things they order cannot be
// read as an order at all, and the first person to ask "what closes first" has to read
// every file to find out.
//
// A layer registered twice throws. Two things at the same depth would be dismissed in
// whatever order they happened to register in, and an order that depends on load order
// is not an order, it is a coincidence that has not broken yet.

export const LAYERS = Object.freeze({
  MENU: 10,
  PROMPT: 20,
  OVERLAY: 30,
  DRAG: 40,
  DOCK: 50,
});

export const LAYER_NUMBERS = Object.freeze(Object.values(LAYERS).sort((a, b) => a - b));

export class Dismiss {
  #open = new Map();

  /**
   * @param {number} layer one of LAYERS
   * @param {() => boolean} close answers true when it took the dismissal
   * @returns {() => void} the release
   */
  hold(layer, close) {
    if (!LAYER_NUMBERS.includes(layer)) {
      throw new RangeError(`${layer} is not a dismiss layer; the layers are ${LAYER_NUMBERS.join(', ')}`);
    }
    if (this.#open.has(layer)) {
      throw new Error(`layer ${layer} is already held; two holders at one depth have no stated order`);
    }
    this.#open.set(layer, close);
    return () => { this.#open.delete(layer); };
  }

  /** @returns {number|null} the layer that took it, or null when nothing was open */
  dismiss() {
    for (const layer of LAYER_NUMBERS) {
      const close = this.#open.get(layer);
      if (close && close() !== false) return layer;
    }
    return null;
  }

  get held() { return Object.freeze([...this.#open.keys()].sort((a, b) => a - b)); }
}
