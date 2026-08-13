//! `gx policy lint` / `gx policy test` (44 §1.2), and the three accessors M5 armed with a kill
//! condition.
//!
//! # 🔴 **M6-21** — this file is the consumer, and the naming is now paid
//!
//! §41 M5H4-8 armed `Gate::policies`, `Gate::invariants` and `PolicyEngine::is_empty` with 「M6
//! reqdef が具体的な消費者を 1 つも名指せなければ、M6 手 1 で退役印形」. req/88 §4 M6-21 named three
//! consumers and req/38 §47 採(a) dissolved the kill condition **with a second condition attached**:
//!
//! > 名指しただけでは消費者にならない——該当手の DoD に「3 accessor それぞれの呼び出し箇所を機械で
//! > 数え、0 なら RED」を含める
//!
//! [`crate::consumers::GATE_ACCESSOR_CONSUMERS`] names this file three times and
//! `probes/doubt/tests/m6_surface_doubt.rs` counts the calls in it. All three are below, each doing
//! the job its reason gave it:
//!
//! * **`Gate::policies`** — how many statements a pack parsed into, and under which ids. A gate
//!   whose policy set is invisible is a gate nobody can testify about afterwards.
//! * **`Gate::invariants`** — 44 §1.2's `lint` text describes Cedar syntax only, and a `Verdict` is
//!   the composition of policy **and** invariant (FR-027). A diagnostic that checked one half would
//!   misreport what it checked.
//! * **`PolicyEngine::is_empty`** — 「policy 0 本で立った」 is the most dangerous configuration a
//!   fail-closed deployment can be in, and it is invisible unless something asks.
//!
//! # 🔴 What `gx policy test` is, in 則 1's terms
//!
//! It calls [`gx_gate::Gate::verify`], which is 44 §1.2's own instruction — 「指定シナリオ（Intent/
//! Evidence/期待Verdictの組）に対しGate評価を実行し期待値と照合」 — and it is **not** a second
//! judgement. The gate answers; this reports the answer beside the operator's expectation and
//! exits on whether the two agree. No `Verdict` is constructed here (則 1 (ii)): what leaves this
//! module is [`gx_core::VerdictKind`], the display discriminant gx-core owns.
//!
//! # 🔴 A scenario's identity fields are placeholders, and that is a consequence of M3-10
//!
//! `GateInput` wants a whole `Transformation` and a whole `ObjectSnapshot`, and a scenario file
//! cannot supply the ids of things that do not exist. It does not have to: req/38 §19's M3-10 fixes
//! what a pack may reason over — 「v0.1 pack の実効範囲=**locator/actor/context/order 級**」 — and
//! the object id, the digest and the transformation id are not in it. So the six facts a policy can
//! read come from the file and the rest are zeroes, which is honest for a hypothetical and is
//! stated here rather than discovered by somebody whose scenario 「did not take」. A pack that grew
//! a rule about an id would make this module's placeholders visible as wrong answers, and that is
//! the day M3-10's range moved.
//!
//! # 🔴 **G-4** — this command and the shipped packs' own conformance are one road (**M7 hand 2**)
//!
//! `req/38_ERRATA_2026-08-07.md` §19 reserved G-4 for M7: 「第三者 pack を投入した時、conformance 検査
//! が**同じ 1 本の経路**で走る(自社/第三者で分岐しない)」. A pack an operator names on the command line
//! is exactly 「第三者 pack」, so this module used to be the second implementation of the thing
//! `crates/gx-gate/tests/ac_028.rs` did for the shipped one: its own case type, its own loop, its own
//! `GateInput` construction. Two implementations of one check are two answers waiting to differ, and
//! the one a stranger runs would have been the one nobody's acceptance criterion measured.
//!
//! So the loop and the hypothetical moved into `gx_gate::packs::check_pack`, which is library code
//! this command calls and `crates/gx-gate/tests/ac_074.rs` calls. What stays here is the operator's
//! half: reading the file, mapping its strings onto types, and rendering 44 §1.2's output. **則 1 (ii)
//! is unchanged and strengthened** — no `Verdict` is constructed here and none is read; what crosses
//! this boundary is `gx_core::VerdictKind`.

use std::path::Path;

use gx_core::VerdictKind;
use gx_gate::packs::{self, PackCase, PackExpectation};
use gx_gate::{Gate, PolicyEngine};
use serde::Deserialize;

use crate::exit::{Outcome, ERROR};
use crate::{Error, Result};

/// Read a Cedar pack off disk and parse it.
///
/// The one place in this binary that turns a **file** into a `PolicyEngine`. FR-028's shipped pack
/// takes the other road (`gx_gate::packs::fs_pack`, an `include_str!` inside gx-gate), and the two
/// stay separate on purpose: 「pack の file が build に埋め込まれる経路が 1 本」 is a claim about the
/// build, and a pack an operator names on the command line is not in the build.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read, [`Error::Gate`] if Cedar refuses it or a statement
/// carries no `@id` (ASM-62-1).
pub fn load(path: &Path) -> Result<PolicyEngine> {
    let src = std::fs::read_to_string(path).map_err(crate::io("read", path))?;
    Ok(PolicyEngine::parse(&src)?)
}

/// 🔴 `gx policy lint <PATH>` (44 §1.2) — 「Cedar policy構文・スキーマ検証」, **and the invariant half**.
///
/// 44 §1.2 writes only the Cedar side. FR-027 makes a `Verdict` the composition of policies and
/// invariants, so a linter that reported on one of the two would be answering 「is this deployment's
/// admissibility predicate well formed」 with a check of half of it. Both counts are printed, and
/// the invariant registry being empty is reported as a **fact** rather than as a fault: 41 §4 types
/// it as a `Vec` and 「nobody was asked」 is a state a deployment may legitimately be in.
///
/// The one thing that is a **warning** is a policy set with no statements. Cedar is default-deny, so
/// an empty set is not a permissive gate — it is a gate that admits nothing at all, and a
/// fail-closed deployment that came up that way would refuse every change with no rule to point at.
/// [`PolicyEngine::is_empty`] is the question, and this is the only caller.
///
/// # Errors
/// Whatever [`load`] refuses. A pack that will not parse is a **refusal** (44 §1.2's exit 1 comes
/// through [`Error::exit_code`]), not a diagnostic line: there is nothing to report about a set
/// that does not exist.
pub fn lint(path: &Path) -> Result<Outcome> {
    let engine = load(path)?;
    // The gate is built here so that the diagnostic is made of the same two halves a running
    // deployment decides with, read back through the accessors rather than off the local variable.
    let gate = Gate::with_policies(engine);

    let policies = gate.policies();
    let ids = policies.map(PolicyEngine::policy_ids).unwrap_or_default();
    let empty = policies.is_some_and(PolicyEngine::is_empty);
    let invariants = gate.invariants();
    let invariant_ids: Vec<String> = invariants.ids().into_iter().map(str::to_string).collect();

    let mut warnings: Vec<String> = Vec::new();
    if empty {
        warnings.push(
            "this pack parses into zero statements. Cedar is default-deny, so a gate built from it \
             admits nothing at all — a fail-closed deployment would refuse every change with no \
             rule to name (M6-21: 「policy 0 本で立った」の起動時警告)"
                .to_string(),
        );
    }
    if invariant_ids.is_empty() {
        warnings.push(
            "no InvariantCheck is registered, so this diagnostic covers the policy half of FR-027 \
             only. That is a fact about the registry rather than a defect in the pack: 41 §4 types \
             the registry as a Vec and D-9 ships no ready-made invariant with the fs pack"
                .to_string(),
        );
    }

    Ok(Outcome::ok(serde_json::json!({
        "path": path.display().to_string().replace('\\', "/"),
        "policies": { "count": ids.len(), "ids": ids, "is_empty": empty },
        "invariants": { "count": invariants.len(), "ids": invariant_ids },
        "warnings": warnings,
        "diagnostics": Vec::<String>::new(),
    })))
}

/// One row of a `--scenario` file: what to ask the gate, and what the author expects back.
#[derive(Debug, Deserialize)]
pub struct Scenario {
    /// What this case is called, for the line the operator reads.
    pub name: String,
    /// 42 §3.1's substrate. `fs` / `git` / `mcp` / `custom:<NAME>`.
    pub substrate: String,
    /// 42 §3.1's locator, in the adapter's own spelling.
    pub locator: String,
    /// 42 §3.2's `ChangeContext`.
    pub context: String,
    /// The key the acting principal is identified by (ASM-60-1 maps `actor.key()` and nothing else).
    pub actor_key: String,
    /// 41 §3's order. v0.1 admits 0..=2 (ASM-6).
    #[serde(default)]
    pub order: u8,
    /// FR-043's flag. **E-M3-4** makes `false` the one condition producing an `Escalate` in v0.1.
    #[serde(default = "yes")]
    pub invert_available: bool,
    /// 42 §3.7's values, passed to the gate 「そのまま」 (AC-016).
    #[serde(default)]
    pub evidence: Vec<gx_witness::Evidence>,
    /// `Admit` / `Deny` / `Escalate`.
    pub expect: VerdictKind,
}

const fn yes() -> bool {
    true
}

/// 🔴 `gx policy test <PATH> --scenario <FILE>` (44 §1.2).
///
/// > 指定シナリオ（Intent/Evidence/期待Verdictの組）に対しGate評価を実行し期待値と照合
///
/// One process, one pack, every case in the file. The exit is 44 §1.2's 「0=問題なし, 1=lint/test
/// エラーあり」, and a case whose gate call **failed** (`Gate::verify` is fallible — E-M3-3) is
/// reported as a failure with its refusal rather than as a mismatch: 「the policy could not be
/// evaluated」 and 「the policy said something else」 are different facts and E-M3-3 is the standing
/// rule against giving them one face.
///
/// # Errors
/// Whatever [`load`] refuses, [`Error::Io`] for an unreadable scenario file, [`Error::Malformed`]
/// for one that is not a list of [`Scenario`], and [`Error::Usage`] for a scenario naming a
/// substrate, context or order this build cannot represent.
pub fn test(path: &Path, scenario: &Path) -> Result<Outcome> {
    let gate = Gate::with_policies(load(path)?);
    let raw = std::fs::read_to_string(scenario).map_err(crate::io("read", scenario))?;
    let scenarios: Vec<Scenario> = parse_scenarios(&raw, scenario)?;

    let cases = scenarios
        .iter()
        .map(case_of)
        .collect::<Result<Vec<PackCase>>>()?;
    let report = packs::check_pack(&gate, &cases)?;

    let rows: Vec<serde_json::Value> = scenarios
        .iter()
        .zip(report.rows())
        .map(|(scenario, row)| {
            serde_json::json!({
                "name": row.name,
                "expected": scenario.expect,
                "actual": row.actual,
                "pass": row.pass,
                "detail": row.detail,
            })
        })
        .collect();

    let failed = report.failures().len();
    let json = serde_json::json!({
        "path": path.display().to_string().replace('\\', "/"),
        "scenario": scenario.display().to_string().replace('\\', "/"),
        "cases": rows,
        "passed": rows.len() - failed,
        "failed": failed,
    });
    Ok(if failed == 0 {
        Outcome::ok(json)
    } else {
        Outcome::refused(json, ERROR)
    })
}

/// A scenario file: a JSON array, or one object for the single-case form.
fn parse_scenarios(raw: &str, at: &Path) -> Result<Vec<Scenario>> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|detail| Error::Malformed {
            what: "scenario file",
            path: at.display().to_string(),
            detail: detail.to_string(),
        })?;
    let cases = if value.is_array() {
        serde_json::from_value(value)
    } else {
        serde_json::from_value(value).map(|one: Scenario| vec![one])
    };
    cases.map_err(|detail| Error::Malformed {
        what: "scenario file",
        path: at.display().to_string(),
        detail: detail.to_string(),
    })
}

/// The [`PackCase`] a scenario describes — the operator's strings, as the one road's types.
///
/// The mapping of the two string fields stays here: `substrate` and `context` are 44 §1.2's
/// spellings and `crate::substrate_kind` / `crate::change_context` are where this binary refuses one
/// it cannot represent. What is **not** here any more is the hypothetical `Transformation` — that is
/// `gx_gate::packs`'s, so that a scenario and a shipped pack's conformance case reach a gate through
/// one construction rather than two (G-4; see the module header).
fn case_of(scenario: &Scenario) -> Result<PackCase> {
    let expect = match scenario.expect {
        // No policy id: 44 §1.2 gives a scenario 「期待Verdict」 and nothing finer, so the weaker
        // expectation is the honest one to build. `Deny(None)` is satisfied by a refusal from a
        // statement **and** by Cedar's third rule, which is what this command has always meant.
        VerdictKind::Admit => PackExpectation::Admit(None),
        VerdictKind::Deny => PackExpectation::Deny(None),
        VerdictKind::Escalate => PackExpectation::Escalate,
    };
    let mut case = PackCase::new(
        scenario.name.clone(),
        crate::substrate_kind(&scenario.substrate)?,
        scenario.locator.clone(),
        expect,
    )
    .in_context(crate::change_context(&scenario.context)?)
    .by(scenario.actor_key.clone())
    .at_order(scenario.order)
    .with_evidence(scenario.evidence.clone());
    if !scenario.invert_available {
        case = case.without_inverse();
    }
    Ok(case)
}
