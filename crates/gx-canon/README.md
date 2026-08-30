# gx-canon

Canonical DAG-CBOR form, CID, and the JCS compatibility layer (41 §2 / 42).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> Two faces, kept apart on purpose (A-4, in `req/38_ERRATA_2026-08-07.md` §1): **wire** —
> encode and decode every field, so a value survives the round trip byte for byte (AC-009,
> AC-012). Whether bytes are canonical is decided by the encoder, never by the fact that a
> decoder accepted them ... **identity** — project a value through `IdentityView`, encode that
> projection on the wire face, hash it with BLAKE3 (AC-011, AC-013). `id` and `created_at` are
> not in the projection, so the same transformation recorded at two times has one CID.

> The identity face is defined as that composition, which is how the projection stops being
> skippable: there is no second road to a `Cid` (AC-014).

## What this crate does not guarantee

> 41 §6 types errors with `thiserror` and calls a panic a bug, which is why every refusal here
> is a returned value — including the ones that come from input a caller can control.

The wire face and the identity face are stated as deliberately separate questions (A-4); a
value that survives the round trip is not thereby claimed canonical, and bytes the encoder
would not have produced are refused (`Error::NotCanonicalizable`) rather than accepted.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (module list, dependency line);
`req/spec/40-architecture/42-*.md` (the encoding rules this crate implements).

## Not covered

`gx-core` stays unaware of this crate's encoding logic by design (A-1) — this crate defines
`IdentityView` and implements it for `gx-core` types, but the type definitions themselves live
in `gx-core`, not here.
