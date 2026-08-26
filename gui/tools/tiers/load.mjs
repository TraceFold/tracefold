// SPDX-License-Identifier: Apache-2.0
// T1 -- the module is evaluated rather than read.
//
// This tier exists because eighty-five text readings once passed over a file that
// could not be parsed. A regular expression cannot tell a live module from a dead
// one; an import can, and that is the entire claim this tier makes.

import url from 'node:url';
import path from 'node:path';

export const LOAD_MESSAGES = {
  EVALUATED: 'imported and evaluated',
  THREW: 'threw while being evaluated',
  NO_EXPORTS: 'evaluated but exposes nothing, which a module in this tree should not do',
};

export async function evaluates(entry, world) {
  const absolute = path.join(world.manifest.root, entry.path);
  try {
    // Cache-busted so a second run in one process is a second evaluation.
    const module = await import(`${url.pathToFileURL(absolute).href}?load=${entry.digest}`);
    if (Object.keys(module).length === 0) return `${LOAD_MESSAGES.NO_EXPORTS}: ${entry.path}`;
    return true;
  } catch (err) {
    return `${LOAD_MESSAGES.THREW}: ${entry.path} -- ${err.message}`;
  }
}
