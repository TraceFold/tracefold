// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What 43 §7's recovery owes the operator who set it off, said by **whichever verb set it off**.
//!
//! # 🔴 Why this module exists (`req/470` H-01, `req/38` §264 ruling 3 item 1)
//!
//! R34 wrote the sentence 43 §7-3c's road owes an operator and wired it to `gx serve`. Audit 34
//! then counted the roads and found that `gx serve` is **one of five**: `Engine::recover` has six
//! shipped call sites (`pipeline.rs:253` = `gx verify`, `pipeline.rs:373` = `gx commit`,
//! `lifecycle.rs:197` = `gx undo`, `repair.rs:778` = `gx repair --yes`, `serve.rs:729` = `gx
//! serve`, and `session.rs`'s own wrapper through which the first three pass), while
//! `announced_road_note` had **one**. The measurement, verbatim:
//!
//! ```text
//! A34_DENOM recover_call_sites=6 note_call_sites=1
//! ```
//!
//! On the same bed — a project cut inside the window, a third party's `THIRD PARTY` written into
//! the file afterwards — `gx verify` answered `rc 0` with an `Admit` on stdout, replaced the third
//! party's bytes, and printed **0 bytes on stderr**. `gx commit` (`rc 1`) and `gx repair --yes`
//! (`rc 0`, `repaired: true`, `refusals: []`) did the same. `docs/LIMITS.md` v0.5-t said, in a
//! paragraph twenty lines after it had itself measured `gx repair` printing nothing: "**it changes
//! all of it.** Every row that walks that road now prints a sentence".
//!
//! # The shape of the defect, and therefore the shape of the repair
//!
//! The defect was never "one function was wrong". It was **a road existed and nobody had wired the
//! sentence to it** — and a repair that hand-wires today's five roads leaves that shape intact for
//! the sixth. So the sentence lives here, and the wiring is arranged so that silence is the thing
//! that takes work:
//!
//! * the three verbs that pass through [`crate::session::Session::recover`] announce **inside**
//!   that call, so a seventh write verb added to 44 §1.2 tomorrow is loud without its author
//!   having to know this module exists;
//! * the two callers that hold an `Engine` directly and cannot use the session's wrapper
//!   (`repair.rs`, `serve.rs`) call [`announce_recovery`] by name;
//! * `crates/gx-cli/tests/r35_shared_road_sentence.rs` holds a source census that fails if a new
//!   `.recover(` site appears on neither road.
//!
//! # The design that was not taken, and why
//!
//! `req/471` §0-1 left the choice open between (a) composing the sentence at each verb from
//! [`crate::session::Session::recover`]'s return value and (b) moving the sentence to a shared
//! module wired from all five entry points. **Neither alone was taken, and (a) was rejected
//! outright**: it duplicates the emit loop at three sites and — the deciding objection — it leaves
//! the next verb silent by default, which is precisely the failure being repaired. (b) is here,
//! but (b) alone still asks each of five callers to remember a call, so half of the defect's shape
//! would survive. What ships is (b)'s shared module entered through (a)'s shared call: the
//! announcement is *inside* `Session::recover`, and only the two engine-holding callers pay an
//! explicit line.
//!
//! # What is **not** claimed, here or by the sentence
//!
//! That gx knows whether anybody else wrote. It does not, and neither R34 nor this lane gave it a
//! way to find out. `ApplyStarted` is `{transformation, delta_cid, at}` and carries no
//! fingerprint; the journal's only fingerprint is `Planned.fp0`; and `req/78` §3.2 Λ4 is the proof
//! that the comparison cannot simply be re-run — with an `ApplyStarted` present, a fingerprint
//! that differs from `fp0` is what a **successful** apply looks like. A post-apply fingerprint
//! would separate the two and the record for it does not exist (the engine's own doc raises it as
//! **M5H5-3**); adding one is a change to 42 §3.13 and a DR of its own. So the honest verb is to
//! say what was done and what was not compared, from every mouth that does it.
//!
//! # Why a sentence on stderr and not a counter
//!
//! Audit 34 §1-6 item 3 put the strongest case against its own finding: `gx repair --json` already
//! answers `apply_was_announced: 1`, so the information is there. Half of that is right and the
//! half that is not is the load-bearing half. A counter says **a row walked the road**. It does
//! not say *what could not be compared*, and it does not say *that this run may have written over
//! somebody else's bytes and cannot tell you so* — and those two are the whole content of the
//! claim `docs/LIMITS.md` makes. Three of the four silent verbs had no counter either.

use gx_core::TransformationId;
use gx_engine::pipeline::{Recovered, RecoveryPath};

/// The sentence 43 §7-3c's road owes an operator, in the voice of `verb`, or `None` for every
/// other road.
///
/// `verb` is the command as a reader would type it — `"gx verify"`, `"gx repair"` — and becomes
/// the sentence's prefix. Before this lane the prefix was the literal `"gx serve: "`, baked into
/// the format string at `serve.rs:495`/`:503`, so a naive move of the function would have had
/// `gx verify` announce itself as `gx serve`.
///
/// # The two findings this answers, and the one thing it does not claim
///
/// **H-01** (`req/449`). A row the ledger holds no leaf for is finished by applying the delta, and
/// the delta is written over whatever the substrate holds **at that moment**. The audit
/// reconstructed a crash in that window, let a third party write to the file afterwards, and
/// watched `gx serve` replace their bytes, answer `refused: 0`, answer `/healthz` `200 ok`, and
/// say nothing at all. The world moving is not the finding — the silence is.
///
/// **H-02** (`req/449`). When the adapter refuses the re-apply, the row ends `Aborted(ApplyFailed)`
/// with `Rollback::NotAttempted` (`pipeline.rs`'s `RecoveredWithoutRebuilding`), which means the
/// compensation did not run. `req/372` M-01 built the fixture for the commonest real shape of that
/// failure — *the call worked and the answer was lost coming back* — so "the adapter refused" and
/// "the world did not move" are **not** the same statement, and this sentence does not pretend
/// they are.
///
/// **What is not claimed** is in this module's header: gx cannot tell its own unrecorded apply
/// from somebody else's write on this road, and says so rather than implying otherwise.
#[must_use]
pub fn announced_road_note_for(verb: &str, row: &Recovered) -> Option<String> {
    if row.path != RecoveryPath::ApplyWasAnnounced {
        return None;
    }
    let id = row.transformation.0.to_text();
    if row.ended_aborted() {
        // 🔴 **`req/449` H-02** — the row that used to be counted as resumed and printed nothing.
        Some(format!(
            "{verb}: 43 §7-3c did **not** finish {id} — its apply was announced, the adapter \
             then refused, and the row is now terminal `Aborted(ApplyFailed)` with the roll-back \
             **not attempted** (there was no rebuilt inverse in this run to attempt). Whether the \
             substrate moved is a question this process cannot answer: an adapter that performs \
             the change and then loses the answer coming back is the commonest real failure \
             (req/372 M-01), so treat this row's object as **possibly changed and certainly \
             unrecorded** — `gx replay {id}` names it and the ledger holds no leaf for it. This \
             row is counted under `recover.apply_was_announced` and under \
             `recover.announced_and_aborted`, which `gx repair --json` answers for any verb and \
             `gx serve` prints in its start-up line (req/449 H-02, req/470 H-01)"
        ))
    } else {
        // 🔴 **`req/449` H-01** — the road that writes, saying what it wrote over.
        Some(format!(
            "{verb}: 43 §7-3c finished {id} by **applying its delta** — the ledger holds no leaf \
             for this row, so the commit had not completed and finishing it is what the recovery \
             is for. What was **not** checked: whether anything had written to that object between \
             the crash and now. On this road there is nothing to check against — with an \
             `ApplyStarted` in the journal, a fingerprint that differs from the planned one is \
             also what a successful apply looks like (req/78 §3.2 Λ4), and no post-apply \
             fingerprint is recorded (M5H5-3). ∴ if something else wrote to this object after the \
             crash, this run has just written over it and cannot tell you so. 43 §7-3b's \
             road, where the ledger *does* hold the leaf, refuses instead — it has a leaf to \
             compare with (req/397 H-01, req/449 H-01)"
        ))
    }
}

/// Say, on stderr, what the recovery just did — once per row that walked the road that writes.
///
/// # Why this is a loop and not a counter, and why it is on stderr
///
/// The module header answers both. The short form: `gx repair`'s `apply_was_announced` says a road
/// was walked, and the two things an operator needs — *what could not be compared* and *that this
/// run may have destroyed somebody's bytes* — are not in a number. stderr because stdout is
/// spoken for: 44 §1.3 gives it to the verb's own JSON, and `gx wrap`'s transport gives it to MCP
/// outright ("the server **MUST NOT** write anything to its stdout that is not a valid MCP
/// message"), which is why the membrane can carry this sentence at all.
///
/// # 🔴 What this deliberately does **not** move out of `gx serve`
///
/// `serve.rs` says three more things per row: a refusal (`req/227` H-01), a row closed from the
/// ledger's leaf (`req/244` H-03), and the torn-tail note (`DR-43-9`). They are **not** moved
/// here, and that is a scope decision rather than an oversight. Audit 34 measured one silence and
/// this lane repairs one silence; widening the change would alter the stderr of every write verb
/// on roads no audit has measured, which is how a repair becomes the next lane's finding.
/// `req/472` records it as a remaining gap rather than letting it read as covered: **a `gx verify`
/// that sets off a recovery which then refuses a row still says nothing about that refusal.**
pub fn announce_recovery(verb: &str, rows: &[Recovered]) {
    for row in rows {
        if let Some(sentence) = announced_road_note_for(verb, row) {
            crate::note!("{sentence}");
        }
    }
}

/// 🔴 **`req/476` H-01** — the sentence the **`Err`** road owes, for a row whose delta landed and
/// whose commit could not be recorded.
///
/// R35's sentence above says 43 §7-3c "**finished**" the row. On this road it did not: `apply_once`
/// answered `Ok`, the substrate moved, and one of the eight steps after it — the attest rebuild,
/// the payload digest, `ledger.append`, the inclusion proof, `Receipt::issue`, filing that receipt,
/// the `Committed` record, the head — raised. Saying "finished" there would be a second false
/// sentence in place of the silence, so this is its own text.
///
/// # What it must carry, and why each clause is load-bearing
///
/// 1. **The delta landed.** This is the fact the whole finding is about: audit 35 watched
///    `"THIRD PARTY\n"` become `"two\n"` on four verbs that said nothing.
/// 2. **The record did not.** The row is left where 43 §7-3b's window leaves it, and the next
///    start-up closes it once the archive takes a file again — so this is not a state an operator
///    has to repair by hand, and the sentence should not frighten them into inventing one.
/// 3. **What was not compared**, and **that this run may have written over somebody else's
///    bytes** — the two halves R35's module header argues a counter cannot carry. They are not less
///    true here; they are more, because on this road there is not even a `Recovered` row for
///    `gx repair --json` to count.
#[must_use]
pub fn applied_unrecorded_note_for(verb: &str, id: &TransformationId) -> String {
    let id = id.0.to_text();
    format!(
        "{verb}: 43 §7-3c wrote its delta and then could not record it — {id}'s delta **was** \
         applied to the substrate, and the step after it raised, so this run moved the world and \
         left no terminal record of having done so. The row stays resumable and 43 §7-3b's window \
         is what closes it: fix what the error beside this sentence names and run a write verb \
         again. What was **not** checked, exactly as on the road that succeeds: whether anything \
         had written to that object between the crash and now — with an `ApplyStarted` in the \
         journal, a fingerprint that differs from the planned one is also what a successful apply \
         looks like (req/78 §3.2 Λ4), and no post-apply fingerprint is recorded (M5H5-3). ∴ if \
         something else wrote to this object after the crash, this run has just written over it \
         and cannot tell you so. This row reaches no `recover` counter, because the recovery did \
         not return one for it (req/476 H-01)"
    )
}

/// 🔴 **`req/496` M-01** — the sentence for the row whose commit **was** recorded and whose head
/// was not.
///
/// # Why R36's sentence could not be reused, and why this is not a wording change
///
/// [`applied_unrecorded_note_for`] tells an operator two things: that the delta landed, and that
/// **no terminal record says so**. The second is the half that decides what they do next, and R36
/// spelled out the consequence itself — "the row stays resumable ... run a write verb again".
///
/// Audit 36 sealed `.gx/checkpoints/` on R36's own bed, so that everything up to and including
/// 43 §7-2's `Committed` record succeeded and only `record_head` raised. The journal's `Committed`
/// count went from 1 to 2 while the operator was told there was none. Then it did what the remedy
/// said: the next `gx repair --yes` answered `terminal: 2, resumed: 0` and printed nothing, because
/// the row the operator had been sent to close had been closed by the run that said it had not
/// been. A remedy that names no reachable action is worse than silence — it spends the one thing
/// an interrupted recovery has, which is the operator's attention.
///
/// # What this one carries
///
/// 1. **The delta landed**, unchanged from the road above: this is still a run that moved somebody's
///    world without being asked.
/// 2. **The record landed too**, which is the correction. The row is `Committed` in the journal and
///    43 §7's recovery has nothing left to do with it.
/// 3. **The head did not move**, which is the one fact that is still wrong on the disk — and it is
///    a fact with a consequence: until a write verb records a head over this tree, `ledger_agrees`
///    style checks and `GET /ledger/checkpoint` see a project behind the head it published.
/// 4. **What was not compared**, identically to both other roads. It is not less true here.
#[must_use]
pub fn recorded_without_head_note_for(verb: &str, id: &TransformationId) -> String {
    let id = id.0.to_text();
    format!(
        "{verb}: 43 §7-3c recorded the commit and could not move the head — {id}'s delta **was** \
         applied to the substrate **and** 43 §7-2's terminal `Committed` record **was** written, \
         so this row is closed and there is nothing left to resume about it. What did not happen \
         is the last write of the sequence: the signed head still describes the tree as it was \
         before this leaf (DR-43-11). Fix what the error beside this sentence names and run a write \
         verb again — not to close this row, which is closed, but so that a head is recorded over \
         the tree that now holds it. What was **not** checked, exactly as on the road that \
         succeeds: whether anything had written to that object between the crash and now — with an \
         `ApplyStarted` in the journal, a fingerprint that differs from the planned one is also \
         what a successful apply looks like (req/78 §3.2 Λ4), and no post-apply fingerprint is \
         recorded (M5H5-3). ∴ if something else wrote to this object after the crash, this run has \
         just written over it and cannot tell you so (req/496 M-01)"
    )
}

/// Say what a `recover` that **raised** had already done, from the engine that still holds it.
///
/// # Why the caller passes an engine rather than rows
///
/// Because the row that matters was never a row. [`gx_engine::pipeline::Engine::recover`] builds
/// `Vec<Recovered>` as it goes and the failing row never reaches it, so a signature taking
/// `&[Recovered]` could not express the finding at all — which is exactly how the fact stayed
/// unsaid through R35's repair, three audits and one `docs/LIMITS.md` paragraph claiming otherwise.
///
/// Both halves are announced: the rows that **finished** before the failing one get R35's sentence
/// (they walked the same road and wrote the same way), and the rows that applied without being
/// recorded get [`applied_unrecorded_note_for`].
pub fn announce_interrupted_recovery<E, C>(verb: &str, engine: &gx_engine::pipeline::Engine<E, C>)
where
    E: gx_engine::EvidenceSource,
    C: gx_engine::Canonicalizer,
{
    let partial = engine.recovery_before_error();
    announce_recovery(verb, &partial.finished);
    for id in &partial.applied_unrecorded {
        crate::note!("{}", applied_unrecorded_note_for(verb, id));
    }
    // 🔴 **R37 / `req/496` M-01** — the third shape, and the reason it is a third loop rather than
    // a branch inside the second: a row is in exactly one of the two lists, so this prints once per
    // row and never twice for the same one.
    for id in &partial.recorded_without_head {
        crate::note!("{}", recorded_without_head_note_for(verb, id));
    }
}
