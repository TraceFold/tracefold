// SPDX-License-Identifier: Apache-2.0
// The command a person could type to arrive back at what they are looking at.
//
// SS551 asks for a copyable command block per object view -- "the gx verb that
// reproduces what is shown". This module holds no DOM, no clipboard, and no face
// id literal of its own; it takes the same act shape the frame already sends to
// `#act()` and formats it as a line of text. Pure in, pure out, so it is testable
// with `node --test` the way every other kernel module here is.

const flag = (name, value) => `--${name} ${value}`;

/**
 * @param {'dock'|'stage'} kind
 * @param {object} args
 * @returns {string} a `gx` line that reproduces the one thing being looked at
 */
export function commandFor(kind, args) {
  if (kind === 'dock') {
    const { index, side, at, id } = args;
    const parts = [flag('index', index), flag('side', side), flag('at', at)];
    if (id !== undefined && id !== null) parts.push(flag('id', id));
    return `gx dock:go ${parts.join(' ')}`;
  }
  if (kind === 'stage') {
    const { index, path, at, id } = args;
    const parts = [flag('index', index), flag('path', Array.isArray(path) ? path.join('.') : path), flag('at', at)];
    if (id !== undefined && id !== null) parts.push(flag('id', id));
    return `gx tab:go ${parts.join(' ')}`;
  }
  throw new RangeError(`commandFor does not know the kind "${kind}"`);
}
