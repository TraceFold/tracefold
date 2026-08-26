// SPDX-License-Identifier: Apache-2.0
// req/99_SEMANTICS_REGISTRY.md's four-column tables, parsed once into one JSON file so
// req/08's main-surface semantics can consume the registry as data instead of hand-
// copying rows out of prose. This is a mirror, the same shape parts/tools/
// generate-tokens.mjs already uses for the token stylesheet: read the one file of
// record, stamp the bytes it read as a SHA-256, and let a separate drift check (this
// same gate, run without --generate) prove the mirror has not gone stale rather than
// asking anyone to trust that it was regenerated after the last edit.
//
//   node tools/sem_registry_generate.mjs           -> writes docs/sem_registry.json
//   node tools/sem_registry_gate.mjs                -> also reports FRESH/STALE/ABSENT for it
//
// Three states only, named rather than inferred from a diff a reader has to reconstruct:
//   FRESH   docs/sem_registry.json's source_sha256 matches req/99_SEMANTICS_REGISTRY.md today
//   STALE   the JSON exists but the registry has been edited since it was generated
//   ABSENT  the JSON has never been generated (or was deleted)

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import crypto from 'node:crypto';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
export const REGISTRY_AT = path.join(ROOT, 'req', '99_SEMANTICS_REGISTRY.md');
export const OUTPUT_AT = path.join(ROOT, 'docs', 'sem_registry.json');
const SOURCE_RELATIVE = 'req/99_SEMANTICS_REGISTRY.md';

/**
 * One row is `| \`path\` | role | defining req | consumed-by |`. Split on `|` and drop
 * the leading/trailing empty cells a well-formed markdown row produces either side of
 * its content; a row that does not split into exactly six pieces is malformed and is
 * refused rather than guessed at, because a parser that guesses at a bad row is a
 * parser that can silently drop or merge a real one.
 */
export function parseRows(text) {
  const rows = [];
  for (const line of text.split('\n')) {
    if (!/^\| `/.test(line)) continue;
    const cells = line.split('|');
    if (cells.length !== 6) throw new Error(`sem_registry_generate: malformed row (expected 4 cells, got ${cells.length - 2}): ${line}`);
    const [, pathCell, roleCell, reqCell, consumedCell] = cells.map((c) => c.trim());
    const cleanPath = pathCell.replace(/^`|`$/g, '');
    rows.push({
      path: cleanPath,
      role: roleCell,
      defining_req: reqCell,
      consumed_by: consumedCell,
    });
  }
  return rows;
}

export function digestOf(text) {
  return crypto.createHash('sha256').update(text).digest('hex');
}

export function buildRegistry(root = ROOT) {
  const text = fs.readFileSync(path.join(root, SOURCE_RELATIVE), 'utf8');
  const rows = parseRows(text);
  return {
    schema: 'glovrex_app.sem_registry/0.1',
    source: SOURCE_RELATIVE,
    source_sha256: digestOf(text),
    generated_at: new Date().toISOString(),
    row_count: rows.length,
    rows,
  };
}

/**
 * FRESH / STALE / ABSENT / SOURCE_ABSENT, computed from disk -- never from memory of
 * the last run.
 *
 * SOURCE_ABSENT is the published-tree case and is why it exists (req/883). The
 * generated JSON ships; the registry Markdown it is generated FROM does not, because
 * req/ is an internal corpus. Until this guard, that pairing threw an uncaught ENOENT
 * on the line below -- the reader of a fresh clone got a stack trace where a verdict
 * belonged. It is deliberately NOT folded into ABSENT: ABSENT means "nobody has
 * generated the JSON yet", which a reader fixes by running the generator, and telling
 * someone to run a generator whose input does not exist is worse than saying nothing.
 */
export function driftState(root = ROOT, outputAt = path.join(root, 'docs', 'sem_registry.json')) {
  if (!fs.existsSync(outputAt)) return { state: 'ABSENT', detail: `${path.relative(root, outputAt)} has not been generated` };
  const sourceAt = path.join(root, SOURCE_RELATIVE);
  if (!fs.existsSync(sourceAt)) {
    return {
      state: 'SOURCE_ABSENT',
      detail: `${SOURCE_RELATIVE} is not in this tree, so the JSON cannot be checked against it. Freshness is UNMEASURED -- not fresh, and not stale.`,
    };
  }
  const registryText = fs.readFileSync(sourceAt, 'utf8');
  const currentHash = digestOf(registryText);
  let written;
  try {
    written = JSON.parse(fs.readFileSync(outputAt, 'utf8'));
  } catch (err) {
    return { state: 'STALE', detail: `${path.relative(root, outputAt)} is not valid JSON: ${err.message}` };
  }
  if (written.source_sha256 !== currentHash) {
    return { state: 'STALE', detail: `source_sha256 in the JSON (${written.source_sha256}) does not match ${SOURCE_RELATIVE} today (${currentHash})` };
  }
  if (written.row_count !== parseRows(registryText).length) {
    return { state: 'STALE', detail: `row_count in the JSON (${written.row_count}) does not match a fresh parse (${parseRows(registryText).length})` };
  }
  return { state: 'FRESH', detail: `source_sha256 matches, ${written.row_count} rows` };
}

if (process.argv[1] === url.fileURLToPath(import.meta.url)) {
  const registry = buildRegistry(ROOT);
  fs.mkdirSync(path.dirname(OUTPUT_AT), { recursive: true });
  fs.writeFileSync(OUTPUT_AT, `${JSON.stringify(registry, null, 2)}\n`);
  console.log(`sem_registry_generate: wrote ${path.relative(ROOT, OUTPUT_AT)} (${registry.row_count} rows, source_sha256 ${registry.source_sha256.slice(0, 16)}...)`);
}
