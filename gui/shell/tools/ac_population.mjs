// SPDX-License-Identifier: Apache-2.0
// W13 -- "the AC's population is not the empty set of changed files" -- read literally:
// for each of the 15 AC rows in req/02_SHELL_WLAYER.md section 4, what set of things does
// the AC quantify over, how many are there right now on the static tree (no browser, no
// running shell, just what `git` would show if asked), and is that count above zero.
//
// A row whose population is always empty on a static tree cannot fail: a gate over
// nothing holds forever, which is indistinguishable from a gate that works. This module
// is the harness that makes each row's population a checked, printed number rather than
// an assumption at authoring time.
//
// Each row's population is read off a real export from the kernel/demo/tools tree that
// the corresponding AC (req/02 §4 and tools/gates.mjs) already quantifies over, not a
// number invented for this file. Where a row's AC has more than one clause, the
// population named here is the primary one the AC's count is over.

import { readManifest, railFaces, FACE_FIELDS } from '../kernel/manifest.mjs';
import { DOCK_SIDES } from '../kernel/layout.mjs';
import { VERBS } from '../kernel/acts.mjs';
import { KEYMAP } from '../kernel/keys.mjs';
import { NOT_STATE } from '../kernel/viewpoint.mjs';
import { SPACES } from '../kernel/shell.mjs';
import { kernelFiles, faceFiles, shippedFiles } from './gates.mjs';
import { MANIFEST } from '../demo/manifest.gen.mjs';
import { MODULES } from '../demo/modules.gen.mjs';

const declared = () => readManifest(MANIFEST);

/**
 * One row per AC in req/02_SHELL_WLAYER.md §4. `quantifies` is the population the AC's
 * machine basis (its "件数"/"母集団") is counted over; `population` returns that set.
 */
export const ROWS = Object.freeze([
  { ac: 'W1', quantifies: 'kernel source files, checked for a face id literal', population: () => kernelFiles() },
  { ac: 'W2', quantifies: 'face module files, checked for an import of the frame/membrane/port', population: () => faceFiles() },
  { ac: 'W3', quantifies: 'acts in the registry, checked for a declared invert', population: () => [...VERBS] },
  { ac: 'W4', quantifies: 'shipped files, checked for a second state-assignment site', population: () => shippedFiles() },
  { ac: 'W5', quantifies: 'spaces whose built state is round-tripped through serialise/parse', population: () => [...SPACES] },
  { ac: 'W6', quantifies: 'fields declared not-state (kernel/viewpoint.mjs NOT_STATE), each checked absent from the line', population: () => [...NOT_STATE] },
  { ac: 'W7', quantifies: 'dock sides, each carrying a declared capacity/accepts rule', population: () => [...DOCK_SIDES] },
  { ac: 'W8', quantifies: 'declared faces, each a place whose mount/unmount count is tracked', population: () => [...declared().faces] },
  { ac: 'W9', quantifies: 'keyboard chords, each checked against the act registry', population: () => [...KEYMAP] },
  { ac: 'W10', quantifies: 'shipped files, checked for eval / new Function', population: () => shippedFiles() },
  { ac: 'W11', quantifies: 'shipped files, checked for an SDK import or a bare network call', population: () => shippedFiles() },
  { ac: 'W12', quantifies: 'declared schema fields, whose required-ness is baselined', population: () => [...FACE_FIELDS] },
  { ac: 'W13', quantifies: 'the AC rows of req/02 §4 themselves', population: () => ROWS },
  { ac: 'W14', quantifies: 'declared faces with rail:true, whose count is checked against the rail ceiling', population: () => railFaces(declared()) },
  { ac: 'W15', quantifies: 'declared face modules, each required to be mounted and read, not merely parsed', population: () => [...MODULES.keys()] },
]);

/** @returns {{ac: string, quantifies: string, count: number}[]} */
export function populationCounts() {
  return ROWS.map((row) => ({ ac: row.ac, quantifies: row.quantifies, count: row.population().length }));
}

if (process.argv[1] && process.argv[1].endsWith('ac_population.mjs')) {
  const rows = populationCounts();
  for (const row of rows) process.stdout.write(`${row.ac.padEnd(4)} n=${String(row.count).padEnd(4)} ${row.quantifies}\n`);
  const empty = rows.filter((r) => r.count === 0);
  process.stdout.write(`\n${rows.length - empty.length}/${rows.length} AC rows have a non-empty static population\n`);
  process.exit(empty.length === 0 ? 0 : 1);
}
