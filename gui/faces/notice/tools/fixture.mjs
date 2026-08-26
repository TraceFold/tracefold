// SPDX-License-Identifier: Apache-2.0
// The face, written out as pages a real browser can draw.
//
// The tree in each fixture is the same tree the unit tests read and the same tree
// `render` would mount, serialised once -- the defects this rebuild exists not to
// repeat were both invisible to a test reading a DOM built by a fake, so the thing
// photographed has to be the thing asserted about.
//
// Three pages: a representative window with one of every shape this face tells
// apart, a window that has not asked anything yet, and a window whose record runs
// past the drawn budget, so the truncation line is something a person can look at
// rather than only something a test counts. The entries are written out here rather
// than imported from a test helper, so a fixture this tool writes never depends on
// what a test file happens to hold today.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { face, DISPLAY_CAP } from '../notice.mjs';
import { parts } from '../binding.mjs';
// Owner directive #335 (2)/(4): the one rule set a static page needs, from the one
// module that owns it -- never a second hand-written copy in this file.
import { SURFACE_CSS as surfaceCss } from '../../../parts/src/surface.mjs';
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
 * One of every shape this face tells apart: a call still in flight, one answered,
 * one refused with the engine's own words, one that failed in transport, one that
 * named a route the table does not carry (the hole mark), one act the shell itself
 * refused, one the shell routed elsewhere, one carrying an outcome word this face
 * has never seen (the undefined mark), and two items that are not records at all.
 */
const REPRESENTATIVE = [
  { seq: 1, at: '2026-08-24T10:00:00Z', through: 'shell', method: 'get_transformations', outcome: 'asked' },
  {
    seq: 2, at: '2026-08-24T10:00:01Z', method: 'get_transformations', verb: 'GET', path: '/v1/transformations', outcome: 'answered', status: 200, result: { outcome: 'answered', status: 200, body: { items: [] } },
  },
  {
    seq: 3,
    at: '2026-08-24T10:00:02Z',
    method: 'post_candidates_id_commit',
    verb: 'POST',
    path: '/v1/candidates/{id}/commit',
    outcome: 'refused',
    status: 409,
    result: {
      outcome: 'refused',
      status: 409,
      gx_code: 'IDEMPOTENCY_CONFLICT',
      problem: {
        type: 'about:blank', title: 'conflict', status: 409, detail: 'this candidate was already committed', gx_code: 'IDEMPOTENCY_CONFLICT',
      },
    },
  },
  {
    seq: 4, at: '2026-08-24T10:00:03Z', method: 'get_candidates', verb: 'GET', path: '/v1/candidates', outcome: 'failed', status: null, result: { outcome: 'failed', reason: 'transport', status: null, detail: 'the socket was closed before an answer arrived' },
  },
  {
    seq: 5, at: '2026-08-24T10:00:04Z', method: 'get_everything_i_wish_for', verb: null, path: null, outcome: 'absent', status: null, result: { outcome: 'absent', reason: 'no_such_route', requested: { name: 'get_everything_i_wish_for' } },
  },
  {
    seq: 6, at: '2026-08-24T10:00:05Z', through: 'shell', method: 'pane:divide', outcome: 'refused', said: 'there is no act called "pane:divide" in this space',
  },
  {
    seq: 7, at: '2026-08-24T10:00:06Z', through: 'shell', method: 'theme:set', outcome: 'elsewhere', said: 'theme:set belongs to a different screen',
  },
  {
    seq: 8, at: '2026-08-24T10:00:07Z', method: 'get_transformations', verb: 'PATCH', path: '/v1/nowhere', outcome: 'partially_answered', status: 207,
  },
  'this entry is not a record',
  42,
];

function overflowing() {
  const out = [];
  for (let i = 0; i < DISPLAY_CAP + 12; i += 1) {
    out.push({ seq: i + 1, at: '2026-08-24T10:00:00Z', through: 'shell', method: `get_transformations_${i}`, outcome: 'asked' });
  }
  out.push({
    seq: DISPLAY_CAP + 13, at: '2026-08-24T10:05:00Z', method: 'get_candidates', verb: 'GET', path: '/v1/candidates', outcome: 'answered', status: 200, result: { outcome: 'answered', status: 200, body: { items: [] } },
  });
  return out;
}

export const STATES = Object.freeze({
  notice: REPRESENTATIVE,
  'notice-empty': [],
  'notice-overflow': overflowing(),
});

function pageHtml(name, notices) {
  const sprite = toHtml(parts.sheet());
  const state = face.read(notices);
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
</body>
</html>
`;
}

export function writeFixtures() {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  const written = [];
  for (const [name, notices] of Object.entries(STATES)) {
    const file = path.join(FIXTURE_DIR, `${name}.html`);
    fs.writeFileSync(file, pageHtml(name, notices), 'utf8');
    written.push({ name, path: file, viewports: name === 'notice' ? ['narrow', 'wide', 'dark'] : ['narrow', 'dark'] });
  }
  return written;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  for (const written of writeFixtures()) process.stdout.write(`${written.path}\n`);
}
