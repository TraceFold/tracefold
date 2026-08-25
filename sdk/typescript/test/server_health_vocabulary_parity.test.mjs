// 🔴 **L-02 / `req/38` §369 item 1 / R44-E** — the fourth additive key on `GET /receipts/{tid}`,
// `server_health`, as an equality between the engine and the SDK rather than as a memory.
//
// The defect this file exists for: `crates/gx-api/src/handlers.rs::server_health` mounts a
// `{ status, status_reason }` object beside every receipt (handlers.rs, `get_receipt`), and
// `sdk/typescript/src/types.ts` did not type it — a caller reading `receipt.server_health.status`
// fell off the `[extra: string]: unknown` index signature into `unknown`, which is the one shape a
// caller cannot switch on. The whole point of the key (a good document from a deployment that is
// currently in dispute vs. a good document from a well deployment) was invisible at the surface the
// key was added for.
//
// Nothing measured the pairing, which is why it could drift. This is the measurement: the status
// vocabulary and the object's keys are parsed out of the real handler and compared as sets against
// the SDK's `HealthStatus` union and `ServerHealth` interface, so the next word added on either side
// fails the build on the other. `inverse_status_vocabulary_parity.test.mjs` is the same move one
// field over.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readRepoFile } from "../testlib/support.mjs";

/** The four status words and the two keys the real `server_health` handler can write. */
function engineServerHealth() {
  const source = readRepoFile("crates/gx-api/src/handlers.rs");
  const start = source.indexOf("fn server_health(state: &AppState)");
  assert.notEqual(start, -1, "server_health moved — this parser is reading the wrong signature");
  // Bound the body at the next top-level fn (`get_receipt`), so the reason strings of a later
  // handler cannot leak into the match this reads.
  const end = source.indexOf("\npub async fn ", start);
  assert.notEqual(end, -1, "server_health has no following `pub async fn` to bound its body");
  const body = source.slice(start, end);

  // Ground truth for the vocabulary: the first element of each returned `(status, reason)` tuple.
  // A `"word"` immediately followed by `, Some(...)` or `, None` is a status arm; a reason string
  // (the long `format!` bodies) is never followed by `Some`/`None`.
  const statuses = [
    ...new Set([...body.matchAll(/"([a-z]+)"\s*,\s*(?:Some|None)\b/g)].map((m) => m[1])),
  ];

  // The object's own keys, off the `serde_json::json!({ ... })` at the end of the function.
  const jsonStart = body.indexOf("serde_json::json!({");
  assert.notEqual(jsonStart, -1, "server_health no longer builds its object with serde_json::json!");
  const keys = [
    ...new Set(
      [...body.slice(jsonStart).matchAll(/"([a-z_]+)"\s*:/g)].map((m) => m[1]),
    ),
  ];
  return { statuses, keys };
}

/** The `HealthStatus` union's bare-string members in `sdk/typescript/src/types.ts`. */
function sdkHealthStatus() {
  const source = readRepoFile("sdk/typescript/src/types.ts");
  const start = source.indexOf("export type HealthStatus =");
  assert.notEqual(start, -1, "the SDK has no `HealthStatus` union — `server_health.status` is untyped");
  const body = source.slice(start, source.indexOf(";", start));
  return [...body.matchAll(/"([a-z]+)"/g)].map((m) => m[1]);
}

/** The field names of the `ServerHealth` interface in `sdk/typescript/src/types.ts`. */
function sdkServerHealthKeys() {
  const source = readRepoFile("sdk/typescript/src/types.ts");
  const start = source.indexOf("export interface ServerHealth {");
  assert.notEqual(start, -1, "the SDK has no `ServerHealth` interface for the receipt's fourth key");
  const body = source.slice(start, source.indexOf("\n}", start));
  return [...body.matchAll(/^\s*([a-z_]+)\??\s*:/gm)].map((m) => m[1]);
}

/** Whether the `Receipt` interface carries an optional `server_health` field. */
function receiptExposesServerHealth() {
  const source = readRepoFile("sdk/typescript/src/types.ts");
  const start = source.indexOf("export interface Receipt {");
  assert.notEqual(start, -1, "the `Receipt` interface moved");
  const body = source.slice(start, source.indexOf("\n}", start));
  return /server_health\?\s*:\s*ServerHealth\b/.test(body);
}

test("the health words the engine can write are exactly the SDK's HealthStatus union", () => {
  const { statuses } = engineServerHealth();
  const engine = [...statuses].sort();
  const sdk = [...sdkHealthStatus()].sort();
  console.log(`SERVER_HEALTH_ENGINE=${engine.length} ${JSON.stringify(engine)}`);
  console.log(`SERVER_HEALTH_SDK=${sdk.length} ${JSON.stringify(sdk)}`);

  const missingFromSdk = engine.filter((w) => !sdk.includes(w));
  const missingFromEngine = sdk.filter((w) => !engine.includes(w));
  assert.deepEqual(
    missingFromSdk,
    [],
    `the engine writes these status words and the SDK's HealthStatus cannot name them: ${missingFromSdk}. ` +
      `A caller who switches on server_health.status would fall through every arm.`,
  );
  assert.deepEqual(
    missingFromEngine,
    [],
    `the SDK names status words no engine writes: ${missingFromEngine}. A caller would write a dead arm.`,
  );
  assert.equal(engine.length, sdk.length, "and the counts are equal, which the two filters imply");
});

test("server_health's own keys are exactly the ServerHealth interface's fields", () => {
  const { keys } = engineServerHealth();
  const engine = [...keys].sort();
  const sdk = [...sdkServerHealthKeys()].sort();
  console.log(`SERVER_HEALTH_KEYS_ENGINE=${JSON.stringify(engine)}`);
  console.log(`SERVER_HEALTH_KEYS_SDK=${JSON.stringify(sdk)}`);
  assert.deepEqual(
    sdk,
    engine,
    `the handler's object keys (${JSON.stringify(engine)}) and ServerHealth's fields ` +
      `(${JSON.stringify(sdk)}) are not the same set.`,
  );
});

test("Receipt exposes server_health as an optional ServerHealth (additive, non-breaking)", () => {
  assert.ok(
    receiptExposesServerHealth(),
    "the `Receipt` interface does not carry `server_health?: ServerHealth` — the fourth key on " +
      "GET /receipts/{tid} is still read as `unknown` off the index signature.",
  );
});
