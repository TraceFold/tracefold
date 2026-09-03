// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

function option(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  if (at === -1 || at + 1 >= process.argv.length) return fallback;
  return process.argv[at + 1];
}

const BIN = option('bin', null);
const DB = option('db', null);
const OUT = option('out', null);
const RUNNER = option('runner', 'wsl -d Ubuntu-24.04 -- bash');
const REPEAT = Number(option('repeat', '5'));

function refuse(reason, detail) {
  process.stderr.write(`bench: ${reason}\n${detail}\n`);
  process.exit(2);
}

if (!BIN || !DB) {
  refuse(
    'TARGET_ABSENT',
    'bench needs --bin <path to db inside the runner> and --db <path to a DB inside the runner>.\n' +
      'Both are paths as the runner sees them, not as Windows sees them, because the binary is built\n' +
      'and timed under WSL while node runs on Windows.'
  );
}
if (!Number.isInteger(REPEAT) || REPEAT < 3) {
  refuse('REPEAT_TOO_LOW', `--repeat ${REPEAT}; a median over fewer than three runs is one run wearing a hat`);
}

const runnerParts = RUNNER.split(' ').filter((piece) => piece.length > 0);

function inside(windowsPath) {
  const forward = String(windowsPath).split('\\').join('/');
  const asked = spawnSync(runnerParts[0], [...runnerParts.slice(1, -1), 'wslpath', '-u', forward], {
    encoding: 'utf8'
  });
  if (asked.status !== 0) {
    refuse('RUNNER_NOT_RUN', `${RUNNER} could not translate ${windowsPath}: ${asked.stderr || asked.error}`);
  }
  return asked.stdout.trim();
}

const SRC = option('src', inside(path.join(ROOT, 'crates', 'db', 'src')));

const BAND = option('band', null);
const narrow = BAND ? ['--band', BAND] : [];
const SCOPE = option('scope', 'bands/decisions');

const CASES = [
  { name: 'compile', kind: 'db', argv: ['compile'] },
  { name: 'gate', kind: 'db', argv: ['gate'] },
  { name: 'bands', kind: 'db', argv: ['bands'] },
  { name: 'ls --layer L0 --cursor begin', kind: 'db', argv: ['ls', ...narrow, '--layer', 'L0', '--cursor', 'begin'] },
  { name: 'ls --layer L1 --lod 1 --cursor begin', kind: 'db', argv: ['ls', ...narrow, '--layer', 'L1', '--lod', '1', '--cursor', 'begin'] },
  { name: 'show <anchor> --lod 2', kind: 'db', argv: ['show', option('anchor', 'Overview'), '--lod', '2'] },
  { name: 'find <needle>', kind: 'db', argv: ['find', option('needle', 'regenerable')] },
  { name: 'find <needle> --strict', kind: 'db', argv: ['--strict', 'find', option('needle', 'regenerable')] },
  { name: 'find <needle> --layer L1', kind: 'db', argv: ['find', option('needle-l1', 'lean'), '--layer', 'L1'] },
  { name: 'selftest', kind: 'db', argv: ['selftest', '--path', SRC] },
  { name: 'grep -rIn <needle> bands/', kind: 'raw', argv: [] },
  { name: `grep -rIn <needle> ${SCOPE}/`, kind: 'scoped', argv: [] }
];

function quote(value) {
  return `'${String(value).split("'").join(`'"'"'`)}'`;
}

function script() {
  const lines = [
    '#!/bin/bash',
    `BIN=${quote(BIN)}`,
    `DB=${quote(DB)}`,
    'TMP=$(mktemp -d)',
    'export TIMEFORMAT=%R',
    'measure() {',
    '  local name="$1"; shift',
    `  for run in $(seq 1 ${REPEAT}); do`,
    '    local started ended clock_a clock_b bytes rows code',
    '    started=$(date +%s%N)',
    '    "$@" > "$TMP/out" 2> "$TMP/err"',
    '    code=$?',
    '    ended=$(date +%s%N)',
    '    clock_a=$(( (ended - started) / 1000 ))',
    '    clock_b=$( { time "$@" > /dev/null 2>&1 ; } 2>&1 )',
    '    bytes=$(wc -c < "$TMP/out")',
    '    rows=$(wc -l < "$TMP/out")',
    '    echo "RESULT|$name|$code|$clock_a|$clock_b|$bytes|$rows"',
    '  done',
    '}'
  ];
  for (const item of CASES) {
    if (item.kind === 'db') {
      const argv = item.argv.map(quote).join(' ');
      lines.push(`measure ${quote(item.name)} "$BIN" --db "$DB" ${argv}`);
    } else if (item.kind === 'scoped') {
      lines.push(`measure ${quote(item.name)} grep -rIn ${quote(option('needle', 'regenerable'))} "$DB/${SCOPE}"`);
    } else {
      lines.push(`measure ${quote(item.name)} grep -rIn ${quote(option('needle', 'regenerable'))} "$DB/bands"`);
    }
  }
  lines.push('rm -rf "$TMP"');
  return lines.join('\n') + '\n';
}

const scriptPath = path.join(os.tmpdir(), `db_bench_${process.pid}.sh`);
writeFileSync(scriptPath, script());
const scriptInside = inside(scriptPath);

const run = spawnSync(runnerParts[0], [...runnerParts.slice(1), scriptInside], { encoding: 'utf8' });
if (run.error) refuse('RUNNER_NOT_RUN', `${RUNNER}: ${run.error.message}`);
const lines = String(run.stdout || '').split('\n').filter((line) => line.startsWith('RESULT|'));
if (lines.length === 0) {
  refuse(
    'NO_SAMPLE',
    `the runner produced no RESULT line, so nothing was measured; this is UNTESTABLE, never a pass.\nstdout: ${run.stdout}\nstderr: ${run.stderr}`
  );
}

const samples = new Map();
for (const line of lines) {
  const [, name, code, clockA, clockB, bytes, rows] = line.split('|');
  if (!samples.has(name)) samples.set(name, []);
  samples.get(name).push({
    exit: Number(code),
    us_date: Number(clockA),
    ms_time: Number(clockB) * 1000,
    bytes: Number(bytes),
    rows: Number(rows)
  });
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 1) return sorted[middle];
  return (sorted[middle - 1] + sorted[middle]) / 2;
}

const rows = [];
for (const item of CASES) {
  const found = samples.get(item.name);
  if (!found || found.length === 0) {
    rows.push({ command: item.name, measured: false, note: 'the runner returned no sample for this case' });
    continue;
  }
  const failed = found.filter((one) => one.exit !== 0);
  if (failed.length > 0) {
    rows.push({
      command: item.name,
      measured: false,
      note: `${failed.length} of ${found.length} run(s) exited ${failed[0].exit} and returned ${failed[0].bytes} byte; timing a command that did not answer measures how fast it fails`
    });
    continue;
  }
  const dateMs = median(found.map((one) => one.us_date / 1000));
  const timeMs = median(found.map((one) => one.ms_time));
  const spread = timeMs === 0 && dateMs === 0 ? 0 : Math.abs(dateMs - timeMs) / Math.max(dateMs, timeMs, 0.001);
  rows.push({
    command: item.name,
    measured: true,
    runs: found.length,
    exit: found[0].exit,
    ms_by_date: Number(dateMs.toFixed(2)),
    ms_by_time: Number(timeMs.toFixed(2)),
    clock_spread: Number(spread.toFixed(3)),
    bytes: median(found.map((one) => one.bytes)),
    rows: median(found.map((one) => one.rows))
  });
}

const rawRow = rows.find((one) => one.command === 'grep -rIn <needle> bands/');
const scopedRow = rows.find((one) => one.command.startsWith('grep') && one !== rawRow);
for (const row of rows) {
  if (!row.measured) continue;
  if (rawRow && rawRow.measured && rawRow.bytes > 0) {
    row.bytes_vs_grep = Number((rawRow.bytes / Math.max(row.bytes, 1)).toFixed(1));
  }
  if (scopedRow && scopedRow.measured && scopedRow.bytes > 0) {
    row.bytes_vs_scoped_grep = Number((scopedRow.bytes / Math.max(row.bytes, 1)).toFixed(1));
    row.ms_vs_scoped_grep = Number((row.ms_by_date / Math.max(scopedRow.ms_by_date, 0.001)).toFixed(2));
  }
}

const target = OUT || path.join(ROOT, 'build', 'bench.jsonl');
mkdirSync(path.dirname(target), { recursive: true });
writeFileSync(
  target,
  rows.map((row) => JSON.stringify({ ...row, repeat: REPEAT, db: DB, bin: BIN })).join('\n') + '\n'
);

const header = [
  '| command | runs | exit | ms (date) | ms (time) | clock spread | bytes | rows | bytes vs grep | bytes vs scoped grep | ms vs scoped grep |',
  '|---|---|---|---|---|---|---|---|---|---|---|'
];
const table = rows.map((row) =>
  row.measured
    ? `| \`${row.command}\` | ${row.runs} | ${row.exit} | ${row.ms_by_date} | ${row.ms_by_time} | ${row.clock_spread} | ${row.bytes} | ${row.rows} | ${row.bytes_vs_grep ?? 'n/a'} | ${row.bytes_vs_scoped_grep ?? 'n/a'} | ${row.ms_vs_scoped_grep ?? 'n/a'} |`
    : `| \`${row.command}\` | Not measured | Not measured | Not measured | Not measured | Not measured | Not measured | Not measured | Not measured | Not measured | Not measured | ${row.note} |`
);
process.stdout.write([...header, ...table].join('\n') + '\n');

const unmeasured = rows.filter((row) => !row.measured).length;
const disagreeing = rows.filter((row) => row.measured && row.clock_spread > 0.5).length;
process.stdout.write(
  `\n${rows.length - unmeasured} of ${rows.length} case(s) measured over ${REPEAT} run(s) each, two clocks per run ` +
    `(date +%s%N around the call, and the shell's own time builtin on a second call); ` +
    `${disagreeing} case(s) where the two clocks differ by more than half.\n` +
    `written to ${target}\n`
);
process.exit(unmeasured > 0 || disagreeing > 0 ? 1 : 0);
