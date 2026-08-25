// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// 🔴 **E-SDK-1** (`req/00-LOOP_STATE` #11, `req/38` §369 item1's SDK-band box) -- `GxClient.raw`'s
// own doc comment claims to be the escape hatch for four 44 §2.6 extension endpoints (`GET
// /candidates`, `GET /escalations`, `GET /transformations`, `GET /ledger/consistency`), but
// `ledgerConsistency` (below `raw` in this same file) already names a dedicated method for the
// fourth one. The comment is stale: only three extensions still need `raw`.
//
// # Why read the source and not just re-count by eye
//
// The same argument the wasm-vocab and gx_code census tests make: a hand-written list of "the
// endpoints raw covers" is a second place that list lives, and it can drift again the next time an
// extension gets promoted to a named method (as `ledgerConsistency` itself was, per this file's own
// class-header comment) without this test having to be rewritten. So this file derives, from
// `EXTENSION_METHODS` and each named method's own `request()` call, which paths already have a
// dedicated method, and asserts `raw`'s doc-comment does not still claim to be their escape hatch.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = dirname(here);
const CLIENT_SOURCE = join(pkgRoot, "src", "client.ts");

/** `EXTENSION_METHODS`'s entries other than `"raw"` itself -- each one already has a dedicated
 * method, so `raw`'s own doc should not also claim to be an escape hatch for its endpoint. */
function namedExtensionsOtherThanRaw(src) {
  const declStart = src.indexOf("export const EXTENSION_METHODS");
  assert.ok(declStart >= 0, `could not find "export const EXTENSION_METHODS" in ${CLIENT_SOURCE}`);
  const declEnd = src.indexOf("]", declStart) + 1;
  const decl = src.slice(declStart, declEnd);
  return [...decl.matchAll(/"([a-zA-Z0-9_]+)"/g)].map((m) => m[1]).filter((n) => n !== "raw");
}

/** The path literal a named method's own `request()`/`fetchRaw()` call sends -- the wire truth for
 * what that method actually covers, read from the method body rather than its surrounding comment
 * (which is exactly the kind of text that can go stale). */
function pathOwnedByMethod(src, methodName) {
  const methodStart = src.indexOf(`async ${methodName}(`);
  assert.ok(methodStart >= 0, `EXTENSION_METHODS names "${methodName}" but no "async ${methodName}(" method exists in ${CLIENT_SOURCE}`);
  const methodEnd = src.indexOf("\n  }", methodStart);
  const body = src.slice(methodStart, methodEnd);
  const call = body.match(/[`"](\/[a-zA-Z/]+)/);
  assert.ok(call, `could not read a leading path literal out of "${methodName}"'s own request() call`);
  return call[1];
}

/** `raw`'s own JSDoc block -- the text immediately preceding `async raw<`. */
function rawDocComment(src) {
  const declIdx = src.indexOf("async raw<");
  assert.ok(declIdx >= 0, `could not find "async raw<" in ${CLIENT_SOURCE}`);
  const before = src.slice(0, declIdx);
  const docStart = before.lastIndexOf("/**");
  assert.ok(docStart >= 0, `could not find raw()'s preceding doc comment in ${CLIENT_SOURCE}`);
  return src.slice(docStart, declIdx);
}

/** The paths `raw`'s doc-comment enumerates as needing the escape hatch: the backtick-quoted
 * `GET`/`POST /path` items inside its first parenthetical group, up to (not past) any `--`
 * clarifying aside within that same group -- so a note like "(-- `ledgerConsistency` above covers
 * this)" added by the fix does not itself count as a re-claim. */
function pathsRawDocClaims(doc) {
  // JSDoc line-wraps mid-sentence (` * ` continuation prefixes), which would otherwise split a
  // backtick-quoted `GET /path` across a line break and hide it from the regex below.
  const flat = doc.replace(/\n\s*\*\s?/g, " ").replace(/\s+/g, " ");
  const parenMatch = flat.match(/\(([^)]*)\)/);
  if (!parenMatch) return [];
  const beforeAside = parenMatch[1].split("--")[0];
  return [...beforeAside.matchAll(/`(?:GET|POST) (\/[a-zA-Z/]+)`/g)].map((m) => m[1]);
}

test("E-SDK-1: raw()'s doc-comment does not re-claim an endpoint EXTENSION_METHODS already names", () => {
  const src = readFileSync(CLIENT_SOURCE, "utf8");
  const namedExtensions = namedExtensionsOtherThanRaw(src);
  console.log(`EXTENSION_METHODS_OTHER_THAN_RAW=${JSON.stringify(namedExtensions)}`);
  assert.ok(namedExtensions.length >= 1, `parsed no named extensions (besides raw) out of ${CLIENT_SOURCE}`);

  const claimedPaths = pathsRawDocClaims(rawDocComment(src));
  console.log(`RAW_DOC_CLAIMED_PATHS=${JSON.stringify(claimedPaths)}`);

  for (const name of namedExtensions) {
    const path = pathOwnedByMethod(src, name);
    assert.ok(
      !claimedPaths.includes(path),
      `raw()'s doc-comment still lists ${path} as needing the escape hatch, but "${name}" (named in ` +
        `EXTENSION_METHODS) already has a dedicated method for it -- E-SDK-1.`,
    );
  }
});
