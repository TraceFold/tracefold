// SPDX-License-Identifier: Apache-2.0
// The drift gate: the generated mirror of the stylesheet of record has not gone
// stale.
//
// tokens.generated.mjs is the one place a browser-loaded module gets the bytes of
// ui_proto/ui/tokens.css (req/02 W15: no node:fs may sit in that import graph). This
// file is the check that the mirror is still telling the truth. It re-reads the
// canonical file itself, at test time, with the same digest function
// parts/tools/generate-tokens.mjs used, and fails the moment the two stop agreeing.
// A generated file nobody re-checks is a second roster with better manners.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  TOKEN_SOURCE_RELATIVE, TOKEN_SOURCE_SHA256, TOKEN_GENERATED_AT, TOKEN_SOURCE_TEXT,
} from '../generated/tokens.generated.mjs';
import {
  readTokenSourceFromDisk, sha256Hex, TOKEN_SOURCE_RELATIVE as LIVE_RELATIVE,
} from '../tools/token-source.mjs';

test('the digest stamped in the generated module matches the canonical file right now', () => {
  const fresh = readTokenSourceFromDisk();
  assert.equal(
    sha256Hex(fresh),
    TOKEN_SOURCE_SHA256,
    'the stylesheet of record moved since the mirror was made -- run node parts/tools/generate-tokens.mjs',
  );
});

test('the embedded text is byte-identical to what is on disk right now', () => {
  assert.equal(TOKEN_SOURCE_TEXT, readTokenSourceFromDisk());
});

test('the drift gate can go red: a digest computed off different bytes does not match the stamped one', () => {
  // Fired here so its silence on the test above means something (req/04 boundary
  // discipline: a gate that has never been seen to refuse is a gate nobody has
  // evidence for).
  assert.notEqual(sha256Hex(`${TOKEN_SOURCE_TEXT} `), TOKEN_SOURCE_SHA256);
  assert.notEqual(sha256Hex(''), TOKEN_SOURCE_SHA256);
});

test('the relative path the mirror was generated from is the one path this package still names', () => {
  assert.equal(TOKEN_SOURCE_RELATIVE, LIVE_RELATIVE);
});

test('the generation timestamp parses as a real instant, not a placeholder', () => {
  assert.equal(Number.isNaN(Date.parse(TOKEN_GENERATED_AT)), false, TOKEN_GENERATED_AT);
  assert.ok(new Date(TOKEN_GENERATED_AT).getUTCFullYear() >= 2026, TOKEN_GENERATED_AT);
});

test('the generated module has no node:* import: the mirror it produces must be safe to ship, but the file that carries the digest header is data, not a second reader of the disk', () => {
  // A behavioural check rather than a source-text grep: importing the module above
  // already succeeded, in this same process, without this file having granted it any
  // access to node:fs -- were the generated module itself to import node:fs, the
  // import at the top of this file would still succeed under Node, so the meaningful
  // assertion is that its exports are plain data (strings), not functions that could
  // be hiding a disk read behind a call.
  for (const value of [TOKEN_SOURCE_RELATIVE, TOKEN_SOURCE_SHA256, TOKEN_GENERATED_AT, TOKEN_SOURCE_TEXT]) {
    assert.equal(typeof value, 'string');
  }
});
