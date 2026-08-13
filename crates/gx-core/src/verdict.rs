//! Which of the three verdicts a receipt is about, as a type rather than a string (**E-M3-2**).
//!
//! Spec: 41 §4 for `Verdict`'s three arms, 42 §3.10 for the receipt field that names one,
//! **E-M3-2** (`req/38_ERRATA_2026-08-07.md` §19) for why the discriminant lives here.
//!
//! # The cycle this file is shaped to avoid
//!
//! 42 §0 files `VerdictKind` under `gx-engine` (M5) and `Verdict` under `gx-gate` (M3), and 42
//! §3.10 gives `ReceiptPayload.verdict` a `VerdictSummary { kind, proof_digest }` in gx-witness
//! (M2). Meanwhile 41 §4 types `GateInput.evidence` as `&[Evidence]`, so **gx-gate names
//! gx-witness**. Read literally, gx-witness would have had to name gx-gate to type its own
//! discriminant, and the two crates would form the cycle E-M2-1 removed once already:
//!
//! > 「M3-13(採用): `VerdictKind`(3 値 enum)を **gx-core** へ、`Verdict`(payload つき)は gx-gate。
//! > gx-witness の `VERDICT_KINDS` 文字列検査は型検査へ置換(H5-8 満期)。循環 0 が機械条件」
//!
//! Same rule as `Cid` (A-1), `InclusionProof` (E-M2-1) and `PlannedDeltaBytes` (E-M3-1): **the data
//! comes down, the computation stays up**. The three names are here; the payloads each arm carries
//! -- `AdmitProof`, `Vec<Reason>`, `EscalationTicket` -- stay in gx-gate, where the evaluation that
//! fills them is.
//!
//! # What H5-8 promised, and what it cost until now
//!
//! M2 hand 5 could not write this type (req/49 §1 N-03 forbade minting an M5 name early), so
//! `receipt.rs` carried `pub const VERDICT_KINDS: [&str; 3]` and refused an unknown spelling at
//! *verification* time. The ruling recorded the debt as 「M3 で `VerdictKind` が生えたら型化へ
//! 寄せる」, and this is the hand it falls due. The difference is when the error is found: a
//! payload whose `kind` is `"Admitted"` now fails to decode, where before it decoded and was
//! rejected two calls later.

use serde::{Deserialize, Serialize};

use core::fmt;

/// The three verdicts, 42 §3.10's spellings (**E-M3-2**).
///
/// Serialising a fieldless variant writes the variant's name, so the wire face of this enum is the
/// three text strings 42 §3.10 already fixed -- `"Admit"`, `"Deny"`, `"Escalate"` -- and receipts
/// written before this type existed decode into it unchanged. `gx-witness/tests/pae_golden.rs` is
/// what says so mechanically: it pins signed bytes, and those bytes did not move when the field's
/// type did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum VerdictKind {
    /// The transformation may proceed. Carries an `AdmitProof` in gx-gate's `Verdict`.
    Admit,
    /// It may not. Carries the reasons.
    Deny,
    /// Nobody here can say; a human decides (43 T-5). Carries the ticket.
    Escalate,
}

impl VerdictKind {
    /// All three, in 42 §3.10's order.
    ///
    /// Declared once, like [`crate::TheoremId::ALL`], so that a test enumerating the verdicts reads
    /// the implementation instead of restating it -- a second list is a list that can drift.
    pub const ALL: [VerdictKind; 3] =
        [VerdictKind::Admit, VerdictKind::Deny, VerdictKind::Escalate];

    /// 42 §3.10's spelling, for the places a string is what the format holds.
    ///
    /// This is the same text serde writes, and the two are not allowed to disagree:
    /// `gx-core/tests/m3_types.rs` round-trips every variant through JSON and compares.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            VerdictKind::Admit => "Admit",
            VerdictKind::Deny => "Deny",
            VerdictKind::Escalate => "Escalate",
        }
    }
}

impl fmt::Display for VerdictKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
