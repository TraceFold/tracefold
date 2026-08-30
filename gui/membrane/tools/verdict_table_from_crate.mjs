// SPDX-License-Identifier: Apache-2.0
// Reads the verdict vocabulary out of the crate that owns it, so wire.mjs's
// VERDICT_KINDS is a measurement and not a hand-copied comment. req/972 §1-E #11
// found the gap this closes: route-table.json has route_table_from_crate.mjs
// (this file's model), but VERDICT_KINDS only carried a comment pointing at
// gx-core/src/verdict.rs:46-54,69-75 -- no drift test read the crate back.
//
//   node tools/verdict_table_from_crate.mjs   -> prints the extracted kinds
//
// No --write: unlike route-table.json, wire.mjs's VERDICT_KINDS is not generated
// data with a JSON half to rewrite. It is hand-authored source; membrane/test/
// verdict_table.test.mjs is what compares it against this extraction.
//
// Scope, named precisely (req/972 §3-1's mixed-up-two-3-values mistake is the
// warning): this reads gx-core's VerdictKind enum only. wire.mjs's OUTCOME
// (answered/refused/failed/absent) is the membrane's own vocabulary for the
// shape of a reply, not a type read out of any crate -- there is nothing there
// for a crate extractor to check, and this file does not claim to check it.

import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '..', '..');

// Same resolution shape as route_table_from_crate.mjs (req/883), pointed at the
// crate that actually owns VerdictKind (gx-core, not gx-api). A separate env var
// because the two extractors read different crates and a shared name would let
// one override silently stand in for the other.
const CANDIDATES = [
  process.env.GX_VERDICT_RS ? resolve(process.env.GX_VERDICT_RS) : null,
  join(ROOT, '..', 'crates', 'gx-core', 'src', 'verdict.rs'),
  join(ROOT, '..', 'glovrex', 'crates', 'gx-core', 'src', 'verdict.rs'),
].filter(Boolean);

/** The first candidate that exists, or null when the crate is not visible from here. */
export function crateLibPath() {
  return CANDIDATES.find((p) => existsSync(p)) ?? null;
}

/** Why the crate could not be read, in words a reader can act on. */
export const CRATE_ABSENT_REASON =
  'the gx-core crate is not visible from this tree: set GX_VERDICT_RS to its '
  + 'src/verdict.rs, or check this tree out beside the engine tree. wire.mjs\'s '
  + 'VERDICT_KINDS is UNMEASURED in this run -- not verified, and not refuted.';

export function crateAvailable() {
  const at = crateLibPath();
  if (!at) return false;
  try {
    readFileSync(at, 'utf8');
    return true;
  } catch {
    return false;
  }
}

/**
 * Pull the `VerdictKind` enum's variant names, in declaration order, out of the
 * crate source text. Textual on purpose, same reason as route_table_from_crate.mjs:
 * no build required, cheap enough to run on every test pass. Doc comments on each
 * variant are stripped before the identifier is read, so prose never becomes a kind.
 */
export function extractVerdictKinds(source) {
  const enumMatch = /pub enum VerdictKind\s*\{([\s\S]*?)\n\}/.exec(source);
  if (!enumMatch) throw new Error('VerdictKind enum not found in crate source');
  const kinds = [];
  for (const rawLine of enumMatch[1].split('\n')) {
    const line = rawLine.trim();
    if (line === '' || line.startsWith('//') || line.startsWith('#[')) continue;
    const m = /^([A-Z][A-Za-z0-9_]*)\s*,?\s*$/.exec(line);
    if (m) kinds.push(m[1]);
  }
  return kinds;
}

export function extractFromCrate() {
  const at = crateLibPath();
  if (!at) throw new Error(CRATE_ABSENT_REASON);
  return { verdict_kinds: extractVerdictKinds(readFileSync(at, 'utf8')) };
}

if (process.argv[1] && process.argv[1].endsWith('verdict_table_from_crate.mjs')) {
  if (!crateAvailable()) {
    console.error(`verdict_table_from_crate: UNMEASURED -- ${CRATE_ABSENT_REASON}`);
    process.exit(2);
  }
  console.log(JSON.stringify(extractFromCrate(), null, 2));
}
