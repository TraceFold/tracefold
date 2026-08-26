// SPDX-License-Identifier: Apache-2.0
// The denominator of the whole harness.
//
// The retired instrument counted the readings it happened to contain, so nobody
// could say what a full count covered. Here the denominator is the acceptance
// criteria written in the requirement files: it grows when a requirement is written,
// not when a reading is written, and a criterion nothing stands behind is printed
// rather than absent.

export const LEDGER_MESSAGES = {
  SOURCE_ABSENT: 'the requirement file naming the acceptance criteria is not in the manifest',
  BUILT: 'acceptance ledger built',
};

// Bold cells in the acceptance tables: **AC-I12**, **W15**, **AC-M4**.
const CRITERION_IN_TABLE = /\*\*(AC-[A-Z]+\d+|W\d+)\*\*/g;

export function buildAcLedger(manifest, sources) {
  const criteria = new Map();
  const missing = [];
  for (const relative of sources) {
    const entry = manifest.at(relative);
    if (!entry) { missing.push(relative); continue; }
    for (const match of entry.text.matchAll(CRITERION_IN_TABLE)) {
      if (!criteria.has(match[1])) criteria.set(match[1], { id: match[1], from: relative });
    }
  }
  return { criteria: [...criteria.values()], missingSources: missing };
}

export function coverage(ledger, assays) {
  const stood = new Set();
  for (const assay of assays) for (const id of assay.backs ?? []) stood.add(id);
  const known = new Set(ledger.criteria.map((c) => c.id));
  const unbacked = ledger.criteria.filter((c) => !stood.has(c.id)).map((c) => c.id);
  // A reading claiming to stand behind a criterion that is not written anywhere is
  // its own kind of wrong, and it is not allowed to inflate the numerator.
  const claimsNothingWritten = [...stood].filter((id) => !known.has(id));
  return {
    total: ledger.criteria.length,
    backed: ledger.criteria.length - unbacked.length,
    unbacked,
    claimsNothingWritten,
  };
}
