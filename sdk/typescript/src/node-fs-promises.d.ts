// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * The minimal ambient shape `gxfile.ts`'s `readGxObjectFile` needs from `node:fs/promises`.
 *
 * This package declares no dependency on `@types/node` (`req/132` §6 ruling 2: DOM's ambient
 * types cover every browser-shaped global this SDK touches everywhere else, so a `Node` types
 * package would be pulled in for one file). Rather than widen that to the whole `node:fs`
 * surface, this is the four members `readGxObjectFile` actually calls -- a real `node:fs/promises`
 * satisfies it structurally; nothing here re-implements or narrows what Node itself provides.
 */
declare module "node:fs/promises" {
  interface GxFileHandle {
    stat(): Promise<{ size: number }>;
    read(
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number,
    ): Promise<{ bytesRead: number }>;
    close(): Promise<void>;
  }
  export function open(path: string, flags: string): Promise<GxFileHandle>;
}
