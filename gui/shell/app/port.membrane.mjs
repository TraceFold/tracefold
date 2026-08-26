// SPDX-License-Identifier: Apache-2.0
// The membrane, in the window. req/803 gap 1, and the single largest move on the GUI band.
//
// Everything this file needed already existed. The membrane was built, and it passed a
// live smoke against a real gx server on 2026-08-24 (`membrane/tools/smoke_2026-08-24.log`,
// 10/10). The window was built, and it mounts six real faces. They had never been
// introduced, because the membrane read its route table off the disk at import time and a
// browser has no disk -- so `req/803` measured C3, "wired to a real backend", at 0/16 for
// every surface in the application. That was one `node:fs` import, not a missing feature.
//
// Two honesty rules this file exists to keep:
//
//   1. It does not fall back quietly. If no bed was named, this window uses the stand-in
//      port and SAYS SO, because "the ledger was never read" and "the ledger is empty" are
//      opposite facts and every face in `faces/` draws them differently on purpose. A
//      silent fallback would make a demonstration look alive and be believed afterwards --
//      the exact failure `port.stand-in.mjs` was written against.
//   2. It classifies nothing. Whatever the engine says, the membrane says. A transport
//      failure stays `failed`, a gate's refusal stays `refused`, and neither is redrawn
//      here as absence.
//
// The origin is this window's own. The bed carries `/v1/*` through to the engine and holds
// the bearer token on its side (`shell/tools/serve.mjs`), so the policy in `app.html` stays
// `connect-src 'self'` unrelaxed and the token never enters the document.

import { createMembrane, BASE_PATH } from '/membrane/src/index.mjs';
import { standInPort, NO_MEMBRANE_SAID } from './port.stand-in.mjs';

/**
 * Who this window says is acting. gx-core/src/context.rs's externally-tagged Actor.
 *
 * It is stated once, here, because the membrane refuses a caller that names its own actor
 * ("an identity a face may state is an identity a face may state falsely"). A window driven
 * by a person at a keyboard is the Human variant; when this application grows a real key it
 * is read from wherever that key lives, and this constant is the one place that changes.
 */
export const ACTOR = Object.freeze({ Human: { key: 'window' } });

/** What the window says about itself, in each of the two states. Said, never inferred. */
export const BOUND_SAID = Object.freeze({
  bound: (origin) => `this window carries the membrane and is bound to ${origin}${BASE_PATH}, so every row below is the server's own or a stated reason why it is not`,
  unbound: NO_MEMBRANE_SAID,
});

/**
 * @param {object} options
 * @param {{named: boolean}} options.bed  what `/.bed.gen.mjs` declared
 * @param {string} options.origin         this window's origin
 * @param {Array} options.notices         the ledger every call is written into
 * @returns {{port: object, bound: boolean, said: string, notices: Array}}
 */
export function windowPort({ bed, origin, notices = [] } = {}) {
  if (!bed?.named) {
    return { port: standInPort(), bound: false, said: BOUND_SAID.unbound, notices };
  }
  // No token here on purpose: the bed attaches it. A token a window holds is a token a
  // window can be made to hand over, and this one has no use for it.
  const { port } = createMembrane({ origin, token: '', actor: ACTOR, notices });
  return { port, bound: true, said: BOUND_SAID.bound(origin), notices };
}
