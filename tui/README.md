# tracefold-tui

The `gx tui` terminal face: four `GET` routes rendered to a buffer, and no engine.

Part of [Tracefold](https://github.com/TraceFold/tracefold), a defensive agent-safety layer that
makes AI-agent tool execution reversible and independently verifiable (escrow-before-apply, an
append-only Merkle receipt log, offline verification with no issuer round-trip, and an explicit
`docs/LIMITS.md` for what the layer does not cover).

This crate is the terminal renderer only: it consumes `gx-api`'s HTTP surface and nothing else — no
`gx-core`, no engine, no policy evaluation. `cargo tree -e normal -p gx-tui` names no other
`tracefold-*`/`gx-*` crate, which is the property this split exists to make structural rather than a
claim about source.

License: Apache-2.0. See the crate's `[lib]` doc comment (`src/lib.rs`) for the extraction rationale.
