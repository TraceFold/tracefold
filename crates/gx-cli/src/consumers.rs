//! The accessors M5 armed with kill conditions, and who consumes them.
//!
//! Two tickets end here, and they are the same shape: an accessor exists, nothing calls it, and M5
//! refused to let 「a hand will use it later」 stand as an answer. D-7's rule is that a deferral
//! carries a kill condition; M5H4-8 wrote one and M5FIX-3 wrote another.
//!
//! # M6-21 — `Gate::policies` / `Gate::invariants` / `PolicyEngine::is_empty`
//!
//! §41 M5H4-8 armed them: 「**M6 reqdef が具体的な消費者(`gx policy lint`/`gx serve` 等)を 1 つも名指せ
//! なければ、M6 手 1 で退役印形(E-7 の形)を実施**」. req/88 §4 M6-21 named three and req/38 §47 採(a)
//! dissolved the kill condition — with a second condition attached, because naming is cheap:
//!
//! > 名指しただけでは消費者にならない——**該当手の DoD に「3 accessor それぞれの呼び出し箇所を機械で
//! > 数え、0 なら RED」を含める**
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
//! req/38 §47 settled M6-16 as 採(a) — `gx receipt show --level 1..4` — and therefore settled M6-22
//! as (b):
//!
//! > M6-16 採(a)(段階開示 `--level 1..4`)を採用したので従属どおり **(b)=L4(生署名)出力が
//! > `signature_for` の消費者**として結線(退役印は打たない)
//!
//! So: **no retirement mark**, and the consumer is level 4 of `gx receipt show`, which is **hand 2**.
//! req/88 §6.2 手 1 ⑧ asks this hand for exactly one thing about it — 「従属を doc に 1 行(隠さない)」 —
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
                  startup log (44 §1.2: 「起動ログ（構造化JSON行）」) records which policy set it \
                  came up with. A gate whose policy set is invisible at startup is a gate nobody \
                  can testify about afterwards.",
    },
    AccessorConsumer {
        accessor: "invariants",
        consumer_path: "crates/gx-cli/src/policy.rs",
        hand: 4,
        because: "44 §1.2's `lint` text describes Cedar syntax only, but a `Verdict` is the \
                  composition of policy **and** invariant (FR-027). A diagnostic that checked one \
                  half would misreport what it checked, which is M4H4-2's 「未実装と失敗を同じ顔に \
                  するな」 wearing a linter's clothes.",
    },
    AccessorConsumer {
        accessor: "is_empty",
        consumer_path: "crates/gx-cli/src/policy.rs",
        hand: 4,
        because: "「policy 0 本で立った」 is the most dangerous configuration a fail-closed \
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
`Receipt::signature_for` is not retired. req/38 §47 settled M6-16 as 採(a) (staged disclosure, \
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
