# gx-adapter-mcp

The MCP SubstrateAdapter: a tool call that cannot reach a server except through an admitted apply (41 §4, FR-046, AC-051).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> | **object** | the **resource** at one URI on one server ... | **CAS scope** (42 §3.5) | the
> **resource**, same as the object ... | **footprint** (`commutation`) | the **server** ...
> `Commutes` is the **fail-open** direction (M4-25), so two changes on one server are
> `Conflicts` and 43 §8 makes one of them wait.

> locator := `<server endpoint URI> "#" <resource URI>` ... Both parts are required and neither
> may be empty ... The **server endpoint carries a scheme** (`https://`, `stdio://`, `unix://`).

## What this crate does not guarantee

> A tool call whose effect lands on a resource **other** than the one its transformation is
> about is invisible to the CAS: the fingerprint reads the object, and the object is not where
> the effect went. What the membrane offers against it is not detection but **serialisation**
> — the footprint is the server, so nothing else on that server runs beside it. Detection would
> need the server to tell a proxy what a tool touches, which no part of MCP does.

> What that costs is stated where a reader will find it ...: **no JSON-RPC framing ships here**,
> so "the proxy speaks MCP" is not among the things this hand measured. What it measured is that
> nothing in this workspace can call a tool without going through one admitted `apply`.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (crate), §4 (the seven methods); `42-*.md`
§3.4 (delta), §3.5 (fingerprint). Obligation FR-046 (`32-functional.md`), measured by AC-051
(`34-*.md`). M7 requirement definition `req/98_M7_REQDEF_2026-08-11.md` §7-2 hand 3, ratified
`req/38` §59.

## Not covered

A `#` in the resource URI is refused (RFC 3986 reserves `#` for the fragment, so the grammar
cannot then find its own separator); this residue is raised in `req/101`.
