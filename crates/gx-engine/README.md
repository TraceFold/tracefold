# gx-engine

The Transformation lifecycle: engine journal, escrow store, deterministic replay (41 §2 / §5, 42 §3.12-3.13, 43).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> **State is never encoded**: a `Transformation` holds no lifecycle state at all (Draft/
> Candidate/…/Committed, see 43). State is managed by an external table on the engine's side,
> keyed by `TransformationId` (the engine store).

> every transition is written to the journal **before** any side effect, which makes the
> journal the truth and the in-memory table a cache. That ordering is the reason a crash
> cannot leave a change applied with nothing recording it — the property gx exists to sell.

> `undo`/`cancel`/`escalation` are hand 6's and are **absent rather than stubbed** —
> `tests/engine_shape.rs` fails on a sixth entry point as readily as on a missing fifth, so the
> boundary between hands is a measurement instead of an intention.

## What this crate does not guarantee

> `Engine::recover` is **not** a ninth entry point: 43 §7 is a procedure over the journal
> rather than a transition.

The doc comment names which entry points are implemented as of a given hand (`submit`, `plan`,
`verify`, `canonicalize`, `commit`) and which are absent (`undo`/`cancel`/`escalation`) — this
README does not restate a point-in-time count; `tests/engine_shape.rs` is the current source.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (crate, module list), §5 (commit protocol);
`42-*.md` §3.12 (`EscrowedInverse`), §3.13 (journal); `43-*.md` (the eleven states and
nineteen transitions); `32-functional.md` FR-030..FR-040; `34-*.md` AC-030..AC-045.

## Not covered

`N-13`: no substrate adapter is a dependency of this crate, and none may be — an engine that
linked `gx-adapter-fs` would ship one substrate's grammar to every user of every substrate.
