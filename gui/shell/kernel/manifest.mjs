// SPDX-License-Identifier: Apache-2.0
// What a face is allowed to say about itself, and the only place the shell learns that a
// face exists.
//
// The shell never holds a list of faces. It holds this schema. A face that is not
// declared cannot appear, and a face that is declared appears without the shell being
// edited -- which is the same sentence read from both ends, and the reason the rail here
// cannot drift into a hand-written strip of names the way rails do.
//
// Every field below names the file that reads it. A field with no reader is a promise
// with no creditor: it survives review, it is quoted in comments, and it means nothing.
// tools/gates.mjs refuses a schema that declares one.

import { NAME_PATTERN, DOCK_SIDES } from './layout.mjs';

export const PLACES = Object.freeze([...DOCK_SIDES, 'stage']);

/** What a face may be for. The shell does not interpret these; it only counts them. */
export const PURPOSES = Object.freeze(['find', 'read']);

/** A face states its own condition. The shell draws the statement, never the meaning. */
export const CONDITIONS = Object.freeze(['plain', 'held']);

export const FACE_FIELDS = Object.freeze([
  { name: 'id', required: true, reader: 'kernel/manifest.mjs' },
  { name: 'title', required: true, reader: 'kernel/render.mjs' },
  { name: 'place', required: true, reader: 'kernel/manifest.mjs' },
  { name: 'purpose', required: true, reader: 'kernel/manifest.mjs' },
  { name: 'rail', required: true, reader: 'kernel/render.mjs' },
  { name: 'consumes', required: true, reader: 'kernel/mount.mjs' },
  { name: 'condition', required: false, reader: 'kernel/render.mjs' },
  { name: 'leastWidth', required: false, reader: 'kernel/render.mjs' },
  { name: 'leastHeight', required: false, reader: 'kernel/render.mjs' },
]);

const FIELD_NAMES = Object.freeze(FACE_FIELDS.map((f) => f.name));

/**
 * The rule a dock is under. `accepts` is the old workbench law -- "only the faces you
 * search with belong on the side" -- written as a constraint on a declared value rather
 * than as a habit, so it can be counted instead of remembered.
 */
export const DOCK_RULES = Object.freeze({
  left: Object.freeze({ capacity: 2, accepts: Object.freeze(['find']), least: 180, most: 520 }),
  right: Object.freeze({ capacity: 3, accepts: Object.freeze(['find', 'read']), least: 220, most: 640 }),
  bottom: Object.freeze({ capacity: 3, accepts: Object.freeze(['find', 'read']), least: 120, most: 480 }),
});

/** The strip has a stated ceiling, and a manifest that exceeds it is refused, not trimmed. */
export const RAIL = Object.freeze({ capacity: 8 });

export const REFUSAL = Object.freeze({
  UNKNOWN_FIELD: 'unknown-field',
  MISSING_FIELD: 'missing-field',
  BAD_ID: 'bad-id',
  BAD_PLACE: 'bad-place',
  BAD_PURPOSE: 'bad-purpose',
  BAD_CONDITION: 'bad-condition',
  DUPLICATE_ID: 'duplicate-id',
  RAIL_OVER_CAPACITY: 'rail-over-capacity',
  DOCK_OVER_CAPACITY: 'dock-over-capacity',
  DOCK_REFUSES_PURPOSE: 'dock-refuses-purpose',
  NOT_DECLARED_HERE: 'not-declared-here',
  NOT_A_FACE: 'not-a-face',
});

export class Refused extends Error {
  constructor(code, said) {
    super(said);
    this.name = 'Refused';
    this.code = code;
  }
}

function checkFace(entry, faults) {
  const say = (code, said) => faults.push({ code, said });
  for (const key of Object.keys(entry)) {
    if (!FIELD_NAMES.includes(key)) say(REFUSAL.UNKNOWN_FIELD, `a face declared "${key}", which no part of the shell reads`);
  }
  for (const field of FACE_FIELDS) {
    if (field.required && entry[field.name] === undefined) {
      say(REFUSAL.MISSING_FIELD, `a face did not declare "${field.name}"`);
    }
  }
  if (typeof entry.id !== 'string' || !NAME_PATTERN.test(entry.id ?? '')) {
    say(REFUSAL.BAD_ID, `"${entry.id}" is not a face id`);
  }
  if (!PLACES.includes(entry.place)) say(REFUSAL.BAD_PLACE, `"${entry.id}" asks for the place "${entry.place}", which does not exist`);
  if (!PURPOSES.includes(entry.purpose)) say(REFUSAL.BAD_PURPOSE, `"${entry.id}" states the purpose "${entry.purpose}", which is not one of ${PURPOSES.join(', ')}`);
  if (entry.condition !== undefined && !CONDITIONS.includes(entry.condition)) {
    say(REFUSAL.BAD_CONDITION, `"${entry.id}" states the condition "${entry.condition}", which is not one of ${CONDITIONS.join(', ')}`);
  }
  if (entry.place !== 'stage' && DOCK_RULES[entry.place] && !DOCK_RULES[entry.place].accepts.includes(entry.purpose)) {
    say(REFUSAL.DOCK_REFUSES_PURPOSE, `the ${entry.place} dock takes ${DOCK_RULES[entry.place].accepts.join(' or ')} faces, and "${entry.id}" states "${entry.purpose}"`);
  }
}

/**
 * @param {{faces: object[]}} manifest
 * @returns {{faces: object[], byId: Map<string, object>}} frozen
 * @throws {Refused} with every fault listed, because reporting the first one makes a
 *   person fix a manifest one round trip per mistake.
 */
export function readManifest(manifest) {
  const faults = [];
  const entries = manifest?.faces;
  if (!Array.isArray(entries)) throw new Refused(REFUSAL.NOT_A_FACE, 'a manifest declares an array of faces');

  const seen = new Set();
  for (const entry of entries) {
    checkFace(entry, faults);
    if (seen.has(entry.id)) faults.push({ code: REFUSAL.DUPLICATE_ID, said: `"${entry.id}" is declared twice` });
    seen.add(entry.id);
  }

  const onRail = entries.filter((e) => e.rail === true);
  if (onRail.length > RAIL.capacity) {
    faults.push({
      code: REFUSAL.RAIL_OVER_CAPACITY,
      said: `the rail holds ${RAIL.capacity} faces and ${onRail.length} were declared for it`,
    });
  }
  for (const side of DOCK_SIDES) {
    const here = entries.filter((e) => e.place === side);
    if (here.length > DOCK_RULES[side].capacity) {
      faults.push({
        code: REFUSAL.DOCK_OVER_CAPACITY,
        said: `the ${side} dock holds ${DOCK_RULES[side].capacity} faces and ${here.length} were declared for it`,
      });
    }
  }

  if (faults.length > 0) {
    throw new Refused(faults[0].code, `this manifest was refused:\n  ${faults.map((f) => `${f.code}: ${f.said}`).join('\n  ')}`);
  }

  const faces = Object.freeze(entries.map((e) => Object.freeze({ ...e })));
  return Object.freeze({ faces, byId: new Map(faces.map((f) => [f.id, f])) });
}

/**
 * May this face stand here? The shell answers in words either way, because a placement
 * that is silently ignored is indistinguishable from a placement that worked.
 */
export function placement(read, id, side) {
  const face = read.byId.get(id);
  if (!face) return { ok: false, code: REFUSAL.NOT_A_FACE, said: `no face named "${id}" is declared` };
  if (side === 'stage') {
    return face.place === 'stage'
      ? { ok: true }
      : { ok: false, code: REFUSAL.NOT_DECLARED_HERE, said: `"${id}" declares the ${face.place} dock, so it cannot stand on the stage` };
  }
  const rule = DOCK_RULES[side];
  if (!rule) return { ok: false, code: REFUSAL.BAD_PLACE, said: `there is no ${side} dock` };
  if (face.place !== side) {
    return { ok: false, code: REFUSAL.NOT_DECLARED_HERE, said: `"${id}" declares ${face.place}, and this is the ${side} dock` };
  }
  if (!rule.accepts.includes(face.purpose)) {
    return { ok: false, code: REFUSAL.DOCK_REFUSES_PURPOSE, said: `the ${side} dock takes ${rule.accepts.join(' or ')} faces and "${id}" states "${face.purpose}"` };
  }
  return { ok: true };
}

export const railFaces = (read) => read.faces.filter((f) => f.rail === true);
