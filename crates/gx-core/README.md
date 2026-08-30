# gx-core

Core types of the Glovrex transformation calculus. No I/O, deterministic (41 §3 / §6).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> Spec: ... §3 for the type signatures it is required to carry, §6 for what it is forbidden to
> do — no I/O, deterministic, no `unsafe`.

> The dependency between the two implementation crates runs one way. `gx-canon` names
> `gx-core`; `gx-core` never names `gx-canon`. That is what lets `Cid` be defined here while the
> BLAKE3 computation that fills one in lives over there (`ASM-16`, and the A-1 ruling ...).

> One rule holds all four: **the data comes down, the computation stays up**. Nothing added
> here signs, hashes, verifies or compares — that is A-1's shape (`Cid` here, BLAKE3 in
> gx-canon) applied a second time, and it is what makes the cycle absent from the dependency
> graph instead of forbidden by a rule someone has to remember.

## What this crate does not guarantee

This crate holds type definitions only. It performs no I/O, no hashing, no signing, no
verification and no comparison — those operations live in the crates that depend on it
(`gx-canon` for hashing/CID computation, `gx-witness` for signing).

## Position

`req/spec/40-architecture/41-architecture.md` §2 (where this crate sits in the workspace),
§3 (type signatures it must carry), §6 (what it may not do).

## Not covered

This crate does not define `IdentityView` or any encoding — those are `gx-canon`'s, and
`gx-core` is deliberately unaware that `gx-canon` exists (A-1).
