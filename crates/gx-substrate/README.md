# gx-substrate

The SubstrateAdapter boundary and its delta types (41 §2 / §4, 42 §3.4).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> 41 §2 places this crate in the workspace and calls it "the point that secures P-6", 41 §4
> gives the `SubstrateAdapter` trait, 42 §3.4 gives the field tables of `PlannedDelta` and
> `AppliedDelta`.

> Nothing here reads a clock, opens a file or computes a digest — the trait says what an
> adapter promises and implements none of it; a crate that does none of those cannot be an
> adapter, and it is not meant to be one. `crates/gx-substrate/tests/substrate_contract.rs`
> asserts that absence rather than leaving it to be noticed later.

> Locator normalisation (normative): 1. **Lexical only** ... performs no I/O ... 2.
> **Dot-segment folding** ... 3. **Duplicate-separator removal** ... 4. **Trailing-separator
> convention** ... 5. **Symlink/realpath resolution is v0.2+.**

## What this crate does not guarantee

> Clause 5 leaves a hole and the hole is named rather than papered over. **TH-2 residue**: a
> locator reaching a policy-protected path through a symbolic link is not closed by lexical
> normalisation, so an actor who can create links can still choose which spelling the gate
> sees.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (crate placement), §4 (`SubstrateAdapter`
trait); `42-*.md` §3.4 (`PlannedDelta`/`AppliedDelta` field tables).

## Not covered

Symlink/realpath resolution (v0.2+, not this crate's obligation — see TH-2 residue above);
the shared contract harness that measures an implementation against this boundary is a
separate crate, `gx-substrate-conformance`.
