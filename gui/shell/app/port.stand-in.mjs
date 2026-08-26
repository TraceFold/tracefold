// SPDX-License-Identifier: Apache-2.0
// The port this window hands the six real faces, and the whole truth about it.
//
// There is no membrane in this page. `shell/demo/port.mock.mjs` states the reason
// this matters better than a restatement would: the failure being guarded against is
// not a wrong number on a screen, it is a screen that looks alive during a
// demonstration and is believed afterwards. So this object invents nothing. It does
// not answer with an empty list, either -- "the ledger is empty" and "the ledger was
// never read" are opposite facts, and every face in `faces/` is built to draw the
// second one differently from the first. This port produces the second one, on
// purpose, so the shell shows six real faces each saying honestly that no membrane
// was reachable from this window.
//
// What that demonstrates, and it is the thing req/97's gap-list item 2 asked for: the
// shell's rail, launcher and tab strip now reach the six real faces rather than seven
// demo placeholders, and each of them mounts, draws and unmounts inside the frame.
// What it does not demonstrate is any face drawing rows, which needs the membrane in
// front of a bed and is a different lane's work. That limit is stated here, in the
// module, rather than only in a commit message.

import { ROUTES } from '../demo/routes.gen.mjs';

export const STAND_IN = true;

export const NO_MEMBRANE = 'no_membrane_in_this_window';

export const NO_MEMBRANE_SAID = 'this window carries the shell and the faces and no membrane, so nothing was asked of a server and nothing below was read';

/**
 * Every declared route, and the fold a walking face calls, all answering the one
 * shape a face reads as "this was not read, and here is the reason": `absent`, with
 * the reason named and the request echoed. The method names are not invented here --
 * they come from routes.gen.mjs, which the membrane derived from the route table it
 * extracted from the crate, so a face calling a method this object has is calling a
 * route that exists.
 */
export function standInPort() {
  const absent = (name, input) => ({
    outcome: 'absent',
    standIn: true,
    reason: NO_MEMBRANE,
    said: NO_MEMBRANE_SAID,
    requested: { name, input: input ?? null },
  });
  const port = {};
  for (const route of ROUTES) port[route.name] = async (input) => absent(route.name, input);
  port.fold = async (name, input) => absent(name, input);
  port.routes = () => ROUTES.map((r) => ({ ...r }));
  return port;
}
