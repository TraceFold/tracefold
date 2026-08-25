// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx policy lint` / `gx policy test` (44 §1.2), and the three accessors M5 armed with a kill
//! condition.
//!
//! # 🔴 **M6-21** — this file is the consumer, and the naming is now paid
//!
//! §41 M5H4-8 armed `Gate::policies`, `Gate::invariants` and `PolicyEngine::is_empty` with "if the
//! M6 reqdef names not a single concrete consumer, M6 hand 1 carries out the retirement-mark
//! form" (sem: SEM-gx-cli-159). req/88 §4 M6-21 named three
//! consumers and req/38 §47 adopted (a) dissolved the kill condition **with a second condition attached**:
//!
//! > naming alone does not make a consumer -- the relevant hand's DoD must include "machine-count
//! > each of the 3 accessors' call sites; 0 is RED" (sem: SEM-gx-cli-160)
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
//! * **`PolicyEngine::is_empty`** — "came up with zero policies" (sem: SEM-gx-cli-161) is the most dangerous configuration a
//!   fail-closed deployment can be in, and it is invisible unless something asks.
//!
//! # 🔴 What `gx policy test` is, in Rule 1's terms (sem: SEM-gx-cli-162)
//!
//! It calls [`gx_gate::Gate::verify`], which is 44 §1.2's own instruction — "run Gate evaluation
//! against the specified scenario (an Intent/Evidence/expected-Verdict tuple) and check it against
//! the expected value" (sem: SEM-gx-cli-163) — and it is **not** a second
//! judgement. The gate answers; this reports the answer beside the operator's expectation and
//! exits on whether the two agree. No `Verdict` is constructed here (Rule 1 (ii); sem: SEM-gx-cli-164): what leaves this
//! module is [`gx_core::VerdictKind`], the display discriminant gx-core owns.
//!
//! # 🔴 A scenario's identity fields are placeholders, and that is a consequence of M3-10
//!
//! `GateInput` wants a whole `Transformation` and a whole `ObjectSnapshot`, and a scenario file
//! cannot supply the ids of things that do not exist. It does not have to: req/38 §19's M3-10 fixes
//! what a pack may reason over — "v0.1 pack's effective scope = **locator/actor/context/order
//! class**" (sem: SEM-gx-cli-165) — and
//! the object id, the digest and the transformation id are not in it. So the six facts a policy can
//! read come from the file and the rest are zeroes, which is honest for a hypothetical and is
//! stated here rather than discovered by somebody whose scenario "did not take" (sem: SEM-gx-cli-166). A pack that grew
//! a rule about an id would make this module's placeholders visible as wrong answers, and that is
//! the day M3-10's range moved.
//!
//! # 🔴 **G-4** — this command and the shipped packs' own conformance are one road (**M7 hand 2**)
//!
//! `req/38_ERRATA_2026-08-07.md` §19 reserved G-4 for M7: "when a third-party pack is fed in, the
//! conformance check **runs through the same single road** (does not branch between in-house and
//! third-party)" (sem: SEM-gx-cli-167). A pack an operator names on the command line
//! is exactly "a third-party pack", so this module used to be the second implementation of the thing
//! `crates/gx-gate/tests/ac_028.rs` did for the shipped one: its own case type, its own loop, its own
//! `GateInput` construction. Two implementations of one check are two answers waiting to differ, and
//! the one a stranger runs would have been the one nobody's acceptance criterion measured.
//!
//! So the loop and the hypothetical moved into `gx_gate::packs::check_pack`, which is library code
//! this command calls and `crates/gx-gate/tests/ac_074.rs` calls. What stays here is the operator's
//! half: reading the file, mapping its strings onto types, and rendering 44 §1.2's output. **Rule 1 (ii) (sem: SEM-gx-cli-168)
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
/// stay separate on purpose: "there is exactly one road by which a pack's file gets embedded into
/// the build" (sem: SEM-gx-cli-169) is a claim about the
/// build, and a pack an operator names on the command line is not in the build.
///
/// # Errors
/// [`Error::Io`] if the file cannot be read, [`Error::Gate`] if Cedar refuses it or a statement
/// carries no `@id` (ASM-62-1).
pub fn load(path: &Path) -> Result<PolicyEngine> {
    let src = std::fs::read_to_string(path).map_err(crate::io("read", path))?;
    Ok(PolicyEngine::parse(&src)?)
}

/// 🔴 `gx policy lint <PATH>` (44 §1.2) — "Cedar policy syntax/schema verification" (sem: SEM-gx-cli-170), **and the invariant half**.
///
/// 44 §1.2 writes only the Cedar side. FR-027 makes a `Verdict` the composition of policies and
/// invariants, so a linter that reported on one of the two would be answering "is this deployment's
/// admissibility predicate well formed" (sem: SEM-gx-cli-171) with a check of half of it. Both counts are printed, and
/// the invariant registry being empty is reported as a **fact** rather than as a fault: 41 §4 types
/// it as a `Vec` and "nobody was asked" (sem: SEM-gx-cli-172) is a state a deployment may legitimately be in.
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
             rule to name (M6-21: the startup warning for \"came up with zero policies\"; sem: SEM-gx-cli-173)"
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
    /// 42 §3.7's values, passed to the gate "as-is" (sem: SEM-gx-cli-174) (AC-016).
    #[serde(default)]
    pub evidence: Vec<gx_witness::Evidence>,
    /// `Admit` / `Deny` / `Escalate`.
    pub expect: VerdictKind,
    /// 🔴 **Which statement decided it** (req/446 AC-PP-10), when the author wants to say.
    ///
    /// G-4 asks that "when a third-party pack is dropped in, conformance checking runs down the
    /// same single road". Until this field existed the road was the same but the **vehicle** was
    /// not: a shipped pack's own conformance table writes `PackExpectation::deny_by("git-deny-
    /// nonbranch-refs")` and a third party writing a scenario file could only reach
    /// `Deny(None)` — an expectation satisfied by a refusal from a statement *and* by Cedar's
    /// third rule, which are different facts. A pack whose forbid silently stopped matching would
    /// keep every one of its Deny cases green. Naming the id closes that, because 42 §1.3 puts
    /// `PolicyDecisionRecord` inside `AdmitProof`'s IdentityView: which rule answered is part of
    /// what a receipt claims, so it is part of what a conformance case may assert.
    ///
    /// Refused in combination with `deny_by_no_policy` (a decision cannot both name a statement
    /// and name none) and with `expect: "Escalate"` (E-M3-4's escalation is the gate's, not a
    /// statement's).
    #[serde(default)]
    pub expect_policy_id: Option<String>,
    /// 🔴 **That nothing had an opinion** (req/446 AC-PP-10) — Cedar's third rule, asserted.
    ///
    /// The other half of the expressiveness gap, and the one a **deny-default** pack cannot ship
    /// without: `policies/postgres/` declares no permit, so the sentence its scenario file has to
    /// be able to write is "this request reaches no statement of any pack and is denied for that
    /// reason". `Deny(None)` cannot say it — it is satisfied either way, which is exactly the
    /// green-for-the-wrong-reason failure `PACK_FORMAT.md` F4 is about.
    ///
    /// Only meaningful with `expect: "Deny"`; refused otherwise.
    #[serde(default)]
    pub deny_by_no_policy: bool,
}

const fn yes() -> bool {
    true
}

/// 🔴 `gx policy test <PATH> --scenario <FILE>` (44 §1.2).
///
/// > run Gate evaluation against the specified scenario (an Intent/Evidence/expected-Verdict
/// > tuple) and check it against the expected value (sem: SEM-gx-cli-175)
///
/// One process, one pack, every case in the file. The exit is 44 §1.2's "0 = no problem, 1 =
/// lint/test error present" (sem: SEM-gx-cli-176), and a case whose gate call **failed** (`Gate::verify` is fallible — E-M3-3) is
/// reported as a failure with its refusal rather than as a mismatch: "the policy could not be
/// evaluated" and "the policy said something else" are different facts and E-M3-3 is the standing
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

    // 🔴 `expected_by` and `deciding` are reported beside the arm, and they have to be, now that
    // an expectation can be finer than the arm (AC-PP-10). Without them a row that named the wrong
    // statement printed `expected Deny, actual Deny, pass false` — a refusal an operator cannot
    // read, and the one shape of failure the new fields exist to produce. `deciding` is the fact
    // the row was judged against; `expected_by` is what the file asked for.
    let rows: Vec<serde_json::Value> = scenarios
        .iter()
        .zip(report.rows())
        .map(|(scenario, row)| {
            let expected_by = if scenario.deny_by_no_policy {
                serde_json::json!("<no policy applied>")
            } else {
                match &scenario.expect_policy_id {
                    Some(id) => serde_json::json!(id),
                    None => serde_json::Value::Null,
                }
            };
            serde_json::json!({
                "name": row.name,
                "expected": scenario.expect,
                "expected_by": expected_by,
                "actual": row.actual,
                "deciding": row.deciding,
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
    let expect = expectation_of(scenario)?;
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

/// 🔴 The [`PackExpectation`] a scenario row states (req/446 AC-PP-10).
///
/// Three shapes, and the two refusals between them are the point. 44 §1.2 originally gave a
/// scenario an "expected Verdict" and nothing finer, so every row landed on the weak arm-only
/// expectation; `expect_policy_id` and `deny_by_no_policy` are the two sentences a shipped pack's
/// own conformance table could already write and an operator's scenario file could not.
///
/// The refusals are here rather than in serde because they are about a *pair* of fields:
///
/// * both set — a decision either names the statement that made it or names none of them, and a
///   row claiming both describes no verdict that can occur.
/// * either set with `expect: "Escalate"` — E-M3-4 makes `invert_available == false` the one
///   condition that escalates in v0.1, and it is the gate's rule rather than a statement's, so
///   there is no policy id to name and no third rule to have fallen to.
///
/// Both come back as [`Error::Usage`] for the reason [`crate::substrate_kind`] does: the file is
/// readable and its JSON is well-formed: what it asks for is not a thing the gate can be asked.
///
/// # Errors
/// [`Error::Usage`] for either combination above.
fn expectation_of(scenario: &Scenario) -> Result<PackExpectation> {
    if scenario.expect_policy_id.is_some() && scenario.deny_by_no_policy {
        return Err(Error::Usage {
            detail: format!(
                "scenario {:?} sets both `expect_policy_id` and `deny_by_no_policy`; \
                 a verdict names the statement that decided it or names none, never both",
                scenario.name
            ),
        });
    }
    if scenario.expect == VerdictKind::Escalate
        && (scenario.expect_policy_id.is_some() || scenario.deny_by_no_policy)
    {
        return Err(Error::Usage {
            detail: format!(
                "scenario {:?} expects Escalate and also names a deciding statement; \
                 E-M3-4's escalation is the gate's rule (invert_available == false)",
                scenario.name
            ),
        });
    }
    if scenario.deny_by_no_policy && scenario.expect != VerdictKind::Deny {
        return Err(Error::Usage {
            detail: format!(
                "scenario {:?} sets `deny_by_no_policy` but expects {:?}; Cedar's third \
                 rule produces a Deny and nothing else",
                scenario.name, scenario.expect
            ),
        });
    }
    Ok(
        match (scenario.expect, scenario.expect_policy_id.as_deref()) {
            (VerdictKind::Admit, id) => PackExpectation::Admit(id.map(str::to_string)),
            (VerdictKind::Deny, _) if scenario.deny_by_no_policy => PackExpectation::DenyByNoPolicy,
            (VerdictKind::Deny, id) => PackExpectation::Deny(id.map(str::to_string)),
            (VerdictKind::Escalate, _) => PackExpectation::Escalate,
        },
    )
}
