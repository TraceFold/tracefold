// SPDX-License-Identifier: Apache-2.0
// The one place the tree is read.
//
// Assays are handed a manifest and never a filesystem, which is the whole of the
// answer to a denominator that changed between two runs of the same commit. It is
// built once, at the start, and it is built again at the end: a run whose tree moved
// underneath it did not measure one tree, and says so instead of averaging.

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import os from 'node:os';

export const MANIFEST_MESSAGES = {
  ROOT_ABSENT: 'the manifest root does not exist',
  BUILT: 'manifest built',
};

// .run holds what a run writes, so leaving it in would make every run change the
// tree it just measured. Everything else stays in -- including the baseline ledger,
// so that updating a baseline moves the tree digest and cannot be done quietly.
const SKIPPED_DIRECTORIES = new Set(['.git', 'node_modules', '.run', 'public']);

function walk(root, current, out) {
  for (const entry of fs.readdirSync(current, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
    const full = path.join(current, entry.name);
    if (entry.isDirectory()) {
      if (SKIPPED_DIRECTORIES.has(entry.name)) continue;
      walk(root, full, out);
    } else if (entry.isFile()) {
      const bytes = fs.readFileSync(full);
      out.push({
        path: path.relative(root, full).split(path.sep).join('/'),
        bytes: bytes.length,
        digest: crypto.createHash('sha256').update(bytes).digest('hex').slice(0, 16),
        text: bytes.toString('utf8'),
      });
    }
  }
  return out;
}

export function buildManifest(root) {
  if (!fs.existsSync(root)) throw new Error(`${MANIFEST_MESSAGES.ROOT_ABSENT}: ${root}`);
  const files = walk(root, root, []);
  const treeDigest = crypto.createHash('sha256')
    .update(files.map((f) => `${f.path}:${f.digest}`).join('\n'))
    .digest('hex').slice(0, 16);
  return {
    root,
    files,
    treeDigest,
    // Named lookups so a selector never has to know where anything lives.
    under: (prefix) => files.filter((f) => f.path.startsWith(prefix)),
    withExtension: (ext) => files.filter((f) => f.path.endsWith(ext)),
    at: (relative) => files.find((f) => f.path === relative) ?? null,
  };
}

// What a picture of this tree depends on that the tree does not contain. Recorded
// rather than removed, because it cannot be removed -- a report from another
// machine is comparable only if this line matches.
export function environmentDigest(extra = {}) {
  const facts = {
    node: process.version,
    platform: `${os.platform()}-${os.arch()}`,
    tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
    locale: Intl.DateTimeFormat().resolvedOptions().locale,
    ...extra,
  };
  return {
    facts,
    digest: crypto.createHash('sha256').update(JSON.stringify(facts)).digest('hex').slice(0, 16),
  };
}
