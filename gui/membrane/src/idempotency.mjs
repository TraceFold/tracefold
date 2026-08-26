// SPDX-License-Identifier: Apache-2.0
// The key for the two routes that cache one.
//
// Derived from the method and the row and nothing else. Not the clock, not a random
// id -- a key that changes between two identical calls makes a retry a new request,
// which is the exact opposite of what the header is for. Not the body either: the
// server answers 409 IDEMPOTENCY_CONFLICT when the same key arrives with a different
// body (gx-api/src/idempotency.rs:218-222), and folding the body into the key would
// erase the very disagreement that check exists to report.
//
// Hashed rather than spelled, so that a transformation id can hold any byte a caller
// likes without deciding what ends up in an HTTP header.

// The digest is taken through WebCrypto rather than `node:crypto`, and the derivation is
// unchanged: the same scheme, the same JSON, the same SHA-256, the same first 32 hex
// characters. `test/tables_gen.test.mjs` G5 pins that by computing the old way and the
// new way and comparing them, so this is a change of instrument and not of value. The
// reason is that `node:crypto` cannot be imported by a browser, and one such import
// anywhere in this folder keeps the whole membrane out of every window -- which is what
// req/803 was measuring as C3 = 0/16.
//
// It is async because `crypto.subtle` is. The only caller already awaits inside an async
// request, so nothing upstream changes shape.

/** Bumped only if the derivation changes; two schemes must not collide. */
export const KEY_SCHEME = 'glovrex-app-membrane-1';

const HEX = (bytes) => [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');

export async function stableKey(methodName, row) {
  const text = JSON.stringify([KEY_SCHEME, methodName, row ?? null]);
  const digest = await globalThis.crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return HEX(new Uint8Array(digest)).slice(0, 32);
}
