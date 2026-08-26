// SPDX-License-Identifier: Apache-2.0
// A stand-in document.
//
// It answers the handful of calls `render` makes (createElement, createElementNS,
// createTextNode, appendChild, removeChild, setAttribute, addEventListener) and
// nothing else, which is enough to prove that mounting fills a host and unmounting
// empties it. It proves nothing about how anything looks: the two defects this
// rebuild exists not to repeat were both invisible to a fake document, so every
// visual claim this face makes comes from tools/shoot.mjs, in front of a real
// renderer, instead.

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
    addEventListener(type, handler) { self.listeners.push({ type, handler }); },
    removeEventListener(type, handler) {
      const at = self.listeners.findIndex((l) => l.type === type && l.handler === handler);
      if (at >= 0) self.listeners.splice(at, 1);
    },
  };
  return self;
}

export function standInDocument() {
  return {
    createElement: (tag) => node(tag),
    createElementNS: (ns, tag) => node(tag, ns === SVG_NS ? 'svg' : ns),
    createTextNode: (value) => ({ text: String(value), childNodes: [], parentNode: null }),
  };
}

export function standInHost() {
  const host = node('div');
  host.ownerDocument = standInDocument();
  return host;
}

/** Every node under a mounted stand-in tree, parents before children. */
export function nodesOf(root) {
  const out = [];
  const visit = (n) => {
    out.push(n);
    for (const child of n.childNodes ?? []) visit(child);
  };
  visit(root);
  return out;
}

export function textOfHost(root) {
  return nodesOf(root).map((n) => (typeof n.text === 'string' ? n.text : '')).join('');
}

export function attrValues(root, name) {
  return nodesOf(root)
    .filter((n) => n.attrs && Object.prototype.hasOwnProperty.call(n.attrs, name))
    .map((n) => n.attrs[name]);
}
