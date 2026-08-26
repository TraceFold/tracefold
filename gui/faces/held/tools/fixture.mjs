// SPDX-License-Identifier: Apache-2.0
// The face, written out as a page a real browser can draw. Same discipline as
// faces/ledger's own fixture.mjs: the tree in the fixture is the same tree the unit
// tests read, serialised once, never a hand-written copy of it.
//
// Three pages: the happy path (a held list with a mix of what makes the seal-hole
// and clip-risk disclosures actually fire), an empty list, and a read that did not
// answer -- the state a face that fails open would draw as an empty list.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { face } from '../held.mjs';
import { parts } from '../binding.mjs';
import { ACTS } from '../declaration.mjs';
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

export const NARROW = { width: 720, height: 1400 };
export const WIDE = { width: 1280, height: 1400 };

const { toHtml } = parts.element;

/**
 * Rows chosen for what they make the page do: one candidate missing a member so its
 * note opens, one whose verdict is a word this sheet does not carry so the
 * undefined mark appears, one with a path too long for its column, and one item
 * that is not a record at all so the dropped count is not zero.
 */
const HELD = [
  {
    id: 'c-101', sequence: 6, at: '2026-08-24T10:02:00Z', actor: 'agent:packer',
    effect: 'write', verdict: 'Escalate', path: '/work/contract.pdf', digest: '91aa47f0e6b2115d',
  },
  {
    id: 'c-102', sequence: 7, at: '2026-08-24T10:05:00Z',
    effect: 'delete', verdict: 'Escalate', path: '/work/tmp/cache.bin', digest: '3c6b02d8ff41907e',
  },
  {
    id: 'c-103', sequence: 8, at: '2026-08-24T10:08:00Z', actor: 'agent:packer',
    effect: 'write', verdict: 'pending', path: '/work/notes/very/long/path/that/has/to/be/clipped/rather/than/wrapped.md',
    digest: 'a1cd0f9e64b23301',
  },
  'this item is not a record',
];

const walked = (items, { pages = 1, stopped = false } = {}) => ({
  outcome: 'answered', items, requests: pages, pages, stopped_at_budget: stopped, repeated_cursor: false, budget: 64,
});

/** The act log two entries deep, with the withheld one's reason taken from the
 * declaration rather than copied into this file. The copy that used to live here was
 * a second original of a sentence the face draws from the declaration -- edit one and
 * the photograph and the screen disagree, with nothing to catch it. */
const withheldReason = (act) => ACTS.find((spec) => spec.act === act).why;

const ACT_LOG = [
  { act: 'commit', method: 'commit', id: 'c-100', outcome: 'answered', detail: 'sent, status 202' },
  { act: 'escalate', method: 'escalation', id: 'c-101', outcome: 'withheld', detail: withheldReason('escalate') },
];

export const STATES = Object.freeze({
  'held': {
    held: walked(HELD, { pages: 2, stopped: true }),
    acts: ACT_LOG,
  },
  // The same read with a candidate chosen. Nothing else on this face is photographed
  // in its chosen state: the pane's own subject, the accent bed on the selected row,
  // and every gate that can only open once this window holds a candidate's identity
  // are all invisible on a page where nobody has clicked anything.
  'held-chosen': {
    held: walked(HELD, { pages: 2, stopped: true }),
    acts: ACT_LOG,
    selected: 'c-101',
  },
  // A right-click on the first candidate, and the menu it opens: the four declared
  // acts answered by the four gates for that candidate, and the value the pointer
  // was over. Photographed because nothing else on this face draws a menu, and
  // because the two properties tools/gate.mjs reads off a whole screen -- one menu
  // at most, and an offer whose gate is shut naming no row to send against -- have
  // no population to be applied to on a page where nobody has right-clicked.
  'held-menu': {
    held: walked(HELD, { pages: 2, stopped: true }),
    acts: ACT_LOG,
    selected: 'c-101',
    menu: {
      at: 'row:c-101', subject: 'c-101', value: '/work/contract.pdf', copy: null,
    },
  },
  'held-empty': {
    held: walked([]),
    acts: [],
  },
  'held-unread': {
    held: {
      outcome: 'refused',
      status: 403,
      gx_code: 'UNAUTHORIZED',
      problem: { type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not read the candidates', gx_code: 'UNAUTHORIZED' },
    },
    acts: [],
  },
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

/** The two pages that carry rows are photographed at the wide viewport as well: the
 * ladder, the band and the pane all lay out differently once there is room for the
 * pane to sit beside the list, and a narrow-only capture would never show it. */
const WIDE_TOO = new Set(['held', 'held-chosen', 'held-menu']);

export function writeFixtures() {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  const written = [];
  for (const [name, state] of Object.entries(STATES)) {
    const file = path.join(FIXTURE_DIR, `${name}.html`);
    fs.writeFileSync(file, pageHtml(name, state), 'utf8');
    written.push({ name, path: file, viewports: WIDE_TOO.has(name) ? ['narrow', 'wide', 'dark'] : ['narrow', 'dark'] });
  }
  return written;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  for (const written of writeFixtures()) process.stdout.write(`${written.path}\n`);
}
