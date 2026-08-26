// SPDX-License-Identifier: Apache-2.0
// The face, written out as a page a real browser can draw. Same discipline as
// every other face's own fixture.mjs: the tree in the fixture is the same tree the
// unit tests read, serialised once, never a hand-written copy of it.
//
// Three pages: the happy path (several subjects, most closed, one forced open by a
// genuinely long path so the "closed by default, opens when it has to" property is
// seen actually happening at least once -- the same discipline faces/graph's own
// fixture holds for its edge-leaving-the-window case), a read that did not answer,
// and a read that answered with zero items.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { face } from '../atlas.mjs';
import { parts } from '../binding.mjs';
// Owner directive #335 (2)/(4): the one rule set a static page needs, from the one
// module that owns it -- never a second hand-written copy in this file.
import { SURFACE_CSS as surfaceCss } from '../../../parts/src/surface.mjs';
import { QUESTION } from '../declaration.mjs';
import { tokenHref } from '../../../parts/tools/token-source.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const FIXTURE_DIR = path.join(HERE, '..', 'fixtures');
export const SHOT_DIR = path.join(FIXTURE_DIR, 'shots');

export const NARROW = { width: 720, height: 1600 };
export const WIDE = { width: 1280, height: 1600 };

const { toHtml } = parts.element;

function item(id, sequence, pth, extra = {}) {
  return {
    id, sequence, path: pth, at: `2026-08-24T09:${String(sequence).padStart(2, '0')}:00Z`,
    actor: 'agent:packer', effect: sequence % 4 === 0 ? 'delete' : 'write', verdict: sequence % 4 === 0 ? 'Deny' : 'Admit',
    digest: `a1b2c3d4e5f6${String(sequence).padStart(4, '0')}`,
    ...extra,
  };
}

const HAPPY_ITEMS = [
  item('t-101', 1, '/work/report.md'),
  item('t-102', 2, '/work/report.md'),
  item('t-103', 3, '/work/report.md'),
  item('t-104', 4, '/work/notes.md'),
  item('t-105', 5, '/work/contract.pdf'),
  item('t-106', 6, '/work/contract.pdf'),
  // Forced-open case: a path long enough to overrun the summary column's own
  // 28-character budget (req/100 SS8) -- proves this fixture actually exercises
  // needsOpen()'s auto-open path, not only its closed-by-default one.
  item('t-107', 7, '/work/a-path-so-long-it-cannot-possibly-fit-in-the-fixed-summary-column-width.md'),
];

const answered = (items) => ({ outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: 64 });
const refused = (problem, status = 403) => ({ outcome: 'refused', status, gx_code: problem.gx_code ?? null, problem });

export const STATES = Object.freeze({
  atlas: { transformations: answered(HAPPY_ITEMS) },
  'atlas-empty': { transformations: answered([]) },
  'atlas-unread': { transformations: refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not list transformations', gx_code: 'UNAUTHORIZED' }) },
});

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
    written.push({ name, path: file, viewports: name === 'atlas' ? ['narrow', 'wide', 'dark'] : ['narrow', 'dark'] });
  }
  return written;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  for (const written of writeFixtures()) process.stdout.write(`${written.path}\n`);
}
