//! 🔴 **M6-09** — 44 §2.3's twelve `gx_code`s, and the one place every refusal is mapped onto them.
//!
//! > **(a)** 写像表を **1 箇所**に置く+写らない refusal は `INTERNAL`(500) へ畳み、**畳んだ事実を doc
//! > に 1 行**  … 写像表には **1 行 1 refusal・`_` arm 禁**(E-M2-23 / H-3 の形)を課し、engine に
//! > variant が増えたら compile error になる形にする
//!
//! # 🔴 The quotient, with its denominator (req/88 §3 Λ4)
//!
//! Λ4's claim is that this map 「は refusal 空間の商写像であり、全射でも単射でもない。情報は必ず
//! 落ちる」, and that the discipline is therefore not 「畳むな」 but 「畳んだ事を書け」. So the
//! denominator is counted rather than described:
//!
//! | source | refusal kinds |
//! |---|---|
//! | `gx_engine::ERROR_KINDS` | 14 |
//! | `gx_gate::ERROR_KINDS` | 6 |
//! | `gx_witness::ERROR_KINDS` | 8 |
//! | `gx_log::ERROR_KINDS` | 5 |
//! | **total** | **33** |
//!
//! onto 44 §2.3's **12**. [`REFUSALS`] carries one row per refusal kind, all thirty-three, and
//! [`FOLDS`] is the list Λ4 asks to be written down: the rows whose meaning does not survive the
//! code they land on.
//!
//! # 🔴 The `_` arm, and the honest form of 「compile error」
//!
//! §47's ruling asks that 「engine に variant が増えたら compile error」. Three of the four enums are
//! `#[non_exhaustive]` (gx-engine, gx-witness, gx-log), which means **a `match` written in this
//! crate is required by the language to have a wildcard arm** — the exact shape the ruling forbids.
//! There is no way to have both from outside those crates.
//!
//! So the map does not `match` at all. Each crate declares its own vocabulary as an array
//! (E-M2-23's 「crate 毎 Error 語彙表を 1 箇所宣言」) and answers `Error::kind()`, whose `match` lives
//! **inside** the defining crate where `#[non_exhaustive]` does not bind and where no `_` arm is
//! written. This file maps `&'static str` to code, and the compile error the ruling asks for is a
//! [`const` assertion](DENOMINATOR) against each array's length: a new variant obliges its own crate
//! to grow its array (each crate's own probe enforces that), and the moment the array grows, this
//! file stops compiling.
//!
//! 🔴 Two of those arrays did not exist before this hand — gx-witness's and gx-log's — and their
//! absence is why nothing had ever counted the denominator. Raised as **M6H5-2**.
//!
//! # What is *not* here
//!
//! `IDEMPOTENCY_CONFLICT` and `UNAUTHORIZED` are 44 §2.3 codes with no refusal kind behind them:
//! they are produced by [`crate::idempotency`] and [`crate::auth`], which are HTTP-layer facts and
//! not any crate's `Error`. They are in [`GX_CODES`] and absent from [`REFUSALS`], and the two
//! tables' disagreement on exactly those two is asserted rather than tolerated.

/// One row of 44 §2.3's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GxCode {
    /// The machine-readable code (44 §2.3: 「機械可読コード（CLI終了コードおよび`Reason.code`と語彙を
    /// 共有）」).
    pub code: &'static str,
    /// The HTTP status 44 §2.3 pairs it with.
    pub status: u16,
    /// The CLI exit status 44 §2.3's third column gives it.
    pub cli_exit: u8,
    /// 44 §2.3's own fourth column, verbatim.
    pub meaning: &'static str,
}

/// 🔴 44 §2.3's twelve, transcribed **once**.
///
/// `probes/doubt/tests/m6_gx_code.rs` parses the same table out of
/// `req/spec/40-architecture/44-api-spec.md` and compares all four columns, which is the I-11 shape
/// (`adapter_spec.rs`: 「正本の markdown を parse して実型と突合」). A table that lived only here
/// would be a second copy of 44 §2.3, and two copies of a table are two things that drift — which is
/// precisely what M6-25 found between 44 §1.2's per-command lists and §1.4's common table.
pub const GX_CODES: [GxCode; 12] = [
    GxCode {
        code: "VALIDATION_ERROR",
        status: 422,
        cli_exit: 1,
        meaning: "リクエスト不正",
    },
    GxCode {
        code: "NOT_FOUND",
        status: 404,
        cli_exit: 6,
        meaning: "対象ID不在",
    },
    GxCode {
        code: "NOT_ADMITTED",
        status: 403,
        cli_exit: 2,
        meaning: "Verdict≠Admitでcommit不可（非record-only）",
    },
    GxCode {
        code: "PRECONDITION_CHANGED",
        status: 409,
        cli_exit: 3,
        meaning: "CAS失敗（CON-2）",
    },
    GxCode {
        code: "APPLY_FAILED",
        status: 422,
        cli_exit: 5,
        meaning: "adapter.apply失敗",
    },
    GxCode {
        code: "ESCALATION_PENDING",
        status: 409,
        cli_exit: 4,
        meaning: "Escalated状態のまま操作不可",
    },
    GxCode {
        code: "INVERSE_UNAVAILABLE",
        status: 409,
        cli_exit: 1,
        meaning: "undo対象の逆deltaが利用不可",
    },
    GxCode {
        code: "IDEMPOTENCY_CONFLICT",
        status: 409,
        cli_exit: 1,
        meaning: "同一Idempotency-Keyで異なるリクエスト本文",
    },
    GxCode {
        code: "ADAPTER_ERROR",
        status: 502,
        cli_exit: 1,
        meaning: "substrate adapter内部エラー",
    },
    GxCode {
        code: "POLICY_ERROR",
        status: 500,
        cli_exit: 1,
        meaning: "Cedar評価自体の失敗（policyバグ等）",
    },
    GxCode {
        code: "UNAUTHORIZED",
        status: 401,
        cli_exit: 1,
        meaning: "認証失敗（§2.6）",
    },
    GxCode {
        code: "INTERNAL",
        status: 500,
        cli_exit: 1,
        meaning: "分類不能な内部エラー",
    },
];

/// 🔴 44 §2.2's own name for a transition asked for from the wrong state, which §2.3's table omits.
///
/// `POST /candidates/{id}/verify`, `/escalation` and `/cancel` all specify 「`409`（…,
/// `gx_code=INVALID_STATE`）」 in §2.2, and §2.3's twelve-row table does **not** contain it. That is a
/// gap inside one document rather than a choice this hand gets to make, so the constant is named
/// here, used where §2.2 names it, and raised as **M6H5-3**. `GX_CODES` is left at twelve because it
/// is a transcription of §2.3 and a transcription that quietly gained a row would stop being one.
pub const INVALID_STATE: &str = "INVALID_STATE";

/// The status 44 §2.2 gives [`INVALID_STATE`] at every one of its three sites.
pub const INVALID_STATE_STATUS: u16 = 409;

/// 🔴 **E-M6-22** (§53, M6H6-4 採(a)) — the code 44 has no word for: *the server is going away*.
///
/// > **M6H6-4 採(a)=E-M6-22**: `UNAVAILABLE`(503・CLI 1)を 44 §2.3 の後方互換な追加として読む
/// > (運用 code の不在=手5 KeyPermissions と同根の解)。実装窓=手7 以降最初に触る手
///
/// 44 §2.3's twelve codes are twelve words about a **request** — 「リクエスト不正」, 「対象ID不在」,
/// 「Verdict≠Admit」 — and not one of them is a word about a **server**. Hand 5 hit the wall folding
/// `KeyPermissions` and hand 6 hit it again when graceful shutdown's stage 1 had to refuse a request
/// the server was perfectly capable of serving a second earlier. Both folded to `INTERNAL`, both said
/// so, and §53 ruled the addition.
///
/// 🔴 What the fold cost, stated as behaviour rather than as tidiness: a client's retry policy reads
/// `gx_code`. `INTERNAL` means 「this server made a mistake」 and invites a bug report;
/// `UNAVAILABLE` means 「this server is leaving and the replacement will answer」 and invites a
/// retry. One of those two instructions was wrong for every shutdown this server has ever performed.
pub const UNAVAILABLE: &str = "UNAVAILABLE";

/// The status [`UNAVAILABLE`] carries. Unchanged by the erratum: hand 6's status was already honest
/// and only the code was a fold.
pub const UNAVAILABLE_STATUS: u16 = 503;

/// 44 §1.4's exit for [`UNAVAILABLE`] — **1**, 「エラー（入力不正・内部エラー・adapterエラー）」.
///
/// 🔴 There is no exit value for 「the server went away」, which is M6H6-3's other half: 44 gives
/// `gx serve` 0 and 1 and nothing else, so the CLI column of this row is itself a fold. Named here
/// so that the fold is one line of code somebody can find rather than a sentence in one report.
pub const UNAVAILABLE_CLI_EXIT: u8 = 1;

/// 🔴 The codes 44 §2.3 does **not** have and a ruling gave this repository.
///
/// Two, both from 44 §2.6's 「後方互換な追加」 clause and each ruled in `req/38`:
///
/// | code | status | CLI | ruling | why 44 has no row |
/// |---|---|---|---|---|
/// | `INVALID_STATE` | 409 | 2 | **E-M6-18** (§52, M6H5-3) | §2.2 names it three times; §2.3 omits it |
/// | `UNAVAILABLE` | 503 | 1 | **E-M6-22** (§53, M6H6-4) | §2.3 has no word about the server |
///
/// Kept **out** of [`GX_CODES`], which is a transcription of §2.3 and would stop being one the
/// moment it gained a row nobody wrote there. `probes/doubt/tests/m6_gx_code.rs` parses the markdown
/// and compares it with `GX_CODES`; this list is the place a reader looks for the difference between
/// 「what 44 says」 and 「what this server sends」, which is a difference that has to be findable.
pub const RULED_ADDITIONS: [GxCode; 2] = [
    GxCode {
        code: INVALID_STATE,
        status: INVALID_STATE_STATUS,
        cli_exit: 2,
        meaning: "状態機械が許さない遷移（E-M6-18・44 §2.2 が 3 箇所で名指し §2.3 に行が無い）",
    },
    GxCode {
        code: UNAVAILABLE,
        status: UNAVAILABLE_STATUS,
        cli_exit: UNAVAILABLE_CLI_EXIT,
        meaning: "サーバが停止中で新規リクエストを受けない（E-M6-22・§2.3 に運用 code が無い）",
    },
];

/// Where a refusal kind came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// `gx_engine::ERROR_KINDS`.
    Engine,
    /// `gx_gate::ERROR_KINDS`.
    Gate,
    /// `gx_witness::ERROR_KINDS`.
    Witness,
    /// `gx_log::ERROR_KINDS`.
    Log,
}

impl Origin {
    /// The crate's declared vocabulary, so that a caller can count what it is mapping.
    #[must_use]
    pub fn vocabulary(self) -> &'static [&'static str] {
        match self {
            Origin::Engine => &gx_engine::ERROR_KINDS,
            Origin::Gate => &gx_gate::ERROR_KINDS,
            Origin::Witness => &gx_witness::ERROR_KINDS,
            Origin::Log => &gx_log::ERROR_KINDS,
        }
    }
}

/// One refusal kind, and what 44 §2.3 calls it.
#[derive(Clone, Copy, Debug)]
pub struct Refusal {
    /// Which crate's vocabulary.
    pub origin: Origin,
    /// The `Error::kind()` string.
    pub kind: &'static str,
    /// The `gx_code` it is answered with.
    pub code: &'static str,
    /// 🔴 Whether meaning is lost on the way (Λ4). `Some(..)` is a row of [`FOLDS`].
    pub fold: Option<&'static str>,
}

/// 🔴 **The map** — thirty-three refusal kinds, one row each, in vocabulary order.
///
/// Read it as Λ4's quotient made explicit. What a reader should be able to do with it is exactly
/// what a `_` arm prevents: look up a refusal they saw in a log and find the sentence explaining why
/// it wears the code it wears.
pub const REFUSALS: [Refusal; 33] = [
    // ---- gx_engine::ERROR_KINDS (14) -------------------------------------------------
    Refusal {
        origin: Origin::Engine,
        kind: "Canon",
        code: "INTERNAL",
        fold: Some(
            "gx-canon refusing means a value the engine built has no canonical form. That is this \
             system's bug and never the request's, so it is not `VALIDATION_ERROR`; 44 §2.3 has no \
             code for 「the server built something unencodable」 and `INTERNAL` 「分類不能な内部 \
             エラー」 is the honest bucket.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Core",
        code: "VALIDATION_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Io",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Malformed",
        code: "VALIDATION_ERROR",
        fold: Some(
            "Two producers with two different subjects (gx-engine's own note says so): a journal \
             record over `MAX_RECORD_BYTES` is the server's fault and a blob that does not hash to \
             its own name is a stored artefact's. Neither is 「リクエスト不正」 in the ordinary \
             sense, and both arrive on the road a request opened. 422 is the closer of 44's two \
             wrong answers because it is the one that does not tell a client to retry.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "InconsistentEscrow",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "InconsistentTicket",
        code: "INTERNAL",
        fold: Some(
            "E-6's checked constructor refusing means a ticket's id disagrees with its contents — a \
             value **claiming** to hash to its own name. That is a collaborator being wrong, which \
             44 §2.3 has no word for at all; `POLICY_ERROR` would blame Cedar for a digest.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "EvidenceUnavailable",
        code: "INTERNAL",
        fold: Some(
            "🔴 The one refusal that normally never reaches this map: 43 T-4d turns it into \
             `AbortReason::VerifierUnavailable` **inside** `Engine::verify`, which is a state and \
             not an error. It arrives here only where a caller sees the `Err` directly. 44 §2.3 has \
             no 「the verifier could not be reached」 and 503 is not one of its statuses.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "NotIdempotent",
        code: "INTERNAL",
        fold: Some(
            "AC-033's `canon(canon(x)) != canon(x)`. A broken canonicaliser is the deepest \
             invariant this system has and `INTERNAL` is the shallowest code 44 offers; the JSON \
             `detail` carries the transformation so the fold is readable.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "NotFound",
        code: "NOT_FOUND",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "InvalidState",
        code: INVALID_STATE,
        fold: Some(
            "🔴 `INVALID_STATE` is 44 **§2.2**'s word and is not in §2.3's twelve-row table. See \
             `INVALID_STATE` above and M6H5-3: the endpoint sections name it three times and the \
             code table does not carry it.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Adapter",
        code: "ADAPTER_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Ledger",
        code: "INTERNAL",
        fold: Some(
            "The engine's own note distinguishes two facts here — 「the ledger already holds a \
             different receipt for this transformation」 (INV-S3's guard, an exactly-once violation) \
             and 「the ledger could not be fsynced」 (a durability failure) — and 44 §2.3 has one \
             code for both. The `action` field survives in `detail`.",
        ),
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Witness",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Engine,
        kind: "Unrepresentable",
        code: "INTERNAL",
        fold: Some(
            "🔴 **M5FIX-8's first of two.** 「the canon has no way to write down what happened」 — a \
             T-4e degraded admission whose `CommitReceipt` would need a `proof_digest` for a gate \
             nobody asked. req/88 M6-09 names this exact row: it is not client input, so it is not \
             `VALIDATION_ERROR`, and it is not a failure either, so `INTERNAL` is a fold and not a \
             classification. Adding a code is 44 §2.6's 「後方互換な追加」 and is M6H5-4.",
        ),
    },
    // ---- gx_gate::ERROR_KINDS (6) ----------------------------------------------------
    Refusal {
        origin: Origin::Gate,
        kind: "EmptyDeny",
        code: "POLICY_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Gate,
        kind: "NotDigestible",
        code: "POLICY_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Gate,
        kind: "PolicySetUnreadable",
        code: "POLICY_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Gate,
        kind: "RegistryUnusable",
        code: "POLICY_ERROR",
        fold: Some(
            "🔴 gx-gate's own source says it: 「**44 has no code for the invariant side**」 \
             (req/64 §4's erratum). 44 §2.3 defines `POLICY_ERROR` as 「Cedar評価自体の失敗（policy \
             バグ等）」 and an invariant registry that cannot name its invariants apart is not Cedar. \
             **E-M3-16** already ruled the repair — widen `POLICY_ERROR`'s description — and named \
             M6's API window as the implementation site, which is this row.",
        ),
    },
    Refusal {
        origin: Origin::Gate,
        kind: "Unevaluable",
        code: "POLICY_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Gate,
        kind: "UnknownReasonCode",
        code: "POLICY_ERROR",
        fold: None,
    },
    // ---- gx_witness::ERROR_KINDS (8) -------------------------------------------------
    Refusal {
        origin: Origin::Witness,
        kind: "SignatureInvalid",
        code: "VALIDATION_ERROR",
        fold: Some(
            "A receipt whose signature does not check is a **document the caller supplied** that is \
             not what it says it is, so 422 is right and 500 would be wrong. What is lost is that \
             this is the one refusal an attacker can provoke on purpose, and 44 gives it the same \
             code as a missing field.",
        ),
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Schema",
        code: "VALIDATION_ERROR",
        fold: Some(
            "🔴 **M5FIX-8's second of two**, and req/90 §2.4 measured it: 「`gx_witness::Error::\
             Schema`「receipt が legal でない」は入力起因だが `VALIDATION_ERROR` とも違う(渡された \
             物が壊れている)」. When the engine raises it the subject is the engine's own payload \
             (ASM-14's obligations) and when a verifier raises it the subject is the caller's file. \
             One code, two subjects.",
        ),
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Canon",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Log",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Io",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "KeyFormat",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Witness,
        kind: "KeyPermissions",
        code: "INTERNAL",
        fold: Some(
            "req/90 §2.4's third: 「`gx_witness::Error::KeyPermissions` は**運用の状態**であって \
             request の性質でない」. A key file group-readable on the server is a deployment fault a \
             client can neither cause nor repair, and 44 §2.3 has no operational code — 503 is not \
             among its statuses.",
        ),
    },
    Refusal {
        origin: Origin::Witness,
        kind: "Entropy",
        code: "INTERNAL",
        fold: None,
    },
    // ---- gx_log::ERROR_KINDS (5) -----------------------------------------------------
    Refusal {
        origin: Origin::Log,
        kind: "Canon",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Log,
        kind: "OutOfRange",
        code: "VALIDATION_ERROR",
        fold: None,
    },
    Refusal {
        origin: Origin::Log,
        kind: "Malformed",
        code: "VALIDATION_ERROR",
        fold: Some(
            "req/90 §2.4's first: 「`gx_log::Error::Malformed`「範囲外の tree size」は **client の \
             入力起因**であって内部エラーではない(`VALIDATION_ERROR` が正しい)」. Hand 2 folded it \
             into `INTERNAL` because 44 §1.2 gives the read side only 0 and 1; this hand has the \
             statuses and pays the debt. What is still lost is that the same variant also carries \
             「a proof shape this crate cannot read」, which is not the caller's doing.",
        ),
    },
    Refusal {
        origin: Origin::Log,
        kind: "Io",
        code: "INTERNAL",
        fold: None,
    },
    Refusal {
        origin: Origin::Log,
        kind: "Conflict",
        code: "INTERNAL",
        fold: Some(
            "ASM-43-1's guard: the ledger holds a **different** receipt under this transformation. \
             That is an exactly-once violation — the strongest thing this system can discover about \
             itself — and it lands on 「分類不能」 because 44 §2.3 has no word for it.",
        ),
    },
];

/// 🔴 The denominator, as a compile-time assertion (the honest form of 「variant が増えたら compile
/// error」 — see the module header).
///
/// Each crate's own probe keeps its `ERROR_KINDS` array in step with its enum, so a variant added
/// anywhere obliges an array to grow, and the moment one grows this constant stops evaluating.
pub const DENOMINATOR: usize = {
    let total = gx_engine::ERROR_KINDS.len()
        + gx_gate::ERROR_KINDS.len()
        + gx_witness::ERROR_KINDS.len()
        + gx_log::ERROR_KINDS.len();
    assert!(
        total == REFUSALS.len(),
        "M6-09: every declared refusal kind needs a row in REFUSALS. A crate grew its ERROR_KINDS \
         array and this map did not."
    );
    total
};

/// 🔴 The rows Λ4 asks to be written down: refusals whose meaning does not survive their code.
///
/// Not a defect list. Λ4 proves the map 「は…全射でも単射でもない。情報は必ず落ちる」, so a fold is
/// the normal case and the abnormal thing would be a table claiming none. What makes it a discipline
/// rather than a shrug is that each row says **what** was lost, in a sentence somebody can disagree
/// with.
#[must_use]
pub fn folds() -> Vec<&'static Refusal> {
    REFUSALS.iter().filter(|r| r.fold.is_some()).collect()
}

/// The `gx_code` for one refusal kind, or `None` if the vocabulary has no such row.
///
/// `None` is reachable only from a caller that invented a kind string; every value produced by an
/// `Error::kind()` in this workspace is in the table, and [`DENOMINATOR`] is why.
#[must_use]
pub fn of_kind(origin: Origin, kind: &str) -> Option<&'static Refusal> {
    REFUSALS
        .iter()
        .find(|r| r.origin == origin && r.kind == kind)
}

/// 44 §2.3's row for a code, by name.
#[must_use]
pub fn code(name: &str) -> Option<&'static GxCode> {
    GX_CODES.iter().find(|c| c.code == name)
}

/// The HTTP status one `gx_code` carries — 44 §2.3's second column, and [`INVALID_STATE`]'s 409.
#[must_use]
pub fn status_of(name: &str) -> u16 {
    if name == INVALID_STATE {
        return INVALID_STATE_STATUS;
    }
    code(name).map_or(500, |c| c.status)
}

/// 44 §2.3's `type` URI for a code.
///
/// 44 §2.3's example is `https://glovrex.dev/errors/precondition-changed`, which is the code
/// lowercased with `_` becoming `-`. gx-cli's `Error::problem` derives it the same way, and the two
/// derivations agreeing is what makes 「CLI終了コードおよび`Reason.code`と語彙を共有」 true of the
/// `type` field as well as of the code.
#[must_use]
pub fn type_uri(name: &str) -> String {
    format!(
        "https://glovrex.dev/errors/{}",
        name.to_lowercase().replace('_', "-")
    )
}
