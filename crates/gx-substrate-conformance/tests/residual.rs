//! **M4-14** — a `Conflicts{residual}` names a delta, and 「名指す」 has to mean something.
//!
//! req/38 §28 M4-14 逐語: 「41 §3 の `Conflicts{residual: DeltaRef}` 形は**不変**。E-M4-8 の保管行が
//! residual 本体(adapter が鋳造する `PlannedDelta`)も覆う=「指し先の無い CID」は保管義務違反として排除。
//! 手6 に「residual CID が test harness 内で解決可能」の判別 test」.
//!
//! req/69 §6.2 gives hand 3 「形だけ」 of that test, because there is no store to resolve against: the
//! delta store is the engine's and the engine is M5. So what is here is the **question**, asked
//! through [`Fixture::resolve`] over a mock that keeps what it minted -- and the negative case, which
//! is what stops the question from being rhetorical.
//!
//! # Why a CID with no referent is a defect rather than a shrug
//!
//! `Commutation::Conflicts` carries a `DeltaRef`, and 42 §3.6 calls the residual 「独立でない部分」. An
//! engine that met one has two jobs: stop (43 §8), and tell the operator what clashed. The second is
//! only possible if the reference resolves. 42 §5's storage table had no row for a `PlannedDelta`
//! body at all until **E-M4-8** added one -- 「`PlannedDelta.payload` は**保管する(必須)**」 -- and
//! M4-14 reads that row as covering the residual too. This file is the smallest measurement of that
//! reading.

mod support;

use gx_core::{Cid, Commutation, DeltaRef, SubstrateKind};
use gx_substrate_conformance::Fixture;
use support::MockFixture;

/// The residual of a real conflict resolves, and resolves to the delta it names.
#[test]
fn a_residual_cid_resolves_to_the_delta_it_names() {
    let fixture = MockFixture::new();
    let (a, b) = fixture
        .conflicting_pair()
        .expect("the mock supplies 51 §7's 非可換 case");

    let residual = match fixture
        .adapter()
        .commutation(&a, &b)
        .expect("the mock can compare its own deltas")
    {
        Commutation::Conflicts { residual } => residual,
        Commutation::Commutes => panic!("two writes to one locator are not independent"),
    };

    let body = fixture
        .resolve(&residual)
        .expect("E-M4-8: the body of a minted residual is kept, so its CID has a referent");
    println!(
        "RESIDUAL_RESOLVES=1 SUBSTRATE={:?} PAYLOAD_BYTES={}",
        residual.substrate,
        body.payload().len()
    );

    assert_eq!(
        body.reference(),
        &residual,
        "the resolved delta's own reference is not the CID that named it, so the store answered \
         with something else"
    );
    assert_eq!(
        &residual.substrate,
        &fixture.adapter().kind(),
        "the residual names a grammar this adapter does not speak"
    );
    assert!(
        !body.payload().is_empty(),
        "a residual with an empty payload names no obstruction"
    );
}

/// A reference nobody minted does not resolve.
///
/// The half that makes the probe above a measurement. A `resolve` that returned something for every
/// input -- an empty delta, a default -- would satisfy the first test while proving that no store
/// was consulted, which is the B-3 shape (req/67 §2.1) at the size of one lookup.
#[test]
fn a_reference_nobody_minted_does_not_resolve() {
    let fixture = MockFixture::new();
    let invented = DeltaRef {
        substrate: SubstrateKind::Custom("mock".to_string()),
        cid: Cid([0x5a; 32]),
    };
    assert!(
        fixture.resolve(&invented).is_none(),
        "the fixture resolved a CID it never minted; 「解決可能」 would then be a property of the \
         lookup rather than of the store"
    );
}
