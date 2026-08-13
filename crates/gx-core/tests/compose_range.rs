//! **E-M3-13** (D-6) — the value range a composed arrow's metadata has to be in.
//!
//! `req/38_ERRATA_2026-08-07.md` §22 逐語: 「🔴**D-6(採用=erratum E-M3-13・述語①②③・実装は手 6 窓)**:
//! A-7 値域検査を **gx-core `compose`(と `Transformation::new`)側に置く**事を erratum として採用。
//! 述語は ①`created_at ≥ 0` ②`intent_id ≠ 全 0 placeholder` ③`created_at ≥ max(f,g)`(compose のみ)。
//! **④(intent ∈ {f,g})は不採用**……「未来でない」は時計=M5」.
//!
//! Until this hand the gap was `req/38` §7's: 「合成後 `created_at`/`intent_id` の許容値域が未定義の
//! まま `compose()` は無検査で通す(46B WARN)」. `CompositionMetadata` is supplied by a caller, and
//! nothing read it -- so a composite could be dated before the arrows it was made of, and could name
//! a Draft that does not exist, and the crate had no opinion.
//!
//! # What this suite is written to keep, and what it deliberately does not assert
//!
//! Four of the probes below assert that something is **admitted**. They are not filler: ④ and 「未来
//! でない」 were ruled *out*, and a ruling that only lives in prose is a ruling the next hand
//! re-litigates. `the_composite_may_carry_an_intent_neither_part_carries` fails the day somebody
//! implements ④, and `a_far_future_created_at_is_not_this_crates_business` fails the day somebody
//! reaches for a clock in a crate that 41 §6 forbids one to (the check would need `now`, and gx-core
//! cannot have it -- which is *why* M5 owns it).
//!
//! The boundary probes are the other half: ① is `>= 0` and not `> 0`, and ③ is `>=` and not `>`, so
//! the epoch and the tie are both admitted. A mutation that swaps either comparison changes exactly
//! those two answers, which is what `tools/verify_m3h6.sh` §7-9 measures.
//!
//! # The third door (**reported by M3 hand 6, closed by M4 hand 1**)
//!
//! [`gx_core::identity`] takes the same [`CompositionMetadata`] and was infallible -- 41 §3's
//! reading recorded in `transformation.rs`: 「returning a `Result` whose error arm is unreachable
//! would be a lie about the API」. E-M3-13 named `compose` and `Transformation::new` and said
//! nothing about `identity`, so M3 hand 6 implemented the two it names and **measured** the third
//! rather than widening a ruling on its own authority; `req/66` §4 raised it and `req/38` §25 H-1
//! ruled it (**E-M3-18**). `identity_is_the_third_door_and_e_m3_18_closed_it` is the same fixture
//! with the answer inverted, and `crates/gx-core/tests/value_range_closure.rs` is where the claim
//! stops being about one function and becomes a count taken from the source.

mod conformance;

use conformance::{cid, metadata, snapshot, World};
use gx_core::{
    compose, identity, CompositionMetadata, Error, IntentId, ObjectSnapshot, Subject, Timestamp,
    Transformation, TransformationId,
};

/// An arrow from `from` to `to`, dated `created_at`.
///
/// `conformance::arrow` dates every arrow at the epoch, which is the one timestamp predicate ③
/// cannot be tested with -- everything is `>= 0`.
fn arrow_at(
    seed: u8,
    from: &ObjectSnapshot,
    to: &ObjectSnapshot,
    created_at: i64,
) -> Transformation {
    let mut meta = metadata(seed);
    meta.created_at = Timestamp(created_at);
    Transformation::new(
        TransformationId(cid(seed.wrapping_add(64))),
        0,
        Subject::Object(*from.id()),
        Some(*to.digest()),
        Vec::new(),
        meta,
    )
    .expect("the fixture is inside the range this suite is about")
}

/// The metadata of `seed`, with `created_at` moved.
fn meta_at(seed: u8, created_at: i64) -> CompositionMetadata {
    let mut meta = metadata(seed);
    meta.created_at = Timestamp(created_at);
    meta
}

/// A composite of `f` and `g` carrying `meta`, over a world that resolves both.
fn compose_with(world: &World, meta: CompositionMetadata) -> Result<Transformation, Error> {
    compose(&world.f, &world.g, world.resolve(), meta, |_| {
        TransformationId(cid(99))
    })
}

// ---------------------------------------------------------------------------
// The control: a well-formed composition still composes
// ---------------------------------------------------------------------------

/// req/29 §2 in one line: a suite whose refusals are all it asserts cannot tell a working check from
/// a check that refuses everything.
#[test]
fn a_composite_inside_the_range_is_still_built() {
    let world = World::new(1, 2, 3);
    let composed = compose_with(&world, meta_at(1, 1_754_000_000_000_000_000))
        .expect("nothing about this composition is out of range");
    assert_eq!(composed.parents.len(), 2);
    assert_eq!(composed.created_at, Timestamp(1_754_000_000_000_000_000));
}

// ---------------------------------------------------------------------------
// ① created_at >= 0
// ---------------------------------------------------------------------------

/// 「①`created_at ≥ 0`」, at `compose`.
#[test]
fn d6_1_compose_refuses_a_negative_created_at() {
    let world = World::new(1, 2, 3);
    let got = compose_with(&world, meta_at(1, -1)).expect_err("a composite dated before the epoch");
    assert_eq!(got, Error::CreatedAtNegative { got: -1 }, "{got}");
}

/// 「①」, at the other door E-M3-13 names.
#[test]
fn d6_1_new_refuses_a_negative_created_at() {
    let x = snapshot(1, 4);
    let y = snapshot(2, 4);
    let got = Transformation::new(
        TransformationId(cid(5)),
        0,
        Subject::Object(*x.id()),
        Some(*y.digest()),
        Vec::new(),
        meta_at(1, -1_000_000),
    )
    .expect_err("an arrow dated before the epoch");
    assert_eq!(got, Error::CreatedAtNegative { got: -1_000_000 }, "{got}");
}

/// The boundary: `>= 0` admits the epoch, and a check written `> 0` would not.
///
/// The epoch is the value `gx-gate`'s escalation ticket carries as its placeholder
/// (`crates/gx-gate/src/lib.rs`, ASM-4 keeps it out of the IdentityView), so a stricter predicate
/// here would have made that placeholder unbuildable.
#[test]
fn d6_1_the_epoch_itself_is_admitted() {
    let world = World::new(1, 2, 3);
    compose_with(&world, meta_at(1, 0)).expect("`>= 0` admits zero");
}

// ---------------------------------------------------------------------------
// ② intent_id != the all-zero placeholder
// ---------------------------------------------------------------------------

/// 「②`intent_id ≠ 全 0 placeholder`」, at `compose`.
///
/// The all-zero id is the crate's own spelling for 「no value yet」 -- `PROVISIONAL_ID` in
/// `transformation.rs` is exactly those bytes -- and 42 §1.3 puts `intent_id` **inside** the
/// IdentityView. So an arrow carrying it has an identity computed over a placeholder.
#[test]
fn d6_2_compose_refuses_the_placeholder_intent_id() {
    let world = World::new(1, 2, 3);
    let mut meta = meta_at(1, 10);
    meta.intent_id = IntentId(gx_core::Cid([0u8; 32]));
    let got = compose_with(&world, meta).expect_err("a composite accounted to no Draft");
    assert_eq!(got, Error::IntentIdUnset, "{got}");
}

/// 「②」, at `Transformation::new`.
#[test]
fn d6_2_new_refuses_the_placeholder_intent_id() {
    let x = snapshot(1, 4);
    let y = snapshot(2, 4);
    let mut meta = meta_at(1, 10);
    meta.intent_id = IntentId(gx_core::Cid([0u8; 32]));
    let got = Transformation::new(
        TransformationId(cid(5)),
        0,
        Subject::Object(*x.id()),
        Some(*y.digest()),
        Vec::new(),
        meta,
    )
    .expect_err("an arrow accounted to no Draft");
    assert_eq!(got, Error::IntentIdUnset, "{got}");
}

/// One zero byte away from the placeholder is a legitimate id, so the check is equality with the
/// placeholder rather than 「looks mostly empty」.
#[test]
fn d6_2_an_almost_zero_intent_id_is_admitted() {
    let world = World::new(1, 2, 3);
    let mut meta = meta_at(1, 10);
    let mut raw = [0u8; 32];
    raw[31] = 1;
    meta.intent_id = IntentId(gx_core::Cid(raw));
    compose_with(&world, meta).expect("only the all-zero value is the placeholder");
}

// ---------------------------------------------------------------------------
// ③ created_at >= max(f, g) -- compose only
// ---------------------------------------------------------------------------

/// 「③`created_at ≥ max(f,g)`(compose のみ)」, with `f` holding the maximum.
#[test]
fn d6_3_compose_refuses_a_composite_dated_before_f() {
    let x = snapshot(1, 4);
    let y = snapshot(2, 4);
    let z = snapshot(3, 4);
    let f = arrow_at(10, &x, &y, 500);
    let g = arrow_at(20, &y, &z, 100);
    let world = World { x, y, z, f, g };
    let got = compose_with(&world, meta_at(1, 499))
        .expect_err("the composite predates the arrow it is made of");
    assert_eq!(
        got,
        Error::CreatedAtBeforeParts {
            got: 499,
            at_least: 500
        },
        "{got}"
    );
}

/// The same, with `g` holding the maximum -- so the check is `max` and not `f.created_at`.
#[test]
fn d6_3_compose_refuses_a_composite_dated_before_g() {
    let x = snapshot(1, 4);
    let y = snapshot(2, 4);
    let z = snapshot(3, 4);
    let f = arrow_at(10, &x, &y, 100);
    let g = arrow_at(20, &y, &z, 700);
    let world = World { x, y, z, f, g };
    let got =
        compose_with(&world, meta_at(1, 699)).expect_err("the composite predates the second arrow");
    assert_eq!(
        got,
        Error::CreatedAtBeforeParts {
            got: 699,
            at_least: 700
        },
        "{got}"
    );
}

/// The boundary: `>=` admits the tie. Two arrows and their composite recorded in the same
/// nanosecond is a machine writing three values in one tick, not a violation.
#[test]
fn d6_3_the_tie_is_admitted() {
    let x = snapshot(1, 4);
    let y = snapshot(2, 4);
    let z = snapshot(3, 4);
    let f = arrow_at(10, &x, &y, 700);
    let g = arrow_at(20, &y, &z, 700);
    let world = World { x, y, z, f, g };
    compose_with(&world, meta_at(1, 700)).expect("`>=` admits equality");
}

/// ③ is `compose`'s and not `Transformation::new`'s, because `new` has no parts to be before.
///
/// A hand-built arrow may be dated before the arrows in its `parents` list: those ids name values
/// this crate cannot resolve (41 §6 forbids the I/O that would), so `new` cannot evaluate the
/// predicate at all. Composition can, because it holds `f` and `g`.
#[test]
fn d6_3_is_not_applied_by_new_which_has_no_parts_to_be_before() {
    let x = snapshot(1, 4);
    let y = snapshot(2, 4);
    Transformation::new(
        TransformationId(cid(5)),
        0,
        Subject::Object(*x.id()),
        Some(*y.digest()),
        vec![TransformationId(cid(77))],
        meta_at(1, 1),
    )
    .expect("`new` checks ① and ② and stops there");
}

// ---------------------------------------------------------------------------
// What was ruled OUT, kept as a check so that it stays out
// ---------------------------------------------------------------------------

/// **④ was not adopted**, and this is the probe that says so mechanically.
///
/// req/38 §22 逐語: 「**④(intent ∈ {f,g})は不採用**——`CompositionMetadata` の doc 自身が「両者が別
/// intent から来うる」と書き、合成物が新しい intent を持つ読みを spec は禁じていない」.
#[test]
fn the_composite_may_carry_an_intent_neither_part_carries() {
    let world = World::new(1, 2, 3);
    let mut meta = meta_at(1, 10);
    meta.intent_id = IntentId(cid(200));
    assert_ne!(meta.intent_id, world.f.intent_id);
    assert_ne!(meta.intent_id, world.g.intent_id);
    compose_with(&world, meta).expect("④ is not implemented, and must not be");
}

/// 「『未来でない』は時計=M5」 -- a far-future timestamp is admitted here.
///
/// Not an oversight: deciding it needs `now`, and 41 §6 forbids this crate the clock that would
/// answer. The ruling routes it to M5, where the clock is injected at the engine boundary.
#[test]
fn a_far_future_created_at_is_not_this_crates_business() {
    let world = World::new(1, 2, 3);
    compose_with(&world, meta_at(1, i64::MAX)).expect("「未来でない」 is M5's, not this crate's");
}

// ---------------------------------------------------------------------------
// The third door, closed by E-M3-18
// ---------------------------------------------------------------------------

/// `identity` takes the same metadata and now refuses what the other two doors refuse.
///
/// **This probe is the pin rewritten, not a new one.** M3 hand 6 wrote
/// `identity_is_a_third_door_and_this_hand_did_not_close_it`, which asserted the *opposite* -- that
/// `identity` builds a value `compose` and `new` refuse -- on the discipline that a known
/// discrepancy is pinned rather than hidden, 「so that changing it is a deliberate act」. `req/38`
/// §25 H-1 is the deliberate act (**E-M3-18**), so the assertion is inverted here rather than
/// deleted: the same fixture, the same two out-of-range fields, and the answer that changed.
///
/// The wider claim -- that no door anywhere in the crate is still open -- is not this probe's.
/// `value_range_closure.rs` reads the source for that, because a hand adding a fourth door would
/// not think to come here.
#[test]
fn identity_is_the_third_door_and_e_m3_18_closed_it() {
    let x = snapshot(1, 4);
    let mut meta = meta_at(1, -1);
    meta.intent_id = IntentId(gx_core::Cid([0u8; 32]));
    let got = identity(&x, meta, |_| TransformationId(cid(5)))
        .expect_err("both ① and ② are violated by this metadata");
    assert_eq!(
        got,
        Error::CreatedAtNegative { got: -1 },
        "① is checked before ②, at every door, because `in_range` fixes the order once (E-M3-13)"
    );

    let mut only_intent = meta_at(1, 0);
    only_intent.intent_id = IntentId(gx_core::Cid([0u8; 32]));
    assert_eq!(
        identity(&x, only_intent, |_| TransformationId(cid(5))).expect_err("② alone"),
        Error::IntentIdUnset
    );
}

// ---------------------------------------------------------------------------
// The vocabulary this hand added to
// ---------------------------------------------------------------------------

/// gx-core's `Error` has eleven variants and a match without `_` is what keeps the count honest.
///
/// Eight when M3 hand 6 wrote this -- five, and the three E-M3-13 added -- nine since M4 hand 1
/// added `FingerprintScopeMismatch` for **E-M4-15**, ten since M4 hand 2 added
/// `FingerprintSubstrateMismatch` for **E-M4-27**, and eleven since M4 hand 4 added `ScopeTooLong`
/// for **M4H1-2**. The name of the probe carries the number and is
/// therefore a claim that goes stale on every arrival; it has now been rewritten twice, which is the
/// point of putting a number in a name at all -- the edit is forced and visible rather than
/// optional.
///
/// req/66 §4 raised that gx-core had no `ERROR_KINDS` table beside this compile-time half, and
/// `req/38` §25 H-3 sent it to this milestone. It exists now: `gx_core::ERROR_KINDS`, reconciled
/// against the enum in `crates/gx-core/tests/core_error_vocabulary.rs`. The two instruments answer
/// different questions and both are kept -- this one fails to **compile** when a variant appears,
/// that one fails to **pass** when the table and the enum disagree, and a hand that added a variant
/// and its table row while forgetting a `kind` arm would be caught by the first alone.
#[test]
fn the_error_vocabulary_is_a_closed_enum_of_eleven_variants() {
    let all = [
        Error::OrderExceeded { got: 3, max: 2 },
        Error::CidText {
            detail: String::new(),
        },
        Error::NotComposable,
        Error::SubjectUnresolved,
        Error::TargetMissing,
        Error::CreatedAtNegative { got: -1 },
        Error::IntentIdUnset,
        Error::CreatedAtBeforeParts {
            got: 0,
            at_least: 1,
        },
        Error::FingerprintScopeMismatch {
            left: String::new(),
            right: String::from("other"),
        },
        Error::FingerprintSubstrateMismatch {
            left: gx_core::SubstrateKind::Fs,
            right: gx_core::SubstrateKind::Git,
        },
        Error::ScopeTooLong {
            bytes: gx_core::MAX_SCOPE_BYTES + 1,
            max: gx_core::MAX_SCOPE_BYTES,
        },
    ];
    // The array holds one value per variant; the set below is what makes that a claim rather than
    // an arrangement, since two entries spelling the same variant would collapse in it.
    let mut kinds = std::collections::BTreeSet::new();
    for e in &all {
        // No `_` arm: a new variant is a compile error here, which is the whole of the check.
        let name = match e {
            Error::OrderExceeded { .. } => "OrderExceeded",
            Error::CidText { .. } => "CidText",
            Error::NotComposable => "NotComposable",
            Error::SubjectUnresolved => "SubjectUnresolved",
            Error::TargetMissing => "TargetMissing",
            Error::CreatedAtNegative { .. } => "CreatedAtNegative",
            Error::IntentIdUnset => "IntentIdUnset",
            Error::CreatedAtBeforeParts { .. } => "CreatedAtBeforeParts",
            Error::FingerprintScopeMismatch { .. } => "FingerprintScopeMismatch",
            Error::FingerprintSubstrateMismatch { .. } => "FingerprintSubstrateMismatch",
            Error::ScopeTooLong { .. } => "ScopeTooLong",
        };
        kinds.insert(name);
        // The table is the other declaration of this same list, so the two are joined here as
        // well as in `core_error_vocabulary.rs`: a name spelled one way in the match and another
        // in the table would otherwise be two vocabularies that never meet.
        assert_eq!(name, e.kind(), "`Error::kind` disagrees with this match");
    }
    assert_eq!(
        kinds.len(),
        11,
        "gx-core's Error variants, by name: {kinds:?}"
    );
    assert_eq!(
        kinds.len(),
        gx_core::ERROR_KINDS.len(),
        "the enum and ERROR_KINDS hold different numbers of names"
    );
    println!("GX_CORE_ERROR_VARIANTS={} ({kinds:?})", kinds.len());
}
