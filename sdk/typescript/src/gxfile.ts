// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * The `.gx` object file (`req/922` §1-§3, §7-§8; `crates/gx-witness/src/gxfile.rs`'s TypeScript
 * twin) -- `magic || format_version || kind || claimed identity || the DSSE-envelope document`,
 * read with the identity recomputed rather than believed, and written with the receipt's signed
 * bytes carried through unchanged.
 *
 * # This is an independent implementation, not a port
 *
 * The wire *shape* below (byte offsets, the twelve-entry kind registry, the refusal vocabulary) is
 * `req/922`'s format specification, which any conforming reader has to reproduce identically to
 * open the same files -- that is what "cross-implementation round trip" means. The *code* that
 * walks that shape is written fresh for this language: `gxfile.rs`'s `serde`-derived structs and
 * `Result`-returning functions do not translate line for line into TypeScript's `JSON.parse` and
 * discriminated unions, and nothing here was produced by transliterating that file.
 *
 * # The one thing this phase's Rust reader gets wrong, that this reader does not (M-4)
 *
 * `req/922_artifacts/Q1_e2e_audit.md` §4 (finding M-4) measured `gx object verify` reading an
 * entire hostile file -- ninety-six megabytes, in that run -- into memory before it looks at the
 * first two bytes: `std::fs::read(path)` precedes every check, so a stranger's file sets the peak
 * allocation. `readGxObjectFile` below is header-first by construction: it opens the file, reads
 * exactly {@link HEADER_LEN} bytes, and refuses on magic/version/kind *before* a second read ever
 * asks the filesystem for the body -- and that second read is itself bounded by
 * {@link DEFAULT_MAX_BODY_BYTES} (overridable), refused by name rather than attempted. `gx object
 * verify` is exactly the verb a third party points at a document somebody else produced (the
 * audit's own words), which is why the SDK closes this rather than reproducing it.
 */

import { blake3 } from "./blake3.js";

/** The two bytes every `.gx` file opens with (`gxfile.rs`'s `MAGIC`). */
export const MAGIC = new Uint8Array([0x67, 0x78]); // "gx"

/** The envelope-format version this reads and the only one it reads (`gxfile.rs`'s
 * `FORMAT_VERSION`). Independent of the receipt payload's own schema generation -- see that file's
 * header for why the two are not folded into one number. */
export const FORMAT_VERSION = 1;

/** `magic(2) + format_version(2) + kind(2) + claimed identity(32)`. */
export const HEADER_LEN = 38;

/** A file this build has never seen the like of, refused before its body is even requested from
 * disk (`readGxObjectFile`'s M-4 closing, above). 64 MiB is generous for a document whose real
 * examples run to a few kilobytes; it bounds the attacker's choice of peak allocation rather than
 * guessing a tight limit this format has not declared. */
export const DEFAULT_MAX_BODY_BYTES = 64 * 1024 * 1024;

/** `application/vnd.glovrex.receipt+dagcbor` -- `gx_witness::dsse::RECEIPT_PAYLOAD_TYPE`, restated
 * here rather than imported (there is no crate boundary to import across in TypeScript; the
 * constant is 42 §3.10's, not this file's invention). */
export const RECEIPT_PAYLOAD_TYPE = "application/vnd.glovrex.receipt+dagcbor";

/** 🔴 The kind registry (`req/922` F1; `gxfile.rs`'s `GxKind`). Twelve names, numbered 1-12 in
 * this exact order -- the order is the wire, so it is never resorted. `0` is reserved and names no
 * kind (a header of zeroed bytes is refused, not read as entry zero). */
export const GX_KIND_REGISTRY = [
  "Receipt",
  "Candidate",
  "Transformation",
  "LedgerProof",
  "ConsistencyProof",
  "Checkpoint",
  "VerdictCheckpoint",
  "EscalationTicket",
  "AttachSource",
  "EngineJournal",
  "LedgerLeaf",
  "EscrowedInverse",
] as const;

export type GxKind = (typeof GX_KIND_REGISTRY)[number];

/** The wire number a kind is written as (`registry index + 1`, matching `GxKind::code`). */
export function gxKindCode(kind: GxKind): number {
  return GX_KIND_REGISTRY.indexOf(kind) + 1;
}

/** The kind a wire number names, or `undefined` for a number no entry holds. */
export function gxKindFromCode(code: number): GxKind | undefined {
  return GX_KIND_REGISTRY[code - 1];
}

/** Whether **this build** has a codec for a kind -- `Receipt` alone, honestly, the same as
 * `GxKind::is_shipped`. Naming the other eleven is not shipping them. */
export function isShipped(kind: GxKind): boolean {
  return kind === "Receipt";
}

/** Why a `.gx` file (or a candidate for one) was not admitted -- `gxfile.rs`'s `Refusal`, one
 * variant per condition and in the same reading order the wrapper checks them, so a caller told
 * `reason` can act on it rather than parsing a sentence. */
export type GxRefusalReason =
  | { kind: "NotGxObjectFile"; detail: string }
  | { kind: "FormatVersion"; found: number }
  | { kind: "UnknownKind"; code: number }
  | { kind: "KindNotShipped"; gxKind: GxKind }
  | { kind: "Body"; detail: string }
  | { kind: "PayloadType"; expected: string; found: string }
  | { kind: "BodyNotCanonical"; detail: string }
  | { kind: "IdentityMismatch"; claimed: string; recomputed: string }
  | { kind: "TooLarge"; bodyBytes: number; maxBodyBytes: number };

function refusalMessage(reason: GxRefusalReason): string {
  switch (reason.kind) {
    case "NotGxObjectFile":
      return `not a gx object file: ${reason.detail}`;
    case "FormatVersion":
      return `the file declares object-format version ${reason.found} and this build reads ${FORMAT_VERSION}`;
    case "UnknownKind":
      return `kind ${reason.code} is in no entry of this build's registry of ${GX_KIND_REGISTRY.length}, so the file is refused rather than read as something it may not be`;
    case "KindNotShipped":
      return `kind ${reason.gxKind} is registered and this build has no codec for it; the only kind it reads and writes is Receipt`;
    case "Body":
      return `the wrapped document does not decode: ${reason.detail}`;
    case "PayloadType":
      return `the header names a kind whose payload type is ${reason.expected} and the envelope carries ${reason.found}`;
    case "BodyNotCanonical":
      return `the body is not canonical DAG-CBOR: ${reason.detail}`;
    case "IdentityMismatch":
      return `the file claims identity ${reason.claimed} and its body digests to ${reason.recomputed}; the stored value is a claim and the recomputed one decides`;
    case "TooLarge":
      return `the file's body is ${reason.bodyBytes} byte(s), past the ${reason.maxBodyBytes}-byte ceiling this reader declares rather than reading further to find out`;
  }
}

/** A `.gx` file (or write attempt) refused, carrying the structured {@link GxRefusalReason} a
 * caller can `switch` on -- `.message` is the same sentence `gxfile.rs`'s `Display` impl prints,
 * kept in step for a reader moving between the CLI and this SDK. */
export class GxRefusalError extends Error {
  readonly reason: GxRefusalReason;

  constructor(reason: GxRefusalReason) {
    super(refusalMessage(reason));
    this.name = "GxRefusalError";
    this.reason = reason;
  }
}

// ---------------------------------------------------------------------------
// Base64 (RFC 4648 §4, standard alphabet, padded -- `gx_core::b64`'s spelling) and the `gx1:`
// text form of a CID (RFC 4648 base32, lowercase, unpadded -- `gx_core::Cid::to_text`). Hand-
// written rather than pulled from an npm package for the same "typescript is the one dev
// dependency" reason `blake3.ts` states, and because both are a dozen lines apiece: reaching for a
// dependency here would trade a one-screen function for a supply-chain edge to audit.
// ---------------------------------------------------------------------------

function base64Decode(text: string): Uint8Array {
  const bin = atob(text);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

const CID_TEXT_PREFIX = "gx1:";
/** RFC 4648 base32, lowercase, matching `gx_core::CID_ALPHABET` exactly. */
const CID_ALPHABET = "abcdefghijklmnopqrstuvwxyz234567";

/** `gx1:<base32>` over exactly 32 bytes -- the same bit-shifting `Cid::to_text` does, ported as an
 * algorithm (RFC 4648 base32) rather than as a translation of that function's Rust. */
function cidToText(bytes: Uint8Array): string {
  let out = CID_TEXT_PREFIX;
  let acc = 0;
  let bits = 0;
  for (const byte of bytes) {
    acc = (acc << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      out += CID_ALPHABET[(acc >>> bits) & 0x1f];
    }
  }
  if (bits > 0) {
    out += CID_ALPHABET[(acc << (5 - bits)) & 0x1f];
  }
  return out;
}

/** `BLAKE3(payloadBytes)`, the raw 32 bytes -- `gxfile::body_cid` minus the canonical-DAG-CBOR
 * audit `gx_canon::cbor::scan_strict` does, which this SDK does not carry (42 §2.1's decoder is
 * the engine's; a client that merely reads and re-carries `envelope.payload` never needs to parse
 * DAG-CBOR itself -- see {@link readGxObject}'s header for what that limit means for a body this
 * build did not itself mint). */
function bodyCid(payloadBytes: Uint8Array): Uint8Array {
  return blake3(payloadBytes);
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/** A `.gx` file that has been read, and whose identity claim has been checked against its body
 * (`gxfile.rs`'s `GxObjectFile`). Only `kind: "Receipt"` is ever returned -- every other registry
 * entry is a {@link GxRefusalError} before this type is constructed, per {@link isShipped}. */
export interface GxObjectFile {
  formatVersion: number;
  kind: "Receipt";
  /** The **recomputed** identity, `gx1:<base32>` -- never the header's claim, which is compared
   * against this and then dropped (`req/922` §7-3). */
  cid: string;
  /** The wrapped document's own bytes (`bytes[HEADER_LEN..]`), decoded as UTF-8 text. This is
   * **exactly** what {@link verifyReceiptOffline} takes as `receiptJson` -- the bridge `req/922`
   * §10 asks for is this field, not a re-serialization of it. */
  receiptJson: string;
  /** The same document, parsed once so a caller does not have to `JSON.parse` it a second time.
   * Only `envelope.payload_type`, `envelope.payload` and `envelope.signatures` are read by this
   * module; everything else on this value is exactly what `JSON.parse(receiptJson)` produced. */
  receipt: {
    envelope: { payload_type: string; payload: string; signatures: unknown[] };
    [extra: string]: unknown;
  };
}

/** Read a `.gx` file already fully in memory, refusing anything this build cannot admit
 * (`gxfile::read`). For bytes that came from disk under a caller's control rather than from a
 * network response or a `<input type=file>` picker, prefer {@link readGxObjectFile}: this function
 * has already paid the cost of holding `bytes` before it is called, which is the cost M-4 is about
 * -- it cannot un-read what its caller already read.
 *
 * The order is the requirement, same as the Rust reader: magic, then version, then kind (and
 * whether this build ships it) -- all four **before** the body is parsed at all, so a file naming
 * an unshipped or unknown kind is refused without a decoder ever seeing its bytes.
 *
 * @throws {GxRefusalError} one reason per refused condition, in the order above.
 */
export function readGxObject(bytes: Uint8Array): GxObjectFile {
  if (bytes.length < HEADER_LEN) {
    throw new GxRefusalError({
      kind: "NotGxObjectFile",
      detail: `the file is ${bytes.length} byte(s) and a header is ${HEADER_LEN}`,
    });
  }
  if (bytes[0] !== MAGIC[0] || bytes[1] !== MAGIC[1]) {
    throw new GxRefusalError({
      kind: "NotGxObjectFile",
      detail: `the first two bytes are 0x${bytes[0]!.toString(16).padStart(2, "0")} 0x${bytes[1]!.toString(16).padStart(2, "0")} and a gx object file opens with "gx"`,
    });
  }
  const formatVersion = (bytes[2]! << 8) | bytes[3]!;
  if (formatVersion !== FORMAT_VERSION) {
    throw new GxRefusalError({ kind: "FormatVersion", found: formatVersion });
  }
  const code = (bytes[4]! << 8) | bytes[5]!;
  const gxKind = gxKindFromCode(code);
  if (gxKind === undefined) {
    throw new GxRefusalError({ kind: "UnknownKind", code });
  }
  if (!isShipped(gxKind)) {
    throw new GxRefusalError({ kind: "KindNotShipped", gxKind });
  }
  const claimed = bytes.subarray(6, HEADER_LEN);

  const bodyBytes = bytes.subarray(HEADER_LEN);
  const receiptJson = new TextDecoder("utf-8", { fatal: true }).decode(bodyBytes);
  let receipt: GxObjectFile["receipt"];
  try {
    receipt = JSON.parse(receiptJson) as GxObjectFile["receipt"];
  } catch (e) {
    throw new GxRefusalError({ kind: "Body", detail: (e as Error).message });
  }
  if (
    typeof receipt !== "object" ||
    receipt === null ||
    typeof receipt.envelope !== "object" ||
    receipt.envelope === null ||
    typeof receipt.envelope.payload_type !== "string" ||
    typeof receipt.envelope.payload !== "string"
  ) {
    throw new GxRefusalError({
      kind: "Body",
      detail: "missing field `envelope.payload_type` or `envelope.payload`",
    });
  }
  if (receipt.envelope.payload_type !== RECEIPT_PAYLOAD_TYPE) {
    throw new GxRefusalError({
      kind: "PayloadType",
      expected: RECEIPT_PAYLOAD_TYPE,
      found: receipt.envelope.payload_type,
    });
  }

  let payloadBytes: Uint8Array;
  try {
    payloadBytes = base64Decode(receipt.envelope.payload);
  } catch (e) {
    throw new GxRefusalError({
      kind: "BodyNotCanonical",
      detail: `the payload is not valid base64: ${(e as Error).message}`,
    });
  }
  const recomputed = bodyCid(payloadBytes);
  let mismatch = recomputed.length !== claimed.length;
  if (!mismatch) {
    for (let i = 0; i < recomputed.length; i++) {
      if (recomputed[i] !== claimed[i]) {
        mismatch = true;
        break;
      }
    }
  }
  if (mismatch) {
    throw new GxRefusalError({
      kind: "IdentityMismatch",
      claimed: cidToText(claimed),
      recomputed: cidToText(recomputed),
    });
  }

  return {
    formatVersion,
    kind: "Receipt",
    cid: cidToText(recomputed),
    receiptJson,
    receipt,
  };
}

/** Node-only (`node:fs/promises`): read a `.gx` file **header-first**, closing M-4 rather than
 * reproducing it -- see this module's header for the finding and the shape of the fix.
 *
 * Two reads, never one: the first is exactly {@link HEADER_LEN} bytes and decides magic, version,
 * kind and whether this build ships it, all before the second read is ever issued. The second is
 * bounded by `maxBodyBytes` (default {@link DEFAULT_MAX_BODY_BYTES}) -- a file whose declared size
 * exceeds it is refused **by size**, named, rather than read into memory to find out.
 *
 * @throws {GxRefusalError} the same reasons {@link readGxObject} throws, plus `"TooLarge"`.
 */
export async function readGxObjectFile(
  path: string,
  options?: { maxBodyBytes?: number },
): Promise<GxObjectFile> {
  const maxBodyBytes = options?.maxBodyBytes ?? DEFAULT_MAX_BODY_BYTES;
  const fs = await import("node:fs/promises");
  const handle = await fs.open(path, "r");
  try {
    const stat = await handle.stat();

    const header = new Uint8Array(HEADER_LEN);
    const { bytesRead } = await handle.read(header, 0, HEADER_LEN, 0);
    // A short header is `NotGxObjectFile`, not a truncated buffer read as though it were full --
    // `readGxObject` below draws the same line, so the padding is inert and the message is theirs.
    const headerSlice = header.subarray(0, bytesRead);
    if (headerSlice.length < HEADER_LEN) {
      throw new GxRefusalError({
        kind: "NotGxObjectFile",
        detail: `the file is ${stat.size} byte(s) and a header is ${HEADER_LEN}`,
      });
    }
    // Everything `readGxObject` checks about the header alone (magic, version, kind, shipped) is
    // re-derived here, over these 38 bytes, before the body is requested from disk at all.
    if (headerSlice[0] !== MAGIC[0] || headerSlice[1] !== MAGIC[1]) {
      throw new GxRefusalError({
        kind: "NotGxObjectFile",
        detail: `the first two bytes are 0x${headerSlice[0]!.toString(16).padStart(2, "0")} 0x${headerSlice[1]!.toString(16).padStart(2, "0")} and a gx object file opens with "gx"`,
      });
    }
    const formatVersion = (headerSlice[2]! << 8) | headerSlice[3]!;
    if (formatVersion !== FORMAT_VERSION) {
      throw new GxRefusalError({ kind: "FormatVersion", found: formatVersion });
    }
    const code = (headerSlice[4]! << 8) | headerSlice[5]!;
    const gxKind = gxKindFromCode(code);
    if (gxKind === undefined) {
      throw new GxRefusalError({ kind: "UnknownKind", code });
    }
    if (!isShipped(gxKind)) {
      throw new GxRefusalError({ kind: "KindNotShipped", gxKind });
    }

    const bodyBytes = stat.size - HEADER_LEN;
    if (bodyBytes > maxBodyBytes) {
      throw new GxRefusalError({ kind: "TooLarge", bodyBytes, maxBodyBytes });
    }

    // The header passed every check bytes could decide without the body; only now is the rest of
    // the file asked for, and only up to the size this call already knows it is.
    const whole = new Uint8Array(stat.size);
    whole.set(headerSlice, 0);
    if (bodyBytes > 0) {
      await handle.read(whole, HEADER_LEN, bodyBytes, HEADER_LEN);
    }
    return readGxObject(whole);
  } finally {
    await handle.close();
  }
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/** Write a `.gx` file from a receipt already serialized to JSON text -- exactly what
 * {@link GxClient.getReceipt} / {@link GxClient.commitCandidate} returns, `JSON.stringify`d (the
 * same convention {@link verifyReceiptOffline}'s `receiptJson` takes, for the same reason: a
 * caller who has the *raw* text has the exact bytes this workspace's JSON encoder wrote, where a
 * caller who parsed and re-`JSON.stringify`s a `Receipt` object risks exactly one thing --
 * `issued_at`'s nanosecond epoch is a `number` in this SDK's types (`req/132`'s "no `bigint` in
 * the public surface" choice) and JavaScript's `number` cannot hold every `i64` exactly past
 * `Number.MAX_SAFE_INTEGER`, which every real `issued_at` is past. This function never parses and
 * re-stringifies: the text handed in is embedded byte for byte, so no precision this SDK's number
 * type cannot carry is ever asked to survive one).
 *
 * Determinism (`req/922` §0 principle ②): calling this twice on the *same* `receiptJson` string
 * writes the same bytes, because nothing here re-encodes it -- the header is a pure function of
 * `envelope.payload`, and the body is the input, unchanged. Reading a `.gx` file with
 * {@link readGxObject} and writing `.receiptJson` straight back with this function reproduces the
 * original bytes exactly, the same identity `gxfile.rs`'s own round-trip test asserts
 * (`gxfile::write_receipt(gxfile::read(bytes).receipt) == bytes`).
 *
 * @throws {GxRefusalError} `"Body"` when `receiptJson` is not JSON, or does not carry
 *   `envelope.payload_type` / `envelope.payload`; `"PayloadType"` when the payload type is not
 *   {@link RECEIPT_PAYLOAD_TYPE}; `"BodyNotCanonical"` when `envelope.payload` is not valid base64
 *   (there is no DAG-CBOR audit here -- see {@link readGxObject}'s header for why this SDK does
 *   not carry that decoder).
 */
export function writeGxObject(receiptJson: string): Uint8Array {
  let receipt: { envelope?: { payload_type?: unknown; payload?: unknown } };
  try {
    receipt = JSON.parse(receiptJson) as typeof receipt;
  } catch (e) {
    throw new GxRefusalError({ kind: "Body", detail: (e as Error).message });
  }
  if (
    typeof receipt !== "object" ||
    receipt === null ||
    typeof receipt.envelope !== "object" ||
    receipt.envelope === null ||
    typeof receipt.envelope.payload_type !== "string" ||
    typeof receipt.envelope.payload !== "string"
  ) {
    throw new GxRefusalError({
      kind: "Body",
      detail: "missing field `envelope.payload_type` or `envelope.payload`",
    });
  }
  if (receipt.envelope.payload_type !== RECEIPT_PAYLOAD_TYPE) {
    throw new GxRefusalError({
      kind: "PayloadType",
      expected: RECEIPT_PAYLOAD_TYPE,
      found: receipt.envelope.payload_type,
    });
  }

  let payloadBytes: Uint8Array;
  try {
    payloadBytes = base64Decode(receipt.envelope.payload);
  } catch (e) {
    throw new GxRefusalError({
      kind: "BodyNotCanonical",
      detail: `the payload is not valid base64: ${(e as Error).message}`,
    });
  }
  const cid = bodyCid(payloadBytes);

  const body = new TextEncoder().encode(receiptJson);
  const out = new Uint8Array(HEADER_LEN + body.length);
  out.set(MAGIC, 0);
  out[2] = (FORMAT_VERSION >>> 8) & 0xff;
  out[3] = FORMAT_VERSION & 0xff;
  const code = gxKindCode("Receipt");
  out[4] = (code >>> 8) & 0xff;
  out[5] = code & 0xff;
  out.set(cid, 6);
  out.set(body, HEADER_LEN);
  return out;
}
