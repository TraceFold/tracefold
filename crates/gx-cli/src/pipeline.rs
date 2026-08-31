// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx submit` / `gx plan` / `gx verify` / `gx commit` — 44 §1.2's pipeline, front half.
//!
//! Four verbs, four processes, one transformation. [`crate::session`] is where the four are made
//! into one; this module is what each of them does once the row is in front of it.
//!
//! # 🔴 What the CLI is doing here, in Rule 1's terms (sem: SEM-gx-cli-246)
//!
//! Every function below is the same shape: parse arguments, hand them to one of 43's eight entry
//! points, read accessors, print JSON. No `Verdict` is built (41 §4), no `Cid` is minted (41 §6), no
//! `Lifecycle` is written (42 §1.3-3). The one place that could look otherwise is [`verify`]'s exit
//! code, which is a **reading** of the state the engine returned rather than a second judgement —
//! and 44 §1.4 is the table it reads from.
//!
//! # 🔴 Three things 44 §1.2 asks for that v0.1 cannot produce, written down rather than faked
//!
//! req/88 §3 Λ4: "the discipline is not 'do not fold' but 'write down what you folded'" (sem: SEM-gx-cli-247). Three folds, all in [`verify`]:
//!
//! 1. **`AdmitProof` and the deny reasons have no accessor.** 44 §1.2's stdout for `gx verify` is
//!    `{"kind":"Admit","proof":AdmitProof}` / `{"kind":"Deny","reasons":[Reason]}` /
//!    `{"kind":"Escalate","ticket":EscalationTicket}`. The engine keeps the *digest* of the proof
//!    (E-M5-7's `verdict_digest`) and the ticket, and exposes neither the proof nor the reasons —
//!    `Engine::verdict` answers a `VerdictKind`. So this prints the kind, the digest and the ticket,
//!    and an operator asking "why was I denied" (sem: SEM-gx-cli-248) gets a digest. Raised as **M6H3-2**.
//! 2. **`Aborted` has no exit of its own on `verify`.** §1.2 gives `verify` 0/1/2/4/6, and 43 T-4d
//!    (`VerifierUnavailable`) and T-6 (`Expired`) both end at `Aborted` from inside this call. They
//!    land on **1**, which §1.4 spells "error (invalid input / internal error / adapter error)" (sem: SEM-gx-cli-249) — true of
//!    T-4d and a stretch for a TTL. Raised as **M6H3-3**.
//! 3. **43 §8's waiting has no exit at all.** A `Candidate` held behind a conflicting predecessor is
//!    a successful call that reached no verdict, and 44 has no seat for it. It lands on 1 with
//!    `"held_by"` in the JSON, because 0 would say "Admit" (44 §1.4: "reached the intended state"; sem: SEM-gx-cli-250) about a
//!    transformation nothing has judged. Same ticket.

use std::path::PathBuf;

use gx_core::{
    AbortReason, Actor, ChangeContext, EnforcementMode, GoalBytes, Intent, SubstrateKind,
    Timestamp, TransformationId,
};
use gx_engine::store::FingerprintRecord;
use gx_engine::Lifecycle;

use crate::exit::{
    Outcome, APPLY_FAILED, DENIED, ERROR, ESCALATED, NOT_FOUND, OK, PRECONDITION_CHANGED,
};
use crate::receipt::{ReceiptStore, StoredKind};
use crate::session::Session;
use crate::{Error, Result};

/// The arguments `gx submit` was given, after parsing and before the engine sees them.
///
/// A struct rather than eight parameters because 44 §1.2's synopsis is eight flags and the mapping
/// from flag to field is the thing a reader checks.
pub struct SubmitSpec {
    /// `--substrate <fs|git|mcp>`.
    pub substrate: SubstrateKind,
    /// `--locator <STR>`, in the adapter's own spelling (42 §3.3).
    pub locator: String,
    /// The bytes `--intent <FILE|->` carried.
    pub goal: Vec<u8>,
    /// `--context <Time|Evidence|…|Custom:NAME>`.
    pub context: ChangeContext,
    /// `--actor-key`, `--actor-kind` and `--actor-model`, resolved into 42 §3.2's value.
    pub actor: Actor,
    /// `--order <0|1|2>`.
    pub order: u8,
    /// `--parent <TID>...`.
    pub parents: Vec<TransformationId>,
}

/// 🔴 `gx submit` (44 §1.2) — T-1, and the draft file that lets T-2 happen in another process.
///
/// # `--order` and `--parent` are accepted and refused (M6H3-5)
///
/// 44 §1.2's synopsis has both. `Engine::submit` takes an `Intent`, and 42 §3.3's `Intent` is five
/// fields — substrate, locator, goal, context, actor — with no room for either; `Engine::plan` then
/// builds the `Transformation` with `order = 0` and `parents = []`, and the one producer of a
/// non-empty parents list in the whole engine is `undo` (T-12's guard). So there is **no receiving
/// surface**: a value passed here would reach nothing.
///
/// Refused rather than ignored. "do not give the unimplemented and a failure the same face" (sem: SEM-gx-cli-251) (M4H4-2) has a twin that M4H5-5
/// wrote — an argument that changes nothing must not look like an argument that worked — and an
/// operator who typed `--order 2` and got a Draft of order 0 back would have been lied to by a flag
/// 44 told them about.
///
/// # Errors
/// [`Error::Usage`] for an order or a parent v0.1 cannot carry. Anything the engine refuses,
/// unchanged — [`Error::Engine`] for an intent with no canonical form and for a journal that will
/// not append.
pub fn submit(
    session: &mut Session,
    spec: &SubmitSpec,
    rng_seed: u64,
    at: Timestamp,
) -> Result<Outcome> {
    // 🔴 **DR-43-4's entry sweep** (`req/38` §148 ruling 1(iv)) — see [`Session::sweep`].
    session.sweep(at)?;
    if spec.order != 0 {
        return Err(Error::Usage {
            detail: format!(
                "--order {} has nowhere to go: 42 §3.3's `Intent` carries no order and `plan` fixes \
                 it at 0 for every transformation of an object. v0.1 admits order <= 2 as a type \
                 (ASM-6) and produces only 0 (M6H3-5)",
                spec.order
            ),
        });
    }
    if !spec.parents.is_empty() {
        return Err(Error::Usage {
            detail:
                "--parent has nowhere to go: 43 T-12's guard is about `undo`, which is the one \
                     producer of a non-empty parents list, and `Engine::submit` takes an `Intent` \
                     (42 §3.3, five fields) (M6H3-5)"
                    .to_string(),
        });
    }

    let intent = Intent::new(
        spec.substrate.clone(),
        spec.locator.clone(),
        GoalBytes(spec.goal.clone()),
        spec.context.clone(),
        spec.actor.clone(),
    );
    // 🔴 The id is the **engine's**, and the draft is filed under it (Rule 1 (i); sem: SEM-gx-cli-252). A CLI that computed
    // the name it stored a body under would be minting an identity out of bytes it chose.
    let intent_id = session.engine().submit(&intent, rng_seed, at)?;
    session.drafts().put(&intent_id, &intent)?;

    // 44 §1.2's Draft object. Two fields are `null` for reasons worth naming rather than assuming:
    //
    // * `id` — "`TransformationId` is undetermined until `plan()` completes, ASM-11" (sem: SEM-gx-cli-253), which 44 writes itself;
    // * `subject` — 44 lists it, and 43 T-2 fixes it from the **snapshot** (`Subject::Object` of the
    //   object the adapter read). Before `plan` there is no snapshot, so there is no subject.
    //   Emitting an invented one would put a name in a Draft that the Candidate then disagreed with.
    //   Raised as **M6H3-6**.
    //
    // `created_at` is nanoseconds and says so in the field name — M6H2-4 adopted (a) left RFC 3339 to the (sem: SEM-gx-cli-254)
    // hand that owns 44 §2's fourteen endpoints, and a wrong RFC 3339 string is worse than a right
    // integer.
    Ok(Outcome::ok(serde_json::json!({
        "intent_id": intent_id.0.to_text(),
        "id": serde_json::Value::Null,
        "order": spec.order,
        "subject": serde_json::Value::Null,
        "delta": serde_json::Value::Null,
        "target": serde_json::Value::Null,
        "context": spec.context,
        "actor": spec.actor,
        "parents": spec.parents.iter().map(|p| p.0.to_text()).collect::<Vec<_>>(),
        "created_at_unix_nanos": at.0,
        "state": "Draft",
    })))
}

/// 🔴 `gx plan <ID>` (44 §1.2) — T-2, from either spelling of the id (44 §0).
///
/// > `<ID>` accepts either `IntentId` (Draft stage) or `TransformationId` (id-resolution rule,
/// > §0) (sem: SEM-gx-cli-255)
///
/// The rule is three steps and the third has the authority: parse the text, look for a draft under
/// it, and if there is none ask the engine which intent the transformation came from. `index.rs`
/// deliberately provides no one-call helper for this, because folding the three would put the order
/// of authority inside a cache module.
///
/// # Errors
/// [`Error::NotFound`] (44 §1.4's 6) if neither reading names anything this `.gx/` has seen.
/// [`Error::Engine`] for an adapter that cannot plan — 44 §1.2's "1 = adapter error (unable to plan)" (sem: SEM-gx-cli-256).
/// [`Error::Usage`] when 43 T-2's re-plan guard refuses because this intent's transformation has
/// left `Candidate` (`req/981` F1 — see the arm below).
pub fn plan(session: &mut Session, id: &str, at: Timestamp) -> Result<Outcome> {
    // 🔴 **DR-43-4's entry sweep** (`req/38` §148 ruling 1(iv)) — see [`Session::sweep`].
    session.sweep(at)?;
    let cid = crate::index::parse_id(id)?;
    let (as_intent, as_transformation) = crate::index::both_readings(cid);

    let intent = match session.draft(&as_intent)? {
        Some(intent) => Some((as_intent, intent)),
        None => match session.intent_of(&as_transformation)? {
            Some(iid) => session.draft(&iid)?.map(|i| (iid, i)),
            None => None,
        },
    };
    let Some((intent_id, intent)) = intent else {
        return Err(Error::NotFound {
            what: "draft or transformation",
            id: id.to_string(),
        });
    };

    // 🔴 **`req/981` §6 F1** — 43 T-2's re-plan guard, wearing the fact instead of `INTERNAL`.
    //
    // `Engine::plan` refuses with `InvalidState` when this intent already resolved to a
    // transformation that has left `Candidate`, and this crate carried every `Engine(_)` it did not
    // name to `ROW_INTERNAL` — 44 §2.3's word for what **cannot be classified**. `req/981` measured
    // what that costs at the surface an operator types at: `gx plan` answering
    // `{"gx_code":"INTERNAL","title":"the operation could not be completed",
    // "detail":"TransformationId(Cid(opaque)) is Committed, and 43 §3 has no `plan` from there"}`
    // — an opaque id, an unclassified word, and no road out — on **140 of 300** census runs, for
    // the most ordinary thing an agent does: submitting the same deletion twice.
    //
    // The condition is completely classified, which is exactly `INTERNAL`'s complement: the state
    // machine named the state and named the transition it does not offer. It is the same argument
    // `ROW_LAYOUT_BLOCKED`, `ROW_JOURNAL_UNREADABLE` and R23's `NOT_FOUND` arm each made one
    // condition over, and it costs no new word and no new status: 44 §1.2 already gives `gx plan`
    // **1**, `VALIDATION_ERROR` declares 1, and `ROW_VALIDATION_ERROR`'s own `why` already claims
    // this shape ("the arguments … do not describe an operation" — this request names an intent
    // whose only remaining transition is one nobody can ask for).
    //
    // 🔴 What is **not** taken here, and why, so that the next lane does not have to re-derive it:
    // `gx-api` answers this same `gx_engine::Error::InvalidState` with `INVALID_STATE` (409), whose
    // `gx-api` row declares `cli_exit` **2**. Wearing that word on this face is the right end state
    // — the two faces of one system should not hold two names for one refusal (R21, `req/304` D5) —
    // but `EXIT_AGREEMENT` (`r21_refusal_map_is_whole.rs`) would then force this verb to exit 2, and
    // 44 §1.4's 2 is "refused (denied)", which on `gx verify` and `gx commit` already means *the
    // gate said no*. One number with two meanings on one verb is discipline 52's failure, and
    // `req/306` §1 forbids moving an exit status without a ruling. Filed rather than taken.
    let transformation = match session.engine().plan(&intent, at) {
        Ok(transformation) => transformation,
        Err(gx_engine::Error::InvalidState {
            state,
            attempted: "plan",
            ..
        }) => {
            // What a plan of this intent would name now, measured rather than described: it is the
            // read-only half of the same `plan_shape` (R3, `req/222` H-03) and it writes nothing.
            let names_now = session.engine().planned_id(&intent, at).map_or_else(
                |_| "a transformation this substrate would not answer for".to_string(),
                |t| t.0.to_text(),
            );
            return Err(Error::Usage {
                detail: format!(
                    "{id} names an intent this project has already planned, and its transformation \
                     is {state}: 43 T-2 allows a re-plan only while the row is still a `Candidate`, \
                     so this one is refused and **nothing was written**. Planning it now would name \
                     {names_now} instead, which is 43 §8's stale `Fingerprint₀` seen from the \
                     intent's side. The road forward is a **different** intent: 42 §3.3 puts the \
                     substrate, the locator, the goal, the context and the actor in the `IntentId`, \
                     so submitting the identical request again names this same intent back. \
                     🔴 `req/981` F1 measured this on a deletion, whose goal is fixed at zero bytes \
                     and therefore carries no free bits — the two fields that would unstick it \
                     (`--context`, `--actor-key`) are the record of *why* the change is being made \
                     and *who* is asking, and renaming either to get past a refusal corrupts the \
                     record this system exists to keep"
                ),
            });
        }
        Err(e) => return Err(e.into()),
    };
    session.remember(intent_id, transformation);

    let engine = session.read();
    let state = engine.state(&transformation);
    Ok(Outcome::ok(serde_json::json!({
        "transformation": engine.transformation(&transformation),
        "delta_ref": engine.planned_delta(&transformation).map(|d| d.reference()),
        "precondition_fingerprint": engine
            .precondition_fingerprint(&transformation)
            .map(FingerprintRecord::of),
        // 🔴 The state the engine holds, not the literal "Candidate" (sem: SEM-gx-cli-257) 44 writes. They are the same on
        // the road 44 describes, and they are **not** the same when this call resumed a row another
        // process had already verified (see `session::Session::resume`): printing 44's word there
        // would report a rewind that did not happen.
        "state": state,
    })))
}

/// 🔴 `gx verify <TID>` (44 §1.2) — T-3 → T-4a/b/c/d/e, and the first command in this workspace that
/// can exit **2**.
///
/// `mode` is 44 §1.2's `--record-only`, "force DR-2's record-only mode **per this command**
/// (overriding the global setting)" (sem: SEM-gx-cli-258), carried to the engine as **M6-08 adopted (a)**'s per-call argument rather than through
/// `with_mode`. On this single-shot surface the two would be indistinguishable; the argument is what
/// makes the same call correct for `gx serve`, where a field on shared state would leak one
/// request's posture into another's.
///
/// # Errors
/// [`Error::NotFound`] for a transformation this `.gx/` has never planned. Anything the engine
/// refuses, unchanged.
pub fn verify(
    session: &mut Session,
    id: &TransformationId,
    mode: Option<EnforcementMode>,
    at: Timestamp,
) -> Result<Outcome> {
    // 🔴 The journal first (see `Session::recorded`). A transformation the gate has already answered
    // about is answered from Σ: 43 T-4a's idempotency column is "re-evaluation against the same
    // evidence set is deterministic (`Gate::verify` is defined as a pure function)" (sem: SEM-gx-cli-259), so the recorded verdict **is** the verdict, and
    // asking again would append a second `VerifyStarted` and a second `Verdict` for one evaluation —
    // which is precisely the equality req/88 §3 Λ2 asserts between N processes and one engine.
    //
    // `reverified: false` is in the output because "we asked" and "we read what was asked before" (sem: SEM-gx-cli-260)
    // are different facts and req/29 §4 is the standing rule against giving them one face.
    // 🔴 **DR-43-4's entry sweep** (`req/38` §148 ruling 1(iv)) — before the recorded-verdict
    // shortcut, because a `Candidate` whose deadline passed is exactly the row this verb is about
    // to ask the gate about, and 43 T-6 says it is `Aborted` rather than verifiable.
    session.sweep(at)?;
    if let Some(row) = session.recorded(id) {
        if !matches!(row.state, Some(Lifecycle::Candidate)) {
            return Ok(recorded_verdict(id, &row, mode));
        }
    }
    session.resume(id, at)?;
    let key = session.signing_key(id)?;
    // 🔴 **43 §7's recovery, wired** (`req/38` §148 ruling 1(iv); `req/213` §7(b) left this
    // unwired and said why: `Session::open` has no key and is also a read verb's road). This is
    // the first line of a **write** verb at which the key exists, so it is where M5H5-1's "the
    // first operation after the adapters are registered" becomes true on the CLI side too. The
    // key is the actor's, which is the right one for the row this verb names and an approximation
    // for any other `Committing` row the crash left — the same approximation `gx serve` makes with
    // the server's key, declared in `req/221` §7 rather than assumed away.
    session.recover(at, &key, "gx verify")?;
    let state = session.engine().verify(id, at, &key, mode)?;

    // 🔴 **M6H4-7** — ASM-14's receipt, filed under its own kind.
    //
    // "every `Verdict` -- Admit/Deny/Escalate -- is issued, 43 T-4a/T-4b/T-4c" (sem: SEM-gx-cli-261) (42 §3.10), and the engine holds
    // it in a table `Engine::open` does not rebuild (M5H3-5). Until this hand only the commit
    // receipt reached `.gx/receipts/`, so the document attesting the **judgement** — the one an
    // escalated or denied transformation is the whole story of, because it never commits — existed
    // for the lifetime of one process. `verdict_receipts` is a list (M5H4-6) and the one this call
    // produced is the last of it.
    let stored_verdict = match session.read().verdict_receipts(id).last() {
        Some(receipt) => {
            Some(ReceiptStore::in_layout(session.layout()).put(id, StoredKind::Verdict, receipt)?)
        }
        None => None,
    };

    let engine = session.read();
    let kind = engine.verdict(id);
    let json = serde_json::json!({
        // 44 §1.2's `Verdict` JSON is `{"kind": …}`.
        "kind": kind,
        "transformation": id.0.to_text(),
        "state": state,
        // 🔴 **M6H3-2 adopted (a)**, the two fields 44 §1.2 writes and hand 3 could not produce: (sem: SEM-gx-cli-262)
        // `{"kind":"Admit","proof":AdmitProof}` / `{"kind":"Deny","reasons":[Reason]}`. Hand 3
        // printed the verdict's **digest** and raised the ticket in as many words — "an operator
        // asking 'why was I denied' gets a digest" (sem: SEM-gx-cli-263) — because the engine consumed the `Verdict` for
        // its kind and dropped the value. `Engine::admit_proof` and `Engine::deny_reasons` are the
        // table reads §50 ruled, and this is one of their two consumers (the other is the HTTP
        // problem object's `detail`, 44 §2.3).
        //
        // 🔴 Both are `null` on a **re-read** rather than on a fresh verify, and that is not a bug
        // to be papered over: 42 §3.13 records `verdict_digest` and never the proof, so a row
        // rebuilt from Σ has the digest and not the value. `reverified` below is the field that
        // tells the two apart, and `recorded_verdict` prints `null` for these two with the same
        // `reverified: false` beside it.
        "proof": admit_proof_json(engine.admit_proof(id)),
        "reasons": engine.deny_reasons(id),
        "enforced": engine.enforced(id),
        "fail_posture_engaged": engine.fail_posture_engaged(id),
        "ticket": engine.ticket(id),
        "held_by": engine.blocked_by(id).map(|b| b.0.to_text()),
        "record_only": matches!(mode, Some(EnforcementMode::RecordOnly)),
        "reverified": true,
        "receipt_stored_at": stored_verdict.map_or(serde_json::Value::Null, |p| {
            p.display().to_string().replace('\\', "/").into()
        }),
    });

    // 44 §1.2: "exit: 0=Admit, 2=Deny, 4=Escalate, 1=internal/adapter error, 6=not-found" (sem: SEM-gx-cli-264). Everything the
    // list does not name lands on 1 and is listed in the module header rather than hidden.
    let code = match state {
        Lifecycle::Admitted => OK,
        Lifecycle::Denied => DENIED,
        Lifecycle::Escalated => ESCALATED,
        _ => ERROR,
    };
    Ok(if code == OK {
        Outcome::ok(json)
    } else {
        Outcome::refused(json, code)
    })
}

/// 🔴 `gx commit <TID>` (44 §1.2) — T-8/T-8r then T-9..T-11, and the **writer of `.gx/receipts/`**.
///
/// > canonicalize (T3 self-consistency check) → re-verify Fingerprint₁ (CAS) → apply →
/// > ledger.append → issue receipt, executed (sem: SEM-gx-cli-265)
///
/// Two engine calls, because 43 gives the two halves two transitions and `gx commit` is the command
/// 44 gives them both to. The receipt store is req/38 §49 M6H2-1's: "`.gx/receipts/`
/// (nature=Source), confirmed -- **the writer is `gx commit` (hand 3)**" (sem: SEM-gx-cli-266). Without it `gx receipt show` answers "not-found" forever,
/// because `Engine::receipt` reads a table `Engine::open` does not rebuild (M5H3-5).
///
/// # `--idempotency-key` (M6-11 is hand 5's)
///
/// 44 §1.2: "when unspecified, the CLI deterministically derives it from `transformation_id`
/// (retrying the same execution is naturally idempotent)" (sem: SEM-gx-cli-267). The
/// derivation is here and the **cache is not**: 44 §2.4's `(transformation_id, idempotency_key)`
/// table is an HTTP concern (M6-11, hand 5, `.gx/index/idempotency/`). What makes a repeated
/// `gx commit` safe today is 43 T-11's own idempotence — "`ledger.append` is an idempotent
/// operation keyed on `TransformationId`…" (sem: SEM-gx-cli-268) — and the engine's early return on a `Committed` row. The key is echoed so
/// that a script can see which one was used; nothing branches on it, and saying so is the difference
/// between a flag that works and a flag that looks like it does.
///
/// # Errors
/// [`Error::NotFound`] for an unknown transformation. Anything the engine refuses, unchanged.
pub fn commit(
    session: &mut Session,
    id: &TransformationId,
    idempotency_key: Option<&str>,
    mode: Option<EnforcementMode>,
    at: Timestamp,
) -> Result<Outcome> {
    let derived = idempotency_key.map_or_else(
        || format!("gx-commit:{}", id.0.to_text()),
        std::string::ToString::to_string,
    );

    // 🔴 The journal first, and here it is load-bearing rather than an optimisation.
    //
    // A resume re-plans, a re-plan reads the substrate, and **a successful commit has moved the
    // substrate** — so a `Committed` transformation can never be planned into itself again, and a
    // retried `gx commit` would be refused with "the world moved" (sem: SEM-gx-cli-269). 44 §1.2 promises the opposite:
    // "retrying the same execution is naturally idempotent" (sem: SEM-gx-cli-270). What makes that true is 43 T-11's own idempotence plus the
    // receipt already in `.gx/receipts/`, and neither needs a row.
    // 🔴 **DR-43-4's entry sweep** (`req/38` §148 ruling 1(iv)) — see [`Session::sweep`].
    session.sweep(at)?;
    if let Some(row) = session.recorded(id) {
        if let Some(terminal) = terminal_answer(session, id, &row, &derived, mode) {
            return Ok(terminal);
        }
    }
    session.resume(id, at)?;
    let key = session.signing_key(id)?;
    // 🔴 43 §7's recovery, wired — see `pipeline::verify` for the whole of the reasoning. This is
    // the verb it matters most for: 43 §7-3's crash window is *inside* T-9, so the row a crash
    // left `Committing` is a row this very command was running when the power went.
    session.recover(at, &key, "gx commit")?;

    // T-8 / T-8r. A `Canonicalized` row answers early, so a retried `gx commit` does not re-enter.
    if let Err(e) = session.engine().canonicalize(id, at, mode) {
        return Ok(canonicalise_refusal(session, id, &Error::from(e)));
    }
    let state = session.engine().commit(id, at, &key)?;

    let idempotency_key = derived;

    if matches!(state, Lifecycle::Committed) {
        let stored = match session.read().receipt(id) {
            Some(receipt) => {
                let store = ReceiptStore::in_layout(session.layout());
                Some(store.put(id, StoredKind::Commit, receipt)?)
            }
            None => None,
        };
        let engine = session.read();
        let receipt = engine.receipt(id);
        let mut json = serde_json::to_value(receipt).unwrap_or(serde_json::Value::Null);
        if let Some(map) = json.as_object_mut() {
            map.insert("transformation".into(), id.0.to_text().into());
            map.insert("state".into(), to_json(&state));
            map.insert("enforced".into(), to_json(&engine.enforced(id)));
            map.insert("idempotency_key".into(), idempotency_key.into());
            map.insert(
                "stored_at".into(),
                stored.map_or(serde_json::Value::Null, |p| {
                    p.display().to_string().replace('\\', "/").into()
                }),
            );
        }
        return Ok(Outcome::ok(json));
    }

    // 44 §1.2: "on `Aborted`: `{"aborted":true,"reason":…,"detail":…}`" (sem: SEM-gx-cli-271).
    let (reason, code) = match state {
        Lifecycle::Aborted(AbortReason::PreconditionChanged) => {
            ("PreconditionChanged", PRECONDITION_CHANGED)
        }
        Lifecycle::Aborted(AbortReason::ApplyFailed) => ("ApplyFailed", APPLY_FAILED),
        Lifecycle::Aborted(AbortReason::VerifierUnavailable) => ("VerifierUnavailable", ERROR),
        Lifecycle::Aborted(AbortReason::Expired) => ("Expired", ERROR),
        Lifecycle::Aborted(AbortReason::OwnerCancelled) => ("OwnerCancelled", ERROR),
        Lifecycle::Aborted(AbortReason::InternalError) => ("InternalError", ERROR),
        // 🔴 **M5-11 / blocker item 5** (`req/919` A1) — the seventh reason, named rather than
        // left to the `_` arm below, which would print `NotAborted` for a row that is aborted.
        // 44 §1.4 has no code of its own for it, so it takes the generic one the three reasons
        // above it take; a code is a wire contract and inventing one here would be this file
        // deciding 44's table.
        Lifecycle::Aborted(AbortReason::PostconditionMismatch) => ("PostconditionMismatch", ERROR),
        // 43 T-9's idempotency column answers a re-entrant `commit` with `Committing`, which is not
        // an abort and is not a success: the critical section is somebody else's. 44 has no code for
        // it (M6H3-3).
        _ => ("NotAborted", ERROR),
    };
    let engine = session.read();
    // The predicate is a named function, and the reason is worth one line: see [`is_committing`].
    let aborted = !is_committing(state);
    Ok(Outcome::refused(
        serde_json::json!({
            "aborted": aborted,
            "reason": reason,
            "detail": engine.rollback(id),
            // 🔴 **`req/324` §5(d) (`req/38` §231 ruling 5)** — the cause beside the value.
            //
            // `detail` is one word for three facts, and the proxy writes a sentence about it. An
            // additive member rather than a fourth `Rollback` value: the value is on a record that
            // is already written and already signed, and nothing here decides anything. `null` when
            // this process did not reach the abort itself, which the proxy has an arm for.
            "not_attempted_because": engine
                .rollback_not_attempted_because(id)
                .map(|because| because.kind()),
            "transformation": id.0.to_text(),
            "state": state,
            "idempotency_key": idempotency_key,
        }),
        code,
    ))
}

/// 🔴 Whether a state is 43 T-9's critical section — a **function**, for two readers that disagree.
///
/// Rule 1 (iii) is a text scan (`crates/gx-canon/tests/authority_boundary.rs`) and it reads two shapes
/// as "the CLI keeps a state table" (sem: SEM-gx-cli-272): a line that declares a field and names `Lifecycle`, and a line
/// with `Lifecycle::` to the right of an `=`. Both are right about what they hunt, and the obvious
/// spellings of this predicate hit one each — `"aborted": !matches!(state, Lifecycle::Committing),`
/// the first, `let aborted = !matches!(…)` the second. Writing it as a `match` cleared both and then
/// clippy asked for the `matches!` back (`match_like_matches_macro`), which is where a named
/// function stops being a workaround and starts being the answer: the macro lives on a line with no
/// assignment, and the call site names a fact rather than a variant.
fn is_committing(state: Lifecycle) -> bool {
    matches!(state, Lifecycle::Committing)
}

/// 🔴 What a refused `canonicalize` means for 44 §1.2's `gx commit` exit list.
///
/// 43 T-8's from-state is `Admitted`, and T-8r adds `Denied` **under `RecordOnly` only**. So a
/// refusal here is the state machine answering, and the two answers 44 gives it codes for are the
/// two this reads:
///
/// * a `Denied` row outside record-only is "2 = refused because Deny/not-Admit (outside
///   record-only and Verdict≠Admit)" (sem: SEM-gx-cli-273);
/// * an `Escalated` row is "4 = cannot operate while still Escalated (ESCALATION_PENDING)", which §1.4 spells
///   out for exactly this call.
///
/// Everything else — a `Candidate` nobody verified, a `Committed` row, `NotIdempotent` from T3 —
/// exits 1. The state is read from the **engine** rather than from the error's text, because a
/// refusal message is prose and a state is a value.
fn canonicalise_refusal(session: &Session, id: &TransformationId, e: &Error) -> Outcome {
    let engine = session.read();
    let state = engine.state(id);
    let code = match state {
        Some(Lifecycle::Denied) => DENIED,
        Some(Lifecycle::Escalated) => ESCALATED,
        None => NOT_FOUND,
        _ => ERROR,
    };
    Outcome::refused(
        serde_json::json!({
            "aborted": false,
            "reason": "NotCanonicalizable",
            "detail": e.to_string(),
            "transformation": id.0.to_text(),
            "state": state,
        }),
        code,
    )
}

/// 🔴 A verdict the journal already holds, reported without asking the gate again.
///
/// The verdict-to-status map is 44 §1.2's own — "0=Admit, 2=Deny, 4=Escalate" (sem: SEM-gx-cli-274) — read from Σ rather
/// than from a call that happened in another process, which is what makes 43 T-4a's "re-evaluation is deterministic"
/// the honest reading rather than a hope. Anything past a verdict (`Canonicalized`, `Committed`,
/// `Aborted`, `Superseded`) is not a verify at all and lands on 1.
fn recorded_verdict(
    id: &TransformationId,
    row: &gx_engine::StateRow,
    mode: Option<EnforcementMode>,
) -> Outcome {
    let json = serde_json::json!({
        "kind": row.verdict,
        "transformation": id.0.to_text(),
        "state": row.state,
        // 🔴 **M6H3-2's limit, printed rather than hidden.** 42 §3.13 records the verdict's digest
        // and never the proof, so a verdict read back out of Σ has a kind and no `AdmitProof` and
        // no `[Reason]`. `null` here beside `reverified: false` says "the gate was asked in another
        // process and what it said is not recoverable from the log" (sem: SEM-gx-cli-275); a caller that needs the value
        // has to ask the gate again, which 43 T-4a says is deterministic and which this function
        // deliberately does not do (Λ2: two evaluations for one judgement).
        "proof": serde_json::Value::Null,
        "reasons": serde_json::Value::Null,
        "enforced": row.enforced,
        "fail_posture_engaged": row.fail_posture_engaged,
        "ticket": serde_json::Value::Null,
        "held_by": serde_json::Value::Null,
        "record_only": matches!(mode, Some(EnforcementMode::RecordOnly)),
        // 🔴 The one field that separates this answer from the one above it. A reader who cannot
        // tell "the gate was asked" from "the log was read" (sem: SEM-gx-cli-276) cannot tell how many evaluations
        // produced the receipts they are holding.
        "reverified": false,
    });
    let code = match row.state {
        Some(Lifecycle::Admitted) => OK,
        Some(Lifecycle::Denied) => DENIED,
        Some(Lifecycle::Escalated) => ESCALATED,
        None => NOT_FOUND,
        _ => ERROR,
    };
    if code == OK {
        Outcome::ok(json)
    } else {
        Outcome::refused(json, code)
    }
}

/// 🔴 A transformation the journal says is finished, answered without rebuilding a row.
///
/// `None` means "there is still work to do" (sem: SEM-gx-cli-277) and the caller resumes. The three that answer here:
///
/// * **`Committed`** — 44 §1.2's 0, with the receipt from `.gx/receipts/`. This is what makes
///   "retrying the same execution is naturally idempotent" (sem: SEM-gx-cli-278) true across processes; see [`commit`]'s note on why a resume
///   would refuse instead.
/// * **`Denied`** — 44 §1.2's "2 = refused because Deny/not-Admit (outside record-only and
///   Verdict≠Admit)" (sem: SEM-gx-cli-279). It is
///   answered here rather than by driving T-8 so that the refusal costs no substrate read, and it is
///   **not** answered here under `RecordOnly`, where T-8r is a road rather than a wall.
/// * **`Escalated`** — "4 = cannot operate while still Escalated (ESCALATION_PENDING)" (sem: SEM-gx-cli-280), which 44 §1.4 names for
///   this call in as many words.
///
/// `Aborted` and `Superseded` are terminal and are reported with 44's code for the reason.
fn terminal_answer(
    session: &Session,
    id: &TransformationId,
    row: &gx_engine::StateRow,
    idempotency_key: &str,
    mode: Option<EnforcementMode>,
) -> Option<Outcome> {
    let state = row.state?;
    let mut object = serde_json::json!({
        "transformation": id.0.to_text(),
        "state": state,
        "enforced": row.enforced,
        "idempotency_key": idempotency_key,
        "reentered": true,
    });
    match state {
        Lifecycle::Committed => {
            let store = ReceiptStore::in_layout(session.layout());
            let receipt = store.get(id, StoredKind::Commit).ok().flatten();
            let mut json = serde_json::to_value(&receipt).unwrap_or(serde_json::Value::Null);
            if let Some(map) = json.as_object_mut() {
                map.insert("transformation".into(), id.0.to_text().into());
                map.insert("state".into(), to_json(&state));
                map.insert("enforced".into(), row.enforced.into());
                map.insert("idempotency_key".into(), idempotency_key.into());
                map.insert("reentered".into(), true.into());
                return Some(Outcome::ok(json));
            }
            // The receipt store was emptied. The commit still happened — Σ says so — and saying
            // "it did not" (sem: SEM-gx-cli-281) because a derived-looking directory is missing would be a worse lie than
            // a `null` beside a true state. `.gx/receipts/` is `Nature::Source` and `Layout::recover`
            // answers `Lost` for it (req/56 §2).
            if let Some(map) = object.as_object_mut() {
                map.insert("receipt".into(), serde_json::Value::Null);
            }
            Some(Outcome::ok(object))
        }
        Lifecycle::Denied
            if !matches!(
                mode.unwrap_or_else(|| session.read().mode()),
                EnforcementMode::RecordOnly
            ) =>
        {
            Some(Outcome::refused(object, DENIED))
        }
        Lifecycle::Escalated => Some(Outcome::refused(object, ESCALATED)),
        Lifecycle::Aborted(reason) => {
            let code = match reason {
                AbortReason::PreconditionChanged => PRECONDITION_CHANGED,
                AbortReason::ApplyFailed => APPLY_FAILED,
                _ => ERROR,
            };
            if let Some(map) = object.as_object_mut() {
                map.insert("aborted".into(), true.into());
                map.insert("reason".into(), to_json(&reason));
                // 🔴 **`req/329` M-01 (`req/38` §233 ruling 2)** — the roll-back account, on the
                // re-entrant road too.
                //
                // This arm used to build `aborted` and `reason` and stop, so the same question
                // asked twice answered `detail="NotAttempted"` and then `detail=null`, with the
                // same verb, the same exit code and the same `reason`. 44 §1.2's contract is that
                // "retrying the same execution is naturally idempotent", and a script that
                // branches on `detail` could not tell *not attempted* from *succeeded* from
                // *failed* the second time it asked.
                //
                // Nothing is re-derived here. The value is written into the journal's `Aborted`
                // record, `Engine::rollback` reads it back off the shadow row, and the
                // **non-terminal** abort road forty lines up this same file already asks for it.
                // The twenty-sixth audit measured that Σ still holds it after the writing process
                // exited (`A26_SIGMA_HOLDS aborted_records=1 carries_not_attempted=true`), so what
                // was missing was the asking.
                //
                // The cause keeps its honest `None`: it is not a component of Σ (see
                // `NotAttemptedBecause`), so a row this process did not abort has none to give, and
                // `wrap::apply_failed_clause` has an arm for exactly that. `null` here is a fact
                // about where this answer came from; an **absent member** is not, which is why the
                // member is written on every road that answers about an abort.
                let engine = session.read();
                map.insert("detail".into(), to_json(&engine.rollback(id)));
                map.insert(
                    "not_attempted_because".into(),
                    to_json(
                        &engine
                            .rollback_not_attempted_because(id)
                            .map(|because| because.kind()),
                    ),
                );
            }
            Some(Outcome::refused(object, code))
        }
        Lifecycle::Superseded => Some(Outcome::refused(object, ERROR)),
        _ => None,
    }
}

/// Read `--evidence <FILE>...` — 42 §3.7 values, one per line (44 §1.2: "additionally fed in as JSONL"; sem: SEM-gx-cli-282).
///
/// Empty lines are skipped and a line that will not decode is [`Error::Malformed`] naming the file
/// and the line: "if omitted, only gx-gate's built-in InvariantCheck/Cedar evaluation runs" (sem: SEM-gx-cli-283) is a different input from "a
/// collector file that was half readable", and `InjectedEvidence::none` already means the first.
///
/// # Errors
/// [`Error::Io`] if a file cannot be read, [`Error::Malformed`] for a line that is not an `Evidence`.
pub fn read_evidence(paths: &[PathBuf]) -> Result<Vec<gx_witness::Evidence>> {
    let mut out = Vec::new();
    for path in paths {
        let raw = std::fs::read_to_string(path).map_err(crate::io("read", path))?;
        for (n, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let item = serde_json::from_str(line).map_err(|detail| Error::Malformed {
                what: "evidence line",
                path: format!("{}:{}", path.display(), n + 1),
                detail: detail.to_string(),
            })?;
            out.push(item);
        }
    }
    Ok(out)
}

/// 🔴 42 §3.8's `AdmitProof` as JSON, through its **accessors** (**M6H3-2**).
///
/// The type has no `Serialize` derive on purpose — gx-gate's own note says so: "Deriving
/// `Deserialize` in particular would open a second door into the struct that skips
/// `AdmitProof::new` and therefore skips E-M3-9" (sem: SEM-gx-cli-284), E-M3-9 being the canonical ordering the
/// constructor imposes. A `Deserialize` is a road **into** the type and this is a road out, so the
/// five public accessors are read and the object is built from them.
///
/// The five field names are 42 §3.8's own, which is what `AdmitProofView` does for the digest and
/// what makes a reader of the JSON and a reader of the specification see one vocabulary.
fn admit_proof_json(proof: Option<&gx_gate::AdmitProof>) -> serde_json::Value {
    let Some(proof) = proof else {
        return serde_json::Value::Null;
    };
    serde_json::json!({
        "policy_decisions": proof.policy_decisions(),
        "invariant_results": proof.invariant_results(),
        "evidence_digests": proof
            .evidence_digests()
            .iter()
            .map(gx_core::Cid::to_text)
            .collect::<Vec<_>>(),
        "composed_from": proof
            .composed_from()
            .iter()
            .map(|t| t.0.to_text())
            .collect::<Vec<_>>(),
        "proof_ref": proof.proof_ref(),
    })
}

/// Serialise a value that has no business failing to serialise, and say `null` if it did.
///
/// Every caller below hands this an owned enum or an `Option` of one. A `Result` in the signature
/// would put a refusal in `gx commit`'s success path for a case that cannot arise, and an `expect`
/// would make it a panic; 41 §6 counts a panic as a bug.
fn to_json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}
