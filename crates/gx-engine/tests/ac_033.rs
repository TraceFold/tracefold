// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-033 (FR-033) — `canonicalize` checks T3 first, and refuses when it does not hold.
//!
//! 34 AC-033, verbatim (sem: SEM-gx-engine-427): "Given: T in the `Admitted` state. When: the
//! `canonicalize` step runs. Then: it checks canon idempotency (T3) and then transitions to the
//! `Canonicalized` state. **In the abnormal case where a broken canon implementation that returns
//! an idempotency violation is injected, it returns an error and does not transition to
//! `Canonicalized`**." unit (normal case + abnormal-case injection)
//!
//! # The abnormal case is what makes the normal one mean anything
//!
//! T3 (`canon(canon(x)) = canon(x)`, 12 F0) holds of gx-canon, and `gx-canon/tests/ac_012.rs`
//! already proves it as a property. So a `canonicalize` that never checked would pass every normal
//! case forever. The criterion's second sentence is the one with content: it asks that the engine
//! *would notice*, which can only be shown by giving it an encoder that fails.
//!
//! # 🔴 What is injectable, and what is not (41 §6)
//!
//! 41 §6: "every canonical encode goes through gx-canon only (no bypass allowed)" (sem:
//! SEM-gx-engine-428). That and "inject a broken canon implementation"
//! cannot both be about the same road, so they are not: the `canonical_cid` this engine journals
//! always comes from `gx_canon::cid::compute`, whatever the injected [`Canonicalizer`] says, and
//! what is injected is the **bytes T-8's guard runs over**. A broken canonicalizer can make the
//! engine refuse; it cannot make the engine mint a CID gx-canon did not compute. That separation is
//! measured below by [`ac_033_a_broken_canon_cannot_change_a_canonical_cid`], and raised as
//! **M5H2-4** because the tension is worth a ruling even though it resolves.

mod support;

use std::sync::Arc;

use gx_core::{EnforcementMode, Timestamp};
use gx_engine::{Engine, EngineJournalRecord, EvidenceSource, InjectedEvidence, Lifecycle};
use support::{
    gate, intent, scratch, signing_key, BrokenCanon, StubAdapter, FORBID_ETC, PERMIT_ALL,
};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

fn engine(name: &str, policies: &str) -> Engine<InjectedEvidence> {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(policies),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal");
    engine.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
    engine
}

/// `submit` → `plan` → `verify`, leaving the transformation wherever the gate put it.
fn upto_verify<E: EvidenceSource, C: gx_engine::Canonicalizer>(
    e: &mut Engine<E, C>,
    locator: &str,
) -> (gx_core::TransformationId, Lifecycle) {
    let i = intent(locator, "v1");
    e.submit(&i, 42, AT).expect("submit");
    let id = e.plan(&i, AT).expect("plan");
    let state = e.verify(&id, AT, &signing_key(), None).expect("verify");
    (id, state)
}

// ---------------------------------------------------------------------------
// The normal case (T-8)
// ---------------------------------------------------------------------------

/// An `Admitted` transformation canonicalises, and the CID is gx-canon's.
#[test]
fn ac_033_an_admitted_transformation_canonicalises() {
    let mut e = engine("ac033_normal", PERMIT_ALL);
    let (id, state) = upto_verify(&mut e, "/tmp/x");
    assert_eq!(state, Lifecycle::Admitted);

    let to = e.canonicalize(&id, AT, None).expect("T-8");
    assert_eq!(to, Lifecycle::Canonicalized);
    assert_eq!(e.state(&id), Some(Lifecycle::Canonicalized));

    let cid = e.canonical_cid(&id).expect("T-8 fixes a canonical CID");
    let expected = gx_canon::cid::compute(e.transformation(&id).expect("the row"))
        .expect("the transformation has a canonical form");
    assert_eq!(cid, expected, "the CID is gx-canon's, over the value");

    let recorded = e
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            EngineJournalRecord::Canonicalized {
                transformation,
                canonical_cid,
                enforced,
                ..
            } if *transformation == id => Some((*canonical_cid, *enforced)),
            _ => None,
        })
        .expect("T-8 wrote a Canonicalized record");
    assert_eq!(recorded.0, cid);
    assert_eq!(
        recorded.1, None,
        "an enforced transformation records no `enforced` flag; 42 §3.13 gives it to T-8r"
    );
}

/// 43 T-8's idempotency column: "recomputing gives the same canon_cid" (sem: SEM-gx-engine-429),
/// and no second record.
#[test]
fn ac_033_canonicalising_twice_is_the_same_cid_and_one_record() {
    let mut e = engine("ac033_idempotent", PERMIT_ALL);
    let (id, _) = upto_verify(&mut e, "/tmp/x");

    e.canonicalize(&id, AT, None).expect("T-8");
    let first = e.canonical_cid(&id).expect("fixed");
    let records = e.journal().len();

    e.canonicalize(&id, AT, None).expect("again");
    assert_eq!(e.canonical_cid(&id), Some(first), "the same canon_cid");
    assert_eq!(
        e.journal().len(),
        records,
        "and no second event: a value already fixed is not a transition"
    );
}

// ---------------------------------------------------------------------------
// 🔴 The abnormal case ("inject a broken canon implementation"; sem: SEM-gx-engine-430)
// ---------------------------------------------------------------------------

/// A canonicalizer whose output is not a fixed point is refused, and nothing moves.
///
/// `BrokenCanon` answers with a DAG-CBOR map whose two keys are out of order. 42 §2.1-2 sorts map
/// keys bytewise, so those bytes are ones gx-canon would not have written: re-encoding them yields a
/// different string, which is `canon(canon(x)) != canon(x)` exactly. It is a real defect and not a
/// sentinel value the engine was taught to recognise.
///
/// Three things are asserted, and the last two are the criterion: an error came back, the state did
/// **not** become `Canonicalized`, and the journal did not grow. A transition that refused loudly
/// but had already appended its record would leave a replay believing a canonicalisation happened.
#[test]
fn ac_033_a_broken_canon_is_refused_and_nothing_transitions() {
    let dir = scratch("ac033_broken");
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal")
    .with_canonicalizer(BrokenCanon);
    e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

    let (id, state) = upto_verify(&mut e, "/tmp/x");
    assert_eq!(state, Lifecycle::Admitted);
    let before = e.journal().len();

    let refused = e
        .canonicalize(&id, AT, None)
        .expect_err("T3 does not hold of these bytes");
    assert_eq!(refused.kind(), "NotIdempotent", "{refused}");
    assert_eq!(
        e.state(&id),
        Some(Lifecycle::Admitted),
        "\"does not transition to Canonicalized\" (sem: SEM-gx-engine-431)"
    );
    assert_eq!(e.canonical_cid(&id), None, "nothing was fixed");
    assert_eq!(
        e.journal().len(),
        before,
        "and the check ran before the write, so a replay sees no canonicalisation"
    );
}

/// 🔴 41 §6 survives the injection: a broken canonicalizer cannot move a canonical CID.
///
/// The same transformation is canonicalised twice, once through each canonicalizer, and the CID the
/// working one produces is compared with `gx_canon::cid::compute`. If the injected encoder were on
/// the identity road, the broken engine would either mint a different CID or mint one at all -- it
/// does neither, because the only thing it can reach is the guard.
#[test]
fn ac_033_a_broken_canon_cannot_change_a_canonical_cid() {
    let mut good = engine("ac033_good_cid", PERMIT_ALL);
    let (id, _) = upto_verify(&mut good, "/tmp/x");
    good.canonicalize(&id, AT, None).expect("T-8");
    let honest = good.canonical_cid(&id).expect("fixed");

    let dir = scratch("ac033_broken_cid");
    let mut broken = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal")
    .with_canonicalizer(BrokenCanon);
    broken.register_adapter(Arc::new(StubAdapter::default()), "stub-1");
    let (broken_id, _) = upto_verify(&mut broken, "/tmp/x");

    assert_eq!(
        broken_id, id,
        "the ids agree: a canonicalizer is not on the identity road at all (41 §6)"
    );
    assert!(broken.canonicalize(&broken_id, AT, None).is_err());
    assert_eq!(
        broken.canonical_cid(&broken_id),
        None,
        "a broken canon mints nothing rather than minting something wrong"
    );
    assert_eq!(
        honest,
        gx_canon::cid::compute(good.transformation(&id).expect("the row")).expect("canonical")
    );
}

// ---------------------------------------------------------------------------
// T-8r, and the state guard
// ---------------------------------------------------------------------------

/// **T-8r**: a `Denied` transformation canonicalises under `RecordOnly`, carrying `enforced=false`.
///
/// 43 T-8r: "the same processing as T-8, plus stamping the `enforced=false` flag into the
/// Transformation's accompanying metadata; journal: `Canonicalized{id, canon_cid,
/// enforced=false}`" (sem: SEM-gx-engine-432). This is what DR-2's record-only mode is for --
/// "it was applied, but policy had denied it" stays third-party checkable (43 §4, P-7).
#[test]
fn t_8r_a_denied_transformation_canonicalises_under_record_only() {
    let dir = scratch("ac033_t8r");
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(FORBID_ETC),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal")
    .with_mode(EnforcementMode::RecordOnly);
    e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

    let (id, state) = upto_verify(&mut e, "/etc/passwd");
    assert_eq!(state, Lifecycle::Denied);

    let to = e.canonicalize(&id, AT, None).expect("T-8r");
    assert_eq!(to, Lifecycle::Canonicalized);
    assert_eq!(e.enforced(&id), Some(false), "43 T-8r's flag");

    let recorded = e
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            EngineJournalRecord::Canonicalized {
                transformation,
                enforced,
                ..
            } if *transformation == id => Some(*enforced),
            _ => None,
        })
        .expect("T-8r wrote a Canonicalized record");
    assert_eq!(
        recorded,
        Some(false),
        "\"enforced=false\", in the journal (sem: SEM-gx-engine-433)"
    );
}

/// Under `Enforce`, 43 §1's "`Denied` is terminal" (sem: SEM-gx-engine-434) stands and T-8r does not fire.
#[test]
fn t_8r_does_not_fire_when_the_mode_is_enforce() {
    let mut e = engine("ac033_denied_terminal", FORBID_ETC);
    assert_eq!(e.mode(), EnforcementMode::Enforce, "DR-2's default");
    let (id, state) = upto_verify(&mut e, "/etc/passwd");
    assert_eq!(state, Lifecycle::Denied);

    let refused = e
        .canonicalize(&id, AT, None)
        .expect_err("Denied is terminal outside RecordOnly");
    assert_eq!(refused.kind(), "InvalidState", "{refused}");
    assert_eq!(e.state(&id), Some(Lifecycle::Denied));
}

/// 🔴 **M5H2-3**: `enforced = Some(false)` is reachable from **T-8**, not only from T-8r.
///
/// 42 §3.13 annotates the record "only T-8r has enforced=Some(false)" (sem: SEM-gx-engine-435).
/// 43 §4 disagrees by construction: a
/// **T-4e** transformation is `Admitted` and degraded to "the record-only-mode equivalent", so it reaches
/// canonicalisation through T-8 while carrying `enforced=false`. Writing `None` there to satisfy
/// 42's parenthetical would erase the fact INV-S5 requires to be visible.
///
/// The flag follows the transformation, not the transition, and this is the case that proves the two
/// readings differ.
#[test]
fn m5h2_3_a_degraded_admission_canonicalises_through_t_8_with_enforced_false() {
    let dir = scratch("ac033_t4e_t8");
    let mut e = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        gx_engine::UnreachableEvidence::new("the collector is down"),
    )
    .expect("a fresh journal")
    .with_posture(gx_core::FailPosture::FailOpen);
    e.register_adapter(Arc::new(StubAdapter::default()), "stub-1");

    let (id, state) = upto_verify(&mut e, "/tmp/x");
    assert_eq!(state, Lifecycle::Admitted, "T-4e admitted it, degraded");
    assert_eq!(e.enforced(&id), Some(false));

    e.canonicalize(&id, AT, None).expect("T-8, not T-8r");
    let recorded = e
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            EngineJournalRecord::Canonicalized {
                transformation,
                enforced,
                ..
            } if *transformation == id => Some(*enforced),
            _ => None,
        })
        .expect("a Canonicalized record");
    assert_eq!(
        recorded,
        Some(false),
        "T-8 wrote `enforced=Some(false)`, which 42 §3.13's parenthetical says only T-8r does \
         (M5H2-3)"
    );
}

// ---------------------------------------------------------------------------
// M5H8-11 — the return value, not just the road
// ---------------------------------------------------------------------------

/// 🔴 **M5H8-11, adopted (b)** (`req/38_ERRATA_2026-08-07.md` §45), verbatim (sem: SEM-gx-engine-436):
///
/// > **M5H8-11, adopted (b)**: on the engine side, a return-value check that "`canonical_cid`
/// > matches `gx_canon::cid::compute`'s output" is added as a **fix batch**. To 41 §6's verbatim
/// > "every canonical encode goes through gx-canon only", this adds the **return-value** face to
/// > the road check (§39 M5H2-4). The grounds are the measured fact that "a constant function is
/// > idempotent" is the T-8 guard's blind spot (survivors 4, 5). (sem: SEM-gx-engine-436)
///
/// # The blind spot, stated exactly
///
/// req/86 §3.3 mutated [`gx_engine::CanonEncoder::canonical_form`] into `Ok(vec![0])` and
/// `Ok(vec![1])` and **the whole gx-engine suite stayed green**. The reason is structural rather
/// than an oversight: 43 T-8's guard is T3 (`canon(canon(x)) = canon(x)`), and a constant function
/// satisfies T3 perfectly. AC-033 above measures the contrapositive's *other* side — an encoder
/// whose output gx-canon would not have written — and a one-byte integer **is** canonical DAG-CBOR,
/// so `is_canonical` waves it through. The value of the canonical form was pinned only in
/// gx-canon's own golden vectors, on the other side of a crate boundary the engine never crossed in
/// a probe.
///
/// # What is checked here, in three links
///
/// 1. the shipping encoder returns exactly `gx_canon::cbor::encode(t.identity_view())` — the bytes
///    `gx_canon::cid::compute` hashes, so this is "gx-canon's answer was used" (sem: SEM-gx-engine-437) as a byte string;
/// 2. the CID the engine journalled equals `gx_canon::cid::compute(&t)` — the road check §39
///    M5H2-4 already confirmed and implied (sem: SEM-gx-engine-437), now stated as an equality rather than as an absence of a second
///    call site;
/// 3. both against **literals taken from the tree**, hand 6's golden-vector shape: a golden that is
///    regenerated records what the code does, a golden carried in the source records what a change
///    did not do. If these two lines ever move, every `TransformationId` this project has ever
///    issued has moved with them.
///
/// # No src change was needed and none was made
///
/// The ruling's "return-value check" (sem: SEM-gx-engine-438) is satisfiable from outside: the probe calls gx-canon independently
/// and compares. Putting the same comparison **inside** `canonicalize` would have made the engine
/// re-encode every transformation twice on the commit path, and would have made AC-033's injected
/// broken canon fail with a different error than the one 34 asks for.
#[test]
fn m5h8_11_the_canonical_form_is_the_one_gx_canon_would_have_written() {
    use gx_canon::cid::IdentityView;
    use gx_engine::{CanonEncoder, Canonicalizer};

    let mut e = engine("m5h8_11_return_value", PERMIT_ALL);
    let (id, state) = upto_verify(&mut e, "/tmp/x");
    assert_eq!(state, Lifecycle::Admitted);
    e.canonicalize(&id, AT, None).expect("T-8");

    let t = e.transformation(&id).cloned().expect("the engine holds it");

    // (1) the bytes.
    let shipped = CanonEncoder
        .canonical_form(&t)
        .expect("the shipping encoder has a canonical form for a submitted transformation");
    let from_gx_canon = gx_canon::cbor::encode(&t.identity_view()).expect("gx-canon encodes it");
    let hex: String = shipped.iter().map(|b| format!("{b:02x}")).collect();
    println!("CANONICAL_FORM_LEN={} HEX={hex}", shipped.len());
    assert_eq!(
        shipped, from_gx_canon,
        "`CanonEncoder::canonical_form` returned bytes gx-canon did not write. 41 §6 says every \
         canonical encode goes through gx-canon; a road that goes there and discards the answer \
         satisfies the road check and breaks the sentence"
    );

    // (2) the identity.
    let journalled = e.canonical_cid(&id).expect("T-8 fixed it");
    let independent = gx_canon::cid::compute(&t).expect("gx-canon computes it");
    println!("CANONICAL_CID={}", gx_canon::cid::to_text(&journalled));
    assert_eq!(
        journalled, independent,
        "the journalled canonical_cid is not gx_canon::cid::compute's output"
    );
    assert_eq!(
        journalled, id.0,
        "42 §3.10: `canonical_cid` is `Transformation.id`"
    );

    // (3) the goldens, as literals.
    assert_eq!(
        hex, GOLDEN_CANONICAL_FORM_HEX,
        "the canonical form of this fixture moved; every TransformationId ever issued moved with it"
    );
    assert_eq!(
        gx_canon::cid::to_text(&journalled),
        GOLDEN_CANONICAL_CID,
        "the CID of this fixture moved"
    );
}

/// `canonical_dagcbor(intent("/tmp/x","v1") planned at seed 42)`'s identity view, from the tree.
const GOLDEN_CANONICAL_FORM_HEX: &str = concat!(
    "a8656163746f72a1654167656e74a2636b65796b6b65792d6167656e742d31656d6f64656c6e636c61756465",
    "2d6661626c652d356564656c7461a2636369645820a82c6025effa39dd4316dc195a6f08161b3d6c0669614b",
    "209643223b0a6a4abf69737562737472617465624673656f726465720066746172676574f667636f6e746578",
    "7466506f6c69637967706172656e747380677375626a656374a1664f626a65637458202f746d702f78000000",
    "000000000000000000000000000000000000000000000069696e74656e745f6964582039c7d3b47a069cb011",
    "f2783384a3c3cd3aeae93802d960d61b6e3b00ff70880e",
);

/// `gx_canon::cid::to_text` of the same value, from the tree.
const GOLDEN_CANONICAL_CID: &str = "gx1:w7gnjwdjxmfhxkjwbzwubwcoz74yf6eudeyyr27pyvehwivqgfdq";
