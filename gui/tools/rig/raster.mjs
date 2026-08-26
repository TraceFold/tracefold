// SPDX-License-Identifier: Apache-2.0
// Enough of PNG to answer one question: are these two captures the same picture,
// and if not, where and by how much. Decoding is the honest form of that answer --
// comparing base64 tells you two runs differed without telling you they differed in
// a way anybody could see.
//
// Scope, declared: 8-bit depth, colour types 0/2/6, no interlace. Chrome's
// captureScreenshot emits inside that set. Anything else raises rather than
// guessing, because a decoder that guesses is a detector that lies.

import zlib from 'node:zlib';
import crypto from 'node:crypto';

export const RASTER_MESSAGES = {
  NOT_PNG: 'buffer does not carry a png signature',
  UNSUPPORTED: 'png outside the declared decode scope (8-bit, colour type 0/2/6, non-interlaced)',
  SIZE_MISMATCH: 'two rasters of different size cannot be compared pixel for pixel',
  DECODED: 'raster decoded',
};

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const CHANNELS = { 0: 1, 2: 3, 6: 4 };

function paeth(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  if (pb <= pc) return b;
  return c;
}

export function decodePng(buffer) {
  if (!buffer.subarray(0, 8).equals(SIGNATURE)) throw new Error(RASTER_MESSAGES.NOT_PNG);
  let offset = 8;
  let header = null;
  const idat = [];
  while (offset < buffer.length) {
    const length = buffer.readUInt32BE(offset);
    const type = buffer.toString('latin1', offset + 4, offset + 8);
    const body = buffer.subarray(offset + 8, offset + 8 + length);
    if (type === 'IHDR') {
      header = {
        width: body.readUInt32BE(0),
        height: body.readUInt32BE(4),
        bitDepth: body[8],
        colourType: body[9],
        interlace: body[12],
      };
    } else if (type === 'IDAT') idat.push(Buffer.from(body));
    else if (type === 'IEND') break;
    offset += 12 + length;
  }
  if (!header) throw new Error(RASTER_MESSAGES.NOT_PNG);
  const channels = CHANNELS[header.colourType];
  if (header.bitDepth !== 8 || !channels || header.interlace !== 0) {
    throw new Error(`${RASTER_MESSAGES.UNSUPPORTED}: depth=${header.bitDepth} type=${header.colourType} interlace=${header.interlace}`);
  }

  const raw = zlib.inflateSync(Buffer.concat(idat));
  const { width, height } = header;
  const stride = width * channels;
  const pixels = Buffer.alloc(stride * height);
  let src = 0;
  for (let y = 0; y < height; y += 1) {
    const filter = raw[src];
    src += 1;
    const rowStart = y * stride;
    const prevStart = rowStart - stride;
    for (let x = 0; x < stride; x += 1) {
      const value = raw[src + x];
      const left = x >= channels ? pixels[rowStart + x - channels] : 0;
      const up = y > 0 ? pixels[prevStart + x] : 0;
      const upLeft = y > 0 && x >= channels ? pixels[prevStart + x - channels] : 0;
      let out;
      if (filter === 0) out = value;
      else if (filter === 1) out = value + left;
      else if (filter === 2) out = value + up;
      else if (filter === 3) out = value + ((left + up) >> 1);
      else if (filter === 4) out = value + paeth(left, up, upLeft);
      else throw new Error(`${RASTER_MESSAGES.UNSUPPORTED}: filter ${filter}`);
      pixels[rowStart + x] = out & 0xff;
    }
    src += stride;
  }
  return { width, height, channels, pixels };
}

// Ink is "not the page background". Binarising this way survives the one source of
// drift that a byte comparison cannot: a single channel moving by one step in an
// anti-aliased edge. What it cannot survive is a colour change that keeps the same
// coverage -- declared, not hidden (req/06 §4-3 P-c "cannot take" column).
export function inkMask(raster, { threshold = 24 } = {}) {
  const { width, height, channels, pixels } = raster;
  const mask = new Uint8Array(width * height);
  const bg = [pixels[0], pixels[1] ?? pixels[0], pixels[2] ?? pixels[0]];
  let count = 0;
  for (let i = 0, p = 0; i < mask.length; i += 1, p += channels) {
    const r = pixels[p];
    const g = channels >= 3 ? pixels[p + 1] : r;
    const b = channels >= 3 ? pixels[p + 2] : r;
    const distance = Math.abs(r - bg[0]) + Math.abs(g - bg[1]) + Math.abs(b - bg[2]);
    if (distance > threshold) { mask[i] = 1; count += 1; }
  }
  return { width, height, mask, inkPixels: count, background: bg };
}

export function maskDigest(mask) {
  return crypto.createHash('sha256').update(Buffer.from(mask.mask.buffer, mask.mask.byteOffset, mask.mask.length)).digest('hex').slice(0, 16);
}

export function compareMasks(a, b) {
  if (a.width !== b.width || a.height !== b.height) throw new Error(RASTER_MESSAGES.SIZE_MISMATCH);
  let differing = 0;
  let firstAt = null;
  for (let i = 0; i < a.mask.length; i += 1) {
    if (a.mask[i] !== b.mask[i]) {
      differing += 1;
      if (firstAt === null) firstAt = { x: i % a.width, y: Math.floor(i / a.width) };
    }
  }
  return { differing, firstAt, total: a.mask.length };
}

export function comparePixels(a, b) {
  if (a.width !== b.width || a.height !== b.height || a.channels !== b.channels) throw new Error(RASTER_MESSAGES.SIZE_MISMATCH);
  let differing = 0;
  let maxDelta = 0;
  for (let i = 0; i < a.pixels.length; i += a.channels) {
    let delta = 0;
    for (let c = 0; c < a.channels; c += 1) delta = Math.max(delta, Math.abs(a.pixels[i + c] - b.pixels[i + c]));
    if (delta > 0) { differing += 1; maxDelta = Math.max(maxDelta, delta); }
  }
  return { differingPixels: differing, maxChannelDelta: maxDelta, totalPixels: a.width * a.height };
}

export const digestOf = (buffer) => crypto.createHash('sha256').update(buffer).digest('hex').slice(0, 16);
