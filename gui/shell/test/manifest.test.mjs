// SPDX-License-Identifier: Apache-2.0
// The declaration schema, its refusals, and the two gates that would have caught the
// failure this design was built against: a rail that grew past its stated size, and a
// declared field that nothing read.

import test from 'node:test';
import assert from 'node:assert/strict';

import { readManifest, placement, railFaces, FACE_FIELDS, DOCK_RULES, RAIL, REFUSAL, Refused, PLACES, PURPOSES } from '../kernel/manifest.mjs';
import { MANIFEST } from '../demo/manifest.gen.mjs';

const face = (over = {}) => ({ id: 'probe-x', title: 'Probe X', place: 'left', purpose: 'find', rail: true, consumes: [], ...over });
const of = (faces) => ({ faces });

test('the generated manifest is accepted, and says how many faces there are', () => {
  const read = readManifest(MANIFEST);
  assert.ok(read.faces.length >= 5, `only ${read.faces.length} faces were generated`);
  assert.equal(read.byId.size, read.faces.length);
  assert.ok(railFaces(read).length <= RAIL.capacity);
  for (const declared of read.faces) {
    assert.ok(PLACES.includes(declared.place));
    assert.ok(PURPOSES.includes(declared.purpose));
  }
});

test('a field nobody reads is refused, and every field names its reader', () => {
  for (const field of FACE_FIELDS) {
    assert.match(field.reader, /^kernel\/[a-z]+\.mjs$/, `${field.name} names no reader`);
  }
  assert.throws(() => readManifest(of([face({ colour: 'blue' })])), (error) => {
    assert.ok(error instanceof Refused);
    assert.equal(error.code, REFUSAL.UNKNOWN_FIELD);
    assert.match(error.message, /no part of the shell reads/);
    return true;
  });
});

test('a missing required field, a bad id and a bad place are all said at once', () => {
  assert.throws(() => readManifest(of([{ id: 'probe-x' }])), (error) => {
    const lines = error.message.split('\n').length;
    assert.ok(lines >= 4, `only ${lines - 1} faults were reported; a manifest with several mistakes takes several rounds to fix`);
    return true;
  });
  assert.throws(() => readManifest(of([face({ id: 'Probe X' })])), /not a face id/);
  assert.throws(() => readManifest(of([face({ place: 'ceiling' })])), /does not exist/);
  assert.throws(() => readManifest(of([face({ purpose: 'decoration' })])), /not one of/);
});

test('the rail refuses the face past its stated capacity, and says both numbers', () => {
  const many = Array.from({ length: RAIL.capacity + 1 }, (_, i) => face({ id: `probe-${i}`, place: 'stage', purpose: 'read' }));
  assert.throws(() => readManifest(of(many)), (error) => {
    assert.equal(error.code, REFUSAL.RAIL_OVER_CAPACITY);
    assert.match(error.message, new RegExp(`${RAIL.capacity} faces and ${RAIL.capacity + 1}`));
    return true;
  });
  const justEnough = many.slice(0, RAIL.capacity);
  assert.equal(readManifest(of(justEnough)).faces.length, RAIL.capacity);
});

test('a dock refuses the face past its capacity and the purpose it does not take', () => {
  const over = Array.from({ length: DOCK_RULES.left.capacity + 1 }, (_, i) => face({ id: `probe-${i}`, rail: false }));
  assert.throws(() => readManifest(of(over)), /left dock holds/);
  assert.throws(() => readManifest(of([face({ purpose: 'read' })])), /takes find/);
});

test('a placement the face did not declare is refused in words, both ways round', () => {
  const read = readManifest(MANIFEST);
  const onStage = read.faces.find((f) => f.place === 'stage');
  const onLeft = read.faces.find((f) => f.place === 'left');

  const wrong = placement(read, onStage.id, 'left');
  assert.equal(wrong.ok, false);
  assert.equal(wrong.code, REFUSAL.NOT_DECLARED_HERE);
  assert.match(wrong.said, /this is the left dock/);

  assert.equal(placement(read, onLeft.id, 'stage').ok, false);
  assert.equal(placement(read, onLeft.id, 'left').ok, true);
  assert.equal(placement(read, onStage.id, 'stage').ok, true);
  assert.equal(placement(read, 'nothing-by-that-name', 'stage').code, REFUSAL.NOT_A_FACE);
});

test('a face declared twice is refused rather than quietly deduplicated', () => {
  assert.throws(() => readManifest(of([face(), face()])), /declared twice/);
  assert.throws(() => readManifest({ faces: 'probe-a' }), /array of faces/);
});
