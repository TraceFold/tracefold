// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6-08 adopted (a)** (sem: SEM-gx-engine-842) — `verify`'s `mode` is a **per-call** override, and it overrides.
//!
//! req/38 §47 adopted (a) with 44 §1.2's own words as the reason:
//!
//! > `--record-only`: forces DR-2's record-only mode **per this command**, overriding the global
//! > setting (sem: SEM-gx-engine-843)
//!
//! and named the shape that must **not** be taken (b): "serve swaps the mode per request via
//! `&mut self` -- the mode leaks between concurrent requests (the worst fail-open)" (sem: SEM-gx-engine-843). An argument cannot leak; a field
//! reassignment can. So the parameter exists, and this file is what makes its existence a fact rather
//! than a signature.
//!
//! # What is observable from inside `verify`
//!
//! `EnforcementMode` reaches two places. T-8r is in `canonicalize`, one transition later, and is not
//! this call's. The one inside `verify` is 43 §8's conflict check, and it turns on a single line:
//!
//! > `Lifecycle::Denied => mode == EnforcementMode::RecordOnly`
//!
//! A `Denied` transformation is **terminal** under `Enforce` (43 §1) and is **still going to apply
//! something** under `RecordOnly` (43 §1's own exception, "but only under record-only mode does §3's
//! exception branch advance to `Canonicalized`" (sem: SEM-gx-engine-844)). So it is in flight in one mode and not in the other, and a second
//! transformation over the same subject either waits for it or does not.
//!
//! That is the whole experiment: **one engine, one default, two answers, decided by the argument.**

mod support;

use std::sync::Arc;

use gx_core::{EnforcementMode, Timestamp};
use gx_engine::{Engine, InjectedEvidence, Lifecycle};
use support::{gate_refusing, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

/// The one clock, injected (41 §6).
const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The goal the deployment's invariant refuses, and the one it does not.
const REFUSED: &str = "the change this deployment refuses";
const ALLOWED: &str = "a change nobody objects to";

/// Build an engine whose gate denies [`REFUSED`] and whose adapter says every pair conflicts.
///
/// `conflicting` is 43 §8's precondition: waiting is entered on `Commutation::Conflicts` and on
/// nothing else, so an adapter that always commutes would make both halves of the experiment answer
/// "no conflict" (sem: SEM-gx-engine-845) for a reason that has nothing to do with the mode.
fn engine_with(name: &str) -> Engine<InjectedEvidence> {
    let dir = scratch(name);
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate_refusing(PERMIT_ALL, "deployment-refuses-this", REFUSED),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(adapter.conflicting()), "record-only-per-call-1");
    engine
}

/// Put a `Denied` transformation over `locator` into the table, and return the id.
fn deny_one(engine: &mut Engine<InjectedEvidence>, locator: &str) -> gx_core::TransformationId {
    let i = intent(locator, REFUSED);
    engine.submit(&i, 1, AT).expect("T-1");
    let id = engine.plan(&i, AT).expect("T-2");
    let state = engine.verify(&id, AT, &signing_key(), None).expect("T-4b");
    assert_eq!(
        state,
        Lifecycle::Denied,
        "the fixture's invariant refuses this payload; without that this test measures nothing"
    );
    id
}

/// 🔴 One engine, default `Enforce`, and the argument decides.
///
/// Two runs rather than two calls on one engine, because the first answer **moves the row**: a `t2`
/// that reached `Admitted` in the enforce case cannot be asked again (43 T-3's from-state is
/// `Candidate`), so a second call on the same engine would be measuring the state machine's refusal
/// rather than the mode.
#[test]
fn the_mode_argument_overrides_the_engines_own_for_one_call() {
    // --- Enforce (the engine's default, `mode = None`) ---------------------------------------
    let mut enforcing = engine_with("record_only_enforce");
    let denied = deny_one(&mut enforcing, "/tmp/record-only-probe.txt");
    let second = intent("/tmp/record-only-probe.txt", ALLOWED);
    enforcing.submit(&second, 2, AT).expect("T-1");
    let t2 = enforcing.plan(&second, AT).expect("T-2");
    let enforce_state = enforcing
        .verify(&t2, AT, &signing_key(), None)
        .expect("T-4a");

    // --- RecordOnly, for this call only ------------------------------------------------------
    let mut overridden = engine_with("record_only_override");
    let denied_again = deny_one(&mut overridden, "/tmp/record-only-probe.txt");
    let second = intent("/tmp/record-only-probe.txt", ALLOWED);
    overridden.submit(&second, 2, AT).expect("T-1");
    let t2_again = overridden.plan(&second, AT).expect("T-2");
    let override_state = overridden
        .verify(
            &t2_again,
            AT,
            &signing_key(),
            Some(EnforcementMode::RecordOnly),
        )
        .expect("43 §8 holds it rather than refusing");

    println!(
        "PER_CALL_MODE enforce={enforce_state:?} record_only={override_state:?} \
         denied={denied:?}/{denied_again:?} blocked_by={:?} engine_mode_after={:?}",
        overridden.blocked_by(&t2_again),
        overridden.mode()
    );

    assert_eq!(
        enforce_state,
        Lifecycle::Admitted,
        "under `Enforce` a `Denied` predecessor is terminal (43 §1), so nothing is in flight to \
         conflict with and the second transformation is judged"
    );
    assert_eq!(
        override_state,
        Lifecycle::Candidate,
        "under `RecordOnly` the same `Denied` row is still going to apply something, so 43 §8 holds \
         the second transformation behind it -- \"no new state is added\" (sem: SEM-gx-engine-846), it stays a Candidate"
    );
    assert_eq!(
        overridden.blocked_by(&t2_again),
        Some(denied_again),
        "and it says which transformation it is waiting for (43 §8's \"an internal annotation named `blocked_by`\") (sem: SEM-gx-engine-847)"
    );

    // 🔴 The half that makes it a *per-call* override rather than a setter: **the engine did not
    // change**. This is M6-08(b)'s failure mode measured directly — a `gx serve` that answered 44
    // §2.2's `record_only: bool|null` by reassigning a field would leave the next request in a mode
    // it never asked for, and 43 §4 makes that the worst fail-open there is.
    assert_eq!(
        overridden.mode(),
        EnforcementMode::Enforce,
        "the argument overrode one evaluation and set nothing"
    );

    // And the proof that the override is not simply "everything waits" (sem: SEM-gx-engine-848): the same engine, the same
    // call, with `None`, still answers about a row that is not blocked.
    let third = intent("/tmp/record-only-other.txt", ALLOWED);
    overridden.submit(&third, 3, AT).expect("T-1");
    let t3 = overridden.plan(&third, AT).expect("T-2");
    let unrelated = overridden
        .verify(&t3, AT, &signing_key(), Some(EnforcementMode::RecordOnly))
        .expect("T-4a");
    println!("PER_CALL_MODE_OTHER_SUBJECT={unrelated:?}");
    assert_eq!(
        unrelated,
        Lifecycle::Admitted,
        "43 §8's waiting is per `Subject`; a different object is not held by anything"
    );
}

/// 🔴 The other half of DR-2, at the transition that carries it: **T-8r**.
///
/// 43 §4: "Record-only mode (`EnforcementMode::RecordOnly`, per-substrate or global): even from
/// `Denied`, via T-8r, advance to `Canonicalized → Committing → Committed`. But the receipt must
/// always carry `enforced=false`." (sem: SEM-gx-engine-849)
///
/// This is the path `gx commit --record-only` drives, and it is measured **here** rather than
/// through the binary for a reason worth writing down: the CLI's only route to a `Verdict::Deny` in
/// v0.1 is the shipped pack's `/etc` forbid, and a record-only commit of one would carry `apply`
/// through to a **write under `/etc`** — a permission error for an ordinary user and a destroyed
/// machine for a suite run as root. So the E2E stops at the refusal (`crates/gx-cli/tests/ac_054.rs`)
/// and the road past it is walked here, with a fixture adapter and a temporary directory. Raised as
/// **M6H3-9**: what a record-only E2E needs is a policy pack fixture over a writable path, and 44
/// §1.2 gives `gx verify` no `--policy`.
///
/// 🔴 **Corrected by `docs/LIMITS.md` v0.5-k (`req/318` §(b), ruling `req/38` §350 item 8), quoted
/// rather than restated:** "`gx wrap` **has** `--policy`, and the suites already ship a pack
/// fixture over a writable path. The blocker does not hold on this surface, and it is recorded
/// here rather than left standing."
#[test]
fn record_only_is_what_opens_t8r_and_the_receipt_says_so() {
    let mut engine = engine_with("record_only_t8r").with_mode(EnforcementMode::RecordOnly);
    let denied = deny_one(&mut engine, "/tmp/record-only-t8r.txt");

    let canonicalized = engine.canonicalize(&denied, AT, None).expect("T-8r");
    let committed = engine.commit(&denied, AT, &signing_key()).expect("T-11");
    println!(
        "T8R canonicalized={canonicalized:?} committed={committed:?} enforced={:?} verdict={:?}",
        engine.enforced(&denied),
        engine.verdict(&denied)
    );
    assert_eq!(canonicalized, Lifecycle::Canonicalized);
    assert_eq!(committed, Lifecycle::Committed);
    assert_eq!(
        engine.enforced(&denied),
        Some(false),
        "43 §4: \"the receipt must always carry `enforced=false`\" -- which is what makes \"the apply \
         went through, but policy had denied it\" third-party verifiable (P-7, INV-S5) (sem: SEM-gx-engine-850)"
    );

    // The control, on an engine that is not in record-only: the same row, the same call, refused.
    let mut enforcing = engine_with("record_only_t8r_control");
    let denied = deny_one(&mut enforcing, "/tmp/record-only-t8r.txt");
    let refused = enforcing
        .canonicalize(&denied, AT, None)
        .expect_err("43 §1 makes `Denied` terminal under `Enforce`");
    println!("T8R_CONTROL refused={:?}", refused.kind());
    assert_eq!(refused.kind(), "InvalidState");
}

// ---------------------------------------------------------------------------
// 🔴 **E-M6-20** — the same argument on `canonicalize`, added by M6 hand 6
// ---------------------------------------------------------------------------

/// 🔴 A per-call `RecordOnly` opens T-8r on an engine whose own posture is `Enforce`.
///
/// **E-M6-20** (req/38 §52) put `record_only` in 44 §2.2's commit body, and a long-lived server has
/// no other road to it: `Engine::with_mode` is a builder consumed at `open`, and the alternative --
/// "serve swapping the mode via `&mut self` per request" -- is the form §47 M6-08 ruled **must not be
/// adopted** (sem: SEM-gx-engine-851) because a posture written onto shared state leaks into the next request.
/// So `canonicalize` takes the argument `verify` already took, and this is the pair of runs that
/// shows it is an **override** and not a default: one engine, one posture, two answers.
#[test]
fn a_per_call_record_only_opens_t8r_on_an_enforcing_engine() {
    let mut engine = engine_with("record_only_per_call_canonicalize");
    let denied = deny_one(&mut engine, "/tmp/record-only-per-call.txt");

    let refused = engine.canonicalize(&denied, AT, None).expect_err(
        "`None` means \"this engine's posture\" (sem: SEM-gx-engine-852), which is Enforce",
    );
    println!("PER_CALL_NONE refused={:?}", refused.kind());
    assert_eq!(refused.kind(), "InvalidState");

    let canonicalized = engine
        .canonicalize(&denied, AT, Some(EnforcementMode::RecordOnly))
        .expect("E-M6-20: the per-call override opens T-8r");
    println!(
        "PER_CALL_RECORD_ONLY state={canonicalized:?} enforced={:?}",
        engine.enforced(&denied)
    );
    assert_eq!(canonicalized, Lifecycle::Canonicalized);
    assert_eq!(
        engine.enforced(&denied),
        Some(false),
        "the flag follows the transformation whichever road opened T-8r (E-M5-8)"
    );
}

/// 🔴 And the override runs the other way: `Some(Enforce)` refuses on a record-only engine.
///
/// One direction alone would leave "the argument is read" and "the argument is only ever read as
/// permission" (sem: SEM-gx-engine-853) indistinguishable. 44 §2.2's body is `bool`, so `false` has to **mean** something, and
/// what it means is that a caller can insist on enforcement on a server configured not to enforce.
#[test]
fn a_per_call_enforce_refuses_on_a_record_only_engine() {
    let mut engine =
        engine_with("record_only_per_call_enforce").with_mode(EnforcementMode::RecordOnly);
    let denied = deny_one(&mut engine, "/tmp/record-only-per-call-enforce.txt");

    let refused = engine
        .canonicalize(&denied, AT, Some(EnforcementMode::Enforce))
        .expect_err("an explicit Enforce overrides the engine's RecordOnly");
    println!("PER_CALL_ENFORCE refused={:?}", refused.kind());
    assert_eq!(refused.kind(), "InvalidState");

    // …and the engine is unchanged by the refusal: the next call, with no argument, still gets the
    // engine's own posture. An override that stuck would be the leak M6-08 forbids.
    let canonicalized = engine
        .canonicalize(&denied, AT, None)
        .expect("the engine's posture is still RecordOnly");
    println!("PER_CALL_AFTER state={canonicalized:?}");
    assert_eq!(canonicalized, Lifecycle::Canonicalized);
}
