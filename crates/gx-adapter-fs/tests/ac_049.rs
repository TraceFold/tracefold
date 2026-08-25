// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-049 (FR-044) — the round trip, in three kinds, on a tmpfs.
//!
//! AC-049, verbatim: "Given: fs-adapter with a delta from one of the 3 kinds -- create/change/delete a file. When: for
//! each, run `apply(delta)` -> take the resulting inverse delta and run `apply(inverse)`. Then: the target
//! file's content returns bit-equal to the byte sequence before the operation (3 cases)." Judgment method: "integration (on tmpfs)", M4. (sem: SEM-gx-adapter-fs-113)
//!
//! # 🔴 The order is T-10b's, and the verbatim text is the one that was corrected (sem: SEM-gx-adapter-fs-114)
//!
//! The criterion reads `apply` -> "the resulting inverse delta" -> `apply`, with the inverse obtained after the
//! change. **E-M4-30** (req/38 §31 M4H3-1, adopted (a)) is the erratum: (sem: SEM-gx-adapter-fs-115)
//!
//! > "the erratum over the cell of 51 §7 contract 4 and AC-049's verbatim order -- **escrow (invert) comes before apply**
//! > (43 T-10b is state machine's canonical source). The reason is physical: the inverse of an overwrite/deletion carries the old content's body (42 §5's reason the escrow is required), so
//! > **invert can only be constructed at the point where pre is observable** -- T-10b's order is the only constructible one, and
//! > only an adapter that keeps its own history could satisfy the verbatim order (the requirement exists nowhere)" (sem: SEM-gx-adapter-fs-116)
//!
//! This adapter keeps no history, so the inverse is built while the old bytes are still on the
//! filesystem. Every case below prints `ORDER=T-10b`.
//!
//! # The Given is written down, because **E-M4-3** made that a condition
//!
//! "the Given of the round-trip property carrying state is made a DoD condition". The quantifier of the round trip is "the one `pre`
//! passed to `invert`" and not every state: req/69 §3.2 shows in three lines that reading the round
//! trip and 41 §4's "apply is idempotent" as laws over a whole state map forces every delta to be the identity. (sem: SEM-gx-adapter-fs-117)
//! So each case names the bytes it starts from, in the assertion message and in a printed line, and
//! the property at the end generates **contents** rather than moving the state during a trip.
//!
//! # What tmpfs makes this evidence about
//!
//! The primaries were fetched in full by this hand (`Desktop/GitRepo/REFERENCES.md`, 2026-08-09).
//! POSIX's normative sentence for `rename` is about **concurrent observation** -- "a directory entry
//! named new shall remain visible to other threads throughout the renaming operation and refer either
//! to the file referred to by new or old before the operation began" -- and its RATIONALE is where the
//! word "atomic" appears. On a tmpfs `fsync` is effectively free, so **this suite is evidence about (sem: SEM-gx-adapter-fs-118)
//! atomicity and not about crash durability**, and the crate root says so in the same words rather
//! than leaving a reader to assume it.

mod support;

use gx_adapter_fs::FsAdapter;
use gx_substrate::SubstrateAdapter;
use proptest::prelude::*;
use support::{
    absent_snapshot, content_at, creation, intent_for, planned, removal, snapshot_of, Sandbox,
    BEFORE, GOAL, SUBJECT,
};

/// One round trip in T-10b's order, from whatever is at `locator` now.
///
/// Returns the bytes at the position after the inverse has been applied, so a caller compares them
/// with the ones it wrote before calling. The escrow step is first, which is the whole point.
fn round_trip(
    adapter: &FsAdapter,
    locator: &str,
    delta: &gx_substrate::PlannedDelta,
    pre: &gx_core::ObjectSnapshot,
) -> Option<Vec<u8>> {
    let inverse = adapter
        .invert(delta, pre)
        .expect("invert answers")
        .into_inverse()
        .expect("the escrow ceiling is not reached by these cases");
    adapter.apply(delta).expect("the forward delta applies");
    adapter.apply(&inverse).expect("the inverse applies");
    content_at(locator)
}

/// Change -- the file exists, the delta replaces it, and the old bytes come back. (sem: SEM-gx-adapter-fs-119)
#[test]
fn a_change_comes_back_to_the_bytes_it_started_from() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    let given = content_at(&locator).expect("the sandbox populated the subject");
    assert_eq!(given, BEFORE, "the Given of this case");
    println!(
        "AC_049_CASE=change ORDER=T-10b GIVEN_BYTES={} GIVEN={:?}",
        given.len(),
        String::from_utf8_lossy(&given)
    );

    let pre = snapshot_of(&adapter, &locator);
    let delta = planned(&adapter, &locator, GOAL);
    let after = round_trip(&adapter, &locator, &delta, &pre);

    assert_eq!(
        after.as_deref(),
        Some(given.as_slice()),
        "the round trip did not return to its Given ({:?})",
        String::from_utf8_lossy(&given)
    );
}

/// Creation -- the position holds nothing, the delta creates a file, and the position holds nothing again.
///
/// The Given is "there is no file here", which this adapter cannot express as a snapshot: `snapshot` (sem: SEM-gx-adapter-fs-120)
/// reads, and a position with nothing at it answers [`gx_substrate::Error::Unreadable`]. So the `pre`
/// is built by the test (`support::absent_snapshot`) and the seam is raised in `req/74` §2 -- an
/// engine planning a creation would hit the same wall in M5.
#[test]
fn a_creation_comes_back_to_a_position_that_holds_nothing() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator("made-by-this-test");

    let given = content_at(&locator);
    assert!(given.is_none(), "the Given of this case is an absence");
    println!("AC_049_CASE=create ORDER=T-10b GIVEN=absent");

    let pre = absent_snapshot(&locator);
    let delta = creation(&locator, b"a file that did not exist");
    let inverse = adapter
        .invert(&delta, &pre)
        .expect("invert answers for a position that holds nothing")
        .into_inverse()
        .expect("the inverse of a creation is a removal, which fits any ceiling");

    adapter.apply(&delta).expect("the creation applies");
    assert_eq!(
        content_at(&locator).as_deref(),
        Some(&b"a file that did not exist"[..]),
        "the creation did not land"
    );

    adapter.apply(&inverse).expect("the inverse applies");
    assert_eq!(
        content_at(&locator),
        None,
        "the inverse of a creation is a removal, and the Given was an absence"
    );
}

/// Deletion -- the file exists, the delta removes it, and the old bytes come back. (sem: SEM-gx-adapter-fs-121)
#[test]
fn a_removal_comes_back_to_the_bytes_it_started_from() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    let given = content_at(&locator).expect("the sandbox populated the subject");
    println!(
        "AC_049_CASE=remove ORDER=T-10b GIVEN_BYTES={} GIVEN={:?}",
        given.len(),
        String::from_utf8_lossy(&given)
    );

    let pre = snapshot_of(&adapter, &locator);
    let delta = removal(&locator);
    let inverse = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse()
        .expect("six bytes fit under the escrow ceiling");

    adapter.apply(&delta).expect("the removal applies");
    assert_eq!(
        content_at(&locator),
        None,
        "the removal did not take the file away"
    );

    adapter.apply(&inverse).expect("the inverse applies");
    assert_eq!(
        content_at(&locator).as_deref(),
        Some(given.as_slice()),
        "the round trip did not return to its Given ({:?})",
        String::from_utf8_lossy(&given)
    );
}

/// **L5's two routes agree** (**M4H3-3, adopted (a)**), measured where both of them exist. (sem: SEM-gx-adapter-fs-122)
///
/// One route derives the target from the intent's goal without touching the filesystem; the other is
/// what `apply` observed after its `rename`. The harness runs the same comparison as a law (L5) with
/// the fixture supplying the first route; this states it as a proposition of its own, and prints both
/// digests so that a reader of a CI log sees two numbers rather than a boolean.
///
/// **What it cannot catch**: both routes end in the one digest function 41 §6 admits, so a bug inside
/// `cid::mint` is invisible here. What it does catch is everything between the goal and the bytes on
/// the disk.
#[test]
fn the_promised_target_and_the_observed_digest_are_reached_by_different_roads() {
    use gx_canon::cid::{self, Domain};

    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    // Route 1: the goal alone. No filesystem call happens on this line or anywhere above it.
    let promised = cid::mint(Domain::Leaf, &[GOAL]);

    // Route 2: plan, apply, and read back what landed.
    let pre = snapshot_of(&adapter, &locator);
    let delta = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans a whole-file replacement");
    let applied = adapter.apply(&delta).expect("the delta applies");

    println!(
        "L5_ROUTE_GOAL={} L5_ROUTE_OBSERVED={} AGREE={}",
        cid::to_text(&promised),
        cid::to_text(applied.resulting_digest()),
        applied.resulting_digest() == &promised
    );
    assert_eq!(
        applied.resulting_digest(),
        &promised,
        "the plan promised one digest and the apply observed another (L5, M4-06, adopted (b)) (sem: SEM-gx-adapter-fs-123)"
    );
}

/// The observation is taken from the substrate rather than from the buffer that was written.
///
/// req/69 §3.1: "post is an observation, not a return value". The difference is not visible in a passing run -- (sem: SEM-gx-adapter-fs-124)
/// both roads give one digest when the write landed -- so it is stated as a property of the source
/// and measured by `tools/verify_m4h5.sh` mutation (d), which makes `apply` digest its own input
/// instead of reading back and shows which probes notice.
#[test]
fn the_postcondition_names_the_same_scope_the_precondition_did() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    let pre = snapshot_of(&adapter, &locator);
    let before = adapter.precondition(&pre).expect("the subject is readable");
    let delta = planned(&adapter, &locator, GOAL);
    let applied = adapter.apply(&delta).expect("the delta applies");

    assert_eq!(
        applied.postcondition().scope(),
        before.scope(),
        "a CAS check compares two fingerprints of one scope, so an apply that renamed the scope \
         would make every comparison answer 'that comparison carries no meaning' (E-M4-15) (sem: SEM-gx-adapter-fs-125)"
    );
    assert_eq!(
        applied.applied_at(),
        gx_core::Timestamp(0),
        "E-M4-31: the adapter writes a placeholder and the engine overwrites it at commit"
    );
    match before.cas_eq(applied.postcondition()) {
        Ok(false) => {}
        Ok(true) => panic!("the substrate moved and the fingerprint did not"),
        Err(e) => panic!("the two fingerprints of one scope are not comparable: {e}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(
        std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(64)
    ))]

    /// AC-049's three cases, over generated contents rather than over three chosen ones.
    ///
    /// The generator moves the **Given** and never the state during a trip, which is the distinction
    /// **E-M4-3** turned into a condition: a property whose generator writes to the substrate between
    /// `apply(δ)` and `apply(δ⁻¹)` falsifies a correct adapter, and that is M3-05 one milestone later.
    /// Here each case starts from its own sandbox, the Given is written into the failure message, and
    /// nothing touches the position between the two applications.
    #[test]
    fn a_round_trip_returns_to_whatever_it_started_from(
        given in proptest::collection::vec(any::<u8>(), 0..512),
        goal in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        let sandbox = Sandbox::new();
        let adapter = FsAdapter::new();
        let locator = sandbox.locator(SUBJECT);
        sandbox.write(SUBJECT, &given);

        let pre = snapshot_of(&adapter, &locator);
        let delta = planned(&adapter, &locator, &goal);
        let after = round_trip(&adapter, &locator, &delta, &pre);

        prop_assert_eq!(
            after.as_deref(),
            Some(given.as_slice()),
            "Given {} bytes, the round trip returned something else",
            given.len()
        );
    }
}
