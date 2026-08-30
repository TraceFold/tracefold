#![forbid(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `verify_receipt_offline` — the one function this crate exists to expose (`req/132` §2 item 2).
//!
//! # Rule 1, carried into WASM (sem: SEM-sdk-wasm-verify-004)
//!
//! This crate constructs no `Verdict`, mints no canonical CID, and writes no `Lifecycle` — the same
//! three absences `crates/gx-api` and `crates/gx-cli` carry (see their crate-root docs). All it does
//! is call [`gx_witness::receipt::verify_offline`], the one function the engine already offers a
//! stranger with a receipt, a public key, and (optionally) a checkpoint. A second verifier written
//! here, even a small one, would be the drift `E-M2-12` moved the `Proof` family down to gx-core to
//! avoid — this crate is a **projection** of that function through `wasm-bindgen`, not a
//! reimplementation of it.
//!
//! # The input shapes are 44's own JSON, not an invented DTO
//!
//! * `receipt_json`: exactly what `GET /receipts/{tid}` or `gx receipt show --level 4` hands back —
//!   `Receipt`'s `Deserialize` (42 §3.10's `DsseEnvelope` + `issued_at`).
//! * `public_key_base64`: 44 §1.2's `gx key gen` output field, `public_key` (base64, ed25519,
//!   [`gx_witness::PublicKey::LENGTH`] bytes decoded).
//! * `checkpoint_json`: `GET /ledger/checkpoint`'s body (42 §3.11's `Checkpoint`), or absent for a
//!   `VerdictReceipt` / an unanchored check.
//!
//! # The output vocabulary is the CLI's, not a third spelling
//!
//! `gx_witness::receipt::Checks::inclusion` is a four-value enum and 44 §1.2's prose only names two
//! (`bool|"skipped"`) — the CLI (`crates/gx-cli/src/receipt.rs::INCLUSION_JSON`) already resolved
//! that gap by spelling all four (`not_applicable`/`verified`/`refuted`/`unanchored`, H5-9: a skip and
//! a pass must not share a face). This module reuses those four words rather than inventing a fifth
//! spelling of the same fact.
//!
//! 🔴 **v0.4 · H-09 — the enum is five values now** (`unbridged`: the anchor names a `tree_size` the
//! receipt's proof does not, and nothing bridged the two). The sentence above is left as it was
//! written; what it settles is the *rule* (one vocabulary, the CLI's), and the rule is what carried
//! the fifth word across without a decision. Not a pass — `Checks::verified` excludes it — and not a
//! refutation either, which is the whole of why it is its own word.

use gx_core::Checkpoint;
use gx_witness::receipt::{verify_offline, Checks, InclusionCheck, Receipt};
use gx_witness::PublicKey;
use wasm_bindgen::prelude::wasm_bindgen;

/// The five words `crates/gx-cli/src/receipt.rs::INCLUSION_JSON` already gives
/// [`gx_witness::receipt::InclusionCheck`]. Kept in one place per copy (**not** imported across the
/// crate boundary: gx-cli is a binary crate and this one cannot see it) and asserted equal to the
/// CLI's own table by `tests/inclusion_vocabulary.rs`.
fn inclusion_word(check: InclusionCheck) -> &'static str {
    match check {
        InclusionCheck::NotApplicable => "not_applicable",
        InclusionCheck::Verified => "verified",
        InclusionCheck::Refuted => "refuted",
        InclusionCheck::Unanchored => "unanchored",
        // 🔴 **H-09**.
        InclusionCheck::Unbridged => "unbridged",
    }
}

/// One verification, offline, with no ledger and no clock (AC-018, AC-070 — the same claim
/// `gx_witness::receipt::verify_offline` makes, projected through a string-in/string-out boundary
/// because `wasm-bindgen` needs no `js-sys`/`serde_wasm_bindgen` dependency for that shape).
///
/// # What the returned JSON says
///
/// `{"valid": bool, "checks": {"signature": bool, "canonical_cid": bool, "inclusion": <one of the
/// four words above>, "key_id": string} | null, "anchor_authenticated": bool, "error": string |
/// null}`. `checks` is `null` exactly
/// when `error` is not — a signature failure means nothing downstream ran (AC-019), so reporting
/// `canonical_cid`/`inclusion` at all would claim a comparison that never happened (req/29 §4's
/// "a skip and a pass must not share a face" (sem: SEM-sdk-wasm-verify-005), applied to the WASM
/// boundary rather than dropped at it).
///
/// # 🔴 `anchor_authenticated` — **E-SDK-10** (`req/38` §285, `req/503`)
///
/// [`gx_witness::receipt::verify_offline`] reads a checkpoint's `tree_size` and `root_hash` and
/// **never asks who said them** — its own doc comment says so, and says why (the log's key may
/// differ from the receipt's, 45 ASM-45-1, and one `Ok` meaning two things is how a verifier comes
/// to be trusted for something it never did). `crates/gx-cli` answered that with **M6H8-11**: an
/// opt-in `--checkpoint-key <FILE>` that runs [`gx_witness::dsse::verify_checkpoint`], and a
/// `anchor_authenticated` field on **every** answer saying whether anything did. The SDK carried
/// the same hole with neither half, so a forger holding the receipt and the checkpoint — the two
/// files a third party receives from one hand — could hand over a head signed by nobody, or one
/// belonging to another log, and read `valid: true`.
///
/// The two halves are ported here unchanged in meaning:
///
/// * **(a)** `anchor_authenticated` is on every answer, including the refusals and including the
///   runs that saw no anchor at all. A field that appeared only when it was `true` is a field a
///   reader misses on exactly the runs where it matters.
/// * **(b)** `checkpoint_key_id` + `checkpoint_public_key_base64` (both, or neither) authenticate
///   the anchor. Absent, the anchor is taken on trust — 44's own word for a checkpoint is
///   "known", and always verifying would shut out a third party who holds a head but not the
///   log's key (§55 rejects that option for the CLI, and it is not reopened here).
///
/// **This is not "no anchor"**: `anchor_authenticated: false` with `checks.inclusion` reading
/// `unanchored`/`not_applicable` says nothing was anchored; with `verified` it says the arithmetic
/// held against a head nobody vouched for. No new word is minted for the distinction — the two
/// fields already carry it, and `req/503` §2-2 asks that a new API name be earned rather than
/// added.
///
/// # Never throws
///
/// Every failure — malformed JSON, a public key of the wrong length, a bad signature, a checkpoint
/// that fails its own check — becomes `{"valid": false, "checks": null, "anchor_authenticated":
/// false, "error": "<detail>"}` rather than a JS exception. A thin
/// projection surfaces the engine's own refusal vocabulary; it does not add a second control-flow
/// mechanism (exceptions) beside the one the engine already returns (`Result`).
///
/// 🔴 That promise is about *this* function's failures. It says nothing about a caller who ignores
/// the four parameter types and passes something other than a string — `wasm-bindgen`'s generated
/// glue reaches into WASM linear memory for those and fails as `RuntimeError: memory access out of
/// bounds`. **E-SDK-8** closes that at the TypeScript window (`sdk/typescript/src/verify.ts`),
/// where the caller actually is; nothing in Rust can see a JS value that never became a `&str`.
#[wasm_bindgen]
#[must_use]
pub fn verify_receipt_offline(
    receipt_json: &str,
    key_id: &str,
    public_key_base64: &str,
    checkpoint_json: Option<String>,
    checkpoint_key_id: Option<String>,
    checkpoint_public_key_base64: Option<String>,
) -> String {
    render(run(
        receipt_json,
        key_id,
        public_key_base64,
        checkpoint_json.as_deref(),
        checkpoint_key_id.as_deref(),
        checkpoint_public_key_base64.as_deref(),
    ))
}

/// The verification itself, kept apart from [`verify_receipt_offline`] so `tests/` can call it
/// directly on the host target (no `wasm_bindgen`-generated string in `Ok`, one `Result` to match on).
///
/// # Errors
/// A `String` naming which of the five inputs (receipt JSON, key bytes, checkpoint JSON, the
/// checkpoint's own key, the verification itself) refused, and why.
///
/// The `bool` beside the result is `anchor_authenticated` and is returned on **both** arms rather
/// than folded into `Ok`: a genuine head paired with a tampered receipt authenticated its anchor
/// and then refused the document, and an answer that dropped the first fact because of the second
/// would be the shape this whole field exists to prevent (`crates/gx-cli/src/receipt.rs::judge`
/// puts it on both arms for the same reason).
fn run(
    receipt_json: &str,
    key_id: &str,
    public_key_base64: &str,
    checkpoint_json: Option<&str>,
    checkpoint_key_id: Option<&str>,
    checkpoint_public_key_base64: Option<&str>,
) -> (bool, Result<Checks, String>) {
    let mut anchor_authenticated = false;
    let result = (|| {
        let receipt: Receipt =
            serde_json::from_str(receipt_json).map_err(|e| format!("receipt: {e}"))?;
        let key_bytes = gx_core::b64::decode(public_key_base64)
            .map_err(|e| format!("public_key_base64: {e}"))?;
        let key = PublicKey::from_bytes(key_id.to_string(), &key_bytes)
            .map_err(|e| format!("public_key_base64: {e}"))?;
        let anchor: Option<Checkpoint> = match checkpoint_json {
            Some(s) => Some(serde_json::from_str(s).map_err(|e| format!("checkpoint_json: {e}"))?),
            None => None,
        };
        // 🔴 **E-SDK-10 / M6H8-11 adopted (b)**, before the verification and not after it: a head
        // that failed its own check is not a weaker anchor, it is a different log's head or a
        // forgery, and continuing with it would answer about an inclusion proof reaching a root
        // nobody vouched for (`crates/gx-cli/src/receipt.rs::authenticate_anchor`, verbatim).
        anchor_authenticated = authenticate(
            anchor.as_ref(),
            checkpoint_key_id,
            checkpoint_public_key_base64,
        )?;
        verify_offline(&receipt, &key.verifying(), anchor.as_ref()).map_err(|e| e.to_string())
    })();
    (anchor_authenticated, result)
}

/// The anchor's own DSSE signature, checked only when a key for it is offered (**M6H8-11 adopted
/// (b)**).
///
/// The two key parameters move together. One without the other is a usage error rather than a
/// half-check, on the same argument `crates/gx-cli/src/main.rs` makes for `--checkpoint-key`
/// without `--checkpoint`: authenticating an anchor that does not exist, or with half a key, is
/// not a weaker check, it is no check, and answering `false` would spell it the same as a caller
/// who deliberately declined.
///
/// # Errors
/// A `String` prefixed `checkpoint_key:` — the prefix is the field name, so a caller reading the
/// refusal learns which of the two documents refused without parsing prose.
fn authenticate(
    anchor: Option<&Checkpoint>,
    checkpoint_key_id: Option<&str>,
    checkpoint_public_key_base64: Option<&str>,
) -> Result<bool, String> {
    match (checkpoint_key_id, checkpoint_public_key_base64, anchor) {
        (None, None, _) => Ok(false),
        (Some(id), Some(b64), Some(head)) => {
            let bytes = gx_core::b64::decode(b64)
                .map_err(|e| format!("checkpoint_key: checkpoint_public_key_base64: {e}"))?;
            let key = PublicKey::from_bytes(id.to_string(), &bytes)
                .map_err(|e| format!("checkpoint_key: checkpoint_public_key_base64: {e}"))?;
            gx_witness::dsse::verify_checkpoint(head, &key.verifying()).map_err(|e| {
                format!("{}: {e}", gx_witness::dsse::CHECKPOINT_REFUSAL_PREFIX)
            })?;
            Ok(true)
        }
        (Some(_), Some(_), None) => Err("checkpoint_key: a checkpoint key names the key a \
             checkpoint was signed with, and no checkpoint was given. Authenticating an anchor \
             that does not exist is not a weaker check, it is no check"
            .to_string()),
        _ => Err(
            "checkpoint_key: checkpoint_key_id and checkpoint_public_key_base64 are one \
             argument in two halves — pass both to authenticate the anchor, or neither to take it \
             on trust (the answer says which happened)"
                .to_string(),
        ),
    }
}

/// [`run`]'s result, as the JSON text [`verify_receipt_offline`] returns.
fn render((anchor_authenticated, result): (bool, Result<Checks, String>)) -> String {
    let body = match result {
        Ok(checks) => serde_json::json!({
            "valid": checks.verified(),
            "checks": {
                "signature": true, // reaching here means `DsseEnvelope::verify` did not refuse (AC-019)
                "canonical_cid": checks.canonical_cid,
                "inclusion": inclusion_word(checks.inclusion),
                "key_id": checks.key_id,
            },
            // 🔴 **M6H8-11 adopted (a)**: always present, `false` included.
            "anchor_authenticated": anchor_authenticated,
            "error": null,
        }),
        Err(detail) => serde_json::json!({
            "valid": false,
            "checks": null,
            "anchor_authenticated": anchor_authenticated,
            "error": detail,
        }),
    };
    // `serde_json::Value` from `json!` over already-valid UTF-8 strings and primitives cannot fail
    // to serialise; the fallback exists so this function keeps its "never throws" promise even if
    // that ever stops being true, rather than trading a Rust `unwrap` panic for a JS exception.
    serde_json::to_string(&body).unwrap_or_else(|e| {
        format!(
            r#"{{"valid":false,"checks":null,"anchor_authenticated":false,"error":"internal: result did not serialise: {e}"}}"#
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use gx_core::{Checkpoint, Cid, FingerprintBytes, KeyId, Timestamp, TransformationId};
    use gx_witness::receipt::{ReceiptKind, ReceiptPayload, VerdictSummary};
    use gx_witness::{KeyPair, VerdictKind};

    /// A signed `VerdictReceipt` and the public key that verifies it — the fixture every test below
    /// starts from. `KeyPair::from_seed` (not `generate`): deterministic, and this crate calls no
    /// entropy source anywhere in its own code (the crate header's whole claim).
    fn signed_verdict_receipt() -> (Receipt, KeyPair) {
        let key = KeyPair::from_seed("test-key", &[7u8; 32]);
        let transformation = TransformationId(Cid([9u8; 32]));
        let payload = ReceiptPayload {
            key_id: key.key_id().clone(),
            verdict: Some(VerdictSummary {
                kind: VerdictKind::Admit,
                proof_digest: Cid([3u8; 32]),
            }),
            enforced: true,
            confinement: Some(gx_witness::receipt::ConfinementContext::unconfined()),
            catalogue_hash: None,
            // 🔴 `req/493` §0 / AC-6: a fixture stands where a producer would, and says the true
            // thing about the process that built it — nothing confined this test.
            // DR-46-24(A): absent on a verdict receipt (the escrow reads at 43 T-10b).
            read_set: None,
            // DR-46-26: absent for the same reason — C-25 is answered by that same escrow.
            reversibility: None,
            // DR-46-28: `unknown` for both stages — a hand-built fixture establishes neither.
            determinism_boundary: gx_core::DeterminismBoundary::Unknown,
            receipt_kind: ReceiptKind::VerdictReceipt,
            canonical_cid: transformation.0,
            inverse_delta: None,
            transformation,
            inclusion_proof: None,
            // P2 / DR-46-24(A): 42 §3.5's scope, now inside the signed bytes.
            fingerprint_scope: "wasm-fixture://scope".to_string(),
            fail_posture_engaged: false,
            precondition_fingerprint: FingerprintBytes([1u8; 32]),
            postcondition_fingerprint: None,
            // F7 / R-868-6 (`req/919` W5): a fixture built by this crate's own current code
            // carries the current version, same as any receipt this build issues.
            payload_version: Some(gx_witness::CURRENT_PAYLOAD_VERSION),
            // A2 (`req/919` W8): a fixture built by this crate's own current code names an
            // engine the same way a receipt this build issues does.
            engine_version: Some("gx-engine 0.1.0".to_string()),
        };
        let receipt = Receipt::issue(&payload, Timestamp(0), &key).expect("a legal payload signs");
        (receipt, key)
    }

    #[test]
    fn a_receipt_signed_with_its_own_key_verifies() {
        let (receipt, key) = signed_verdict_receipt();
        let receipt_json = serde_json::to_string(&receipt).expect("Receipt serialises");
        let public_key_b64 = gx_core::b64::encode(&key.public().to_bytes());
        let out = render(run(
            &receipt_json,
            key.key_id(),
            &public_key_b64,
            None,
            None,
            None,
        ));
        println!("VERIFIED_OK={out}");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON out");
        assert_eq!(
            parsed["valid"], true,
            "own-key verification must pass: {out}"
        );
        assert_eq!(parsed["checks"]["inclusion"], "not_applicable");
        assert_eq!(parsed["error"], serde_json::Value::Null);
    }

    #[test]
    fn a_tampered_receipt_refuses_without_touching_downstream_checks() {
        let (receipt, key) = signed_verdict_receipt();
        let mut receipt_json: serde_json::Value =
            serde_json::to_value(&receipt).expect("Receipt serialises to a JSON object");
        // Flip one byte of the DSSE payload's base64 — AC-019's tamper case, at the wire boundary.
        let payload_b64 = receipt_json["envelope"]["payload"]
            .as_str()
            .expect("payload is a base64 string (44 §2.2)")
            .to_string();
        let mut flipped = payload_b64.into_bytes();
        let last = flipped.len() - 1;
        flipped[last] = if flipped[last] == b'A' { b'B' } else { b'A' };
        receipt_json["envelope"]["payload"] =
            serde_json::Value::String(String::from_utf8(flipped).expect("still ASCII"));

        let public_key_b64 = gx_core::b64::encode(&key.public().to_bytes());
        let out = render(run(
            &receipt_json.to_string(),
            key.key_id(),
            &public_key_b64,
            None,
            None,
            None,
        ));
        println!("TAMPERED={out}");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON out");
        assert_eq!(
            parsed["valid"], false,
            "a flipped byte must not verify: {out}"
        );
        assert_eq!(
            parsed["checks"],
            serde_json::Value::Null,
            "a signature failure must not report canonical_cid/inclusion at all (req/29 §4): {out}"
        );
        assert!(
            parsed["error"].as_str().is_some(),
            "a refusal must name why: {out}"
        );
    }

    #[test]
    fn malformed_json_is_a_named_error_not_a_panic() {
        let out = render(run("not json", "any-key", "AAAA", None, None, None));
        println!("MALFORMED={out}");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON out");
        assert_eq!(parsed["valid"], false);
        assert!(parsed["error"]
            .as_str()
            .expect("error is a string")
            .starts_with("receipt:"));
    }

    /// `crates/gx-cli/src/receipt.rs::INCLUSION_JSON`'s five words, copied literally so a future
    /// rename on either side is caught by a failing assertion rather than by two silently
    /// diverging APIs (the same argument `crates/gx-api/tests/router.rs` makes for named lists).
    #[test]
    fn inclusion_vocabulary_matches_the_cli() {
        let cli_words = [
            "not_applicable",
            "verified",
            "refuted",
            "unanchored",
            "unbridged",
        ];
        let wasm_words = [
            inclusion_word(InclusionCheck::NotApplicable),
            inclusion_word(InclusionCheck::Verified),
            inclusion_word(InclusionCheck::Refuted),
            inclusion_word(InclusionCheck::Unanchored),
            inclusion_word(InclusionCheck::Unbridged),
        ];
        assert_eq!(cli_words, wasm_words);
    }

    // `SigningKey` is imported for the doc-comment's promise ("dev-only need") to have a caller;
    // `KeyPair::from_seed` already covers signing internally, so this import is only exercised if a
    // future test needs a bare `SigningKey`. Referenced here so `cargo clippy` does not flag it as
    // unused while it is declared in `[dev-dependencies]`.
    #[test]
    fn ed25519_dalek_signing_key_constructs_from_the_same_seed_keypair_does() {
        let direct = SigningKey::from_bytes(&[7u8; 32]);
        let (_, key) = signed_verdict_receipt();
        assert_eq!(direct.verifying_key().to_bytes(), key.public().to_bytes());
        // And `KeyId`/`Timestamp` really are the plain types the fixture above assumes.
        let _: KeyId = key.key_id().clone();
        let _: Timestamp = Timestamp(0);
    }

    // -----------------------------------------------------------------------
    // 🔴 **E-SDK-10** (`req/503` §0, §3) — the anchor's own signature and origin
    // -----------------------------------------------------------------------

    /// A `CommitReceipt` that a real `TileLog` produced, and the **signed** head that log would
    /// publish, under a *second* key (45 ASM-45-1: the ledger's key need not be the receipt's, and
    /// a fixture that reused one key would let a verifier confuse the two and still pass).
    ///
    /// The recipe is `crates/gx-witness/tests/support/mod.rs::commit_receipt_in_a_log`, restated
    /// here rather than imported: that module is another crate's `tests/` subdirectory and is not
    /// reachable from this one. The one addition is [`gx_witness::dsse::sign_checkpoint`] — the
    /// support helper returns an *unsigned* head, and an unsigned head cannot be the subject of a
    /// test about signatures.
    fn commit_receipt_and_signed_head(others: u64) -> (Receipt, KeyPair, Checkpoint, KeyPair) {
        use gx_log::{proof, TileLog};

        let key = KeyPair::from_seed("receipt-key", &[11u8; 32]);
        let ledger = KeyPair::from_seed("ledger-key", &[12u8; 32]);
        let seed = 42u64;

        let mut log = TileLog::new();
        for i in 0..others {
            log.append(tid(900_000 + i), cid(910_000 + i), Timestamp(i as i64))
                .expect("canonical");
        }

        let staged = commit_payload(&key, seed, empty_proof());
        let index = log.len();
        log.append(
            tid(seed),
            staged.ledger_digest().expect("canonical"),
            Timestamp(1),
        )
        .expect("canonical");

        let inclusion = proof::prove_inclusion(&log, index).expect("the entry is in the log");
        let receipt = Receipt::issue(
            &commit_payload(&key, seed, inclusion),
            Timestamp(1_754_600_000_000_000_000),
            &key,
        )
        .expect("a legal commit receipt signs");

        let unsigned = proof::unsigned_checkpoint(&log, "glovrex-ledger/v1", Timestamp(2))
            .expect("a non-empty log has a head");
        let head =
            gx_witness::dsse::sign_checkpoint(&unsigned, ledger.signing_key(), ledger.key_id())
                .expect("a head signs");
        (receipt, key, head, ledger)
    }

    fn cid(seed: u64) -> Cid {
        let mut raw = [0u8; 32];
        raw[..8].copy_from_slice(&seed.to_be_bytes());
        Cid(raw)
    }

    fn tid(seed: u64) -> TransformationId {
        TransformationId(cid(9_000_000 + seed))
    }

    fn empty_proof() -> gx_core::InclusionProof {
        gx_core::InclusionProof {
            leaf_index: 0,
            tree_size: 0,
            audit_path: Vec::new(),
        }
    }

    /// `crates/gx-witness/tests/support/mod.rs::commit_payload`, restated for the same reason.
    fn commit_payload(key: &KeyPair, seed: u64, proof: gx_core::InclusionProof) -> ReceiptPayload {
        let t = tid(seed);
        ReceiptPayload {
            key_id: key.key_id().clone(),
            verdict: Some(VerdictSummary {
                kind: VerdictKind::Admit,
                proof_digest: cid(500_000 + seed),
            }),
            enforced: true,
            confinement: Some(gx_witness::receipt::ConfinementContext::unconfined()),
            catalogue_hash: None,
            // 🔴 `req/493` §0 / AC-6: a fixture stands where a producer would, and says the true
            // thing about the process that built it — nothing confined this test.
            // `read_set` became `Option<ReadSet>` when DR-46-34 widened the absence of a read set
            // into four spellings (`crates/gx-witness/src/receipt.rs`). This fixture is a receipt
            // that *did* journal its reads, so it carries `Some`; the absent spellings are
            // exercised by `crates/gx-engine/tests/dr4634_read_set_absence.rs`.
            read_set: Some(
                gx_witness::receipt::ReadSet::from_reads(vec![gx_witness::receipt::ReadEntry {
                    digest: cid(700_000 + seed),
                    locator: "mcp://fixture/resource/notes/0000/body.md".to_string(),
                }])
                .expect("the fixture entry has a canonical form"),
            ),
            reversibility: Some(gx_core::Reversibility::True),
            determinism_boundary: gx_core::DeterminismBoundary::Mixed {
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                verdict_derivation: gx_core::BoundaryStage::DeterministicReplay,
            },
            receipt_kind: ReceiptKind::CommitReceipt,
            canonical_cid: t.0,
            inverse_delta: Some(cid(600_000 + seed)),
            transformation: t,
            inclusion_proof: Some(proof),
            fingerprint_scope: "wasm-fixture://scope".to_string(),
            fail_posture_engaged: false,
            precondition_fingerprint: FingerprintBytes([7u8; 32]),
            postcondition_fingerprint: Some(FingerprintBytes([8u8; 32])),
            // F7 / R-868-6 (`req/919` W5): a fixture built by this crate's own current code
            // carries the current version, same as any receipt this build issues.
            payload_version: Some(gx_witness::CURRENT_PAYLOAD_VERSION),
            // A2 (`req/919` W8): a fixture built by this crate's own current code names an
            // engine the same way a receipt this build issues does.
            engine_version: Some("gx-engine 0.1.0".to_string()),
        }
    }

    /// The four strings the boundary takes, for a fixture pair.
    fn args(receipt: &Receipt, key: &KeyPair, head: &Checkpoint) -> (String, String, String) {
        (
            serde_json::to_string(receipt).expect("Receipt serialises"),
            gx_core::b64::encode(&key.public().to_bytes()),
            serde_json::to_string(head).expect("Checkpoint serialises"),
        )
    }

    /// The control the three negatives are measured against: everything genuine verifies, and the
    /// answer says the anchor **was** authenticated when a key for it was offered (`req/503` §3-4).
    #[test]
    fn a_genuine_receipt_and_a_genuine_signed_head_verify() {
        let (receipt, key, head, ledger) = commit_receipt_and_signed_head(6);
        let (receipt_json, pk, head_json) = args(&receipt, &key, &head);
        let out = render(run(
            &receipt_json,
            key.key_id(),
            &pk,
            Some(&head_json),
            Some(ledger.key_id()),
            Some(&gx_core::b64::encode(&ledger.public().to_bytes())),
        ));
        println!("ESDK10_CONTROL={out}");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON out");
        assert_eq!(parsed["valid"], true, "the genuine pair must verify: {out}");
        assert_eq!(parsed["checks"]["inclusion"], "verified");
        assert_eq!(
            parsed["anchor_authenticated"], true,
            "a key was offered and the head is genuine: {out}"
        );
    }

    /// 🔴 **`req/503` §3-1** — one byte of the head's own signature, flipped.
    ///
    /// Before this repair the answer was `valid: true`: `verify_offline` reads `tree_size` and
    /// `root_hash` off the struct and never asks who said them, so a forger holding both files
    /// signs nothing and is believed. `crates/gx-cli` has closed this since M6H8-11; the SDK had
    /// not.
    #[test]
    fn a_head_whose_signature_was_flipped_is_refused() {
        let (receipt, key, head, ledger) = commit_receipt_and_signed_head(6);
        let mut forged = head.clone();
        forged.signature.sig[0] ^= 0x01;
        let (receipt_json, pk, head_json) = args(&receipt, &key, &forged);
        let out = render(run(
            &receipt_json,
            key.key_id(),
            &pk,
            Some(&head_json),
            Some(ledger.key_id()),
            Some(&gx_core::b64::encode(&ledger.public().to_bytes())),
        ));
        println!("ESDK10_SIGFLIP={out}");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON out");
        assert_eq!(
            parsed["valid"], false,
            "a head that does not verify under the key offered for it is not an anchor: {out}"
        );
        assert_eq!(
            parsed["anchor_authenticated"], false,
            "and the answer says which half refused: {out}"
        );
        assert!(
            parsed["error"]
                .as_str()
                .expect("a refusal names why")
                .starts_with("checkpoint_key:"),
            "the refusal names the anchor, not the receipt: {out}"
        );
    }

    /// 🔴 **`req/503` §3-1, second control** — the `origin`, changed.
    ///
    /// `tree_size` and `root_hash` are untouched, so the inclusion arithmetic is unaffected and a
    /// verifier that reads only those two answers `verified` about a head belonging to **another
    /// log**. `checkpoint_signing_message` covers `origin`, so authenticating the anchor is what
    /// catches it, and nothing else in this crate would.
    #[test]
    fn a_head_whose_origin_was_changed_is_refused() {
        let (receipt, key, head, ledger) = commit_receipt_and_signed_head(6);
        let mut forged = head.clone();
        "another-ledger/v1".clone_into(&mut forged.origin);
        let (receipt_json, pk, head_json) = args(&receipt, &key, &forged);

        // First, the fact that makes this test worth having: the inclusion half still passes.
        let unauthenticated = render(run(
            &receipt_json,
            key.key_id(),
            &pk,
            Some(&head_json),
            None,
            None,
        ));
        println!("ESDK10_ORIGIN_UNAUTHENTICATED={unauthenticated}");
        let taken_on_trust: serde_json::Value =
            serde_json::from_str(&unauthenticated).expect("valid JSON out");
        assert_eq!(
            taken_on_trust["checks"]["inclusion"], "verified",
            "the arithmetic does not notice an origin: {unauthenticated}"
        );
        assert_eq!(
            taken_on_trust["anchor_authenticated"], false,
            "and with no key offered the answer must say so rather than stay silent: {unauthenticated}"
        );

        // With a key offered, it is refused.
        let out = render(run(
            &receipt_json,
            key.key_id(),
            &pk,
            Some(&head_json),
            Some(ledger.key_id()),
            Some(&gx_core::b64::encode(&ledger.public().to_bytes())),
        ));
        println!("ESDK10_ORIGIN={out}");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON out");
        assert_eq!(
            parsed["valid"], false,
            "another log's head is not this log's anchor: {out}"
        );
        assert_eq!(parsed["anchor_authenticated"], false);
    }

    /// 🔴 **`req/503` §3-2** — the head's `tree_size`, replaced with 0, 2 and 99.
    ///
    /// This one is **not** E-SDK-10's to close and the test says so: the Rust source already
    /// answers `unbridged` here (H-09's `verify_inclusion_from`), and what was broken was the
    /// *shipped binary*, which is E-SDK-9. Kept in this suite as the discriminator that tells the
    /// two repairs apart — if it ever goes red in Rust, the fault is in the engine and not in a
    /// build step.
    #[test]
    fn a_head_naming_another_tree_size_is_not_a_pass() {
        let (receipt, key, head, ledger) = commit_receipt_and_signed_head(6);
        for size in [0u64, 2, 99] {
            let mut forged = head.clone();
            forged.tree_size = size;
            let (receipt_json, pk, head_json) = args(&receipt, &key, &forged);
            // Unauthenticated on purpose: this is the E-SDK-9 half, and it must hold with or
            // without a checkpoint key.
            let out = render(run(
                &receipt_json,
                key.key_id(),
                &pk,
                Some(&head_json),
                None,
                None,
            ));
            println!("ESDK9_TREE_SIZE_{size}={out}");
            let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON out");
            assert_ne!(
                parsed["checks"]["inclusion"], "verified",
                "a head naming tree_size {size} says nothing about this receipt: {out}"
            );
            assert_eq!(parsed["valid"], false);
        }
        let _ = ledger;
    }

    /// A checkpoint key with no checkpoint is a usage error, not a weaker check
    /// (`crates/gx-cli/src/main.rs:2346-2354`, verbatim reasoning).
    #[test]
    fn a_checkpoint_key_with_no_checkpoint_is_refused() {
        let (receipt, key) = signed_verdict_receipt();
        let receipt_json = serde_json::to_string(&receipt).expect("Receipt serialises");
        let pk = gx_core::b64::encode(&key.public().to_bytes());
        let out = render(run(
            &receipt_json,
            key.key_id(),
            &pk,
            None,
            Some("ledger-key"),
            Some(&pk),
        ));
        println!("ESDK10_KEY_WITHOUT_ANCHOR={out}");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON out");
        assert_eq!(parsed["valid"], false);
        assert_eq!(parsed["anchor_authenticated"], false);
        assert!(parsed["error"]
            .as_str()
            .expect("a refusal names why")
            .starts_with("checkpoint_key:"));
    }

    /// **M6H8-11 adopted (a)**, ported: the field is on **every** answer, including the ones that
    /// never saw an anchor and the ones that refused before reaching it. A field that appeared only
    /// when it was `true` is a field a reader misses on exactly the runs where it matters.
    #[test]
    fn anchor_authenticated_is_present_on_every_answer() {
        let (receipt, key) = signed_verdict_receipt();
        let receipt_json = serde_json::to_string(&receipt).expect("Receipt serialises");
        let pk = gx_core::b64::encode(&key.public().to_bytes());

        let ok = render(run(&receipt_json, key.key_id(), &pk, None, None, None));
        let malformed = render(run("not json", "any-key", "AAAA", None, None, None));
        for out in [&ok, &malformed] {
            let parsed: serde_json::Value = serde_json::from_str(out).expect("valid JSON out");
            assert_eq!(
                parsed["anchor_authenticated"],
                serde_json::json!(false),
                "present and false, not absent: {out}"
            );
        }
        println!("ESDK10_ALWAYS_PRESENT ok={ok} malformed={malformed}");
    }
}
