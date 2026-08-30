// SPDX-License-Identifier: Apache-2.0
// req/967 §4-2 (AC-3/AC-4): coverage.json is hand-maintained (membrane/README.md), and a
// hand-maintained file is exactly where a face's registration and its real behaviour
// drift apart without anyone noticing -- which is what happened before this file existed:
// coverage.json's consumed/drawn were both empty while the terminal face (req/946) was
// already calling four routes and drawing one route's rows.
//
// This suite does not hand-count what the terminal face reads or draws. It imports the
// face's own exports -- `READS`/`STREAM`/`SUBJECT` from terminal/tui.mjs, `COLUMNS` from
// terminal/roles.mjs -- and checks coverage.json against THEM. A number written here by
// hand would be exactly the "rotten index" req/967's own doctrine forbids; the face's
// source is the only thing this file trusts.
//
// STREAM is folded in beside READS on purpose: it is a fifth route this face actually
// calls (tui.mjs's own `main()` -- `void subscribe()` -- opens it unconditionally
// outside `--once`/`--keys` mode), and tui.mjs's OWN apparatus line already computes
// `[...READS, STREAM]` as one set (`apparatusLines`'s `stale` local) to say so. Checking
// READS alone would have missed it -- and did, on the first pass of this file, until the
// live `--once` capture below showed the apparatus line still naming get_stream as
// declared-not-consumed after READS-only registration had gone in.
//
// Independent of terminal/check.mjs on purpose (req/967 AC-3 names this as one of the
// two sanctioned shapes): check.mjs draws a MOCK ledger fixture for its rendering probes
// (P1-P15) and does not read the real coverage.json at all, so this is the one place a
// stale registration is caught before a run against a live engine surfaces it in the
// apparatus line at runtime.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { tableRows } from '../src/membrane.mjs';
import { deriveLedgers } from '../src/ledgers.mjs';
import { READS, STREAM, SUBJECT } from '../../terminal/tui.mjs';
import { COLUMNS } from '../../terminal/roles.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const MEMBRANE = join(HERE, '..');
const ROWS = tableRows();

/** What this face actually calls: the four polled routes, plus the one it subscribes to. */
const CONSUMES = Object.freeze([...READS, STREAM]);

const coverage = () => JSON.parse(readFileSync(join(MEMBRANE, 'coverage.json'), 'utf8'));
const fields = () => JSON.parse(readFileSync(join(MEMBRANE, 'wire-fields.json'), 'utf8')).fields;

/** Every route in CONSUMES must carry "terminal" in coverage.json's consumed. */
function assertReadsAreConsumed(held) {
  const consumed = held.consumed ?? {};
  for (const route of CONSUMES) {
    const faces = consumed[route] ?? [];
    assert.ok(faces.includes('terminal'), `tui.mjs reads/subscribes to "${route}" but coverage.json's consumed does not list "terminal" for it`);
  }
}

/** SUBJECT's rows are what COLUMNS actually draws; coverage.json's drawn must say so. */
function assertSubjectIsDrawn(held) {
  assert.ok(COLUMNS.length > 0, 'roles.mjs declares no columns; this probe measures nothing');
  const drawn = new Set(held.drawn ?? []);
  assert.ok(drawn.has(`${SUBJECT}.items`), `roles.mjs draws ${COLUMNS.length} column(s) from ${SUBJECT}'s rows, but coverage.json's drawn omits "${SUBJECT}.items"`);
}

// ---------------------------------------------------------------------------
// F1/F2 -- consumed, both directions
// ---------------------------------------------------------------------------

test('F1 every route terminal/tui.mjs READS is registered as consumed by "terminal"', () => {
  assertReadsAreConsumed(coverage());
});

test('F1-neg the same check refuses a coverage.json with "terminal" stripped from a route this face reads', () => {
  const stale = coverage();
  stale.consumed.get_healthz = (stale.consumed.get_healthz ?? []).filter((f) => f !== 'terminal');
  assert.throws(() => assertReadsAreConsumed(stale), /get_healthz/);
});

test('F2 coverage.json claims no route for "terminal" that tui.mjs does not actually read or subscribe to', () => {
  const consumed = coverage().consumed ?? {};
  for (const [route, faces] of Object.entries(consumed)) {
    if (!faces.includes('terminal')) continue;
    assert.ok(CONSUMES.includes(route), `coverage.json says "terminal" consumes "${route}", which is in neither tui.mjs's READS nor its STREAM`);
  }
});

test('F2-neg the same check refuses a coverage.json claiming a route this face never reads', () => {
  const rigged = coverage();
  rigged.consumed.get_ledger_proof = ['terminal'];
  const offenders = Object.entries(rigged.consumed).filter(([r, faces]) => faces.includes('terminal') && !CONSUMES.includes(r));
  assert.deepEqual(offenders.map(([r]) => r), ['get_ledger_proof']);
});

// ---------------------------------------------------------------------------
// F3 -- drawn
// ---------------------------------------------------------------------------

test('F3 the SUBJECT route\'s rows, which roles.mjs actually draws columns from, are registered as drawn', () => {
  assertSubjectIsDrawn(coverage());
});

test('F3-neg the same check refuses a coverage.json with the drawn route stripped', () => {
  const stale = coverage();
  stale.drawn = stale.drawn.filter((key) => key !== `${SUBJECT}.items`);
  assert.throws(() => assertSubjectIsDrawn(stale), new RegExp(SUBJECT));
});

// ---------------------------------------------------------------------------
// F4 -- with the registration in place, the real ledger agrees
// ---------------------------------------------------------------------------

test('F4 deriving the ledgers against the real tables reports no problems, and the registered routes have left NOT_CONSUMED', () => {
  const { ledgers, ok, problems } = deriveLedgers({ routes: ROWS, coverage: coverage(), fields: fields() });
  assert.equal(ok, true, problems.join('; '));
  for (const route of CONSUMES) assert.ok(!ledgers.NOT_CONSUMED.members.includes(route), `${route} is registered as consumed and is still in NOT_CONSUMED`);
  assert.ok(!ledgers.NOT_DRAWN.members.includes(`${SUBJECT}.items`), `${SUBJECT}.items is registered as drawn and is still in NOT_DRAWN`);
});

// ---------------------------------------------------------------------------
// F5-neg -- req/967 §4-3 AC-6's own negative control: a fabricated route name
// ---------------------------------------------------------------------------

test('F5-neg a face claiming a fabricated route is refused, not silently passed', () => {
  const rigged = coverage();
  rigged.consumed.get_nonexistent_xyz = ['terminal'];
  const { ok, problems } = deriveLedgers({ routes: ROWS, coverage: rigged, fields: fields() });
  assert.equal(ok, false, 'a fabricated route in consumed must fail the ledger, not pass it');
  assert.ok(problems.some((p) => p.includes('get_nonexistent_xyz')), problems.join('; '));
});
