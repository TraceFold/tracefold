// SPDX-License-Identifier: Apache-2.0
// Filesystem access to the stylesheet of record, kept out of everything a browser
// loads.
//
// tokens.mjs (parts/src) is imported by every face that draws with a token, and a
// face is loaded straight into a real window with no build step in between (req/02
// W15). node:fs does not exist there, so no function that touches a disk may live in
// a file the browser's import graph reaches. This module holds every such function
// instead: resolving the one physical file, proving a junction or a copy is not
// standing in for it, and turning that path into the href a fixture writes into its
// <link>. Only Node-side tools and tests import from here -- generate-tokens.mjs, at
// build time, the fixture writers, at render time, and the drift gate, at test time.
//
// 2026-08-24 (req/03 SS635, glovrex/req/38): this used to point at
// TraceFold_App/ui_proto/ui/tokens.css -- a live dependency on a tree this whole
// project exists to retire and supersede, which is exactly the SS24 retirement
// violation the ruling named. The stylesheet of record is now owned in-repo at
// tokens/tokens.css (glovrex_app root, deliberately outside parts/, shell/kernel/,
// shell/demo/ and every faces/*/ tree -- see that file's own header for why: each of
// those already carries a "no colour literal here" gate that a real token
// declaration would trip).

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));

export const TOKEN_SOURCE_MESSAGES = {
  SOURCE_MISSING: 'the stylesheet of record is not at the path this package consumes',
};

/**
 * The one path. Expressed relative to this file so the pair moves together.
 * parts/tools -> parts -> glovrex_app root -> tokens/tokens.css.
 */
export const TOKEN_SOURCE_RELATIVE = '../../tokens/tokens.css';

export function tokenSourcePath({ from = HERE } = {}) {
  return path.resolve(from, TOKEN_SOURCE_RELATIVE);
}

export function tokenSourceRealPath() {
  const target = tokenSourcePath();
  if (!fs.existsSync(target)) throw new Error(`${TOKEN_SOURCE_MESSAGES.SOURCE_MISSING}: ${target}`);
  return fs.realpathSync.native ? fs.realpathSync.native(target) : fs.realpathSync(target);
}

/** The href a fixture or a face writes into its <link>. Same depth, same one file. */
export function tokenHref(fromDir) {
  return path.relative(fromDir, tokenSourcePath()).split(path.sep).join('/');
}

/** The bytes on disk, read fresh. Used only by the generator and by the drift gate
 * that checks the generator's output against the file it was run against -- runtime
 * code never calls this, because runtime code has no disk to call it against. */
export function readTokenSourceFromDisk() {
  return fs.readFileSync(tokenSourceRealPath(), 'utf8');
}

/**
 * sha256 of a string, hex-encoded. The one place this package computes a digest of
 * the stylesheet, so the generator that stamps one and the test that checks it can
 * never drift apart by computing it two different ways.
 */
export function sha256Hex(text) {
  return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}
