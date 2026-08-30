# gx-gate

Cedar policy evaluation, invariant registry and verdict composition (41 §2 / §4, 42 §3.8).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> `A(f)`: the predicate that decides whether a transformation is admissible.

> It is **not** the state machine. 43's transitions belong to gx-engine (M5): a `Verdict` says
> what is true of a transformation, and what a system *does* about that — refuse the commit,
> record it anyway under `EnforcementMode::RecordOnly`, wait for a human — is decided one layer
> up.

> `Gate::verify` evaluates the policy set, runs every registered invariant, and refuses if
> either refused. ... A gate that was never given a policy set still answers `Err`, because an
> empty `policies/` directory and a working deployment must not look the same (req/29 §4).

## What this crate does not guarantee

> It also **cannot see the change it is judging**. 42 §3.4 makes a delta's payload opaque to
> everything below the adapter (P-6), so what a policy may reason over is the locator, the
> actor, the change context, the order, whether an inverse exists, and the evidence.

> **T1 is not a property of this crate (E-M3-5)** ... 51 §3 asks gx-gate for a property test
> ... asserting `A(f) ∧ A(g) → A(g∘f)`, and **that statement is false of an arbitrary Cedar
> policy set**. The counterexample is one policy: forbid any transformation that touches `/a`
> and `/b` together.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (crate, modules), §4 (`GateInput`,
`InvariantCheck`, `Gate::verify`), §6 (what every crate here may not do); `42-*.md` §3.8
(field tables); `34-*.md` §E (AC-025..029).

## Not covered

No state machine (43 belongs to `gx-engine`, N-01) and no `EnforcementMode`/`FailPosture`
(N-02) — both are named in req/60 §1 as deliberately somebody else's.
