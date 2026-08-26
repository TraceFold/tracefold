// SPDX-License-Identifier: Apache-2.0
// One-off generator for ledger_flip_grandfather.json (SS548/SS558 task 1). Snapshots every
// currently-receiptless [●] row across all configured ledgers at the moment the flip-tool
// mechanism lands, so ledger_dash.mjs can label them "legacy" rather than "new" going forward.
// Not meant to be re-run casually: re-running it after new hand-written [●] rows land would
// silently grandfather them too, defeating the point (SS558: "do NOT mass-migrate").
import { buildReport } from './ledger_dash.mjs';
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const report = buildReport();
const rows = [];
for (const l of report.ledgers) {
  if (l.status !== 'OK') continue;
  for (const w of l.receiptWarnings) {
    rows.push({ ledgerId: l.id, lineNo: w.lineNo, rowId: w.rowId });
  }
}
const outPath = path.join(HERE, 'ledger_flip_grandfather.json');
writeFileSync(outPath, JSON.stringify(rows, null, 2) + '\n');
console.log(`wrote ${rows.length} grandfather rows to ${outPath}`);
