// SPDX-License-Identifier: Apache-2.0
// Reads the routes out of the crate that serves them, so the count is a measurement
// and not a sentence in a header. req/01a §4-3 measured what a hand-kept list does:
// the reference tree's "12 of 22" had been rewritten three times and was wrong again.
//
//   node tools/route_table_from_crate.mjs            -> prints the extracted rows
//   node tools/route_table_from_crate.mjs --write    -> rewrites route-table.json's wire half
//
// Extracted here: base path, verb, path. Everything else on a row (effect, whether
// the crate caches an idempotency key, cursor flavour) is declared in route-table.json
// with its own source reference, and the two halves are held in bijection by the gate.

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

const HERE = dirname(fileURLToPath(import.meta.url));
export const TABLE_PATH = join(HERE, '..', 'route-table.json');

const ROOT = join(HERE, '..', '..');

// The crate that serves these routes lives in the engine's tree, not in this one, so
// where it sits is configuration and must never be a literal. Until req/883 this line
// carried an absolute path into one machine's checkout: a leak of that machine's layout
// when the tree is published, and a line that is simply wrong on every other machine
// including the operator's own after any move. Resolution order, first hit wins:
//
//   1. $GX_CRATE_LIB                       an explicit path, absolute or cwd-relative
//   2. <root>/../crates/…/lib.rs           this tree living inside the engine repo
//   3. <root>/../glovrex/crates/…/lib.rs   this tree checked out beside the engine tree
//
// Candidate 2 is why this gate is alive in a published clone rather than permanently
// dormant there: the GUI ships inside the engine's own repository, so the crate that
// serves these routes is a sibling directory and the bijection can actually be checked
// by anyone who clones it. That was measured, not assumed -- the published crate and
// this table agree (req/883).
//
// None of them present is NOT an error and NOT a pass. crateAvailable() answers false,
// and the route-table gate skips while saying which half it could not read -- a tree
// that cannot see the crate cannot measure the crate, and the one thing it must not do
// is report the bijection as held. Refusal, not silence.
const CANDIDATES = [
  process.env.GX_CRATE_LIB ? resolve(process.env.GX_CRATE_LIB) : null,
  join(ROOT, '..', 'crates', 'gx-api', 'src', 'lib.rs'),
  join(ROOT, '..', 'glovrex', 'crates', 'gx-api', 'src', 'lib.rs'),
].filter(Boolean);

/** The first candidate that exists, or null when the crate is not visible from here. */
export function crateLibPath() {
  return CANDIDATES.find((p) => existsSync(p)) ?? null;
}

/** Why the crate could not be read, in words a reader can act on. */
export const CRATE_ABSENT_REASON =
  'the gx-api crate is not visible from this tree: set GX_CRATE_LIB to its src/lib.rs, '
  + 'or check this tree out beside the engine tree. The wire half of route-table.json '
  + 'is UNMEASURED in this run -- not verified, and not refuted.';

/** The verbs axum's router builders spell, in the order a row may chain them. */
const VERBS = ['get', 'post', 'put', 'patch', 'delete', 'head', 'options'];

/**
 * Pull `{base_path, routes:[{verb, path}]}` out of the crate source text.
 * Deliberately textual: a parser that ran the crate would need the crate to build,
 * and this gate has to be cheap enough to run on every test pass.
 */
export function extractRoutes(source) {
  const base = /pub const BASE_PATH:\s*&str\s*=\s*"([^"]+)"/.exec(source);
  if (!base) throw new Error('BASE_PATH not found in crate source');

  const rows = [];
  const re = /\.route\(\s*"([^"]+)"\s*,\s*([\s\S]*?)\)\s*[,;]?\s*(?=\.|\n\s*\.|\n\s*axum|$)/g;
  // The tail of a `.route(...)` call is matched loosely, so re-scan each capture for
  // verb constructors rather than trusting the boundary.
  for (const m of source.matchAll(/\.route\(/g)) {
    const start = m.index + m[0].length;
    const { text, end } = balanced(source, start);
    if (end < 0) continue;
    const pathMatch = /^\s*"([^"]+)"\s*,/.exec(text);
    if (!pathMatch) continue;
    const rest = text.slice(pathMatch[0].length);
    for (const verb of VERBS) {
      const hit = new RegExp(`(^|[^A-Za-z_])${verb}\\s*\\(`, 'g');
      let found;
      while ((found = hit.exec(rest)) !== null) {
        rows.push({ verb: verb.toUpperCase(), path: pathMatch[1] });
      }
    }
  }
  void re;
  rows.sort((a, b) => (a.path === b.path ? a.verb.localeCompare(b.verb) : a.path.localeCompare(b.path)));
  return { base_path: base[1], routes: rows };
}

/** Text between `start` and the paren that closes the one just opened. */
function balanced(source, start) {
  let depth = 1;
  for (let i = start; i < source.length; i += 1) {
    const c = source[i];
    if (c === '(') depth += 1;
    else if (c === ')') {
      depth -= 1;
      if (depth === 0) return { text: source.slice(start, i), end: i };
    }
  }
  return { text: '', end: -1 };
}

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

export function extractFromCrate() {
  const at = crateLibPath();
  if (!at) throw new Error(CRATE_ABSENT_REASON);
  return extractRoutes(readFileSync(at, 'utf8'));
}

if (process.argv[1] && process.argv[1].endsWith('route_table_from_crate.mjs')) {
  if (!crateAvailable()) {
    console.error(`route_table_from_crate: UNMEASURED -- ${CRATE_ABSENT_REASON}`);
    process.exit(2);
  }
  const extracted = extractFromCrate();
  if (process.argv.includes('--write')) {
    const table = JSON.parse(readFileSync(TABLE_PATH, 'utf8'));
    table.base_path = extracted.base_path;
    writeFileSync(TABLE_PATH, `${JSON.stringify(table, null, 2)}\n`);
    console.log(`base_path=${extracted.base_path}, ${extracted.routes.length} routes extracted`);
  } else {
    console.log(JSON.stringify(extracted, null, 2));
  }
}
