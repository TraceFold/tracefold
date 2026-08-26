// SPDX-License-Identifier: Apache-2.0
// The mount contract, tested without a browser: a host is anything with an ownerDocument
// as far as this module is concerned, because the contract is about the return value and
// the counters, not about the DOM.

import test from 'node:test';
import assert from 'node:assert/strict';

import { Mounted, MOUNT_ARITY } from '../kernel/mount.mjs';
import { Viewpoint, NOT_STATE } from '../kernel/viewpoint.mjs';
import { serialise } from '../kernel/layout.mjs';
import { readManifest } from '../kernel/manifest.mjs';
import { openingState } from '../kernel/shell.mjs';
import { MANIFEST } from '../demo/manifest.gen.mjs';

const host = () => ({ appended: 0, append() { this.appended += 1; } });

const good = (id = 'probe-a') => ({ id, title: id, mount: (h, p, n) => () => { h.stopped = true; } });

test('a face that answers with an unmount is raised, and the tally says so', () => {
  const mounted = new Mounted();
  const where = host();
  const unmount = mounted.raise('stage:0', good(), where, null, []);
  assert.equal(typeof unmount, 'function');
  assert.equal(mounted.idAt('stage:0'), 'probe-a');
  assert.deepEqual(mounted.tally, { mounted: 1, unmounted: 0, refused: 0 });
  assert.ok(mounted.lower('stage:0'));
  assert.equal(where.stopped, true, 'the unmount was never called');
  assert.equal(mounted.idAt('stage:0'), null);
  assert.deepEqual(mounted.tally, { mounted: 1, unmounted: 1, refused: 0 });
});

test('a face that cannot be stopped cannot be mounted', () => {
  const mounted = new Mounted();
  assert.throws(() => mounted.raise('a', { id: 'x', mount: () => undefined }, host(), null, []), /instead of its unmount/);
  assert.throws(() => mounted.raise('a', { id: 'x', mount: () => 'later' }, host(), null, []), /instead of its unmount/);
  assert.throws(() => mounted.raise('a', { id: 'x' }, host(), null, []), /has no mount/);
  assert.equal(mounted.tally.refused, 3);
  assert.equal(mounted.tally.mounted, 0);
});

test('a face asking for a fourth argument is refused, so the contract cannot widen', () => {
  const mounted = new Mounted();
  const greedy = { id: 'x', mount: (a, b, c, d) => () => {} };
  assert.equal(MOUNT_ARITY, 3);
  assert.throws(() => mounted.raise('a', greedy, host(), null, []), /adding a fourth/);
});

test('a face that throws while mounting is named, and does not take the shell with it', () => {
  const mounted = new Mounted();
  const broken = { id: 'sheet-x', mount: () => { throw new Error('paint was never closed'); } };
  assert.throws(() => mounted.raise('stage:0', broken, host(), null, []), /"sheet-x" threw while mounting.*paint was never closed/s);
  assert.equal(mounted.live.length, 0);
});

test('raising over a live place lowers the one that stood there first', () => {
  const mounted = new Mounted();
  mounted.raise('stage:0', good('a'), host(), null, []);
  mounted.raise('stage:0', good('b'), host(), null, []);
  assert.equal(mounted.idAt('stage:0'), 'b');
  assert.equal(mounted.tally.unmounted, 1, 'the first face was left running under the second');
  mounted.lowerAll();
  assert.equal(mounted.live.length, 0);
});

test('the viewpoint is not on the line, by construction and not by intention', () => {
  const read = readManifest(MANIFEST);
  const line = serialise(openingState(read));
  const view = new Viewpoint();
  view.look('0.1');
  view.hover('rail');
  view.keep('0.1', 420);
  assert.equal(view.focus, '0.1');
  assert.equal(view.kept('0.1'), 420);
  assert.equal(view.reading.scrolls, 1);
  for (const name of NOT_STATE) assert.ok(!line.includes(name), `the line carries "${name}"`);
  assert.ok(!line.includes('0.1'));
  assert.equal(typeof view.toLine, 'undefined', 'the viewpoint offers a way onto the line');
});
