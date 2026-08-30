# gx-cli

The `gx` command line: `.gx/` layout, draft store, id-resolution cache (44 §1, req/56).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> this crate holds no semantic authority ... M6 does not extend `Σ`. gx-cli / gx-api hold only
> **observation** of `Σ = (L, J, E, Λ)` and **mapping** onto the engine's 8 entry points.
> Therefore, if M6 adds semantics, that is an implementation defect and not a design choice.

> no canonical encoding. 41 §6: "every canonical encode goes through gx-canon only, bypass
> forbidden". This crate parses `gx1:` text with `gx_core::Cid::from_text` and never computes
> one. ... no `Verdict`. 41 §4 puts the one judgement in `Gate::verify`. ... no `Lifecycle`
> write ... This crate reads states; it keeps none.

> The one asymmetry, stated rather than hidden (req/88 §3 Λ2): "the moment the CLI holds state
> that does not enter `Σ`, equality breaks — M6-01(a)'s `.gx/drafts/` is exactly that." ...
> AC-055's "identical" has to be read as "identical from `Candidate` onward".

## What this crate does not guarantee

> a `View` is a **borrow** type ... `Deserialize` produces an owned value out of bytes it does
> not keep, so a borrowing struct can only implement it by borrowing from the input buffer ...
> Measured rather than asserted: nine `…View` types are public in this workspace ... and eight
> of the nine carry a `<'a>`. [This crate does not implement `Deserialize` for those types.]

## Position

`req/spec/40-architecture/44-api-spec.md` §1 (the `gx` verb surface); `req/56` (the `.gx/`
directory layout this crate declares). Rule 1 = req/88 §3 Λ1; the asymmetry = req/88 §3 Λ2.

## Not covered

The three verbs whose mechanism is not in the public distribution (`gx wrap`/`gx attach`/
`gx confine`/postgres sessions — `req/817`) are feature-gated out of the public manifest
(`public/crates/gx-cli/Cargo.toml`), not silently dropped — the feature names remain declared
there with the reason.
