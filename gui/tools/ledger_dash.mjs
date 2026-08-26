// SPDX-License-Identifier: Apache-2.0
//
// ledger_dash.mjs — deterministic state census across the Glovrex ledgers.
//
// Owner #257 "[] is supposed to be mechanized/deterministic already" + #258
// "build an absorption-status HTML from a machine count" + #259 "single
// dashboard, one section per ledger": this file replaces hand-counting of
// `[]` / `[◐]` / `[●]` / `[退役●]` rows with a parser that walks every known
// ledger, counts states by machine, and refuses to hide anything it could
// not classify. A `[●]` row with no verification pointer (§/lane/一次) in
// its own row text is a lint failure, not a silent pass -- state transition
// to "done" without a traceable reason is exactly the thing #257 wants
// caught mechanically instead of by eye.
//
// Fail-open is banned here on purpose: every table row that carries a
// short bracket cell and does not match one of the four canonical tokens
// is counted and shown (never dropped), and every configured ledger that
// cannot be found on disk is reported as MISSING rather than skipped.
//
//   node tools/ledger_dash.mjs            print the report, write the HTML
//   node tools/ledger_dash.mjs --no-html  print the report only
//
// Zero runtime dependencies (node:fs / node:path / node:crypto / node:url only).

import { readFileSync, existsSync, readdirSync, mkdirSync, writeFileSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const APP_ROOT = path.resolve(HERE, '..');
export const BUILD_ROOT = path.resolve(APP_ROOT, '..');
export const WEB_ROOT = path.join(BUILD_ROOT, 'glovrex_web');

// -- canonical state tokens ------------------------------------------------
// Inner content of a `[...]` cell, after trimming whitespace, matched against
// these exact strings. Anything else that still looks like a short bracket
// cell (<=10 chars inside) is UNRECOGNIZED, never silently absorbed into one
// of the four.
export const STATES = {
  EMPTY: { key: 'EMPTY', token: '[]', label: '未着手' },
  HALF: { key: 'HALF', token: '[◐]', label: '一次読了/起草' },
  DONE: { key: 'DONE', token: '[●]', label: '独立再走確定' },
  RETIRED_DONE: { key: 'RETIRED_DONE', token: '[退役●]', label: '退役確定' },
};

function classify(inner) {
  if (inner === '') return 'EMPTY';
  if (inner === '◐') return 'HALF';
  if (inner === '●') return 'DONE';
  if (inner === '退役●') return 'RETIRED_DONE';
  return null;
}

// -- table-row parsing ------------------------------------------------------
// A "row" is any line of the form `| cell | cell | ... |`. Separator rows
// (`|---|:---:|` etc.) are skipped. Within a data row, every cell is tested
// for a leading short bracket (`^\[(0-10 chars)\]`); a row with exactly one
// such cell has its state read off that cell. Zero matches means the row is
// not a state-bearing ledger row (e.g. a plain data table) and is silently
// not counted -- that is not fail-open, because nothing there claimed a
// state to begin with. More than one match is ambiguous and is counted as
// parse-unable rather than guessed at.
export const ROW_RE = /^\s*\|(.*)\|\s*$/;
export const SEP_RE = /^[\s:\-|]+$/;
export const BRACKET_CELL_RE = /^\[([^\]]{0,10})\]/;
export const POINTER_RE = /§|lane|一次/i;

// -- ledger_flip receipt detection (additive, SS548/SS558 verified-flip mechanization) --------
// tools/ledger_flip.mjs is the ONLY writer allowed to produce this line: it appends it as the
// line immediately following a row it just flipped to the DONE ("●") state, after the row's own
// verification command exited 0. The receipt is deliberately NOT a table row (does not start
// with `|`), so ROW_RE never picks it up as ledger data -- it lives beside the row, never inside it.
//   <!-- ledger_flip receipt: row_line=<N> cmd="<verification command>" exit=0 ts=<ISO8601> -->
// `row_line` must equal the 1-based line number of the verified row itself, so a receipt can only
// ever attest to the one row directly above it -- copy-pasting a receipt to a different row does
// not silently validate that row too.
// `cmd="..."` uses a backslash-escaped-quote grammar ((?:[^"\\]|\\.)*), not a bare [^"]* -- a
// verification command containing its own double quotes (e.g. `node -e "process.exit(0)"`,
// exactly the shape ledger_flip.mjs's own tests run) would otherwise truncate the match at the
// first embedded quote and silently fail to recognize a perfectly good receipt as one.
export const RECEIPT_RE = /^<!--\s*ledger_flip receipt:\s*row_line=(\d+)\s+cmd="((?:[^"\\]|\\.)*)"\s+exit=(\d+)\s+ts=(\S+)\s*-->\s*$/;

export function parseReceiptLine(line) {
  const m = RECEIPT_RE.exec(line);
  if (!m) return null;
  return { rowLine: Number(m[1]), cmd: m[2], exit: Number(m[3]), ts: m[4] };
}

// Locates the single state-bearing bracket cell on ONE raw (untrimmed) line, returning its exact
// character span so a writer (ledger_flip.mjs) can replace it without disturbing anything else on
// the line. Mirrors the same candidate-selection rule parseLedgerText uses per row (exactly one
// leading-bracket cell = state cell; 0 = not a state row; >1 = ambiguous, refuse to guess).
export function locateStateCell(rawLine) {
  const m = ROW_RE.exec(rawLine);
  if (!m) return { ok: false, reason: 'not a table row' };
  const inner = m[1];
  if (SEP_RE.test(inner)) return { ok: false, reason: 'separator row' };
  const firstPipe = rawLine.indexOf('|');
  const rawCells = inner.split('|');
  const candidates = [];
  let offset = firstPipe + 1;
  for (const raw of rawCells) {
    const trimmed = raw.trim();
    const bm = BRACKET_CELL_RE.exec(trimmed);
    if (bm) {
      const leadingWs = raw.length - raw.trimStart().length;
      const bracketStart = offset + leadingWs;
      const bracketEnd = bracketStart + 1 + bm[1].length + 1; // `[` + inner + `]`
      candidates.push({ inner: bm[1].trim(), bracketStart, bracketEnd });
    }
    offset += raw.length + 1; // +1 for the '|' separator consumed by split
  }
  const rowId = (rawCells[0] || '').trim();
  if (candidates.length === 0) return { ok: false, reason: 'no state-bearing cell on this row' };
  if (candidates.length > 1) return { ok: false, reason: `ambiguous: ${candidates.length} bracket cells`, rowId };
  const c = candidates[0];
  return { ok: true, rowId, currentInner: c.inner, bracketStart: c.bracketStart, bracketEnd: c.bracketEnd };
}

// Grandfather snapshot (SS558 task 1: "do NOT mass-migrate existing verified rows -- flag them
// as legacy-verified, grandfathered, listed"). A DONE row with no receipt that matches a
// (ledgerId, lineNo, rowId) triple in this file predates the flip-tool mechanism and is reported
// as legacy debt, not a fresh violation -- but it still counts toward the WARN total (a
// receiptless [●] is a receiptless [●] either way; the grandfather list only changes how it
// is labeled, never whether it is counted).
const GRANDFATHER_FILE = path.join(HERE, 'ledger_flip_grandfather.json');
function loadGrandfather() {
  if (!existsSync(GRANDFATHER_FILE)) return new Set();
  try {
    const rows = JSON.parse(readFileSync(GRANDFATHER_FILE, 'utf8'));
    return new Set(rows.map((r) => `${r.ledgerId}::${r.lineNo}::${r.rowId}`));
  } catch {
    return new Set();
  }
}
const GRANDFATHER = loadGrandfather();

export function parseLedgerText(text, { sourceLabel = '(text)', ledgerId = null } = {}) {
  const lines = text.split(/\r\n|\n/);
  const counts = { EMPTY: 0, HALF: 0, DONE: 0, RETIRED_DONE: 0 };
  const unrecognized = []; // { lineNo, token, raw }
  const ambiguous = []; // { lineNo, n, raw }
  const doneWarnings = []; // { lineNo, rowId, raw } -- [●] row with no §/lane/一次 pointer
  const receiptWarnings = []; // { lineNo, rowId, raw, legacy } -- [●] row with no ledger_flip receipt
  const trackedRows = []; // { lineNo, state, rowId, cells }
  const surfacedTokens = {}; // { rawToken: count } for unrecognized, for the report

  lines.forEach((line, idx) => {
    const lineNo = idx + 1;
    const m = ROW_RE.exec(line);
    if (!m) return;
    const inner = m[1];
    if (SEP_RE.test(inner)) return;
    const cells = inner.split('|').map((c) => c.trim());
    const candidates = [];
    cells.forEach((cell, cellIdx) => {
      const bm = BRACKET_CELL_RE.exec(cell);
      if (bm) candidates.push({ cellIdx, inner: bm[1].trim() });
    });
    if (candidates.length === 0) return;
    if (candidates.length > 1) {
      ambiguous.push({ lineNo, n: candidates.length, raw: line.trim().slice(0, 200) });
      return;
    }
    const cand = candidates[0];
    const state = classify(cand.inner);
    if (!state) {
      unrecognized.push({ lineNo, token: cand.inner, raw: line.trim().slice(0, 200) });
      surfacedTokens[cand.inner] = (surfacedTokens[cand.inner] || 0) + 1;
      return;
    }
    counts[state] += 1;
    const rowId = cells[0] || '';
    trackedRows.push({ lineNo, state, rowId, cells });
    if (state === 'DONE') {
      const rowText = cells.join(' | ');
      if (!POINTER_RE.test(rowText)) {
        doneWarnings.push({ lineNo, rowId: rowId.slice(0, 60), raw: line.trim().slice(0, 200) });
      }
      // SS548/SS558 [●]-flip mechanization: a verified row must carry a tool receipt on the
      // line directly beneath it (written by ledger_flip.mjs on the run whose exit code was 0).
      // No receipt there = a hand-written [●], detectable and never silently accepted.
      const nextLine = lines[lineNo]; // lines[lineNo] is 0-indexed line (lineNo+1)-th = the next line
      const receipt = nextLine !== undefined ? parseReceiptLine(nextLine) : null;
      const hasReceipt = !!(receipt && receipt.rowLine === lineNo);
      if (!hasReceipt) {
        const legacy = ledgerId !== null && GRANDFATHER.has(`${ledgerId}::${lineNo}::${rowId}`);
        receiptWarnings.push({ lineNo, rowId: rowId.slice(0, 60), raw: line.trim().slice(0, 200), legacy });
      }
    }
  });

  const chogaMentions = trackedRows.filter((r) => r.cells.join('').includes('凌駕')).length;

  return {
    sourceLabel,
    counts,
    unrecognized,
    ambiguous,
    surfacedTokens,
    parseUnableCount: unrecognized.length + ambiguous.length,
    doneWarnings,
    receiptWarnings,
    trackedRows,
    totalTracked: trackedRows.length,
    chogaMentions,
  };
}

// -- file resolution ----------------------------------------------------
// A ledger can be named by an exact path, or (for req/92, not yet created
// at authoring time) by a directory + numeric prefix, resolved at run time
// so the tool does not silently keep pointing at a guessed filename that
// was never real.
function resolveByPrefix(dir, prefix) {
  if (!existsSync(dir)) return { status: 'MISSING', reason: `dir not found: ${dir}` };
  const hits = readdirSync(dir).filter((f) => f.startsWith(prefix) && f.toLowerCase().endsWith('.md'));
  if (hits.length === 0) return { status: 'MISSING', reason: `no ${prefix}*.md under ${dir}` };
  if (hits.length > 1) return { status: 'AMBIGUOUS', reason: `${hits.length} candidates: ${hits.join(', ')}` };
  return { status: 'OK', file: path.join(dir, hits[0]) };
}

export const LEDGER_DEFS = [
  { id: '90_retirement', label: '90 退役台帳(app)', kind: 'exact', file: path.join(APP_ROOT, 'req', '90_RETIREMENT_LEDGER.md') },
  { id: '07_oss', label: '07 OSS台帳(app)', kind: 'exact', file: path.join(APP_ROOT, 'req', '07_OSS_LEDGER.md') },
  { id: '92_copycheck', label: '92 copycheck台帳(app)', kind: 'prefix', dir: path.join(APP_ROOT, 'req'), prefix: '92_' },
  { id: '01_membrane', label: '01 膜 AC(app)', kind: 'exact', file: path.join(APP_ROOT, 'req', '01_MEMBRANE.md') },
  { id: '02_shell', label: '02 器 AC(app)', kind: 'exact', file: path.join(APP_ROOT, 'req', '02_SHELL_WLAYER.md') },
  { id: '03_faces', label: '03 面 AC(app)', kind: 'exact', file: path.join(APP_ROOT, 'req', '03_FACES_REBUILD.md') },
  { id: '04_parts', label: '04 部品 AC(app)', kind: 'exact', file: path.join(APP_ROOT, 'req', '04_PARTS_REBUILD.md') },
  { id: '05_reftruth', label: '05 参照真値(app)', kind: 'exact', file: path.join(APP_ROOT, 'req', '05_REFERENCE_TRUTH.md') },
  { id: '06_instruments', label: '06 計器 AC(app)', kind: 'exact', file: path.join(APP_ROOT, 'req', '06_INSTRUMENTS.md') },
  { id: '28_studio', label: '28 Studio吸収(web)', kind: 'exact', file: path.join(WEB_ROOT, 'req', '28_STUDIO_SKELETON_ABSORPTION_2026-08-23.md') },
];

function sha256(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

export function loadLedger(def) {
  let resolved;
  if (def.kind === 'exact') {
    resolved = existsSync(def.file) ? { status: 'OK', file: def.file } : { status: 'MISSING', reason: `not found: ${def.file}` };
  } else {
    resolved = resolveByPrefix(def.dir, def.prefix);
  }
  if (resolved.status !== 'OK') {
    return { id: def.id, label: def.label, status: resolved.status, reason: resolved.reason, file: null };
  }
  const raw = readFileSync(resolved.file, 'utf8');
  const stat = statSync(resolved.file);
  const parsed = parseLedgerText(raw, { sourceLabel: def.label, ledgerId: def.id });
  return {
    id: def.id,
    label: def.label,
    status: 'OK',
    file: resolved.file,
    relFile: path.relative(BUILD_ROOT, resolved.file).replace(/\\/g, '/'),
    hash: sha256(raw),
    mtime: stat.mtime.toISOString(),
    bytes: raw.length,
    ...parsed,
  };
}

// -- req/08 semantics probe (optional section) ------------------------------
function probeSemantics() {
  const dir = path.join(APP_ROOT, 'req');
  const hits = existsSync(dir) ? readdirSync(dir).filter((f) => f.startsWith('08')) : [];
  if (hits.length === 0) {
    return { found: false, note: `req/08* glob checked under ${path.relative(BUILD_ROOT, dir).replace(/\\/g, '/')}: 0 matches` };
  }
  const file = path.join(dir, hits[0]);
  const raw = readFileSync(file, 'utf8');
  const parsed = parseLedgerText(raw, { sourceLabel: hits[0] });
  return { found: true, file: hits[0], hash: sha256(raw), ...parsed };
}

export function buildReport() {
  const ledgers = LEDGER_DEFS.map(loadLedger);
  const semantics = probeSemantics();
  const totals = {
    EMPTY: 0, HALF: 0, DONE: 0, RETIRED_DONE: 0, parseUnable: 0, warn: 0, choga: 0,
    receiptWarn: 0, receiptWarnLegacy: 0, receiptWarnNew: 0,
  };
  for (const l of ledgers) {
    if (l.status !== 'OK') continue;
    totals.EMPTY += l.counts.EMPTY;
    totals.HALF += l.counts.HALF;
    totals.DONE += l.counts.DONE;
    totals.RETIRED_DONE += l.counts.RETIRED_DONE;
    totals.parseUnable += l.parseUnableCount;
    totals.warn += l.doneWarnings.length;
    totals.choga += l.chogaMentions;
    totals.receiptWarn += l.receiptWarnings.length;
    totals.receiptWarnLegacy += l.receiptWarnings.filter((w) => w.legacy).length;
    totals.receiptWarnNew += l.receiptWarnings.filter((w) => !w.legacy).length;
  }
  return { generatedAt: new Date().toISOString(), ledgers, semantics, totals };
}

// -- console report -----------------------------------------------------
function printReport(report) {
  console.log(`ledger_dash — generated ${report.generatedAt}`);
  console.log('');
  const header = ['ledger', '[]', '[◐]', '[●]', '[退役●]', 'parse不能', 'WARN', '受領なし(●)', '凌駕言及'];
  console.log(header.join('\t'));
  for (const l of report.ledgers) {
    if (l.status !== 'OK') {
      console.log(`${l.label}\tMISSING (${l.reason})`);
      continue;
    }
    console.log([
      l.label, l.counts.EMPTY, l.counts.HALF, l.counts.DONE, l.counts.RETIRED_DONE,
      l.parseUnableCount, l.doneWarnings.length, l.receiptWarnings.length, l.chogaMentions,
    ].join('\t'));
  }
  console.log('');
  console.log(`TOTAL\t${report.totals.EMPTY}\t${report.totals.HALF}\t${report.totals.DONE}\t${report.totals.RETIRED_DONE}\t${report.totals.parseUnable}\t${report.totals.warn}\t${report.totals.receiptWarn}\t${report.totals.choga}`);
  console.log('');
  console.log(`WARN数(合計・pointer欠落) = ${report.totals.warn}`);
  console.log(`WARN数(合計・受領書なし[●]) = ${report.totals.receiptWarn}  (legacy/grandfathered ${report.totals.receiptWarnLegacy} / new ${report.totals.receiptWarnNew})`);
  console.log(`parse不能行数(合計) = ${report.totals.parseUnable}`);
  if (report.totals.warn > 0) {
    console.log('');
    console.log('WARN detail ([●] row without §/lane/一次 pointer in its own row text):');
    for (const l of report.ledgers) {
      if (l.status !== 'OK') continue;
      for (const w of l.doneWarnings) console.log(`  ${l.label} L${w.lineNo} ${w.rowId}: ${w.raw}`);
    }
  }
  if (report.totals.receiptWarn > 0) {
    console.log('');
    console.log('WARN detail ([●] row without a tools/ledger_flip.mjs receipt on the line beneath it):');
    for (const l of report.ledgers) {
      if (l.status !== 'OK') continue;
      for (const w of l.receiptWarnings) {
        console.log(`  ${l.label} L${w.lineNo} ${w.legacy ? '[legacy-verified/grandfathered]' : '[NEW, not grandfathered]'} ${w.rowId}: ${w.raw}`);
      }
    }
  }
  if (report.totals.parseUnable > 0) {
    console.log('');
    console.log('parse-unable detail (bracket cell present, not one of the 4 canonical states, or ambiguous):');
    for (const l of report.ledgers) {
      if (l.status !== 'OK') continue;
      for (const u of l.unrecognized) console.log(`  ${l.label} L${u.lineNo} token="${u.token}": ${u.raw}`);
      for (const a of l.ambiguous) console.log(`  ${l.label} L${a.lineNo} ambiguous(${a.n} candidates): ${a.raw}`);
    }
  }
  console.log('');
  if (report.semantics.found) {
    console.log(`意味論節: req/${report.semantics.file} found (${report.semantics.totalTracked} tracked rows).`);
  } else {
    console.log(`意味論節: ${report.semantics.note} — section omitted, not fabricated.`);
  }
}

// -- HTML rendering -----------------------------------------------------
function esc(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function renderLedgerSection(l) {
  if (l.status !== 'OK') {
    return `<section class="ledger missing">
  <h2>${esc(l.label)}</h2>
  <p class="badge badge-missing">MISSING</p>
  <p class="note">${esc(l.reason)}</p>
</section>`;
  }
  const rows = [
    ['<code>[]</code>', STATES.EMPTY.label, l.counts.EMPTY],
    ['<code>[◐]</code>', STATES.HALF.label, l.counts.HALF],
    ['<code>[●]</code>', STATES.DONE.label, l.counts.DONE],
    ['<code>[退役●]</code>', STATES.RETIRED_DONE.label, l.counts.RETIRED_DONE],
  ].map(([tok, label, n]) => `<tr><td class="mono">${tok}</td><td>${esc(label)}</td><td class="num">${n}</td></tr>`).join('\n');

  const warnRows = l.doneWarnings.length
    ? l.doneWarnings.map((w) => `<tr><td class="num">${w.lineNo}</td><td class="mono">${esc(w.rowId)}</td><td class="mono small">${esc(w.raw)}</td></tr>`).join('\n')
    : '<tr><td colspan="3" class="note">0件</td></tr>';

  const receiptWarnRows = l.receiptWarnings.length
    ? l.receiptWarnings.map((w) => `<tr><td class="num">${w.lineNo}</td><td class="mono">${w.legacy ? 'legacy' : 'NEW'}</td><td class="mono">${esc(w.rowId)}</td><td class="mono small">${esc(w.raw)}</td></tr>`).join('\n')
    : '<tr><td colspan="4" class="note">0件</td></tr>';

  const unrecRows = (l.unrecognized.length + l.ambiguous.length)
    ? [
      ...l.unrecognized.map((u) => `<tr><td class="num">${u.lineNo}</td><td class="mono">token="${esc(u.token)}"</td><td class="mono small">${esc(u.raw)}</td></tr>`),
      ...l.ambiguous.map((a) => `<tr><td class="num">${a.lineNo}</td><td class="mono">ambiguous(${a.n})</td><td class="mono small">${esc(a.raw)}</td></tr>`),
    ].join('\n')
    : '<tr><td colspan="3" class="note">0件</td></tr>';

  return `<section class="ledger">
  <h2>${esc(l.label)}</h2>
  <p class="meta">source: <code>${esc(l.relFile)}</code> &middot; sha256 <code class="small">${l.hash}</code> &middot; mtime ${esc(l.mtime)} &middot; ${l.bytes} bytes</p>
  <table class="counts">
    <thead><tr><th>台帳記法</th><th>意味</th><th>count</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>
  <p class="meta">tracked行 ${l.totalTracked} &middot; parse不能 ${l.parseUnableCount} &middot; WARN(pointer) ${l.doneWarnings.length} &middot; WARN(receipt) ${l.receiptWarnings.length} &middot; 凌駕言及行 ${l.chogaMentions}</p>
  <details ${l.doneWarnings.length ? 'open' : ''}>
    <summary>WARN: <code>[●]</code>行に検証pointer(§/lane/一次)なし (${l.doneWarnings.length})</summary>
    <table class="detail"><thead><tr><th>行</th><th>row id</th><th>原文</th></tr></thead><tbody>${warnRows}</tbody></table>
  </details>
  <details ${l.receiptWarnings.length ? 'open' : ''}>
    <summary>WARN: <code>[●]</code>行に <code>tools/ledger_flip.mjs</code> 受領書なし (${l.receiptWarnings.length}, うちlegacy ${l.receiptWarnings.filter((w) => w.legacy).length})</summary>
    <table class="detail"><thead><tr><th>行</th><th>種別</th><th>row id</th><th>原文</th></tr></thead><tbody>${receiptWarnRows}</tbody></table>
  </details>
  <details ${l.parseUnableCount ? 'open' : ''}>
    <summary>parse不能行 (${l.parseUnableCount})</summary>
    <table class="detail"><thead><tr><th>行</th><th>token</th><th>原文</th></tr></thead><tbody>${unrecRows}</tbody></table>
  </details>
</section>`;
}

function renderSemanticsSection(sem) {
  if (!sem.found) {
    return `<section class="ledger">
  <h2>意味論節(req/08)</h2>
  <p class="note">${esc(sem.note)} — 省略(捏造せず不在を明記)。</p>
</section>`;
  }
  return `<section class="ledger">
  <h2>意味論節(req/08)</h2>
  <p class="meta">source: <code>req/${esc(sem.file)}</code> &middot; sha256 <code class="small">${sem.hash}</code></p>
  <p class="meta">tracked行 ${sem.totalTracked} &middot; [] ${sem.counts.EMPTY} / [◐] ${sem.counts.HALF} / [●] ${sem.counts.DONE} / [退役●] ${sem.counts.RETIRED_DONE}</p>
</section>`;
}

export function renderHTML(report) {
  const t = report.totals;
  const sections = report.ledgers.map(renderLedgerSection).join('\n');
  const semantics = renderSemanticsSection(report.semantics);
  return `<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Glovrex 吸収状況 ABSORPTION_STATUS</title>
<style>
:root{ color-scheme: light;
  --ink:#0B1220; --ink2:#33405A; --muted:#6B7894; --faint:#94A0B4;
  --line:rgba(11,18,32,.12); --bg:#F8FAFC; --paper:#FFFFFF;
  --indigo:#1F4FD8; --teal:#0B6B53; --amber:#9A5406; --crim:#A32B44;
}
*{box-sizing:border-box}
body{margin:0; background:var(--bg); color:var(--ink);
  font-family:-apple-system,BlinkMacSystemFont,"Hiragino Sans","Yu Gothic","Noto Sans JP",Meiryo,"Segoe UI",system-ui,sans-serif;
  font-size:15px; line-height:1.7;}
.mono, code, .small{font-family:ui-monospace,"SF Mono",Menlo,Consolas,monospace}
code{background:rgba(11,18,32,.05); padding:1px 5px; border-radius:4px; font-size:.9em}
.small{font-size:11px}
.wrap{max-width:1080px; margin:0 auto; padding:36px 24px 100px}
header h1{font-size:26px; margin:0 0 6px; letter-spacing:-.01em}
header p{color:var(--muted); margin:0 0 4px; font-size:13px}
.gen-meta{font-size:12px; color:var(--faint); margin-bottom:28px}
.totals{display:grid; grid-template-columns:repeat(auto-fit,minmax(140px,1fr)); gap:10px; margin:20px 0 36px}
.tile{background:var(--paper); border:1px solid var(--line); border-radius:12px; padding:14px 16px}
.tile .n{font-size:26px; font-weight:800}
.tile .l{font-size:12px; color:var(--muted); margin-top:2px}
section.ledger{background:var(--paper); border:1px solid var(--line); border-radius:14px; padding:22px 24px; margin-bottom:20px}
section.ledger h2{margin:0 0 8px; font-size:17px}
.meta{font-size:12.5px; color:var(--muted); margin:4px 0}
.note{font-size:13px; color:var(--amber)}
table.counts{width:100%; border-collapse:collapse; margin:12px 0}
table.counts td, table.counts th{border-bottom:1px solid var(--line); padding:6px 8px; text-align:left; font-size:13.5px}
table.counts td.num{text-align:right; font-variant-numeric:tabular-nums; font-weight:700}
table.detail{width:100%; border-collapse:collapse; margin-top:8px}
table.detail td, table.detail th{border-bottom:1px solid var(--line); padding:5px 7px; font-size:12px; vertical-align:top}
table.detail td.num{white-space:nowrap; color:var(--muted)}
details{margin-top:10px}
details summary{cursor:pointer; font-size:13px; font-weight:650; color:var(--ink2)}
.badge{display:inline-block; font-size:12px; font-weight:700; padding:3px 10px; border-radius:999px}
.badge-missing{background:#FCEAEC; color:var(--crim)}
footer{color:var(--faint); font-size:11.5px; margin-top:30px}
</style>
</head>
<body>
<div class="wrap">
<header>
  <h1>Glovrex 吸収状況ダッシュボード</h1>
  <p>台帳群の <code>[]</code> / <code>[◐]</code> / <code>[●]</code> / <code>[退役●]</code> を <code>tools/ledger_dash.mjs</code> が機械countした結果(手書き更新禁)。</p>
</header>
<p class="gen-meta">生成時刻(UTC): ${esc(report.generatedAt)}</p>

<div class="totals">
  <div class="tile"><div class="n">${t.EMPTY}</div><div class="l"><code>[]</code> 未着手(全台帳合計)</div></div>
  <div class="tile"><div class="n">${t.HALF}</div><div class="l"><code>[◐]</code> 一次読了/起草</div></div>
  <div class="tile"><div class="n">${t.DONE}</div><div class="l"><code>[●]</code> 独立再走確定</div></div>
  <div class="tile"><div class="n">${t.RETIRED_DONE}</div><div class="l"><code>[退役●]</code> 退役確定</div></div>
  <div class="tile"><div class="n">${t.parseUnable}</div><div class="l">parse不能(合計)</div></div>
  <div class="tile"><div class="n">${t.warn}</div><div class="l">WARN(pointer欠落合計)</div></div>
  <div class="tile"><div class="n">${t.receiptWarn}</div><div class="l">WARN(受領書なし[●]合計・legacy ${t.receiptWarnLegacy} / new ${t.receiptWarnNew})</div></div>
  <div class="tile"><div class="n">${t.choga}</div><div class="l">凌駕言及行(合計・keyword count)</div></div>
</div>

${sections}
${semantics}

<footer>
  <p>算出方法: 各台帳のmarkdown table行を走査し、短い bracket cell(&le;10文字)を1個だけ持つ行の状態を分類。4トークンいずれにも一致しない/複数候補で曖昧な行は「parse不能」として個別に列挙(silent skip禁)。<code>[●]</code>行は自身のセル文字列に <code>§</code> / <code>lane</code> / <code>一次</code> のいずれも無ければWARN。同じく <code>[●]</code>行の直下に <code>tools/ledger_flip.mjs</code> が書く受領書コメント行が無ければ別のWARN(手書き[●]の検出・SS548/SS558)。受領書が無い既存行のうち導入時点で存在していたものは <code>tools/ledger_flip_grandfather.json</code> に記載され「legacy」と表示される(mass-migrationはしない・WARN件数からは除外しない)。「凌駕言及行」は行文字列に「凌駕」という語が含まれる件数の機械count(判定でなくkeyword count)。</p>
  <p>再生成: <code>node tools/ledger_dash.mjs</code>(git操作なし・本file+docs/ABSORPTION_STATUS.html のみWrite)。</p>
</footer>
</div>
</body>
</html>
`;
}

// -- entrypoint -----------------------------------------------------------
function main() {
  const report = buildReport();
  printReport(report);
  if (!process.argv.includes('--no-html')) {
    const outDir = path.join(APP_ROOT, 'docs');
    mkdirSync(outDir, { recursive: true });
    const outFile = path.join(outDir, 'ABSORPTION_STATUS.html');
    writeFileSync(outFile, renderHTML(report), 'utf8');
    console.log('');
    console.log(`wrote ${path.relative(BUILD_ROOT, outFile).replace(/\\/g, '/')}`);
  }
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) main();
