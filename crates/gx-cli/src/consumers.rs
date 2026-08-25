// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The accessors M5 armed with kill conditions, and who consumes them.
//!
//! Two tickets end here, and they are the same shape: an accessor exists, nothing calls it, and M5
//! refused to let "a hand will use it later" stand as an answer (sem: SEM-gx-cli-004). D-7's rule is that a deferral
//! carries a kill condition; M5H4-8 wrote one and M5FIX-3 wrote another.
//!
//! # M6-21 — `Gate::policies` / `Gate::invariants` / `PolicyEngine::is_empty`
//!
//! §41 M5H4-8 armed them: "**if the M6 reqdef names not a single concrete consumer
//! (`gx policy lint`/`gx serve`, etc.), M6 hand 1 carries out the retirement-mark form (E-7's
//! form)**" (sem: SEM-gx-cli-005). req/88 §4 M6-21 named three and req/38 §47 adopted (a)
//! dissolved the kill condition — with a second condition attached, because naming is cheap:
//!
//! > naming alone does not make a consumer -- **the relevant hand's DoD must include "machine-count
//! > each of the 3 accessors' call sites; 0 is RED"** (sem: SEM-gx-cli-006)
//!
//! [`GATE_ACCESSOR_CONSUMERS`] is that naming, in a form a probe reads
//! (`probes/doubt/tests/m6_surface_doubt.rs`). The wiring hand is **hand 4** and this is hand 1, so
//! the probe is armed on the file rather than asserted today: the moment a consumer's file exists,
//! the accessor has to be called in it. A probe that demanded three calls now would be red for three
//! hands and commented out by the second, which is how a gate becomes decoration.
//!
//! # M6-22 — `Receipt::signature_for`
//!
//! §46 M5FIX-3 left one survivor in gx-witness and sent the consumer question to M6. req/88 §4
//! M6-22 wrote the dependency: the answer depends on **M6-16**'s ruling about staged disclosure.
//! req/38 §47 settled M6-16 as adopted (a) — `gx receipt show --level 1..4` — and therefore settled M6-22 (sem: SEM-gx-cli-007)
//! as (b):
//!
//! > having adopted M6-16's (a) (staged disclosure `--level 1..4`), per the dependency it is
//! > wired as **(b) = the L4 (raw signature) output is `signature_for`'s consumer** (no retirement
//! > mark is stamped) (sem: SEM-gx-cli-008)
//!
//! So: **no retirement mark**, and the consumer is level 4 of `gx receipt show`, which is **hand 2**.
//! req/88 §6.2 hand 1 ⑧ asks this hand for exactly one thing about it — "one line of the
//! dependency in the doc (do not hide it)" (sem: SEM-gx-cli-009) —
//! and [`SIGNATURE_FOR_CONSUMER`] is that line in a place a probe can find it.

/// One armed accessor and the file that has to call it.
#[derive(Clone, Copy, Debug)]
pub struct AccessorConsumer {
    /// The accessor's name, as `pub fn <name>` in gx-gate's source.
    pub accessor: &'static str,
    /// The file that consumes it. Checked **if it exists**: this is the arming.
    pub consumer_path: &'static str,
    /// Which M6 hand wires it.
    pub hand: u8,
    /// Why this consumer and not another.
    pub because: &'static str,
}

/// 🔴 **M6-21** — the three, named.
///
/// The reasons are req/88 §4 M6-21's, kept beside the names because a name without a reason is the
/// thing M5H4-8 refused to accept.
pub const GATE_ACCESSOR_CONSUMERS: [AccessorConsumer; 3] = [
    AccessorConsumer {
        accessor: "policies",
        consumer_path: "crates/gx-cli/src/policy.rs",
        hand: 4,
        because: "`gx policy lint` reads how many policies a pack parsed into, and `gx serve`'s \
                  startup log (44 §1.2: \"startup log (structured JSON line)\") (sem: SEM-gx-cli-010) records which policy set it \
                  came up with. A gate whose policy set is invisible at startup is a gate nobody \
                  can testify about afterwards.",
    },
    AccessorConsumer {
        accessor: "invariants",
        consumer_path: "crates/gx-cli/src/policy.rs",
        hand: 4,
        because: "44 §1.2's `lint` text describes Cedar syntax only, but a `Verdict` is the \
                  composition of policy **and** invariant (FR-027). A diagnostic that checked one \
                  half would misreport what it checked, which is M4H4-2's \"do not give the unimplemented \
                  and a failure the same face\" wearing a linter's clothes. (sem: SEM-gx-cli-011)",
    },
    AccessorConsumer {
        accessor: "is_empty",
        consumer_path: "crates/gx-cli/src/policy.rs",
        hand: 4,
        because: "\"came up with zero policies\" (sem: SEM-gx-cli-012) is the most dangerous configuration a fail-closed \
                  deployment can be in — the gate is present and decides nothing — and it is \
                  invisible unless something asks. This is the startup warning.",
    },
];

/// 🔴 **M6-22** — the dependency, in writing.
///
/// Not a retirement mark: req/38 §47 wired it instead, and it is wired to a hand that is not this
/// one. The sentence exists so that a later reader finds the decision rather than an unused
/// accessor and a guess about it.
pub const SIGNATURE_FOR_CONSUMER: &str = "\
`Receipt::signature_for` is not retired. req/38 §47 settled M6-16 as adopted (a) (sem: SEM-gx-cli-013) (staged disclosure, \
`gx receipt show --level 1..4`), and M6-22 follows it as (b): **level 4 — the raw signatures — is \
the consumer**, and it is written in M6 hand 2. Hand 1 owns none of it; what hand 1 owes is this \
line, because a dependency between two rulings that lives only in a report is a dependency the next \
hand re-decides.";

/// The hand that wires [`SIGNATURE_FOR_CONSUMER`].
pub const SIGNATURE_FOR_HAND: u8 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    /// Three, with a reason each, and all of them pointing at a hand later than this one.
    #[test]
    fn every_armed_accessor_names_a_hand_and_a_reason() {
        assert_eq!(GATE_ACCESSOR_CONSUMERS.len(), 3);
        for c in GATE_ACCESSOR_CONSUMERS {
            assert!(c.hand > 1, "{} is wired in a later hand", c.accessor);
            assert!(
                c.because.len() > 80,
                "{}'s reason is too short to be one",
                c.accessor
            );
        }
        assert_eq!(SIGNATURE_FOR_HAND, 2);
        assert!(SIGNATURE_FOR_CONSUMER.contains("M6-16"));
    }
}
