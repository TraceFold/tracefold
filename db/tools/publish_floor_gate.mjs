// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const DEFAULT_README = path.join(ROOT, 'README.md');
const DEFAULT_TESTS = path.join(ROOT, 'crates', 'db', 'tests');

function option(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  if (at === -1 || at + 1 >= process.argv.length) return fallback;
  return process.argv[at + 1];
}

const README = option('readme', DEFAULT_README);
const PUBLIC_TESTS = option('public', DEFAULT_TESTS);
const PRIVATE_TESTS = option('private', null);

function refuse(reason, detail) {
  process.stderr.write(`publish_floor_gate: ${reason}\n${detail}\n`);
  process.exit(2);
}

function countFiles(dir) {
  let files;
  try {
    files = readdirSync(dir, { withFileTypes: true });
  } catch (error) {
    return null;
  }
  let count = 0;
  for (const entry of files) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) count += countFiles(full) ?? 0;
    else if (statSync(full).isFile()) count += 1;
  }
  return count;
}

function declared(name) {
  const text = readFileSync(README, 'utf8');
  const row = new RegExp(`\\| ${name} \\| (\\d+) \\|`).exec(text);
  if (!row) refuse('DECLARATION_MISSING', `no "${name}" row in ${README}`);
  return Number(row[1]);
}

const declaredCarried = declared('files under `tests/` in this folder');
const declaredExcluded = declared('files left out when this folder was assembled');

const onDisk = countFiles(PUBLIC_TESTS);
if (onDisk === null) refuse('RANGE_LOST', `${PUBLIC_TESTS} is not a directory`);
if (onDisk === 0) refuse('UNTESTABLE', `${PUBLIC_TESTS} has zero files; a scan of nothing is not a pass`);

if (onDisk !== declaredCarried) {
  refuse(
    'FLOOR_MISMATCH',
    `README declares ${declaredCarried} carried file(s); ${PUBLIC_TESTS} holds ${onDisk}`
  );
}

let privateLine = 'PRIVATE_NOT_GIVEN: pass --private <path> to check it against the excluded count too';
if (PRIVATE_TESTS) {
  const privateCount = countFiles(PRIVATE_TESTS);
  if (privateCount === null) refuse('RANGE_LOST', `${PRIVATE_TESTS} is not a directory`);
  if (privateCount === 0) refuse('UNTESTABLE', `${PRIVATE_TESTS} has zero files; a scan of nothing is not a pass`);
  const floor = declaredCarried + declaredExcluded;
  if (privateCount !== floor) {
    refuse(
      'FLOOR_MISMATCH',
      `private tree holds ${privateCount} file(s); README's carried (${declaredCarried}) + excluded (${declaredExcluded}) = ${floor}`
    );
  }
  privateLine = `private=${privateCount} matches carried(${declaredCarried})+excluded(${declaredExcluded})`;
}

process.stdout.write(
  `public=${onDisk} carried_declared=${declaredCarried} excluded_declared=${declaredExcluded}\n${privateLine}\nFLOOR_HOLDS\n`
);
