// SPDX-License-Identifier: Apache-2.0
// The baseline ledger: create, read, update, retire. Committing a baseline is an
// act that gets written down, because a picture that can be quietly replaced is a
// picture nothing can be measured against.
//
//   node tools/baseline.mjs list
//   node tools/baseline.mjs commit <face-id> --reason "..."
//   node tools/baseline.mjs retire <face-id> --reason "..."
//
// The ledger sits inside the manifest, so an entry moving here moves the tree digest
// of every run that follows.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { startRenderer } from './rig/renderer.mjs';
import { captureInk } from './tiers/pixel.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const LEDGER_AT = path.join(HERE, 'baselines', 'LEDGER.json');
const VIEWPORT = { width: 720, height: 320 };

export const BASELINE_MESSAGES = {
  COMMITTED: 'baseline committed',
  RETIRED: 'baseline retired',
  FACE_UNKNOWN: 'no face declares that id',
  REASON_REQUIRED: 'a baseline is not committed or retired without a recorded reason',
};

const readLedger = () => (fs.existsSync(LEDGER_AT)
  ? JSON.parse(fs.readFileSync(LEDGER_AT, 'utf8'))
  : {
    note: 'Committed ink for each face carrying a baseline. Every row records who moved it and why. The mask itself is not stored -- the digest is what a comparison needs, and a stored mask is a second copy of the truth that can drift from the first.',
    baselines: {},
    retired: {},
  });

const writeLedger = (ledger) => {
  fs.mkdirSync(path.dirname(LEDGER_AT), { recursive: true });
  fs.writeFileSync(LEDGER_AT, `${JSON.stringify(ledger, null, 2)}\n`);
};

const faces = () => JSON.parse(fs.readFileSync(path.join(HERE, 'faces.json'), 'utf8')).faces;

const [, , action, faceId] = process.argv;
const reasonAt = process.argv.indexOf('--reason');
const reason = reasonAt === -1 ? null : process.argv[reasonAt + 1];
const ledger = readLedger();

if (action === 'list') {
  const rows = Object.entries(ledger.baselines);
  console.log(`baselines: ${rows.length}`);
  for (const [id, row] of rows) console.log(`  ${id.padEnd(28)} ${row.digest}  ${row.width}x${row.height}  ink ${row.inkPixels}  ${row.updated}  ${row.reason}`);
  const retiredRows = Object.entries(ledger.retired ?? {});
  console.log(`retired: ${retiredRows.length}`);
  for (const [id, row] of retiredRows) console.log(`  ${id.padEnd(28)} ${row.retired}  ${row.reason}`);
} else if (action === 'commit') {
  const face = faces().find((f) => f.id === faceId);
  if (!face) { console.error(`${BASELINE_MESSAGES.FACE_UNKNOWN}: ${faceId}`); process.exit(1); }
  if (!reason) { console.error(BASELINE_MESSAGES.REASON_REQUIRED); process.exit(1); }
  const renderer = await startRenderer({ viewport: VIEWPORT });
  const page = await renderer.openPage();
  await page.open(url.pathToFileURL(path.join(HERE, face.source)).href);
  const ink = await captureInk(page);
  await page.close();
  await renderer.stop();
  const previous = ledger.baselines[faceId];
  ledger.baselines[faceId] = {
    digest: ink.digest,
    width: ink.width,
    height: ink.height,
    inkPixels: ink.inkPixels,
    viewport: `${VIEWPORT.width}x${VIEWPORT.height}`,
    renderer: renderer.product,
    updated: new Date().toISOString().slice(0, 10),
    reason,
    replaces: previous ? previous.digest : null,
  };
  writeLedger(ledger);
  console.log(`${BASELINE_MESSAGES.COMMITTED}: ${faceId} ${ink.digest} (${ink.inkPixels} ink px)${previous ? ` replacing ${previous.digest}` : ''}`);
} else if (action === 'retire') {
  if (!reason) { console.error(BASELINE_MESSAGES.REASON_REQUIRED); process.exit(1); }
  const previous = ledger.baselines[faceId];
  if (!previous) { console.error(`${BASELINE_MESSAGES.FACE_UNKNOWN}: ${faceId}`); process.exit(1); }
  delete ledger.baselines[faceId];
  ledger.retired = ledger.retired ?? {};
  ledger.retired[faceId] = { ...previous, retired: new Date().toISOString().slice(0, 10), reason };
  writeLedger(ledger);
  console.log(`${BASELINE_MESSAGES.RETIRED}: ${faceId}`);
} else {
  console.log('actions: list | commit <face-id> --reason "..." | retire <face-id> --reason "..."');
  process.exitCode = 1;
}
void ROOT;
