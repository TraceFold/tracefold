// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * Receipt offline verification (`req/132` §2 item 2) -- the WASM branch the spike measured.
 *
 * `req/133` §1 carries the full measurement; the summary this file's callers need is: the receipt
 * offline-verification path (`gx_witness::receipt::verify_offline`) never calls `getrandom` (only
 * `KeyPair::generate` does, and this SDK never generates a key), so `sdk/wasm-verify`'s
 * `wasm-bindgen` module -- built from that same path -- has **zero** entropy-related imports, and
 * this function calls it directly rather than shelling out to a CLI (the spike's other, undeployed
 * branch, kept documented in `req/132` §2 item 2 for the record).
 *
 * The generated glue (`./wasm-gen/wasm_verify.js` + `.wasm`) is a build artefact of
 * `sdk/wasm-verify` (`sdk/wasm-verify/build.sh`), not hand-written TypeScript -- see that crate's
 * header for what it is a projection of.
 */

import { verify_receipt_offline } from "./wasm-gen/wasm_verify.js";

/** The five words `gx_witness::receipt::InclusionCheck` and `gx-cli`'s own `INCLUSION_JSON`
 * (`crates/gx-cli/src/receipt.rs`) already use -- this SDK reuses them rather than inventing a
 * sixth spelling (see `sdk/wasm-verify/src/lib.rs`'s header, which makes the identical choice).
 *
 * `"unbridged"` is additive (v0.4, H-09): the anchor names a tree size the receipt's inclusion
 * proof does not, and no RFC 6962 consistency proof tied them together. It is **not** a pass and
 * **not** a refutation -- code that treated everything other than `"verified"`/`"not_applicable"`
 * as tampering was already wrong about `"unanchored"` and is wrong about this one for the same
 * reason. Read `valid` for the verdict; read this field for what to do next. */
export type InclusionCheck =
  | "not_applicable"
  | "verified"
  | "refuted"
  | "unanchored"
  | "unbridged";

export interface OfflineVerifyChecks {
  signature: boolean;
  canonical_cid: boolean;
  inclusion: InclusionCheck;
  key_id: string;
}

export interface OfflineVerifyResult {
  valid: boolean;
  checks: OfflineVerifyChecks | null;
  /** 🔴 **E-SDK-10** (`req/38` §285, `req/503`) -- whether anything checked the *checkpoint's own*
   * DSSE signature, which `gx_witness::receipt::verify_offline` deliberately does not
   * (45 ASM-45-1: the log's key may differ from the receipt's). Ported from `gx receipt verify`'s
   * field of the same name (**M6H8-11 adopted (a)**), and present on every answer for that field's
   * reason: one that appeared only when it was `true` is one a reader misses on exactly the runs
   * where it matters.
   *
   * `false` does **not** mean "no anchor" -- read `checks.inclusion` for that
   * (`unanchored`/`not_applicable`). `false` beside `inclusion: "verified"` means the arithmetic
   * held against a head nobody vouched for, which is the state a forger holding both files wants
   * you to read as a pass. Pass `checkpointKeyId` + `checkpointPublicKeyBase64` to change it. */
  anchor_authenticated: boolean;
  error: string | null;
}

/** Every argument the WASM boundary takes is a string, and TypeScript's types are gone at runtime
 * (**E-SDK-8**, `req/503` §0). A JS caller that hands `verifyReceiptOffline` a *parsed* receipt --
 * the natural mistake, since `GxClient.getReceipt` returns an object -- reached
 * `passStringToWasm0`, which walks WASM linear memory with whatever `.length` it found: measured
 * `RuntimeError: memory access out of bounds` for `{}`, `123` and a receipt-shaped object, and a
 * `TypeError` for `null`/`undefined`. An `Array` was worse than either: it coerced to `""` through
 * `join` and came back as a confident, wrong "malformed JSON" refusal.
 *
 * The doc comment below has promised "never throws" since P4, and it was true only of strings.
 * This is where that promise is made good, because it is the only place a JS value still exists as
 * a JS value -- nothing in Rust can inspect an argument that never became a `&str`. */
function notAString(name: string, value: unknown): string | null {
  if (typeof value === "string") return null;
  const received =
    value === null ? "null" : Array.isArray(value) ? "array" : typeof value;
  return `${name}: expected a string, received ${received}. The WASM boundary takes JSON text, not a parsed value -- JSON.stringify it first`;
}

/** The same, for the three arguments that may legitimately be absent. `undefined` and `null` both
 * mean "not given" (the published signature marks them optional, and `?? null` has always mapped
 * one onto the other); anything else that is not a string is the same mistake as above. */
function notAStringOrAbsent(name: string, value: unknown): string | null {
  if (value === undefined || value === null) return null;
  return notAString(name, value);
}

/**
 * Verify a `Receipt` with no ledger and no network (AC-018, AC-070), against a public key and
 * (for a `CommitReceipt`) an optional known `Checkpoint` -- `gx_witness::receipt::verify_offline`
 * projected through WASM.
 *
 * @param receiptJson  Exactly what {@link GxClient.getReceipt} / {@link GxClient.commitCandidate}
 *   returns, `JSON.stringify`d (or the raw response text -- both are the same bytes).
 * @param keyId        The signing key's id (44 §1.2's `gx key gen` output field `key_id`).
 * @param publicKeyBase64  The same command's `public_key` field, base64, ed25519, 32 bytes decoded.
 * @param checkpointJson  `GET /ledger/checkpoint`'s body, `JSON.stringify`d, or omitted for a
 *   `VerdictReceipt` (ASM-14: always `checks.inclusion === "not_applicable"`) or an unanchored check.
 * @param checkpointKeyId  🔴 **E-SDK-10** -- the `key_id` of the key the *checkpoint* was signed
 *   with. 45 ASM-45-1 allows it to differ from `keyId`, which is why it is a separate argument and
 *   not a reuse. Omit to take the anchor on trust; the answer says which happened.
 * @param checkpointPublicKeyBase64  That key's `public_key` field. Moves together with
 *   `checkpointKeyId`: one without the other is refused rather than half-checked.
 *
 * Never throws: every failure -- malformed JSON, a wrong-length key, a bad signature, a checkpoint
 * that fails its own check, **and an argument that is not a string at all** (E-SDK-8) -- is
 * `{valid: false, checks: null, anchor_authenticated: false, error: "<detail>"}` (see
 * `sdk/wasm-verify/src/lib.rs`'s header for
 * why: a thin projection surfaces the engine's own `Result`, and does not add a second control-flow
 * mechanism beside it).
 */
export function verifyReceiptOffline(
  receiptJson: string,
  keyId: string,
  publicKeyBase64: string,
  checkpointJson?: string,
  checkpointKeyId?: string,
  checkpointPublicKeyBase64?: string,
): OfflineVerifyResult {
  // 🔴 **E-SDK-8**. Checked in declaration order and reported one at a time, so a caller who got
  // two arguments wrong is told about the first rather than about a merged sentence naming
  // neither precisely.
  const refusal =
    notAString("receiptJson", receiptJson) ??
    notAString("keyId", keyId) ??
    notAString("publicKeyBase64", publicKeyBase64) ??
    notAStringOrAbsent("checkpointJson", checkpointJson) ??
    notAStringOrAbsent("checkpointKeyId", checkpointKeyId) ??
    notAStringOrAbsent("checkpointPublicKeyBase64", checkpointPublicKeyBase64);
  if (refusal !== null) {
    // The same shape the engine's own refusals take. A second shape for "you called this wrongly"
    // would make a caller parse two error models to find out that nothing was verified.
    return { valid: false, checks: null, anchor_authenticated: false, error: refusal };
  }
  const raw = verify_receipt_offline(
    receiptJson,
    keyId,
    publicKeyBase64,
    checkpointJson ?? null,
    checkpointKeyId ?? null,
    checkpointPublicKeyBase64 ?? null,
  );
  return JSON.parse(raw) as OfflineVerifyResult;
}
