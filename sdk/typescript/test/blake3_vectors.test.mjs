// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// `src/blake3.ts` is an independent TypeScript implementation of BLAKE3-256 (`gxfile.ts` needs it
// because this package's dependency rule -- `req/132` §6 ruling 2, "dev dependencies are
// typescript only" -- has no room for an npm crypto package). Two kinds of ground truth, neither
// this module's own say-so:
//
// 1. The BLAKE3 reference test suite's published empty-input vector (a published, citable number
//    independent of anything in this repository).
// 2. A real `.gx` file a real `crates/gx-cli` build wrote (`test/fixtures/gxfile/frozen_commit.gx`,
//    see `test/gxfile.test.mjs`'s header for provenance) -- the audited Rust `blake3` crate's own
//    output, over real bytes, not a vector either implementation could have gotten wrong the same
//    way.
import { test } from "node:test";
import assert from "node:assert/strict";

import { blake3 } from "../dist/blake3.js";

function hex(bytes) {
  return Buffer.from(bytes).toString("hex");
}

test("BLAKE3-256 of the empty input matches the published reference vector", () => {
  const digest = blake3(new Uint8Array(0));
  assert.equal(digest.length, 32, "BLAKE3-256 is always 32 bytes");
  assert.equal(
    hex(digest),
    "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
    "the BLAKE3 test suite's own zero-length vector",
  );
});

test("BLAKE3-256 is deterministic and content-sensitive over inputs spanning multiple chunks", () => {
  // CHUNK_LEN is 1024 bytes; this input is 5000, so the tree-merge path (not only the single-chunk
  // path the fixture below exercises, whose payload is well under one chunk) runs at least once.
  const big = new Uint8Array(5000);
  for (let i = 0; i < big.length; i++) big[i] = i % 251;

  const first = blake3(big);
  const second = blake3(big.slice()); // a fresh copy, so this is not the same Uint8Array twice
  assert.equal(hex(first), hex(second), "same bytes, same digest");

  const flipped = big.slice();
  flipped[4999] ^= 0x01; // last byte of the last (partial) block of the last chunk
  assert.notEqual(hex(blake3(flipped)), hex(first), "one flipped bit changes the digest");
});

test("BLAKE3-256 output length is fixed regardless of input length", () => {
  for (const len of [0, 1, 63, 64, 65, 1023, 1024, 1025, 2048]) {
    const input = new Uint8Array(len);
    assert.equal(blake3(input).length, 32, `len=${len}`);
  }
});
