// M9 cross-cutting adversarial audit lane -- an independent re-attack on P4, in two series
// (sem: SEM-sdk-typescript-020):
//  (A) hand a tampered receipt / tampered public key to the WASM offline verification
//      (`verifyReceiptOffline`) and measure that it is refused (the implementing lane's own
//      `e2e.test.mjs` runs only the **positive** paths of both Admit and Deny, and not one
//      tampered input).
//  (B) whether the SDK swallows errors -- point GxClient at a minimal stub server (zero
//      dependencies, node:http only) that really returns broken HTTP responses (non-JSON body, 500,
//      empty body, dropped connection) and measure that the exception is not swallowed and comes
//      out in a distinguishable form.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import {
  GxClient,
  GxApiError,
  GxTransportError,
  verifyReceiptOffline,
} from "../dist/index.js";
import { scratchProject, keyGen, startServe, warmProject } from "../testlib/gx_process.mjs";

const hasBinary = Boolean(process.env.GX_BINARY);
const skip = hasBinary ? false : "GX_BINARY is not set (see testlib/gx_process.mjs::gxBinary)";

// -----------------------------------------------------------------------------------------------
// (A) tampering attacks on the WASM offline verification (sem: SEM-sdk-typescript-021)
// -----------------------------------------------------------------------------------------------

test(
  "audit(m9,p4): a tampered receipt (one flipped byte in the DSSE signature) is rejected, never accepted",
  { skip },
  async (t) => {
    const { project, home, target } = scratchProject("m9-p4-tamper-receipt");
    const { key_id, public_key } = await keyGen(project, home);
    await warmProject(project, home, target, key_id);
    const serving = await startServe(project, home, key_id);
    t.after(() => serving.stop());
    const client = new GxClient({ baseUrl: serving.baseUrl, token: serving.token });

    const created = await client.createCandidate({
      substrate: "fs",
      locator: target,
      goal: "audit-m9-p4-after\n",
      context: "Evidence",
      actor: { Human: { key: key_id } },
    });
    await client.verifyCandidate(created.id);
    await client.commitCandidate(created.id);
    const receipt = await client.getReceipt(created.id);
    // `e2e.test.mjs`'s own AC-P4-2 test passes a checkpoint (a CommitReceipt's `inclusion` check
    // needs one to resolve to "verified" rather than "unanchored", per `verify.ts`'s header) --
    // this audit reuses that same shape rather than a checkpoint-less call, so a real attack signal
    // is not confounded with "this call was never going to verify regardless of tampering".
    const checkpoint = await client.ledgerCheckpoint();
    const checkpointJson = JSON.stringify(checkpoint);

    // Control: the untampered receipt verifies.
    const control = verifyReceiptOffline(JSON.stringify(receipt), key_id, public_key, checkpointJson);
    assert.equal(control.valid, true, `control must verify: ${JSON.stringify(control)}`);
    assert.equal(control.checks.inclusion, "verified", `control must be inclusion-verified: ${JSON.stringify(control)}`);

    // Attack 1: flip one byte inside the DSSE signature's base64 (a real signature-forgery model
    // -- not a JSON-structure attack, a cryptographic-payload attack).
    const tamperedSig = structuredClone(receipt);
    const sigs = tamperedSig.envelope.signatures;
    assert.ok(Array.isArray(sigs) && sigs.length > 0, "a DSSE envelope must carry a signature");
    const sigField = sigs[0].sig ?? sigs[0].signature;
    assert.ok(typeof sigField === "string" && sigField.length > 4, "the signature must be a non-trivial string");
    const flippedChar = sigField[4] === "A" ? "B" : "A";
    const flippedSig = sigField.slice(0, 4) + flippedChar + sigField.slice(5);
    if (sigs[0].sig !== undefined) sigs[0].sig = flippedSig;
    else sigs[0].signature = flippedSig;
    const r1 = verifyReceiptOffline(JSON.stringify(tamperedSig), key_id, public_key, checkpointJson);
    console.log(`AUDIT_M9_P4_TAMPER_SIGNATURE=${JSON.stringify(r1)}`);
    assert.equal(r1.valid, false, "a receipt with a flipped signature byte must never verify");

    // Attack 2: flip one byte inside the payload (the DSSE `pae`-protected body, so tampering it
    // must also fail even though the signature bytes themselves are untouched).
    const tamperedPayload = structuredClone(receipt);
    const payload = tamperedPayload.envelope.payload;
    assert.ok(typeof payload === "string" && payload.length > 4, "the payload must be a non-trivial base64 string");
    const flippedPayloadChar = payload[4] === "A" ? "B" : "A";
    tamperedPayload.envelope.payload = payload.slice(0, 4) + flippedPayloadChar + payload.slice(5);
    const r2 = verifyReceiptOffline(JSON.stringify(tamperedPayload), key_id, public_key, checkpointJson);
    console.log(`AUDIT_M9_P4_TAMPER_PAYLOAD=${JSON.stringify(r2)}`);
    assert.equal(r2.valid, false, "a receipt with a flipped payload byte must never verify");
  },
);

test(
  "audit(m9,p4): a tampered public key (flipped byte, and a wrong-but-valid key) is rejected, never accepted",
  { skip },
  async (t) => {
    const { project, home, target } = scratchProject("m9-p4-tamper-pubkey");
    const { key_id, public_key } = await keyGen(project, home);
    // A *second*, unrelated, structurally valid key pair -- the "wrong key entirely" attack, as
    // distinct from "the right key with one byte flipped".
    const { key_id: otherKeyId, public_key: otherPublicKey } = await keyGen(project, home);
    assert.notEqual(otherPublicKey, public_key, "the second key must actually differ from the first");

    await warmProject(project, home, target, key_id);
    const serving = await startServe(project, home, key_id);
    t.after(() => serving.stop());
    const client = new GxClient({ baseUrl: serving.baseUrl, token: serving.token });

    const created = await client.createCandidate({
      substrate: "fs",
      locator: target,
      goal: "audit-m9-p4-pubkey-after\n",
      context: "Evidence",
      actor: { Human: { key: key_id } },
    });
    await client.verifyCandidate(created.id);
    await client.commitCandidate(created.id);
    const receipt = await client.getReceipt(created.id);

    // Attack 1: one flipped byte of the base64 public key (a corrupted-key model).
    const flippedChar = public_key[4] === "A" ? "B" : "A";
    const flippedKey = public_key.slice(0, 4) + flippedChar + public_key.slice(5);
    const r1 = verifyReceiptOffline(JSON.stringify(receipt), key_id, flippedKey);
    console.log(`AUDIT_M9_P4_TAMPER_PUBKEY_FLIP=${JSON.stringify(r1)}`);
    assert.equal(r1.valid, false, "a flipped-byte public key must never verify a real receipt");

    // Attack 2: a wholly different, structurally valid public key (an attacker substituting their
    // own key and hoping the verifier trusts whatever key_id/key pair it is handed).
    const r2 = verifyReceiptOffline(JSON.stringify(receipt), otherKeyId, otherPublicKey);
    console.log(`AUDIT_M9_P4_TAMPER_PUBKEY_SUBSTITUTE=${JSON.stringify(r2)}`);
    assert.equal(r2.valid, false, "an unrelated (but structurally valid) public key must never verify");

    // Never throws for any of these -- the module header's own claim ("Never throws... every
    // failure is {valid:false...}"), measured rather than trusted.
  },
);

test("audit(m9,p4): malformed receipt JSON never throws out of verifyReceiptOffline", () => {
  const cases = [
    "not json at all",
    "{}",
    JSON.stringify({ envelope: {} }),
    JSON.stringify({ envelope: { payload: "not-base64!!!", signatures: [] } }),
    "",
  ];
  let threw = 0;
  for (const c of cases) {
    try {
      const r = verifyReceiptOffline(c, "some-key-id", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
      assert.equal(r.valid, false, `malformed input ${JSON.stringify(c)} must not verify`);
    } catch (e) {
      threw += 1;
      console.log(`AUDIT_M9_P4_UNEXPECTED_THROW case=${JSON.stringify(c)} error=${e}`);
    }
  }
  console.log(`AUDIT_M9_P4_MALFORMED_CASES=${cases.length} THREW=${threw}`);
  assert.equal(threw, 0, "the module header's 'never throws' claim must hold for malformed input too");
});

// 🔴 **E-SDK-8** (`req/38` §285, `req/503` §0) -- the denominator hole in the test directly above.
//
// Its five cases are all **strings**. "Never throws" was measured only for the shape TypeScript's
// types already guarantee, and TypeScript's types are gone at runtime. Measured before the repair
// (2026-08-22, `C:/work/sv_logs/attempt_7_esdk8_red.log`), against the published entry point:
//
//   {}                        -> RuntimeError: memory access out of bounds
//   123                       -> RuntimeError: memory access out of bounds
//   {envelope:{...}}          -> RuntimeError: memory access out of bounds   <- the natural mistake
//   null / undefined          -> TypeError: Cannot read properties of null (reading 'length')
//   []                        -> no throw, and worse: coerced to "" and answered
//                                "receipt: EOF while parsing a value" -- a confident wrong refusal
//
// The natural mistake is the third: `GxClient.getReceipt` returns an **object**, so a caller who
// forgets one `JSON.stringify` gets a WASM memory fault out of a function documented never to
// throw. And all four argument positions had it, not just the first (`keyId`, `publicKeyBase64`
// and `checkpointJson` were measured with the same result), which is why the guard covers all six.
test("E-SDK-8: a non-string argument is refused, not a WASM memory fault", () => {
  const anyReceipt = JSON.stringify({ envelope: { payload: "AAAA", signatures: [] }, issued_at: 0 });
  const aKey = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  // [name, args] -- one wrong argument per case, in each of the six positions, and the value kinds
  // the measurement above found distinct failure modes for.
  const cases = [
    ["receiptJson={}", [{}, "k", aKey]],
    ["receiptJson=parsed receipt", [{ envelope: { payload: "AAAA", signatures: [] } }, "k", aKey]],
    ["receiptJson=null", [null, "k", aKey]],
    ["receiptJson=undefined", [undefined, "k", aKey]],
    ["receiptJson=123", [123, "k", aKey]],
    ["receiptJson=[]", [[], "k", aKey]],
    ["keyId={}", [anyReceipt, {}, aKey]],
    ["publicKeyBase64={}", [anyReceipt, "k", {}]],
    ["checkpointJson={}", [anyReceipt, "k", aKey, {}]],
    ["checkpointKeyId={}", [anyReceipt, "k", aKey, undefined, {}]],
    ["checkpointPublicKeyBase64={}", [anyReceipt, "k", aKey, undefined, undefined, {}]],
  ];
  let threw = 0;
  for (const [name, args] of cases) {
    let r;
    try {
      r = verifyReceiptOffline(...args);
    } catch (e) {
      threw += 1;
      console.log(`ESDK8_UNEXPECTED_THROW case=${name} error=${e}`);
      continue;
    }
    console.log(`ESDK8 ${name} -> ${JSON.stringify(r)}`);
    assert.equal(r.valid, false, `${name} must not verify`);
    assert.equal(r.checks, null, `${name}: nothing ran, so nothing is reported (req/29 §4)`);
    assert.equal(r.anchor_authenticated, false, `${name}: the field is on every answer`);
    assert.match(
      r.error,
      /expected a string, received/,
      `${name}: the refusal must name the mistake, not describe malformed JSON that was never parsed`,
    );
  }
  console.log(`ESDK8_CASES=${cases.length} THREW=${threw}`);
  assert.equal(threw, 0, "no argument shape may leave this function by exception");
});

// The optional arguments' *legitimate* absent forms are not the mistake above, and must still work:
// `undefined` is what an omitted parameter is, and `null` is what `?? null` has always turned it
// into. A guard that refused those would break every caller written before this repair.
test("E-SDK-8: undefined and null still mean 'not given' for the optional arguments", () => {
  const bad = "not json at all";
  const aKey = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  for (const absent of [undefined, null]) {
    const r = verifyReceiptOffline(bad, "k", aKey, absent, absent, absent);
    console.log(`ESDK8_ABSENT ${String(absent)} -> ${JSON.stringify(r)}`);
    assert.equal(r.valid, false);
    // Reached the engine (its own vocabulary), rather than being turned back at the guard.
    assert.match(r.error, /^receipt:/);
  }
});

// -----------------------------------------------------------------------------------------------
// (B) the error-swallowing check -- run GxClient against a stub server that really returns broken
//     HTTP responses (sem: SEM-sdk-typescript-022)
// -----------------------------------------------------------------------------------------------

/** A minimal stub HTTP server (node:http only, zero deps) that answers every request with
 * whatever `responder` decides -- not a mock of gx-api's semantics, a fixture for malformed wire
 * behaviour a *real* server should never produce but a client must still survive without silently
 * losing the failure. */
function startStub(responder) {
  return new Promise((resolve) => {
    const server = createServer((req, res) => {
      responder(req, res);
    });
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({ baseUrl: `http://127.0.0.1:${port}`, stop: () => server.close() });
    });
  });
}

test("audit(m9,p4): a 500 with a non-JSON body surfaces as a distinguishable GxTransportError, not swallowed", async (t) => {
  const stub = await startStub((req, res) => {
    res.writeHead(500, { "content-type": "text/plain" });
    res.end("internal server misbehaviour, not JSON, not a ProblemDetail");
  });
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  await assert.rejects(
    () => client.healthz(),
    (err) => {
      assert.ok(err instanceof GxTransportError, `expected GxTransportError, got ${err?.constructor?.name}: ${err}`);
      assert.equal(err.status, 500);
      return true;
    },
  );
});

test("audit(m9,p4): a well-formed ProblemDetail body still surfaces as GxApiError with its gx_code intact", async (t) => {
  const stub = await startStub((req, res) => {
    res.writeHead(403, { "content-type": "application/json" });
    res.end(
      JSON.stringify({
        type: "https://example.test/x",
        title: "denied",
        status: 403,
        detail: "audit(m9,p4) stub refusal",
        gx_code: "NOT_ADMITTED",
      }),
    );
  });
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  await assert.rejects(
    () => client.healthz(),
    (err) => {
      assert.ok(err instanceof GxApiError, `expected GxApiError, got ${err?.constructor?.name}: ${err}`);
      assert.equal(err.code, "NOT_ADMITTED");
      return true;
    },
  );
});

test("audit(m9,p4): an empty 200 body does not silently coerce into a fabricated success value", async (t) => {
  const stub = await startStub((req, res) => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(""); // an empty body on a 2xx -- healthz's own real shape always carries a body;
    // this fixture models a server that answered 2xx without one.
  });
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  const result = await client.healthz();
  console.log(`AUDIT_M9_P4_EMPTY_200_BODY_RESULT=${JSON.stringify(result)}`);
  // `client.ts::request` returns `undefined` for a zero-length ok body (read from source: `text.length
  // === 0 ? undefined : JSON.parse(text)`) rather than throwing or fabricating a `{status:"ok"}` --
  // this is the finding, not a pass/fail in isolation: a caller that does `if (result.status ===
  // "ok")` without a defined-check would throw a *different*, unrelated TypeError right after this
  // point, which is a real ergonomic edge this audit records rather than treats as fail-open (no
  // data was invented; `undefined` is an honest "nothing came back").
  assert.equal(result, undefined, "an empty 2xx body must become `undefined`, not a fabricated object");
});

test("audit(m9,p4): a connection that is refused entirely (server not listening) throws rather than resolving falsily", async () => {
  // Port 1 on loopback: reserved, nothing listens there in this environment.
  const client = new GxClient({ baseUrl: "http://127.0.0.1:1", token: "irrelevant" });
  await assert.rejects(
    () => client.healthz(),
    (err) => {
      console.log(`AUDIT_M9_P4_CONN_REFUSED_ERROR_TYPE=${err?.constructor?.name} message=${err?.message}`);
      // `fetch`'s own network-level failure -- not a GxApiError/GxTransportError (those model an
      // HTTP *response*, and there is none here), but it must not resolve to `undefined`/`null`/a
      // falsy "looks like success" value, which is the actual fail-open shape worth ruling out.
      assert.ok(err instanceof Error, "a connection-refused failure must still be a real Error");
      return true;
    },
  );
});
