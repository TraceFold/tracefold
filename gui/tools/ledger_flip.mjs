// SPDX-License-Identifier: Apache-2.0
//
// ledger_flip.mjs -- the exclusive writer of the verified ("[●]") state on a ledger row.
//
// SS548 (Owner #283) named the gap precisely: ledger_dash.mjs reads/counts states by machine,
// but the STATE FLIP itself ([]->[◐]->[●]) was still written by hand under discipline, with no
// machine guard beyond the pointer-text lint. SS551(b) points at Studio mechanism 5 (verified
// state is writable ONLY by the tool that verified it, on exit 0, never by hand) as the answer.
// This file is that tool for our own ledgers: it runs a named verification command, and ONLY
// when that command exits 0 does it rewrite the row's state cell to `[●]` and append a receipt
// line naming the command, its exit code, and a timestamp -- so a future reader (or
// ledger_dash.mjs's receipt-WARN check, added alongside this file) can tell a machine-verified
// `[●]` apart from a hand-typed one without re-running anything.
//
// A failing verification command must NEVER flip the row -- the file is not touched at all on a
// non-zero exit. This is the negative control the SS558 dispatch asked for by name.
//
// usage:
//   node tools/ledger_flip.mjs --ledger <id> --line <N>        --cmd "<verification command>"
//   node tools/ledger_flip.mjs --ledger <id> --row "<row id>"  --cmd "<verification command>"
//   node tools/ledger_flip.mjs --file <path> --line <N> --cmd "<cmd>"                (direct file, bypasses LEDGER_DEFS -- used by tests and ad-hoc ledgers)
//   node tools/ledger_flip.mjs --ledger <id> --line <N> --cmd "<cmd>" --cwd <dir>    (cwd for the command; default process.cwd())
//   node tools/ledger_flip.mjs --ledger <id> --line <N> --cmd "<cmd>" --force        (allow flipping a `[退役●]` row -- refused by default)
//
// `--ledger <id>` must be one of LEDGER_DEFS' ids (see ledger_dash.mjs; run
// `node tools/ledger_dash.mjs --no-html` to see the current id/label list); `--file <path>` names
// a markdown file directly instead. Exactly one of the two is required. `--row` matches the
// row's first cell text exactly (after trim); if more than one row shares that text, the tool
// refuses to guess and asks for `--line` instead. The verification command runs via the shell
// (`spawnSync(cmd, { shell: true })`), inheriting stdio, so its own output is visible live.
//
// exit codes: 0 flipped · 64 usage error · 3 ledger/row not found · 4 ambiguous row ·
//             5 row is already `[退役●]` (retired) and --force was not given ·
//             otherwise: the verification command's own exit code, propagated unchanged.
//
// Zero runtime dependencies beyond node:fs / node:path / node:child_process and this repo's own
// ledger_dash.mjs (single source for row parsing -- SS552 flagged duplicated ledger-parsing logic
// once already; this file does not re-implement it a second time).

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { LEDGER_DEFS, loadLedger, locateStateCell, parseReceiptLine } from './ledger_dash.mjs';

// Table-driven, not an if/else chain walked by a hand-incremented counter: each
// recognized flag maps to a setter that receives the accumulator and the token
// immediately following the flag, so adding a flag never means adding another
// `else if` limb.
const FLAG_SETTERS = {
  '--ledger': (acc, value) => { acc.ledger = value; },
  '--file': (acc, value) => { acc.file = value; },
  '--row': (acc, value) => { acc.row = value; },
  '--line': (acc, value) => { acc.line = Number(value); },
  '--cmd': (acc, value) => { acc.cmd = value; },
  '--cwd': (acc, value) => { acc.cwd = value; },
};

function parseArgs(argv) {
  const out = { ledger: null, file: null, row: null, line: null, cmd: null, cwd: process.cwd(), force: false };
  let cursor = 0;
  while (cursor < argv.length) {
    const flag = argv[cursor];
    if (flag === '--force') {
      out.force = true;
      cursor += 1;
      continue;
    }
    const setter = FLAG_SETTERS[flag];
    if (!setter) {
      return { error: `unrecognized argument: ${flag}` };
    }
    setter(out, argv[cursor + 1]);
    cursor += 2;
  }
  if (!out.ledger && !out.file) return { error: 'exactly one of --ledger <id> or --file <path> is required' };
  if (out.ledger && out.file) return { error: '--ledger and --file are mutually exclusive (pick one)' };
  if (!out.cmd) return { error: 'missing required --cmd "<verification command>"' };
  if (out.row === null && out.line === null) return { error: 'one of --row <text> or --line <N> is required' };
  if (out.row !== null && out.line !== null) return { error: '--row and --line are mutually exclusive (pick one)' };
  if (out.line !== null && (!Number.isInteger(out.line) || out.line < 1)) return { error: '--line must be a positive integer' };
  return { args: out };
}

function findByLine(lines, lineNo) {
  if (lineNo < 1 || lineNo > lines.length) {
    return { ok: false, reason: `line ${lineNo} is out of range (file has ${lines.length} lines)` };
  }
  const loc = locateStateCell(lines[lineNo - 1]);
  if (!loc.ok) return { ok: false, reason: `line ${lineNo} is not a state-bearing ledger row: ${loc.reason}` };
  return { ok: true, lineNo, ...loc };
}

function findByRow(lines, rowText) {
  const hits = [];
  lines.forEach((line, idx) => {
    const loc = locateStateCell(line);
    if (loc.ok && loc.rowId === rowText) hits.push({ lineNo: idx + 1, ...loc });
  });
  if (hits.length === 0) return { ok: false, reason: `no state-bearing row with row id exactly "${rowText}"` };
  if (hits.length > 1) {
    const at = hits.map((h) => h.lineNo).join(', ');
    return { ok: false, reason: `ambiguous: ${hits.length} rows share row id "${rowText}" (lines ${at}) -- use --line to disambiguate` };
  }
  return { ok: true, ...hits[0] };
}

function main() {
  const parsed = parseArgs(process.argv.slice(2));
  if (parsed.error) {
    console.error(`ledger_flip: usage error: ${parsed.error}`);
    process.exit(64);
  }
  const { ledger: ledgerId, file: filePath, row, line, cmd, cwd, force } = parsed.args;

  let info;
  if (filePath) {
    const resolved = path.resolve(filePath);
    if (!existsSync(resolved)) {
      console.error(`ledger_flip: --file "${filePath}" not found (resolved: ${resolved})`);
      process.exit(3);
    }
    info = { status: 'OK', file: resolved, label: path.basename(resolved) };
  } else {
    const def = LEDGER_DEFS.find((d) => d.id === ledgerId);
    if (!def) {
      const ids = LEDGER_DEFS.map((d) => d.id).join(', ');
      console.error(`ledger_flip: unknown --ledger "${ledgerId}". Known ids: ${ids}`);
      process.exit(3);
    }
    info = loadLedger(def);
    if (info.status !== 'OK') {
      console.error(`ledger_flip: ledger "${ledgerId}" could not be resolved: ${info.reason}`);
      process.exit(3);
    }
  }

  const raw = readFileSync(info.file, 'utf8');
  const eol = raw.includes('\r\n') ? '\r\n' : '\n';
  const lines = raw.split(/\r\n|\n/);

  const located = line !== null ? findByLine(lines, line) : findByRow(lines, row);
  if (!located.ok) {
    console.error(`ledger_flip: ${located.reason}`);
    process.exit(located.reason.startsWith('ambiguous') ? 4 : 3);
  }

  if (located.currentInner === '退役●' && !force) {
    console.error(`ledger_flip: line ${located.lineNo} is already retired ("[退役●]"); refusing to overwrite without --force.`);
    process.exit(5);
  }

  console.log(`ledger_flip: verifying "${info.label}" line ${located.lineNo} (row "${located.rowId}") via: ${cmd}`);
  const result = spawnSync(cmd, { shell: true, stdio: 'inherit', cwd });
  const status = result.status === null ? (result.signal ? 128 : 1) : result.status;

  if (status !== 0) {
    console.error(`ledger_flip: verification FAILED (exit ${status}, signal ${result.signal ?? 'none'}) -- row NOT flipped, file NOT written.`);
    process.exit(status === 0 ? 1 : status);
  }

  const ts = new Date().toISOString();
  const receiptLine = `<!-- ledger_flip receipt: row_line=${located.lineNo} cmd="${cmd.replace(/"/g, '\\"')}" exit=0 ts=${ts} -->`;

  const rawLine = lines[located.lineNo - 1];
  const newLine = rawLine.slice(0, located.bracketStart) + '[●]' + rawLine.slice(located.bracketEnd);
  lines[located.lineNo - 1] = newLine;

  // Refresh, not stack: if a receipt already sits on the line beneath (a re-run of a previously
  // flipped row), replace it in place rather than growing a chain of stale receipts.
  const nextIdx = located.lineNo; // 0-indexed position of the (lineNo+1)-th line
  const existingReceipt = nextIdx < lines.length ? parseReceiptLine(lines[nextIdx]) : null;
  if (existingReceipt && existingReceipt.rowLine === located.lineNo) {
    lines[nextIdx] = receiptLine;
  } else {
    lines.splice(nextIdx, 0, receiptLine);
  }

  writeFileSync(info.file, lines.join(eol), 'utf8');
  console.log(`ledger_flip: verification PASSED -- line ${located.lineNo} flipped to [●], receipt written on line ${located.lineNo + 1}.`);
  process.exit(0);
}

main();
