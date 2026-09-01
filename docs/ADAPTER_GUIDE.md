# /adapter-guide -- connecting a substrate to gx

This page answers one question, asked directly (Owner #268): "can an adapter actually connect to
any OSS?" The short answer is no -- the correct question is narrower, and this page states the
narrower one, then shows what it costs to answer yes to it.

Everything below is drawn from `req/1019_ADAPTER_CONNECTIVITY_AUDIT_2026-09-01.md`, a read-only
audit against `HEAD` at commit `17d87355` (`git show HEAD:<path>` was used throughout that audit
because `crates/` was under concurrent edit by another lane at the time). Figures here are that
audit's figures, not re-derived.

## 1. What an adapter is

An adapter implements `SubstrateAdapter` (`crates/gx-substrate/src/adapter.rs`), a fixed 7-method
trait. It has held at 7 for about 3 months: a test (`adapter_spec.rs`) watches the method count by
hand, and two separate proposals to add an 8th method were both absorbed into an existing method's
return type instead of landing as a new one.

| method | contract | effect |
|---|---|---|
| `kind` | which substrate this adapter speaks for | none |
| `snapshot` | read a locator's current state | read only |
| `plan` | `(intent, snapshot) -> PlannedDelta`, deterministic | pure (reads allowed, writes not) |
| `precondition` | a fingerprint that must change when the state changes (CAS basis) | read only |
| `apply` | execute a gate-approved delta; safe to retry the same delta | writes |
| `invert` | construct the delta that undoes `apply`, from a prior snapshot | partial (defined at one point: the prior snapshot) |
| `commutation` | do two deltas conflict or not | none |

`invert` is the one worth reading twice. It returns a three-valued `Reversibility`
(`True`/`False`/`Unknown`), carried inside `InvertOutcome`, and **`Ok(None)` -- "no inverse" -- is
a valid, structural answer, not a failure the adapter author forgot to handle.** The gate reads
that value and escalates to a human rather than refusing the effect outright. An adapter that can
only ever return `Ok(None)` for a given operation is telling the truth about that operation, not
failing to implement `invert`.

## 2. Can my system be an adapter?

"Any OSS" is the wrong frame. The frame that holds: **any substrate that can answer three
questions.**

1. **Readable** -- current state expressible as an `ObjectSnapshot`, with a `Fingerprint` that is
   guaranteed to change when the state changes.
2. **Writable** -- an `Intent` can be turned into a `PlannedDelta` and executed by `apply`, and
   `apply` is safe to retry on the same delta.
3. **Inverse or restore, declarable** -- either `invert` can construct a true inverse (fs: write
   the old bytes back; git: point the ref back at what it pointed at before), or, short of that,
   a deployer can declare per-operation what restoring looks like (the MCP catalogue model,
   below). If neither is possible, `Ok(None)` is the honest answer, not a gap to fill.

All three are required. Several classes of operation cannot satisfy condition 3 by construction,
and gx does not pretend otherwise:

- **send-type effects** (`send`/`post`/`upload`) -- the receiving side's state is outside gx's
  reach entirely.
- **append-only creation** (`create`/`add`/`insert`) -- there is no prior value to restore; the
  escrow would be empty.
- **pure discard** (`delete`/`remove`) -- "write it back with the same call" is definitionally a
  different call, not a retry of the original one.

Measured against a census of 747 write-type MCP tools (`req/1009`, classified by the tool name's
leading verb only -- `inputSchema` contents were not checked): **103 of 747 (13.8%) fall into
these three structurally-irreversible categories.** A further **9 of 747 (1.2%)** are
escrow-synthesizable in principle but have no implementation yet. The remainder, **532 of 747
(71.2%), is Unknown** -- namespace-prefixed verbs (`notion_execute`), nonstandard verbs (`forget`,
`revoke`), or bare `write` (which could mean create or overwrite and cannot be told apart from the
name alone). Unknown is kept as Unknown and is not folded into either irreversible or reversible;
folding it either way would misreport the number that matters.

If your system can read its own state, apply a change deterministically from an intent, and either
construct a true inverse or let a deployer declare a restore, it can be an adapter. If it can only
ever accept and never answer what undoing looks like, it cannot -- and that is a property of the
operation, not a defect in gx.

## 3. Cost

Measured, `HEAD`-time line counts for the 5 adapters that exist (`git show HEAD:<path>`, counted
per file):

| crate | files | lines | trait methods implemented | where the lines go |
|---|--:|--:|--:|---|
| gx-adapter-fs | 8 | 1,555 | 7/7 | `delta.rs` (356) + `apply.rs` (297) are the bulk |
| gx-adapter-git | 9 | 1,806 | 7/7 | + `repo.rs`, low-level operations via gitoxide |
| gx-adapter-postgres | 11 | 2,530 | 7/7 | `sql.rs` (469) + `db.rs` (387) -- SQL generation and connection management |
| gx-adapter-mysql | 11 | 2,964 | 7/7 | `sql.rs` (606) + `db.rs` (612) -- heavier than postgres on dialect differences |
| gx-adapter-mcp | 12 | 6,432 | 7/7 | `catalogue.rs` alone is 3,235 (50% of the crate) |

The trait's 7 methods are not where most of the cost sits. For fs and git it is the
substrate-specific read/write plumbing. For postgres and mysql it is SQL dialect handling and
connection/catalog introspection. For MCP, `catalogue.rs` is not protocol-connection code -- it is
a declaration DSL (the `ArgSource` enum: `Forward`, `Const`, `ConstJson`, `PriorContentsUtf8`,
`PriorJson`, `GitBlobSha1OfForward`, `DoResult`, `DoResultNumberFrom`) for saying, per tool, what
restoring that tool's effect looks like.

**Estimate for a new adapter**, on this measured basis:

- **Lower bound** (fs-class, a plain read/write substrate): 1,500-1,900 lines.
- **Mid** (SQL substrate): 2,500-3,000 lines, dominated by dialect and connection handling.
- **Upper bound** (declaration-model substrate, MCP-class): 6,000+ lines, dominated by writing a
  DSL for per-operation restore declarations, not by speaking the protocol itself.

## 4. Conformance

A `Fixture` implementation is what an adapter author actually writes; the harness itself
(`gx-substrate-conformance`) is a fixed, pre-written suite. `Fixture` (public trait, in
`gx-substrate-conformance/src/lib.rs`) has 11 methods: **5 required** (`adapter`, `locator`,
`intent`, `reset`, `disturb`) and **6 with defaults** (`uninvertible`, `commuting_pair`,
`conflicting_pair`, and 3 more) that only need overriding if the substrate has a real subject for
that case.

`run_all`, `run_contracts`, and `run_laws` are public functions, callable from one `#[test]` in the
adapter's own crate. Each check reports one of three outcomes, not two: `Pass`, `Fail`, or
`NotSupplied` -- "it failed" and "it was not measured" are kept as different answers on purpose, so
an adapter cannot claim conformance by leaving contracts unimplemented and unreported. A verdict
is only `is_conformant()` (0 failures) *and* `is_complete()` (0 unmeasured) when both are true.

**Minimal recipe:**

1. Implement `SubstrateAdapter`'s 7 methods.
2. Implement `Fixture`'s 5 required methods over your adapter.
3. Call `run_all(&fixture)` from one `#[test]`.
4. Assert `report.is_conformant()` and `report.is_complete()` are both `true`.

**Worked example** (read the source, not this paraphrase): `crates/gx-adapter-fs/tests/conformance.rs`.
`the_fs_adapter_meets_every_one_of_the_seven_contracts` calls `run_contracts`; a second test,
`one_run_reports_sixteen_obligations_and_meets_the_completion_condition`, calls `run_all` and
asserts `is_conformant()`, `is_complete()`, and `meets_51_7()` together (16 = 7 contracts + 9 laws).
The `Fixture` implementation itself is `crates/gx-adapter-fs/tests/support/mod.rs`.

## 5. What we do not promise

- **`publish = false`** on every crate's `Cargo.toml`, confirmed on `gx-substrate` and
  `gx-adapter-mcp` directly. There is no `cargo add` path to any of this. `public/README.md`
  states, in its own words, "Not released." Reading the source on GitHub is not the same as
  installing a package from a registry.
- **`gx-adapter-postgres` and `gx-adapter-mysql` are private.** They exist (2,530 and 2,964 lines,
  7/7 methods, per the table above) but are not in `public/Cargo.toml`'s member list and are not
  code anyone outside this project can currently read.
- **`@mahirhir/tracefold` on npm is a separate thing** -- a WASM receipt verifier, not this Rust
  adapter trait. Do not read the npm package's existence as evidence this trait is packaged
  anywhere.
- **MCP's wire layer connects to any MCP server that speaks the protocol; reversibility does not
  follow automatically.** The wire layer has no bundled client library by design (AC-051: an
  external client library would be a second, gate-bypassing path). But which tool undoes which is
  not discoverable from the protocol -- it is declared per tool by whoever deploys the server
  (the catalogue model). A tool with zero declared restore is treated as irreversible and routed
  to human escalation; gx does not invent a restore for it. The one real external target measured
  so far is `github-mcp-server` -- this is `n=1` for protocol compatibility, and generalizing to
  other MCP servers (Notion, Slack, and similar) is not yet demonstrated.
- **No adapter-writing guide existed before this page.** `public/.github/CONTRIBUTING.md` names no
  section for adding an adapter, and a grep across the repo for `new adapter|adapter guide|write
  adapter`-shaped phrasing returned 0 hits before this file was written. This page is the direct
  remedy, not a claim that the gap was already covered elsewhere.

---

Source: `req/1019_ADAPTER_CONNECTIVITY_AUDIT_2026-09-01.md`, recommendation 1. That audit's own
unread-range disclosures (its §6) apply here by inheritance: the census this page cites classifies
tool names by leading verb only, and `catalogue.rs`'s 3,235 lines were read in full for structure
but not read in full for content beyond the first ~400 lines and the `ArgSource`/`PriorPointer`
sections.
