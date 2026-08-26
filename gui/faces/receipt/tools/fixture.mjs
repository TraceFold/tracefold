// SPDX-License-Identifier: Apache-2.0
// The face, written out as a page a real browser can draw. Same discipline as
// faces/ledger's and faces/held's own fixture.mjs: the tree in the fixture is the
// same tree the unit tests read, serialised once, never a hand-written copy of it.
//
// Three pages: the happy path (delta and receipt both answered, digests agree,
// address and bytes both present), a read that did not answer (the state a face
// that fails open would draw as an empty receipt), and a genuine digest mismatch
// (the one negative-truth reading specific to this face -- req/03 §5's "confirm
// without the issuer" claim has to be seen actually failing at least once, the
// same discipline faces/held's seal-hole negative control holds).

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { face } from '../receipt.mjs';
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

export const NARROW = { width: 720, height: 1400 };
export const WIDE = { width: 1280, height: 1400 };

const { toHtml } = parts.element;

const DIGEST = 'a1b2c3d4e5f60001';

const DELTA = {
  id: 't-101', sequence: 12, prev: 't-100', at: '2026-08-24T10:12:00Z', actor: 'agent:packer',
  effect: 'write', verdict: 'Admit', path: '/work/report-12.md', digest: DIGEST,
};

const RECEIPT = {
  digest: DIGEST, algorithm: 'sha256', anchor: 'https://example.test/anchor/t-101', basis: 'exact',
};

const answered = (body) => ({ outcome: 'answered', status: 200, body });
const refused = (problem, status = 403) => ({
  outcome: 'refused', status, gx_code: problem.gx_code ?? null, problem,
});

export const STATES = Object.freeze({
  receipt: {
    id: 't-101',
    delta: answered(DELTA),
    receipt: answered(RECEIPT),
  },
  'receipt-unread': {
    id: 't-102',
    delta: refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not read this transformation', gx_code: 'UNAUTHORIZED' }),
    receipt: refused({ type: 'about:blank', title: 'forbidden', status: 403, detail: 'this token may not read this receipt', gx_code: 'UNAUTHORIZED' }),
  },
  'receipt-mismatch': {
    id: 't-103',
    delta: answered({ ...DELTA, id: 't-103', prev: 't-102', digest: DIGEST }),
    receipt: answered({ ...RECEIPT, digest: 'deadbeef00000001', anchor: undefined }),
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

export function writeFixtures() {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  const written = [];
  for (const [name, state] of Object.entries(STATES)) {
    const file = path.join(FIXTURE_DIR, `${name}.html`);
    fs.writeFileSync(file, pageHtml(name, state), 'utf8');
    written.push({ name, path: file, viewports: name === 'receipt' ? ['narrow', 'wide', 'dark'] : ['narrow', 'dark'] });
  }
  return written;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  for (const written of writeFixtures()) process.stdout.write(`${written.path}\n`);
}
