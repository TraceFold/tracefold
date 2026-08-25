// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-003 (FR-003) — the order ceiling is checked at run time, not by the type.
//!
//! AC-003, verbatim (quoted in SEM-gx-core-105): "Given: the order-producing API
//! `Transformation::with_order(n: u8)`. When: called with `n=0,1,2`. Then: all `Ok(_)`. When: called
//! with `n=3` and `n=255`. Then: all return `Err(OrderExceeded)` (it compiles, but is a runtime
//! error)."
//!
//! Two spec facts pin the signature between them. The AC shows one parameter and no `self`, so
//! it is an associated function; 41 §3 keeps `Transformation.order` a bare `u8`, so it cannot
//! return a validated newtype that the struct would then have to hold. What is left is a
//! validating constructor for the order value itself, which is exactly how req/31 §7 uses it --
//! composition sets `order := max(f.order, g.order)` and "always passes through `with_order`'s
//! `<= 2` check" (sem: SEM-gx-core-106).
//!
//! FR-003 adds the reason the check is not a type: "an attempt to produce order > 2 passes the
//! compile-time type check and still returns an Error at runtime" (sem: SEM-gx-core-107). ASM-6
//! and DR-7 (DEFAULT: <=2) set the ceiling at 2 for
//! v0.1, and a ceiling that may move with a later DR belongs in a value check, not in a type.
//!
//! # What the AC does not say, and F-2 does
//!
//! `with_order`'s own doc claims "every place that sets an order is required to come through
//! here" (sem: SEM-gx-core-108). `req/46C_AUDIT_PRACTICAL_2026-08-07.md` §3 W-2 measured that
//! claim and found it was
//! discipline rather than a fact: `order` was a plain `pub` field, so a struct literal wrote
//! `order: 250` with no compile error and no panic. `req/46D_AUDIT_RULING_2026-08-07.md` §1 F-2
//! rules the field private with a getter, which leaves `Transformation::new` as the only way in
//! from outside the crate -- and `new` routes through `with_order`. The three tests at the end of
//! this file are that ruling's fixture.

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, Error, IntentId, ObjectId, Subject,
    SubstrateKind, Timestamp, Transformation, TransformationId, MAX_ORDER,
};

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn metadata() -> CompositionMetadata {
    CompositionMetadata {
        intent_id: IntentId(cid(0x11)),
        delta: DeltaRef {
            substrate: SubstrateKind::Fs,
            cid: cid(0x33),
        },
        context: ChangeContext::Policy,
        actor: Actor::Human {
            key: "ac-003".to_string(),
        },
        created_at: Timestamp(0),
    }
}

fn built(order: u8) -> gx_core::Result<Transformation> {
    Transformation::new(
        TransformationId(cid(0x00)),
        order,
        Subject::Object(ObjectId(cid(0x01))),
        Some(cid(0x22)),
        Vec::new(),
        metadata(),
    )
}

#[test]
fn ac_003_orders_zero_one_two_are_accepted() {
    for n in [0u8, 1, 2] {
        let got = Transformation::with_order(n);
        assert!(got.is_ok(), "order {n} must be admitted (ASM-6, DR-7)");
        assert_eq!(got.unwrap(), n);
    }
}

#[test]
fn ac_003_orders_three_and_255_are_rejected() {
    for n in [3u8, 255] {
        match Transformation::with_order(n) {
            Err(Error::OrderExceeded { got, max }) => {
                assert_eq!(got, n);
                assert_eq!(max, 2);
            }
            other => panic!("order {n} must be rejected with OrderExceeded, got {other:?}"),
        }
    }
}

#[test]
fn ac_003_ceiling_is_two_across_the_whole_u8_range() {
    // The AC samples 0,1,2,3,255. Nothing stops the remaining 251 values from being checked, and
    // an off-by-one at the boundary is the failure this catches that the five samples do not.
    assert_eq!(MAX_ORDER, 2);
    for n in 0u8..=255 {
        assert_eq!(
            Transformation::with_order(n).is_ok(),
            n <= MAX_ORDER,
            "order {n} classified wrongly"
        );
    }
}

// ---------------------------------------------------------------------------
// F-2 (46C §3 W-2, 46D §1): the ceiling holds for the value, not only for the checker
// ---------------------------------------------------------------------------

/// The public constructor classifies exactly as `with_order` does, over the whole `u8` range.
///
/// 46C's finding was that a `Transformation` could *hold* an order the checker rejects. This is
/// the closing statement: every order the constructor admits is an order `with_order` admits, and
/// the value that comes out carries it.
#[test]
fn f_002_the_only_public_constructor_classifies_as_with_order_does() {
    for n in 0u8..=255 {
        let admitted = built(n);
        assert_eq!(
            admitted.is_ok(),
            n <= MAX_ORDER,
            "Transformation::new disagreed with with_order at order {n}"
        );
        if let Ok(t) = admitted {
            assert_eq!(t.order(), n);
        }
    }
}

/// The exact refusal 46C could not obtain: `order = 250` is now a value that cannot be built.
#[test]
fn f_002_an_over_high_order_is_refused_at_construction() {
    assert_eq!(
        built(250).expect_err("46C built this value with no error at all"),
        Error::OrderExceeded {
            got: 250,
            max: MAX_ORDER
        }
    );
}

/// Mutation goes through the same check. A checked constructor with an unchecked setter beside it
/// would be the same hole one call later.
#[test]
fn f_002_mutating_the_order_goes_through_the_same_check() {
    let mut t = built(0).expect("order 0 is admitted");

    t.set_order(MAX_ORDER)
        .expect("the ceiling itself is admitted");
    assert_eq!(t.order(), MAX_ORDER);

    assert_eq!(
        t.set_order(250)
            .expect_err("the ceiling holds for mutation too"),
        Error::OrderExceeded {
            got: 250,
            max: MAX_ORDER
        }
    );
    assert_eq!(t.order(), MAX_ORDER, "a refused write must not land");
}
