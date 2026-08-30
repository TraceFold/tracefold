// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx receipt show` / `gx receipt verify` — 44 §1.2, and **AC-057**.
//!
//! # 🔴 `.gx/receipts/` (M6H2-1): the local store 44 names and nothing implemented
//!
//! 44 §1.2: "`show`: fetch and display a `Receipt` from the local store/gx-api" (sem: SEM-gx-cli-285). There was no local store, and
//! the three places a receipt could have been are each ruled out by something:
//!
//! * `Engine::receipt` reads an in-memory table, and `Engine::open` leaves that table empty on
//!   purpose (M5H3-5) — a second `gx` process rebuilds the draft phase and the ledger, not the rows;
//! * the journal's thirteen record kinds hold no receipt (42 §3.13), and `Committed` carries
//!   `{transformation, ledger_seq, at}`;
//! * the ledger leaf carries a **digest** of one — 42 §3.11 keeps the body out so the leaf stays
//!   small.
//!
//! So `gx receipt show` was unimplementable, and with it M6-16's staged disclosure (§47 adopted (a); sem: SEM-gx-cli-286) and
//! M6-22, which hangs on it. [`ReceiptStore`] is the store; the writer is `gx commit`, which is
//! hand 3, and the arming for that is in [`crate::consumers`]'s shape rather than left implicit.
//!
//! # 🔴 M6-16 adopted (a): `--level 1..4`, and what level 4 is (sem: SEM-gx-cli-287)
//!
//! 48 §3.1's four layers are "L1=verdict badge / L2=Receipt summary / L3=full expansion
//! (provenance chain, evidence list, fingerprints) / L4=independent verification result" (sem: SEM-gx-cli-288). This implements L1–L3 as written and L4 as
//! **the raw signatures**, which is what §47 M6-22 adopted (b) settled: "the L4 (raw signature)
//! output is `signature_for`'s consumer".
//!
//! The other half of 48's L4 — "independent verification result" (sem: SEM-gx-cli-289) — is deliberately **not** here, and the reason is that
//! it already has a subcommand. `gx receipt verify` takes a key (a verification without one is
//! arithmetic about nothing) and `show` has no key argument in 44 §1.2. A `show` that verified would
//! be a second verifier in this binary, differing from the first in what it was given. Written down
//! rather than folded: req/88 §6.0-10.
//!
//! # 🔴 `checks.inclusion` is four values, not two (§5 row 4 / H5-9) (sem: SEM-gx-cli-290)
//!
//! 44 §1.2 writes `inclusion: bool|"skipped"`. `gx_witness::InclusionCheck` has **four**, and H5-9
//! ruled that `Unanchored` must not be reported as a pass. Folding the four into the two would put
//! "the ledger claim was not checked" under the same face as "it was checked and held" (sem: SEM-gx-cli-291), which is
//! req/29 §4's "do not give skip and pass the same face". So the field carries four lowercase strings and
//! [`INCLUSION_JSON`] is the mapping back to 44's vocabulary. Raised as **M6H2-3**.
//!
//! 🔴 **v0.4 · H-09 — it is five now, and the fifth was taken out of `refuted`.** The heading above
//! is left as it was written (no-delete): what changed is the count, not the argument. `req/222`
//! measured a project with three commits answering `inclusion: "refuted"` for the two older
//! receipts, because an inclusion proof is relative to the `tree_size` it names and the default
//! anchor is the head **now**. "Refuted" is the word an operator reads as tampering, so a growing
//! log was manufacturing accusations. `unbridged` is the honest fifth word for "the anchor and the
//! proof are about different trees and nothing tied them together"; RFC 6962 §2.1.2's consistency
//! proof is the tie, and when the CLI can produce one (`--consistency`, or the local ledger on the
//! default path) the answer is `verified` — reached, not widened.

use std::path::{Path, PathBuf};

use gx_core::{Checkpoint, TransformationId};
use gx_witness::receipt::{
    Anchorage, Checks, InclusionCheck, Receipt, ReceiptPayload, RevocationCheck, RevocationPolicy,
};
use gx_witness::{PublicKey, VerifyingKeyRef};

use crate::exit::{Outcome, NOT_FOUND, VERIFY_FAILED};
use crate::{io, layout::Layout, Error, Result};

/// 🔴 The five values of `checks.inclusion`, and what 44 §1.2's two-value spelling calls each.
///
/// A table rather than a `match` in one function, because the divergence from 44 is the point and
/// `crates/gx-cli/tests/receipt_disclosure.rs` reads this to assert that the five are five. The
/// second column is empty where 44 has no word for the value — which is the whole of M6H2-3, and
/// **H-09** added the third such row rather than reusing one of the first two.
pub const INCLUSION_JSON: [(&str, &str); 5] = [
    // `NotApplicable` — a VerdictReceipt: ASM-14 says the ledger has seen nothing yet.
    ("not_applicable", "44 §1.2's `\"skipped\"`"),
    // `Verified` — the proof reached the anchor's root, directly or across a consistency proof.
    ("verified", "44 §1.2's `true`"),
    // 🔴 `Refuted` — the proof did not reach the root. A forged proof, or an anchor from another log.
    ("refuted", "no word in 44 §1.2"),
    // 🔴 `Unanchored` — a CommitReceipt verified with no anchor. **Not** a pass (H5-9).
    ("unanchored", "no word in 44 §1.2"),
    // 🔴 `Unbridged` (**H-09**) — the anchor names a different `tree_size` and nothing bridged the
    // two. Not a pass, and **not** a refutation: the fifth word exists because the fourth had been
    // doing this one's job, and `refuted` is the word an operator reads as tampering.
    ("unbridged", "no word in 44 §1.2"),
];

/// The JSON spelling of one [`InclusionCheck`].
#[must_use]
pub fn inclusion_json(check: InclusionCheck) -> &'static str {
    match check {
        InclusionCheck::NotApplicable => INCLUSION_JSON[0].0,
        InclusionCheck::Verified => INCLUSION_JSON[1].0,
        InclusionCheck::Refuted => INCLUSION_JSON[2].0,
        InclusionCheck::Unanchored => INCLUSION_JSON[3].0,
        InclusionCheck::Unbridged => INCLUSION_JSON[4].0,
    }
}

/// 🔴 **M6H4-7** — which of a transformation's receipts a file holds.
///
/// > migrate `.gx/receipts/` to `<TID>.<kind>.json` (kind ∈ verdict/ruling/commit) -- update
/// > both hand 3's writer and hand 2's reader; the migration needs no backward compatibility (not
/// > yet distributed) (sem: SEM-gx-cli-292)
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
/// preference: "who allowed this" erasing "what was decided" is the loss INV-S6 exists to (sem: SEM-gx-cli-293)
/// prevent, and it happened in the directory req/56 §2 files as `Nature::Source` — "is lost".
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
/// The commit receipt is first because it is the one 44 §1.2's own example is about ("fetch and
/// display a `Receipt`" (sem: SEM-gx-cli-294) after a commit) and because it is the only one carrying an `inclusion_proof` — the
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
    /// The id is an argument and is not read out of the receipt, which is Rule 1 in the signature: the (sem: SEM-gx-cli-295)
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
        // 🔴 **R9 / `req/236` M-03** — a document this archive already holds is not written again.
        //
        // R8 registered [`ArchiveSink`] so that the **engine** files the commit receipt inside
        // T-11's critical section, and left the CLI's own `put` where it was — so `gx commit` wrote
        // the same path twice. `req/236` M-03 read it out of `strace`: two `openat`+`fsync`+`rename`
        // pairs plus two directory fsyncs for one commit, and `req/235` §7-4's "the number of writes
        // has not gone up" was false. That sentence is corrected in the R9 report rather than
        // edited out of R8's (no-delete).
        //
        // Removing one of the two `put` calls would have been the obvious repair and is the wrong
        // one: the CLI's call is what files a receipt on a road where **no sink is registered**
        // (every engine built by a test, and the pre-R8 shape this crate still supports), and the
        // engine's call is what makes `req/38` §154 true. What is actually wrong is writing bytes
        // that are already on the disk, so that is what stops — the same rule
        // `BlobStore::put` takes for a body under its content address (`req/236` H-01).
        //
        // The comparison is over the **bytes**, so a re-issued receipt (a new `issued_at`, `req/236`
        // L-03) is a different document and is written.
        if std::fs::read(&path).is_ok_and(|held| held == body) {
            return Ok(path);
        }
        // 🔴 **R8 / `req/234` H-01** — tmp + fsync + rename + directory fsync, the five steps of
        // `gx_adapter_fs::apply`'s own citation (LWN 457667).
        //
        // A plain `write(2)` was honest while this file was written *after* the commit returned: a
        // crash mid-write left a half receipt beside a commit that was already finished, and the
        // next `gx receipt show` said so. Since R8 this write happens **inside** T-11's critical
        // section and the `Committed` record waits on it, so a torn file here would be a detector
        // failing open on the very event it was moved to survive — the same shape `head.json`'s
        // write already avoids.
        let temp = path.with_extension("json.tmp");
        let write = || -> std::io::Result<()> {
            {
                use std::io::Write;
                let mut file = std::fs::File::create(&temp)?;
                file.write_all(&body)?;
                file.sync_all()?;
            }
            std::fs::rename(&temp, &path)?;
            #[cfg(unix)]
            {
                std::fs::File::open(&self.dir)?.sync_all()?;
            }
            Ok(())
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_file(&temp);
            return Err(io("write", &path)(e));
        }
        Ok(path)
    }

    /// 🔴 **R8 / `req/234` H-01** — how many **commit** receipts this store holds.
    ///
    /// The number `gx repair` subtracts from `ledger_leaves`. `req/234` H-01's closing sentence is
    /// that the difference "is the subtraction of two numbers gx already holds, and there is no
    /// verb that computes it"; this is one of the two.
    ///
    /// Counted by walking the directory rather than by asking about a list of ids, because the
    /// question is about the *store* and a caller who could only ask "is this one here" would have
    /// to know every id first — which is exactly the read a project with a trimmed journal cannot
    /// do.
    #[must_use]
    pub fn commit_count(&self) -> usize {
        let suffix = format!(".{}.json", StoredKind::Commit.tag());
        std::fs::read_dir(&self.dir).map_or(0, |entries| {
            entries
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_name().to_string_lossy().ends_with(&suffix))
                .count()
        })
    }

    /// Read one back, if it is there.
    ///
    /// `Ok(None)` for "no such receipt" and `Err` for "there is a file and it is not a receipt" (sem: SEM-gx-cli-296)
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
    /// commit receipt to a verdict receipt would answer "here is the receipt" about a document with
    /// no `inclusion_proof`, and a reader who could not tell the two apart would read "this change
    /// was applied" out of "this change was judged" (sem: SEM-gx-cli-297).
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

/// 🔴 **R8 / `req/234` H-01** — `.gx/receipts/` as the engine's commit-receipt sink.
///
/// The engine holds a `TransformationId` and a `Receipt` and knows nothing about req/56 §2's
/// directory or M6H4-7's `<TID>.<kind>.json`; this is the one line that joins the two, and it is in
/// the crate that owns the layout for the reason `Engine::open`'s note gives about the head store.
///
/// [`gx_engine::pipeline::CommitReceiptSink`] carries what registering one changes: the archive
/// write moves **inside** T-11's critical section and in front of the `Committed` record, so a
/// commit whose receipt will not file is a commit that did not happen (`req/38` §154).
#[derive(Debug, Clone)]
pub struct ArchiveSink(ReceiptStore);

impl ArchiveSink {
    /// The sink over an opened layout's `.gx/receipts/`.
    #[must_use]
    pub fn in_layout(layout: &Layout) -> Self {
        Self(ReceiptStore::in_layout(layout))
    }
}

impl gx_engine::pipeline::CommitReceiptSink for ArchiveSink {
    fn store(&self, id: &TransformationId, receipt: &Receipt) -> std::result::Result<(), String> {
        self.0
            .put(id, StoredKind::Commit, receipt)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// 🔴 **R9 / `req/236` H-03** — the `key_id` inside the commit receipt this project already
    /// holds for `id`.
    ///
    /// Read out of the **payload** rather than off the DSSE signature: the payload's field is what
    /// 43 §7-3b's rebuild has to reproduce, and a document whose two disagree is one
    /// `verify_offline` refuses anyway (42 §3.10's fourth condition).
    ///
    /// Every failure is `None` — no file, an unreadable file, a payload that will not decode. The
    /// caller's fallback (the key it was handed) is what every recovery did before R9, so a
    /// degraded read here is exactly the old behaviour and not a new refusal.
    fn filed_key_id(&self, id: &TransformationId) -> Option<gx_core::KeyId> {
        self.0
            .get(id, StoredKind::Commit)
            .ok()
            .flatten()
            .and_then(|receipt| receipt.payload().ok())
            .map(|payload| payload.key_id)
    }

    /// 🔴 **R13 / `req/244` H-03** — the whole commit receipt this project holds for `id`.
    ///
    /// [`Self::filed_key_id`] reads one field out of this document; 43 §7-3b needs the rest of it.
    /// The audit measured why: a `gx wrap` commit killed between `ledger.append` and the
    /// `Committed` record leaves a leaf the journal does not witness, and the only way the old
    /// recovery could write that record was to re-apply the delta and rebuild the payload — which a
    /// `gx repair` with no MCP server cannot do. The document is right here, it is signed, and its
    /// payload digests to the leaf the ledger already witnessed. The engine compares the two and
    /// writes the record from the agreement; this line is the read.
    ///
    /// Every failure is `None` — no file, an unreadable file, a payload that will not decode — and
    /// `None` is the pre-R13 behaviour exactly: the recovery falls through to the road that
    /// re-applies, and refuses without a terminal record where it cannot.
    fn filed_receipt(&self, id: &TransformationId) -> Option<gx_witness::Receipt> {
        self.0.get(id, StoredKind::Commit).ok().flatten()
    }
}

/// Decode a receipt from the JSON face 44 §2.2 fixes ("`payload` is base64"; sem: SEM-gx-cli-298).
fn read_receipt(raw: &[u8], whence: &Path) -> Result<Receipt> {
    serde_json::from_slice(raw).map_err(|detail| Error::Malformed {
        what: "receipt",
        path: whence.display().to_string(),
        detail: detail.to_string(),
    })
}

// ---------------------------------------------------------------------------
// `gx receipt show` — M6-16 adopted (a) (sem: SEM-gx-cli-299)
// ---------------------------------------------------------------------------

/// The highest disclosure level.
pub const MAX_LEVEL: u8 = 4;

/// 🔴 `gx receipt show <TID> --level 1..4` (44 §1.2, M6-16 adopted (a); sem: SEM-gx-cli-300).
///
/// `--json` "is always the full amount" (M6-16 adopted (a)'s own clause: "machines do not need
/// staged disclosure; the rationale for staged disclosure is human cognitive load") (sem: SEM-gx-cli-301), which the caller expresses by passing `level = MAX_LEVEL`.
///
/// # Errors
/// [`Error::Usage`] for a level outside 1..=4. Everything else is an [`Outcome`]: a receipt that is
/// not there is 44 §1.2's `6=not-found` **with an object on stdout** (sem: SEM-gx-cli-302), because a script that asked for a
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
                // 🔴 **M6H4-7**: which names were looked for. A "not found" (sem: SEM-gx-cli-303) that does not say what
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
        // the two fields together are the answer to "which document am I reading" (sem: SEM-gx-cli-304).
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

    // L1 — 48 §3.1's "verdict badge" (sem: SEM-gx-cli-305). What a human needs to decide whether to read on.
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

    // L2 — "Receipt summary" (sem: SEM-gx-cli-306).
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
    // 🔴 Nanoseconds, where 44 §0 asks for RFC 3339 ("datetimes are RFC 3339 … conversion from the
    // internal `Timestamp` (nanosecond epoch) is the API layer's responsibility" (sem: SEM-gx-cli-307)). This workspace has no date library and adding one is a dependency
    // decision for the hand that owns the surface where `at` fields are everywhere (44 §2's fourteen
    // endpoints, hands 5 and 6). Emitting a wrong RFC 3339 string would be worse than emitting a
    // right integer, and a home-made civil-date conversion is a date library written badly. Raised
    // as **M6H2-4**; the field name says which unit it is in so nothing reads it as seconds.
    map.insert("issued_at_unix_nanos".into(), receipt.issued_at.0.into());
    if level == 2 {
        return Ok(out);
    }

    // L3 — "full expansion" (sem: SEM-gx-cli-308). The eleven fields of 42 §3.10, through the type's own `Serialize` rather than
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

    // 🔴 L4 — the raw signatures. **M6-22 adopted (b)** (sem: SEM-gx-cli-309): this is `Receipt::signature_for`'s consumer, and
    // the call is what discharges the accessor that M5FIX-3 left as gx-witness's one survivor.
    //
    // Asked for **by the id the payload declares**, not by iterating `signatures`. That is the
    // difference the accessor exists to make: 42 §3.10 requires the payload's `key_id` and the
    // envelope's `keyid` to agree, so "the signature this receipt says signed it" (sem: SEM-gx-cli-310) is a lookup and
    // not a position. An envelope carrying somebody else's signature and none of its own answers
    // `null` here, and that is the true answer rather than "the first one" (sem: SEM-gx-cli-311).
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
/// "`{ "key_id": KEY_ID, "public_key": <base64> }`" (sem: SEM-gx-cli-312) and that object is what `--key` accepts. It also
/// accepts a `gx` key file (the owner verifying their own receipts), because refusing to read a key
/// the same binary wrote would be a gratuitous second step. When `--key` is omitted the key is
/// looked up in the local key store under the id **the receipt declares**, which is a convenience
/// for the owner and is never available to the third party AC-057 is about.
///
/// # `--offline`, `--checkpoint` and 🔴 `--checkpoint-key` (**M6H8-11**)
///
/// 44 §1.2: "with `--offline`, verify only the mathematical consistency of `inclusion_proof`
/// against the known `Checkpoint` given via `--checkpoint`, and make no network access" (sem: SEM-gx-cli-313). The checkpoint's **own** signature is not
/// checked by `gx_witness::verify_offline` — 44 calls the checkpoint "known", i.e. already believed,
/// and a verification that checked it silently would make one `Ok` mean two things.
///
/// Hand 8 asked the question §49 reserved for it — "does it accept a fake checkpoint?" (sem: SEM-gx-cli-314) — and the answer was
/// **yes**: `gx_witness::dsse::verify_checkpoint` exists and had **zero callers in shipping code**,
/// so a checkpoint with an empty, broken or foreign signature was accepted as an anchor and the
/// output said `valid: true` with nothing recording what had not been checked. 44's own text is
/// satisfied by that; AC-057's third party is not, because they receive the receipt and the
/// checkpoint **from the same hand** and "known" names nothing they hold (45 TH-6's split view; sem: SEM-gx-cli-315).
///
/// req/38 §55 rules **M6H8-11 adopted (b), with (a) first** (sem: SEM-gx-cli-316), and both halves are here:
///
/// * **(a)** every answer carries `anchor_authenticated`, and it is `false` unless something
///   authenticated the anchor. The field costs one line and changes no semantics; what it changes is
///   that "what was not checked" is on the wire rather than in a document (sem: SEM-gx-cli-317). A flag alone would leave
///   whoever forgets it receiving the weaker verification silently, which is today's state.
/// * **(b)** `--checkpoint-key <FILE>` verifies the checkpoint's DSSE signature (45 ASM-45-1 allows a
///   different key from the receipt's, which is why it is a second flag and not the same `--key`) and
///   only then reports `anchor_authenticated: true`.
///
/// Always verifying was **not** adopted (§55 rejects (c)): it collides with 44's "known" (sem: SEM-gx-cli-318) and would
/// shut out a third party who holds a checkpoint but not the log's key.
///
/// Without `--offline`, 44 asks for "a ledger inquiry" (sem: SEM-gx-cli-319). v0.1 has no ledger client — `gx-api` serves nothing
/// until hands 5 and 6 — so the enquiry is against the **local** ledger, and `anchor_source` in the
/// output says which of the two happened. A command that said "verified" (sem: SEM-gx-cli-320) without saying against
/// what would be the shape req/29 §4 is about.
///
/// # Errors
/// [`Error::Usage`] for input that is not a receipt, [`Error::Io`] for a file that cannot be read,
/// [`Error::Witness`] for a receipt whose payload will not decode.
pub fn verify(
    source: &Source<'_>,
    key: &PublicKey,
    anchorage: Option<&Anchorage<'_>>,
    anchor_source: &'static str,
    anchor_authenticated: bool,
    revocation: Option<&RevocationPolicy<'_>>,
) -> Result<Outcome> {
    let receipt = read(source)?;
    Ok(judge(
        &receipt,
        key,
        anchorage,
        anchor_source,
        anchor_authenticated,
        revocation,
    ))
}

/// The receipt a [`Source`] holds, read once.
///
/// 🔴 **H-09** made the caller need this before it can choose an anchor: a bridge is between the
/// `tree_size` the receipt names and the one the head names, so `main.rs` has to know the first
/// number before it opens the ledger for the second. Reading the file twice would have been the
/// cheaper edit and the wrong one — two reads of a path an operator can replace between them are
/// two different receipts, and the answer would be about whichever one the second read got.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read, [`Error::Usage`] if the bytes are not a receipt.
pub fn read(source: &Source<'_>) -> Result<Receipt> {
    let (raw, whence) = match source {
        Source::File(path) => (
            std::fs::read(path).map_err(io("read", path))?,
            path.display().to_string(),
        ),
        Source::Stdin(bytes) => ((*bytes).to_vec(), "<stdin>".to_string()),
    };
    read_receipt(&raw, Path::new(&whence))
}

/// 🔴 **H-09** — the `tree_size` a `CommitReceipt`'s inclusion proof is relative to.
///
/// `None` for a `VerdictReceipt` (ASM-14: no proof) and for a payload that will not decode — in
/// both cases there is no bridge to build, and the verification itself answers the second one.
#[must_use]
pub fn proof_tree_size(receipt: &Receipt) -> Option<u64> {
    receipt
        .payload()
        .ok()?
        .inclusion_proof
        .as_ref()
        .map(|proof| proof.tree_size)
}

/// Read a `ConsistencyProof` out of a JSON file, for 🔴 `--consistency` (**H-09**).
///
/// The bytes `gx log consistency --from <SIZE> --to <SIZE>` prints, unchanged. One document, two
/// commands: a second spelling here would be a second wire format for the same proof, and the day
/// they drift the verifier is the one that looks wrong.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read, [`Error::Malformed`] if it is not a consistency proof.
pub fn read_consistency(path: &Path) -> Result<gx_log::proof::ConsistencyProof> {
    let raw = std::fs::read(path).map_err(io("read", path))?;
    serde_json::from_slice(&raw).map_err(|detail| Error::Malformed {
        what: "consistency proof",
        path: path.display().to_string(),
        detail: detail.to_string(),
    })
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
    // ("consulting the revocation list is optional on the verifier's side" (sem: SEM-gx-cli-321)) and this word is what keeps a declined option from
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

/// 🔴 `--checkpoint-key <FILE>`: check the anchor's own DSSE signature (**M6H8-11 adopted (b)**; sem: SEM-gx-cli-322).
///
/// The one caller of `gx_witness::dsse::verify_checkpoint` in shipping code. Before this batch that
/// function had none — the mirror image of M6-24, which found `sign_checkpoint` with no producer;
/// here the producer exists (`gx log checkpoint`) and the **verifier** was the one nobody called.
///
/// # Errors
/// [`Error::Witness`] if the signature is absent, malformed, made by another key, or over other
/// bytes. A refusal here stops the verification: an anchor that failed its own check is not a weaker
/// anchor, it is a different log's head or a forgery, and continuing with it would answer "valid" (sem: SEM-gx-cli-323)
/// about an inclusion proof reaching a root nobody vouched for.
pub fn authenticate_anchor(anchor: &Checkpoint, key: &PublicKey) -> Result<()> {
    gx_witness::dsse::verify_checkpoint(anchor, &key.verifying())?;
    Ok(())
}

/// The answer when `--checkpoint-key` was given and the anchor failed its own check.
///
/// 44 §1.2's `7=invalid` (sem: SEM-gx-cli-324) rather than an internal error: a checkpoint that does not verify under the key
/// offered for it is the **verification** failing, not the binary. The object says which half
/// refused, because "valid: false" (sem: SEM-gx-cli-325) alone would read as a bad receipt when the receipt was never
/// reached.
///
/// 🔴 The `refusal` string is prefixed with [`gx_witness::dsse::CHECKPOINT_REFUSAL_PREFIX`] rather
/// than a CLI-local sentence — `web_verify/src/verdict.js::CHECKPOINT_REFUTED` matches on that exact
/// spelling, and a paraphrase here previously fell through to the classifier's default `hold` instead
/// of `deny` (B1 / DIV-901-1, `req/914` §7, `req/874` R3: a checkpoint refusing its own signature
/// check is one of R3's four `deny` triggers).
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
            "refusal": format!("{}: {refusal}", gx_witness::dsse::CHECKPOINT_REFUSAL_PREFIX),
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
/// # 🔴 `anchor_authenticated` is always present (**M6H8-11 adopted (a)**; sem: SEM-gx-cli-326)
///
/// Including on the failure path, and including when there is no anchor at all. A field that
/// appeared only when it was `true` would be a field a reader could miss on exactly the runs where
/// it matters; `"anchor": "none"` with `anchor_authenticated: false` says "nothing was anchored and
/// nothing was authenticated" (sem: SEM-gx-cli-327), which is two facts and not one.
#[must_use]
pub fn judge(
    receipt: &Receipt,
    key: &PublicKey,
    anchorage: Option<&Anchorage<'_>>,
    anchor_source: &'static str,
    anchor_authenticated: bool,
    revocation: Option<&RevocationPolicy<'_>>,
) -> Outcome {
    let verifying: VerifyingKeyRef<'_> = key.verifying();
    // 🔴 **H-09**: one entry point for both roads. `verify_offline_against` is
    // `verify_offline`/`verify_offline_consulting` with the bridge carried, so the revocation
    // branch that used to pick between two functions now picks between two arguments — and the
    // inclusion half is decided in exactly one place either way.
    let verified =
        gx_witness::receipt::verify_offline_against(receipt, &verifying, anchorage, revocation);
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
                    // checked belongs on the wire (M6H8-11 adopted (a); sem: SEM-gx-cli-328).
                    "revocation": revocation_json(checks.revocation),
                },
                "key_id": checks.key_id,
                "anchor": anchor_source,
                "anchor_authenticated": anchor_authenticated,
                "retroaction": revocation.map(|p| p.retroaction.as_str()),
                // 🔴 **R10 / audit 9 L-03** — the timestamp on this document is **not** covered by
                // the signature this answer just checked, and the answer says so.
                //
                // **E-M2-6** (`req/38` §8) took `issued_at` out of the signed core deliberately —
                // it is what makes two receipts for one commit byte-identical, which is what makes
                // 43 ASM-43-1's idempotence observable — and `gx-witness`'s module header, the
                // `Receipt` doc comment and `keys.rs`'s revocation arm all record the choice.
                // Audit 9 L-03 measured what was missing: the fact is written in four source files
                // and in **no answer**. A verifier reading `{"valid": true}` beside
                // `issued_at_unix_nanos` had nothing on the wire telling them which of the two the
                // key vouches for.
                //
                // Always present and always `false`, for `anchor_authenticated`'s reason: a field
                // that appeared only on the runs where it mattered is a field a reader misses. The
                // ruling is not reversed here — reversing it would change every receipt's payload
                // digest and is a DR, not a repair lane's to take.
                "issued_at_signed": false,
                "issued_at_unix_nanos": receipt.issued_at.0,
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
                    // and "not_consulted" (sem: SEM-gx-cli-329) would claim a decision about a list that was never
                    // reached.
                    "revocation": serde_json::Value::Null,
                },
                "key_id": key.key_id(),
                "anchor": anchor_source,
                "anchor_authenticated": anchor_authenticated,
                // 🔴 **R10 / audit 9 L-03** — on the refusal path too. E-M2-6's exclusion is a
                // property of the document, not of the outcome.
                "issued_at_signed": false,
                "issued_at_unix_nanos": receipt.issued_at.0,
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

/// 🔴 **P-1b** (`req/544` §3 R-3c, ruled in `req/38` §313) — `gx receipt coverage <FILE>
/// [--face <FILE>]`: the four questions, and which of them this document answers.
///
/// # What makes this a reading and not a report
///
/// The table is [`gx_witness::ReceiptCoverage::of`]'s projection of the fifteen members the receipt
/// already carries. Nothing was added to the payload to make it answerable, nothing is stored
/// beside the receipt, and there is no input to this function except the document itself — so the
/// state "the declaration says one thing and the receipt says another" is not refused here, it is
/// unconstructible.
///
/// # 🔴 The two levels, side by side, and why the pair is not a contradiction
///
/// With `--face`, the face's own claim is printed **beside** the answer, in the vocabulary
/// [`crate::face`] keeps separate on purpose. A face saying `can-measure` about the read question
/// and a receipt answering `unknown` about the same question is a **legitimate** pair: the route
/// can observe reads, and on this run there was nothing to observe. `req/544` AC-11 asks for that
/// combination to come out as an answer rather than a refusal, and it does — the exit status is
/// `0` and the pair is printed.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read, [`Error::Usage`] if the bytes are not a receipt,
/// [`Error::Witness`] if the envelope's payload is not a decodable receipt payload — which is
/// itself a fact about the document (`docs/LIMITS.md` names the specimen it is true of) and not a
/// coverage question.
pub fn coverage(file: &str, face: Option<&Path>) -> Result<Outcome> {
    let receipt = read(&Source::File(Path::new(file)))?;
    let payload = receipt.payload()?;
    let table = gx_witness::ReceiptCoverage::of(&payload);
    let mut answer = crate::face::receipt_coverage_json(&table);
    let claim = match face {
        Some(path) => {
            let raw = std::fs::read(path).map_err(io("read", path))?;
            let document: serde_json::Value =
                serde_json::from_slice(&raw).map_err(|detail| Error::Malformed {
                    what: "face declaration",
                    path: path.display().to_string(),
                    detail: detail.to_string(),
                })?;
            Some(document)
        }
        None => None,
    };
    if let serde_json::Value::Object(map) = &mut answer {
        map.insert("gx".to_string(), serde_json::json!("receipt coverage"));
        map.insert(
            "transformation".to_string(),
            serde_json::json!(payload.transformation.0.to_text()),
        );
        map.insert("face_claim".to_string(), serde_json::json!(claim));
        // 🔴 The sentence that stops the two tables being read as one. Printed whether or not a
        // face was offered, because a reader of the receipt table alone is exactly the reader who
        // might take an answer for a capability.
        map.insert(
            "levels_are_not_comparable".to_string(),
            serde_json::json!(
                "the face table says what this route could observe; this table says what this one \
                 document answers. A face that can measure a question and a receipt that answers \
                 `unknown` to it are both right — the run had nothing to attest."
            ),
        );
    }
    Ok(Outcome::ok(answer))
}
