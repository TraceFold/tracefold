// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **H-2** — the locator this adapter hands a gate is the **normalised** one (**M7 hand 4**).
//!
//! req/98 §3-4 row 3 is what M7 owes on `req/38` §25's H-2: "the git/mcp adapter normalises the locator
//! before handing it to the gate, fixed by an **adapter-side probe** (in-gate normalisation is not adopted)". This file is the git half (sem: SEM-gx-adapter-git-102)
//! and `crates/gx-adapter-mcp/tests/h2_normalised_before_the_gate.rs` is the mcp one; they are the
//! same three probes over two grammars, because H-2 is a claim about a **road** and both adapters
//! stand on the same one:
//!
//! > `Engine::plan` calls `adapter.snapshot(intent.locator())` and hands the result to `Gate::verify`
//! > as `pre` (`crates/gx-engine/src/pipeline.rs`), and `RequestView` reads `pre.locator()` into
//! > Cedar's `resource.locator`.
//!
//! What makes the claim worth measuring on **this** substrate is clause 2 of the crate root: an
//! unqualified reference name is a branch, and `tags/v1` and `remotes/origin/main` name their
//! namespaces already. So `tags/v1.0.0` and `refs/tags/v1.0.0` are one position spelled two ways —
//! and the shipped git pack's forbid is written on the second spelling. Without normalisation the
//! first would be a road around it, which is precisely M3-10's "locator level" turning two spellings (sem: SEM-gx-adapter-git-103)
//! into two policy subjects.

mod support;

use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
use gix::refs::{FullName, Target};
use gx_adapter_git::locator;
use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_gate::{packs, Gate, GateInput};
use gx_substrate::SubstrateAdapter;
use support::{GitFixture, SUBJECT};

/// A reference at `main`'s starting commit: a position the shipped pack **refuses**.
///
/// The fixture does not carry one because nothing before this hand needed a refused position —
/// `git_conformance.rs` works on branches, which is what the pack deliberately permits. Both
/// namespaces the forbid names are made here, so the probes below cover both disjuncts.
///
/// 🔴 Written through a transaction carrying an explicit committer rather than through
/// `Repository::reference`, which takes the identity from the repository's configuration: this
/// sandbox has none, so that call answers `CreateOrUpdateRefLog(MissingCommitter)` for any reference
/// git keeps a reflog for. It looked like a namespace restriction for an hour — `refs/tags/*` went
/// in and `refs/remotes/*` did not — because **git does not log tags**. The lesson is the one §30
/// keeps re-teaching: a refusal that correlates with the thing you are studying is not evidence
/// about the thing you are studying.
fn point_at_origin(fixture: &GitFixture, name: &str) {
    let repo = fixture.sandbox().repository();
    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: "h2 probe".into(),
            },
            expected: PreviousValue::Any,
            new: Target::Object(fixture.sandbox().origin()),
        },
        name: FullName::try_from(name).expect("a full reference name"),
        deref: false,
    };
    repo.refs
        .transaction()
        .prepare(
            vec![edit],
            gix::lock::acquire::Fail::Immediately,
            gix::lock::acquire::Fail::Immediately,
        )
        .unwrap_or_else(|e| panic!("{name}: the reference transaction prepares: {e}"))
        .commit(support::fixture_signature().to_ref(&mut gix::date::parse::TimeBuf::default()))
        .unwrap_or_else(|e| panic!("{name}: the reference transaction commits: {e}"));
}

/// Spellings of the two refused positions, and what each one exercises.
///
/// Each row is `(clause, raw spelling, the normal form it folds to)`. **None of them is spelled the
/// way the pack's patterns are written**, which is the point: a pattern can only be written on the
/// full spelling, because that is the only unambiguous one.
fn spellings(fixture: &GitFixture) -> Vec<(&'static str, String, String)> {
    let dir = fixture.sandbox().dir().display().to_string();
    let tag = format!("{dir}#refs/tags/v1.0.0:{SUBJECT}");
    let remote = format!("{dir}#refs/remotes/origin/main:{SUBJECT}");
    vec![
        (
            "clause 2: an unqualified `tags/…` is the tag namespace, spelled in full",
            format!("{dir}#tags/v1.0.0:{SUBJECT}"),
            tag.clone(),
        ),
        (
            "clause 2, the forbid's other disjunct: `remotes/…`",
            format!("{dir}#remotes/origin/main:{SUBJECT}"),
            remote,
        ),
        (
            "clause 4: duplicate separators collapse",
            format!("{dir}#refs//tags/v1.0.0:{SUBJECT}"),
            tag.clone(),
        ),
        (
            "clauses 1, 3 and 5: the repository folds, a `..` cancels, a path `./` goes",
            format!("{dir}/.#tags/v1.0.0/x/..:./{SUBJECT}"),
            tag,
        ),
    ]
}

fn change() -> Transformation {
    Transformation::new(
        TransformationId(Cid([0u8; 32])),
        0,
        Subject::Object(ObjectId(Cid([1u8; 32]))),
        None,
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(Cid([1u8; 32])),
            delta: DeltaRef {
                substrate: SubstrateKind::Git,
                cid: Cid([1u8; 32]),
            },
            context: ChangeContext::Substrate,
            actor: Actor::Agent {
                key: "key-agent-1".to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("orders 0..=2 are admitted")
}

fn verdict_for(gate: &Gate, pre: &ObjectSnapshot) -> VerdictKind {
    let t = change();
    let planned = PlannedDeltaBytes(b"opaque to everything below the adapter (P-6)".to_vec());
    gate.verify(GateInput {
        t: &t,
        pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
        // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
        // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
        // varying this field alone moves no verdict) about the field and not about this fixture.
        decided_at: Timestamp(0),
    })
    .expect("the shipped pack evaluates this request")
    .kind()
}

/// A snapshot carrying a locator **exactly as written** — what an adapter that did not normalise
/// would hand a gate.
fn snapshot_spelled(locator: &str) -> ObjectSnapshot {
    ObjectSnapshot::new(
        ObjectId(Cid([1u8; 32])),
        SubstrateKind::Git,
        locator.to_string(),
        Cid([2u8; 32]),
        ReprKind::Bytes,
    )
}

// ---------------------------------------------------------------------------
// 1. The value that reaches `pre`
// ---------------------------------------------------------------------------

/// Whatever spelling `snapshot` is asked about, the snapshot it returns names the **normal form**.
#[test]
fn snapshot_reports_the_normal_form_of_whatever_spelling_it_was_asked_about() {
    let fixture = GitFixture::new();
    point_at_origin(&fixture, "refs/tags/v1.0.0");
    point_at_origin(&fixture, "refs/remotes/origin/main");
    let mut checked = 0usize;
    for (clause, raw, normal) in spellings(&fixture) {
        let snap = fixture
            .git()
            .snapshot(&raw)
            .unwrap_or_else(|e| panic!("{clause}: {raw} — {e}"));
        assert_eq!(
            snap.locator(),
            normal,
            "{clause}: `snapshot` handed on {:?} instead of the normal form",
            snap.locator()
        );
        assert_eq!(
            locator::normalize(&raw),
            normal,
            "{clause}: and the function agrees with the method"
        );
        checked += 1;
    }
    println!("H2_GIT_SNAPSHOT_NORMALISES clauses={checked}");
    assert_eq!(
        checked, 4,
        "four spellings, over the crate root's five clauses"
    );
}

// ---------------------------------------------------------------------------
// 2. What that buys at the gate
// ---------------------------------------------------------------------------

/// 🔴 The shipped git pack refuses every spelling, and **all four** would have evaded it raw.
///
/// The forbid is written `"*#refs/tags/*"` and `"*#refs/remotes/*"` — on the **full** spelling, which
/// is the only one a policy author can reasonably be asked to write, because it is the only one that
/// is unambiguous (git itself resolves an unqualified name against several namespaces in order).
/// Everything that makes the short spellings mean the same thing is this adapter's clause 2.
#[test]
fn the_shipped_pack_refuses_a_spelling_that_would_have_evaded_it() {
    let fixture = GitFixture::new();
    point_at_origin(&fixture, "refs/tags/v1.0.0");
    point_at_origin(&fixture, "refs/remotes/origin/main");
    let gate = Gate::with_policies(packs::git_pack().expect("the shipped git pack parses"));
    let mut evaded_raw = 0usize;
    for (clause, raw, _) in spellings(&fixture) {
        let snap = fixture.git().snapshot(&raw).expect("the sandbox holds it");
        assert_eq!(
            verdict_for(&gate, &snap),
            VerdictKind::Deny,
            "{clause}: the pack must refuse this position however it was spelled"
        );
        if verdict_for(&gate, &snapshot_spelled(&raw)) != VerdictKind::Deny {
            evaded_raw += 1;
        }
    }
    println!("H2_GIT_GATE spellings=4 denied_after_snapshot=4 would_have_evaded_raw={evaded_raw}");
    assert!(
        evaded_raw >= 3,
        "a probe in which no raw spelling evades the pack is a probe measuring nothing"
    );
}

/// 🔴 The gate normalises nothing, and that is why the probe above is in **this** crate.
///
/// "in-gate normalisation is not adopted" (req/38 §25 H-2, req/98 §3-4). A gate that resolved `tags/v1.0.0` would need (sem: SEM-gx-adapter-git-104)
/// git's own namespace-resolution order, which is a repository read inside a policy evaluation — and
/// two reads a moment apart could disagree about one locator, which is the argument
/// `crates/gx-adapter-git/src/locator.rs` makes about why even *this* adapter's version is purely
/// textual.
#[test]
fn the_gate_normalises_nothing_and_that_is_why_this_probe_is_here() {
    let gate = Gate::with_policies(packs::git_pack().expect("the shipped git pack parses"));
    let evading = "/srv/repo#tags/v1.0.0:VERSION";
    let answer = verdict_for(&gate, &snapshot_spelled(evading));
    println!("H2_GIT_GATE_IS_NOT_A_NORMALISER spelling={evading:?} answer={answer:?}");
    assert_eq!(
        answer,
        VerdictKind::Admit,
        "recorded, not endorsed: if this becomes Deny, something in gx-gate resolves reference \
         names now, and req/38 §25's H-2 ruling has been reversed without being re-ruled"
    );
    assert_eq!(
        verdict_for(&gate, &snapshot_spelled(&locator::normalize(evading))),
        VerdictKind::Deny,
        "the pack is not indifferent to the position — only to the spelling"
    );
}
