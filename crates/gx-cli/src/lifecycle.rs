//! `gx undo` / `gx cancel` / `gx escalation approve|reject` — 44 §1.2's back half.
//!
//! Three verbs and three of 43's transitions: T-12's first half ([`undo`]), T-7 ([`cancel`]) and
//! T-5/T-5b ([`escalation`]). [`crate::pipeline`] drives a transformation forward; this module is
//! what an operator reaches for when the answer is 「take it back」, 「stop it」 or 「I decide」.
//!
//! # 🔴 `gx cancel` no longer accepts a `Draft` — **E-M6-1**
//!
//! 44 L101 lists `{Draft, Candidate, Verifying, Admitted, Canonicalized, Escalated}` and req/38 §47
//! M6-03 採(c) removed the first:
//!
//! > 44 L101 の `gx cancel` from-set から `Draft` を外す erratum(本 § が正本・44 は書き換えない)+
//! > AC-073 の Given も同読み替え
//!
//! and it withdrew §45 M5H8-2 採(b) — 「M6 の id-resolution が Draft を担う」 — with the frame
//! correction written out: id-resolution solves the problem of *pointing* at something and a draft's
//! problem is that there is **no seat**. A `Draft` has no `TransformationId` (43 T-1, E-M5-3),
//! `Aborted` is keyed on one, and M5-17 採(b) keeps the draft phase in the journal alone. So
//! [`cancel`] refuses a draft **by name**, tells the operator what discarding one actually is (the
//! file under `.gx/drafts/`), and does not invent a verb for it — whether `gx draft discard` should
//! exist is raised as **M6H4-2** rather than decided by the hand that noticed.
//!
//! # 🔴 `gx undo` returns 44 §1.4's **2** — M6-25's body
//!
//! 44 §1.2 gives `undo` the list 0/1/3/5/6 and no seat for 「拒否（denied）」, while AC-040's second
//! case is exactly that: 「T_uに対応するinvariant/policyを故意にDenyへセットしたケースではT_uが
//! Committedへ到達せずDeniedのまま」, implemented in the engine since M5. req/38 §47 M6-25 採(a)+(c)
//! reads §1.2's list as an **excerpt** and §1.4's common table as the answer, so a denied undo exits
//! **2** and not 1. Folding it into 1 would put 「could not run」 and 「the gate refused you」 under
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
//! M6-04 採(c), the natural extension of 44 §0's id-resolution rule, which also removes the asymmetry
//! between the CLI's `<TICKET_ID>` and 44 §2.2's `{id}`.

use gx_core::{Actor, Timestamp, TransformationId, VerdictKind};
use gx_engine::{HumanRuling, Lifecycle};
use gx_gate::TicketId;

use crate::exit::{
    Outcome, APPLY_FAILED, DENIED, ERROR, ESCALATED, NOT_FOUND, OK, PRECONDITION_CHANGED,
};
use crate::session::Session;
use crate::{Error, Result};

/// 🔴 `gx undo <TID>` (44 §1.2) — T-12's first half, then the whole ordinary pipeline.
///
/// > 対象のcommit済み`EscrowedInverse`（42 §3.12, `status=Available`のもの）を`delta`として新たな
/// > `Transformation`をsubmit〜commitまで一括実行する（P-5: 取り消しは新たな逆変換のcommit）
///
/// `Engine::undo` builds the `Candidate` and, in its own words, 「does not undo anything」: 43 §5-2
/// makes T_u walk `Draft→Candidate→Verifying→…→Committed` on its own feet, 「undoであっても検証を
/// 免除されない」. So this function drives the same four steps `gx submit`..`gx commit` drive, in one
/// process because 44 says 「一括実行」, and the supersede edge is drawn by the commit rather than by
/// the undo (T-12, `Engine::supersede_after_commit`).
///
/// # 🔴 The signing key comes from the original's actor, and the reason is worth a line
///
/// `Engine::undo` mints T_u's `Intent` with **T_o's context and actor** (P-5: the undo is a change
/// by whoever's change is being taken back), so [`Session::signing_key`] resolves to the same key
/// the original receipt was signed under. 44 §1.2's `--actor-key` on this verb therefore selects
/// nothing today, and is refused rather than ignored (M4H5-5) — raised as **M6H4-3**.
///
/// # Errors
/// [`Error::NotFound`] (44's 6) for an unknown transformation or one with no escrowed inverse,
/// [`Error::Usage`] for `--actor-key`. Anything the engine refuses, unchanged — 44 §1.2's 「1=
/// `InverseStatus`が`Unavailable`/`Expired`/`Consumed`で実行不能」 arrives that way.
pub fn undo(
    session: &mut Session,
    id: &TransformationId,
    idempotency_key: Option<&str>,
    rng_seed: u64,
    at: Timestamp,
) -> Result<Outcome> {
    // The original has to be in the table before T-12 can read its escrow, and after a commit the
    // row is exactly what a fresh process does not have (M6H3-1). `Session::resume` is refused for
    // a committed row — a re-plan reads a substrate the commit has already moved — so the road in
    // is the engine's own recovery of the escrow, which `Engine::open` does rebuild from Σ.
    session.rehydrate_committed(id)?;

    let (intent_id, undoing) = session.engine().undo(id, rng_seed, at)?;
    session.remember(intent_id, undoing);

    // 43 §5-2. The same three calls `gx verify` and `gx commit` make, in one process because 44 §1.2
    // says 「submit〜commitまで一括実行する」 — and with the same meanings, which is why a denied undo
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
    // 44 §1.2: 「stdout: 新`Transformation`の`Receipt`」.
    let mut json =
        serde_json::to_value(engine.receipt(&undoing)).unwrap_or(serde_json::Value::Null);
    if let Some(map) = json.as_object_mut() {
        map.insert("transformation".into(), undoing.0.to_text().into());
        map.insert("undone".into(), id.0.to_text().into());
        map.insert("state".into(), to_json(&state));
        // 43 T-12: 「`T_o`のメタデータに`superseded_by = T_u.id`を追記」. The edge is the engine's and
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

/// 🔴 Where an undo stopped, and the status 44 gives that stop — **M6-25's 2 among them**.
///
/// The five 44 §1.2 writes for `undo` are 0/1/3/5/6, and the sixth is the one §1.4 has and §1.2's
/// list does not: a `Denied` T_u is 「拒否（denied）」 and exits **2**. AC-040's second case is
/// exactly this transformation and the engine has produced it since M5, so the alternative was to
/// answer 「実行不能」 about a gate that ran and said no.
///
/// 🔴 The state is **read from the engine** rather than passed in, and the reason is one 則 1 (iii)
/// found rather than one this hand chose. `crates/gx-canon/tests/authority_boundary.rs` reads 「a
/// declaration whose type mentions `Lifecycle` and ends in a comma」 as 「the CLI keeps a state
/// table」, and a **parameter list** is exactly that shape once rustfmt puts one argument per line.
/// The probe is right to be coarse (a state table is declared in one line), so the code moved: three
/// arguments fit on one line, and reading the state back from the engine is what every other verb in
/// this crate does anyway. Hand 3 hit the same scanner twice and drew the same conclusion.
fn halted(session: &Session, original: &TransformationId, undoing: &TransformationId) -> Outcome {
    let state = session.read().state(undoing);
    let code = match state {
        // 🔴 M6-25 採(a)+(c). §1.2's list is an excerpt; §1.4's common table is the answer.
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
/// > `{Candidate, Verifying, Admitted, Canonicalized, Escalated}`のいずれかから`Aborted(OwnerCancelled)`
/// > へ遷移する。`Committing`到達後は実行不可
///
/// (44 L101 writes `Draft` first; req/38 §47 M6-03 採(c) removed it. See the module header.)
///
/// # 🔴 The owner-permission guard is not enforced, and 44's `--actor-key` selects nothing
///
/// 43 T-7's guard is 「actorがオーナー権限（Actor::Human{key}相当）を保持」 and v0.1 has no
/// authorization layer (M5H6-4 採(a)): `Engine::cancel` takes no actor, the `Aborted` record has no
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
    // 🔴 **E-M6-1** — a draft, refused by name.
    //
    // The id-resolution of 44 §0 accepts either spelling, so a `gx cancel <IntentId>` reaches here
    // with a value that names a draft this project is holding. That is not a transformation and
    // never becomes an `Aborted` record; what it is is a file, and the operator is told so.
    // 🔴 A row past `Committing` is answered from **Σ**, before any resume.
    //
    // 34 AC-073's second half is 「既に`Committing`以降（Committed含む）のTに対し…無効操作として拒否
    // され既存状態を変更しない」, and a resume would refuse it first — with 「43 §3 has no `plan`
    // from a Committed row」, which is true and is an answer to a different question. An operator
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
                // 🔴 **E-M6-13** (req/38 §51 M6H4-1 採(a)) — 44 §1.4's **2**, not its 1.
                //
                // Hand 4 refused this with `Error::Usage` and wrote the disagreement into its own
                // exit table: 「44 §1.2 gives this refusal exit 1 although it is a state machine
                // saying no, which is what §1.4's 2 is for」. §51 ruled it. What changes with the
                // number is what a script can do: 1 says 「you asked wrongly, try again differently」
                // and 2 says 「the machine refused, and it will refuse the same way for ever」.
                //
                // The object goes to **stdout** rather than the message to stderr, for the reason
                // every other refusal in this crate does it: an `Outcome` is 「the command ran and
                // answered no」 and an `Err` is 「the command could not run」, and 44 §1.3 gives the
                // first a JSON object.
                return Ok(Outcome::refused(
                    serde_json::json!({
                        "transformation": id.0.to_text(),
                        "state": state.name(),
                        "refused": "OutsideCancelWindow",
                        "detail": format!(
                            "{} is {}, and 43 T-7 cancels only before the critical section: its \
                             from-set is {{Candidate, Verifying, Admitted, Canonicalized, \
                             Escalated}} and its guard is 「`Committing`到達前」. Nothing was \
                             changed (E-M6-13: 44 §1.2 writes 1 for this and §1.4's 2 is \
                             「拒否（denied）」)",
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
                 §47 M6-03 採(c): 44 L101 の from-set から Draft を外す). 43 T-1 leaves a draft \
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
        // reaches T-7 rather than a resume error. Both are 43 §3 saying 「no transition from here」
        // and giving them two different statuses would make the exit depend on which of two equal
        // roads the request happened to take.
        Err(e) => return state_machine_refusal(id, "cancel", Error::from(e)),
    };
    // 44 §1.2: 「stdout: `{ "transformation": <id>, "state": "Aborted", "reason": "OwnerCancelled" }`」.
    let reason = match state {
        Lifecycle::Aborted(reason) => Some(reason),
        _ => None,
    };
    Ok(Outcome::ok(serde_json::json!({
        "transformation": id.0.to_text(),
        // 🔴 44 §1.2's own shape: 「`{ "transformation": <id>, "state": "Aborted", "reason":
        // "OwnerCancelled" }`」 — the **name** of the state and the reason beside it. `Lifecycle`'s
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
    /// T-5 — 「人間裁定 = Admit」, to `Admitted`.
    Approve,
    /// T-5b — 「人間裁定 = Deny」, to `Denied`.
    Reject,
}

impl Decision {
    /// 42 §3.13: 「kindはAdmit|Denyのみ」.
    fn kind(self) -> VerdictKind {
        match self {
            Decision::Approve => VerdictKind::Admit,
            Decision::Reject => VerdictKind::Deny,
        }
    }
}

/// 🔴 `gx escalation approve|reject <TICKET_ID> --reason <TEXT>` (44 §1.2) — T-5 / T-5b.
///
/// AC-071 and AC-072 are this call, and INV-S6 is what it is for: 「`Escalated`はT-5/T-5bの署名済み
/// 人間裁定receiptを経由せずに`Admitted`/`Denied`へ自動遷移しない」.
///
/// `id` is 44 §0's id-resolution one verb further out (**M6-04 採(c)**): a `TicketId`, which is what
/// §1.2 writes, or a `TransformationId`, which is what 44 §2.2's `{id}` writes for the same
/// operation. Accepting both is what makes the CLI and the HTTP surface name one thing.
///
/// # Errors
/// [`Error::NotFound`] (44's 6, 「未検出（チケット不明）」) when neither reading names an escalated
/// transformation this project holds. Anything the engine refuses, unchanged — 「対象が`Escalated`
/// でない」 arrives as `Error::InvalidState` on 44 §1.2's **1**, which this hand raises as
/// **M6H4-1** rather than repairing.
pub fn escalation(
    session: &mut Session,
    id: &str,
    decision: Decision,
    reason: &str,
    ruler: &Actor,
    at: Timestamp,
) -> Result<Outcome> {
    let cid = crate::index::parse_id(id)?;
    // Both readings, in the order 44 §1.2 writes them: a ticket first, because that is the name the
    // command's own synopsis gives, and a transformation second.
    let named_ticket = TicketId(cid);
    let transformation = match session.read().transformation_of_ticket(&named_ticket)? {
        Some(found) => found,
        None => TransformationId(cid),
    };

    session.resume(&transformation, at)?;
    // 🔴 The **ruler's** key, not the transformation's. 43 T-5's side effect is 「人間裁定receipt
    // （署名済み）」 and its guard is 「裁定者が有効な署名鍵を保持」 — a receipt signed with the
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
        // 🔴 **E-M6-13**. 44 §1.2 wrote 「1=裁定者鍵不正**または**対象が`Escalated`でない」 as one
        // status for two events. The key half is a genuine 「入力不正」 and stays on 1 — it arrives
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

    // 44 §1.2: 「stdout: `{ "transformation": <id>, "state": "Admitted"|"Denied" }`」, plus the two
    // facts AC-071/072 ask a reader to confirm — the ruling reached the trail and under whose name.
    Ok(Outcome::ok(serde_json::json!({
        "transformation": transformation.0.to_text(),
        "state": state,
        "ticket": ticket,
        "decision": ruling.decision,
        "reason": ruling.reason,
        "ruled_by": ruling.actor,
        // 43 T-5's side effect is 「人間裁定receipt（署名済み）をprovenance鎖に追記」, and the count is
        // what AC-071's 「receipt trail」 is made of. Printed rather than asserted here: the suite
        // asserts, and an operator gets to see that something was signed.
        "verdict_receipts": session.read().verdict_receipts(&transformation).len(),
        "receipt_stored_at": stored_ruling.map_or(serde_json::Value::Null, |p| {
            p.display().to_string().replace('\\', "/").into()
        }),
        // 🔴 **Which key signed the ruling.** 43 T-5's guard is 「裁定者が有効な署名鍵を保持」 and the
        // whole point of INV-S6 is that an escalation records who **allowed** a change — so 「a
        // receipt was issued」 is not the fact worth printing; 「it was issued under the ruler's
        // key」 is. Without this field a receipt signed with the submitter's key would be
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
/// > cancel/escalation の状態機械拒否に exit **2** を足す(§1.2 列は抜粋・M6-25 の読みの残り 2 verb
/// > への適用)。実装は手5 以降の最初に踏む手
///
/// One function for the two verbs, because the ruling is one ruling and a second copy would be a
/// second place for the two to disagree. What it branches on is [`gx_engine::Error::InvalidState`]
/// — the engine's own name for 「a transition was asked for from a state 43 §3 does not offer it
/// from」 — and **nothing else**: an adapter refusal, an I/O refusal and a witness refusal all still
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
/// 「6=未検出（チケット不明）」 and reaches [`Error::NotFound`], which `Error::exit_code` already maps.
/// The constant is named so that a reader looking for 「where does escalation's 6 come from」 finds a
/// line rather than an absence.
pub const TICKET_NOT_FOUND: u8 = NOT_FOUND;

/// The status a completed verb exits with. Named for the same reason as [`TICKET_NOT_FOUND`].
pub const COMPLETED: u8 = OK;

fn to_json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}
