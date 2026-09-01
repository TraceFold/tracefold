// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//
// The shipped **postgres** pack, beside `policies/{fs,git,mcp}/`, and the first pack written
// against `policies/PACK_FORMAT.md` rather than by copying the one before it.
//
// # 🔴 This pack declares a **deny-default**. It has no permit statement, and that is the ruling.
//
// The other three packs open with `<substrate>-permit-default`, and the argument for that shape
// (req/101 §9-1) is that shipping such a pack is not a loosening but a **becoming-judged**: before
// it ships nothing has an opinion and Cedar's third rule denies everything; after it ships the
// forbid decides the protected surface and the permit decides the rest. That argument has a
// premise: that the substrate has a small, *universal* thing worth protecting and a bounded
// remainder it is honest to admit by default.
//
// The premise holds for `fs` (`/etc`), for `git` (`refs/tags`, `refs/remotes`) and for `mcp`
// (`file:///etc`). **It does not hold here.** What is universal about postgres is exactly the two
// system catalogs below; the surface that actually gets damaged — a business table's rows — is
// entirely deployment-specific, and a shipped pack cannot know one row of it. A permit-default
// here would hand out an unbounded admit surface in exchange for a two-line forbid, which is a
// trade the other three packs were never asked to make.
//
// So: forbid only. Concretely, adding this pack to `SHIPPED_PACKS` changes the admissible surface
// by **zero**. A change to a non-catalog postgres object was denied by Cedar's third rule before
// this file existed and is denied by Cedar's third rule after it. What the pack buys is that a
// change to `pg_catalog` or `information_schema` is now refused **by a named statement**, which is
// a refusal a receipt can cite, instead of by the absence of any statement, which is not. The
// difference is measured rather than asserted: `crates/gx-gate/tests/ac_074.rs` runs the same
// requests against the three-pack set and the four-pack set and requires the verdicts to differ in
// exactly the catalog rows and nowhere else.
//
// A deployment that wants its postgres substrate *judged* writes the one statement this file
// deliberately does not:
//
//     @id("my-postgres-permit-default")
//     permit ( principal, action, resource ) when { resource.substrate == "custom:postgres" };
//
// That is one line, it is the deployment's own decision to make, and `PACK_FORMAT.md` names it as
// the entry point. Shipping it on the deployment's behalf is what this pack refuses to do.
//
// # 🔴 The substrate tag is `custom:postgres`, not `postgres`
//
// `gx-adapter-postgres` declares `SubstrateKind::Custom("postgres")`, and
// `crates/gx-gate/src/policy.rs`'s `substrate_tag` maps a `Custom(name)` to **`custom:{name}`**.
// The scenario side agrees: `gx-cli`'s `substrate_kind` only builds a `Custom` from a spelling that
// carries the `custom:` prefix.
//
// Getting this wrong is silent. A pack written with `resource.substrate == "postgres"` parses,
// lints clean, embeds, ships — and never fires a statement, because no request ever carries that
// string. Worse, a conformance suite made only of Deny cases stays **green** through the whole
// thing: an unmatched request falls to Cedar's third rule and denies anyway, so every Deny case
// passes for the wrong reason and the pack decides nothing. This is the trap F4 of PACK_FORMAT
// exists for, and `ac_074.rs` closes it by comparing the literal in this file against the string
// `RequestView` actually produces for `SubstrateKind::Custom("postgres")`, rather than against a
// constant somebody typed twice.
//
// # 🔴 Why the anchor holds: `postgres://*/` can only be the schema position
//
// `gx-adapter-postgres`'s locator is `postgres://<alias>/<schema>.<table>?<pk_column>=<pk_value>`
// (`crates/gx-adapter-postgres/src/locator.rs`, `Position::locator`). The forbid patterns below
// anchor on `postgres://*/`, and the claim that this anchor cannot be evaded or misfired rests on
// counting the `/` characters a produced locator can contain. It is **exactly three**: two in the
// `postgres://` scheme and one after the alias. Nothing else in the string can be a `/`:
//
//   * `alias`, `schema`, `table` and `pk_column` all go through `is_identifier`, whose whole
//     alphabet is `^[A-Za-z_][A-Za-z0-9_]{0,62}$` — no `/` is admissible.
//   * `pk_value` goes through `percent_encode`, which passes only the RFC 3986 unreserved set
//     (`A-Za-z0-9-._~`) and rewrites everything else as `%XX`. A `/` in a primary key value
//     arrives as `%2F`.
//
// Two consequences, and both are the point:
//
//   1. **It cannot be evaded.** The third `/` is the alias/schema boundary and the schema name
//     cannot contain a `/` to push the boundary rightwards, so `postgres://*/pg_catalog.` matches
//     the schema position and only the schema position.
//   2. **It cannot be misfired.** A primary key value of `/pg_catalog.x` — the obvious forgery —
//     is encoded to `%2Fpg_catalog.x` and does not match. Neither does an alias named
//     `pg_catalog`, because the pattern requires the literal `/` before it and there is no `/`
//     inside an alias.
//
// This is the same shape of argument the mcp pack makes about `#`, and it is kept in the same
// place: a test, not a paragraph. `ac_074.rs::every_case_names_a_locator_the_postgres_adapter_
// could_produce` runs every conformance locator through the adapter's own `parse()` and counts the
// slashes, so the argument goes red if either the identifier alphabet or the encoder changes.
//
// # Why these two schemas and nothing else
//
// `pg_catalog` and `information_schema` are postgres's own, present in every database that exists,
// and they are the same shape as the fs pack's `/etc`: what a change to them alters is what other
// things *mean*. Rewriting a `pg_catalog` row does not change one table's contents, it changes what
// the server believes about every table. `information_schema` is the SQL-standard view onto the
// same, and a deployment reading it to decide what is safe to touch is reading a claim this pack
// keeps honest.
//
// Both are bare identifiers, so both pass `is_identifier` and are spellings the adapter can
// actually produce — F7 / D-4, kept measured by the test named above rather than assumed.
//
// What is **not** here: any rule naming a business schema (`public`, or anything else). That is the
// deployment's, for the reason the whole header is about.
//
// `@id` on every statement is a format requirement, not decoration (ASM-62-1 / C-4):
// `PolicySet::from_str` names statements by position, that id lands in `PolicyDecisionRecord`, and
// 42 §1.3 puts the record inside `AdmitProof`'s IdentityView — so reordering this file would change
// what a receipt says without changing what was decided.

@id("postgres-deny-system-catalogs")
forbid (
    principal,
    action,
    resource
)
when {
    resource.substrate == "custom:postgres"
    && (   resource.locator like "postgres://*/pg_catalog.*"
        || resource.locator like "postgres://*/information_schema.*" )
};
