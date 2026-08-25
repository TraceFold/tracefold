// Shared helpers for the SDK's own test suite (node:test, zero test-framework dependency --
// Fable ruling 2, req/132 §6: "test runner = node:test (zero-dependency policy)"; sem:
// SEM-sdk-typescript-030).
//
// Deliberately **outside** `test/`: Node's `--test` default discovery includes every file under a
// directory literally named `test`/`tests`, so a helper module living there would be "run" as its
// own zero-assertion test (harmless, but noise `endpoint_parity.test.mjs` et al. do not need).
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

/** `<repo root>` -- three levels up from `sdk/typescript/testlib/`. */
export function repoRoot() {
  return join(here, "..", "..", "..");
}

/** `sdk/typescript` -- one level up from `testlib/`. */
export function sdkRoot() {
  return join(here, "..");
}

export function readRepoFile(relPath) {
  return readFileSync(join(repoRoot(), relPath), "utf8");
}

/**
 * 44 §2.1's own table, parsed the way `crates/gx-cli/tests/exit_map.rs::spec_gx_codes` parses
 * 44 §2.3's: lines starting with `` | ` `` inside the section, read off the markdown rather than
 * hand-copied (doc-conformance, the I-11 shape).
 */
export function specifiedEndpointsFromSpec() {
  const doc = readRepoFile("req/spec/40-architecture/44-api-spec.md");
  const lines = doc.split(/\r?\n/);
  // The heading's title is Japanese and lives in the canon; the section is located by its number,
  // the same way `endIdx` below locates §2.2 (sem: SEM-sdk-typescript-031 records the exact title).
  const startIdx = lines.findIndex((l) => l.trim().startsWith("### 2.1 "));
  if (startIdx === -1) throw new Error("44 §2.1's heading was not found");
  const endIdx = lines.findIndex(
    (l, i) => i > startIdx && l.trim().startsWith("### 2.2 "),
  );
  if (endIdx === -1) throw new Error("44 §2.2's heading (the section's end) was not found");
  const section = lines.slice(startIdx, endIdx);
  const rows = section
    .map((l) => l.trim())
    .filter((l) => l.startsWith("| `") && l.split("|").length >= 5);
  return rows.map((l) => l.split("`")[1]);
}
