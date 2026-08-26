// SPDX-License-Identifier: Apache-2.0
// A stand-in document, and a warning attached to it.
//
// The parts this face draws with no longer import a node builtin anywhere in their
// static graph (parts/src/tokens.mjs reads the stylesheet of record from a
// build-time-generated module now, not from disk at import time -- req/02 W15), so
// the drawing code can in fact be loaded straight into a real window with no build
// step. faces/ledger/tools/browser-mount-smoke.mjs does exactly that, against a real
// renderer, and is the load-bearing evidence for W15's third clause.
//
// This file still exists because that real-renderer mount is slow and needs a
// headless Chrome binary on the machine running the test, and most of what W2 checks
// -- host filled, unmount empties it, unmounting twice does not throw -- does not
// need real layout to verify. Mounting is exercised here, fast and renderer-free,
// against an object that answers the four calls `render` makes, and that is all it
// proves: that the face asks a document for the right things in the right order.
//
// It proves nothing about drawing. The two defects this rebuild exists to not repeat
// were both invisible to a fake document (req/03 N-1, N-2), so every visual claim in
// this face is made from the fixture in front of a real renderer instead (shoot.mjs,
// and now browser-mount-smoke.mjs for the mounted-live-in-a-window case too).

const SVG_NS = 'http://www.w3.org/2000/svg';

function node(tag, namespace = null) {
  const self = {
    tag,
    namespace,
    attrs: {},
    childNodes: [],
    listeners: [],
    parentNode: null,
    get firstChild() { return self.childNodes[0] ?? null; },
    setAttribute(name, value) { self.attrs[name] = String(value); },
    getAttribute(name) { return Object.prototype.hasOwnProperty.call(self.attrs, name) ? self.attrs[name] : null; },
    appendChild(child) { child.parentNode = self; self.childNodes.push(child); return child; },
    removeChild(child) {
      const at = self.childNodes.indexOf(child);
      if (at >= 0) self.childNodes.splice(at, 1);
      child.parentNode = null;
      return child;
    },
    /**
     * The one query the face makes of a node it was handed: which enclosing element
     * is the control that was pressed. Two selector shapes are answered because two
     * are asked for -- an attribute in brackets and a bare tag name -- and anything
     * else raises rather than quietly returning null, because a stand-in that answers
     * "no" to a question it does not understand is a stand-in that makes a handler
     * look inert when it is not.
     */
    closest(selector) {
      const attribute = /^\[([a-z-]+)\]$/.exec(selector);
      if (!attribute && !/^[a-z]+$/.test(selector)) throw new Error(`this stand-in does not answer that selector: ${selector}`);
      let at = self;
      while (at) {
        const hit = attribute
          ? at.attrs && Object.prototype.hasOwnProperty.call(at.attrs, attribute[1])
          : at.tag === selector;
        if (hit) return at;
        at = at.parentNode;
      }
      return null;
    },
    /**
     * Whether a node is this one or inside it. The face asks this of its host to tell
     * a press that landed on it from one that landed on the shell around it, and a
     * stand-in that answered by guessing would make the dismissal it decides look
     * correct while being wrong in a window.
     */
    contains(other) {
      let at = other;
      while (at) {
        if (at === self) return true;
        at = at.parentNode;
      }
      return false;
    },
    addEventListener(type, handler) { self.listeners.push({ type, handler }); },
    removeEventListener(type, handler) {
      const at = self.listeners.findIndex((l) => l.type === type && l.handler === handler);
      if (at >= 0) self.listeners.splice(at, 1);
    },
  };
  return self;
}

/**
 * The document, which now takes listeners as well as building nodes.
 *
 * The face puts two of its handlers on the document rather than on its own host, and
 * both are about a press or a key that happened somewhere this face does not own -- a
 * click on the shell's chrome, Escape struck while the focus has left the face. A
 * stand-in with no way to carry those would leave the only two handlers whose whole
 * point is being outside the host untested.
 */
export function standInDocument() {
  const listeners = [];
  return {
    listeners,
    createElement: (tag) => node(tag),
    createElementNS: (ns, tag) => node(tag, ns === SVG_NS ? 'svg' : ns),
    createTextNode: (value) => ({ text: String(value), childNodes: [], parentNode: null }),
    addEventListener(type, handler) { listeners.push({ type, handler }); },
    removeEventListener(type, handler) {
      const at = listeners.findIndex((l) => l.type === type && l.handler === handler);
      if (at >= 0) listeners.splice(at, 1);
    },
  };
}

export function standInHost() {
  const host = node('div');
  host.ownerDocument = standInDocument();
  return host;
}

/** Every node in a mounted stand-in tree, parents first. */
export function nodesOf(root) {
  const out = [];
  const visit = (n) => {
    out.push(n);
    for (const child of n.childNodes ?? []) visit(child);
  };
  visit(root);
  return out;
}

/** Every readable string in a mounted stand-in tree, in order. */
export function textOfHost(root) {
  return nodesOf(root).map((n) => (typeof n.text === 'string' ? n.text : '')).join('');
}

/**
 * A press, delivered the way a window delivers one: to the listener the face put on
 * the host, with the node under the pointer as the event's target.
 *
 * This is not a claim about layout, and it does not need to be. What it exercises is
 * the handler itself -- the closure that reads the face's state, sends an act, and
 * paints what came back -- which is where a click that is not serialised loses one of
 * two acts. That failure is a property of the handler and not of the renderer: it
 * reproduces here in milliseconds, deterministically, and the same defect was seen in
 * a real browser first (req/103 finding 1, 4 of 4 act buttons).
 */
export function press(host, target) {
  if (!target) throw new Error('nothing was pressed: no such node in this tree');
  let defaulted = true;
  const event = { type: 'click', target, preventDefault() { defaulted = false; } };
  for (const listener of [...host.listeners]) {
    if (listener.type === 'click') listener.handler(event);
  }
  return { defaulted };
}

/**
 * The other button, delivered the way a window delivers one.
 *
 * Same shape as press() above and the same reason for existing: what a right-click does
 * is decided by a closure reading and writing this window's state, and that closure is
 * what has to be exercised. `defaulted` is what the caller checks to know the native
 * menu was refused -- a face that opened its own menu and left the browser's one
 * underneath it has drawn two menus for one press.
 */
export function rightPress(host, target) {
  if (!target) throw new Error('nothing was pressed: no such node in this tree');
  let defaulted = true;
  const event = { type: 'contextmenu', target, preventDefault() { defaulted = false; } };
  for (const listener of [...host.listeners]) {
    if (listener.type === 'contextmenu') listener.handler(event);
  }
  return { defaulted };
}

/** A key struck anywhere in the document, which is where the face listens for one. */
export function strike(doc, key) {
  const event = { type: 'keydown', key, preventDefault() {} };
  for (const listener of [...doc.listeners]) {
    if (listener.type === 'keydown') listener.handler(event);
  }
}

/** A press that landed on the document outside the face's own host. */
export function pressAway(doc, target) {
  const event = { type: 'click', target, preventDefault() {} };
  for (const listener of [...doc.listeners]) {
    if (listener.type === 'click') listener.handler(event);
  }
}

/** The first node in a mounted tree carrying this attribute with this value. */
export function nodeWith(root, name, value) {
  return nodesOf(root).find((n) => n.attrs && n.attrs[name] === value) ?? null;
}

export function attrValues(root, name) {
  return nodesOf(root)
    .filter((n) => n.attrs && Object.prototype.hasOwnProperty.call(n.attrs, name))
    .map((n) => n.attrs[name]);
}
