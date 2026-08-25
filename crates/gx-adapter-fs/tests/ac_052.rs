// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-052 (FR-047) — two changes are independent, or one of them is held back and named.
//!
//! AC-052, verbatim: "Given: for each fs/git/mcp adapter, a commuting pair of 2 deltas (e.g. writes to
//! different files) and a non-commuting pair of 2 deltas (a conflicting write to the same file). When: `adapter.commutation(a,b)` is called. Then: for a commuting pair
//! `Commutes`, for a non-commuting pair `Conflicts{residual}` (at least one pair per adapter, 6 cases total)." Judgment method: "unit". (sem: SEM-gx-adapter-fs-126)
//! This file is the **fs third** of those six cases; git and mcp are M7 (**N-03**).
//!
//! # What the criterion does not look at, and this file does
//!
//! AC-052 stops at the shape of the answer. req/38 §28 **M4-14** asks for one thing more -- "a discrimination test in
//! hand 6 for whether the residual CID is resolvable within the test harness" -- because `Conflicts` carries a
//! `DeltaRef` and a reference whose referent nobody can produce is "a CID with nowhere to point". So the (sem: SEM-gx-adapter-fs-127)
//! residual's **contents** are measured here: the CID is re-derived from the delta the adapter says
//! is held back, and a reference nobody minted is shown not to be it.
//!
//! # Symmetry and reflexivity (**M4-25, adopted (a)**) (sem: SEM-gx-adapter-fs-128)
//!
//! 41 §4 and 51 §7 leave the relation between `commutation(a,b)` and `commutation(b,a)` unwritten;
//! §28 added it and fixed `commutation(a,a)` at `Conflicts`. L6 in the shared harness is where those
//! two are obligations; here they are measured over this adapter's own pairs and, in the property at
//! the foot of the file, over generated spellings. The property is the one that matters: two chosen
//! pairs cannot tell a symmetric implementation from one that happens to agree twice.
//!
//! 🔴 **The verdict is symmetric and the residual is not**, by construction and on purpose. See
//! [`the_verdict_does_not_depend_on_the_order_of_the_arguments`] and `req/75` §2 -- the trait's
//! contract row writes the symmetry as `commutation(a,b) == commutation(b,a)`, which is stronger than
//! both the ruling's property and every implementation in this workspace, and that seam is raised
//! rather than decided.

mod support;

use gx_adapter_fs::{normalize, FsAdapter, FsDelta, FsOp};
use gx_core::{Cid, Commutation, DeltaRef, SubstrateKind};
use gx_substrate::{PlannedDelta, SubstrateAdapter};
use proptest::prelude::*;
use support::{spelled, OTHER, SUBJECT};

/// A sandbox-free pair of positions. `commutation` reads no filesystem, which is measured below.
const HERE: &str = "/tmp/glovrex-ac052/subject";
const BESIDE: &str = "/tmp/glovrex-ac052/beside";

fn verdict(a: &PlannedDelta, b: &PlannedDelta) -> Commutation {
    FsAdapter::new()
        .commutation(a, b)
        .expect("both payloads are this adapter's own grammar")
}

/// The delta a caller would hold for a whole-file replacement at `locator`.
fn change(locator: &str, content: &[u8]) -> PlannedDelta {
    spelled(&normalize(locator), content)
}

// ---------------------------------------------------------------------------
// AC-052's two cases
// ---------------------------------------------------------------------------

/// Commuting: "writes to different files" answers `Commutes`. (sem: SEM-gx-adapter-fs-129)
#[test]
fn writes_to_two_positions_are_independent() {
    let answer = verdict(&change(HERE, b"one"), &change(BESIDE, b"two"));
    println!("AC_052_PAIR=commuting ANSWER={answer:?}");
    assert_eq!(
        answer,
        Commutation::Commutes,
        "two whole-file replacements of two files touch no shared resource, which is DPO parallel \
         independence at the size of this grammar (ASM-2)"
    );
}

/// Non-commuting: "a conflicting write to the same file" answers `Conflicts{residual}`. (sem: SEM-gx-adapter-fs-130)
#[test]
fn two_writes_to_one_position_conflict() {
    let answer = verdict(&change(HERE, b"one"), &change(HERE, b"two"));
    let Commutation::Conflicts { residual } = &answer else {
        panic!("two whole-file replacements of one file are not independent: {answer:?}");
    };
    println!(
        "AC_052_PAIR=conflicting ANSWER=Conflicts RESIDUAL_SUBSTRATE={:?}",
        residual.substrate
    );
    assert_eq!(
        residual.substrate,
        SubstrateKind::Fs,
        "the residual names a grammar this adapter does not speak"
    );
}

/// A removal and a write at one position conflict too: the grammar's shape is not the question.
///
/// Independence is about the **resource**, and both operations of this grammar take the whole file.
/// A test that only ever paired two writes would leave "is the verdict about the operation or about
/// the position" unmeasured. (sem: SEM-gx-adapter-fs-131)
#[test]
fn a_removal_and_a_write_at_one_position_conflict() {
    let removal = support::removal(HERE);
    let answer = verdict(&removal, &change(HERE, b"two"));
    assert!(
        matches!(answer, Commutation::Conflicts { .. }),
        "a removal and a replacement of one file are not parallel-independent: {answer:?}"
    );
    assert_eq!(
        verdict(&removal, &change(BESIDE, b"two")),
        Commutation::Commutes,
        "the control: the same removal against another position is independent"
    );
}

/// Two spellings of one position conflict, because the verdict is taken after normalisation.
///
/// **E-M4-12** is the reason this is a probe and not a detail. M3-10 fixed a v0.1 policy pack's
/// effective range at the "locator level", so `/a/../a/x` and `/a/x` are one file and two subjects; a (sem: SEM-gx-adapter-fs-132)
/// `commutation` that compared the spellings would answer `Commutes` for two changes to one file,
/// which is the fail-open direction (43 §8 stops on `Conflicts`).
#[test]
fn two_spellings_of_one_position_are_one_resource() {
    let plain = spelled(HERE, b"one");
    let roundabout = spelled("/tmp/glovrex-ac052/beside/../subject", b"two");
    let answer = verdict(&plain, &roundabout);
    println!(
        "AC_052_SPELLINGS RAW=/tmp/glovrex-ac052/beside/../subject NORMALISED={} ANSWER={answer:?}",
        normalize("/tmp/glovrex-ac052/beside/../subject")
    );
    assert!(
        matches!(answer, Commutation::Conflicts { .. }),
        "two spellings of one position answered {answer:?}, so a gate could be shown two subjects \
         for one file (E-M4-12, M3-10)"
    );
}

// ---------------------------------------------------------------------------
// M4-14: the residual has a referent
// ---------------------------------------------------------------------------

/// **M4-14**: the residual resolves, and it resolves to the change that is held back.
///
/// req/38 §28 M4-14, verbatim: "the storage-obligation line of E-M4-8 also covers the residual body (the
/// `PlannedDelta` the adapter mints) = 'a CID with nowhere to point' is excluded as a storage-obligation violation. A discrimination test in hand 6 for
/// whether the residual CID is resolvable within the test harness". (sem: SEM-gx-adapter-fs-133)
///
/// There is no delta store in M4 -- that is the engine's, in M5 (**E-M4-8**) -- so "resolvable" here (sem: SEM-gx-adapter-fs-134)
/// means the body can be **constructed** from what the caller already has: this adapter's residual is
/// the second delta itself, re-minted at the normalised spelling of its position. When the payload
/// was already normalised (everything `plan` writes is), the residual *is* `b.reference()`, and the
/// engine's obligation to keep forward payloads (E-M4-8) is what keeps the CID pointing at something.
#[test]
fn the_residual_names_the_delta_that_is_held_back() {
    let a = change(HERE, b"one");
    let b = change(HERE, b"two");
    let Commutation::Conflicts { residual } = verdict(&a, &b) else {
        panic!("two writes to one position conflict");
    };

    println!(
        "AC_052_RESIDUAL RESOLVES_TO_B={} EQUALS_A={}",
        &residual == b.reference(),
        &residual == a.reference()
    );
    assert_eq!(
        &residual,
        b.reference(),
        "the residual is the later change itself: for whole-file replacement nothing of it is \
         independent of the first, which is 42 §3.6's 'the part that is not independent' at this grammar's size (sem: SEM-gx-adapter-fs-135)"
    );
    assert_ne!(
        &residual,
        a.reference(),
        "the residual named the first delta, so 'last wins' and 'first wins' would be one answer (sem: SEM-gx-adapter-fs-136)"
    );

    // The body, reconstructed rather than looked up -- the discrimination M4-14 asks for. A
    // `DeltaRef` that agrees with a payload the test built is a reference with a referent.
    let rebuilt = PlannedDelta::new(
        SubstrateKind::Fs,
        FsDelta::one(FsOp::write(normalize(HERE), b"two".to_vec()))
            .encode()
            .expect("a one-operation sequence has a canonical form"),
    )
    .expect("the projection is encodable");
    assert_eq!(
        rebuilt.reference(),
        &residual,
        "the residual could not be re-derived from the change it names, so nothing in this \
         milestone can turn the CID back into a body"
    );
    let decoded = FsDelta::decode(rebuilt.payload()).expect("the adapter reads its own grammar");
    assert_eq!(decoded.ops()[0].locator(), normalize(HERE));
    assert_eq!(decoded.ops()[0].content(), Some(b"two".as_slice()));

    // 🔴 The residual is minted at the **position** and not at the spelling it arrived in. A payload
    // written by hand can name one position any way L7 admits, and a residual carrying the caller's
    // spelling would put a second name for one change into a receipt -- M3-10 fixed the gate's
    // effective range at the "locator level", so two names are two subjects. Mutation (e) of (sem: SEM-gx-adapter-fs-137)
    // `tools/verify_m4h6.sh` survived its first run because nothing here said this; the probe was
    // the thing that was wrong, and this is the repair (req/75 §1.9).
    let roundabout = spelled("/tmp/glovrex-ac052/beside/../subject", b"two");
    let Commutation::Conflicts {
        residual: from_a_spelling,
    } = verdict(&a, &roundabout)
    else {
        panic!("two spellings of one position conflict");
    };
    assert_eq!(
        &from_a_spelling, &residual,
        "the residual of a conflict depends on how the caller spelled the position, so one change \
         would be named twice"
    );
}

/// The half that makes the probe above a measurement: a reference nobody minted is not the residual.
///
/// Without it, "the residual resolves" would be satisfied by an adapter that answered with a (sem: SEM-gx-adapter-fs-138)
/// constant, which is the B-3 shape (req/67 §2.1) at the size of one CID. The mock's
/// `a_reference_nobody_minted_does_not_resolve` (hand 3) is the same probe against a store; this is
/// its form for an adapter that has none.
#[test]
fn a_reference_nobody_minted_is_not_the_residual() {
    let Commutation::Conflicts { residual } = verdict(&change(HERE, b"one"), &change(HERE, b"two"))
    else {
        panic!("two writes to one position conflict");
    };
    let invented = DeltaRef {
        substrate: SubstrateKind::Fs,
        cid: Cid([0x5a; 32]),
    };
    assert_ne!(residual, invented);

    // And a change at another position mints another CID, so the residual is a function of the
    // change rather than of the fact that something clashed.
    let elsewhere = change(BESIDE, b"two");
    assert_ne!(&residual, elsewhere.reference());
}

// ---------------------------------------------------------------------------
// M4-25: symmetry and the reflexive case
// ---------------------------------------------------------------------------

/// **M4-25, adopted (a)**: the verdict is the same both ways round, and the residual is not. (sem: SEM-gx-adapter-fs-139)
///
/// The first half is the ruling: "commutation's **symmetry is added to the contract** and measured by the L6 property", and DPO (sem: SEM-gx-adapter-fs-140)
/// parallel independence is a symmetric relation, so the asymmetric reading was never the intended
/// one. This adapter gets it structurally -- the answer is decided by `position(a) == position(b)`,
/// which is a question about an unordered pair -- rather than by two branches that agree.
///
/// 🔴 The second half is a seam and is printed rather than smoothed: `Conflicts` carries the delta
/// that is **held back**, which is a question about an ordered pair (43 §8 keeps `T2` waiting for
/// `T1`). So the two directions name two different residuals. The trait's contract row writes the
/// obligation as `commutation(a,b) == commutation(b,a)`; the harness's L6 compares the two answers as
/// answers and says in its own comment why comparing residuals would be a stronger law than the
/// ruling made. This implementation satisfies L6 and not the row's `==`, and `req/75` §2 raises which
/// of the two is meant.
#[test]
fn the_verdict_does_not_depend_on_the_order_of_the_arguments() {
    for (name, a, b) in [
        ("independent", change(HERE, b"one"), change(BESIDE, b"two")),
        ("dependent", change(HERE, b"one"), change(HERE, b"two")),
    ] {
        let forward = verdict(&a, &b);
        let backward = verdict(&b, &a);
        let agree = matches!(
            (&forward, &backward),
            (Commutation::Commutes, Commutation::Commutes)
                | (Commutation::Conflicts { .. }, Commutation::Conflicts { .. })
        );
        let residuals_agree = forward == backward;
        println!(
            "AC_052_SYMMETRY PAIR={name} VERDICTS_AGREE={agree} VALUES_AGREE={residuals_agree}"
        );
        assert!(
            agree,
            "the {name} pair answers {forward:?} one way and {backward:?} the other (M4-25, adopted (a)) (sem: SEM-gx-adapter-fs-141)"
        );
        if name == "dependent" {
            assert!(
                !residuals_agree,
                "the two directions named one residual, so 'held back' stopped being a property of \
                 the order -- which would make the seam in `req/75` §2 moot and this probe stale (sem: SEM-gx-adapter-fs-142)"
            );
        }
    }
}

/// **M4-25**'s reflexive clause: `commutation(a,a)` is `Conflicts`.
///
/// "double interference on the same resource = the conservative side is fail-closed; DPO parallel independence generally does not hold for the same match". The (sem: SEM-gx-adapter-fs-143)
/// conservative side matters because 43 §8 acts on the answer: a `Commutes` lets two changes proceed
/// together, so a false `Commutes` is the fail-open direction and this is the case where the fail-open
/// answer is easiest to produce by accident (a comparison that returns "no difference"). (sem: SEM-gx-adapter-fs-144)
#[test]
fn a_delta_does_not_commute_with_itself() {
    let a = change(HERE, b"one");
    let answer = verdict(&a, &a);
    println!("AC_052_REFLEXIVE ANSWER={answer:?}");
    let Commutation::Conflicts { residual } = answer else {
        panic!("a delta commuted with itself, which is the fail-open side of 43 §8");
    };
    assert_eq!(
        &residual,
        a.reference(),
        "the residual of (a, a) is a itself"
    );
}

// ---------------------------------------------------------------------------
// The refusals, and the absence of a substrate read
// ---------------------------------------------------------------------------

/// Another adapter's delta is refused from either slot.
///
/// Both slots, because a check on one argument would leave the other reading a `git` payload with fs
/// eyes -- **E-M4-27**'s reasoning about a mis-wired engine, at the delta rather than at the
/// fingerprint.
#[test]
fn a_delta_from_another_adapter_is_refused_from_either_slot() {
    let ours = change(HERE, b"one");
    let theirs = PlannedDelta::new(SubstrateKind::Git, b"a git payload".to_vec())
        .expect("the projection is encodable");
    let adapter = FsAdapter::new();

    for (slot, refusal) in [
        ("left", adapter.commutation(&theirs, &ours)),
        ("right", adapter.commutation(&ours, &theirs)),
    ] {
        let error = refusal.expect_err("a git payload is not this adapter's grammar");
        println!("AC_052_FOREIGN SLOT={slot} KIND={}", error.kind());
        assert_eq!(error.kind(), "ForeignDelta");
    }
}

/// `commutation` answers without reading the substrate, and the source is where that is measured.
///
/// It is not an accident of this grammar but a consequence of it: under **M4-13, adopted (a)** every
/// operation takes the whole file, so "do these two touch one resource" is decidable from the two
/// payloads alone. That is also what makes AC-053's "outside the engine pipeline" easy -- there is nothing
/// for a pipeline to supply. The scan reads code lines only (§30's rule about "absence" greps). (sem: SEM-gx-adapter-fs-145)
#[test]
fn the_module_that_answers_reads_no_filesystem() {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/commutation.rs"),
    )
    .expect("src/commutation.rs is readable");
    let code: Vec<&str> = src
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//") && !line.is_empty())
        .collect();
    let offenders: Vec<&&str> = code
        .iter()
        .filter(|line| {
            ["std::fs", "fs::read", "File::", "OpenOptions"]
                .iter()
                .any(|token| line.contains(token))
        })
        .collect();
    println!(
        "COMMUTATION_IO_INVOCATIONS={} SCANNED_CODE_LINES={}",
        offenders.len(),
        code.len()
    );
    assert!(
        offenders.is_empty(),
        "independence is decided from the payloads under M4-13(a), and these lines read a \
         substrate instead: {offenders:?}"
    );
}

/// The harness's own fixture supplies both of 51 §7's cases, which is what closes contract 6.
///
/// The pairs above are written in this file; the ones the shared harness runs come from
/// `support::FsFixture`, and a pair that existed only here would leave 51 §7's contract 6 unmeasured
/// while AC-052 was green.
#[test]
fn the_fixture_supplies_both_of_the_two_cases() {
    use gx_substrate_conformance::Fixture;

    let fixture = support::FsFixture::new();
    let (a, b) = fixture.commuting_pair().expect("51 §7's commuting case"); // (sem: SEM-gx-adapter-fs-146)
    let (c, d) = fixture
        .conflicting_pair()
        .expect("51 §7's non-commuting case");
    assert_eq!(verdict(&a, &b), Commutation::Commutes);
    assert!(matches!(verdict(&c, &d), Commutation::Conflicts { .. }));

    let subject = fixture.sandbox().locator(SUBJECT);
    let beside = fixture.sandbox().locator(OTHER);
    println!(
        "AC_052_FIXTURE_PAIRS COMMUTING={subject} + {beside} CONFLICTING={subject} + {subject}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(
        std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(64)
    ))]

    /// **L6** over generated spellings: the verdict is a function of the two **normalised** positions,
    /// it does not depend on the order, and every delta conflicts with itself.
    ///
    /// Two chosen pairs cannot separate a symmetric implementation from one that agrees twice, and
    /// they cannot reach the spellings where a positional comparison goes wrong (`/a/./b`, `/a/../a/b`,
    /// `/a//b`). The generator builds those on purpose: the alphabet is the one E-M4-12's clauses are
    /// about, so most pairs it draws are two spellings that have to be decided as one position or as
    /// two.
    #[test]
    fn the_verdict_is_a_function_of_the_two_normalised_positions(
        left in position(),
        right in position(),
        one in proptest::collection::vec(any::<u8>(), 0..8),
        two in proptest::collection::vec(any::<u8>(), 0..8),
    ) {
        let a = spelled(&left, &one);
        let b = spelled(&right, &two);
        let same_position = normalize(&left) == normalize(&right);

        let forward = verdict(&a, &b);
        let backward = verdict(&b, &a);
        prop_assert_eq!(
            matches!(forward, Commutation::Conflicts { .. }),
            same_position,
            "{:?} and {:?} normalise to {:?} and {:?}",
            left, right, normalize(&left), normalize(&right)
        );
        prop_assert_eq!(
            matches!(forward, Commutation::Conflicts { .. }),
            matches!(backward, Commutation::Conflicts { .. }),
            "the verdict changed with the order of the arguments"
        );
        prop_assert!(
            matches!(verdict(&a, &a), Commutation::Conflicts { .. }),
            "a delta commuted with itself"
        );
    }
}

/// Absolute spellings built from the segments E-M4-12's clauses are about.
fn position() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just("a".to_string()),
            Just("b".to_string()),
            Just(".".to_string()),
            Just("..".to_string()),
            Just(String::new()),
        ],
        1..5,
    )
    .prop_map(|segments| format!("/{}", segments.join("/")))
}
