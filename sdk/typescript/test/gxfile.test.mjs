// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// `.gx` object file support (`req/922`; `sdk/typescript/src/gxfile.ts`, the TypeScript twin of
// `crates/gx-witness/src/gxfile.rs`) -- F3, the SDK-parity half of Q3.
//
// # Fixture provenance
//
// `test/fixtures/gxfile/frozen_commit.gx` and `frozen_commit.key.pub.json` are a real
// `crates/gx-cli` build's real output: `gx key gen` -> `gx submit` -> `gx plan` -> `gx verify` ->
// `gx commit` -> `gx object export`, run once (2026-08-30, `~/.sg/target/debug/gx`, this
// repository's own `ed0091a4`-family tree) and copied byte for byte, the same "copied read-only
// from a real run" provenance `crates/gx-witness/tests/fixtures/frozen_receipts/` states for its
// own specimens. **This file must not be edited or regenerated in place** -- if the CID this suite
// asserts (`gx1:wgjho4frzifyiyipy4hglzx6wbp34njoe2aj4hl6zh6umpwuuioq`) ever needs to change, mint a
// new fixture under a new name and freeze that one, so a stale assertion is a visible diff and not
// a silently moved goalpost (`req/38`'s frozen-corpus discipline, one language over).
//
// Its `cid` was independently confirmed against **the CLI's own printed answer** at capture time
// (`gx object export`'s JSON: `"cid":"gx1:wgjho4frzifyiyipy4hglzx6wbp34njoe2aj4hl6zh6umpwuuioq"`),
// which is what makes the assertions below a cross-implementation check and not this
// implementation agreeing with itself.
//
// # Live-binary tests (the AC-P4-2 convention)
//
// A handful of tests additionally *drive a real `gx` process* when `GX_BINARY` is set
// (`testlib/gx_process.mjs`'s existing convention, `tools/verify_p4.sh` sets it) -- these are named
// `SKIP` rather than absent when it is not, exactly as `test/e2e.test.mjs` already does. The frozen
// fixture above covers the same ground (real Rust bytes) without requiring a local Rust build, so
// this file's coverage does not shrink when `GX_BINARY` is unset; the live tests add a second,
// fresh round trip against whatever `gx` the machine actually has.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync, mkdtempSync, openSync, writeSync, closeSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

import {
  readGxObject,
  readGxObjectFile,
  writeGxObject,
  verifyReceiptOffline,
  GxRefusalError,
  GX_KIND_REGISTRY,
  GX_HEADER_LEN,
  GX_FORMAT_VERSION,
} from "../dist/index.js";

const HERE = dirname(fileURLToPath(import.meta.url));
const FIXTURE_DIR = join(HERE, "fixtures", "gxfile");
const FIXTURE_GX = join(FIXTURE_DIR, "frozen_commit.gx");
const FIXTURE_KEY = join(FIXTURE_DIR, "frozen_commit.key.pub.json");
const FIXTURE_CID = "gx1:wgjho4frzifyiyipy4hglzx6wbp34njoe2aj4hl6zh6umpwuuioq";

const fixtureBytes = readFileSync(FIXTURE_GX);
const fixtureKey = JSON.parse(readFileSync(FIXTURE_KEY, "utf8"));

// ---------------------------------------------------------------------------
// (a) Cross-implementation round trip, direction 1: a real Rust-exported file, read by the SDK.
// ---------------------------------------------------------------------------

test("readGxObject reads a real gx-cli export and recomputes the CID the CLI itself printed", () => {
  const file = readGxObject(fixtureBytes);
  assert.equal(file.formatVersion, GX_FORMAT_VERSION);
  assert.equal(file.kind, "Receipt");
  assert.equal(file.cid, FIXTURE_CID, "the recomputed identity matches gx-cli's own printed cid");
  const receipt = JSON.parse(file.receiptJson);
  assert.equal(receipt.envelope.payload_type, "application/vnd.glovrex.receipt+dagcbor");
});

test("readGxObject's receiptJson bridges directly into verifyReceiptOffline", () => {
  const file = readGxObject(fixtureBytes);
  const result = verifyReceiptOffline(file.receiptJson, fixtureKey.key_id, fixtureKey.public_key);
  assert.equal(result.checks?.signature, true, JSON.stringify(result));
  assert.equal(result.checks?.canonical_cid, true, JSON.stringify(result));
  // The fixture is a *commit* receipt verified with no anchor -- `unanchored` is the correct,
  // honest answer (`Checks::verified` refuses to call this a pass), not a defect in the bridge.
  assert.equal(result.checks?.inclusion, "unanchored", JSON.stringify(result));
});

test("readGxObjectFile (header-first) agrees with readGxObject (whole-buffer) on the same bytes", async () => {
  const fromDisk = await readGxObjectFile(FIXTURE_GX);
  const fromMemory = readGxObject(fixtureBytes);
  assert.equal(fromDisk.cid, fromMemory.cid);
  assert.equal(fromDisk.receiptJson, fromMemory.receiptJson);
});

// ---------------------------------------------------------------------------
// (b) Cross-implementation round trip, direction 2: SDK-written bytes, read back by the SDK, and
// (when GX_BINARY is set) accepted by the real `gx object verify`.
// ---------------------------------------------------------------------------

test("writeGxObject(read(bytes).receiptJson) reproduces the original bytes exactly (determinism)", () => {
  const file = readGxObject(fixtureBytes);
  const rewritten = writeGxObject(file.receiptJson);
  assert.equal(Buffer.compare(Buffer.from(rewritten), fixtureBytes), 0, "byte-for-byte identical");
});

test("writeGxObject twice on the same input writes the same bytes", () => {
  const file = readGxObject(fixtureBytes);
  const a = writeGxObject(file.receiptJson);
  const b = writeGxObject(file.receiptJson);
  assert.equal(Buffer.compare(Buffer.from(a), Buffer.from(b)), 0);
});

{
  const hasBinary = Boolean(process.env.GX_BINARY);
  const skip = hasBinary ? false : "GX_BINARY is not set (see testlib/gx_process.mjs::gxBinary)";

  test("a live gx object verify accepts an SDK-written file (round trip, direction 2, live)", { skip }, () => {
    const file = readGxObject(fixtureBytes);
    const rewritten = writeGxObject(file.receiptJson);
    const dir = mkdtempSync(join(tmpdir(), "gxfile-sdk-write-"));
    const path = join(dir, "sdk_written.gx");
    writeFileSync(path, rewritten);

    let stdout;
    try {
      stdout = execFileSync(process.env.GX_BINARY, ["object", "verify", path, "--key", FIXTURE_KEY], {
        encoding: "utf8",
      });
    } catch (e) {
      // exit 7 ("failed check") still prints the JSON verdict on stdout -- only a crash (no stdout
      // at all) is a real failure here.
      stdout = e.stdout;
      assert.ok(stdout, `gx object verify produced no stdout: ${e.message}`);
    }
    const verdict = JSON.parse(stdout);
    assert.equal(verdict.checks.identity, true, JSON.stringify(verdict));
    assert.equal(verdict.checks.signature, true, JSON.stringify(verdict));
    assert.equal(verdict.cid, file.cid, JSON.stringify(verdict));
  });

  test("a live gx export -> SDK read -> SDK write -> live gx verify, fresh each run", async () => {
    if (skip) return;
    const dir = mkdtempSync(join(tmpdir(), "gxfile-live-pipeline-"));
    const project = join(dir, "project");
    const home = join(dir, "home");
    const { mkdirSync } = await import("node:fs");
    mkdirSync(project, { recursive: true });
    mkdirSync(home, { recursive: true });
    writeFileSync(join(project, "target.txt"), "before\n");
    writeFileSync(join(project, "intent.txt"), "say after\n");

    const env = { ...process.env, HOME: home, USERPROFILE: home };
    const gx = (args) =>
      JSON.parse(execFileSync(process.env.GX_BINARY, ["--project", project, ...args], { env, encoding: "utf8" }));

    const key = gx(["key", "gen"]);
    const submitted = gx([
      "submit", "--substrate", "fs", "--locator", join(project, "target.txt"),
      "--intent", join(project, "intent.txt"), "--context", "Evidence", "--actor-key", key.key_id,
    ]);
    const planned = gx(["plan", submitted.intent_id]);
    const tid = planned.transformation.id;
    gx(["verify", tid]);
    gx(["commit", tid]);

    const exportedPath = join(dir, "exported.gx");
    execFileSync(process.env.GX_BINARY, ["--project", project, "object", "export", tid, "--out", exportedPath]);

    // Direction 1: SDK reads what the CLI exported.
    const file = await readGxObjectFile(exportedPath);
    assert.equal(file.kind, "Receipt");

    // Direction 2: the CLI verifies what the SDK re-wrote from what it just read.
    const rewritten = writeGxObject(file.receiptJson);
    const sdkPath = join(dir, "sdk_written.gx");
    writeFileSync(sdkPath, rewritten);
    const keyPath = join(dir, "key.pub.json");
    writeFileSync(keyPath, JSON.stringify(key));

    let stdout;
    try {
      stdout = execFileSync(process.env.GX_BINARY, ["object", "verify", sdkPath, "--key", keyPath], {
        encoding: "utf8",
      });
    } catch (e) {
      stdout = e.stdout;
    }
    const verdict = JSON.parse(stdout);
    assert.equal(verdict.checks.identity, true, JSON.stringify(verdict));
    assert.equal(verdict.checks.signature, true, JSON.stringify(verdict));
  });
}

// ---------------------------------------------------------------------------
// (c) Fail-closed: a single flipped bit is refused, never silently accepted.
// ---------------------------------------------------------------------------

test("a flipped identity-claim byte is refused as IdentityMismatch, not accepted", () => {
  const tampered = Buffer.from(fixtureBytes);
  tampered[10] ^= 0x01; // inside the 32-byte claimed-CID region (offset 6..38)
  assert.throws(
    () => readGxObject(tampered),
    (e) => e instanceof GxRefusalError && e.reason.kind === "IdentityMismatch",
  );
});

test("a flipped magic byte is refused as NotGxObjectFile", () => {
  const tampered = Buffer.from(fixtureBytes);
  tampered[0] ^= 0x01;
  assert.throws(
    () => readGxObject(tampered),
    (e) => e instanceof GxRefusalError && e.reason.kind === "NotGxObjectFile",
  );
});

test("a flipped body byte inside the JSON either breaks the parse or moves the identity -- never silently accepted", () => {
  let sawBody = false;
  let sawIdentity = false;
  let sawOther = false;
  for (let offset = GX_HEADER_LEN; offset < fixtureBytes.length; offset++) {
    const tampered = Buffer.from(fixtureBytes);
    tampered[offset] ^= 0x01;
    try {
      readGxObject(tampered);
      // Only reachable if the byte flip landed inside `issued_at` or another unsigned/metadata
      // field that neither the JSON parser nor the identity check is sensitive to -- `gxfile.rs`'s
      // own exhaustive flip census documents the same class (`unsigned_metadata_only`). Not a
      // failure of this test as long as it is not the *whole* body.
      sawOther = true;
    } catch (e) {
      assert.ok(e instanceof GxRefusalError, `byte ${offset}: threw a non-GxRefusalError: ${e}`);
      if (e.reason.kind === "Body" || e.reason.kind === "PayloadType") sawBody = true;
      else if (e.reason.kind === "IdentityMismatch" || e.reason.kind === "BodyNotCanonical") sawIdentity = true;
    }
  }
  assert.ok(sawBody, "no byte flip was ever caught as a JSON/shape refusal");
  assert.ok(sawIdentity, "no byte flip was ever caught as an identity mismatch");
  void sawOther;
});

// ---------------------------------------------------------------------------
// (d) Unknown / unshipped kinds are REJECT, never a silent shrug -- and the two are different
// reasons, mirroring `gxfile.rs`'s `Refusal::UnknownKind` vs `Refusal::KindNotShipped`.
// ---------------------------------------------------------------------------

test("an unregistered kind number is refused as UnknownKind, by name", () => {
  const tampered = Buffer.from(fixtureBytes);
  tampered.writeUInt16BE(GX_KIND_REGISTRY.length + 1, 4);
  assert.throws(
    () => readGxObject(tampered),
    (e) => e instanceof GxRefusalError && e.reason.kind === "UnknownKind" && e.reason.code === GX_KIND_REGISTRY.length + 1,
  );
});

test("kind 0 (reserved) is refused as UnknownKind", () => {
  const tampered = Buffer.from(fixtureBytes);
  tampered.writeUInt16BE(0, 4);
  assert.throws(
    () => readGxObject(tampered),
    (e) => e instanceof GxRefusalError && e.reason.kind === "UnknownKind" && e.reason.code === 0,
  );
});

test("a registered but unshipped kind (Checkpoint) is refused as KindNotShipped, a different reason", () => {
  const tampered = Buffer.from(fixtureBytes);
  const checkpointCode = GX_KIND_REGISTRY.indexOf("Checkpoint") + 1;
  tampered.writeUInt16BE(checkpointCode, 4);
  assert.throws(
    () => readGxObject(tampered),
    (e) => e instanceof GxRefusalError && e.reason.kind === "KindNotShipped" && e.reason.gxKind === "Checkpoint",
  );
});

test("every registered kind other than Receipt is refused as KindNotShipped, none silently read", () => {
  for (const kind of GX_KIND_REGISTRY) {
    if (kind === "Receipt") continue;
    const tampered = Buffer.from(fixtureBytes);
    tampered.writeUInt16BE(GX_KIND_REGISTRY.indexOf(kind) + 1, 4);
    assert.throws(
      () => readGxObject(tampered),
      (e) => e instanceof GxRefusalError && e.reason.kind === "KindNotShipped" && e.reason.gxKind === kind,
      `kind ${kind} was not refused as KindNotShipped`,
    );
  }
});

test("a future format_version is refused as FormatVersion, fail-closed rather than misread", () => {
  const tampered = Buffer.from(fixtureBytes);
  tampered.writeUInt16BE(GX_FORMAT_VERSION + 1, 2);
  assert.throws(
    () => readGxObject(tampered),
    (e) => e instanceof GxRefusalError && e.reason.kind === "FormatVersion" && e.reason.found === GX_FORMAT_VERSION + 1,
  );
});

// ---------------------------------------------------------------------------
// Truncation and malformed-body cases (`gxfile.rs`'s own hostile-file census, §3 of Q1's audit).
// ---------------------------------------------------------------------------

test("truncated headers (every length under HEADER_LEN) are refused as NotGxObjectFile", () => {
  for (let n = 0; n < GX_HEADER_LEN; n++) {
    assert.throws(
      () => readGxObject(fixtureBytes.subarray(0, n)),
      (e) => e instanceof GxRefusalError && e.reason.kind === "NotGxObjectFile",
      `length ${n}`,
    );
  }
});

test("a header with no body (exactly HEADER_LEN bytes) is refused as a Body decode failure", () => {
  assert.throws(
    () => readGxObject(fixtureBytes.subarray(0, GX_HEADER_LEN)),
    (e) => e instanceof GxRefusalError && e.reason.kind === "Body",
  );
});

test("an empty JSON object body is refused as a Body decode failure naming the missing field", () => {
  const header = fixtureBytes.subarray(0, GX_HEADER_LEN);
  const bytes = Buffer.concat([header, Buffer.from("{}")]);
  assert.throws(
    () => readGxObject(bytes),
    (e) => e instanceof GxRefusalError && e.reason.kind === "Body",
  );
});

test("a wrong payload_type is refused as PayloadType, naming both", () => {
  const file = readGxObject(fixtureBytes);
  const receipt = JSON.parse(file.receiptJson);
  receipt.envelope.payload_type = "application/vnd.glovrex.something-else";
  // writeGxObject itself refuses at write time -- both directions are checked, the same as
  // `gxfile::write_receipt` refusing to export a body it did not sign under the declared type.
  assert.throws(
    () => writeGxObject(JSON.stringify(receipt)),
    (e) => e instanceof GxRefusalError && e.reason.kind === "PayloadType",
  );
});

test("invalid base64 in envelope.payload is refused as BodyNotCanonical", () => {
  const file = readGxObject(fixtureBytes);
  const receipt = JSON.parse(file.receiptJson);
  receipt.envelope.payload = "not valid base64!!";
  assert.throws(
    () => writeGxObject(JSON.stringify(receipt)),
    (e) => e instanceof GxRefusalError && e.reason.kind === "BodyNotCanonical",
  );
});

test("writeGxObject refuses non-JSON text as a Body decode failure", () => {
  assert.throws(
    () => writeGxObject("not json at all"),
    (e) => e instanceof GxRefusalError && e.reason.kind === "Body",
  );
});

// ---------------------------------------------------------------------------
// M-4 -- header-first bounded reads (`req/922_artifacts/Q1_e2e_audit.md` §4).
//
// Q1 measured `gx object verify` reading a 96 MiB hostile file entirely into memory (87 ms) before
// its first two bytes were ever inspected. `readGxObjectFile` is asked to do the opposite: refuse
// on the header alone, without a second read for the body, and to refuse a declared-oversized body
// by name rather than allocate it to find out it was too big.
// ---------------------------------------------------------------------------

test("M-4: a large file with bad magic is refused header-first, without reading the body", async () => {
  const dir = mkdtempSync(join(tmpdir(), "gxfile-m4-"));
  const path = join(dir, "big_bad_magic.gx");
  const fd = openSync(path, "w");
  const chunk = Buffer.alloc(1024 * 1024, 0x5a); // 1 MiB of 'Z', nothing resembling "gx"
  const mib = 24; // enough to be clearly "large"; the assertion is on wall-clock, not the number
  for (let i = 0; i < mib; i++) writeSync(fd, chunk);
  closeSync(fd);

  const t0 = Date.now();
  await assert.rejects(
    () => readGxObjectFile(path),
    (e) => e instanceof GxRefusalError && e.reason.kind === "NotGxObjectFile",
  );
  const elapsed = Date.now() - t0;
  // A whole-file read of `mib` megabytes measurably costs tens of milliseconds even on fast local
  // disks; a header-first refusal is single-digit milliseconds. 2000 ms is a generous ceiling that
  // catches "read the whole thing first" without being a timing-sensitive hairline.
  assert.ok(elapsed < 2000, `refusal took ${elapsed}ms -- looks like the whole file was read first`);
});

test("M-4: a declared-oversized body is refused by size (TooLarge), never allocated", async () => {
  const dir = mkdtempSync(join(tmpdir(), "gxfile-m4-"));
  const path = join(dir, "big_declared.gx");
  const header = Buffer.alloc(GX_HEADER_LEN);
  header.write("gx", 0, "latin1");
  header.writeUInt16BE(GX_FORMAT_VERSION, 2);
  header.writeUInt16BE(1, 4); // Receipt
  const fd = openSync(path, "w");
  writeSync(fd, header);
  const chunk = Buffer.alloc(1024 * 1024, 0x41);
  for (let i = 0; i < 8; i++) writeSync(fd, chunk); // 8 MiB declared body
  closeSync(fd);

  await assert.rejects(
    () => readGxObjectFile(path, { maxBodyBytes: 1024 * 1024 }), // 1 MiB ceiling
    (e) => e instanceof GxRefusalError && e.reason.kind === "TooLarge",
  );
});

test("M-4: a body under the ceiling is read normally (the ceiling does not false-positive)", async () => {
  const file = await readGxObjectFile(FIXTURE_GX, { maxBodyBytes: 1024 * 1024 });
  assert.equal(file.cid, FIXTURE_CID);
});
