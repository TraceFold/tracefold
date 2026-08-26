// SPDX-License-Identifier: Apache-2.0
// The face, written out as a page a real browser can draw. Same discipline as
// faces/ledger's, faces/held's and faces/receipt's own fixture.mjs: the tree in the
// fixture is the same tree the unit tests read, serialised once, never a
// hand-written copy of it.
//
// Three pages: the happy path (several paths, some touched once, a genuine
// in-window chain, and an edge that leaves the window -- the one negative-truth
// reading specific to this face has to be seen actually happening at least once,
// the same discipline faces/receipt's digest-mismatch fixture holds), a read that
// did not answer, and a read that answered with zero items.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { face } from '../graph.mjs';
import { parts } from '../binding.mjs';
// Owner directive #335 (2)/(4): the one rule set a static page needs, from the one
// module that owns it -- never a second hand-written copy in this file.
import { SURFACE_CSS as surfaceCss } from '../../../parts/src/surface.mjs';
import { MEASURED_CLIP } from '../../../parts/src/receipt-row.mjs';
import { openWhereClipped } from '../../../parts/src/element.mjs';
import { QUESTION } from '../declaration.mjs';
import { tokenHref } from '../../../parts/tools/token-source.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const FIXTURE_DIR = path.join(HERE, '..', 'fixtures');
export const SHOT_DIR = path.join(FIXTURE_DIR, 'shots');

export const NARROW = { width: 720, height: 1600 };
export const WIDE = { width: 1280, height: 1600 };

const { toHtml } = parts.element;

/**
 * A digest the way the wire sends one: hexadecimal, and long enough to be cut.
 *
 * What stood here was `d001`, which is four characters and is not a digest of anything.
 * Nothing noticed while the shared scan line left the fingerprint column out -- the
 * value was read, normalised and drawn nowhere. It is drawn now (graph.mjs
 * GROUP_COLUMNS), and parts/src/serial.mjs correctly refuses to cut six characters out
 * of four, so a fixture carrying the old shape photographs an empty cell for a reason
 * that has nothing to do with this application. Deterministic, so the same fixture
 * produces the same page every time, and different in its leading characters per touch,
 * because a cut that is identical on every row is the echo this round removed.
 */
const digestFor = (sequence) => Array.from(
  { length: 16 },
  (_, i) => ((sequence * 11 + i * 7 + 3) % 16).toString(16),
).join('');

function item(id, sequence, pth, prev, extra = {}) {
  return {
    id, sequence, prev, path: pth, at: `2026-08-24T09:${String(sequence).padStart(2, '0')}:00Z`,
    actor: 'agent:packer', effect: sequence % 4 === 0 ? 'delete' : 'write', verdict: sequence % 4 === 0 ? 'Deny' : 'Admit',
    digest: digestFor(sequence),
    ...extra,
  };
}

const HAPPY_ITEMS = [
  item('t-101', 1, '/work/report.md', null),
  item('t-102', 2, '/work/notes.md', null),
  item('t-103', 3, '/work/report.md', 't-101'),
  item('t-104', 4, '/work/report.md', 't-103'),
  item('t-105', 5, '/work/contract.pdf', 't-900'),
  item('t-106', 6, '/work/contract.pdf', 't-105'),
];

const answered = (items) => ({ outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: 64 });
const refused = (problem, status = 403) => ({ outcome: 'refused', status, gx_code: problem.gx_code ?? null, problem });

export const STATES = Object.freeze({
  graph: { transformations: answered(HAPPY_ITEMS) },
  /**
   * The screen a hand has actually used (Owner #348 (2), and SS24k: a still of a screen
   * nobody has pressed is not evidence about a screen).
   *
   * One touch chosen, so the pane holds something -- which until this round no live
   * window could ever produce, because nothing wired a press to `state.selected` -- and
   * that same touch's menu open over its `effect` cell. Photographing it puts the menu
   * under every reading tools/shoot.mjs takes (overlaps, clipped cells, the 36px tap
   * budget, horizontal overflow) and under all of tools/gate.mjs's tree checks, rather
   * than leaving an interaction state proved only against a structural stand-in.
   */
  'graph-open': {
    transformations: answered(HAPPY_ITEMS),
    selected: 't-103',
    menu: { row: 't-103', cell: 'effect', item: null, outcome: null },
  },
  'graph-empty': { transformations: answered([]) },
  'graph-unread': { transformations: refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not list transformations', gx_code: 'UNAUTHORIZED' }) },
});

const measuredOpenSource = openWhereClipped.toString();
const measuredClipSpec = JSON.stringify(MEASURED_CLIP);

function pageHtml(name, state) {
  const sprite = toHtml(parts.sheet());
  const body = toHtml(face.view(state));
  const href = tokenHref(FIXTURE_DIR);
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${QUESTION} -- ${name}</title>
<link rel="stylesheet" href="${href}">
<style>
html,body{margin:0;padding:0}
body{background:var(--bg);color:var(--ink);font-family:var(--sans);font-size:var(--t-record);line-height:var(--lh-record)}
${surfaceCss}
</style>
</head>
<body>
${sprite}
<main>${body}</main>
<script>
/* The shipped function itself, serialised by Function.prototype.toString() in
   tools/fixture.mjs -- never a hand-written copy. It runs once now and once on
   load, because a web font that arrives late changes the very metric it reads,
   and it only ever opens a row, so running it twice is running it once. */
const openWhereClipped = ${measuredOpenSource};
const runIt = () => openWhereClipped(document, ${measuredClipSpec});
runIt();
window.addEventListener('load', runIt);
</script>
</body>
</html>
`;
}

export function writeFixtures() {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  const written = [];
  for (const [name, state] of Object.entries(STATES)) {
    const file = path.join(FIXTURE_DIR, `${name}.html`);
    fs.writeFileSync(file, pageHtml(name, state), 'utf8');
    written.push({ name, path: file, viewports: name.startsWith('graph-open') || name === 'graph' ? ['narrow', 'wide', 'dark'] : ['narrow', 'dark'] });
  }
  return written;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  for (const written of writeFixtures()) process.stdout.write(`${written.path}\n`);
}
