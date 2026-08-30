# Error taxonomy — when a crate's error is a thiserror enum, and when manual `Display` is allowed

Written 2026-08-25 (req/813), because req/808 §3-3 measured the debt exactly: the messages in
the manual-`Display` tier are fine, but **two error-type patterns coexisted with no written rule
saying when each applies**. This page is that rule. It ratifies the shapes the census found —
no site is convicted retroactively — and binds new code.

## The canon (default for every new crate)

One `pub enum Error`, `#[derive(thiserror::Error)]`, in a dedicated `src/error.rs`.
The template by adoption is **gx-substrate** (req/808 §1): all five adapters consume
`gx_substrate::Error` rather than minting their own, and eight crates carry exactly one
thiserror derive site each (gx-canon, gx-cli, gx-core, gx-engine, gx-gate, gx-log,
gx-substrate, gx-witness — the complete inventory, cross-checked in req/808 §2).

Two corollaries:

1. **Adapters do not mint boundary errors.** A `SubstrateAdapter` implementation speaks
   `gx_substrate::Error` at its trait boundary — that is what makes one refusal vocabulary
   readable across fs/git/mcp/mysql/postgres.
2. **Message discipline is orthogonal and unchanged** by this page: English text,
   fact + remedy + citation, the 44 §1.3 problem object on stderr for the CLI, exit codes per
   discipline 52. A thiserror derive buys none of that by itself and a manual `Display` loses
   none of it (req/808 §3-1/§3-3 measured both tiers at the same message quality).

## When manual `Display` is allowed

A crate may hand-implement `std::error::Error`/`Display` when **any** of these holds, and its
error type's doc comment says which one:

- **(a) Deliberately minimal dependency set.** The crate keeps its `[dependencies]` list a
  reviewable handful and thiserror is not in it. This is gx-confine (`ConfineError`; the crate
  reads a catalogue and talks to Landlock, and its Cargo.toml documents every line) and
  gx-mcp-wire (four enums below).
- **(b) Protocol-local leaf enums.** The error names a stage of one wire protocol rather than
  the crate's single fallible surface, and several small enums say more than one wide one:
  gx-mcp-wire's `WireError` / `ConfigError` / `DetachError` / `ParseError`, which fold into
  `gx_substrate::Error` before they cross the transport boundary.
- **(c) Wire-shaped structs.** The type mirrors a serialized object, not a variant set:
  gx-api's `ApiError` is 44 §2.3's problem object member for member, and `ServeError` is the
  pre-server startup refusal beside it.

Everything else defaults to the canon. A new crate that wants an exemption cites (a), (b) or
(c) in the type's doc comment; an exemption nobody can name is a refactor waiting to be filed.

## What this page deliberately does not do

No migration of the existing manual-`Display` sites (req/808 atom 6 offered that road; this
lane took the rule instead — the sites above are compliant under (a)-(c), and a derive-only
rewrite of four wire enums buys uniformity of spelling, not of behavior). If a future ruling
prefers one pattern everywhere, that is three small derive-only commits and this page is where
the decision gets recorded.
