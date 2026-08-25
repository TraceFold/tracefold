// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! policy pack v0 (req/446) — the postgres pack, and the two properties of the pack **set**.
//!
//! Four obligations live here, and each of them exists because a specific way of being wrong was
//! measured rather than imagined:
//!
//! * **AC-PP-04** — the substrate literal in `policies/postgres/deny-system-catalogs.cedar` is the
//!   string `RequestView` actually produces for `SubstrateKind::Custom("postgres")`. A pack that
//!   wrote `"postgres"` instead of `"custom:postgres"` parses, lints clean, embeds and ships
//!   without ever firing a statement, and a Deny-only conformance table stays green throughout,
//!   because an unmatched request falls to Cedar's third rule and denies anyway.
//! * **AC-PP-05** — every conformance locator of that pack is a spelling `gx-adapter-postgres`
//!   could produce, and the `postgres://*/` anchor is load-bearing (D-4 / F7). The half that runs
//!   the adapter's real `parse()` is in `crates/gx-adapter-postgres/tests/pack_locators.rs`,
//!   because gx-gate does not name an adapter (the rule `ac_074.rs`'s `git_locator` states); the
//!   half here is the anchor arithmetic on the pack source itself.
//! * **AC-PP-06** — 🔴 the **differential**. Adding this pack to `SHIPPED_PACKS` must not admit one
//!   request the three-pack set denied. Not asserted, measured: the same table is run against a
//!   set built from the first three packs and against the shipped four, and the difference is
//!   required to be exactly the catalog rows moving from `DenyByNoPolicy` to a **named** Deny.
//! * **AC-PP-09** — composition order does not move a verdict or a deciding id, **and the gate
//!   fires**. Cedar evaluates statement-wise with forbid over permit and default deny, so order
//!   independence is a property the set already has; a gate over a property nothing can break is a
//!   gate that never goes red, which `INHERITED_PRINCIPLES.md` §3d refuses. So the negative control
//!   is here beside it: a deliberately id-colliding composition, which the set builder must refuse.

// 🔴 `req/832` ATOM -1: this whole suite is about the postgres pack (`req/446` V0-C), and a build
// without the `pg` feature does not embed one -- `gx_gate::packs` gates the `include_str!` because
// that is the only place it can be gated (`req/817` §2.5). So the file compiles away publicly
// rather than naming constants that are not there.
//
// 🔴 **Declared coverage loss, not a silent one**: two tests here are set-wide rather than
// postgres-specific (`every_shipped_pack_declares_its_default`, `composition_order_moves_no_verdict
// _and_no_deciding_id`) and they go unrun in a `pg`-less build. They are named in `req/832` §ATOM-1
// rather than left for a later reader to discover as a hole.
#![cfg(feature = "pg")]

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId,
};
use gx_gate::packs::{
    self, PackCase, PackExpectation, PackRow, ShippedPack, POSTGRES_PACK_POLICY_IDS,
    POSTGRES_PACK_SOURCE, POSTGRES_SUBSTRATE_TAG, SHIPPED_PACKS,
};
use gx_gate::policy::RequestView;
use gx_gate::{Gate, GateInput, PolicyEngine};

// ---------------------------------------------------------------------------
// AC-PP-04 — the substrate literal is the string the projection produces
// ---------------------------------------------------------------------------

/// A [`RequestView`] over a `custom:postgres` request, built the only way a gate builds one.
///
/// The point of going through `GateInput` rather than calling a formatter is that `substrate_tag`
/// is private: the only honest way to learn what a policy will be compared against is to ask the
/// projection that does the comparing. If `substrate_tag` ever changed, this derivation changes
/// with it and the assertion below goes red — which a second copy of the literal would not.
fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

fn projected_view(kind: SubstrateKind) -> RequestView {
    let pre = ObjectSnapshot::new(
        ObjectId(cid(2)),
        kind,
        "postgres://main/public.orders?id=7".to_string(),
        cid(6),
        ReprKind::Bytes,
    );
    let t = Transformation::new(
        TransformationId(cid(1)),
        0,
        Subject::Object(ObjectId(cid(2))),
        Some(cid(3)),
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(cid(4)),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(5),
            },
            context: ChangeContext::Policy,
            actor: Actor::Process {
                key: "key-1".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("order 0 is below MAX_ORDER");
    let planned = PlannedDeltaBytes(Vec::new());
    let input = GateInput {
        t: &t,
        pre: &pre,
        planned: &planned,
        evidence: &[],
        invert_available: true,
        // E-DR4627-1 (DR-46-27): the sixth field. This file's subject is not the clock, so the
        // epoch pins it -- a value chosen once here is what makes `decided_at_seat.rs`'s claim (that
        // varying this field alone moves no verdict) about the field and not about this fixture.
        decided_at: Timestamp(0),
    };
    RequestView::of(&input)
}

fn projected_substrate_of(kind: SubstrateKind) -> String {
    projected_view(kind)
        .resource_attrs()
        .get("substrate")
        .expect("the ASM-60-1 mapping supplies a substrate attribute")
        .clone()
}

/// 🔴 **AC-PP-04**: the literal in the shipped postgres pack is what the gate will compare against.
#[test]
fn the_postgres_pack_compares_against_the_tag_the_projection_produces() {
    let produced = projected_substrate_of(SubstrateKind::Custom("postgres".to_string()));
    println!("PACK_V0_SUBSTRATE_TAG produced={produced:?} declared={POSTGRES_SUBSTRATE_TAG:?}");
    assert_eq!(
        produced, POSTGRES_SUBSTRATE_TAG,
        "the constant the pack row declares is not what a request carries"
    );
    // Comment lines are excluded on purpose: the pack's header quotes the wrong spelling in the
    // paragraph explaining why it is wrong, and a check that could not tell a warning from a rule
    // would force the warning out of the file.
    let code: String = POSTGRES_PACK_SOURCE
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let needle = format!("resource.substrate == \"{produced}\"");
    assert!(
        code.contains(&needle),
        "the pack's statements must compare against {needle:?}; a pack comparing against anything \
         else parses, lints and ships without ever firing (PACK_FORMAT F4)"
    );
    assert!(
        !code.contains("resource.substrate == \"postgres\""),
        "`postgres` without the `custom:` prefix is the silent spelling; no request carries it"
    );
}

/// The trap is real rather than hypothetical: the bare spelling matches no request.
///
/// A negative control for the test above. Without it, "compare against the produced tag" is a rule
/// whose cost nobody has seen; with it, the cost is a number — a pack written the wrong way answers
/// **zero** of its own conformance rows by a statement, while every Deny row still passes.
#[test]
fn the_bare_postgres_spelling_fires_no_statement_and_stays_green_anyway() {
    let wrong = POSTGRES_PACK_SOURCE.replace(
        "resource.substrate == \"custom:postgres\"",
        "resource.substrate == \"postgres\"",
    );
    assert_ne!(wrong, POSTGRES_PACK_SOURCE, "the substitution must apply");
    let gate = Gate::with_policies(PolicyEngine::parse(&wrong).expect("the wrong pack parses"));

    let catalog = catalog_case(PackExpectation::deny_by("postgres-deny-system-catalogs"));
    let by_id = answered(&gate, &catalog);
    assert!(
        !by_id.pass,
        "a pack comparing against the bare spelling cannot decide a catalog row by name"
    );
    assert!(
        by_id.deciding.is_empty(),
        "no statement should have fired: {:?}",
        by_id.deciding
    );

    // The same request, expressed the weak way a scenario file could only express before
    // AC-PP-10 — and it passes. That is the green-for-the-wrong-reason failure, measured.
    let weak = catalog_case(PackExpectation::Deny(None));
    assert!(
        answered(&gate, &weak).pass,
        "🔴 the point: an arm-only Deny expectation is satisfied by Cedar's third rule, so the \
         broken pack's conformance table would have shipped green"
    );
}

// ---------------------------------------------------------------------------
// AC-PP-05 — the `postgres://*/` anchor, as arithmetic
// ---------------------------------------------------------------------------

/// 🔴 **AC-PP-05 (gate half)**: every forbid pattern anchors on the schema position.
///
/// The pack's header argues that a produced locator holds exactly three `/` — two in the scheme and
/// one after the alias — so `postgres://*/x` can only mean "the schema is `x`". This test holds the
/// pattern side of that argument: every `like` pattern in the pack begins with the anchor, and the
/// locator side (a real `parse()`, a real slash count) is
/// `crates/gx-adapter-postgres/tests/pack_locators.rs`.
#[test]
fn every_forbid_pattern_of_the_postgres_pack_anchors_on_the_schema_position() {
    let patterns: Vec<&str> = POSTGRES_PACK_SOURCE
        .lines()
        .filter(|line| line.contains("resource.locator like"))
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    println!("PACK_V0_ANCHOR patterns={}", patterns.len());
    assert!(
        !patterns.is_empty(),
        "the pack must state at least one locator pattern, or its forbid is about nothing"
    );
    for line in &patterns {
        assert!(
            line.contains("\"postgres://*/"),
            "{line}: a pattern that does not anchor on `postgres://*/` can match an alias or a \
             percent-decoded key value rather than a schema"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-PP-06 — the differential: what shipping the fourth pack changes
// ---------------------------------------------------------------------------

fn catalog_case(expect: PackExpectation) -> PackCase {
    PackCase::new(
        "postgres: a write into pg_catalog",
        SubstrateKind::Custom("postgres".to_string()),
        "postgres://main/pg_catalog.pg_authid?oid=10",
        expect,
    )
}

/// The requests the differential is measured over: one per shipped substrate, plus the postgres
/// surface on both sides of the forbid, plus a substrate nobody speaks for.
fn differential_locators() -> Vec<(SubstrateKind, &'static str)> {
    vec![
        (SubstrateKind::Fs, "/tmp/x"),
        (SubstrateKind::Fs, "/etc/passwd"),
        (SubstrateKind::Git, "/srv/repo#refs/heads/main:README.md"),
        (SubstrateKind::Git, "/srv/repo#refs/tags/v1.0.0:VERSION"),
        (
            SubstrateKind::Mcp,
            "https://mcp.example/sse#file:///srv/notes.md",
        ),
        (
            SubstrateKind::Mcp,
            "https://mcp.example/sse#file:///etc/passwd",
        ),
        (
            SubstrateKind::Custom("postgres".to_string()),
            "postgres://main/pg_catalog.pg_authid?oid=10",
        ),
        (
            SubstrateKind::Custom("postgres".to_string()),
            "postgres://main/information_schema.tables?table_name=orders",
        ),
        (
            SubstrateKind::Custom("postgres".to_string()),
            "postgres://main/public.orders?id=7",
        ),
        (
            SubstrateKind::Custom("postgres".to_string()),
            "postgres://main/public.notes?slug=%2Fpg_catalog.x",
        ),
        (SubstrateKind::Custom("s3".to_string()), "s3://bucket/key"),
    ]
}

fn set_of(packs_in: &[ShippedPack]) -> Gate {
    let mut composed = String::new();
    for pack in packs_in {
        composed.push_str(pack.source);
        composed.push('\n');
    }
    Gate::with_policies(PolicyEngine::parse(&composed).expect("the packs compose"))
}

fn answered(gate: &Gate, case: &PackCase) -> PackRow {
    let report =
        packs::check_pack(gate, std::slice::from_ref(case)).expect("the case is evaluable");
    report.rows().first().expect("one case, one row").clone()
}

/// The verdict arm and the deciding ids a gate gives one locator, as a comparable string.
fn decision(gate: &Gate, substrate: SubstrateKind, locator: &str) -> String {
    // `Admit(None)` is the weakest expectation there is, so the row's *reported* answer is a
    // function of the gate rather than of what was asked. Only `actual` and `deciding` are read.
    let case = PackCase::new("probe", substrate, locator, PackExpectation::Admit(None));
    let row = answered(gate, &case);
    let mut ids = row.deciding;
    ids.sort();
    format!("{:?} by {ids:?}", row.actual)
}

/// 🔴 **AC-PP-06**: shipping the postgres pack admits nothing that was not admitted before.
///
/// The number this test prints is the whole of req/446's killer assumption. The other three packs
/// each open with a `<substrate>-permit-default`, and shipping one of those *does* move requests
/// from `Deny(NoPolicyApplied)` to `Admit`. Ruling 1 of req/446 refused that shape here, so the
/// expected increase in admitted surface is **zero** — and zero is exactly what a measurement can
/// state and a header cannot.
#[test]
fn adding_the_postgres_pack_admits_nothing_it_did_not_admit_before() {
    let three: Vec<ShippedPack> = SHIPPED_PACKS
        .into_iter()
        .filter(|p| p.substrate != POSTGRES_SUBSTRATE_TAG)
        .collect();
    assert_eq!(three.len(), 3, "the three packs that shipped before v0");
    let before = set_of(&three);
    let after = set_of(&SHIPPED_PACKS);

    let mut newly_admitted: Vec<String> = Vec::new();
    let mut changed: Vec<String> = Vec::new();
    for (substrate, locator) in differential_locators() {
        let was = decision(&before, substrate.clone(), locator);
        let now = decision(&after, substrate.clone(), locator);
        if was != now {
            changed.push(format!("{locator}: {was} -> {now}"));
        }
        if !was.contains("Admit") && now.contains("Admit") {
            newly_admitted.push(format!("{locator}: {was} -> {now}"));
        }
    }

    println!(
        "PACK_V0_DIFFERENTIAL probes={} changed={} newly_admitted={}",
        differential_locators().len(),
        changed.len(),
        newly_admitted.len()
    );
    for line in &changed {
        println!("PACK_V0_DIFFERENTIAL_ROW {line}");
    }

    assert!(
        newly_admitted.is_empty(),
        "🔴 shipping a forbid-only pack must not admit one request the three-pack set denied: \
         {newly_admitted:#?}"
    );
    assert_eq!(
        changed.len(),
        2,
        "exactly the two system-catalog rows should move, and they should move from an unnamed \
         refusal to a named one: {changed:#?}"
    );
    for line in &changed {
        let (was, now) = line
            .split_once(" -> ")
            .unwrap_or_else(|| panic!("{line}: a differential row names both sides"));
        assert!(
            now.ends_with("by [\"postgres-deny-system-catalogs\"]"),
            "{line}: the only change a deny-default pack may make is which statement refuses"
        );
        assert!(
            was.ends_with("by []"),
            "{line}: before the pack, nothing had an opinion — that is what makes the change a \
             gain in citability rather than in permission"
        );
        assert!(
            was.contains("Deny") && now.contains("Deny"),
            "{line}: the arm itself must not move"
        );
    }
}

/// The postgres pack ships **no permit**, and that is a declared property rather than an oversight.
#[test]
fn the_postgres_pack_declares_a_deny_default() {
    assert_eq!(
        POSTGRES_PACK_POLICY_IDS.len(),
        1,
        "a second statement in this pack is a decision, not a tidy-up"
    );
    for id in POSTGRES_PACK_POLICY_IDS {
        assert!(
            !id.contains("permit"),
            "{id}: PACK_FORMAT F3 admits a deny-default pack; adding a permit-default here is the \
             ruling req/446 ruling 1 refused and must be re-taken, not slipped in"
        );
    }
    let statements: Vec<&str> = POSTGRES_PACK_SOURCE
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("permit") || line.starts_with("forbid"))
        .collect();
    println!("PACK_V0_DEFAULT statements={statements:?}");
    assert!(
        !statements.iter().any(|line| line.starts_with("permit")),
        "the shipped postgres pack must contain no permit statement"
    );
}

// ---------------------------------------------------------------------------
// AC-PP-09 — order independence, with the negative control that makes it fire
// ---------------------------------------------------------------------------

/// 🔴 **AC-PP-09**: no permutation of the shipped packs moves a verdict or a deciding id.
#[test]
fn composition_order_moves_no_verdict_and_no_deciding_id() {
    let base: Vec<ShippedPack> = SHIPPED_PACKS.into_iter().collect();
    let orders: Vec<Vec<usize>> = vec![
        vec![0, 1, 2, 3],
        vec![3, 2, 1, 0],
        vec![2, 0, 3, 1],
        vec![1, 3, 0, 2],
    ];
    let mut seen: Vec<(Vec<usize>, Vec<String>)> = Vec::new();
    for order in &orders {
        let permuted: Vec<ShippedPack> = order.iter().map(|i| base[*i]).collect();
        let gate = set_of(&permuted);
        let answers: Vec<String> = differential_locators()
            .into_iter()
            .map(|(substrate, locator)| decision(&gate, substrate, locator))
            .collect();
        seen.push((order.clone(), answers));
    }
    println!(
        "PACK_V0_ORDER permutations={} probes={}",
        seen.len(),
        seen[0].1.len()
    );
    let (first_order, first) = &seen[0];
    for (order, answers) in seen.iter().skip(1) {
        assert_eq!(
            answers, first,
            "composition order changed an answer: {first_order:?} vs {order:?}"
        );
    }
}

/// 🔴 The negative control §3d asks for: this gate **can** go red, and here is the input that does
/// it.
///
/// Order independence is a property Cedar's evaluation rules already give, so the test above would
/// pass on any set anybody could write — including one whose statements collided, which is the
/// failure `shipped_pack_set`'s duplicate-id refusal exists for. A gate that no input can break is
/// not a gate. This is the input that breaks it: two packs claiming one `@id`, which the composed
/// set must refuse rather than resolve by position.
#[test]
fn two_packs_claiming_one_id_are_refused_rather_than_ordered() {
    let postgres = SHIPPED_PACKS
        .into_iter()
        .find(|p| p.substrate == POSTGRES_SUBSTRATE_TAG)
        .expect("the postgres pack ships");
    let collided = ShippedPack {
        path: "policies/collision/second-claim.cedar",
        source: postgres.source,
        policy_ids: postgres.policy_ids,
        substrate: postgres.substrate,
    };
    let mut composed = String::new();
    for pack in [postgres, collided] {
        composed.push_str(pack.source);
        composed.push('\n');
    }
    let refused = PolicyEngine::parse(&composed);
    println!("PACK_V0_COLLISION refused={}", refused.is_err());
    assert!(
        refused.is_err(),
        "two statements claiming `postgres-deny-system-catalogs` must be refused: resolving them \
         by position would make a receipt's PolicyDecisionRecord name a rule nobody can find"
    );
}

// ---------------------------------------------------------------------------
// R35 / req/470 M-02 — PACK_FORMAT F3 has a mechanism, and it can refuse
// ---------------------------------------------------------------------------

/// The `deny_by_no_policy` cases a pack's shipped scenario file declares.
///
/// Read off the disk rather than out of a constant, because F3's second shape is a claim about
/// what a **deployment** can run: `gx policy test <PACK> --scenario <FILE>` is the operator's face
/// of it, and the file is what they point it at.
fn deny_by_no_policy_cases(pack: &ShippedPack) -> usize {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/gx-gate sits at <root>/crates/gx-gate");
    let scenarios = root
        .join(
            std::path::Path::new(pack.path)
                .parent()
                .expect("a pack path names a directory"),
        )
        .join("scenarios.json");
    let text = std::fs::read_to_string(&scenarios).unwrap_or_default();
    text.matches("\"deny_by_no_policy\": true").count()
}

/// 🔴 **`req/470` M-02** — every shipped pack declares its default, and a pack that declares
/// neither is refused **by the road a deployment walks**.
///
/// The audit's finding was not that a pack was wrong. All four shipped packs hold F3 today. It was
/// that **nothing checked**: the document counted F3 among the clauses with machinery, the shell
/// gate did not look at it, and the only thing in the tree resembling a check was hard-wired to
/// the postgres pack by constant name. A clause held by four artifacts and no mechanism is held
/// until the fifth artifact.
///
/// So the assertion that matters here is the **negative** one. Green over the shipped four proves
/// nothing on its own — a checker that returned `Ok(())` unconditionally would pass that half.
#[test]
fn every_shipped_pack_declares_its_default() {
    let mut shapes: Vec<(&str, &str, usize)> = Vec::new();
    for pack in SHIPPED_PACKS {
        packs::declares_its_default(&pack)
            .unwrap_or_else(|e| panic!("a shipped pack fails PACK_FORMAT F3: {e}"));
        // 🔴 **R36 / `req/476` M-01** — counted the way the gate counts, which is Cedar's way. This
        // test used to run its own line-scan beside the gate's, so both would have been fooled by
        // the same `@id(..) permit` on one line and this file would have agreed that a pack
        // shipping a permit was a deny-default one.
        let permits = gx_gate::policy::PolicyEngine::parse(pack.source)
            .expect("a shipped pack parses")
            .permit_count();
        let shape = if permits == 0 {
            "deny-default"
        } else {
            "permit-default"
        };
        let cases = deny_by_no_policy_cases(&pack);
        println!(
            "PACK_V0_F3 pack={} shape={shape} permits={permits} deny_by_no_policy_cases={cases}",
            pack.path
        );
        shapes.push((pack.path, shape, cases));

        // F3's second shape carries an obligation the first does not: a pack that admits nothing
        // by default has to **show** that, or "deny-default" is a word in a header rather than a
        // measured property.
        if permits == 0 {
            assert!(
                cases >= 1,
                "{}: PACK_FORMAT F3 admits a deny-default pack only with at least one \
                 conformance case expecting `deny_by_no_policy`; its scenario file declares none, \
                 so nothing measures that the default is what the header says",
                pack.path
            );
        }
    }
    assert!(
        shapes.iter().any(|(_, shape, _)| *shape == "deny-default"),
        "both shapes must be exercised by the shipped set or the deny-default branch of this \
         gate is never walked"
    );
    // 🔴 **R36 / `req/476` §2-3** — the audit's honest deduction about this very assertion, and it
    // is printed rather than left to be re-derived: the `cases >= 1` arm above is F3's **third**
    // conjunct, it only fires where `permits == 0`, and exactly one of the four shipped packs is
    // that shape. "The disk half is covered by a test" is a claim about 1 of 4, and the number is
    // in the log so the next audit reads it instead of counting.
    let deny_default_packs = shapes
        .iter()
        .filter(|(_, shape, _)| *shape == "deny-default")
        .count();
    println!(
        "PACK_V0_F3_COVERAGE shipped={} deny_default={deny_default_packs} \
         disk_half_applies_to={deny_default_packs}",
        shapes.len()
    );

    // 🔴 **R36 / `req/476` M-01** — shape 2's **second** conjunct, on the shipped artifacts. The
    // gate refuses a deny-default pack that does not declare itself, so a pack reaching this line
    // as `deny-default` has the declaration; this asserts the pairing directly so the two facts
    // are not left to be inferred from the gate not having raised.
    for pack in SHIPPED_PACKS {
        let declared = pack
            .source
            .lines()
            .map(str::trim_start)
            .filter(|l| l.starts_with("//"))
            .any(|l| l.contains(packs::DENY_DEFAULT_DECLARATION));
        let permits = gx_gate::policy::PolicyEngine::parse(pack.source)
            .expect("a shipped pack parses")
            .permit_count();
        println!(
            "PACK_V0_F3_HEADER pack={} permits={permits} declares_deny_default={declared}",
            pack.path
        );
        assert_eq!(
            declared,
            permits == 0,
            "{}: PACK_FORMAT F3 shape 2 is a conjunction — the header's declaration and the \
             absence of permits have to be the same fact about the same pack (req/476 M-01)",
            pack.path
        );
    }

    // 🔴 The negative bed: the fifth pack, which declares neither. This is the artifact the audit
    // said nothing in the tree would stop.
    let undeclared = ShippedPack {
        path: "policies/fifth/no-declared-default.cedar",
        source: "@id(\"fifth-deny-something\")\nforbid ( principal, action, resource )\n  when { resource.substrate == \"custom:fifth\" };\n\n@id(\"fifth-allow-the-rest\")\npermit ( principal, action, resource )\n  when { resource.substrate == \"custom:fifth\" };\n",
        policy_ids: &["fifth-allow-the-rest", "fifth-deny-something"],
        substrate: "custom:fifth",
    };
    let refused = packs::declares_its_default(&undeclared);
    println!(
        "PACK_V0_F3_NEGATIVE refused={} detail={:?}",
        refused.is_err(),
        refused.as_ref().err().map(std::string::ToString::to_string)
    );
    assert!(
        refused.is_err(),
        "a pack carrying a permit statement whose id is not `<substrate>-permit-default` has not \
         declared its default, and F3 says a pack must. Nothing in the tree refused this before \
         R35 (req/470 M-02)"
    );

    // 🔴 **R38 / `req/513` L-03** — the declaration in text a **reader of the pack does not read as
    // the pack's header**: a Cedar statement's string literal.
    //
    // R36 scanned line beginnings, so only a `//` line could declare. R37 flattened the whole
    // source to close the trailing-comment, wrapped-comment and `@doc(...)` annotation shapes, and
    // the flatten does not know what a comment is — audit 37 measured the sentence being accepted
    // from ordinary program text (`req/513` §4-6, `case=non_comment_text`).
    //
    // Audit 37 measured it with a `permit`-carrying pack and said so (`req/513` §8-7): shape 2
    // requires zero permits, so that example did not on its own put a pack through F3. This is the
    // combination that would — **zero permits**, and the only occurrence of the sentence inside a
    // `forbid`'s string literal. It is here as a permanent case whichever way it answers: if F3
    // accepts this pack, the clause is satisfied by a document whose author never wrote a header.
    let text_only = ShippedPack {
        path: "policies/fifth/declared-inside-a-string.cedar",
        // 🔴 The literal carries `DENY_DEFAULT_DECLARATION` **verbatim**, asterisks included, and
        // the assertion below holds it to that. A first draft of this case wrote the phrase in
        // prose without the asterisks; `declares_deny_default` answered `false` and the case
        // passed — for the wrong reason, measuring a string that is not the declaration. A
        // negative case that can go vacuous when a constant is reworded is not a case.
        // 🔴 And the sentence carries **none** of `DENIALS`. A second draft read `... and ships no
        // permit`, and ` no ` is on that list, so the occurrence was discounted as a denial and the
        // case passed again for a second wrong reason. Two vacuous passes on one case is why the
        // two assertions below are separate: one holds the phrase, the other holds the verdict.
        source: "@id(\"fifth-deny-something\")\nforbid ( principal, action, resource )\n  when { resource.locator == \"this pack declares a **deny-default**\" };\n",
        policy_ids: &["fifth-deny-something"],
        substrate: "custom:fifth",
    };
    assert!(
        text_only.source.contains(packs::DENY_DEFAULT_DECLARATION),
        "this case is only a case while its source carries the declaration verbatim; the constant \
         was reworded and the fixture was not"
    );
    let from_a_string_literal = packs::declares_deny_default(text_only.source);
    assert!(
        !from_a_string_literal,
        "🔴 `req/513` L-03: the declaration was read out of a Cedar statement's string literal. \
         R38 narrowed the scan to the pack's comments and annotation values, which is where a \
         reader looks for the pack's own statement about itself"
    );
    let f3_verdict = packs::declares_its_default(&text_only);
    println!(
        "PACK_V0_F3_NON_COMMENT declares_deny_default={from_a_string_literal} \
         f3_accepts={} detail={:?}",
        f3_verdict.is_ok(),
        f3_verdict
            .as_ref()
            .err()
            .map(std::string::ToString::to_string)
    );
    assert!(
        f3_verdict.is_err(),
        "🔴 `req/513` L-03: a pack with no permit statement, whose only occurrence of the \
         declaration is inside a Cedar statement's string literal, satisfied F3. The clause exists \
         so that a pack picks its default **out loud**, and program text is not where a reader \
         looks for the pack's own statement about itself"
    );

    // And the other way a pack can disagree with itself: no permit, but an id promising one.
    let lying = ShippedPack {
        path: "policies/fifth/says-permit-ships-none.cedar",
        source: "@id(\"fifth-deny-something\")\nforbid ( principal, action, resource );\n",
        policy_ids: &["fifth-permit-default"],
        substrate: "custom:fifth",
    };
    assert!(
        packs::declares_its_default(&lying).is_err(),
        "declared ids naming a permit default over a pack that ships no permit is a disagreement \
         between the row and the artifact, which is the failure mode this module is built against"
    );
}

// ---------------------------------------------------------------------------
// R35 / req/470 L-01 — F6's row counts what the projection carries
// ---------------------------------------------------------------------------

/// 🔴 **`req/470` L-01** — the row that tells a reader to count, counted.
///
/// `PACK_FORMAT.md`'s F6 row lists the attributes a statement may speak about — `resource.locator`,
/// `resource.substrate`, the principal, the action, and `context.{order, invert_available,
/// evidence_count, evidence_kinds}` — which is **eight**, and then said "the whole projection is
/// those **seven** reads". The implementation has eight. Nothing measured the sentence against the
/// type, so a cell instructing the reader to count was the one thing in the document that did not.
///
/// Counted from a real [`RequestView`] rather than from the source text: two resource attributes,
/// four context entries, plus the principal and the action, which are fields rather than map
/// entries and are therefore counted by name.
#[test]
fn the_projection_is_the_eight_reads_this_row_lists() {
    let view = projected_view(SubstrateKind::Custom("postgres".to_string()));
    let attrs: Vec<&String> = view.resource_attrs().keys().collect();
    let context: Vec<&String> = view.context().keys().collect();
    let reads = attrs.len() + context.len() + 2;
    println!(
        "PACK_V0_F6_READS attrs={attrs:?} context={context:?} principal_and_action=2 total={reads}"
    );
    assert_eq!(
        reads, 8,
        "PACK_FORMAT F6 names eight reads. A ninth in the projection is a widening of what a \
         policy can see, which is a change to the clause and not to this number"
    );

    let doc = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("<root>")
            .join("policies")
            .join("PACK_FORMAT.md"),
    )
    .expect("PACK_FORMAT.md ships");
    let f6 = doc
        .lines()
        .find(|line| line.starts_with("| F6 |"))
        .expect("PACK_FORMAT.md declares F6");
    assert!(
        !f6.contains("those seven reads"),
        "F6 says seven and the projection has eight (req/470 L-01): {f6}"
    );
    assert!(
        f6.contains("**eight** reads"),
        "the F6 row must state the count the projection actually carries: {f6}"
    );
}
