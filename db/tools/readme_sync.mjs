// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const DEFAULT_README = path.join(ROOT, 'README.md');

function option(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  if (at === -1 || at + 1 >= process.argv.length) return fallback;
  return process.argv[at + 1];
}

const WRITE = process.argv.includes('--write');
const README = option('readme', DEFAULT_README);
const DUMP = option('dump', null);
const EXEC = option('exec', 'db');

function refuse(reason, detail) {
  process.stderr.write(`readme_sync: ${reason}\n${detail}\n`);
  process.exit(2);
}

function loadDump() {
  if (DUMP) {
    try {
      return JSON.parse(readFileSync(DUMP, 'utf8'));
    } catch (error) {
      refuse('DUMP_UNREADABLE', `${DUMP}: ${error.message}`);
    }
  }
  const parts = EXEC.split(' ').filter((piece) => piece.length > 0);
  const run = spawnSync(parts[0], [...parts.slice(1), '--dump-commands'], { encoding: 'utf8' });
  if (run.error) refuse('BINARY_NOT_RUN', `${EXEC}: ${run.error.message}`);
  if (run.status !== 0) refuse('BINARY_REFUSED', `${EXEC} --dump-commands exited ${run.status}\n${run.stderr || ''}`);
  try {
    return JSON.parse(run.stdout);
  } catch (error) {
    refuse('DUMP_NOT_JSON', `${EXEC} --dump-commands did not print json: ${error.message}`);
  }
}

function usage(command) {
  const positional = command.arguments.filter((item) => item.positional);
  const flags = command.arguments.filter((item) => !item.positional);
  const words = [`db ${command.name}`];
  for (const item of positional) words.push(item.value || `<${item.name}>`);
  if (flags.length > 0) words.push('[flags]');
  return words.join(' ');
}

function commandsSection(dump) {
  const lines = ['## Commands', ''];
  lines.push('Derived from the command tree by `tools/readme_sync.mjs`; edit the `#[command]` and `#[arg]');
  lines.push('help text in `crates/db/src/main.rs`, never this section.', '');
  lines.push('```');
  const width = Math.max(...dump.commands.map((command) => usage(command).length));
  for (const command of dump.commands) {
    lines.push(`${usage(command).padEnd(width)}  ${command.about || ''}`);
  }
  lines.push('```', '');
  const globals = dump.global_arguments.filter((item) => !item.positional);
  if (globals.length > 0) {
    lines.push('Every command also takes:', '');
    for (const item of globals) {
      const body = item.value ? `${item.long} ${item.value}` : item.long;
      lines.push(`- \`${body}\` — ${item.help || ''}`);
    }
    lines.push('');
  }
  for (const command of dump.commands) {
    const flags = command.arguments.filter((item) => !item.positional);
    if (flags.length === 0) continue;
    lines.push(`\`db ${command.name}\`:`, '');
    for (const item of flags) {
      const body = item.value ? `${item.long} ${item.value}` : item.long;
      lines.push(`- \`${body}\` — ${item.help || ''}`);
    }
    lines.push('');
  }
  return lines.join('\n').trimEnd() + '\n';
}

function exitSection(dump) {
  const lines = ['## Exit codes', ''];
  lines.push('Three codes, one meaning each, the same meaning in every command. Derived from');
  lines.push('`EXIT_CODES` in `crates/db/src/main.rs` by `tools/readme_sync.mjs`.', '');
  lines.push('| code | meaning | when |');
  lines.push('|---|---|---|');
  for (const item of dump.exit_codes) {
    lines.push(`| **${item.code}** | ${item.meaning} | ${item.when} |`);
  }
  lines.push('');
  lines.push('Every refusal also prints `reason: <TOKEN>` on stderr, so a caller reads a machine');
  lines.push('token and not only prose. An empty answer is **2**, never 0: a page with no rows is');
  lines.push('not an answer. `UNKNOWN` is a third verdict inside `gate`, never a fourth exit code —');
  lines.push('it is counted and printed, never folded into a failure, and a run carrying one exits 2.');
  return lines.join('\n') + '\n';
}

function replaceSection(text, heading, body) {
  const lines = text.split('\n');
  const start = lines.findIndex((line) => line.trim() === heading);
  if (start === -1) return { text, found: false };
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (lines[index].startsWith('## ')) {
      end = index;
      break;
    }
  }
  const rebuilt = [...lines.slice(0, start), ...body.split('\n'), '', ...lines.slice(end)];
  return { text: rebuilt.join('\n').replace(/\n{3,}/g, '\n\n'), found: true };
}

const dump = loadDump();
let readme;
try {
  readme = readFileSync(README, 'utf8');
} catch (error) {
  refuse('README_UNREADABLE', `${README}: ${error.message}`);
}

let next = readme;
const missing = [];
for (const [heading, body] of [
  ['## Commands', commandsSection(dump)],
  ['## Exit codes', exitSection(dump)]
]) {
  const applied = replaceSection(next, heading, body);
  if (!applied.found) missing.push(heading);
  next = applied.text;
}

if (missing.length > 0) {
  refuse(
    'HEADING_ABSENT',
    `${README} has no ${missing.join(' and no ')}; the gate refuses rather than reporting no drift over a section it never found`
  );
}

if (next === readme) {
  process.stdout.write(`readme_sync: no drift; ${dump.commands.length} command(s) and ${dump.exit_codes.length} exit code(s) agree with ${path.basename(README)}\n`);
  process.exit(0);
}

if (WRITE) {
  writeFileSync(README, next);
  process.stdout.write(`readme_sync: rewrote the two derived sections of ${path.basename(README)}\n`);
  process.exit(0);
}

const before = readme.split('\n');
const after = next.split('\n');
let shown = 0;
process.stderr.write(`readme_sync: DRIFT between the command tree and ${path.basename(README)}\n`);
for (let index = 0; index < Math.max(before.length, after.length); index += 1) {
  if (before[index] === after[index]) continue;
  if (shown >= 12) {
    process.stderr.write('  ... further differences not shown\n');
    break;
  }
  process.stderr.write(`  line ${index + 1}\n    readme: ${before[index] ?? '(absent)'}\n    code:   ${after[index] ?? '(absent)'}\n`);
  shown += 1;
}
process.stderr.write('run with --write to take the code as the source\n');
process.exit(1);
