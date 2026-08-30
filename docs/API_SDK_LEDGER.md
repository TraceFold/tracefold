# API/SDK ledger — what the public HTTP surface and the TypeScript SDK actually cover

This page is a registry, not a pitch: one row per asset that would appear in an
industry-standard "API + SDK" package (spec, route table, field table, generated
client, quickstart, changelog, examples). For each asset it names where the number
came from, whether a script re-derives it, and whether anything turns red if the
number drifts. A cell with no script or no gate says so instead of a number that
looks machine-checked and is not.

Everything below was counted on 2026-08-31 by opening the named files directly (not
copied from an earlier report). Two development-tree files this page cites --
`req/spec/40-architecture/44-api-spec.md` and `glovrex_app/membrane/*.json` -- are
**not shipped in this public repository** (the SDK's own `README.md` already
discloses this for the spec: "internal to the development tree this package is cut
from -- not part of this public repository"). This page lives at repo root
`docs/`, alongside `docs/LIMITS.md`, for the same reason `LIMITS.md` does: a
private-tree page that a public reader cannot open the citations of is still more
honest than no page.

## 1. Asset registry (derivation triple: source / script / drift gate)

| Asset | Present? | Derivation source (file:line) | Derivation script | Drift gate |
|---|---|---|---|---|
| Machine-readable spec (single source of truth) | **Absent** | -- | -- | none -- `req/spec/40-architecture/44-api-spec.md` is hand-written prose (headings + tables), not OpenAPI/JSON Schema. The file's own §2.1 note says, verbatim, "本table自体はpin無し" -- "this table itself carries no pin [against the router]". |
| Route/verb table | **Present** | `glovrex_app/membrane/route-table.json` (24 entries in `routes[]`) | `glovrex_app/membrane/tools/route_table_from_crate.mjs` (extracts from `crates/gx-api`) | drift test named in the file's own header comment (not re-run by this page) |
| Field/kind table (response-body members) | **Present, incomplete by declaration** | `glovrex_app/membrane/wire-fields.json` (12 field entries, covering 5 of the 24 routed list/read endpoints, page-level only) | none -- hand-maintained | none. The file's own `"hole"` key names the gap: `handlers.rs` response bodies are unread, so item/row-level members are absent, not merely uncounted. |
| Typed HTTP client | **Present** | `sdk/typescript/src/client.ts` -- `SPECIFIED_METHODS` (17 entries) + `EXTENSION_METHODS` (8 entries, one of which, `raw`, is a generic escape hatch tied to no single route) | none generates the class; it is hand-written | `sdk/typescript/test/endpoint_parity.test.mjs` asserts `SPECIFIED_METHODS.length` equals the row count parsed from spec §2.1 by `sdk/typescript/testlib/support.mjs::specifiedEndpointsFromSpec()`. **This test is excluded from the public repository** (`tools/pub_sync_dryrun.sh` `HAND_FLOOR`, because it reads the private spec file) -- a public clone cannot re-run this gate itself. |
| Human-readable generated reference | **Absent** | -- | -- | none -- neither the spec nor the route table has ever been rendered into a separate reference doc; the spec's prose is the only reference and is not generated |
| Quickstart | **Present** | `sdk/typescript/README.md` + `sdk/typescript/scripts/quickstart.mjs` | n/a (hand-written, shipped as-is) | `sdk/typescript/test/quickstart.test.mjs` (exercises the script; does not check README prose against it) |
| Changelog | **Present, repo-wide only** | `CHANGELOG.md` (root) | n/a | none. No crate-level or SDK-level changelog exists; one root file covers everything. |
| Examples | **Present, single file** | `examples/ci/receipt-check.yml` (+ `examples/README.md`) | n/a | none |

## 2. Route coverage: three counts that do not agree, and why

Three artifacts each claim to enumerate the HTTP surface. Counted directly, they
give three different totals:

| Source | Count | Method |
|---|---|---|
| `crates/gx-api/src/lib.rs::router` (the actual server) | **25** method-paths | Counted every `.route(...)` call in the function body (lines 551-608), 2026-08-31. |
| `glovrex_app/membrane/route-table.json` | **24** routes | Counted `routes[]` array entries. |
| `req/spec/40-architecture/44-api-spec.md` §2.1 table | **17** rows visible to the automated parser; **21** rows in the file | The parser (`specifiedEndpointsFromSpec()`) matches lines starting `` | ` `` -- the four rows the file's own 2026-08-30 "M-13" correction added for the attach-source registry are written as `` | 🔴 `POST /attach-sources` ``, so the leading emoji moves the match point past the parser's prefix and those four rows are invisible to the count the SDK gate checks. The gate still passes today (17 == 17) only because `SPECIFIED_METHODS` was never asked to include them either -- they are covered by `EXTENSION_METHODS` instead (§3). |

**The gap is one route**: `POST /attach-sources/{id}/observations`
(`crates/gx-api/src/lib.rs:601-604`, handler `observations::ingest`, the
observation-ingest endpoint for the attach-source registry) exists in the
live router but is absent from `route-table.json`'s `routes[]` array and has no
type, method, or mention anywhere in `sdk/typescript/src/types.ts` or `client.ts`.
It is reachable only through `GxClient.raw()`, untyped. This was not previously
recorded in this repository's own ledgering of the same assets -- it surfaced from
reading `crates/gx-api/src/lib.rs` directly rather than trusting `route-table.json`'s
own claim to already be machine-synced with the crate.

## 3. SDK method coverage against each denominator

`sdk/typescript/src/client.ts` names 25 methods total: 17 in `SPECIFIED_METHODS`
(spec §2.1's parsed rows, gated 1:1 by `endpoint_parity.test.mjs`) and 8 in
`EXTENSION_METHODS` (`ledgerConsistency`, `listCandidates`, `listEscalations`,
`listTransformations`, `registerAttachSource`, `listAttachSources`,
`getAttachSource`, `raw`). Excluding `raw` (generic, tied to no single route), 24
named methods remain.

- **Against `route-table.json` (24 routes)**: 24/24 named methods map one-to-one.
  Verified by manual cross-reference of every `routes[]` entry against a
  `client.ts` method comment, 2026-08-31 -- **no script performs this check and no
  gate would turn red if it broke**. Recorded here as an open gap rather than
  implied to be covered.
- **Against the live router (25 method-paths, §2)**: 24/25 (96%). The one
  uncovered method-path is the observations-ingest route named in §2.

## 4. Publication path (pub_sync transport)

This file is written to root `docs/`, per this task's instruction. Checked against
`tools/pub_sync_dryrun.sh::build_manifest()` (2026-08-31):

- The root-`docs/` loop (script lines 247-250) is a **fixed two-file whitelist**
  (`docs/LIMITS.md`, `docs/TUTORIAL.md`) -- confirmed by the script's own adjacent
  comment ("this loop is a fixed whitelist ... it has to be declared"). A file
  placed at `docs/API_SDK_LEDGER.md` is **not** picked up by this loop.
  `docs/ERROR_TAXONOMY.md` is in the same unpicked state today, for the same
  reason -- and this repository already carries a same-named file staged
  separately under `public/docs/ERROR_TAXONOMY.md`, which is the pattern the
  next point describes.
- Separately, `git ls-tree -r --name-only HEAD -- public/` (script line 203) walks
  the entire `public/` tree and maps every path under it to the matching
  repo-root path. A copy placed at `public/docs/API_SDK_LEDGER.md` would be
  picked up by this walk with no script change.
- **Transport declaration**: neither of the above is done by this page (this task
  is scoped to writing `docs/API_SDK_LEDGER.md` only; editing
  `tools/pub_sync_dryrun.sh` or staging a `public/` copy is reserved for the lane
  that runs the `pub_sync` dry-run). Until one of the two happens, this file
  exists in the private tree and does **not** reach the published repository.

## 5. Conformance (this page as a LatentGraphToken instance)

This ledger is a single mapping: `asset (8 rows) -> (source, script-or-none,
gate-or-none)`, total and single-valued over an **enumerated, not scanned** row
set (§1's 8 rows were hand-listed from the existing comparison in this repository's
own reqdef trail, not produced by a directory walk -- so the row set itself has no
completeness gate; a ninth asset could exist unlisted). No cell is a quotient: every
value traces to one primary `file:line` and nothing is discarded in deriving it.
Verdict vocabulary follows the repository's three-value convention
(present / present-but-incomplete / absent) rather than collapsing "not
independently checkable" into "absent."

## 6. Self-reported errors in producing this page

1. The first pass at §2's route count used an earlier internal comparison table
   (which lists `route-table.json` as the routed source without flagging
   staleness) instead of re-opening `crates/gx-api/src/lib.rs` directly.
   Re-opening the crate source was what surfaced the missing
   `/attach-sources/{id}/observations` route -- the earlier table would have been
   transcribed as-is otherwise, silently inheriting a one-route gap.
2. `req/spec/40-architecture/44-api-spec.md` was assumed to still be 774 lines (a
   number carried from an earlier reqdef) until read directly; it is over 1075
   lines as of 2026-08-31 (the file has grown past line 1075 with content this
   page does not otherwise cite). The 774-line figure is not repeated above.

## 7. Not examined (declared, not silently skipped)

- `crates/gx-api/src/handlers.rs` response-body construction was not read line by
  line in this task; §1's "field/kind table" row relies on `wire-fields.json`'s own
  hole declaration rather than an independent re-count.
- The 12 non-`sdk` crates' `Cargo.toml` descriptions (the crate-README denominator
  from a separate, already-landed atom) were not re-checked here -- out of this
  page's scope (API/SDK surface only, not per-crate documentation).
- `sdk/typescript/src/types.ts` was grepped for the string `Observation` (zero
  hits) but not read end to end; a differently-named type covering the same field
  set cannot be ruled out with certainty higher than that one grep provides.
- Denominator: this page reads 4 primary files in full or by targeted grep
  (`route-table.json`, `wire-fields.json`, `client.ts`, `lib.rs`'s router
  function) out of the full set of files a complete API/SDK audit would touch
  (spec §2.1-§2.7 prose, all of `types.ts`, all test files under
  `sdk/typescript/test/`) -- those remain unread and are not claimed as checked.
