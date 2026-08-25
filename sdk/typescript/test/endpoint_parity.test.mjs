// AC-P4-1: "the count parsed from 44 §2.1's table = the SDK's client method count" (sem:
// SEM-sdk-typescript-025) — the doc-conformance shape
// `crates/gx-api/src/lib.rs`'s `SPECIFIED_ENDPOINTS` const-vs-router test takes one language over.
import { test } from "node:test";
import assert from "node:assert/strict";
import { specifiedEndpointsFromSpec } from "../testlib/support.mjs";
import { GxClient, SPECIFIED_METHODS, EXTENSION_METHODS } from "../dist/index.js";

test("44 §2.1's table has a countable denominator", () => {
  const rows = specifiedEndpointsFromSpec();
  console.log(`SPEC_44_2_1_ROWS=${rows.length} (${JSON.stringify(rows)})`);
  assert.ok(rows.length > 0, "the parser found no rows at all — it is reading the wrong section");
});

test("AC-P4-1: SDK client method count equals 44 §2.1's row count", () => {
  const rows = specifiedEndpointsFromSpec();
  console.log(
    `SPEC_ROWS=${rows.length} SPECIFIED_METHODS=${SPECIFIED_METHODS.length} ` +
      `EXTENSION_METHODS=${EXTENSION_METHODS.length}`,
  );
  assert.equal(
    SPECIFIED_METHODS.length,
    rows.length,
    `44 §2.1 names ${rows.length} endpoints and GxClient.SPECIFIED_METHODS names ` +
      `${SPECIFIED_METHODS.length}. Surface parity (AC-P4-1) is an equality, not an "at least".`,
  );
});

test("every name in SPECIFIED_METHODS is a real, callable GxClient method", () => {
  const client = new GxClient({ baseUrl: "http://127.0.0.1:1", token: "x" });
  const missing = SPECIFIED_METHODS.filter((name) => typeof client[name] !== "function");
  assert.deepEqual(missing, [], `these names are declared but not real methods: ${missing}`);
});

test("EXTENSION_METHODS are real too, and disjoint from SPECIFIED_METHODS", () => {
  const client = new GxClient({ baseUrl: "http://127.0.0.1:1", token: "x" });
  const missing = EXTENSION_METHODS.filter((name) => typeof client[name] !== "function");
  assert.deepEqual(missing, [], `these extension names are not real methods: ${missing}`);
  const overlap = SPECIFIED_METHODS.filter((n) => EXTENSION_METHODS.includes(n));
  assert.deepEqual(overlap, [], `a name cannot be both specified and an extension: ${overlap}`);
});
