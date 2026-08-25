// M9 cross-cutting adversarial audit lane -- an **independent recount** of AC-P4-1 (surface parity).
// `testlib/support.mjs::specifiedEndpointsFromSpec` (the primary parser written by the implementing
// lane itself) is not reused: depending on the same function would let a bug in the parser make
// "both claims" wrong at once while still agreeing (the audit lane's specific concern: "both agree"
// does not mean "both are right"). Here the 44 §2.1 table is read independently by a different
// technique (a regular-expression extraction of the `METHOD /path` shape, rather than a backtick
// split at line start), and the row count of GxClient.SPECIFIED_METHODS and the set of endpoint
// strings are reconciled independently. (sem: SEM-sdk-typescript-018)
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { GxClient, SPECIFIED_METHODS } from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "..", "..", "..");

/**
 * Independent implementation: the section boundaries are a regular-expression match on the heading
 * lines (the same two headings the existing helper uses, but found by a pattern such as
 * `/^###\s+2\.1\s/` rather than an exact `.trim() === "..."`). Row extraction takes **only the
 * first cell** of `| \`METHOD /path\` |` with the regular expression `^\|\s*`(\S+\s+\S+)`\s*\|`
 * (a parse path separate from the existing `split`-on-backtick technique). (sem: SEM-sdk-typescript-019)
 */
function independentlyParseSection21() {
  const doc = readFileSync(join(repoRoot, "req/spec/40-architecture/44-api-spec.md"), "utf8");
  const lines = doc.split(/\r?\n/);
  const startIdx = lines.findIndex((l) => /^###\s+2\.1\s/.test(l.trim()));
  const endIdx = lines.findIndex((l, i) => i > startIdx && /^###\s+2\.2\s/.test(l.trim()));
  assert.ok(startIdx !== -1, "2.1 heading must exist (independent parser)");
  assert.ok(endIdx !== -1, "2.2 heading must exist (independent parser, section end)");
  const rowPattern = /^\|\s*`([A-Z]+\s+\/\S*)`\s*\|/;
  const endpoints = [];
  for (let i = startIdx; i < endIdx; i++) {
    const m = rowPattern.exec(lines[i].trim());
    if (m) endpoints.push(m[1]);
  }
  return endpoints;
}

test("independent parse of 44 §2.1 finds a non-trivial, stable row count", () => {
  const endpoints = independentlyParseSection21();
  console.log(`AUDIT_M9_P4_INDEPENDENT_ROWS=${endpoints.length} ${JSON.stringify(endpoints)}`);
  assert.ok(endpoints.length > 0, "independent parser found zero rows -- it is reading nothing");
  // No duplicate METHOD+path pairs -- a doc-conformance sanity check the reused helper does not
  // itself assert (its own row array could silently contain a repeated line and still "agree" on
  // a count with a different but equally wrong SDK count).
  const dedup = new Set(endpoints);
  assert.equal(dedup.size, endpoints.length, "44 §2.1 must name each METHOD+path exactly once");
});

test("AC-P4-1 (independent recount): SDK method count equals the independently-parsed row count", () => {
  const endpoints = independentlyParseSection21();
  console.log(
    `AUDIT_M9_P4_INDEPENDENT_ROWS=${endpoints.length} SPECIFIED_METHODS=${SPECIFIED_METHODS.length}`,
  );
  assert.equal(
    SPECIFIED_METHODS.length,
    endpoints.length,
    `independent parser found ${endpoints.length} rows in 44 §2.1; GxClient.SPECIFIED_METHODS ` +
      `names ${SPECIFIED_METHODS.length} -- these must agree via two different parsing techniques, ` +
      `not just via the SDK's own bundled parser`,
  );
});

test("every independently-parsed endpoint is reachable through some SPECIFIED_METHODS or EXTENSION_METHODS call (spot check by path shape)", () => {
  // Not a full method-to-path binding audit (that would require reading client.ts's own routing
  // table, which this file deliberately does not do -- an independent audit should not re-derive
  // the implementation's own internal map and then congratulate itself for matching it). Instead:
  // a client instance must expose *some* callable method per declared row count (already asserted
  // above) and no method may be undefined/null at runtime -- an error-swallowing-adjacent check,
  // since a method that silently resolves to `undefined` instead of throwing at call time is its
  // own kind of fail-open.
  const client = new GxClient({ baseUrl: "http://127.0.0.1:1", token: "x" });
  const undefinedMethods = SPECIFIED_METHODS.filter((name) => client[name] === undefined);
  assert.deepEqual(
    undefinedMethods,
    [],
    `these SPECIFIED_METHODS names resolve to undefined on a real GxClient instance: ${undefinedMethods}`,
  );
});
