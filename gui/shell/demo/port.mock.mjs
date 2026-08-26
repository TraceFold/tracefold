// SPDX-License-Identifier: Apache-2.0
// A stand-in for the membrane, so the frame can be shown without a server.
//
// It answers nothing true. Every reply says so, in the reply, under `standIn: true` --
// because the failure this guards against is not a wrong number on a screen, it is a
// screen that looks alive during a demonstration and is believed afterwards.
//
// The method names are not invented here: they come from routes.gen.mjs, which the
// membrane derived from the route table it extracted from the crate. A face that calls a
// method this object has is calling a route that exists.

import { ROUTES } from './routes.gen.mjs';

export const STAND_IN = true;

export function standInPort() {
  const port = {};
  for (const route of ROUTES) {
    port[route.name] = async () => ({
      outcome: 'answered',
      standIn: true,
      said: `${route.verb} ${route.path} was not sent; this shell is running against a stand-in`,
    });
  }
  port.routes = () => ROUTES.map((r) => ({ ...r }));
  port.ledgers = () => ({
    standIn: true,
    said: 'the three declarations of what is not covered are the membrane\'s to compute, and it is not here',
  });
  return port;
}
