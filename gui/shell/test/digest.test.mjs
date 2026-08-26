// SPDX-License-Identifier: Apache-2.0
// The digest is checked against BLAKE3's published vectors, not against itself.
//
// The vectors below are the standard ones: the input of length n is the bytes i mod 251,
// and the expected value is the first 32 bytes of the extended output. Written here as
// literals so that a wrong implementation cannot agree with a wrong expectation -- if
// this file were generated from the implementation it would pass forever.

import test from 'node:test';
import assert from 'node:assert/strict';

import { hash, toHex, digestOf, DIGEST_NAME } from '../kernel/digest.mjs';

const VECTORS = Object.freeze({
  0: 'af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262',
  1: '2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213',
  1023: '10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11',
  1024: '42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7',
  1025: 'd00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444',
  2048: 'e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a',
  102400: 'bc3e3d41a1146b069abffad3c0d44860cf664390afce4d9661f7902e7943e085',
});

const material = (n) => {
  const bytes = new Uint8Array(n);
  for (let i = 0; i < n; i += 1) bytes[i] = i % 251;
  return bytes;
};

test('the digest is blake3, by its own vectors', () => {
  for (const [length, expected] of Object.entries(VECTORS)) {
    assert.equal(toHex(hash(material(Number(length)))), expected, `length ${length}`);
  }
  assert.equal(DIGEST_NAME, 'blake3');
});

test('one bit apart is a different digest', () => {
  const a = material(300);
  const b = material(300);
  b[199] ^= 0x01;
  assert.notEqual(toHex(hash(a)), toHex(hash(b)));
});

test('text goes in as utf-8 and the answer is 64 hex characters', () => {
  const value = digestOf('gxw1|light|0|verify');
  assert.match(value, /^[0-9a-f]{64}$/);
  assert.notEqual(value, digestOf('gxw1|dark|0|verify'));
});
