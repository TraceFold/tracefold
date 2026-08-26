// SPDX-License-Identifier: Apache-2.0
// The DevTools protocol on top of the hand-built wire: request/response by id,
// events by name, and flat sessions so a page is addressed without a second socket.

import { openWire } from './wire_ws.mjs';

export const CDP_MESSAGES = {
  CALL_FAILED: 'the renderer refused the call',
  NO_SESSION: 'the page target did not attach',
  SOCKET_GONE: 'the renderer socket closed while a call was outstanding',
};

export async function openCdp(browserWsUrl) {
  const wire = await openWire(browserWsUrl);
  let nextId = 0;
  const pending = new Map();
  const eventHandlers = new Map();

  wire.onMessage((text) => {
    let msg;
    try { msg = JSON.parse(text); } catch { return; }
    if (msg.id !== undefined && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(`${CDP_MESSAGES.CALL_FAILED}: ${msg.error.message} (${msg.error.code})`));
      else resolve(msg.result);
      return;
    }
    if (msg.method) {
      for (const fn of eventHandlers.get(msg.method) ?? []) fn(msg.params, msg.sessionId);
    }
  });

  wire.onClose(() => {
    for (const { reject } of pending.values()) reject(new Error(CDP_MESSAGES.SOCKET_GONE));
    pending.clear();
  });

  const call = (method, params = {}, sessionId) => new Promise((resolve, reject) => {
    nextId += 1;
    const id = nextId;
    pending.set(id, { resolve, reject });
    const frame = { id, method, params };
    if (sessionId) frame.sessionId = sessionId;
    wire.send(JSON.stringify(frame));
  });

  const on = (method, fn) => {
    if (!eventHandlers.has(method)) eventHandlers.set(method, []);
    eventHandlers.get(method).push(fn);
  };

  return {
    call,
    on,
    close: () => wire.close(),

    // A page target, attached flat, wrapped so callers never carry the session id.
    async newPage() {
      const { targetId } = await call('Target.createTarget', { url: 'about:blank' });
      const { sessionId } = await call('Target.attachToTarget', { targetId, flatten: true });
      if (!sessionId) throw new Error(CDP_MESSAGES.NO_SESSION);
      const page = {
        targetId,
        sessionId,
        send: (method, params) => call(method, params, sessionId),
        on: (method, fn) => on(method, (params, sid) => { if (sid === sessionId) fn(params); }),
        close: () => call('Target.closeTarget', { targetId }),
      };
      await page.send('Page.enable');
      await page.send('Runtime.enable');
      return page;
    },
  };
}
