// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **E-DR4627-1 / DR-46-27** — `GateInput.decided_at` is a seat, and a seat is all it is.
//!
//! # The question this file exists to answer
//!
//! DR-46-27's killer assumption, in the words of `req/454` §3-3: *"the moment you put the judging
//! time into the gate's input, does replay stop being determinate?"* The claim replay rests on is
//! **same input, same verdict** — and a field whose value is "now" is the one kind of input that
//! is different on every run. If a verdict read it, two replays of one recorded transformation
//! would answer differently, and every receipt in the ledger would be a claim about a coin flip.
//!
//! The answer is not "we were careful". It is three measurements, and each one covers a hole the
//! other two leave open:
//!
//! 1. [`ps_1_the_cedar_projection_does_not_read_decided_at`] — the **structural** half. Cedar
//!    cannot read what is not projected to it, and `RequestView::of` is the whole projection.
//!    Source-scanned, because a struct's provenance is not available at run time.
//! 2. [`varying_decided_at_alone_moves_no_verdict`] — the **behavioural** half. Everything else
//!    held still, `decided_at` swept across five values spanning `i64`, all three verdict arms:
//!    the `Verdict`s compare equal. This is the direct form of "replay is determinate in
//!    `decided_at`".
//! 3. [`the_seat_reaches_a_registered_invariant_and_still_moves_no_verdict`] — the **wiring**
//!    half, and the one that stops 2 from being a lie of omission. A field nothing ever reads is
//!    trivially inert; the point of DR-46-27 is a field an invariant *can* read. So an invariant
//!    is registered that does read it, records what it saw, and returns a constant. 2 stays true
//!    *with that invariant in the registry*, and 3 proves the five distinct values really did
//!    arrive — which is what makes 2's invariance a fact about the verdict rather than a fact
//!    about a field that never left the struct.
//!
//! # What this file does not claim, said plainly
//!
//! **It is not a law. It is a measurement of v0.** DR-46-27 ships the seat and no window
//! predicate. The day someone writes the `now ∈ [a, b]` invariant that DR-46-27 exists to make
//! possible, test 2 is **supposed to go red**: at that moment `decided_at` starts moving verdicts
//! *on purpose*, and a green here would mean the new predicate does not work. `req/454` §4 says it
//! in as many words — "a future time-window DR breaking this differential test on purpose is the
//! correct way forward; do not misread it then as 'it broke, so it failed'".
//!
//! Test 1 is the opposite: **it is a law**, and `req/454` §4 asks in as many words that it not be
//! deleted as "a v0-only test". PS-1 (do not grow Cedar's vocabulary) is a decision about the
//! policy language, not about this milestone; a later DR that gives `decided_at` to an invariant
//! must still not give it to Cedar, and if that is ever reversed it is reversed by a DR that
//! rewrites this test deliberately, not by a projection that quietly grows a line.
//!
//! **It does not stop a deployment's invariant from reading a clock of its own.** `req/447` §5-5,
//! carried verbatim into DR-46-27 §2-4: nothing in the type system prevents an
//! `impl InvariantCheck` from calling `SystemTime::now()`, and that was as true before this field
//! as after it. This field offers a lawful road in; it closes no other road.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::verdict::InvariantResult;
use gx_gate::{Gate, GateInput, InvariantCheck, InvariantRegistry, PolicyEngine, Result, Verdict};
use gx_witness::Evidence;

/// Plan time for every fixture below: `t.created_at`, the clock that was already reachable.
///
/// It is a *named* constant so the sweep below can be read against it — the three interesting
/// values in `req/454` §3-3 are "before `t.created_at`", "equal to it" and "after it", and a bare
/// literal would hide which is which.
const PLANNED_AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The five `decided_at` values swept, in the order they are swept.
///
/// Three are `req/454` §3-3's N >= 3 (before / equal / after plan time). The other two are the
/// ends of the type. They are here because "three values near each other" and "the whole domain"
/// fail differently: a projection that bucketed the timestamp — an hour-of-day rule, a sign test,
/// an epoch-window comparison — could easily agree across three neighbouring nanosecond values and
/// disagree across `i64::MIN` and `i64::MAX`. Sweeping the ends costs one line and closes that.
const SWEEP: [Timestamp; 5] = [
    Timestamp(PLANNED_AT.0 - 86_400_000_000_000),
    PLANNED_AT,
    Timestamp(PLANNED_AT.0 + 86_400_000_000_000),
    Timestamp(i64::MIN),
    Timestamp(i64::MAX),
];

/// Permits everything except `/etc/*`, so the sweep below can reach `Admit` and `Deny` by locator
/// alone — the same fixture shape `verdict_meet.rs` uses, for the same reason.
const PACK: &str = r#"@id("permit-default")
permit (principal, action, resource);

@id("forbid-etc")
forbid (principal, action, resource)
when { resource.locator like "/etc/*" };
"#;

// ---------------------------------------------------------------------------
// 1. The structural half: PS-1, source-scanned
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

/// The body of a function, from its signature line to the `}` that closes it.
///
/// Brace-counted rather than indentation-matched, so a `rustfmt` run that re-wraps the projection
/// cannot silently shorten what this test looks at. The same instrument shape as
/// `gate_input_spec.rs`'s [`fields_of`] — the source is the only place a projection's *shape* is
/// visible, since at run time all that exists is the `RequestView` it produced.
fn body_of(text: &str, signature: &str) -> String {
    let start = text
        .find(signature)
        .unwrap_or_else(|| panic!("no line reads `{signature}` -- this test reads the wrong file"));
    let mut depth = 0usize;
    let mut opened = false;
    for (offset, ch) in text[start..].char_indices() {
        match ch {
            '{' => {
                depth += 1;
                opened = true;
            }
            '}' => {
                depth -= 1;
                if opened && depth == 0 {
                    return text[start..start + offset + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("the `{signature}` block is not closed");
}

/// The reads `req/454` §3-3 enumerates, in one place so the presence arm and the completeness arm
/// cannot drift apart (**R36**, `req/476` L-01).
const DECLARED_READS: [&str; 7] = [
    "input.pre.locator()",
    "input.pre.substrate()",
    "input.evidence",
    "input.t.order()",
    "input.invert_available",
    "input.t.actor.key()",
    "input.t.context",
];

/// 🔴 **R36 / `req/476` L-01** — every `input.…` path a body reads, sorted and de-duplicated.
///
/// The unit is the **path**, not the field: `input.t.order()` and `input.t.context` are two reads
/// of one field and are counted as two, because Cedar's vocabulary grows by the term that reaches
/// it rather than by the struct member it came from. A projection that started putting
/// `input.t.deadline` into the context would be reading a field the list already knows (`t`) and
/// would still be adding a term — this is what catches it.
///
/// The scan takes identifier characters, `.` and `()` after the literal `input.`, which is the
/// grammar every read in this projection is written in. It is deliberately not a Rust parser: the
/// projection is one function whose whole job is to build a map out of one struct, and a scan whose
/// failure mode is "sees too much" is the right kind — an extra member fails the comparison and is
/// read by a human, rather than being silently dropped.
fn reads_of(body: &str) -> Vec<String> {
    const NEEDLE: &str = "input.";
    let bytes = body.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut from = 0usize;
    while let Some(at) = body[from..].find(NEEDLE) {
        let start = from + at;
        let mut end = start + NEEDLE.len();
        while end < bytes.len() {
            let ch = bytes[end] as char;
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                end += 1;
            } else if ch == '(' && end + 1 < bytes.len() && bytes[end + 1] == b')' {
                // The read ends at its **first** call. What is done with the value afterwards is
                // this function's business — `.to_string()`, `.clone()` — and folding those in
                // would make the declared list depend on how the projection copies its data
                // rather than on which terms it reads.
                end += 2;
                break;
            } else {
                break;
            }
        }
        // A trailing `.` belongs to the sentence, not to the read (`input.evidence.` in prose).
        let mut read = &body[start..end];
        while read.ends_with('.') {
            read = &read[..read.len() - 1];
        }
        out.push(read.to_string());
        from = end;
    }
    out.sort();
    out.dedup();
    out
}

/// 🔴 **R37 / `req/496` L-01** — the second half of the completeness arm: the spellings a read
/// could take that [`reads_of`] would **not** see.
///
/// # Why this exists instead of a bigger scanner
///
/// R36 wrote beside `reads_of` that "there is no spelling it could take that this would not see".
/// Audit 36 spelled one read three ways and the scan saw none of them:
///
/// ```text
/// let g = input;  let _ = g.judged_at.0;                  // rebound
/// let GateInput { judged_at, .. } = *input;                // destructured
/// let _ = input . judged_at . 0;                           // whitespace around the dot
/// ```
///
/// Two repairs were available. Growing the extractor until it handles rebinding is growing it into
/// a Rust borrow-checker, and the next audit writes a fourth spelling; the claim would keep being
/// larger than the instrument. The other is to notice what the extractor actually is — a check on a
/// **convention**, that the projection reads its input directly — and to make the convention
/// checkable rather than assumed.
///
/// That is what this is. Every spelling of a new read is now in exactly one of two buckets: the
/// extractor **sees** it, or this refuses the projection for taking a shape the extractor cannot
/// read. There is no third bucket, and that is the property `ps_1c` drives.
///
/// The returned strings are what a reader is shown, so they name the shape rather than the scan.
fn spellings_the_scan_cannot_see(body: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();

    // (a) The whole `input` bound to a second name — by value, by reference or by dereference.
    //     Every read after it goes through the new name and carries no `input.` at all.
    if compact.contains("=input;") || compact.contains("=&input;") || compact.contains("=*input;") {
        out.push("a rebound local: `let g = input;` and then `g.<field>`");
    }
    // (b) A pattern that takes the fields out and binds them as bare names, which is the same
    //     escape with the binding done by the pattern instead of by a second statement.
    if compact.contains("}=input") || compact.contains("}=&input") || compact.contains("}=*input") {
        out.push("a destructuring `let`: `let GateInput { judged_at, .. } = *input;`");
    }
    // 🔴 **R38 / `req/513` L-01 (a)** — a parenthesised dereference or reference.
    //
    // `reads_of`'s needle is the literal `input.`, and `(*input).judged_at` puts a `)` between the
    // name and its dot, so the scan sees nothing. It is not a rebinding — the read is direct — but
    // it is the same *class* of failure this function exists for: a spelling of a direct read that
    // the extractor's one needle cannot reach. `rustfmt` leaves it alone and the borrow checker
    // asks for it, which is what makes it a spelling a human writes rather than one an attacker
    // constructs.
    if compact.contains("(*input)") || compact.contains("(&input)") {
        out.push("a parenthesised dereference: `(*input).<field>`");
    }
    // 🔴 **R38 / `req/513` L-01 (b)** — `input` passed whole to a call.
    //
    // The three shapes above are all *assignments of `input` itself*, so `let g = borrow(input);`
    // walks between them: the rebinding is done by the callee's parameter. Rather than enumerate a
    // fourth assignment shape and wait for a fifth, this refuses the antecedent — the convention
    // being checked is that **the projection reads its input directly**, and a projection that
    // hands `input` to a function has stopped doing that whatever the function is.
    //
    // Bare `input` only: `f(&input.t.context)` is a read of a field, which `reads_of` sees, and the
    // shipped projection contains one (`action_id(&input.t.context)`). The control arm in `ps_1c`
    // is what holds that distinction to a measurement instead of to this comment.
    if compact.contains("(input)")
        || compact.contains("(input,")
        || compact.contains(",input)")
        || compact.contains(",input,")
    {
        out.push("`input` passed whole to a call: `let g = borrow(input);` and then `g.<field>`");
    }
    // (c) Whitespace between the name and its dot. `rustfmt` does not write this and a human can.
    let bytes = body.as_bytes();
    let mut from = 0usize;
    while let Some(at) = body[from..].find("input") {
        let start = from + at;
        let end = start + "input".len();
        from = end;
        // `inputs`, `input_view` and the like are other names, not this one.
        if bytes
            .get(end)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            continue;
        }
        let mut scan = end;
        while bytes.get(scan).is_some_and(u8::is_ascii_whitespace) {
            scan += 1;
        }
        if scan > end && bytes.get(scan) == Some(&b'.') {
            out.push("whitespace between `input` and its dot: `input . judged_at . 0`");
            break;
        }
    }
    out
}

/// 🔴 **PS-1**: Cedar's seven-tuple does not include `decided_at`, and this is why replay is safe.
///
/// The projection `RequestView::of` performs is the entire surface Cedar sees: a policy names a
/// principal, an action, a resource with its attributes, and a context, and every one of those is
/// built in that function out of `GateInput`. A field the function does not touch is a field no
/// policy in any pack a stranger could send can mention, which is a stronger statement than "our
/// packs do not mention it".
///
/// The test has two arms and needs both. The **absence** arm is the claim. The **presence** arm —
/// asserting the seven things the projection *does* read are all still named there — is what stops
/// the absence arm from passing vacuously: a refactor that moved the projection elsewhere, renamed
/// the function, or gutted it to a `todo!()` would satisfy "no `decided_at` here" perfectly while
/// destroying the property being measured.
#[test]
fn ps_1_the_cedar_projection_does_not_read_decided_at() {
    let policy = std::fs::read_to_string(workspace_root().join("crates/gx-gate/src/policy.rs"))
        .expect("policy.rs is readable");
    let projection = body_of(&policy, "pub fn of(input: &GateInput<'_>) -> Self {");

    // The presence arm: the seven reads `req/454` §3-3 enumerates are all still here, so the
    // absence arm below is about a live projection rather than about an empty one.
    for read in DECLARED_READS {
        assert!(
            projection.contains(read),
            "`RequestView::of` no longer reads {read:?}. Either the seven-tuple changed -- which is \
             a 41 §4 / ASM-60-1 decision and not a refactor -- or this test is now scanning \
             something that is no longer the Cedar projection, in which case the absence assertion \
             below has stopped measuring anything"
        );
    }

    // 🔴 **R36 / `req/476` L-01 — and that those are *all* of them.**
    //
    // The arm above asks whether seven reads are **present**. Audit 35 measured what it does not
    // ask: whether they are the whole list. A later DR that adds a field to `GateInput`, has
    // `RequestView::of` put it into `context`, and names it anything other than `decided_at` passes
    // the presence arm (all seven are still there), passes the absence arm (the literal is not
    // there), and passes the differential sweep below (which only varies `decided_at`, so the new
    // term is constant across every case). Three green tests, and Cedar has grown a term.
    //
    // The completeness arm closes it by extraction rather than by enumeration: every `input.…` path
    // in the projection is read out of the source and the **set** is compared with the declared
    // one. A new read written the way this projection writes its reads is a new member of that set.
    //
    // 🔴 **R37 / `req/496` L-01** — and the sentence that used to stand here said more than that:
    // "there is no spelling it could take that this would not see". Audit 36 wrote three spellings
    // it does not see (`req/496` §4-5). The extractor is unchanged and the claim is now the true
    // one — it sees the reads written in this projection's convention — with the convention itself
    // checked one line down, so that a read spelled outside it is refused rather than invisible.
    let unreadable = spellings_the_scan_cannot_see(&projection);
    println!("PS_1_CONVENTION unreadable={unreadable:?}");
    assert!(
        unreadable.is_empty(),
        "🔴 `req/496` L-01: `RequestView::of` is written in a shape the completeness arm below \
         cannot read — {unreadable:?}. The arm extracts `input.<path>` out of this function's \
         source, so a read that reaches its field through a second binding, through a destructuring \
         pattern, or with whitespace around its dot is a term Cedar has grown and this test cannot \
         see. Write the read as `input.<path>`, or change the arm and this guard together and say \
         in the DR which spellings the new one covers"
    );

    let found = reads_of(&projection);
    let declared: Vec<String> = {
        let mut d: Vec<String> = DECLARED_READS.iter().map(|s| (*s).to_string()).collect();
        d.push("input.evidence.len()".to_string());
        d.sort();
        d.dedup();
        d
    };
    println!("PS_1_READS found={found:?}");
    assert_eq!(
        found, declared,
        "🔴 `req/476` L-01: `RequestView::of` reads a different set of `GateInput` fields than the \
         seven `req/454` §3-3 enumerates. If a field was added to the projection, Cedar's \
         vocabulary has grown and PS-1's question has to be asked about the new term as well — \
         which is a DR's decision, and this test is where it is recorded. If a read was removed, \
         the projection is smaller than the document says it is"
    );

    // The absence arm: PS-1 itself.
    assert!(
        !projection.contains("decided_at"),
        "PS-1 is broken: `RequestView::of` now reads `decided_at`, so a Cedar policy can branch on \
         the judging time. Two things follow immediately and neither is acceptable without a DR \
         that says so: replay of a recorded transformation can answer differently from the run \
         that was recorded, and the policy language has grown a term DR-46-27 decided not to add. \
         If this is intended, the DR that intends it rewrites this test; it does not delete it"
    );

    // And the whole file, not just the one function -- a helper that reached the field and fed it
    // into `context` from two lines away would pass the arm above.
    assert!(
        !policy.contains("decided_at"),
        "`policy.rs` names `decided_at` somewhere outside `RequestView::of`. The projection is the \
         only road into Cedar, but a helper it calls is part of the projection: PS-1 is a property \
         of this file, not of one function in it"
    );

    println!(
        "PS_1_SCAN projection_bytes={} decided_at_in_projection=false decided_at_in_policy_rs=false",
        projection.len()
    );
}

/// 🔴 **R36 / `req/476` L-01, the mutant the audit said it had not built.**
///
/// Audit 35 §7-4, verbatim in its own grading: *"mutant was not built ... this is a source reading
/// and not a measurement"*. It was a read-only audit and could not add a field to `GateInput`, so
/// the claim "all three tests would stay green" was an argument. This drives it instead, without
/// touching the shipped struct: the mutation is applied to a **copy of the projection's text**, and
/// both arms — the one that shipped and the one R36 added — are run against it.
///
/// The two answers are the finding and its repair, side by side:
///
/// * the presence arm answers **pass** on a projection that has grown a term Cedar can read;
/// * the completeness arm answers **fail**, and names it.
///
/// This is a test about a test, which is worth one sentence of justification: `ps_1` is a **law**
/// (this file's header, and `req/454` §4 asks that it not be deleted as a v0-only test), and a law
/// enforced by a check nobody has ever seen refuse is a law on paper. This is the refusal.
#[test]
fn ps_1b_the_completeness_arm_catches_a_read_the_presence_arm_does_not() {
    let policy = std::fs::read_to_string(workspace_root().join("crates/gx-gate/src/policy.rs"))
        .expect("policy.rs is readable");
    let projection = body_of(&policy, "pub fn of(input: &GateInput<'_>) -> Self {");

    // The mutation: a later DR adds a field to `GateInput` and the projection puts it into the
    // context. It is **not** called `decided_at` — that is the whole point, since the absence arm
    // looks for that literal and would catch it.
    let mutant = projection.replace(
        "context.insert(CONTEXT_EVIDENCE_KINDS.to_string(), ContextValue::Set(kinds));",
        "context.insert(CONTEXT_EVIDENCE_KINDS.to_string(), ContextValue::Set(kinds));\n        \
         context.insert(\"judged_at\".to_string(), ContextValue::Long(input.judged_at.0));",
    );
    assert_ne!(
        mutant, projection,
        "instrument: the mutation did not apply, so this test measured the unmutated projection. \
         The anchor line is in `RequestView::of` and moves when that function is re-written"
    );

    // Arm 1 — what shipped before R36. Every one of the seven is still present, so it passes.
    let presence_passes = DECLARED_READS.iter().all(|read| mutant.contains(read));
    // The absence arm passes too: the new field is not spelt `decided_at`.
    let absence_passes = !mutant.contains("decided_at");
    // Arm 2 — R36's. The extracted set has a member the declared list does not.
    let found = reads_of(&mutant);
    let unexpected: Vec<&String> = found
        .iter()
        .filter(|read| {
            !DECLARED_READS.contains(&read.as_str()) && read.as_str() != "input.evidence.len()"
        })
        .collect();
    println!(
        "PS_1B_MUTANT presence_arm_passes={presence_passes} absence_arm_passes={absence_passes} \
         completeness_arm_catches={} unexpected={unexpected:?}",
        !unexpected.is_empty()
    );

    assert!(
        presence_passes && absence_passes,
        "the two arms that shipped must **pass** on this mutant — if they refuse it, the hole \
         `req/476` L-01 describes is not the hole this test is measuring"
    );
    assert_eq!(
        unexpected.len(),
        1,
        "🔴 the completeness arm must refuse the projection the other two accept, and name exactly \
         the read that was added: {found:?}"
    );
    assert_eq!(unexpected[0], "input.judged_at.0");
}

/// 🔴 **R37 / `req/496` L-01** — the three spellings audit 36 wrote, each one **seen or refused**.
///
/// `ps_1b` above drives the spelling the extractor was built for and proves the arm refuses it.
/// This drives the three it was not built for, and proves the disjunction that replaced R36's
/// universal claim: for every spelling here, either [`reads_of`] gains a member (the arm catches
/// the new read) or [`spellings_the_scan_cannot_see`] names the shape (the projection is refused
/// for being unreadable). A spelling in **neither** bucket is a term Cedar can grow with three
/// green tests, which is what `req/496` §4-5 measured.
///
/// The control is the unmutated projection: it must be in the *first* bucket's clean state — no
/// unreadable shape and no unexpected read — or this suite is measuring a bed that was already
/// broken.
#[test]
fn ps_1c_every_spelling_is_either_seen_or_refused() {
    let policy = std::fs::read_to_string(workspace_root().join("crates/gx-gate/src/policy.rs"))
        .expect("policy.rs is readable");
    let projection = body_of(&policy, "pub fn of(input: &GateInput<'_>) -> Self {");
    let base = reads_of(&projection);

    // The control, both ways round.
    let control_unreadable = spellings_the_scan_cannot_see(&projection);
    println!(
        "PS_1C control base_reads={} unreadable={control_unreadable:?}",
        base.len()
    );
    assert!(
        control_unreadable.is_empty(),
        "the bed failed before the product did: the shipped projection already trips the guard"
    );

    // R36's own spelling, which the extractor is built for. Kept as the fourth case so that this
    // suite says which bucket each spelling landed in rather than only that it landed somewhere.
    let cases: [(&str, String); 6] = [
        (
            "input_dot",
            format!("{projection}\n    let _grown = input.judged_at.0;\n"),
        ),
        // 🔴 **R38 / `req/513` L-01** — the two spellings audit 37 wrote, which landed in neither
        // bucket. `reads_of`'s needle is the literal `input.`, and a parenthesised dereference puts
        // a `)` between the name and its dot; the guard's three shapes are all *assignments of
        // `input` itself*, and passing `input` whole to a call rebinds it through the callee's
        // parameter instead. Both are shapes `rustfmt` leaves alone and a borrow checker asks for.
        (
            "parenthesised_deref",
            format!("{projection}\n    let _grown = (*input).judged_at.0;\n"),
        ),
        (
            "rebound_through_a_call",
            format!(
                "{projection}\n    let g = std::convert::identity(input);\n    \
                 let _grown = g.judged_at.0;\n"
            ),
        ),
        (
            "rebound_local",
            format!("{projection}\n    let g = input;\n    let _grown = g.judged_at.0;\n"),
        ),
        (
            "destructured",
            format!(
                "{projection}\n    let GateInput {{ judged_at, .. }} = *input;\n    \
                 let _grown = judged_at.0;\n"
            ),
        ),
        (
            "whitespace_around_the_dot",
            format!("{projection}\n    let _grown = input . judged_at . 0;\n"),
        ),
    ];

    let mut invisible: Vec<&str> = Vec::new();
    for (name, text) in &cases {
        let seen = reads_of(text).len() > base.len();
        let refused = !spellings_the_scan_cannot_see(text).is_empty();
        println!(
            "PS_1C spelling={name} seen_by_the_extractor={seen} refused_by_the_guard={refused}"
        );
        if !seen && !refused {
            invisible.push(name);
        }
    }

    assert!(
        invisible.is_empty(),
        "🔴 `req/496` L-01: {invisible:?} reach `GateInput` without the extractor seeing the read \
         and without the convention guard refusing the shape. That is a term Cedar has grown with \
         every arm of `ps_1` still green, which is the hole this pair was built to close"
    );
}

// ---------------------------------------------------------------------------
// 2 + 3. The behavioural half, and the invariant that makes it non-vacuous
// ---------------------------------------------------------------------------

/// The one invariant DR-46-27 ships in v0, and it is a **stub on purpose**.
///
/// It does two things and neither is a judgement. It *reads* `decided_at`, which is the whole
/// point — DR-46-27 exists so that an invariant can — and it records what it read, so that
/// [`the_seat_reaches_a_registered_invariant_and_still_moves_no_verdict`] can show the values
/// really arrived. What it returns is a constant.
///
/// Why a constant and not a window: `req/454` §2-2 puts the window predicate outside v0, and this
/// is the reason stated mechanically. If `check` returned `holds: decided_at ∈ [a, b]`, then
/// [`varying_decided_at_alone_moves_no_verdict`] would be measuring an invariant that is *designed*
/// to move the verdict, and the KA would be unanswerable in the same file. Splitting them is what
/// lets the report say which of the two facts is doing the work: **the verdict is unchanged
/// because nothing derives from the field, not because the field failed to arrive.**
///
/// Note also where this lives: `crates/gx-gate/tests/`, not `crates/gx-gate/src/`. `pack_embedding
/// .rs`'s `the_shipped_artifact_carries_no_invariant` scans `crates/gx-gate/src/` for
/// `impl InvariantCheck for` and requires **zero** — D-9, "no ready-made invariant ships". A
/// sample invariant in `src` would ship one and break that gate; a sample invariant in a test file
/// demonstrates the road without putting anything in the artifact, which is exactly the
/// distinction D-9 draws.
struct RecordsTheMoment {
    id: &'static str,
    seen: Arc<Mutex<Vec<Timestamp>>>,
}

impl InvariantCheck for RecordsTheMoment {
    fn id(&self) -> &str {
        self.id
    }

    fn check(&self, input: &GateInput<'_>) -> Result<InvariantResult> {
        self.seen
            .lock()
            .expect("no other thread holds this in a single-threaded test")
            .push(input.decided_at);
        // A constant, and independent of what was just recorded. Deliberately not
        // `holds: input.decided_at >= something` -- see the type doc.
        InvariantResult::new(
            self.id.to_string(),
            true,
            Some("this invariant reads the moment and decides nothing by it".to_string()),
        )
    }
}

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn transformation(order: u8) -> Transformation {
    Transformation::new(
        TransformationId(cid(1)),
        order,
        Subject::Object(ObjectId(cid(2))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context: ChangeContext::Substrate,
            actor: Actor::Human {
                key: "key-human-1".to_string(),
            },
            created_at: PLANNED_AT,
        },
    )
    .expect("order is within ASM-6's 2")
}

fn snapshot(locator: &str) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(cid(2)),
        SubstrateKind::Fs,
        locator.to_string(),
        cid(6),
        ReprKind::Bytes,
    )
}

/// Judge one fixture at one `decided_at`, with `seen` collecting what the invariant received.
fn judge(
    locator: &str,
    invert_available: bool,
    decided_at: Timestamp,
    seen: &Arc<Mutex<Vec<Timestamp>>>,
) -> Result<Verdict> {
    let mut registry = InvariantRegistry::new();
    registry
        .register(Box::new(RecordsTheMoment {
            id: "dr-46-27-sample",
            seen: Arc::clone(seen),
        }))
        .expect("one invariant, one id");
    let gate = Gate::with_policies(PolicyEngine::parse(PACK).expect("the fixture pack parses"))
        .with_invariants(registry);

    let t = transformation(0);
    let pre = snapshot(locator);
    let planned = PlannedDeltaBytes(vec![0xde, 0xad]);
    let evidence: [Evidence; 0] = [];

    gate.verify(GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &evidence,
        invert_available,
        decided_at,
    })
}

/// 🔴 **The KA, answered**: the same input at five different judging moments gives one verdict.
///
/// The experiment holds `t`, `pre`, `planned`, `evidence` and `invert_available` fixed and moves
/// `decided_at` alone, which is the definition of a differential test: anything that comes out
/// different is caused by the one thing that went in different. Five values, spanning the whole of
/// `i64` (see [`SWEEP`]), against all three arms of `Verdict`:
///
/// * `Admit` — a permitted locator with an inverse available;
/// * `Deny`  — `/etc/*`, refused by the pack's second policy;
/// * `Escalate` — `invert_available: false`, which **E-M3-4** makes the one v0.1 condition for it.
///
/// All three arms matter, because they are built by different code: `Admit` assembles an
/// `AdmitProof` out of the policy decisions, the invariant results and the evidence digests;
/// `Escalate` builds an `EscalationTicket` whose identity is a CID over its projection. A field
/// that leaked into any of those would move the value here, and `Verdict` derives `PartialEq`
/// structurally, so "did not move" means every reason code, every recorded decision, every digest
/// and every id — not merely the same discriminant.
///
/// **On the day this test goes red.** It is evidence about v0, not a law: DR-46-27 ships no window
/// predicate, so nothing may derive from the field *yet*. The DR that writes the first real
/// `now ∈ [a, b]` invariant is expected to break this test and to say so in its own report. Red
/// here means "a verdict now depends on the judging time" — the question is only ever whether that
/// was decided or whether it leaked.
#[test]
fn varying_decided_at_alone_moves_no_verdict() {
    for (case, locator, invert_available, expected) in [
        ("admit", "workspace/a.txt", true, VerdictKind::Admit),
        ("deny", "/etc/shadow", true, VerdictKind::Deny),
        ("escalate", "workspace/a.txt", false, VerdictKind::Escalate),
    ] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut answers: Vec<Verdict> = Vec::new();
        for decided_at in SWEEP {
            answers.push(
                judge(locator, invert_available, decided_at, &seen)
                    .expect("the fixture is evaluable at every moment"),
            );
        }

        // The arm is the one intended, so that a fixture that silently answered `Deny` everywhere
        // could not pass this as "invariant".
        assert_eq!(
            answers[0].kind(),
            expected,
            "the {case} fixture no longer produces {expected:?}; the sweep below would then be \
             measuring invariance of the wrong verdict"
        );

        for (i, answer) in answers.iter().enumerate().skip(1) {
            assert_eq!(
                answer, &answers[0],
                "case {case}: the verdict at decided_at={:?} differs from the verdict at {:?}, and \
                 those two runs differ in nothing else. Replay is no longer determinate in \
                 `decided_at`: a receipt recorded under one clock reading can be re-derived into a \
                 different verdict under another",
                SWEEP[i], SWEEP[0]
            );
        }

        println!(
            "DIFFERENTIAL case={case} kind={:?} sweep={} identical=true",
            answers[0].kind(),
            SWEEP.len()
        );
    }
}

/// 🔴 The wiring: the five values reached a registered invariant, and the verdict still did not
/// move.
///
/// This is the assertion that stops the one above from being a lie of omission. "Varying a field
/// changes nothing" is trivially true of a field that never leaves the struct, and DR-46-27's whole
/// claim is that the field *does* leave it — that an invariant a deployment registers can read the
/// moment it is being asked. So the two facts are separated: the invariant records every
/// `decided_at` it is handed, and this test asserts the recording is exactly [`SWEEP`], in order,
/// once each.
///
/// Read together with [`varying_decided_at_alone_moves_no_verdict`], the pair says: **the verdict
/// is unchanged because nothing derives a decision from the field, not because the field failed to
/// arrive.** `req/454` §3-3 calls that the distinction the third piece exists to draw, and it is
/// also the "shipping it changes what is admitted by zero" shape `req/447` AC-PP-06 uses — the
/// count of admissions across the sweep is one value, five times.
#[test]
fn the_seat_reaches_a_registered_invariant_and_still_moves_no_verdict() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut admitted = 0usize;
    for decided_at in SWEEP {
        let verdict = judge("workspace/a.txt", true, decided_at, &seen)
            .expect("the fixture is evaluable at every moment");
        if verdict.kind() == VerdictKind::Admit {
            admitted += 1;
        }
    }

    let observed = seen.lock().expect("single-threaded").clone();
    assert_eq!(
        observed,
        SWEEP.to_vec(),
        "the invariant did not receive the five moments that were handed to `Gate::verify`, in \
         order and once each. Either the field is not reaching the registry -- in which case \
         DR-46-27 delivered a struct field and not a seat, and the differential test above is \
         vacuous -- or the registry called it a different number of times, which is AC-026's \
         subject and is broken"
    );
    assert!(
        observed.iter().any(|m| *m != PLANNED_AT),
        "every moment the invariant saw equals `t.created_at`. That is the failure DR-46-27 was \
         raised to fix: plan time was already reachable through `t`, and a `decided_at` that only \
         ever carries plan time is the old reachability wearing a new field name"
    );

    // AC-PP-06's shape: shipping the seat changes what is admitted by zero.
    assert_eq!(
        admitted,
        SWEEP.len(),
        "the number of admissions moved across the sweep, so the seat changed what is admitted"
    );

    println!(
        "WIRING observed={} distinct_from_plan_time={} admitted={admitted}/{}",
        observed.len(),
        observed.iter().filter(|m| **m != PLANNED_AT).count(),
        SWEEP.len()
    );
}
