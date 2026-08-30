// SPDX-License-Identifier: Apache-2.0
// The face, written out as a page a real browser can draw.
//
// The tree in the fixture is the same tree the unit tests read and the same tree
// `render` would mount, serialised once. That matters: the defects this face is built
// not to repeat were both invisible to a test reading a DOM built by a fake, so the
// thing photographed has to be the thing asserted about and not a hand-written copy of
// it.
//
// Two pages, because there are two states worth looking at and only one of them is the
// happy one. The second is a read that did not answer, which is the state a face that
// fails open draws as an empty ledger.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { face } from '../ledger.mjs';
import { parts } from '../binding.mjs';
// Owner directive #335 (2)/(4): the one rule set a static page needs, from the one
// module that owns it -- never a second hand-written copy in this file.
import { SURFACE_CSS as surfaceCss } from '../../../parts/src/surface.mjs';
import { MEASURED_CLIP } from '../../../parts/src/receipt-row.mjs';
import { openWhereClipped } from '../../../parts/src/element.mjs';
import { QUESTION } from '../declaration.mjs';
// Node-only: resolves a real path against a real disk. Not part of the runtime seam
// (binding.mjs) on purpose -- see the note there.
import { tokenHref } from '../../../parts/tools/token-source.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const FIXTURE_DIR = path.join(HERE, '..', 'fixtures');
export const SHOT_DIR = path.join(FIXTURE_DIR, 'shots');

export const NARROW = { width: 720, height: 1400 };
export const WIDE = { width: 1280, height: 1400 };

const { toHtml } = parts.element;

/**
 * Rows chosen for what they make the page do, not for looking well: one row missing a
 * member so a note opens under it, one child row so the spine column draws, one row
 * with a word where a verdict was expected so the undefined mark appears, and one item
 * that is not a record at all so the dropped count is not zero.
 */
const SETTLED = [
  {
    id: 't-001', sequence: 1, prev: null, at: '2026-08-24T09:01:00Z', actor: 'agent:packer',
    effect: 'write', verdict: 'Admit', path: '/work/report.md', digest: '4f10ab77c2d90013', basis: 'exact',
  },
  {
    id: 't-002', sequence: 2, prev: 't-001', at: '2026-08-24T09:04:00Z', actor: 'agent:packer',
    effect: 'delete', verdict: 'Deny', path: '/work/keys/private.pem', digest: 'b83e0c1547aa2260', basis: 'derived',
  },
  {
    id: 't-003', sequence: 3, prev: 't-002', at: '2026-08-24T09:07:00Z',
    effect: 'write', verdict: 'Escalate', path: '/work/invoice.csv', digest: 'c07d99e3b1f4408a',
  },
  // The next two digests begin with the same six characters on purpose. The serial is
  // the same on both rows and the records are not the same record, which is the thing
  // the sentence under every serial says and which is easier to believe when it is on
  // the screen than when it is only written down.
  {
    id: 't-004', sequence: 4, prev: 't-003', at: '2026-08-24T09:12:00Z', actor: 'human:owner',
    effect: 'undo', verdict: 'Admit', path: '/work/invoice.csv', digest: 'd2e5a80416cc7731',
    basis: 'exact', undo_of: 't-003',
  },
  {
    id: 't-005', sequence: 5, prev: 't-004', at: '2026-08-24T09:15:00Z', actor: 'process:sweeper',
    effect: 'write', verdict: 'approved', path: '/work/notes/very/long/path/that/has/to/be/clipped/rather/than/wrapped.md',
    digest: 'd2e5a89f30b1c604', basis: 'exact',
  },
  'this item is not a record',
];

const HELD = [
  {
    id: 'c-101', sequence: 6, at: '2026-08-24T10:02:00Z', actor: 'agent:packer',
    effect: 'write', verdict: 'Escalate', path: '/work/contract.pdf', digest: '91aa47f0e6b2115d',
  },
  {
    id: 'c-102', sequence: 7, at: '2026-08-24T10:05:00Z', actor: 'agent:packer',
    effect: 'delete', verdict: 'Escalate', path: '/work/tmp/cache.bin', digest: '3c6b02d8ff41907e',
  },
];

const walked = (items, { pages = 1, stopped = false } = {}) => ({
  outcome: 'answered',
  items,
  requests: pages,
  pages,
  stopped_at_budget: stopped,
  repeated_cursor: false,
  budget: 64,
});

/**
 * What these pages were drawn from, in their own words.
 *
 * The rows below are written here, in this file, and no engine has ever seen them. The
 * face cannot tell the difference -- an envelope that says it answered is an envelope
 * that answered -- so the page states it instead, and the strip at the foot of every
 * shot prints what the page said. A photograph of this face that claimed an engine on
 * the other end of it would be the one failure this whole application is against.
 */
const STAND_IN = 'a stand-in, not an engine';

export const STATES = Object.freeze({
  'ledger': {
    source: STAND_IN,
    settled: walked(SETTLED, { pages: 3, stopped: true }),
    held: walked(HELD),
    consistency: { outcome: 'answered', status: 200, body: { consistent: true, checked_from: 1, checked_to: 5 } },
    acts: [
      { act: 'commit', method: 'commit', id: 'c-100', outcome: 'answered', detail: 'sent, status 202' },
      { act: 'escalate', method: 'escalation', id: 'c-101', outcome: 'withheld', detail: 'the members of this request were never read out of the crate that serves it, so this window does not know what a correct one looks like and will not send a guess.' },
    ],
  },
  // The half that has not happened, on its own, so it can be looked at without
  // scrolling past the half that has. The one settled row here is the row whose engine
  // word is not one of the three, which is where the undefined mark is drawn.
  'ledger-held': {
    source: STAND_IN,
    settled: walked([SETTLED[4], SETTLED[5]]),
    held: walked(HELD),
    consistency: { outcome: 'answered', status: 200, body: { consistent: false, checked_from: 1, checked_to: 5 } },
    acts: [],
  },
  // The other button, open (Owner #348 (2)). A menu is state on this face rather than
  // an overlay a handler put on the page, which is exactly why it can be photographed
  // at all: this page is the same view() every other capture here is, handed a state
  // with a menu in it. What is drawn is the widest case -- a held row, which offers
  // three acts, one of them withheld -- with the pointer on the path cell, so the copy
  // item is in the menu too. The strip at the top is what the last copy did; a control
  // that looked the same whether or not it worked is the shape the shell's own copy
  // control already refuses.
  'ledger-menu': {
    source: STAND_IN,
    settled: walked([SETTLED[0], SETTLED[1]]),
    held: walked(HELD),
    consistency: { outcome: 'answered', status: 200, body: { consistent: true, checked_from: 1, checked_to: 2 } },
    acts: [],
    menu: { id: 'c-101', cell: 'path' },
    copied: { from: 'at', state: 'copied', why: null },
    // req/893: this page's job has changed. It used to be a picture of the row menu,
    // which was a second place a row's acts appeared. The rebuilt screen has one place,
    // so what is worth photographing here is a row that is open -- the widest case, a
    // held candidate, which offers three acts one of which is withheld. The menu state
    // is left in place rather than deleted: the rebuilt screen does not draw it yet, and
    // that is recorded as an open item in req/893 S6 rather than tidied away here.
    selected: 'c-101',
  },
  'ledger-unread': {
    settled: { outcome: 'failed', reason: 'transport', status: null, detail: 'the socket was closed before an answer arrived' },
    held: {
      outcome: 'refused',
      status: 403,
      gx_code: 'UNAUTHORIZED',
      problem: { type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not read the candidates', gx_code: 'UNAUTHORIZED' },
    },
    consistency: { outcome: 'absent', reason: 'no_such_route', requested: { name: 'consistency' } },
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

/**
 * Which widths each page is worth photographing at. Stated per page rather than worked
 * out from its name: the menu page is wanted wide as well as narrow, because the thing
 * it is a picture of is a control surface whose width is the question.
 */
const VIEWPORTS = Object.freeze({
  'ledger': ['narrow', 'wide', 'dark'],
  'ledger-menu': ['narrow', 'wide', 'dark'],
});
const DEFAULT_VIEWPORTS = Object.freeze(['narrow', 'dark']);

export function writeFixtures() {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  const written = [];
  for (const [name, state] of Object.entries(STATES)) {
    const file = path.join(FIXTURE_DIR, `${name}.html`);
    fs.writeFileSync(file, pageHtml(name, state), 'utf8');
    written.push({ name, path: file, viewports: VIEWPORTS[name] ?? DEFAULT_VIEWPORTS });
  }
  return written;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  for (const written of writeFixtures()) process.stdout.write(`${written.path}\n`);
}
