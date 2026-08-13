//! 🔴 The **composed** shipped set — the obligation that arrived with the third pack (**M7 hand 4**).
//!
//! `req/38` §60 ruled the pair req/100 §9 R-1 raised: 「既定 policy set と既定 registry は**対**で決まる
//! (pack を足しても adapter が register されなければ NotFound)」, and took req/101 §9-1 as its material.
//! That material named this file before it existed:
//!
//! > **AC-074**(pack 単位の conformance)——`check_pack` は pack ごとに gate を作るので、**既定 set を
//! > 変えても AC-074 は動かない**。逆に言えば AC-074 は「合わせた set」を測っていない。合わせるなら
//! > **合成 set の conformance** が新しい obligation として要る(既存の 3 pack が互いの判定を変えない事)。
//!
//! # The obligation, stated so that it can be false
//!
//! 「互いの判定を変えない」 is not quite the property, and the difference is the whole of what this
//! file measures. Composing the packs **does** change one thing on purpose: a request on a substrate
//! whose pack has now joined the set stops falling to Cedar's third rule (nothing satisfied → `Deny`
//! with [`gx_gate::ReasonSource::NoPolicyApplied`]) and starts being judged by that pack's rules.
//! req/101 §9-1: 「それは緩和ではなく『判定されるようになる』であり…**新たに到達可能になる**」.
//!
//! What must **not** change is any judgement a pack was already making. So the property is
//! **locality**:
//!
//! > for every request, the composed set answers exactly what the pack that speaks for that
//! > request's substrate answers alone — the same arm **and the same deciding statement ids** — and
//! > a request on a substrate no pack speaks for still reaches Cedar's third rule.
//!
//! It holds because every statement of every shipped pack carries `resource.substrate == "<its
//! own>"` ([`gx_gate::packs::ShippedPack::substrate`], checked against each file's text in
//! `pack_embedding.rs`), and Cedar evaluates statement by statement. The reasoning is one paragraph;
//! the measurement is this file, because a pack added later with one unscoped statement would leave
//! the paragraph standing and the property gone.
//!
//! **The deciding ids and not only the arm.** 42 §1.3 puts `PolicyDecisionRecord` inside
//! `AdmitProof`'s IdentityView and an `AdmitProof` reaches a receipt's CID, so a composition that
//! kept every answer and moved which statement gave it would move what receipts say about changes
//! nobody touched. That is `AC-025`'s half of hand 4's brief — 「AC-025 の policy_id 確認」 — and it
//! has a test of its own below.
//!
//! # What this file does not measure
//!
//! Whether the packs are *right*: that is AC-074's and AC-028's, per pack, and this file would pass
//! on three wrong packs that were each wrong locally. And whether a **deployment's own** statements
//! compose safely: a deployment writing `permit(principal, action, resource);` unscoped is entitled
//! to, and nothing here is in force on a set gx did not ship.

use gx_core::SubstrateKind;
use gx_gate::packs::{self, PackCase, PackExpectation, PackRow, ShippedPack, SHIPPED_PACKS};
use gx_gate::{Gate, PolicyEngine};

/// A gate holding every shipped pack, as [`packs::shipped_pack_set`] composes them.
fn composed_gate() -> Gate {
    Gate::with_policies(packs::shipped_pack_set().expect("the shipped packs compose into one set"))
}

/// A gate holding one pack.
fn one_pack_gate(pack: &ShippedPack) -> Gate {
    Gate::with_policies(PolicyEngine::parse(pack.source).expect("a shipped pack parses"))
}

/// The pack that speaks for a substrate, by the tag [`gx_gate::packs::ShippedPack::substrate`]
/// carries — or `None` when no shipped pack does.
fn pack_for(substrate: &str) -> Option<ShippedPack> {
    SHIPPED_PACKS.into_iter().find(|p| p.substrate == substrate)
}

/// One request, and the tag of the substrate whose pack owns the answer.
struct Row {
    tag: &'static str,
    case: PackCase,
}

fn row(tag: &'static str, case: PackCase) -> Row {
    Row { tag, case }
}

/// Requests that reach every statement of every shipped pack, plus one no pack speaks for.
///
/// Written here rather than borrowed from `ac_028.rs` / `ac_074.rs`: those tables are each pack's
/// own conformance and belong to the pack, while this table is about the **composition** and has a
/// different completeness condition — every declared statement of every declared pack has to decide
/// at least one row of it, which
/// [`every_statement_of_every_shipped_pack_decides_a_row_of_this_table`] asserts rather than
/// assumes.
fn rows() -> Vec<Row> {
    vec![
        row(
            "fs",
            PackCase::new(
                "fs: a change under /tmp",
                SubstrateKind::Fs,
                "/tmp/x",
                PackExpectation::admit_by("fs-permit-default"),
            )
            .because("AC-025's second half, and the fs permit"),
        ),
        row(
            "fs",
            PackCase::new(
                "fs: a change to /etc/passwd",
                SubstrateKind::Fs,
                "/etc/passwd",
                PackExpectation::deny_by("fs-deny-etc"),
            )
            .because("AC-025's first half, and the fs forbid"),
        ),
        row(
            "git",
            PackCase::new(
                "git: a change to an entry on a branch",
                SubstrateKind::Git,
                "/srv/repo#refs/heads/main:README.md",
                PackExpectation::admit_by("git-permit-default"),
            )
            .because("the git permit — and the wedge case the git pack deliberately does not refuse"),
        ),
        row(
            "git",
            PackCase::new(
                "git: moving a tag",
                SubstrateKind::Git,
                "/srv/repo#refs/tags/v1.0.0:VERSION",
                PackExpectation::deny_by("git-deny-nonbranch-refs"),
            )
            .because("the git forbid, which composition makes reachable from a v0.1 surface"),
        ),
        row(
            "mcp",
            PackCase::new(
                "mcp: a tool call on an ordinary resource",
                SubstrateKind::Mcp,
                "https://mcp.example/sse#file:///srv/notes.md",
                PackExpectation::admit_by("mcp-permit-default"),
            )
            .because("the mcp permit"),
        ),
        row(
            "mcp",
            PackCase::new(
                "mcp: a tool call that writes under /etc",
                SubstrateKind::Mcp,
                "https://mcp.example/sse#file:///etc/passwd",
                PackExpectation::deny_by("mcp-deny-etc-resources"),
            )
            .because(
                "the mcp forbid. In the composed set this is the row that shows the fs pack's rule \
                 is no longer bypassable by changing substrate",
            ),
        ),
    ]
}

/// The one request no shipped pack speaks for.
///
/// `Custom` rather than one of 42 §3.1's three, because all three now have a pack: after this hand
/// there is no *named* substrate left that reaches Cedar's third rule, and a probe about that rule
/// needs a subject. `crates/gx-gate/tests/false_admit.rs`'s FA-5 is the same request one layer up,
/// for the same reason and by the same ruling (**H-9**).
fn unspoken_case() -> PackCase {
    PackCase::new(
        "a substrate no shipped pack speaks for",
        SubstrateKind::Custom("s3".to_string()),
        "s3://bucket/key",
        PackExpectation::DenyByNoPolicy,
    )
    .because(
        "Cedar's third rule is the whole of gx's fail-closed default for an unspoken substrate, and \
         a set that grew a pack whose statements were not substrate-scoped would admit this",
    )
}

fn answered(gate: &Gate, case: &PackCase) -> PackRow {
    let report =
        packs::check_pack(gate, std::slice::from_ref(case)).expect("the case is evaluable");
    report.rows().first().expect("one case, one row").clone()
}

// ---------------------------------------------------------------------------
// The set itself
// ---------------------------------------------------------------------------

/// The composed set is every declared statement of every declared pack, and nothing else.
///
/// Derived from [`SHIPPED_PACKS`] rather than written out, so that a pack added without its ids
/// declared, or ids declared without a pack, is a failure here as well as in `pack_embedding.rs`.
#[test]
fn the_composed_set_is_every_declared_statement_and_no_other() {
    let set = packs::shipped_pack_set().expect("the shipped packs compose");
    let mut declared: Vec<String> = SHIPPED_PACKS
        .iter()
        .flat_map(|p| p.policy_ids.iter().map(|id| (*id).to_string()))
        .collect();
    declared.sort();
    println!(
        "SHIPPED_SET packs={} statements={} ids={:?}",
        SHIPPED_PACKS.len(),
        set.len(),
        set.policy_ids()
    );
    assert_eq!(
        set.policy_ids(),
        declared,
        "the composed set answers under exactly the ids the packs declare"
    );
    assert_eq!(
        set.len(),
        declared.len(),
        "a statement in the composition that no pack declares is a rule nobody can find"
    );
}

/// 🔴 **The locality obligation**: composing changes no judgement a pack was already making.
///
/// Same arm **and** same deciding ids, row by row. A pack whose statements stopped being scoped to
/// its own substrate would show up here as a second deciding id on somebody else's row, or as an
/// arm that moved.
#[test]
fn each_pack_decides_only_its_own_substrate() {
    let composed = composed_gate();
    let mut divergences: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for Row { tag, case } in rows() {
        let pack = pack_for(tag).unwrap_or_else(|| panic!("{tag}: no shipped pack speaks for it"));
        let alone = answered(&one_pack_gate(&pack), &case);
        let together = answered(&composed, &case);
        checked += 1;
        assert!(
            alone.pass,
            "{}: the owning pack does not answer what this table says it does ({alone:?})",
            case.name()
        );
        if (alone.actual, &alone.deciding) != (together.actual, &together.deciding) {
            divergences.push(format!(
                "{}: alone {:?} by {:?}, composed {:?} by {:?}",
                case.name(),
                alone.actual,
                alone.deciding,
                together.actual,
                together.deciding
            ));
        }
    }
    println!(
        "SHIPPED_SET_LOCALITY rows={checked} divergences={}",
        divergences.len()
    );
    assert!(
        divergences.is_empty(),
        "composing the shipped packs moved a judgement one of them was already making: \
         {divergences:#?}"
    );
}

/// The table is not vacuous: every declared statement of every declared pack decides a row of it.
///
/// Without this, [`each_pack_decides_only_its_own_substrate`] would keep passing while a pack's
/// forbid quietly stopped being exercised — the same fail-open `ac_074.rs` closes for one pack, at
/// the size of the set.
#[test]
fn every_statement_of_every_shipped_pack_decides_a_row_of_this_table() {
    let composed = composed_gate();
    let mut deciding: Vec<String> = Vec::new();
    for Row { case, .. } in rows() {
        deciding.extend(answered(&composed, &case).deciding);
    }
    deciding.sort();
    deciding.dedup();
    let mut declared: Vec<String> = SHIPPED_PACKS
        .iter()
        .flat_map(|p| p.policy_ids.iter().map(|id| (*id).to_string()))
        .collect();
    declared.sort();
    println!("SHIPPED_SET_DECIDING={deciding:?} DECLARED={declared:?}");
    assert_eq!(
        deciding, declared,
        "a declared statement that decides no row of this table is a statement the composition \
         obligation does not measure"
    );
}

/// A substrate no shipped pack speaks for still reaches Cedar's third rule.
///
/// The fail-closed default, measured on the composed set rather than assumed from the packs: a
/// composition is where an unscoped `permit` would do its damage, and this is the row it would
/// break.
#[test]
fn a_substrate_no_pack_speaks_for_reaches_cedars_third_rule() {
    let case = unspoken_case();
    let row = answered(&composed_gate(), &case);
    println!(
        "SHIPPED_SET_UNSPOKEN actual={:?} deciding={:?} pass={}",
        row.actual, row.deciding, row.pass
    );
    assert!(
        row.pass,
        "the composed set must refuse a substrate it has no statement about, and refuse it by \
         NoPolicyApplied rather than by a rule that happened to match: {row:?}"
    );
}

/// 🔴 **AC-025's ids do not move when the set is composed** (hand 4's brief).
///
/// req/101 §9-1 raised it as the thing nobody had measured: 「`fs-permit-default` は substrate scope
/// なので、git/mcp の statement が増えても fs の判定は動かない。ただし *`policy_id` が変わらない事*は
/// 測られていないので、pack を合わせる手が `PolicyDecisionRecord` の id を確認する必要が在る(receipt の
/// CID に入る=42 §1.3)」.
///
/// Written against AC-025's own two locators and its own two ids, so that this is the criterion's
/// question and not a paraphrase of it: 34 AC-025 逐語 「`/etc/passwd`への書き込みTransformationを評価。
/// Then: Cedar評価結果がDeny相当に写像される。`/tmp/x`へは Admit相当。」
#[test]
fn ac_025_answers_and_ids_are_unmoved_by_the_composition() {
    let fs_alone = one_pack_gate(&pack_for("fs").expect("the fs pack ships"));
    let composed = composed_gate();
    for (locator, expect, id) in [
        (
            "/etc/passwd",
            PackExpectation::deny_by("fs-deny-etc"),
            "fs-deny-etc",
        ),
        (
            "/tmp/x",
            PackExpectation::admit_by("fs-permit-default"),
            "fs-permit-default",
        ),
    ] {
        let case = PackCase::new(
            format!("AC-025 {locator}"),
            SubstrateKind::Fs,
            locator,
            expect,
        )
        .because("34 AC-025 逐語");
        let alone = answered(&fs_alone, &case);
        let together = answered(&composed, &case);
        println!(
            "AC025_UNDER_COMPOSITION locator={locator} alone={:?}/{:?} composed={:?}/{:?}",
            alone.actual, alone.deciding, together.actual, together.deciding
        );
        assert!(
            alone.pass && together.pass,
            "AC-025 holds either way: {together:?}"
        );
        assert_eq!(
            together.deciding,
            vec![id.to_string()],
            "the deciding statement's id reaches a receipt's CID (42 §1.3); composing the set must \
             not move it"
        );
        assert_eq!(
            alone.deciding, together.deciding,
            "and it is the same id the fs pack gave alone"
        );
    }
}
