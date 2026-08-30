// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * BLAKE3-256, unkeyed, fixed 32-byte output -- the one primitive `gxfile.ts` needs and the one
 * this package's dependency rule (`req/132` §6 ruling 2: "dev dependencies are typescript only")
 * has no room for as an npm package.
 *
 * This is an independent implementation of the algorithm the BLAKE3 specification and reference
 * implementation describe (chunking into 1024-byte chunks, compressing 64-byte blocks with the
 * ChaCha-style `G` function, and folding chunk outputs into a binary Merkle tree) -- not a port of
 * `crates/gx-canon/src/cid.rs`, which calls the audited `blake3` Rust crate rather than containing
 * an implementation of its own. Nothing here is ported from that file line for line: the wire
 * shape (chunking, domain-separation flags, tree reduction) is the published algorithm, which any
 * conforming implementation reproduces byte-for-byte with any other -- that is what makes a
 * cross-language identity check possible at all -- while the code that walks it is written fresh.
 *
 * Only what `gxfile.ts` needs is here: the plain (unkeyed) hash, truncated to the first output
 * block. No keyed mode, no `derive_key`, no extendable-output streaming past 32 bytes -- 42 §1.1
 * fixes the CID at exactly `BLAKE3(enc(body))`'s first 32 bytes, and the tree this project's
 * ledger leaves and nodes use (`42 §3.11`) is domain-separated with a one-byte prefix rather than
 * BLAKE3's own keyed mode, so neither extension is reachable from this format.
 *
 * Correctness is not asserted by this file's own say-so: `test/blake3_vectors.test.mjs` checks the
 * empty-input vector the BLAKE3 reference test suite publishes, and `test/gxfile.test.mjs` checks
 * this module against real bytes a real `crates/gx-cli` build produced (ground truth from the
 * audited Rust crate, not from this implementation agreeing with itself).
 */

const OUT_LEN = 32;
const BLOCK_LEN = 64;
const CHUNK_LEN = 1024;

const CHUNK_START = 1 << 0;
const CHUNK_END = 1 << 1;
const PARENT = 1 << 2;
const ROOT = 1 << 3;

/** The BLAKE2s IV, reused as BLAKE3's (both the initial chaining value and half of every
 * compression's fixed half-state). */
const IV = Uint32Array.of(
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
);

/** The fixed message-word permutation applied between rounds. */
const MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

function rotr(x: number, n: number): number {
  return ((x >>> n) | (x << (32 - n))) >>> 0;
}

/** The `G` mixing function over four state words and two message words -- BLAKE2s's, unchanged. */
function g(
  state: Uint32Array,
  a: number,
  b: number,
  c: number,
  d: number,
  mx: number,
  my: number,
): void {
  state[a] = (state[a]! + state[b]! + mx) >>> 0;
  state[d] = rotr(state[d]! ^ state[a]!, 16);
  state[c] = (state[c]! + state[d]!) >>> 0;
  state[b] = rotr(state[b]! ^ state[c]!, 12);
  state[a] = (state[a]! + state[b]! + my) >>> 0;
  state[d] = rotr(state[d]! ^ state[a]!, 8);
  state[c] = (state[c]! + state[d]!) >>> 0;
  state[b] = rotr(state[b]! ^ state[c]!, 7);
}

/** One round: four column mixes, then four diagonal mixes. */
function roundFn(state: Uint32Array, m: Uint32Array): void {
  g(state, 0, 4, 8, 12, m[0]!, m[1]!);
  g(state, 1, 5, 9, 13, m[2]!, m[3]!);
  g(state, 2, 6, 10, 14, m[4]!, m[5]!);
  g(state, 3, 7, 11, 15, m[6]!, m[7]!);
  g(state, 0, 5, 10, 15, m[8]!, m[9]!);
  g(state, 1, 6, 11, 12, m[10]!, m[11]!);
  g(state, 2, 7, 8, 13, m[12]!, m[13]!);
  g(state, 3, 4, 9, 14, m[14]!, m[15]!);
}

function permute(m: Uint32Array): Uint32Array {
  const out = new Uint32Array(16);
  for (let i = 0; i < 16; i++) out[i] = m[MSG_PERMUTATION[i]!]!;
  return out;
}

/** The compression function: sixteen output words from an eight-word chaining value, a sixteen-word
 * message block, a chunk/output counter, the block's real length, and the domain-separation flags. */
function compress(
  cv: Uint32Array,
  blockWords: Uint32Array,
  counter: number,
  blockLen: number,
  flags: number,
): Uint32Array {
  const counterLow = (counter % 0x1_0000_0000) >>> 0;
  const counterHigh = Math.floor(counter / 0x1_0000_0000) >>> 0;

  const state = new Uint32Array(16);
  state.set(cv, 0);
  state.set(IV.subarray(0, 4), 8);
  state[12] = counterLow;
  state[13] = counterHigh;
  state[14] = blockLen >>> 0;
  state[15] = flags >>> 0;

  let block = blockWords;
  for (let round = 0; round < 7; round++) {
    roundFn(state, block);
    if (round < 6) block = permute(block);
  }

  for (let i = 0; i < 8; i++) {
    state[i] = (state[i]! ^ state[i + 8]!) >>> 0;
    state[i + 8] = (state[i + 8]! ^ cv[i]!) >>> 0;
  }
  return state;
}

/** Sixteen little-endian words from a (zero-padded, always-64-byte) block buffer. */
function wordsFromBlock(block: Uint8Array): Uint32Array {
  const words = new Uint32Array(16);
  for (let i = 0; i < 16; i++) {
    const o = i * 4;
    words[i] =
      (block[o]! | (block[o + 1]! << 8) | (block[o + 2]! << 16) | (block[o + 3]! << 24)) >>> 0;
  }
  return words;
}

function wordsToBytes(words: Uint32Array): Uint8Array {
  const out = new Uint8Array(words.length * 4);
  for (let i = 0; i < words.length; i++) {
    const w = words[i]!;
    out[i * 4] = w & 0xff;
    out[i * 4 + 1] = (w >>> 8) & 0xff;
    out[i * 4 + 2] = (w >>> 16) & 0xff;
    out[i * 4 + 3] = (w >>> 24) & 0xff;
  }
  return out;
}

/** A not-yet-truncated compression output -- either a chunk's final block or a parent node -- and
 * the two things done with one: folded into the tree as an eight-word chaining value, or (only for
 * the root) compressed once more with {@link ROOT} set and read as bytes. */
interface Output {
  inputCv: Uint32Array;
  blockWords: Uint32Array;
  counter: number;
  blockLen: number;
  flags: number;
}

function outputChainingValue(output: Output): Uint32Array {
  return compress(output.inputCv, output.blockWords, output.counter, output.blockLen, output.flags).slice(0, 8);
}

function parentOutput(leftCv: Uint32Array, rightCv: Uint32Array, flags: number): Output {
  const blockWords = new Uint32Array(16);
  blockWords.set(leftCv, 0);
  blockWords.set(rightCv, 8);
  return { inputCv: IV, blockWords, counter: 0, blockLen: BLOCK_LEN, flags: PARENT | flags };
}

/** One chunk (up to 1024 bytes, up to sixteen 64-byte blocks), chained across its own blocks. */
class ChunkState {
  cv: Uint32Array;
  readonly chunkCounter: number;
  block = new Uint8Array(BLOCK_LEN);
  blockLen = 0;
  blocksCompressed = 0;
  readonly flags: number;

  constructor(chunkCounter: number, flags: number) {
    this.cv = IV.slice();
    this.chunkCounter = chunkCounter;
    this.flags = flags;
  }

  len(): number {
    return this.blocksCompressed * BLOCK_LEN + this.blockLen;
  }

  private startFlag(): number {
    return this.blocksCompressed === 0 ? CHUNK_START : 0;
  }

  update(input: Uint8Array): void {
    let offset = 0;
    while (offset < input.length) {
      if (this.blockLen === BLOCK_LEN) {
        const words = wordsFromBlock(this.block);
        this.cv = compress(
          this.cv,
          words,
          this.chunkCounter,
          BLOCK_LEN,
          this.flags | this.startFlag(),
        ).slice(0, 8);
        this.blocksCompressed += 1;
        this.block = new Uint8Array(BLOCK_LEN);
        this.blockLen = 0;
      }
      const take = Math.min(BLOCK_LEN - this.blockLen, input.length - offset);
      this.block.set(input.subarray(offset, offset + take), this.blockLen);
      this.blockLen += take;
      offset += take;
    }
  }

  output(): Output {
    return {
      inputCv: this.cv,
      blockWords: wordsFromBlock(this.block),
      counter: this.chunkCounter,
      blockLen: this.blockLen,
      flags: this.flags | this.startFlag() | CHUNK_END,
    };
  }
}

/** The whole-input hasher: chunks fed in, folded into a binary tree of chaining values as complete
 * subtrees close (the same "merge while the running chunk count is even" rule the reference
 * implementation uses), finalized into one root {@link Output}. */
class Hasher {
  private chunkState = new ChunkState(0, 0);
  private readonly cvStack: Uint32Array[] = [];
  private readonly flags = 0;

  update(input: Uint8Array): void {
    let offset = 0;
    while (offset < input.length) {
      if (this.chunkState.len() === CHUNK_LEN) {
        const chunkCv = outputChainingValue(this.chunkState.output());
        const totalChunks = this.chunkState.chunkCounter + 1;
        this.addChunkChainingValue(chunkCv, totalChunks);
        this.chunkState = new ChunkState(totalChunks, this.flags);
      }
      const take = Math.min(CHUNK_LEN - this.chunkState.len(), input.length - offset);
      this.chunkState.update(input.subarray(offset, offset + take));
      offset += take;
    }
  }

  private addChunkChainingValue(newCv: Uint32Array, totalChunks: number): void {
    let cv = newCv;
    let remaining = totalChunks;
    while ((remaining & 1) === 0) {
      const left = this.cvStack.pop();
      if (!left) throw new Error("blake3: chunk-merge stack underflow (unreachable)");
      cv = outputChainingValue(parentOutput(left, cv, this.flags));
      remaining = remaining >>> 1;
    }
    this.cvStack.push(cv);
  }

  private finalOutput(): Output {
    let output = this.chunkState.output();
    for (let i = this.cvStack.length - 1; i >= 0; i--) {
      output = parentOutput(this.cvStack[i]!, outputChainingValue(output), this.flags);
    }
    return output;
  }

  /** The first output block (64 bytes of extendable output), truncated to {@link OUT_LEN}. */
  digest(): Uint8Array {
    const out = this.finalOutput();
    const words = compress(out.inputCv, out.blockWords, out.counter, out.blockLen, out.flags | ROOT);
    return wordsToBytes(words).subarray(0, OUT_LEN);
  }
}

/** `BLAKE3(bytes)`, the first 32 bytes of the extendable output -- what 42 §1.1 calls "BLAKE3-256". */
export function blake3(bytes: Uint8Array): Uint8Array {
  const hasher = new Hasher();
  hasher.update(bytes);
  return hasher.digest();
}
