// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx undo` / `gx cancel` / `gx escalation approve|reject` — 44 §1.2's back half.
//!
//! Three verbs and three of 43's transitions: T-12's first half ([`undo`]), T-7 ([`cancel`]) and
//! T-5/T-5b ([`escalation`]). [`crate::pipeline`] drives a transformation forward; this module is
//! what an operator reaches for when the answer is "take it back", "stop it" or "I decide" (sem: SEM-gx-cli-330).
//!
//! # 🔴 `gx cancel` no longer accepts a `Draft` — **E-M6-1**
//!
//! 44 L101 lists `{Draft, Candidate, Verifying, Admitted, Canonicalized, Escalated}` and req/38 §47
//! M6-03 adopted (c) removed the first (sem: SEM-gx-cli-331):
//!
//! > an erratum removing `Draft` from `gx cancel`'s from-set in 44 L101 (this § is canon; 44
//! > itself is not rewritten), plus the same re-reading of AC-073's Given (sem: SEM-gx-cli-332)
//!
//! and it withdrew §45 M5H8-2 adopted (b) — "M6's id-resolution takes charge of Draft" (sem: SEM-gx-cli-333) — with the frame
//! correction written out: id-resolution solves the problem of *pointing* at something and a draft's
//! problem is that there is **no seat**. A `Draft` has no `TransformationId` (43 T-1, E-M5-3),
//! `Aborted` is keyed on one, and M5-17 adopted (b) keeps the draft phase in the journal alone (sem: SEM-gx-cli-334). So
//! [`cancel`] refuses a draft **by name**, tells the operator what discarding one actually is (the
//! file under `.gx/drafts/`), and does not invent a verb for it — whether `gx draft discard` should
//! exist is raised as **M6H4-2** rather than decided by the hand that noticed.
//!
//! # 🔴 `gx undo` returns 44 §1.4's **2** — M6-25's body
//!
//! 44 §1.2 gives `undo` the list 0/1/3/5/6 and no seat for "refused (denied)" (sem: SEM-gx-cli-335), while AC-040's second
//! case is exactly that: "in the case where the invariant/policy corresponding to T_u is
//! deliberately set to Deny, T_u fails to reach Committed and stays Denied", implemented in the engine since M5. req/38 §47 M6-25 adopted (a)+(c)
//! reads §1.2's list as an **excerpt** and §1.4's common table as the answer, so a denied undo exits
//! **2** and not 1. Folding it into 1 would put "could not run" and "the gate refused you" (sem: SEM-gx-cli-336) under
//! one face, which is M4H4-2's standing refusal.
//!
//! # 🔴 `gx escalation` takes a `<TICKET_ID>` and nothing stored it — **M6-04 / M6H3-10**
//!
//! 44 §1.2's trigger is `gx escalation approve <TICKET_ID>` and 43 T-4c declares the ticket 1:1 with
//! a `TransformationId`, but the mapping ran one way only and the journal records the verdict rather
//! than the ticket (42 §3.13). req/38 §50 sent hand 4 to **measure** whether the ticket could be
//! rebuilt from a row before anyone grew the journal's vocabulary, and it can:
//! `gx_gate::escalation_ticket` reads nothing but the id. So [`gx_engine::Engine::transformation_of_ticket`]
//! resolves the name out of Σ, 42 §3.13 is untouched, and this module accepts **either** spelling —
//! M6-04 adopted (c), the natural extension of 44 §0's id-resolution rule, which also removes the asymmetry (sem: SEM-gx-cli-337)
//! between the CLI's `<TICKET_ID>` and 44 §2.2's `{id}`.

use gx_core::{Actor, Timestamp, TransformationId, VerdictKind};
use gx_engine::{HumanRuling, Lifecycle, UndoWitness, Unobservable, WitnessMissing};
use gx_gate::TicketId;
use gx_witness::KeyPair;

use crate::exit::{
    Outcome, APPLY_FAILED, DENIED, ERROR, ESCALATED, NOT_FOUND, OK, PRECONDITION_CHANGED,
};
use crate::keys::KeyStore;
use crate::session::Session;
use crate::{Error, Result};

/// 🔴 `gx undo <TID>` (44 §1.2) — T-12's first half, then the whole ordinary pipeline.
///
/// > take the target's committed `EscrowedInverse` (42 §3.12, with `status=Available`) as the
/// > `delta` of a new `Transformation` and run submit through commit in one batch (P-5: undo is
/// > committing a new inverse transformation) (sem: SEM-gx-cli-338)
///
/// `Engine::undo` builds the `Candidate` and, in its own words, "does not undo anything": 43 §5-2
/// makes T_u walk `Draft→Candidate→Verifying→…→Committed` on its own feet, "even an undo is not
/// exempt from verification" (sem: SEM-gx-cli-339). So this function drives the same four steps `gx submit`..`gx commit` drive, in one
/// process because 44 says "run as one batch", and the supersede edge is drawn by the commit rather than by
/// the undo (T-12, `Engine::supersede_after_commit`).
///
/// # 🔴 The signing key comes from the original's actor, and the reason is worth a line
///
/// `Engine::undo` mints T_u's `Intent` with **T_o's context and actor** (P-5: the undo is a change
/// by whoever's change is being taken back), so [`Session::signing_key`] resolves to the same key
/// the original receipt was signed under. 44 §1.2's `--actor-key` on this verb therefore selects
/// nothing today, and is refused rather than ignored (M4H5-5) — raised as **M6H4-3**.
///
/// # 🔴 The settle pre-flight (`req/38` §98 ruling 2 — DR-2 option A, `req/160` §2-1) (sem: SEM-gx-cli-340)
///
/// Between the rehydrate and the launch, this verb polls [`gx_engine::Engine::live_digest`] until
/// the world reports the digest T_o's **own commit receipt** recorded as its
/// `postcondition_fingerprint` — the signed observation T_o made right after its apply (42 §3.10).
/// `req/153` §4.1 measured why: a real SaaS substrate (github-mcp-server v1.9.0) is not
/// read-after-write consistent, so an undo fired right after the forward commit reads a stale
/// world — the server's own validation then refuses (`Aborted(ApplyFailed)`, fail-safe) **and**
/// the undo's T-2 snapshot escrows a stale body (the mis-escrow window `req/160` §2-0 names). The
/// pre-flight closes both by waiting, boundedly, for the world to catch up with what T_o attested.
///
/// What it deliberately is **not**:
/// * not a judge — the poll's answer is never used to refuse anything. On timeout the undo fires
///   **once** anyway and T-10a's CAS / the server's own validation give the true answer, so the
///   result vocabulary is unchanged (a genuinely stale world is `Aborted(ApplyFailed)` exactly as
///   before, and "the world moved on legitimately" (sem: SEM-gx-cli-341) is bounded by the deadline instead of waited
///   on forever);
/// * not in the engine — the sleep and the deadline are this function's (41 §6 keeps wall clocks
///   out of the engine; `Engine::live_digest` is a pure read);
/// * not load-bearing for safety — a receipt that is missing, unreadable, or carries no
///   postcondition (nothing was applied) skips the pre-flight and fires as every version before
///   this one did, said out loud on stderr (fail-safe: skip, never block).
///
/// Poll count and elapsed time are reported on stderr (`req/38` §98: the poll distribution rides
/// along so the 120s default can be re-derived from measurements).
///
/// # 🔴 **DR-43-1, adopted (a)** — the first bullet above is now half true (`req/38` §132 ruling 2)
///
/// "not a judge" still describes the **pre-flight**, and it is why the wording above is kept rather
/// than rewritten: [`settle_preflight`] refuses nothing, and the poll's answer still decides
/// nothing. What changed is that it now returns [`UndoWitness`] — what it learned — and
/// [`gx_engine::Engine::undo`] compares that against the world before it mints anything. So the
/// sentence "on timeout the undo fires once anyway" is **false from this ruling onward**: a world
/// that never matched what `T_o` attested is `PRECONDITION_CHANGED` (exit 3, 44 §1.4's own number
/// for a CAS that failed), and the escrowed inverse is not applied over somebody else's change.
///
/// `req/182` H-15 is what forced it — "RC 0 · Committed · file back at `AAA`", with the third
/// party's `CCC` gone and nothing saying so — and `crates/gx-cli/tests/undo_cas_e2e.rs` is that
/// measurement with its answer inverted. The third bullet ("not load-bearing for safety") is
/// unchanged for the cases it names: a missing, unreadable or postcondition-less receipt is
/// [`Unobservable`] and still fires, because DR-46-7 (`req/38` §123 ruling 1) rules that an
/// unobservable face is declared rather than refused.
///
/// # Errors
/// [`Error::NotFound`] (44's 6) for an unknown transformation or one with no escrowed inverse,
/// [`Error::Usage`] for `--actor-key`. Anything the engine refuses, unchanged — 44 §1.2's "1 =
/// unable to execute because `InverseStatus` is `Unavailable`/`Expired`/`Consumed`" (sem: SEM-gx-cli-342) arrives that way.
pub fn undo(
    session: &mut Session,
    id: &TransformationId,
    idempotency_key: Option<&str>,
    rng_seed: u64,
    at: Timestamp,
    settle_secs: u64,
) -> Result<Outcome> {
    // 🔴 **DR-43-4's entry sweep** (`req/38` §148 ruling 1(iv)) — see [`Session::sweep`].
    session.sweep(at)?;
    // The original has to be in the table before T-12 can read its escrow, and after a commit the
    // row is exactly what a fresh process does not have (M6H3-1). `Session::resume` is refused for
    // a committed row — a re-plan reads a substrate the commit has already moved — so the road in
    // is the engine's own recovery of the escrow, ~~which `Engine::open` does rebuild from Σ~~.
    // L-02 (`req/182` §1-3, `req/189`): `Engine::open` rebuilds **nothing** of the table or the
    // escrow (M5H3-5: `open` leaves them empty); the rebuild is `Engine::rehydrate_committed`,
    // called on the next line, and it reads Σ plus the escrowed blob. The struck words are kept
    // because a reader who trusted them would look for the escrow in the wrong function.
    session.rehydrate_committed(id)?;

    // 🔴 **DR-43-1, adopted (a)** (`req/38` §132 ruling 2) — the pre-flight is now the gate's
    // **evidence**, and the engine is the judge. Until this ruling the function below polled, wrote
    // a line on stderr and refused nothing (`req/182` H-15: "settle pre-flight is not used in the
    // judgement"), so a world a third party had moved was overwritten with `RC 0`. It still refuses
    // nothing — what changed is that it now *hands back what it learned*, and `Engine::undo`
    // compares that against the world before it mints anything. The sleep, the deadline and the
    // distribution line stay here, on the CLI side of 41 §6's boundary.
    // 🔴 43 §7's recovery, wired — `pipeline::verify` carries the whole reasoning. Here the key is
    // T_o's actor's, which 43 §5-1 makes T_u's actor too, so the undo's own receipts and any
    // resumed row are signed by the same hand.
    //
    // 🔴 **R3 / `req/222` H-02** — loaded **before** the pre-flight, because the pre-flight now
    // verifies a signature and this is the key it verifies against. ~~Same key, same reasoning: the
    // commit receipt this project holds for `T_o` was signed by `T_o`'s actor, so that is the hand
    // whose signature makes the stored postcondition evidence rather than a file name.~~
    //
    // 🔴 **R4 / `req/225` H-02** — the struck sentence is a **premise**, and it is false in the
    // deployment 44 §1.2 and E-M6-7 describe as ordinary. `gx serve --signing-key <ENGINE>` signs
    // every commit receipt with the *engine's* key, not with `T_o`'s actor's, so on that project
    // this pre-flight verified an authentic engine receipt against an unrelated public key and
    // called it forged: measured at **exit 3 for every committed row**, permanently, with the
    // sentence "does not verify under this project's key … so it is not evidence of anything".
    // The words are kept for no-delete: a reader who trusts them will look for the bug in the
    // archive rather than in the key selection.
    //
    // The key below is still the actor's, because what it is *for* is unchanged — it signs `T_u`'s
    // own receipts and any commit `recover` finishes (43 §5-1). What moved out of it is the
    // verification, which now asks the receipt which key signed it.
    // 🔴 **R9 / `req/236` M-01** — a row whose escrowed body is not readable is refused **by name**,
    // and by the same name the HTTP face uses.
    //
    // R8 taught `inverse_status` to answer `BodyMissing`, and 43 §7.10 recorded that "the CLI settle
    // pre-flight also refuses by name". It did not: the pre-flight's arm returned
    // `Unobservable(LaunchAlreadyDecided)`, whose own note in this file says the word means "fire
    // anyway" — so `gx undo` walked into `Engine::undo`, reached the blob store, and exited 1 with
    // `INTERNAL` and "no blob named gx1:…", while the same project over HTTP answered
    // `409 INVERSE_UNAVAILABLE`. Two faces, one state, two answers, and the CLI's was 44 §2.3's word
    // for "not classifiable" over a state that is entirely classifiable.
    //
    // Only `BodyMissing` is taken here. `Pending`, `Consumed`, `Expired` and `Unavailable` keep the
    // road they have had since R3 — `Engine::undo` refuses each of them by its own name, and
    // rerouting them would change answers `undo_cmd.rs` and `undo_cas_e2e.rs` pin.
    if matches!(
        session.read().inverse_status(id),
        Some(gx_engine::InverseStatus::BodyMissing)
    ) {
        return Err(Error::InverseUnavailable {
            id: id.0.to_text(),
            status: "BodyMissing",
        });
    }
    let recovery_key = session.signing_key(id)?;
    let witness = settle_preflight(session, id, settle_secs)?;

    session.recover(at, &recovery_key, "gx undo")?;

    // 🔴 **`req/182` H-16 closed** (`req/38` §148 ruling 1(iii), lane R2). `Engine::undo` mints
    // `T_u`'s intent in memory, 42 §3.13 records only its id, and `.gx/drafts/` had no entry for
    // it — so `gx undo <T_u>` reached `rehydrate_committed`, found no draft, and answered 44
    // §1.4's 6 (`req/216` §3; `undo_cas_e2e::an_undo_of_an_undo_is_refused_by_name` pinned that
    // number and said this is where it would change). Read **before** the call, because the escrow
    // row it is computed from is `Consumed` once the undo commits; written **after** it, because a
    // refused undo must leave nothing behind and a draft is something.
    let undo_draft = session.read().undo_intent(id)?;
    let (intent_id, undoing) = session.engine().undo(id, &witness, rng_seed, at)?;
    session.remember(intent_id, undoing);
    if let Some(intent) = &undo_draft {
        // Best effort for `Session::remember`'s reason one row along: the undo has happened, the
        // journal and the ledger say so, and a directory that would not take the body is not a
        // reason to report a commit that occurred as a failure. What it costs is stated where a
        // reader meets it — `Session::rehydrate_committed`'s refusal names the missing draft.
        let _ = session.drafts().put(&intent_id, intent);
    }

    // 43 §5-2. The same three calls `gx verify` and `gx commit` make, in one process because 44 §1.2
    // says "run submit through commit as one batch" (sem: SEM-gx-cli-343) — and with the same meanings, which is why a denied undo
    // is reported as a denial rather than as a failure to undo.
    let key = session.signing_key(&undoing)?;
    let verified = session.engine().verify(&undoing, at, &key, None)?;
    if !matches!(verified, Lifecycle::Admitted) {
        return Ok(halted(session, id, &undoing));
    }
    session.engine().canonicalize(&undoing, at, None)?;
    let state = session.engine().commit(&undoing, at, &key)?;
    if !matches!(state, Lifecycle::Committed) {
        return Ok(halted(session, id, &undoing));
    }

    let derived = idempotency_key.map_or_else(
        || format!("gx-undo:{}", undoing.0.to_text()),
        std::string::ToString::to_string,
    );
    let stored = match session.read().receipt(&undoing) {
        Some(receipt) => Some(
            crate::receipt::ReceiptStore::in_layout(session.layout()).put(
                &undoing,
                crate::receipt::StoredKind::Commit,
                receipt,
            )?,
        ),
        None => None,
    };
    let engine = session.read();
    // 44 §1.2: "stdout: the new `Transformation`'s `Receipt`" (sem: SEM-gx-cli-344).
    let mut json =
        serde_json::to_value(engine.receipt(&undoing)).unwrap_or(serde_json::Value::Null);
    if let Some(map) = json.as_object_mut() {
        map.insert("transformation".into(), undoing.0.to_text().into());
        map.insert("undone".into(), id.0.to_text().into());
        map.insert("state".into(), to_json(&state));
        // 43 T-12: "append `superseded_by = T_u.id` to `T_o`'s metadata" (sem: SEM-gx-cli-345). The edge is the engine's and
        // is printed because it is the fact an operator ran the command for.
        map.insert("superseded_state".into(), to_json(&engine.state(id)));
        map.insert("idempotency_key".into(), derived.into());
        map.insert(
            "stored_at".into(),
            stored.map_or(serde_json::Value::Null, |p| {
                p.display().to_string().replace('\\', "/").into()
            }),
        );
    }
    Ok(Outcome::ok(json))
}

/// How long the pre-flight waits between two polls.
///
/// 2s where the E2E harness gate polled at 5s (`tools/a2_invert_e2e.sh`), because the product
/// surface pays the interval on **every** real-SaaS undo and the measured lag (`req/153` §4.1:
/// refused at ~1s, succeeded at ~2min, settled in 1-2 harness polls thereafter) is coarse enough
/// that a finer first read costs nothing and answers the common case sooner. In-process substrates
/// (fs/git/postgres, every existing test and demo) match on poll 1 and never sleep at all.
const SETTLE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

/// 🔴 **R4 / `req/225` H-02** — the verifying key a stored receipt names, out of req/56 §3's store.
///
/// `Err` carries the id that could not be resolved, so the caller can name it: the two failures a
/// caller has to tell apart are "this receipt is signed by a key I do not have" and "this receipt's
/// payload is rubbish", and the second is [`gx_engine::WitnessMissing::Unreadable`] one branch up.
///
/// The **whole** store rather than a declared allow-list, and the reason is key rotation. E-M6-7's
/// `.gx/config.toml` records the key `gx serve` signs with **today**; a receipt issued before a
/// `gx key rotate` names yesterday's, and a check that consulted only the recorded id would refuse
/// every undo of every commit made before the rotation. `req/225` H-02 names that as the same bug
/// seen from the other side. Revocation is a separate question with a separate road
/// (`gx_witness::verify_offline_consulting`, ASM-45-2's "at the verifier's discretion") and is not
/// folded in here.
fn resolve_receipt_key(receipt: &gx_witness::Receipt) -> std::result::Result<KeyPair, String> {
    let named = match receipt.payload() {
        Ok(payload) => payload.key_id,
        // An undecodable payload has no key id to look up. The caller's next branch answers this
        // case by name (`Unreadable`), so the id reported here is one that cannot match anything.
        Err(_) => return Err("<the payload would not decode>".to_string()),
    };
    let store = KeyStore::user_default().map_err(|_| named.to_string())?;
    store.load(&named).map_err(|_| named.to_string())
}

/// 🔴 The settle pre-flight of [`undo`] — poll the live world against T_o's own signed observation.
///
/// The expected value is the **stored commit receipt's** `postcondition_fingerprint`
/// ([`crate::receipt::ReceiptStore`], `StoredKind::Commit`): journal records carry no
/// postcondition (M5H5-3), the receipt does, and it is signed — "one's own signed observation is
/// the value compared against" (sem: SEM-gx-cli-346)
/// (`req/38` §98 ruling 2). The probe is [`gx_engine::Engine::live_digest`]; the sleep, the
/// deadline and the report are here, on the CLI side of 41 §6's line.
///
/// Every exit of this function is a **return to the launch** — it refuses nothing:
/// * `--settle 0` — disabled by the operator, one stderr line;
/// * no stored commit receipt / no postcondition / unreadable receipt — nothing to compare
///   against, pre-flight skipped, pre-existing behaviour (fail-safe side, said on stderr);
/// * probe error — a world that cannot be read is not a world polling settles; fire once and let
///   the pipeline's own error surfaces answer (behaviour difference with the past: zero);
/// * match — the world reports what T_o attested; fire now;
/// * timeout — fire once anyway. "fire once, as-is, after timeout" (sem: SEM-gx-cli-347): the result vocabulary must not
///   grow, so a genuinely stale world still lands on `Aborted(ApplyFailed)` exactly as it did
///   before this function existed, and a world a third party legitimately moved is bounded by
///   the deadline rather than waited on forever (the poll never matches there — the CAS is the
///   judge, not the poll).
///
/// # 🔴 What **DR-43-1, adopted (a)** changes here, and what it deliberately does not
///
/// The list above is kept verbatim because it is the true record of what this function did between
/// `req/38` §98 ruling 2 and §132 ruling 2, and one line of it has since become false: on
/// **timeout** the launch is no longer "fire once anyway". This function still refuses nothing — it
/// returns [`UndoWitness`] and [`gx_engine::Engine::undo`] is what refuses — but the witness it
/// hands back after a timeout is `Attested`, so the engine compares and answers
/// `PRECONDITION_CHANGED` (exit 3) instead of applying the inverse over a world that moved. The
/// other four exits are unchanged in behaviour and in wording, and the three "nothing to compare"
/// ones now name *which* absence they are ([`Unobservable`]) rather than only saying so on stderr.
///
/// `--settle 0` is **not** an override of the ruling. It disables **polling**, which is all it ever
/// meant; the receipt is still read and the witness is still attested, because a flag that turned
/// the CAS off would be the ruling made into a checkbox (`req/213` §8-7's standing refusal).
fn settle_preflight(
    session: &mut Session,
    id: &TransformationId,
    settle_secs: u64,
) -> Result<UndoWitness> {
    // 🔴 **R8 / `req/234` H-03** — the evidence is read under the lock, the **waiting is not**.
    //
    // `req/234` H-03 measured the old shape: this whole function ran inside `Session::open`'s
    // `.gx/LOCK`, so an undo whose world a third party had touched polled for the entire `--settle`
    // budget (**120 seconds by default**) with the project's writer lock in its hand. During those
    // 120 s `POST /v1/candidates`, `GET /v1/ledger/checkpoint`, every CLI write **and `gx repair`
    // itself** answered `BUSY`, while HTTP's own `/undo` — which does not poll at all — answered
    // the same input instantly. One ruling, two surfaces, two behaviours.
    //
    // The order below is the repair, and it is 43 §5.2's own principle applied to the case DR-43-1
    // added: "a request whose answer is already fixed before it is fired is not polled".
    //
    //   1. read the evidence and probe **once**, holding the lock — a read-after-write consistent
    //      substrate (fs, git, postgres: every existing test and demo, 44 §1.2 v0.3-a) matches here
    //      and nothing else in this comment ever happens;
    //   2. only if it did not match, **release** the lock and wait. The poll is a read of somebody
    //      else's world and needs no exclusion on `.gx/`;
    //   3. take the lock again — which catches up on everything that was written while we waited —
    //      and probe once more under it. That last probe is the one the engine's CAS is judged
    //      against, so a world that moved *during* the wait is caught rather than assumed away.
    //
    // 🔴 Self-kill, answered rather than argued: releasing the lock means another `gx` can take it
    // and this undo can come back to a `BUSY` of its own. That is honest — the alternative is the
    // 120-second outage this repair exists for — and it is the same per-operation exclusion
    // `gx wrap` has used since DR-43-2 (`Session::release_writer_lock` is the function it calls).
    // The window it opens is not a correctness hole: everything decided before the release is
    // re-decided after it (step 3), and `Engine::undo` still performs DR-43-1's CAS.
    let expected = match settle_evidence(session, id) {
        Ok(expected) => expected,
        Err(witness) => return Ok(witness),
    };
    if settle_secs == 0 {
        crate::note!("gx undo settle: disabled (--settle 0); firing without a pre-flight");
        return Ok(UndoWitness::Attested(expected));
    }
    match session.read().live_digest(id) {
        Ok(live) if live.0 == expected.0 => {
            crate::note!(
                "gx undo settle: polls=1 elapsed_ms=0 result=matched (--settle {settle_secs}; the                  first probe answered under the lock, so nothing was held and nothing waited)"
            );
            return Ok(UndoWitness::Attested(expected));
        }
        Ok(_) => {}
        Err(e) => {
            crate::note!(
                "gx undo settle: polls=1 elapsed_ms=0 result=abandoned (the probe could not read                  the world: {e}); firing as before"
            );
            return Ok(UndoWitness::Unobservable(Unobservable::NoPostcondition));
        }
    }
    // The world does not match. Everything from here is waiting, and none of it holds `.gx/LOCK`.
    session.release_writer_lock();
    let outcome = settle_poll(session, id, &expected, settle_secs);
    if let Err(e) = session.hold_writer_lock() {
        // 🔴 **R9 / `req/236` M-02** — the reason is the **lock**, and it says so.
        //
        // R8 split the wait out of the critical section (`req/234` H-03) and this is the cost it
        // declared: another `gx` can take the lock while the undo is waiting. What R8 did not do
        // was give the new cost its own word, so the machine-readable answer was
        // `PRECONDITION_CHANGED` with "the archived commit receipt would not decode" and a remedy
        // telling the operator to restore a file under `.gx/receipts/` — over an archive that was
        // untouched. The word for "another gx process is writing to this project" already exists
        // (`BUSY`, DR-43-2), it is the one whose correct response is a retry, and this is that
        // condition exactly.
        crate::note!(
            "gx undo settle: the writer lock was released for the wait (req/234 H-03) and could              not be taken again: {e}"
        );
        return Err(e);
    }
    // 🔴 Step 3. The witness the engine judges against is a probe taken **under** the lock, so a
    // third party who moved the world while we waited is caught by the CAS rather than by luck.
    match session.read().live_digest(id) {
        Ok(live) if live.0 == expected.0 => crate::note!(
            "gx undo settle: {outcome} — and the world still matched when the lock came back"
        ),
        Ok(_) => crate::note!(
            "gx undo settle: {outcome} — the world does not match what T_o attested, so DR-43-1's              CAS is what answers now"
        ),
        Err(e) => {
            crate::note!("gx undo settle: {outcome} — the world would not read afterwards ({e})");
            return Ok(UndoWitness::Unobservable(Unobservable::NoPostcondition));
        }
    }
    Ok(UndoWitness::Attested(expected))
}

/// 🔴 **R8 / `req/234` H-03** — the waiting half, run with `.gx/LOCK` **released**.
///
/// Returns the line the caller prints. It refuses nothing and decides nothing — the CAS in
/// `Engine::undo` is the judge, and the probe under the lock that follows this call is what the
/// judgement is made on.
fn settle_poll(
    session: &Session,
    id: &TransformationId,
    expected: &gx_core::FingerprintBytes,
    settle_secs: u64,
) -> String {
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_secs(settle_secs);
    let mut polls: u32 = 1;
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            return format!(
                "polls={polls} elapsed_ms={} result=timeout (--settle {settle_secs}; the wait was                  outside the project lock, so nothing else was blocked by it)",
                started.elapsed().as_millis()
            );
        }
        std::thread::sleep(SETTLE_POLL_INTERVAL.min(deadline - now));
        polls += 1;
        match session.read().live_digest(id) {
            Ok(live) if live.0 == expected.0 => {
                return format!(
                    "polls={polls} elapsed_ms={} result=matched (--settle {settle_secs})",
                    started.elapsed().as_millis()
                )
            }
            Ok(_) => {}
            Err(e) => {
                return format!(
                    "polls={polls} elapsed_ms={} result=abandoned (the probe could not read the                      world: {e})",
                    started.elapsed().as_millis()
                )
            }
        }
    }
}

/// 🔴 The evidence half of the pre-flight — every read that decides whether there is anything to
/// compare, and none of the waiting (**R8**, split out for `req/234` H-03).
///
/// `Err` is the whole answer: the four refusals of `req/38` §160 ruling 2 and the two skips.
/// `Ok` is T_o's own signed `postcondition_fingerprint`, which is what a poll and the engine's CAS
/// both compare against.
fn settle_evidence(
    session: &Session,
    id: &TransformationId,
) -> std::result::Result<gx_core::FingerprintBytes, UndoWitness> {
    // 🔴 A launch the engine will refuse regardless is not polled: a superseded original (its
    // world is *already* back, so the live digest can never equal T_o's postcondition again) or
    // a consumed/unavailable/pending inverse would otherwise sit out the whole budget in front
    // of an answer that was fixed before the first poll. Measured, not imagined: the second-undo
    // case of `undo_cmd.rs` waited the full 120s before its `Consumed` refusal until this guard.
    // (`matches!` rather than `!=`: the Rule 1 (iii) scanner in `authority_boundary.rs` reads
    // "`=` before `Lifecycle::`" (sem: SEM-gx-cli-348) as the CLI minting a state, and the probe is right to be coarse
    // — `halted` above took the same detour for the same scanner. The state is *read* here,
    // engine-side, and compared; nothing is written.)
    if !matches!(session.read().state(id), Some(Lifecycle::Committed)) {
        crate::note!(
            "gx undo settle: skipped (the original is not Committed — polling cannot change what \
             the launch will answer about that); firing as before"
        );
        return Err(UndoWitness::Unobservable(
            Unobservable::LaunchAlreadyDecided,
        ));
    }
    if !matches!(
        session.read().inverse_status(id),
        Some(gx_engine::InverseStatus::Available)
    ) {
        crate::note!(
            "gx undo settle: skipped (the escrowed inverse is not Available — the launch's \
             refusal is already fixed, and waiting would not unfix it); firing as before"
        );
        return Err(UndoWitness::Unobservable(
            Unobservable::LaunchAlreadyDecided,
        ));
    }
    // 🔴 **R3 / `req/222` H-01, H-02** — the four questions, and every "no" is a refusal.
    //
    // The three `return`s below used to be `Unobservable`, which meant "fire anyway", and the
    // stderr line said `firing as before`. `req/222` measured both halves of what that bought: on
    // the HTTP face, deleting one file under `.gx/receipts/` turned a `409` into a `200` that
    // destroyed a third party's write (H-01, 3/3); on both faces, nothing verified the receipt's
    // DSSE signature or checked that its payload was about the transformation being undone, so a
    // receipt copied from another id was accepted as this one's evidence (H-02). `req/38` §160
    // ruling 2 rules the road fail-closed: no evidence, no undo. The sentences below are what the
    // operator now reads, and they name the road back rather than only the wall.
    let receipt = match crate::receipt::ReceiptStore::in_layout(session.layout())
        .get(id, crate::receipt::StoredKind::Commit)
    {
        Ok(Some(receipt)) => receipt,
        Ok(None) => {
            crate::note!(
                "gx undo refused: there is no stored commit receipt for {} under `.gx/receipts/`, \
                 so what this change left behind cannot be compared with what is there now \
                 (req/38 §160, DR-43-1). Restore the receipt, or accept that gx cannot take this \
                 change back",
                id.0.to_text()
            );
            return Err(UndoWitness::Missing(WitnessMissing::NoReceipt));
        }
        Err(e) => {
            crate::note!("gx undo refused: the receipt store would not answer: {e}");
            return Err(UndoWitness::Missing(WitnessMissing::Unreadable));
        }
    };
    // 🔴 **R4 / `req/225` H-02** — the key comes from the **receipt**, not from the row.
    //
    // 42 §3.10 puts `key_id` inside the signed payload precisely so that a document can say which
    // hand ought to have signed it, and `verify_offline` refuses a receipt whose `key_id` and
    // verifying key disagree — so naming the key here cannot become a way of accepting any
    // signature that happens to check out. What it does is stop this face from insisting on a
    // hand that never touched the document.
    //
    // The trust anchor is req/56 §3's key store (`~/.gx/keys/`), which is outside `.gx/` and 0600:
    // a third party who can write `.gx/receipts/` cannot put a key in it, so a forged receipt
    // still has to be signed by a key this operator holds. A receipt naming a key that is *not*
    // in the store is refused by name rather than reported as a bad signature — see
    // `WitnessMissing::UnknownKey`.
    let verifying = match resolve_receipt_key(&receipt) {
        Ok(key) => key,
        Err(named) => {
            crate::note!(
                "gx undo refused: the stored commit receipt for {} is signed under key {named:?}, \
                 and req/56 §3's key store (`~/.gx/keys/`) holds no key of that id — so its \
                 signature cannot be checked, and an unchecked document is not evidence \
                 (req/225 H-02). `gx key list` shows what is there",
                id.0.to_text()
            );
            return Err(UndoWitness::Missing(WitnessMissing::UnknownKey));
        }
    };
    if let Err(e) = gx_witness::verify_offline(&receipt, &verifying.verifying(), None) {
        crate::note!(
            "gx undo refused: the stored commit receipt for {} does not verify under the key it \
             names, {:?} ({e}), so it is not evidence of anything (req/222 H-02, req/225 H-02)",
            id.0.to_text(),
            verifying.key_id()
        );
        return Err(UndoWitness::Missing(WitnessMissing::Unsigned));
    }
    let expected = match receipt.payload() {
        Ok(payload) if payload.transformation != *id => {
            crate::note!(
                "gx undo refused: the receipt stored under {} says it is about {} — a receipt does \
                 not get to choose its own file name (req/222 H-02)",
                id.0.to_text(),
                payload.transformation.0.to_text()
            );
            return Err(UndoWitness::Missing(WitnessMissing::WrongSubject));
        }
        Ok(payload) => match payload.postcondition_fingerprint {
            Some(bytes) => bytes,
            None => {
                // The one absence that is still a declaration, and it is the substrate's rather
                // than the evidence's: an authentic commit receipt saying nothing was observed is
                // `req/38` §123 ruling 1's tools-only face (DR-46-7).
                crate::note!(
                    "gx undo settle: skipped (the stored commit receipt carries no \
                     postcondition fingerprint, so there is nothing to compare the live \
                     world against); firing as before"
                );
                return Err(UndoWitness::Unobservable(Unobservable::NoPostcondition));
            }
        },
        Err(e) => {
            crate::note!(
                "gx undo refused: the stored commit receipt's payload would not decode: {e}"
            );
            return Err(UndoWitness::Missing(WitnessMissing::Unreadable));
        }
    };

    Ok(expected)
}

/// 🔴 Where an undo stopped, and the status 44 gives that stop — **M6-25's 2 among them**.
///
/// The five 44 §1.2 writes for `undo` are 0/1/3/5/6, and the sixth is the one §1.4 has and §1.2's
/// list does not: a `Denied` T_u is "refused (denied)" (sem: SEM-gx-cli-349) and exits **2**. AC-040's second case is
/// exactly this transformation and the engine has produced it since M5, so the alternative was to
/// answer "unable to execute" (sem: SEM-gx-cli-350) about a gate that ran and said no.
///
/// 🔴 The state is **read from the engine** rather than passed in, and the reason is one Rule 1 (iii)
/// found rather than one this hand chose. `crates/gx-canon/tests/authority_boundary.rs` reads "a
/// declaration whose type mentions `Lifecycle` and ends in a comma" as "the CLI keeps a state
/// table" (sem: SEM-gx-cli-351), and a **parameter list** is exactly that shape once rustfmt puts one argument per line.
/// The probe is right to be coarse (a state table is declared in one line), so the code moved: three
/// arguments fit on one line, and reading the state back from the engine is what every other verb in
/// this crate does anyway. Hand 3 hit the same scanner twice and drew the same conclusion.
fn halted(session: &Session, original: &TransformationId, undoing: &TransformationId) -> Outcome {
    let state = session.read().state(undoing);
    let code = match state {
        // 🔴 M6-25 adopted (a)+(c) (sem: SEM-gx-cli-352). §1.2's list is an excerpt; §1.4's common table is the answer.
        Some(Lifecycle::Denied) => DENIED,
        Some(Lifecycle::Escalated) => ESCALATED,
        Some(Lifecycle::Aborted(gx_core::AbortReason::PreconditionChanged)) => PRECONDITION_CHANGED,
        Some(Lifecycle::Aborted(gx_core::AbortReason::ApplyFailed)) => APPLY_FAILED,
        None => NOT_FOUND,
        _ => ERROR,
    };
    let engine = session.read();
    Outcome::refused(
        serde_json::json!({
            "transformation": undoing.0.to_text(),
            "undone": original.0.to_text(),
            "state": state,
            "kind": engine.verdict(undoing),
            "enforced": engine.enforced(undoing),
            "detail": engine.rollback(undoing),
            // 🔴 **`req/324` §5(d) (`req/38` §231 ruling 5)** — the cause beside the value, on this
            // surface too. `detail` is one word for several facts, and the commit road's twin
            // (`pipeline.rs`) and gx-api's assembler (`rollback_facts`) both carry the cause next to
            // the value so a reader can tell a `NotAttempted` that has a reason from one that does
            // not. An additive member rather than a fourth `Rollback` value: the value is on a
            // record already written and already signed (see [`gx_engine::store::Rollback`]), and
            // nothing here decides anything. `null` when this process did not reach the abort
            // itself, which the proxy has an arm for.
            "not_attempted_because": engine
                .rollback_not_attempted_because(undoing)
                .map(|because| because.kind()),
            // 43 §5-3: the supersede edge fires only when T_u reaches `Committed`. It did not, so
            // the original is still where it was, and saying so is the answer to the question an
            // operator asks next.
            "superseded_state": engine.state(original),
        }),
        code,
    )
}

/// 🔴 `gx cancel <TID>` (44 §1.2) — T-7, with **E-M6-1**'s from-set.
///
/// > transition to `Aborted(OwnerCancelled)` from any of `{Candidate, Verifying, Admitted,
/// > Canonicalized, Escalated}`. Unable to execute once `Committing` is reached (sem: SEM-gx-cli-353)
///
/// (44 L101 writes `Draft` first; req/38 §47 M6-03 adopted (c) removed it; sem: SEM-gx-cli-354. See the module header.)
///
/// # 🔴 The owner-permission guard is not enforced, and 44's `--actor-key` selects nothing
///
/// 43 T-7's guard is "actor holds owner authority (equivalent to `Actor::Human{key}`)" (sem: SEM-gx-cli-355) and v0.1 has no
/// authorization layer (M5H6-4 adopted (a)): `Engine::cancel` takes no actor, the `Aborted` record has no
/// actor field, and nothing in the engine knows who owns a transformation. The engine's own module
/// documentation discloses that; this line is the same disclosure at the surface an operator
/// actually types at, and `--actor-key` is **refused** rather than accepted and dropped.
///
/// # Errors
/// [`Error::NotFound`] (44's 6) for an unknown id, [`Error::Usage`] for a draft (see the module
/// header) and for `--actor-key`. Anything the engine refuses, unchanged — a row past `Committing`
/// arrives as `Error::InvalidState`, which 44 §1.2 gives **1** and which this hand raises as
/// **M6H4-1** rather than repairing.
pub fn cancel(session: &mut Session, id: &TransformationId, at: Timestamp) -> Result<Outcome> {
    // 🔴 **DR-43-4's entry sweep** (`req/38` §148 ruling 1(iv)) — see [`Session::sweep`]. A row
    // whose deadline had already passed is `Aborted(Expired)` and not `Aborted(OwnerCancelled)`,
    // and the difference is the whole of 43 T-6's account of what happened.
    session.sweep(at)?;
    // 🔴 **E-M6-1** — a draft, refused by name.
    //
    // The id-resolution of 44 §0 accepts either spelling, so a `gx cancel <IntentId>` reaches here
    // with a value that names a draft this project is holding. That is not a transformation and
    // never becomes an `Aborted` record; what it is is a file, and the operator is told so.
    // 🔴 A row past `Committing` is answered from **Σ**, before any resume.
    //
    // 34 AC-073's second half is "for a T already at `Committing` or later (including
    // `Committed`) … the operation is refused as invalid and the existing state does not change" (sem: SEM-gx-cli-356), and a resume would refuse it first — with "43 §3 has no `plan`
    // from a Committed row", which is true and is an answer to a different question. An operator
    // who typed `gx cancel` on a committed transformation asked about T-7, so T-7 is what answers.
    if let Some(row) = session.recorded(id) {
        if let Some(state) = row.state {
            if !matches!(
                state,
                Lifecycle::Candidate
                    | Lifecycle::Verifying
                    | Lifecycle::Admitted
                    | Lifecycle::Canonicalized
                    | Lifecycle::Escalated
                    | Lifecycle::Aborted(_)
            ) {
                // 🔴 **E-M6-13** (req/38 §51 M6H4-1 adopted (a); sem: SEM-gx-cli-357) — 44 §1.4's **2**, not its 1.
                //
                // Hand 4 refused this with `Error::Usage` and wrote the disagreement into its own
                // exit table: "44 §1.2 gives this refusal exit 1 although it is a state machine
                // saying no, which is what §1.4's 2 is for" (sem: SEM-gx-cli-358). §51 ruled it. What changes with the
                // number is what a script can do: 1 says "you asked wrongly, try again differently"
                // and 2 says "the machine refused, and it will refuse the same way for ever".
                //
                // The object goes to **stdout** rather than the message to stderr, for the reason
                // every other refusal in this crate does it: an `Outcome` is "the command ran and
                // answered no" (sem: SEM-gx-cli-359) and an `Err` is "the command could not run", and 44 §1.3 gives the
                // first a JSON object.
                return Ok(Outcome::refused(
                    serde_json::json!({
                        "transformation": id.0.to_text(),
                        "state": state.name(),
                        "refused": "OutsideCancelWindow",
                        "detail": format!(
                            "{} is {}, and 43 T-7 cancels only before the critical section: its \
                             from-set is {{Candidate, Verifying, Admitted, Canonicalized, \
                             Escalated}} and its guard is \"before reaching `Committing`\" (sem: SEM-gx-cli-360). Nothing was \
                             changed (E-M6-13: 44 §1.2 writes 1 for this and §1.4's 2 is \
                             \"refused (denied)\")", // (sem: SEM-gx-cli-361)
                            id.0.to_text(),
                            state.name()
                        ),
                    }),
                    DENIED,
                ));
            }
        }
    }

    if let Some(intent) = session.draft(&gx_core::IntentId(id.0))? {
        let _ = intent;
        return Err(Error::Usage {
            detail: format!(
                "{} names a Draft, and a draft is discarded rather than cancelled (E-M6-1, req/38 \
                 §47 M6-03 adopted (c): removes Draft from 44 L101's from-set; sem: SEM-gx-cli-362). 43 T-1 leaves a draft \
                 without a `TransformationId` and 43 T-7's `Aborted` is keyed on one, so there is \
                 no record this engine could write about cancelling it. Discarding it is deleting \
                 `.gx/drafts/{}.json`, which no verb does today (M6H4-2)",
                id.0.to_text(),
                id.0.to_text().replace(':', "_")
            ),
        });
    }

    session.resume(id, at)?;
    let state = match session.engine().cancel(id, at) {
        Ok(state) => state,
        // 🔴 **E-M6-13**, the second road to the same refusal. The Σ check above answers about a row
        // the journal knows; this answers about one the **engine** refused after a resume — an
        // `Aborted` row, say, which the from-set above lets through so that a repeated `gx cancel`
        // reaches T-7 rather than a resume error. Both are 43 §3 saying "no transition from here" (sem: SEM-gx-cli-363)
        // and giving them two different statuses would make the exit depend on which of two equal
        // roads the request happened to take.
        Err(e) => return state_machine_refusal(id, "cancel", Error::from(e)),
    };
    // 44 §1.2: "stdout: `{ "transformation": <id>, "state": "Aborted", "reason": "OwnerCancelled" }`" (sem: SEM-gx-cli-364).
    let reason = match state {
        Lifecycle::Aborted(reason) => Some(reason),
        _ => None,
    };
    Ok(Outcome::ok(serde_json::json!({
        "transformation": id.0.to_text(),
        // 🔴 44 §1.2's own shape: "`{ "transformation": <id>, "state": "Aborted", "reason":
        // "OwnerCancelled" }`" (sem: SEM-gx-cli-365) — the **name** of the state and the reason beside it. `Lifecycle`'s
        // serialised form is `{"Aborted":"OwnerCancelled"}`, which carries the same two facts in
        // one field and is not what 44 writes; a wire contract is a contract about the shape, so
        // the shape is 44's and the reason is not printed twice.
        "state": state.name(),
        "reason": reason,
    })))
}

/// Which of 43's two human rulings this invocation is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// T-5 — "human ruling = Admit" (sem: SEM-gx-cli-366), to `Admitted`.
    Approve,
    /// T-5b — "human ruling = Deny" (sem: SEM-gx-cli-367), to `Denied`.
    Reject,
}

impl Decision {
    /// 42 §3.13: "`kind` is Admit|Deny only" (sem: SEM-gx-cli-368).
    fn kind(self) -> VerdictKind {
        match self {
            Decision::Approve => VerdictKind::Admit,
            Decision::Reject => VerdictKind::Deny,
        }
    }
}

/// 🔴 `gx escalation approve|reject <TICKET_ID> --reason <TEXT>` (44 §1.2) — T-5 / T-5b.
///
/// AC-071 and AC-072 are this call, and INV-S6 is what it is for: "`Escalated` does not
/// auto-transition to `Admitted`/`Denied` without going through T-5/T-5b's signed human-ruling
/// receipt" (sem: SEM-gx-cli-369).
///
/// `id` is 44 §0's id-resolution one verb further out (**M6-04 adopted (c)**; sem: SEM-gx-cli-370): a `TicketId`, which is what
/// §1.2 writes, or a `TransformationId`, which is what 44 §2.2's `{id}` writes for the same
/// operation. Accepting both is what makes the CLI and the HTTP surface name one thing.
///
/// # Errors
/// [`Error::NotFound`] (44's 6, "not-found (unknown ticket)"; sem: SEM-gx-cli-371) when neither reading names an escalated
/// transformation this project holds. Anything the engine refuses, unchanged — "the target is not
/// `Escalated`" arrives as `Error::InvalidState` on 44 §1.2's **1**, which this hand raises as
/// **M6H4-1** rather than repairing.
pub fn escalation(
    session: &mut Session,
    id: &str,
    decision: Decision,
    reason: &str,
    ruler: &Actor,
    at: Timestamp,
) -> Result<Outcome> {
    // 🔴 **DR-43-4's entry sweep** (`req/38` §148 ruling 1(iv)) — see [`Session::sweep`]. 43 T-6
    // reaches `Escalated` with its own, longer TTL (33 NFR-028's 72 hours), and a person ruling on
    // a queue entry that expired last week should be told so rather than allowed to rule.
    session.sweep(at)?;
    let cid = crate::index::parse_id(id)?;
    // Both readings, in the order 44 §1.2 writes them: a ticket first, because that is the name the
    // command's own synopsis gives, and a transformation second.
    let named_ticket = TicketId(cid);
    let transformation = match session.read().transformation_of_ticket(&named_ticket)? {
        Some(found) => found,
        None => TransformationId(cid),
    };

    session.resume(&transformation, at)?;
    // 🔴 The **ruler's** key, not the transformation's. 43 T-5's side effect is "a human-ruling
    // receipt (signed)" and its guard is "the ruler holds a valid signing key" (sem: SEM-gx-cli-372) — a receipt signed with the
    // submitter's key would attest that the party being ruled on approved themselves. This is the
    // one place in the CLI where a key is chosen by the operator rather than by the row, which is
    // also the one place 45 §1's key separation is honoured at this layer (M6H3-4 is the rest).
    let key = crate::keys::KeyStore::user_default()?.load(ruler.key())?;
    let ruling = HumanRuling {
        decision: decision.kind(),
        reason: reason.to_string(),
        actor: ruler.clone(),
    };
    let ticket = session
        .read()
        .ticket(&transformation)
        .map(|t| t.id.0.to_text());
    let state = match session
        .engine()
        .escalation(&transformation, &ruling, at, &key)
    {
        Ok(state) => state,
        // 🔴 **E-M6-13**. 44 §1.2 wrote "1 = the ruler's key is invalid **or** the target is not
        // `Escalated`" (sem: SEM-gx-cli-373) as one
        // status for two events. The key half is a genuine "invalid input" and stays on 1 — it arrives
        // above, from `KeyStore::load`, before the engine is reached — and this half is 43 T-5's
        // guard, which is the state machine and is §1.4's 2.
        Err(e) => return state_machine_refusal(&transformation, "escalation", Error::from(e)),
    };

    // 🔴 **M6H4-7**'s third kind. 43 T-5's receipt is signed by the **ruler's** key and the verdict
    // receipt beside it by the engine's, so the two are two documents; a store with one slot per
    // transformation would let this one erase that one, which is the fact INV-S6 exists to keep.
    // `verdict_receipts` is ordered by issue (M5H4-6), so the ruling is the last of it.
    let stored_ruling = match session.read().verdict_receipts(&transformation).last() {
        Some(receipt) => Some(
            crate::receipt::ReceiptStore::in_layout(session.layout()).put(
                &transformation,
                crate::receipt::StoredKind::Ruling,
                receipt,
            )?,
        ),
        None => None,
    };

    // 44 §1.2: "stdout: `{ "transformation": <id>, "state": "Admitted"|"Denied" }`" (sem: SEM-gx-cli-374), plus the two
    // facts AC-071/072 ask a reader to confirm — the ruling reached the trail and under whose name.
    Ok(Outcome::ok(serde_json::json!({
        "transformation": transformation.0.to_text(),
        "state": state,
        "ticket": ticket,
        "decision": ruling.decision,
        "reason": ruling.reason,
        "ruled_by": ruling.actor,
        // 43 T-5's side effect is "append a human-ruling receipt (signed) to the provenance chain" (sem: SEM-gx-cli-375), and the count is
        // what AC-071's "receipt trail" is made of. Printed rather than asserted here: the suite
        // asserts, and an operator gets to see that something was signed.
        "verdict_receipts": session.read().verdict_receipts(&transformation).len(),
        "receipt_stored_at": stored_ruling.map_or(serde_json::Value::Null, |p| {
            p.display().to_string().replace('\\', "/").into()
        }),
        // 🔴 **Which key signed the ruling.** 43 T-5's guard is "the ruler holds a valid signing key" (sem: SEM-gx-cli-376) and the
        // whole point of INV-S6 is that an escalation records who **allowed** a change — so "a
        // receipt was issued" is not the fact worth printing; "it was issued under the ruler's
        // key" is. Without this field a receipt signed with the submitter's key would be
        // indistinguishable from a correct one at this surface, which is what the battery measured.
        "signed_by": session
            .read()
            .verdict_receipts(&transformation)
            .last()
            .and_then(|receipt| receipt.payload().ok())
            .map(|payload| payload.key_id),
    })))
}

/// 🔴 **E-M6-13** — 43 §3 refusing a transition is 44 §1.4's **2**; everything else is unchanged.
///
/// > add exit **2** to cancel/escalation's state-machine refusal (§1.2's column is an excerpt;
/// > applying the rest of M6-25's reading to the remaining 2 verbs). The implementation is the
/// > first hand from hand 5 onward to take this step (sem: SEM-gx-cli-377)
///
/// One function for the two verbs, because the ruling is one ruling and a second copy would be a
/// second place for the two to disagree. What it branches on is [`gx_engine::Error::InvalidState`]
/// — the engine's own name for "a transition was asked for from a state 43 §3 does not offer it
/// from" (sem: SEM-gx-cli-378) — and **nothing else**: an adapter refusal, an I/O refusal and a witness refusal all still
/// take the generic mapping, because none of them is the state machine answering.
///
/// # Errors
/// Every refusal that is not the state machine's, unchanged.
fn state_machine_refusal(id: &TransformationId, verb: &'static str, e: Error) -> Result<Outcome> {
    let Error::Engine(gx_engine::Error::InvalidState { state, .. }) = &e else {
        return Err(e);
    };
    Ok(Outcome::refused(
        serde_json::json!({
            "transformation": id.0.to_text(),
            "state": state,
            "refused": verb,
            "detail": e.to_string(),
        }),
        DENIED,
    ))
}

/// The status a refusal from this module exits with, where it is not the generic mapping.
///
/// Only one entry today and it is [`NOT_FOUND`]'s: a ticket that resolves to nothing is 44 §1.2's
/// "6=not-found (unknown ticket)" (sem: SEM-gx-cli-379) and reaches [`Error::NotFound`], which `Error::exit_code` already maps.
/// The constant is named so that a reader looking for "where does escalation's 6 come from" (sem: SEM-gx-cli-380) finds a
/// line rather than an absence.
pub const TICKET_NOT_FOUND: u8 = NOT_FOUND;

/// The status a completed verb exits with. Named for the same reason as [`TICKET_NOT_FOUND`].
pub const COMPLETED: u8 = OK;

fn to_json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}
