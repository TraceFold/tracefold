//! `gx receipt show` / `gx receipt verify` — 44 §1.2, and **AC-057**.
//!
//! # 🔴 `.gx/receipts/` (M6H2-1): the local store 44 names and nothing implemented
//!
//! 44 §1.2: 「`show`: ローカルストア/gx-apiから`Receipt`を取得し表示」. There was no local store, and
//! the three places a receipt could have been are each ruled out by something:
//!
//! * `Engine::receipt` reads an in-memory table, and `Engine::open` leaves that table empty on
//!   purpose (M5H3-5) — a second `gx` process rebuilds the draft phase and the ledger, not the rows;
//! * the journal's thirteen record kinds hold no receipt (42 §3.13), and `Committed` carries
//!   `{transformation, ledger_seq, at}`;
//! * the ledger leaf carries a **digest** of one — 42 §3.11 keeps the body out so the leaf stays
//!   small.
//!
//! So `gx receipt show` was unimplementable, and with it M6-16's staged disclosure (§47 採(a)) and
//! M6-22, which hangs on it. [`ReceiptStore`] is the store; the writer is `gx commit`, which is
//! hand 3, and the arming for that is in [`crate::consumers`]'s shape rather than left implicit.
//!
//! # 🔴 M6-16 採(a): `--level 1..4`, and what level 4 is
//!
//! 48 §3.1's four layers are 「L1=verdict バッジ / L2=Receipt 要約 / L3=全展開（provenance 鎖・
//! evidence 一覧・fingerprints）/ L4=独立検証結果」. This implements L1–L3 as written and L4 as
//! **the raw signatures**, which is what §47 M6-22 採(b) settled: 「L4(生署名)出力が
//! `signature_for` の消費者」.
//!
//! The other half of 48's L4 — 「独立検証結果」 — is deliberately **not** here, and the reason is that
//! it already has a subcommand. `gx receipt verify` takes a key (a verification without one is
//! arithmetic about nothing) and `show` has no key argument in 44 §1.2. A `show` that verified would
//! be a second verifier in this binary, differing from the first in what it was given. Written down
//! rather than folded: req/88 §6.0-10.
//!
//! # 🔴 `checks.inclusion` is four values, not two (§5 行 4 / H5-9)
//!
//! 44 §1.2 writes `inclusion: bool|"skipped"`. `gx_witness::InclusionCheck` has **four**, and H5-9
//! ruled that `Unanchored` must not be reported as a pass. Folding the four into the two would put
//! 「the ledger claim was not checked」 under the same face as 「it was checked and held」, which is
//! req/29 §4's 「skip と pass を同じ顔にしない」. So the field carries four lowercase strings and
//! [`INCLUSION_JSON`] is the mapping back to 44's vocabulary. Raised as **M6H2-3**.

use std::path::{Path, PathBuf};

use gx_core::{Checkpoint, TransformationId};
use gx_witness::receipt::{
    Checks, InclusionCheck, Receipt, ReceiptPayload, RevocationCheck, RevocationPolicy,
};
use gx_witness::{PublicKey, VerifyingKeyRef};

use crate::exit::{Outcome, NOT_FOUND, VERIFY_FAILED};
use crate::{io, layout::Layout, Error, Result};

/// 🔴 The four values of `checks.inclusion`, and what 44 §1.2's two-value spelling calls each.
///
/// A table rather than a `match` in one function, because the divergence from 44 is the point and
/// `crates/gx-cli/tests/receipt_disclosure.rs` reads this to assert that the four are four. The
/// third column is empty where 44 has no word for the value — which is the whole of M6H2-3.
pub const INCLUSION_JSON: [(&str, &str); 4] = [
    // `NotApplicable` — a VerdictReceipt: ASM-14 says the ledger has seen nothing yet.
    ("not_applicable", "44 §1.2's `\"skipped\"`"),
    // `Verified` — the proof reached the anchor's root.
    ("verified", "44 §1.2's `true`"),
    // 🔴 `Refuted` — the proof did not reach the root. A forged proof, or an anchor from another log.
    ("refuted", "no word in 44 §1.2"),
    // 🔴 `Unanchored` — a CommitReceipt verified with no anchor. **Not** a pass (H5-9).
    ("unanchored", "no word in 44 §1.2"),
];

/// The JSON spelling of one [`InclusionCheck`].
#[must_use]
pub fn inclusion_json(check: InclusionCheck) -> &'static str {
    match check {
        InclusionCheck::NotApplicable => INCLUSION_JSON[0].0,
        InclusionCheck::Verified => INCLUSION_JSON[1].0,
        InclusionCheck::Refuted => INCLUSION_JSON[2].0,
        InclusionCheck::Unanchored => INCLUSION_JSON[3].0,
    }
}

/// 🔴 **M6H4-7** — which of a transformation's receipts a file holds.
///
/// > `.gx/receipts/` を `<TID>.<kind>.json`(kind ∈ verdict/ruling/commit)へ移行(手3 の writer と
/// > 手2 の reader を両方更新・移行の後方互換は不要=未配布)
///
/// # 🔴 Why a transformation has more than one receipt, and why one slot lost them
///
/// ASM-14 issues a `VerdictReceipt` for **every** verdict (43 T-4a/b/c) and 43 T-11 issues a
/// `CommitReceipt`, so an ordinary admitted-and-committed transformation ends with **two** signed
/// documents that are not each other: different `receipt_kind`, different payload — one attesting a
/// judgement, one attesting an application. An escalated one ends with **three**, and the third is
/// signed by a **different key**: 43 T-5's ruling receipt carries the ruler, not the submitter
/// (INV-S6), which is exactly the fact hand 4's battery point (l) measured.
///
/// Under `<TID>.json` those three shared one slot and the last writer won. That is not a naming
/// preference: 「who allowed this」 erasing 「what was decided」 is the loss INV-S6 exists to
/// prevent, and it happened in the directory req/56 §2 files as `Nature::Source` — 「失われる」.
///
/// The vocabulary is closed and the `match` below has no `_` arm, which is E-M2-23's shape one
/// milestone on: a fourth kind stops this file from compiling rather than landing in a file nobody
/// reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredKind {
    /// 43 T-4a/b/c's `VerdictReceipt`, signed by the engine's key.
    Verdict,
    /// 43 T-5 / T-5b's human ruling, signed by the **ruler's** key (INV-S6).
    Ruling,
    /// 43 T-11's `CommitReceipt`.
    Commit,
}

impl StoredKind {
    /// The infix in `<TID>.<kind>.json`.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            StoredKind::Verdict => "verdict",
            StoredKind::Ruling => "ruling",
            StoredKind::Commit => "commit",
        }
    }
}

/// 🔴 The order [`show`] looks in, most specific first.
///
/// `gx receipt show <TID>` names a transformation and not a document, so something has to choose.
/// The commit receipt is first because it is the one 44 §1.2's own example is about (「`Receipt`を
/// 取得し表示」 after a commit) and because it is the only one carrying an `inclusion_proof` — the
/// field a third party needs. The chosen kind is **printed**, so a reader is never left guessing
/// which of three documents they are looking at.
pub const DISCLOSURE_ORDER: [StoredKind; 3] =
    [StoredKind::Commit, StoredKind::Ruling, StoredKind::Verdict];

/// `.gx/receipts/`, as a store.
#[derive(Debug, Clone)]
pub struct ReceiptStore {
    dir: PathBuf,
}

impl ReceiptStore {
    /// The store inside an opened layout.
    #[must_use]
    pub fn in_layout(layout: &Layout) -> Self {
        Self {
            dir: layout.join("receipts"),
        }
    }

    /// The store at an explicit directory (what an export or a test points at).
    #[must_use]
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Where one receipt is filed (**M6H4-7**).
    ///
    /// The `TransformationId`'s text form with the `:` replaced, exactly as
    /// [`crate::draft::DraftStore::path_of`] does it and for the same reason (Windows), then the
    /// kind, then `.json`. The kind is an **argument** rather than a field read out of the receipt,
    /// for [`ReceiptStore::put`]'s reason: a receipt that chose its own filename could overwrite a
    /// document it is not.
    #[must_use]
    pub fn path_of(&self, id: &TransformationId, kind: StoredKind) -> PathBuf {
        self.dir.join(format!(
            "{}.{}.json",
            id.0.to_text().replace(':', "_"),
            kind.tag()
        ))
    }

    /// File a receipt under the transformation the **engine** named.
    ///
    /// The id is an argument and is not read out of the receipt, which is 則 1 in the signature: the
    /// payload's `transformation` field is a claim the receipt makes about itself, and a store that
    /// keyed on it would let a receipt choose its own filename.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory or the file cannot be written.
    pub fn put(
        &self,
        id: &TransformationId,
        kind: StoredKind,
        receipt: &Receipt,
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir).map_err(io("create", &self.dir))?;
        let path = self.path_of(id, kind);
        let body = serde_json::to_vec_pretty(receipt).map_err(|e| Error::Malformed {
            what: "receipt",
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        std::fs::write(&path, body).map_err(io("write", &path))?;
        Ok(path)
    }

    /// Read one back, if it is there.
    ///
    /// `Ok(None)` for 「no such receipt」 and `Err` for 「there is a file and it is not a receipt」
    /// (E-M4-35).
    ///
    /// # Errors
    /// [`Error::Io`] if the file exists and cannot be read; [`Error::Malformed`] if it is not a
    /// receipt.
    pub fn get(&self, id: &TransformationId, kind: StoredKind) -> Result<Option<Receipt>> {
        let path = self.path_of(id, kind);
        match std::fs::read(&path) {
            Ok(raw) => read_receipt(&raw, &path).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io("read", &path)(e)),
        }
    }

    /// 🔴 The first of [`DISCLOSURE_ORDER`] this store holds, and **which** it was.
    ///
    /// The half that matters is the second return value. A `show` that silently fell back from a
    /// commit receipt to a verdict receipt would answer 「here is the receipt」 about a document with
    /// no `inclusion_proof`, and a reader who could not tell the two apart would read 「this change
    /// was applied」 out of 「this change was judged」.
    ///
    /// # Errors
    /// [`Error::Malformed`] for a file that exists and is not a receipt.
    pub fn first_available(&self, id: &TransformationId) -> Result<Option<(StoredKind, Receipt)>> {
        for kind in DISCLOSURE_ORDER {
            if let Some(receipt) = self.get(id, kind)? {
                return Ok(Some((kind, receipt)));
            }
        }
        Ok(None)
    }
}

/// Decode a receipt from the JSON face 44 §2.2 fixes (「`payload`はbase64」).
fn read_receipt(raw: &[u8], whence: &Path) -> Result<Receipt> {
    serde_json::from_slice(raw).map_err(|detail| Error::Malformed {
        what: "receipt",
        path: whence.display().to_string(),
        detail: detail.to_string(),
    })
}

// ---------------------------------------------------------------------------
// `gx receipt show` — M6-16 採(a)
// ---------------------------------------------------------------------------

/// The highest disclosure level.
pub const MAX_LEVEL: u8 = 4;

/// 🔴 `gx receipt show <TID> --level 1..4` (44 §1.2, M6-16 採(a)).
///
/// `--json` 「は常に全量」 (M6-16 採(a)'s own clause: 「機械は段階開示を要らない・段階開示の根拠は
/// 人間の認知負荷」), which the caller expresses by passing `level = MAX_LEVEL`.
///
/// # Errors
/// [`Error::Usage`] for a level outside 1..=4. Everything else is an [`Outcome`]: a receipt that is
/// not there is 44 §1.2's `6=未検出` **with an object on stdout**, because a script that asked for a
/// receipt and got an exit status deserves to be told which id was missed.
pub fn show(store: &ReceiptStore, id: &TransformationId, level: u8) -> Result<Outcome> {
    if level == 0 || level > MAX_LEVEL {
        return Err(Error::Usage {
            detail: format!("--level takes 1..={MAX_LEVEL} (48 §3.1's four layers); got {level}"),
        });
    }
    let Some((kind, receipt)) = store.first_available(id)? else {
        return Ok(Outcome::refused(
            serde_json::json!({
                "transformation": id.0.to_text(),
                "found": false,
                // 🔴 **M6H4-7**: which names were looked for. A 「not found」 that does not say what
                // it looked for is the shape §30's ledger is about — an operator whose commit
                // receipt is missing and whose verdict receipt is present learns nothing from a
                // bare `false`.
                "looked_for": DISCLOSURE_ORDER.map(StoredKind::tag),
            }),
            NOT_FOUND,
        ));
    };
    let mut json = disclose(&receipt, level)?;
    if let Some(map) = json.as_object_mut() {
        // 🔴 **M6H4-7** — which of the three this is, at **every** level including L1.
        //
        // The payload's own `receipt_kind` (42 §3.10) distinguishes a `VerdictReceipt` from a
        // `CommitReceipt` and does **not** distinguish T-4c's from T-5's: both are `VerdictReceipt`
        // and they are signed by different keys. So the store's own key is printed beside it, and
        // the two fields together are the answer to 「which document am I reading」.
        map.insert("stored_kind".into(), kind.tag().into());
    }
    Ok(Outcome::ok(json))
}

/// The staged view of one receipt. Level `n` contains every field of level `n-1`.
///
/// # Errors
/// [`Error::Witness`] if the envelope's payload is not canonical DAG-CBOR of a receipt — which is
/// a **stored file that is wrong**, not a missing one, and is why this is not an `Option`.
pub fn disclose(receipt: &Receipt, level: u8) -> Result<serde_json::Value> {
    let payload: ReceiptPayload = receipt.payload()?;

    // L1 — 48 §3.1's 「verdict バッジ」. What a human needs to decide whether to read on.
    let mut out = serde_json::json!({
        "level": level,
        "transformation": payload.transformation.0.to_text(),
        "receipt_kind": payload.receipt_kind,
        // `Option<VerdictSummary>` is E-M5-11: under 43 T-4e the gate was never called, so there is
        // no verdict to summarise. `null` says that; an empty proof would not.
        "verdict": payload.verdict.as_ref().map(|v| v.kind),
        "enforced": payload.enforced,
    });
    if level == 1 {
        return Ok(out);
    }

    // L2 — 「Receipt 要約」.
    let map = out.as_object_mut().expect("built as an object");
    map.insert("key_id".into(), payload.key_id.clone().into());
    map.insert(
        "canonical_cid".into(),
        payload.canonical_cid.to_text().into(),
    );
    map.insert(
        "fail_posture_engaged".into(),
        payload.fail_posture_engaged.into(),
    );
    map.insert(
        "has_inclusion_proof".into(),
        payload.inclusion_proof.is_some().into(),
    );
    // 🔴 Nanoseconds, where 44 §0 asks for RFC 3339 (「日時はRFC 3339…内部`Timestamp`（ナノ秒epoch）
    // からの変換はAPI層の責務」). This workspace has no date library and adding one is a dependency
    // decision for the hand that owns the surface where `at` fields are everywhere (44 §2's fourteen
    // endpoints, hands 5 and 6). Emitting a wrong RFC 3339 string would be worse than emitting a
    // right integer, and a home-made civil-date conversion is a date library written badly. Raised
    // as **M6H2-4**; the field name says which unit it is in so nothing reads it as seconds.
    map.insert("issued_at_unix_nanos".into(), receipt.issued_at.0.into());
    if level == 2 {
        return Ok(out);
    }

    // L3 — 「全展開」. The eleven fields of 42 §3.10, through the type's own `Serialize` rather than
    // through a hand-written projection: a second spelling of the payload is a second thing to keep
    // in step with 42.
    let map = out.as_object_mut().expect("built as an object");
    map.insert(
        "payload".into(),
        serde_json::to_value(&payload).map_err(|e| Error::Malformed {
            what: "receipt payload",
            path: payload.transformation.0.to_text(),
            detail: e.to_string(),
        })?,
    );
    map.insert(
        "payload_type".into(),
        receipt.envelope.payload_type.clone().into(),
    );
    if level == 3 {
        return Ok(out);
    }

    // 🔴 L4 — the raw signatures. **M6-22 採(b)**: this is `Receipt::signature_for`'s consumer, and
    // the call is what discharges the accessor that M5FIX-3 left as gx-witness's one survivor.
    //
    // Asked for **by the id the payload declares**, not by iterating `signatures`. That is the
    // difference the accessor exists to make: 42 §3.10 requires the payload's `key_id` and the
    // envelope's `keyid` to agree, so 「the signature this receipt says signed it」 is a lookup and
    // not a position. An envelope carrying somebody else's signature and none of its own answers
    // `null` here, and that is the true answer rather than 「the first one」.
    let signature = receipt.signature_for(&payload.key_id).map(|s| {
        serde_json::json!({
            "keyid": s.keyid,
            "sig_b64": gx_core::b64::encode(&s.sig),
            "sig_bytes": s.sig.len(),
        })
    });
    let map = out.as_object_mut().expect("built as an object");
    map.insert(
        "signature".into(),
        signature.unwrap_or(serde_json::Value::Null),
    );
    map.insert(
        "signatures_in_envelope".into(),
        receipt.envelope.signatures.len().into(),
    );
    Ok(out)
}

// ---------------------------------------------------------------------------
// `gx receipt verify` — AC-018 / AC-019 / AC-057
// ---------------------------------------------------------------------------

/// Where the receipt to verify comes from.
pub enum Source<'a> {
    /// A file path.
    File(&'a Path),
    /// 44 §1.2's `-`: the bytes already read from stdin.
    Stdin(&'a [u8]),
}

/// 🔴 `gx receipt verify <FILE|-> [--offline] [--checkpoint <FILE>] --key <FILE>` (44 §1.2).
///
/// # 🔴 `--key` is not in 44 §1.2, and without it the command cannot exist (M6H2-6)
///
/// `gx_witness::verify_offline(receipt, key, anchor)` takes a key because a signature is only
/// checkable against one, and 44 §1.2's synopsis for `verify` has no key argument at all. AC-057
/// makes the gap concrete: it puts a receipt in an environment with no network and no gx server and
/// asks a **third party** to check it, and a third party has no `~/.gx/keys/`.
///
/// So `--key` is added, and it reads 44's own document rather than a new one: `gx key gen` prints
/// 「`{ "key_id": KEY_ID, "public_key": <base64> }`」 and that object is what `--key` accepts. It also
/// accepts a `gx` key file (the owner verifying their own receipts), because refusing to read a key
/// the same binary wrote would be a gratuitous second step. When `--key` is omitted the key is
/// looked up in the local key store under the id **the receipt declares**, which is a convenience
/// for the owner and is never available to the third party AC-057 is about.
///
/// # `--offline`, `--checkpoint` and 🔴 `--checkpoint-key` (**M6H8-11**)
///
/// 44 §1.2: 「`--offline`時は`--checkpoint`で与えた既知`Checkpoint`に対してのみ`inclusion_proof`の
/// 数学的整合性を検証し、ネットワークアクセスを行わない」. The checkpoint's **own** signature is not
/// checked by `gx_witness::verify_offline` — 44 calls the checkpoint 「既知」, i.e. already believed,
/// and a verification that checked it silently would make one `Ok` mean two things.
///
/// Hand 8 asked the question §49 reserved for it — 「偽 checkpoint を受け入れるか」 — and the answer was
/// **yes**: `gx_witness::dsse::verify_checkpoint` exists and had **zero callers in shipping code**,
/// so a checkpoint with an empty, broken or foreign signature was accepted as an anchor and the
/// output said `valid: true` with nothing recording what had not been checked. 44's own text is
/// satisfied by that; AC-057's third party is not, because they receive the receipt and the
/// checkpoint **from the same hand** and 「既知」 names nothing they hold (45 TH-6's split view).
///
/// req/38 §55 rules **M6H8-11 採(b), with (a) first**, and both halves are here:
///
/// * **(a)** every answer carries `anchor_authenticated`, and it is `false` unless something
///   authenticated the anchor. The field costs one line and changes no semantics; what it changes is
///   that 「what was not checked」 is on the wire rather than in a document. A flag alone would leave
///   whoever forgets it receiving the weaker verification silently, which is today's state.
/// * **(b)** `--checkpoint-key <FILE>` verifies the checkpoint's DSSE signature (45 ASM-45-1 allows a
///   different key from the receipt's, which is why it is a second flag and not the same `--key`) and
///   only then reports `anchor_authenticated: true`.
///
/// Always verifying was **not** adopted (§55 rejects (c)): it collides with 44's 「既知」 and would
/// shut out a third party who holds a checkpoint but not the log's key.
///
/// Without `--offline`, 44 asks for 「台帳照会」. v0.1 has no ledger client — `gx-api` serves nothing
/// until hands 5 and 6 — so the enquiry is against the **local** ledger, and `anchor_source` in the
/// output says which of the two happened. A command that said 「verified」 without saying against
/// what would be the shape req/29 §4 is about.
///
/// # Errors
/// [`Error::Usage`] for input that is not a receipt, [`Error::Io`] for a file that cannot be read,
/// [`Error::Witness`] for a receipt whose payload will not decode.
pub fn verify(
    source: &Source<'_>,
    key: &PublicKey,
    anchor: Option<&Checkpoint>,
    anchor_source: &'static str,
    anchor_authenticated: bool,
    revocation: Option<&RevocationPolicy<'_>>,
) -> Result<Outcome> {
    let (raw, whence) = match source {
        Source::File(path) => (
            std::fs::read(path).map_err(io("read", path))?,
            path.display().to_string(),
        ),
        Source::Stdin(bytes) => ((*bytes).to_vec(), "<stdin>".to_string()),
    };
    let receipt = read_receipt(&raw, Path::new(&whence))?;
    Ok(judge(
        &receipt,
        key,
        anchor,
        anchor_source,
        anchor_authenticated,
        revocation,
    ))
}

/// 🔴 `--revocations <FILE>`: the list this verification consults (**FR-M7-3**).
///
/// Authenticated against the key the receipt is being verified with, which is the only key a third
/// party holds. Entries about **other** keys are counted and ignored — a shared list names many keys
/// and a verifier holding one cannot vouch for the rest — while an entry about *this* key that this
/// key did not sign stops the read (`gx_witness::keys::RevocationLedger::from_signed`).
///
/// # Errors
/// [`Error::Io`] if the file cannot be read, [`Error::Malformed`] if it is not a list of DSSE
/// envelopes, [`Error::Witness`] if an entry about this key is not signed by it.
pub fn read_revocation_ledger(
    path: &Path,
    key: &PublicKey,
) -> Result<(gx_witness::RevocationLedger, usize)> {
    let envelopes = crate::keys::read_revocations(path)?;
    Ok(gx_witness::RevocationLedger::from_signed(
        &envelopes,
        &key.verifying(),
    )?)
}

/// The five words `checks.revocation` is spelled with, and the reason each one exists.
///
/// 44 §1.2 fixes no vocabulary here — the field is this hand's, as `anchor_authenticated` was M6 hand
/// 8's — so the words are declared beside their meanings rather than formatted at the call site.
pub const REVOCATION_JSON: [(&str, &str); 5] = [
    // No list was given. **Not** a claim that the key is fine: ASM-45-2 makes consulting optional
    // (「revocation list参照はverifier側任意」) and this word is what keeps a declined option from
    // reading as a completed check (req/29 §4).
    ("not_consulted", "no --revocations was given"),
    // A list was consulted and this key is not in it.
    ("not_revoked", "the list holds no revocation of this key"),
    // 🔴 A revocation exists and is dated after this verification.
    ("not_yet_in_force", "the revocation is dated later than now"),
    // ASM-45-2's default: the receipt predates the revocation.
    ("valid_at_issue", "issued before the revocation took effect"),
    // 🔴 Invalid. The signature is still valid; the key is not.
    (
        "revoked",
        "the key was revoked, and this receipt is refused",
    ),
];

/// The word for a [`RevocationCheck`].
#[must_use]
pub fn revocation_json(check: RevocationCheck) -> &'static str {
    match check {
        RevocationCheck::NotConsulted => REVOCATION_JSON[0].0,
        RevocationCheck::NotRevoked => REVOCATION_JSON[1].0,
        RevocationCheck::NotYetInForce => REVOCATION_JSON[2].0,
        RevocationCheck::ValidAtIssue => REVOCATION_JSON[3].0,
        RevocationCheck::Revoked => REVOCATION_JSON[4].0,
    }
}

/// 🔴 `--checkpoint-key <FILE>`: check the anchor's own DSSE signature (**M6H8-11 採(b)**).
///
/// The one caller of `gx_witness::dsse::verify_checkpoint` in shipping code. Before this batch that
/// function had none — the mirror image of M6-24, which found `sign_checkpoint` with no producer;
/// here the producer exists (`gx log checkpoint`) and the **verifier** was the one nobody called.
///
/// # Errors
/// [`Error::Witness`] if the signature is absent, malformed, made by another key, or over other
/// bytes. A refusal here stops the verification: an anchor that failed its own check is not a weaker
/// anchor, it is a different log's head or a forgery, and continuing with it would answer 「valid」
/// about an inclusion proof reaching a root nobody vouched for.
pub fn authenticate_anchor(anchor: &Checkpoint, key: &PublicKey) -> Result<()> {
    gx_witness::dsse::verify_checkpoint(anchor, &key.verifying())?;
    Ok(())
}

/// The answer when `--checkpoint-key` was given and the anchor failed its own check.
///
/// 44 §1.2's `7=無効` rather than an internal error: a checkpoint that does not verify under the key
/// offered for it is the **verification** failing, not the binary. The object says which half
/// refused, because 「valid: false」 alone would read as a bad receipt when the receipt was never
/// reached.
#[must_use]
pub fn anchor_refused(anchor_source: &'static str, refusal: &str) -> Outcome {
    Outcome::refused(
        serde_json::json!({
            "valid": false,
            "checks": {
                "signature": serde_json::Value::Null,
                "canonical_cid": serde_json::Value::Null,
                "inclusion": serde_json::Value::Null,
                "revocation": serde_json::Value::Null,
            },
            "anchor": anchor_source,
            "anchor_authenticated": false,
            "refusal": format!("the checkpoint did not verify under --checkpoint-key: {refusal}"),
        }),
        VERIFY_FAILED,
    )
}

/// The verification itself, as 44 §1.2's `{ "valid": bool, "checks": {...} }`.
///
/// # 🔴 What a `false` signature says about the fields below it
///
/// AC-019 makes a flipped bit an `Err(SignatureInvalid)`, so when the signature fails **nothing
/// downstream ran**. The other two checks are reported as `null` and not as `false`: `false` would
/// claim the canonical CID was compared and disagreed, and it was never compared. That is req/29
/// §4's rule at field scale, and it is what makes the tampered case of AC-057 readable — the output
/// says which check refused rather than showing three refusals for one flipped bit.
///
/// # 🔴 `anchor_authenticated` is always present (**M6H8-11 採(a)**)
///
/// Including on the failure path, and including when there is no anchor at all. A field that
/// appeared only when it was `true` would be a field a reader could miss on exactly the runs where
/// it matters; `"anchor": "none"` with `anchor_authenticated: false` says 「nothing was anchored and
/// nothing was authenticated」, which is two facts and not one.
#[must_use]
pub fn judge(
    receipt: &Receipt,
    key: &PublicKey,
    anchor: Option<&Checkpoint>,
    anchor_source: &'static str,
    anchor_authenticated: bool,
    revocation: Option<&RevocationPolicy<'_>>,
) -> Outcome {
    let verifying: VerifyingKeyRef<'_> = key.verifying();
    let verified = match revocation {
        Some(policy) => {
            gx_witness::receipt::verify_offline_consulting(receipt, &verifying, anchor, policy)
        }
        None => gx_witness::receipt::verify_offline(receipt, &verifying, anchor),
    };
    match verified {
        Ok(checks) => {
            let valid = Checks::verified(&checks);
            let json = serde_json::json!({
                "valid": valid,
                "checks": {
                    "signature": true,
                    "canonical_cid": checks.canonical_cid,
                    "inclusion": inclusion_json(checks.inclusion),
                    // **FR-M7-3**. Present on every answer, `not_consulted` included: what was not
                    // checked belongs on the wire (M6H8-11 採(a)).
                    "revocation": revocation_json(checks.revocation),
                },
                "key_id": checks.key_id,
                "anchor": anchor_source,
                "anchor_authenticated": anchor_authenticated,
                "retroaction": revocation.map(|p| p.retroaction.as_str()),
            });
            if valid {
                Outcome::ok(json)
            } else {
                Outcome::refused(json, VERIFY_FAILED)
            }
        }
        Err(e) => Outcome::refused(
            serde_json::json!({
                "valid": false,
                "checks": {
                    "signature": !matches!(e, gx_witness::Error::SignatureInvalid { .. }),
                    "canonical_cid": serde_json::Value::Null,
                    "inclusion": serde_json::Value::Null,
                    // Null rather than a word: when the signature refuses, nothing downstream ran,
                    // and 「not_consulted」 would claim a decision about a list that was never
                    // reached.
                    "revocation": serde_json::Value::Null,
                },
                "key_id": key.key_id(),
                "anchor": anchor_source,
                "anchor_authenticated": anchor_authenticated,
                "refusal": e.to_string(),
            }),
            VERIFY_FAILED,
        ),
    }
}

/// Read a `Checkpoint` out of a JSON file, for `--checkpoint`.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read, [`Error::Malformed`] if it is not a checkpoint.
pub fn read_checkpoint(path: &Path) -> Result<Checkpoint> {
    let raw = std::fs::read(path).map_err(io("read", path))?;
    serde_json::from_slice(&raw).map_err(|detail| Error::Malformed {
        what: "checkpoint",
        path: path.display().to_string(),
        detail: detail.to_string(),
    })
}
