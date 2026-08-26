// SPDX-License-Identifier: Apache-2.0
// BLAKE3, hash mode, no key and no context. Written here because the shell has no
// dependencies and node has no blake3, and because a shell that invented its own
// checksum would be claiming a layout matched on 16 bits of evidence.
//
// The only reason this file is allowed to exist is that its answer is checkable against
// a published vector rather than against itself: see test/digest.test.mjs. An
// implementation nobody can contradict is not a digest, it is a habit.

const IV = Uint32Array.from([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
]);

const PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

export const FLAG = Object.freeze({
  CHUNK_START: 1, CHUNK_END: 2, PARENT: 4, ROOT: 8,
});

export const BLOCK_LEN = 64;
export const CHUNK_LEN = 1024;

const rotr = (word, n) => ((word >>> n) | (word << (32 - n))) >>> 0;

function mix(s, a, b, c, d, mx, my) {
  s[a] = (s[a] + s[b] + mx) >>> 0;
  s[d] = rotr(s[d] ^ s[a], 16);
  s[c] = (s[c] + s[d]) >>> 0;
  s[b] = rotr(s[b] ^ s[c], 12);
  s[a] = (s[a] + s[b] + my) >>> 0;
  s[d] = rotr(s[d] ^ s[a], 8);
  s[c] = (s[c] + s[d]) >>> 0;
  s[b] = rotr(s[b] ^ s[c], 7);
}

function round(s, m) {
  mix(s, 0, 4, 8, 12, m[0], m[1]);
  mix(s, 1, 5, 9, 13, m[2], m[3]);
  mix(s, 2, 6, 10, 14, m[4], m[5]);
  mix(s, 3, 7, 11, 15, m[6], m[7]);
  mix(s, 0, 5, 10, 15, m[8], m[9]);
  mix(s, 1, 6, 11, 12, m[10], m[11]);
  mix(s, 2, 7, 8, 13, m[12], m[13]);
  mix(s, 3, 4, 9, 14, m[14], m[15]);
}

/** The one compression. Everything else in this file only decides what to hand it. */
function compress(cv, block, counter, blockLen, flags) {
  const s = new Uint32Array(16);
  s.set(cv.subarray(0, 8), 0);
  s.set(IV.subarray(0, 4), 8);
  s[12] = counter >>> 0;
  s[13] = Math.floor(counter / 4294967296) >>> 0;
  s[14] = blockLen >>> 0;
  s[15] = flags >>> 0;

  let m = Uint32Array.from(block);
  for (let r = 0; r < 7; r += 1) {
    round(s, m);
    if (r < 6) {
      const next = new Uint32Array(16);
      for (let i = 0; i < 16; i += 1) next[i] = m[PERMUTATION[i]];
      m = next;
    }
  }
  for (let i = 0; i < 8; i += 1) {
    s[i] = (s[i] ^ s[i + 8]) >>> 0;
    s[i + 8] = (s[i + 8] ^ cv[i]) >>> 0;
  }
  return s;
}

function wordsOf(bytes, offset, length) {
  const block = new Uint32Array(16);
  for (let i = 0; i < length; i += 1) {
    block[i >> 2] |= bytes[offset + i] << ((i & 3) * 8);
  }
  return block;
}

/**
 * A node whose compression has not happened yet, so the caller can still decide whether
 * it is the root. This is the only reason the last block is not folded in place.
 */
const node = (cv, block, blockLen, counter, flags) => ({ cv, block, blockLen, counter, flags });

const chainingValue = (n) => compress(n.cv, n.block, n.counter, n.blockLen, n.flags).subarray(0, 8);

function rootBytes(n, outLen) {
  const out = new Uint8Array(outLen);
  let written = 0;
  let counter = 0;
  while (written < outLen) {
    const s = compress(n.cv, n.block, n.counter + counter, n.blockLen, n.flags | FLAG.ROOT);
    for (let i = 0; i < 16 && written < outLen; i += 1) {
      for (let b = 0; b < 4 && written < outLen; b += 1) {
        out[written] = (s[i] >>> (b * 8)) & 0xff;
        written += 1;
      }
    }
    counter += 1;
  }
  return out;
}

/** Every block of one chunk except the last; the last is handed back undecided. */
function chunkNode(bytes, from, len, chunkIndex) {
  let cv = IV;
  const blocks = Math.max(1, Math.ceil(len / BLOCK_LEN));
  for (let b = 0; b < blocks - 1; b += 1) {
    const flags = b === 0 ? FLAG.CHUNK_START : 0;
    cv = compress(cv, wordsOf(bytes, from + b * BLOCK_LEN, BLOCK_LEN), chunkIndex, BLOCK_LEN, flags)
      .subarray(0, 8);
  }
  const last = blocks - 1;
  const lastLen = len - last * BLOCK_LEN;
  const flags = (last === 0 ? FLAG.CHUNK_START : 0) | FLAG.CHUNK_END;
  return node(cv, wordsOf(bytes, from + last * BLOCK_LEN, Math.max(0, lastLen)), Math.max(0, lastLen), chunkIndex, flags);
}

function parentNode(left, right) {
  const block = new Uint32Array(16);
  block.set(left, 0);
  block.set(right, 8);
  return node(IV, block, BLOCK_LEN, 0, FLAG.PARENT);
}

/** @param {Uint8Array} bytes @param {number} [outLen] */
export function hash(bytes, outLen = 32) {
  const total = Math.max(1, Math.ceil(bytes.length / CHUNK_LEN));
  const stack = [];
  let pending = null;

  for (let c = 0; c < total; c += 1) {
    const from = c * CHUNK_LEN;
    const len = Math.min(CHUNK_LEN, bytes.length - from);
    pending = chunkNode(bytes, from, Math.max(0, len), c);
    if (c === total - 1) break;
    let cv = chainingValue(pending);
    let merged = c + 1;
    while ((merged & 1) === 0) {
      cv = chainingValue(parentNode(stack.pop(), cv));
      merged >>= 1;
    }
    stack.push(cv);
  }

  while (stack.length > 0) {
    pending = parentNode(stack.pop(), chainingValue(pending));
  }
  return rootBytes(pending, outLen);
}

const HEX = '0123456789abcdef';

export function toHex(bytes) {
  let out = '';
  for (let i = 0; i < bytes.length; i += 1) out += HEX[bytes[i] >> 4] + HEX[bytes[i] & 15];
  return out;
}

const encoder = new TextEncoder();

/** The shell's one digest of a line of text. */
export const digestOf = (text, outLen = 32) => toHex(hash(encoder.encode(text), outLen));

/** The algorithm's name travels with the value so a receipt never has to be guessed at. */
export const DIGEST_NAME = 'blake3';
