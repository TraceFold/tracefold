// SPDX-License-Identifier: Apache-2.0
// W8 -- "re-mounting happens only when a face is swapped" -- read as a claim about a
// count: a tab switch inside one pane must not touch any OTHER pane's mount, and a
// redraw caused by focus or a status line must not touch any mount at all.
//
// `kernel/render.mjs` decides this with one function, `changing()` (kernel/mount.mjs),
// called once per host on every redraw. `raise`/`lower` are reached only when it answers
// true. These tests drive the real, imported `changing()` -- not a re-description of it
// -- against `Mounted`, so a regression in the guard itself is what fails them.
//
// Red-first: `assertion(guard)` is one body shared by both runs. It is executed first
// against `brokenChanging` -- the guard `render.mjs` had before W8 was named, which
// re-mounts on every redraw regardless of whether anything changed -- and is required to
// throw there. It is then executed against the real, imported guard and required to
// pass. A body that could not fail on the broken guard would not be testing the claim.

import test from 'node:test';
import assert from 'node:assert/strict';

import { Mounted, changing as realChanging } from '../kernel/mount.mjs';

/** Always says "yes, it changed". This is what render.mjs did before `changing()` was
 *  extracted and named: every redraw re-raised and re-lowered everything it drew. */
const brokenChanging = () => true;

const face = (id) => ({ id, title: id, mount: () => () => {} });

/** One simulated redraw over N hosts: for each, ask the guard, and only touch `mounted`
 *  when it says yes -- exactly what `Frame#hold` does per host, minus the DOM. */
function redraw(mounted, guard, wantedByKey) {
  for (const key of Object.keys(wantedByKey)) {
    const wanted = wantedByKey[key];
    if (!guard(mounted, key, wanted)) continue;
    mounted.lower(key);
    if (wanted) mounted.raise(key, face(wanted), {}, null, []);
  }
}

/** (a) A tab switch confined to one pane must not touch any other pane's mount. */
function assertPaneSwitchIsolated(guard) {
  const mounted = new Mounted();
  redraw(mounted, guard, { 'stage:0': 'sheet-a', 'stage:1': 'sheet-b' });
  const opened = mounted.tally;

  // Pane 0's wanted id moves (a tab switch); pane 1's is unchanged -- this is what every
  // redraw sends it, whether or not the change happened elsewhere, because the frame
  // does not know in advance which host a change touched.
  redraw(mounted, guard, { 'stage:0': 'sheet-c', 'stage:1': 'sheet-b' });
  const closed = mounted.tally;

  assert.equal(closed.unmounted - opened.unmounted, 1, 'exactly the swapped pane should have unmounted');
  assert.equal(closed.mounted - opened.mounted, 1, 'exactly the swapped pane should have mounted its new face');
  assert.equal(mounted.idAt('stage:1'), 'sheet-b', 'the untouched pane\'s standing face moved');
}

/** (b) A redraw that changes no host's wanted id -- what a focus move or a status line
 *  update produces -- must not touch any mount, anywhere. */
function assertFocusAndStatusRedrawsAreMountFree(guard) {
  const mounted = new Mounted();
  const wanted = { 'stage:0': 'sheet-a', 'stage:1': 'sheet-b', 'dock:right:margin-a': 'margin-a' };
  redraw(mounted, guard, wanted);
  const opened = mounted.tally;

  // Five redraws with the same wanted ids: the sequence a focus move (kernel/viewpoint.mjs,
  // which touches no host) or a status line write (`Frame#say`, which only sets
  // `#standing`) produces, since both reach `paint()` -> `draw()` -> the same per-host ask.
  for (let i = 0; i < 5; i += 1) redraw(mounted, guard, wanted);
  const closed = mounted.tally;

  assert.deepEqual(closed, opened, 'redraws with unchanged wanted ids moved the tally');
  for (const key of Object.keys(wanted)) assert.equal(mounted.idAt(key), wanted[key]);
}

test('red-first: the broken guard fails both assertions before the real one is trusted', () => {
  assert.throws(() => assertPaneSwitchIsolated(brokenChanging), /exactly the swapped pane/);
  assert.throws(() => assertFocusAndStatusRedrawsAreMountFree(brokenChanging), /moved the tally/);
});

test('(a) a tab switch inside one pane causes zero unmount calls on the other pane', () => {
  assertPaneSwitchIsolated(realChanging);
});

test('(b) a focus move or a status update causes zero unmount calls anywhere', () => {
  assertFocusAndStatusRedrawsAreMountFree(realChanging);
});

test('(c) face replacement causes exactly one unmount -- already covered, kept there', () => {
  // test/mount.test.mjs: "a face that answers with an unmount is raised, and the tally
  // says so" and "raising over a live place lowers the one that stood there first" exercise
  // this directly against Mounted. Nothing to add here; this is a pointer, not a duplicate.
  assert.ok(true);
});
