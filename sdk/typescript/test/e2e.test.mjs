// AC-P4-2: "E2E = SDK -> a real `gx serve` -> submit -> verify -> commit -> fetch the receipt ->
// offline verification green (in the adopted form, WASM or CLI delegation). One Deny path as well
// (received as a gx_code error type)." (sem: SEM-sdk-typescript-023)
//
// A real `gx serve` process, spoken to with nothing but `GxClient` and `verifyReceiptOffline` --
// no mock server, no stub adapter. `GX_BINARY` must name a real `crates/gx-cli` build
// (`tools/verify_p4.sh` sets it); without it this file's tests are named SKIP rather than absent.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { GxClient, GxApiError, verifyReceiptOffline } from "../dist/index.js";
import { scratchProject, keyGen, startServe, warmProject } from "../testlib/gx_process.mjs";

const hasBinary = Boolean(process.env.GX_BINARY);
const skip = hasBinary ? false : "GX_BINARY is not set (see testlib/gx_process.mjs::gxBinary)";

test("AC-P4-2: submit -> verify -> commit -> receipt -> offline verify (Admit path)", { skip }, async (t) => {
  const { project, home, target } = scratchProject("p4-e2e-admit");
  const { key_id, public_key } = await keyGen(project, home);
  await warmProject(project, home, target, key_id);
  const serving = await startServe(project, home, key_id);
  t.after(() => serving.stop());

  const client = new GxClient({ baseUrl: serving.baseUrl, token: serving.token });

  const health = await client.healthz();
  assert.equal(health.status, "ok", `healthz: ${JSON.stringify(health)}`);

  const created = await client.createCandidate({
    substrate: "fs",
    locator: target,
    goal: "after\n",
    context: "Evidence",
    actor: { Human: { key: key_id } },
  });
  console.log(`CREATED=${JSON.stringify(created)}`);
  assert.equal(created.state, "Candidate");
  const id = created.id;
  assert.ok(id, "createCandidate returns an id");

  const verified = await client.verifyCandidate(id);
  console.log(`VERIFIED=${JSON.stringify(verified)}`);
  assert.equal(verified.verdict, "Admit", `expected Admit: ${JSON.stringify(verified)}`);
  assert.equal(verified.state, "Admitted");

  const receiptFromCommit = await client.commitCandidate(id);
  console.log(`COMMIT_RECEIPT=${JSON.stringify(receiptFromCommit)}`);
  assert.equal(receiptFromCommit.state, "Committed");
  assert.ok(receiptFromCommit.envelope, "44 §2.2: commit returns a Receipt (DSSE envelope)");

  assert.equal(
    readFileSync(target, "utf8"),
    "after\n",
    "43 T-10 applied the delta over the real substrate",
  );

  const receiptFromGet = await client.getReceipt(id);
  assert.equal(
    receiptFromGet.envelope.payload,
    receiptFromCommit.envelope.payload,
    "GET /receipts/{tid} agrees with the commit response",
  );

  const checkpoint = await client.ledgerCheckpoint();
  console.log(`CHECKPOINT=${JSON.stringify(checkpoint)}`);

  // req/177: field census **over the HTTP wire**, mirroring the Rust writer gates (req/38 §110:
  // `receipt_verdict_wire.rs`'s Receipt/DsseEnvelope censuses, `m2_types.rs`'s Checkpoint census).
  // The handlers return `serde_json::to_value` of those exact types, so the Rust censuses cover
  // this wire *transitively* -- what they cannot see is a future handler that wraps or decorates
  // the value on its way out. This is the one assert that lives on that boundary; the key lists
  // are the Rust censuses' own, and a divergence here with those tests green names the HTTP layer.
  // v0.4-s (DR-44-9, req/38 §168 ruling 1): a third key, `receipt_view` -- the signed payload
  // decoded on the server so a window needs no DAG-CBOR decoder (types.ts `ReceiptView`). The pair
  // is unmoved, which is the half that matters: `verifyReceiptOffline` below is handed this whole
  // body and still reads `envelope` + `issued_at` only.
  // L-02 (req/38 §369 item 1, req/603 §2): a fourth key, `server_health` -- {status,
  // status_reason}, always present. docs/LIMITS.md declared that a served receipt and a 500
  // /healthz can be true of one server at one instant and rested that on "a caller who wants to
  // know asks /healthz"; the falsifier it wrote in advance named the consumer that would break it,
  // and the consumer turned out to be *this* file's own client (req/566 G-2, req/578 §5 --
  // `getReceipt` calls `healthz()` nowhere). The wire now carries the answer in band. The pair is
  // still unmoved, which is the half that matters: `verifyReceiptOffline` below is handed this
  // whole body and still reads `envelope` + `issued_at` only.
  assert.deepEqual(
    Object.keys(receiptFromGet).sort(),
    ["envelope", "issued_at", "receipt_view", "server_health"],
    "GET /receipts/{tid} answers the Receipt pair plus DR-44-9's decoded view plus L-02's health",
  );
  assert.deepEqual(
    Object.keys(receiptFromGet.server_health).sort(),
    ["status", "status_reason"],
    "L-02 server_health: the two members /healthz answers with, and no verdict about the receipt",
  );
  assert.equal(
    receiptFromGet.server_health.status,
    "ok",
    "a fixture project's server says so in band",
  );
  assert.deepEqual(
    Object.keys(receiptFromGet.receipt_view).sort(),
    [
      "issued_at",
      "key_id",
      "leaf_index",
      "postcondition_fingerprint",
      "root",
      "subject",
      "tree_size",
    ],
    "types.ts ReceiptView: seven members, no `verified` (HTTP carries the proof and does not grade it) and no `alg` (33 NFR-011's closing note -- the algorithm is a property of the key)",
  );
  assert.equal(receiptFromGet.receipt_view.subject, id, "the view names its own subject");
  assert.ok(
    String(receiptFromGet.receipt_view.root).startsWith("gx1:"),
    "the root is a Cid in 42 §1.2's one readable spelling, so it compares with Checkpoint.root_hash by string equality",
  );
  assert.deepEqual(
    Object.keys(receiptFromGet.envelope).sort(),
    ["payload", "payload_type", "signatures"],
    "42 §3.10: the envelope is three fields on the wire too",
  );
  for (const sig of receiptFromGet.envelope.signatures) {
    assert.deepEqual(
      Object.keys(sig).sort(),
      ["keyid", "sig"],
      "33 NFR-011 note 5 (sem: SEM-sdk-typescript-024): {keyid, sig}, no alg rider",
    );
  }
  assert.deepEqual(
    Object.keys(checkpoint).sort(),
    ["origin", "root_hash", "signature", "timestamp", "tree_size"],
    "42 §3.11: a signed tree head is five fields on the wire too",
  );
  assert.deepEqual(Object.keys(checkpoint.signature).sort(), ["keyid", "sig"]);

  // The one field `commit_candidate`'s response adds beside 42 §3.10's own three envelope fields
  // (`transformation`/`state`/`enforced`/`at`) is not part of the signed payload, so stripping it
  // before verification is unnecessary -- `verifyReceiptOffline` reads only `envelope`+`issued_at`.
  const offline = verifyReceiptOffline(
    JSON.stringify(receiptFromGet),
    key_id,
    public_key,
    JSON.stringify(checkpoint),
  );
  console.log(`OFFLINE_VERIFY=${JSON.stringify(offline)}`);
  assert.equal(offline.valid, true, `offline verification must pass: ${JSON.stringify(offline)}`);
  assert.equal(offline.checks.inclusion, "verified", "a CommitReceipt against its own checkpoint");
  assert.equal(offline.checks.canonical_cid, true);

  // v0.4-l (req/189 M-06 + DR-44-1): three more field censuses over the wire, holding the SDK's
  // types (`ReplayOutcome`, `VerdictCheckpointPage`, `ConsistencyProof`) against the real answers
  // -- the three the audit found drifted (req/182 M-06) and the one DR-44-1 changed.
  const replayed = await client.replayTransformation(id);
  console.log(`REPLAY=${JSON.stringify(replayed)}`);
  assert.deepEqual(
    Object.keys(replayed).sort(),
    ["diffs", "dry_run", "matches", "records_replayed", "unchecked"],
    "44 §2.2 replay: the handler's five keys (types.ts ReplayOutcome)",
  );
  assert.equal(typeof replayed.records_replayed, "number");
  assert.equal(replayed.dry_run, false);
  assert.ok(Array.isArray(replayed.diffs));
  for (const diff of replayed.diffs) {
    assert.deepEqual(
      Object.keys(diff).sort(),
      ["component", "journal_ledger_seq", "ledger_index", "transformation"],
      "types.ts ReplayDiff: diffs entries are objects, not strings",
    );
  }

  const issued = await client.issueVerdictCheckpoint();
  const page = await client.listVerdictCheckpoints();
  console.log(`VERDICT_PAGE=${JSON.stringify(page)}`);
  assert.deepEqual(
    Object.keys(page).sort(),
    ["items", "next_cursor", "total"],
    "GET /verdict-checkpoints carries `total` (44 §2.2, not 44 §2.7's two-key page)",
  );
  assert.equal(typeof page.total, "number");
  assert.ok(page.next_cursor === null || typeof page.next_cursor === "number", "cursor is an int");
  assert.ok(page.items.some((c) => c.window_end === issued.window_end));
  // v0.4-s (DR-44-9 ruling 1 (4)): ten members -- the eight the signature covers plus the two the
  // API layer resolves against the journal, because `window_start`/`window_end` are verdict
  // sequence numbers and a reader took them for clock readings (req/187 §5).
  assert.deepEqual(
    Object.keys(issued).sort(),
    [
      "ledger_root_hash",
      "ledger_tree_size",
      "origin",
      "signature",
      "tally",
      "timestamp",
      "window_end",
      "window_end_at",
      "window_start",
      "window_start_at",
    ],
    "types.ts VerdictCheckpoint over the wire, with DR-44-9's two resolved boundaries",
  );
  for (const checkpoint of page.items) {
    const empty = checkpoint.window_start === checkpoint.window_end;
    assert.equal(
      checkpoint.window_start_at === null,
      empty,
      "an empty window names no verdict, so it resolves to null rather than to the nearest record",
    );
    assert.equal(checkpoint.window_end_at === null, empty);
  }

  // DR-44-1 = (a) bare: the second commit is the undo, so the tree grew 1 -> 2 and a consistency
  // proof exists between the two sizes. The answer is 42 §3.11's `ConsistencyProof` itself.
  const undone = await client.undoTransformation(id);
  console.log(`UNDO=${JSON.stringify(undone).slice(0, 200)}`);
  const consistency = await client.ledgerConsistency(1, 2);
  console.log(`CONSISTENCY=${JSON.stringify(consistency)}`);
  assert.deepEqual(
    Object.keys(consistency).sort(),
    ["checked_from", "checked_to", "consistent", "new_size", "old_size", "path"],
    "DR-44-1 (a) + DR-44-9: the bare ConsistencyProof, plus the judgement this endpoint is the single stated exception for (types.ts LedgerConsistencyAnswer)",
  );
  assert.equal(consistency.old_size, 1);
  assert.equal(consistency.new_size, 2);
  assert.equal(consistency.consistent, true);
  assert.equal(consistency.checked_from, consistency.old_size);
  assert.equal(consistency.checked_to, consistency.new_size);
});

test("v0.4-l M-06: the escalation answer's seven keys, over the wire (types.ts EscalationOutcome)", { skip }, async (t) => {
  const { project, home, target } = scratchProject("v04l-e2e-escalation");
  const { key_id } = await keyGen(project, home);
  const ruler = await keyGen(project, home);
  await warmProject(project, home, target, key_id);
  const serving = await startServe(project, home, key_id);
  t.after(() => serving.stop());
  const client = new GxClient({ baseUrl: serving.baseUrl, token: serving.token });

  // E-M3-4's escrow ceiling: an **inverse** over 1 MiB has no escrow, so the shipped pack
  // escalates (`crates/gx-api/tests/wire_census.rs`' own fixture, over the real server): the
  // target's *prior* contents are the inverse's payload, so the file is made large and the goal
  // stays small (a large goal would be refused at plan by the fs adapter's own 1 MiB payload cap,
  // M4H5-4(b) -- a different limit, measured on the first run of this test).
  writeFileSync(target, "x".repeat(1024 * 1024 + 4096));
  const created = await client.createCandidate({
    substrate: "fs",
    locator: target,
    goal: "after\n",
    context: "Evidence",
    actor: { Human: { key: key_id } },
  });
  const verified = await client.verifyCandidate(created.id);
  assert.equal(verified.verdict, "Escalate", `expected Escalate: ${JSON.stringify(verified).slice(0, 300)}`);
  const ruled = await client.escalateCandidate(created.id, {
    decision: "approve",
    reason: "v0.4-l E2E: the inverse is over the escrow ceiling and I accept that",
    actor: { Human: { key: ruler.key_id } },
  });
  console.log(`ESCALATION=${JSON.stringify(ruled)}`);
  assert.deepEqual(
    Object.keys(ruled).sort(),
    ["at", "decision", "reason", "ruled_by", "signed_by", "state", "transformation"],
    "types.ts EscalationOutcome names all seven (signed_by = INV-S6's visibility)",
  );
  assert.equal(ruled.signed_by, ruler.key_id, "the ruling is signed with the ruler's key, not the server's");
  assert.equal(ruled.state, "Admitted");
});

test("AC-P4-2: the Deny path, received as a named GxApiError", { skip }, async (t) => {
  const { project, home, target } = scratchProject("p4-e2e-deny");
  const { key_id } = await keyGen(project, home);
  await warmProject(project, home, target, key_id);
  const serving = await startServe(project, home, key_id);
  t.after(() => serving.stop());

  const client = new GxClient({ baseUrl: serving.baseUrl, token: serving.token });

  // The shipped fs policy pack's one forbid (`policies/fs/deny-etc.cedar`): any locator under
  // `/etc`. Real, read-only (the commit below is never reached, so nothing is written to it) --
  // `crates/gx-cli/tests/support/mod.rs::DENIED_LOCATOR`'s own choice, for the same reason.
  const deniedLocator = "/etc/hostname";

  const created = await client.createCandidate({
    substrate: "fs",
    locator: deniedLocator,
    goal: "gx-sdk-e2e-would-be-denied\n",
    context: "Evidence",
    actor: { Human: { key: key_id } },
  });
  const id = created.id;

  const verified = await client.verifyCandidate(id);
  console.log(`DENY_VERIFIED=${JSON.stringify(verified)}`);
  assert.equal(verified.verdict, "Deny", `expected Deny over ${deniedLocator}: ${JSON.stringify(verified)}`);
  assert.equal(verified.state, "Denied");

  await assert.rejects(
    () => client.commitCandidate(id),
    (err) => {
      assert.ok(err instanceof GxApiError, `expected a GxApiError, got ${err}`);
      assert.equal(err.code, "NOT_ADMITTED", `expected NOT_ADMITTED, got ${err.code}: ${err.detail}`);
      assert.equal(err.status, 403);
      return true;
    },
  );
});
