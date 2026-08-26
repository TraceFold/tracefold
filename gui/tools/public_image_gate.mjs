// SPDX-License-Identifier: Apache-2.0
// Asserts that the published gui/ is the image of this source tree at a named commit.
//
//   node tools/public_image_gate.mjs --pin <sha> --clone <path-to-public-checkout>
//   node tools/public_image_gate.mjs --pin <sha> --clone <path> --json <out>
//
// Rebuilds the image the same way publication does -- `git archive <pin>` minus the
// exclusions written down in SYNC.md -- and compares it to `gui/` in the clone by
// SHA-256, file by file. Three findings are possible and they are kept apart, because
// "the public tree has a file the image does not" and "the two disagree about a file's
// bytes" have different causes and different repairs:
//
//   differs      the same path, different bytes
//   only_image   in the derived image, absent from the public tree  (never published)
//   only_public  in the public tree, absent from the image          (edited in place,
//                or left behind by an exclusion that grew)
//
// Exit 0 all three empty, 1 any non-empty, 2 it could not run (bad pin, no clone, no
// gui/ inside it) -- the same three-way shape the other gates in this directory use, so
// that "could not check" is never spelled the same way as "checked and agreed".
//
// This never writes to either side. A sync tool that also reports on itself would make
// the report worthless.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import url from 'node:url';
import crypto from 'node:crypto';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

/** The exclusion list, as predicates over a repo-relative POSIX path. SYNC.md is prose
 *  about this array; if they disagree, they are both wrong and this one is running. */
const EXCLUDED = [
  (p) => p === 'req' || p.startsWith('req/'),
  (p) => p === '.run' || p.startsWith('.run/'),
  (p) => /(^|\/)record\/critique_[^/]*\//.test(p),
  (p) => /(^|\/)record\/req[0-9][^/]*\//.test(p),
  (p) => /(^|\/)record\/[^/]*\.png$/.test(p),
  (p) => /\/fixtures\/shots\/measurements\.json$/.test(p),
  (p) => /\/fixtures\/shots\/browser-mount-smoke\.json$/.test(p),
  (p) => /(^|\/)record\/shots\.json$/.test(p),
  (p) => /^docs\/APP_ARCHMAP_.*\.html$/.test(p),
  (p) => p === 'docs/ABSORPTION_STATUS.html',
  (p) => p.endsWith('.log'),
  (p) => /\.bak-/.test(p),
];

export function isExcluded(relPath) {
  return EXCLUDED.some((rule) => rule(relPath));
}

const argOf = (name) => {
  const i = process.argv.indexOf(name);
  return i >= 0 ? process.argv[i + 1] : null;
};

function die(message) {
  console.error(`public_image_gate: CANNOT CHECK -- ${message}`);
  process.exit(2);
}

function sha256(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

/** Every tracked path at `pin`, minus the exclusions, with the digest of its bytes. */
function imageAt(pin) {
  let listing;
  try {
    listing = execFileSync('git', ['ls-tree', '-r', '--name-only', pin], { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  } catch {
    die(`\`git ls-tree\` refused the pin "${pin}" -- is it a commit in this tree?`);
  }
  const out = new Map();
  for (const rel of listing.split('\n').map((s) => s.trim()).filter(Boolean)) {
    if (isExcluded(rel)) continue;
    const bytes = execFileSync('git', ['show', `${pin}:${rel}`], { cwd: ROOT, maxBuffer: 64 * 1024 * 1024 });
    out.set(rel, sha256(bytes));
  }
  return out;
}

/** Every file under the clone's gui/, with the digest of its bytes. */
function publicTree(guiDir) {
  const out = new Map();
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) { walk(full); continue; }
      const rel = path.relative(guiDir, full).split(path.sep).join('/');
      out.set(rel, sha256(fs.readFileSync(full)));
    }
  };
  walk(guiDir);
  return out;
}

const pin = argOf('--pin');
const clone = argOf('--clone');
if (!pin || !clone) die('both --pin <sha> and --clone <path> are required');

const guiDir = path.join(path.resolve(clone), 'gui');
if (!fs.existsSync(guiDir)) die(`no gui/ directory inside "${clone}" -- is that a checkout of the public repository?`);

const image = imageAt(pin);
const published = publicTree(guiDir);

const differs = [];
const onlyImage = [];
const onlyPublic = [];
for (const [rel, digest] of image) {
  if (!published.has(rel)) onlyImage.push(rel);
  else if (published.get(rel) !== digest) differs.push(rel);
}
for (const rel of published.keys()) if (!image.has(rel)) onlyPublic.push(rel);

differs.sort(); onlyImage.sort(); onlyPublic.sort();

console.log(`public_image_gate: image at ${pin.slice(0, 12)} = ${image.size} files, published gui/ = ${published.size} files`);
const show = (label, list, why) => {
  if (!list.length) return;
  console.log(`\n  ${label} (${list.length}) -- ${why}:`);
  for (const p of list) console.log(`        - ${p}`);
};
show('differs', differs, 'same path, different bytes');
show('only in image', onlyImage, 'derived here but never published');
show('only in published', onlyPublic, 'in the public tree but not derivable from this pin');

const clean = !differs.length && !onlyImage.length && !onlyPublic.length;
console.log(clean
  ? `\npublic_image_gate: the published gui/ is byte-identical to the image of ${pin.slice(0, 12)}`
  : '\npublic_image_gate: RED -- the published tree is not the image of this pin');

const jsonAt = argOf('--json');
if (jsonAt) {
  fs.mkdirSync(path.dirname(path.resolve(jsonAt)), { recursive: true });
  fs.writeFileSync(path.resolve(jsonAt), `${JSON.stringify({ pin, imageFiles: image.size, publishedFiles: published.size, differs, onlyImage, onlyPublic, clean }, null, 2)}\n`);
}

process.exitCode = clean ? 0 : 1;
