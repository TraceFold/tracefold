# gx-api

The Glovrex HTTP surface (44 §2): thirteen synchronous endpoints, the gx_code map, Bearer auth and Idempotency-Key.

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> this crate holds no semantic authority. The same three absences gx-cli carries, checked by
> the same instrument (`crates/gx-canon/tests/authority_boundary.rs`): no canonical encode
> (41 §6), no `Verdict` construction (41 §4), no `Lifecycle` write (42 §1.3-3).

> `gx_code` — M6-09's one map, thirty-three refusal kinds onto twelve codes, with the folds
> listed rather than implied; `auth` — M6-10's static Bearer, the loopback default, and "the
> check's absence" as a string; `idempotency` — M6-11's hand-written `Idempotency-Key`,
> persisted, with the one line that says what it does **not** protect; `state` — M6-06,
> adopted (a)'s single lock, and 45 §1's two keys as two methods.

> req/88 §6.2 gives this hand "44 §2's synchronous face, **11** endpoints" and hand 1's
> `SPECIFIED_ENDPOINTS` array — read off 44 §2.1 — has **fourteen** rows. Fourteen minus
> `/stream` is **thirteen** ... thirteen are implemented and the discrepancy is raised as
> **M6H5-1**.

## What this crate does not guarantee

> `GET /stream` (M6-12's event map, M6-13's resume cursor), `gx serve`'s runtime and graceful
> shutdown, and M6-05's three list endpoints [are named in the doc comment as a separate hand's
> scope — see `req/spec/40-architecture/44-api-spec.md` §2 and this crate's own module list for
> current implementation status, which this README does not restate].

## Position

`req/spec/40-architecture/41-architecture.md` §2 (crate); `44-api-spec.md` §2 (the endpoint
table this crate implements). Rule 1 = req/88 §3 Λ1.

## Not covered

This crate never constructs a `Verdict` and never performs a canonical encode — both are
mechanically checked by `crates/gx-canon/tests/authority_boundary.rs`, not by this crate itself.
