// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **I-2** (`req/38_ERRATA_2026-08-07.md` §26) — the `source.id()` component of E-M3-12's key, put
//! where a test can see it.
//!
//! # The finding
//!
//! req/67 §2.1's battery B-4 removed `source.id()` from `Reason::canonical_cmp`'s key and every
//! suite stayed green (RC=0). The audit's reading:
//!
//! > "**I-2**: dropping the `source.id()` component of E-M3-12's ruled key breaks no test ... the
//! > reason is that the `message` tie-break added in E-4 effectively proxies for id -- the current
//! > producers (`deny_reasons` / `violations`) spell the id into the message. In the state **'one
//! > component of the ruled key is unobservable'**, the day the message's wording changes, the order
//! > silently reverts to depending on arrival" (sem: SEM-gx-gate-231)
//!
//! `verdict_meet.rs`'s `e_m3_12_a_deny_is_recorded_in_one_order` builds its messages as
//! `format!("{id} forbids this")`, so the id order and the message order agree in every case it
//! holds — and an ordering that has lost its third component still answers correctly. The ruling
//! adopted the fixture that separates them:
//!
//! > "**I-2** (adopted = fix batch): assert that `Verdict::deny`'s output is in **id order** using a
//! > fixture where id order and message order disagree (`Policy{"b"}`/message "a..." vs
//! > `Policy{"a"}`/message "z...")" (sem: SEM-gx-gate-232)
//!
//! # Why this is about identity and not about presentation
//!
//! E-M3-12 exists because M2-10 makes the `Vec<Reason>` the value a `Deny`'s `proof_digest` is taken
//! over (42 §3.10). An order that depends on which system was consulted first is arrival order
//! inside a CID — "the decision is fixed, the record is mutable" (sem: SEM-gx-gate-233), the defect D-1 was ruled to remove. So the assertions below
//! are made twice: once on the sequence, and once on the digest, because the second is the one the
//! rule was written for.

use gx_gate::{Reason, ReasonSource, Verdict, POLICY_DENIED};

/// A refusal from a policy, with the id and the message chosen independently.
fn refusal(policy_id: &str, message: &str) -> Reason {
    Reason::new(
        POLICY_DENIED,
        message.to_string(),
        ReasonSource::Policy {
            policy_id: policy_id.to_string(),
        },
    )
    .expect("POLICY_DENIED is in REASON_CODES")
}

fn recorded(reasons: Vec<Reason>) -> Vec<(String, String)> {
    let Verdict::Deny(reasons) = Verdict::deny(reasons).expect("at least one reason") else {
        panic!("built as a Deny")
    };
    reasons
        .iter()
        .map(|r| (r.source().id().to_string(), r.message().to_string()))
        .collect()
}

/// The two reasons of the ruled fixture. Same rank (both `Policy`), same code, and the id order is
/// the reverse of the message order — so an ordering that reads only the message answers `["b",
/// "a"]` and one that reads the id answers `["a", "b"]`.
fn crossed_pair() -> (Reason, Reason) {
    (
        refusal("b", "a change to the boot partition"),
        refusal("a", "z-index is not a filesystem concept"),
    )
}

/// The output is in id order, and the message order is the opposite one (**E-M3-12**, I-2).
#[test]
fn a_deny_is_ordered_by_the_source_id_and_not_by_the_message() {
    let (b_early_message, a_late_message) = crossed_pair();
    let seen = recorded(vec![b_early_message, a_late_message]);

    let ids: Vec<&str> = seen.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        ["a", "b"],
        "E-M3-12's key is (source kind, id, code) with `message` only as the tie-break (E-4); the \
         recorded order is {seen:?}"
    );

    let messages: Vec<&str> = seen.iter().map(|(_, m)| m.as_str()).collect();
    assert_eq!(
        messages,
        [
            "z-index is not a filesystem concept",
            "a change to the boot partition"
        ],
        "the fixture is only a fixture if the message order really disagrees with the id order"
    );
}

/// Neither input order changes what is recorded, and the two agree on the digest.
///
/// The sequence assertion above is about the order; this one is about M2-10's value. A key that has
/// lost a component still sorts *something* — what it stops doing is making the digest a function of
/// the refusal rather than of the order the two systems were consulted in.
#[test]
fn the_two_input_orders_of_the_crossed_pair_produce_one_deny() {
    let (b_early_message, a_late_message) = crossed_pair();
    let one =
        Verdict::deny(vec![b_early_message.clone(), a_late_message.clone()]).expect("two reasons");
    let other = Verdict::deny(vec![a_late_message, b_early_message]).expect("the same two");

    assert_eq!(
        one, other,
        "the order the reasons arrived in is not recorded"
    );
    assert_eq!(
        one.proof_digest().expect("digestible"),
        other.proof_digest().expect("digestible"),
        "M2-10 takes a Deny's proof_digest over the vector, so an unstable order is an unstable \
         receipt"
    );
}

/// With the ids equal, the message decides — E-4's fourth component, still doing its own work.
///
/// The test above could be satisfied by a key that dropped `message` instead of `id`, and that would
/// be the defect E-4 closed (two reasons agreeing on the ruled triple would keep the order they
/// arrived in). Both components are needed and this is the other half.
#[test]
fn reasons_that_agree_on_the_ruled_triple_are_ordered_by_their_message() {
    let seen = recorded(vec![
        refusal("same", "zulu"),
        refusal("same", "alpha"),
        refusal("same", "mike"),
    ]);

    let messages: Vec<&str> = seen.iter().map(|(_, m)| m.as_str()).collect();
    assert_eq!(
        messages,
        ["alpha", "mike", "zulu"],
        "E-4 made `message` the fourth key so that the triple's equivalence classes are ordered by \
         value rather than by arrival"
    );
}
