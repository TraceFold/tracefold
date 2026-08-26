// SPDX-License-Identifier: Apache-2.0
// A WebSocket client written against RFC 6455 with nothing but node builtins.
//
// Why this exists: the pixel tier needs a real renderer driven from the outside,
// and req/06 §3-4 forbids a dependency to get one. This is the client subset --
// text frames, continuation, ping/pong, close -- and no server side at all.
//
// It carries CDP messages, which run to several megabytes when a screenshot comes
// back, so the length field is handled at all three widths and the reader never
// assumes a frame arrives in one chunk.

import net from 'node:net';
import crypto from 'node:crypto';

const HANDSHAKE_GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';

const OPCODE = { CONTINUATION: 0x0, TEXT: 0x1, BINARY: 0x2, CLOSE: 0x8, PING: 0x9, PONG: 0xa };

export const WIRE_MESSAGES = {
  BAD_URL: 'websocket url must be ws://host:port/path',
  HANDSHAKE_REFUSED: 'server did not accept the websocket upgrade',
  ACCEPT_MISMATCH: 'server returned a Sec-WebSocket-Accept this client did not ask for',
  CLOSED_EARLY: 'socket closed before the frame it was carrying finished',
  OPEN: 'websocket open',
};

function parseWsUrl(url) {
  const m = /^ws:\/\/([^/:]+):(\d+)(\/.*)?$/.exec(url);
  if (!m) throw new Error(`${WIRE_MESSAGES.BAD_URL}: ${url}`);
  return { host: m[1], port: Number(m[2]), path: m[3] || '/' };
}

// One growing buffer, drained frame by frame. Returns the frames it could complete
// and keeps the tail for the next chunk.
function drainFrames(state) {
  const out = [];
  for (;;) {
    const buf = state.buffer;
    if (buf.length < 2) break;
    const first = buf[0];
    const second = buf[1];
    const fin = (first & 0x80) !== 0;
    const opcode = first & 0x0f;
    const masked = (second & 0x80) !== 0;
    let length = second & 0x7f;
    let offset = 2;
    if (length === 126) {
      if (buf.length < offset + 2) break;
      length = buf.readUInt16BE(offset);
      offset += 2;
    } else if (length === 127) {
      if (buf.length < offset + 8) break;
      const big = buf.readBigUInt64BE(offset);
      length = Number(big);
      offset += 8;
    }
    let maskKey = null;
    if (masked) {
      if (buf.length < offset + 4) break;
      maskKey = buf.subarray(offset, offset + 4);
      offset += 4;
    }
    if (buf.length < offset + length) break;
    let payload = Buffer.from(buf.subarray(offset, offset + length));
    if (maskKey) for (let i = 0; i < payload.length; i += 1) payload[i] ^= maskKey[i % 4];
    state.buffer = buf.subarray(offset + length);
    out.push({ fin, opcode, payload });
  }
  return out;
}

function encodeFrame(opcode, payload) {
  const body = Buffer.isBuffer(payload) ? payload : Buffer.from(payload, 'utf8');
  const maskKey = crypto.randomBytes(4);
  let header;
  if (body.length < 126) {
    header = Buffer.alloc(2);
    header[1] = 0x80 | body.length;
  } else if (body.length < 65536) {
    header = Buffer.alloc(4);
    header[1] = 0x80 | 126;
    header.writeUInt16BE(body.length, 2);
  } else {
    header = Buffer.alloc(10);
    header[1] = 0x80 | 127;
    header.writeBigUInt64BE(BigInt(body.length), 2);
  }
  header[0] = 0x80 | opcode;
  const masked = Buffer.allocUnsafe(body.length);
  for (let i = 0; i < body.length; i += 1) masked[i] = body[i] ^ maskKey[i % 4];
  return Buffer.concat([header, maskKey, masked]);
}

// The ceiling on the upgrade. A socket that accepts and then says nothing is
// indistinguishable from a slow one without it, and the difference matters.
const HANDSHAKE_CEILING_MS = 15000;

export function openWire(url) {
  const { host, port, path } = parseWsUrl(url);
  const key = crypto.randomBytes(16).toString('base64');
  const expectedAccept = crypto.createHash('sha1').update(key + HANDSHAKE_GUID).digest('base64');

  return new Promise((resolve, reject) => {
    const socket = net.connect({ host, port });
    socket.setNoDelay(true);
    const state = { buffer: Buffer.alloc(0) };
    let upgraded = false;
    let handshakeBuf = Buffer.alloc(0);
    const listeners = { message: [], close: [] };
    const fragments = { opcode: null, chunks: [] };

    const wire = {
      url,
      send(text) { socket.write(encodeFrame(OPCODE.TEXT, text)); },
      onMessage(fn) { listeners.message.push(fn); },
      onClose(fn) { listeners.close.push(fn); },
      close() {
        try { socket.write(encodeFrame(OPCODE.CLOSE, Buffer.alloc(0))); } catch { /* already gone */ }
        socket.destroy();
      },
    };

    const ceiling = AbortSignal.timeout(HANDSHAKE_CEILING_MS);
    ceiling.addEventListener('abort', () => {
      if (upgraded) return;
      socket.destroy();
      reject(new Error(`${WIRE_MESSAGES.HANDSHAKE_REFUSED}: no response within ${HANDSHAKE_CEILING_MS}ms`));
    });

    socket.on('error', (err) => { if (!upgraded) reject(err); else listeners.close.forEach((fn) => fn(err)); });
    socket.on('close', () => { if (upgraded) listeners.close.forEach((fn) => fn(null)); });

    socket.on('data', (chunk) => {
      if (!upgraded) {
        handshakeBuf = Buffer.concat([handshakeBuf, chunk]);
        const end = handshakeBuf.indexOf('\r\n\r\n');
        if (end === -1) return;
        const head = handshakeBuf.subarray(0, end).toString('latin1');
        state.buffer = handshakeBuf.subarray(end + 4);
        if (!/^HTTP\/1\.1 101/.test(head)) { reject(new Error(`${WIRE_MESSAGES.HANDSHAKE_REFUSED}: ${head.split('\r\n')[0]}`)); socket.destroy(); return; }
        const accept = /sec-websocket-accept:\s*(\S+)/i.exec(head);
        if (!accept || accept[1] !== expectedAccept) { reject(new Error(WIRE_MESSAGES.ACCEPT_MISMATCH)); socket.destroy(); return; }
        upgraded = true;
        resolve(wire);
      } else {
        state.buffer = Buffer.concat([state.buffer, chunk]);
      }
      if (!upgraded) return;
      for (const frame of drainFrames(state)) {
        if (frame.opcode === OPCODE.PING) { socket.write(encodeFrame(OPCODE.PONG, frame.payload)); continue; }
        if (frame.opcode === OPCODE.PONG) continue;
        if (frame.opcode === OPCODE.CLOSE) { socket.destroy(); continue; }
        if (frame.opcode === OPCODE.CONTINUATION) fragments.chunks.push(frame.payload);
        else { fragments.opcode = frame.opcode; fragments.chunks = [frame.payload]; }
        if (!frame.fin) continue;
        const body = Buffer.concat(fragments.chunks);
        fragments.chunks = [];
        if (fragments.opcode === OPCODE.TEXT) listeners.message.forEach((fn) => fn(body.toString('utf8')));
      }
    });

    socket.on('connect', () => {
      socket.write(
        `GET ${path} HTTP/1.1\r\n`
        + `Host: ${host}:${port}\r\n`
        + 'Upgrade: websocket\r\n'
        + 'Connection: Upgrade\r\n'
        + `Sec-WebSocket-Key: ${key}\r\n`
        + 'Sec-WebSocket-Version: 13\r\n\r\n',
      );
    });
  });
}
