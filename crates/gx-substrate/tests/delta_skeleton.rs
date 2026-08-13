//! The two delta values of 42 §3.4, and the three rulings that shape their constructors.
//!
//! | ruling | what it fixes | what is measured here |
//! |---|---|---|
//! | **M4-17** | 「`applied_at` は engine 注入(`AppliedDelta` 構成子が `Timestamp` を引数に取る)」 | the constructor takes the moment, and this crate names no clock |
//! | **E-M4-15** | `Fingerprint` has no `PartialEq` | `AppliedDelta` has none either, so nothing compares two applications by `==` |
//! | **E-M4-11** | `GateInput.planned` stays `PlannedDeltaBytes` | no gate crate is named from here, and this crate names no gate |
//!
//! What was **not** measured here, and now is measured next door: that `reference` is the CID of
//! `{substrate, payload}` (42 §3.4). Hand 1 could not check it without the projection and wrote the
//! gap down (req/70 §3 M4H1-3); §29 ruled case (a), so [`PlannedDelta::new`] mints the reference and
//! `crates/gx-substrate/tests/planned_delta_identity.rs` holds the projection to the **I-1** form.
//! This file therefore stops constructing a `reference` and starts reading the one that was minted --
//! two existing probes were updated rather than deleted, which is the §21 C-9 / §29 M4H1-9 shape and
//! is raised for追認 in req/72 §2.

use gx_core::{Cid, DeltaRef, Fingerprint, SubstrateKind, Timestamp};
use gx_substrate::{AppliedDelta, PlannedDelta};

fn cid_of(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn reference(seed: u8) -> DeltaRef {
    DeltaRef {
        substrate: SubstrateKind::Fs,
        cid: cid_of(seed),
    }
}

fn planned(payload: &[u8]) -> PlannedDelta {
    PlannedDelta::new(SubstrateKind::Fs, payload.to_vec())
        .expect("a delta over a byte payload has a canonical form")
}

/// The three fields of 42 §3.4, read back through the accessors F-6 asks for.
///
/// The third is now derived rather than supplied (**M4H1-3** 採(a)), so what this asserts about it is
/// what 42 §3.4 asks of it: the substrate agrees with the delta's own, and the digest is not the
/// all-zero placeholder the constructor projects through. Whether the digest is the *right* one is
/// `planned_delta_identity.rs`'s question, where the projection is the subject.
#[test]
fn a_planned_delta_is_the_three_fields_of_42_3_4() {
    let delta = planned(b"replace /tmp/x");
    assert_eq!(delta.substrate(), &SubstrateKind::Fs);
    assert_eq!(delta.payload(), b"replace /tmp/x");
    assert_eq!(delta.reference().substrate, SubstrateKind::Fs);
    assert_ne!(
        delta.reference().cid,
        Cid([0u8; 32]),
        "the reference still carries the placeholder `new` starts from"
    );
}

/// Two payloads that differ are two deltas; the type compares by value.
///
/// `PlannedDelta` may derive `PartialEq` where `AppliedDelta` may not, and the difference is worth
/// stating: 42 §1.3 gives this type an IdentityView over `{substrate, payload}` and equality of
/// those fields is equality of the value. `Fingerprint` is the type whose equality has a third
/// answer (E-M4-15), and it is not in this struct.
///
/// Now that `reference` is minted, `==` over all three fields and agreement of the projection are
/// the same relation -- which is the sentence 42 §3.4's 「自身のcanonical参照」 was always making.
#[test]
fn two_payloads_are_two_planned_deltas() {
    assert_eq!(planned(b"a"), planned(b"a"));
    assert_ne!(planned(b"a"), planned(b"b"));
}

/// The four fields of 42 §3.4, with the moment supplied rather than found (**M4-17**).
///
/// 41 §6 逐語: 「乱数・時刻はengine境界で注入（決定的リプレイのため）」. The constructor's signature is
/// the enforcement: an adapter that wanted to date its own record would have to be handed a
/// `Timestamp` first, and there is nowhere in this crate to get one from.
#[test]
fn an_applied_delta_takes_the_moment_it_is_dated_with() {
    let postcondition = Fingerprint::new(SubstrateKind::Fs, "/tmp/x".to_string(), cid_of(9))
        .expect("a short scope is inside M4H1-2's bound");
    let applied = AppliedDelta::new(reference(1), postcondition, cid_of(3), Timestamp(1_700));

    assert_eq!(applied.delta(), &reference(1));
    assert_eq!(applied.postcondition().scope(), "/tmp/x");
    assert_eq!(applied.resulting_digest(), &cid_of(3));
    assert_eq!(applied.applied_at(), Timestamp(1_700));
}

/// Replaying the same application with the same injected moment gives the same record.
///
/// The half that makes M4-17 worth a ruling rather than a preference: FR-039's deterministic replay
/// is only possible if nothing inside the adapter varies with when it ran. There is no `==` on
/// `AppliedDelta` (E-M4-15), so the comparison is field by field, which is also the honest way to
/// say what "the same record" means while the fingerprint's own equality has three answers.
#[test]
fn the_same_application_replayed_with_the_same_moment_records_the_same_thing() {
    let build = || {
        AppliedDelta::new(
            reference(1),
            Fingerprint::new(SubstrateKind::Fs, "/tmp/x".to_string(), cid_of(9))
                .expect("a short scope is inside M4H1-2's bound"),
            cid_of(3),
            Timestamp(1_700),
        )
    };
    let first = build();
    let second = build();

    assert_eq!(first.delta(), second.delta());
    assert_eq!(first.resulting_digest(), second.resulting_digest());
    assert_eq!(first.applied_at(), second.applied_at());
    assert!(first
        .postcondition()
        .cas_eq(second.postcondition())
        .expect("one scope, so the comparison has a meaning"));
}

/// A different moment is a different record and the same state.
///
/// Dating is metadata (ASM-4) and the fingerprint is not, so the two answers differ -- which is the
/// whole reason 42 §1.3 excludes timestamps from every IdentityView.
#[test]
fn the_moment_is_metadata_and_the_fingerprint_is_not() {
    let postcondition = || {
        Fingerprint::new(SubstrateKind::Fs, "/tmp/x".to_string(), cid_of(9))
            .expect("a short scope is inside M4H1-2's bound")
    };
    let early = AppliedDelta::new(reference(1), postcondition(), cid_of(3), Timestamp(1));
    let late = AppliedDelta::new(reference(1), postcondition(), cid_of(3), Timestamp(2));

    assert_ne!(early.applied_at(), late.applied_at());
    assert!(early
        .postcondition()
        .cas_eq(late.postcondition())
        .expect("one scope"));
}
