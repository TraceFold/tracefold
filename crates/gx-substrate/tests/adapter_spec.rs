// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **a preview of I-11's shape** — 41 §4's `SubstrateAdapter` read out of the canon and compared
//! with the (sem: SEM-gx-substrate-092, SEM-gx-substrate-093, SEM-gx-substrate-094,
//! SEM-gx-substrate-095, SEM-gx-substrate-096, SEM-gx-substrate-097, SEM-gx-substrate-098,
//! SEM-gx-substrate-099, SEM-gx-substrate-100, SEM-gx-substrate-101, SEM-gx-substrate-102)
//! trait this crate ships.
//!
//! req/69 §6.2 hand 2, verbatim: "**place, from the start, a test that parses 41 §4's markdown and
//! cross-checks it against the concrete type** (B-4's `gate_input_spec` shape) = bring in I-11's
//! shape ahead of time". `crates/gx-gate/tests/gate_input_spec.rs`
//! is the precedent and this is the same instrument one type over: a struct's five field names there,
//! a trait's seven signatures here.
//!
//! # Why a trait needs it more than a struct did
//!
//! 52 contract 2 forbids adding a method the requirements do not ask for, and N-08 (req/69 §1)
//! spells that out for this trait: "do not grow `SubstrateAdapter` any methods beyond 41 §4's 7". A
//! count
//! written into a report is a claim about the day the report was written; a count taken from the
//! canon and from the source on every run is the thing that stays true. The eighth method M4-05 and
//! M4-07 both wanted (`can_invert`, `compose_delta`) was ruled out in §28 -- E-M4-5 folds the first
//! into the engine's verify step and M4-07 (c) puts composition in the payload as a free monoid --
//! so the guard below is what those two rulings look like from the compiler's side.
//!
//! # The one signature that differs, and why that is not drift
//!
//! **E-M4-4** (req/38 §28) changed `plan`:
//!
//! > "change the signature to `plan(&self, intent: &Intent, pre: &ObjectSnapshot) ->
//! > Result<PlannedDelta>` (a 41 §4 erratum; the method count stays 7 = no conflict with 52 contract
//! > 2; the E-M3-7 precedent)"
//!
//! `req/spec/` is frozen (52), so 41 §4 still writes the one-argument form and the erratum ledger
//! `req/38_ERRATA_2026-08-07.md` is the canonical source. This suite therefore holds **two** claims
//! about `plan`:
//! that the canon still writes the old signature (so the erratum is still live and can be closed
//! when the canon moves), and that the implementation writes E-M4-4's. That pair is exactly what
//! `gate_input_spec.rs`'s `e_m3_7_the_spec_still_writes_the_two_argument_signature` does for the
//! `InvariantCheck::check` erratum, and it is the only shape that lets a frozen canon and an amended
//! implementation both be measured instead of one of them being trusted.
//!
//! # The change **E-M4-28** makes that these signatures cannot see
//!
//! 41 §4 writes a bare `Result<..>` in all seven, and hand 3 changed which crate that name resolves
//! to -- from `gx_core::Result` to this crate's own (`req/38` §30 M4H2-2, adopted (a)). Every
//! signature
//! string above is byte-identical either way, which is precisely the kind of change a text
//! comparison declares "no drift" about while the meaning of every method's failure moves one layer.
//! So [`e_m4_28_the_bare_result_is_this_crates_own`] measures the import instead: 41 §4 says
//! `Result`, and the erratum says whose.

use std::path::{Path, PathBuf};

/// The seven names, in the order 41 §4 writes them. Compared against the canon and against the
/// source rather than trusted -- a third declaration is what makes a drift on either side visible
/// (`gate_input_spec.rs`'s `DECLARED_FIELDS` is the same third party).
const DECLARED_METHODS: [&str; 7] = [
    "kind",
    "snapshot",
    "plan",
    "precondition",
    "apply",
    "invert",
    "commutation",
];

/// The line both files open the trait with. 41 §4 writes the supertrait bound that AC-046 measures,
/// so the two have to agree on it before AC-046 means anything.
const DECLARATION: &str = "pub trait SubstrateAdapter: Send + Sync {";

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<repo>/crates/gx-substrate`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

fn spec_41() -> String {
    std::fs::read_to_string(repo_root().join("req/spec/40-architecture/41-architecture.md"))
        .expect("41 is readable")
}

fn adapter_rs() -> String {
    std::fs::read_to_string(repo_root().join("crates/gx-substrate/src/adapter.rs"))
        .expect("crates/gx-substrate/src/adapter.rs is readable")
}

/// The method signatures of a trait block, read out of a file that declares it.
///
/// One function for both readings -- the canon's markdown and the crate's source -- because the two
/// lists have to be compared and a second parser is a second answer to what a signature is.
/// Comment lines are dropped before anything else, so a doc comment carrying an example that starts
/// with `fn` is not read as a method (the source has several).
fn signatures_in(text: &str, declaration: &str) -> Vec<String> {
    let mut lines = text.lines().skip_while(|l| !l.starts_with(declaration));
    assert!(
        lines.next().is_some(),
        "no line starts with `{declaration}` -- this test is reading the wrong file"
    );

    let mut out: Vec<String> = Vec::new();
    let mut pending = String::new();
    for line in lines {
        if line.starts_with('}') {
            assert!(
                pending.is_empty(),
                "the trait block closes inside a signature: {pending:?}"
            );
            return out;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("#[") {
            continue;
        }
        if pending.is_empty() && !trimmed.starts_with("fn ") {
            continue;
        }
        if !pending.is_empty() {
            pending.push(' ');
        }
        pending.push_str(trimmed);
        if trimmed.ends_with(';') {
            out.push(pending.split_whitespace().collect::<Vec<_>>().join(" "));
            pending.clear();
        }
    }
    panic!("the `{declaration}` block is not closed");
}

/// `fn kind(&self) -> SubstrateKind;` -> `kind`.
fn method_name(signature: &str) -> String {
    signature
        .strip_prefix("fn ")
        .expect("a signature starts with `fn `")
        .split('(')
        .next()
        .expect("split yields at least one part")
        .trim()
        .to_string()
}

fn names(signatures: &[String]) -> Vec<String> {
    signatures.iter().map(|s| method_name(s)).collect()
}

fn from_41() -> Vec<String> {
    signatures_in(&spec_41(), DECLARATION)
}

fn from_source() -> Vec<String> {
    signatures_in(&adapter_rs(), DECLARATION)
}

// ---------------------------------------------------------------------------
// The comparison I-11 asks for: the canon against the type
// ---------------------------------------------------------------------------

/// 41 §4's seven method names are the seven the implementation declares, in that order.
#[test]
fn the_trait_declares_the_seven_methods_41_4_declares() {
    let spec = names(&from_41());
    let implemented = names(&from_source());
    println!(
        "ADAPTER_METHODS_SPEC={} ADAPTER_METHODS_IMPL={}",
        spec.len(),
        implemented.len()
    );
    assert_eq!(
        implemented, spec,
        "41 §4's SubstrateAdapter and gx-substrate's no longer agree on which methods exist"
    );
    assert_eq!(spec, DECLARED_METHODS.to_vec());
}

/// **N-08**: seven, and the eighth that two rulings wanted is not there.
///
/// M4-05 (a) put `invert_available` in the engine's verify step rather than in a `can_invert`, and
/// M4-07 (c) put composition in the payload rather than in a `compose_delta`. Both rulings are only
/// as real as this count.
#[test]
fn n_08_the_trait_has_seven_methods_and_no_eighth() {
    let implemented = from_source();
    assert_eq!(
        implemented.len(),
        7,
        "the trait declares {} methods: {:?}. 52 contract 2 and N-08 fix the number at seven; an \
         eighth needs a ruling, not an edit",
        implemented.len(),
        names(&implemented)
    );
}

/// The supertrait bound AC-046 is about is written on both sides.
///
/// AC-046 measures that `Box<dyn SubstrateAdapter>` is `Send + Sync`; that is a property of the
/// bound in the declaration, so the bound is checked here and the consequence in `ac_046.rs`.
#[test]
fn the_supertrait_bound_is_send_plus_sync_in_both_files() {
    assert!(
        spec_41().contains(DECLARATION),
        "41 §4 no longer declares `{DECLARATION}`"
    );
    assert!(
        adapter_rs().contains(DECLARATION),
        "the implementation no longer declares `{DECLARATION}`; AC-046 has nothing to measure"
    );
}

/// **E-M4-4**: the canon still writes `plan` without a pre-state, and the implementation does not.
///
/// Two assertions on purpose. The first says the erratum is still live -- when a later revision of
/// 41 §4 adopts E-M4-4, this fails and the erratum can be closed (req/08 N-1's shape, and
/// `gate_input_spec.rs`'s two erratum probes are the precedent). The second says the implementation
/// is the amended form rather than the frozen one.
#[test]
fn e_m4_4_the_canon_writes_plan_without_a_pre_state_and_the_trait_writes_it_with_one() {
    let spec = from_41();
    let implemented = from_source();

    let spec_plan = spec
        .iter()
        .find(|s| method_name(s) == "plan")
        .expect("41 §4 declares `plan`");
    let impl_plan = implemented
        .iter()
        .find(|s| method_name(s) == "plan")
        .expect("the trait declares `plan`");

    assert_eq!(
        spec_plan, "fn plan(&self, intent: &Intent) -> Result<PlannedDelta>;",
        "41 §4's `plan` changed; E-M4-4 can be closed"
    );
    assert_eq!(
        impl_plan, "fn plan(&self, intent: &Intent, pre: &ObjectSnapshot) -> Result<PlannedDelta>;",
        "E-M4-4, verbatim: \"change the signature to `plan(&self, intent: &Intent, pre: \
         &ObjectSnapshot) -> Result<PlannedDelta>`\""
    );
    assert_ne!(
        spec_plan, impl_plan,
        "if these agree, one of the two assertions above is measuring nothing"
    );
}

/// The other five signatures are 41 §4 word for word.
///
/// 🔴 **DR-46-26** — six until E-DR4626-1. The sentence below is what this test is *for* and it is
/// unchanged: this is the assertion that the amendments are **exactly the ones an erratum records**,
/// so the number moving is the erratum being registered rather than the guarantee weakening. A
/// third quiet divergence would still be a change to the boundary that nothing records.
#[test]
fn the_six_unamended_signatures_are_41_4_verbatim() {
    let spec = from_41();
    let implemented = from_source();
    let mut compared = 0usize;
    for wanted in &spec {
        let name = method_name(wanted);
        // 🔴 **E-DR4626-1 (DR-46-26)** — the second amendment, and the second name skipped here.
        // Both halves of the difference are measured by
        // `e_dr4626_1_the_canon_writes_invert_returning_an_option_and_the_trait_writes_an_outcome`,
        // which is the same arrangement E-M4-4 has: a signature is either **verbatim here** or
        // **held on both sides there**, never neither.
        if name == "plan" || name == "invert" {
            continue;
        }
        let got = implemented
            .iter()
            .find(|s| method_name(s) == name)
            .unwrap_or_else(|| panic!("the trait does not declare `{name}`"));
        assert_eq!(
            got, wanted,
            "`{name}` diverges from 41 §4 and no erratum records it"
        );
        compared += 1;
    }
    println!("ADAPTER_SIGNATURES_VERBATIM={compared} AMENDED=2 (plan, E-M4-4; invert, E-DR4626-1)");
    assert_eq!(
        compared, 5,
        "five signatures are unamended; `plan` is E-M4-4's and `invert` is E-DR4626-1's"
    );
}
/// 🔴 **E-DR4626-1**: the canon still writes `invert` returning a bare `Option`, and the trait
/// returns an [`gx_substrate::InvertOutcome`].
///
/// The second instance of the shape [`e_m4_4_the_canon_writes_plan_without_a_pre_state_and_the_trait_writes_it_with_one`]
/// established, written the same way on purpose: three assertions, one per direction plus the
/// difference. The first says the erratum is still live -- when a later revision of 41 §4 adopts
/// E-DR4626-1 this test fails and the erratum can be closed. The second says the implementation is
/// the amended form rather than the frozen one. The third says the two are not the same string,
/// because two assertions that both passed against identical text would be measuring nothing.
///
/// **N-08 is untouched and this test is beside the one that says so.**
/// [`n_08_the_trait_has_seven_methods_and_no_eighth`] compares *names*, and the name did not move:
/// DR-46-26 widened a return rather than adding the eighth method M4-05 and M4-07 were both refused
/// (`req/38` §28).
#[test]
fn e_dr4626_1_the_canon_writes_invert_returning_an_option_and_the_trait_writes_an_outcome() {
    let spec = from_41();
    let implemented = from_source();

    let spec_invert = spec
        .iter()
        .find(|s| method_name(s) == "invert")
        .expect("41 §4 declares `invert`");
    let impl_invert = implemented
        .iter()
        .find(|s| method_name(s) == "invert")
        .expect("the trait declares `invert`");

    assert_eq!(
        spec_invert,
        "fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<Option<PlannedDelta>>;",
        "41 §4's `invert` changed; E-DR4626-1 can be closed"
    );
    assert_eq!(
        impl_invert,
        "fn invert(&self, delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome>;",
        "E-DR4626-1, verbatim: \"change the signature to `invert(&self, delta: &PlannedDelta, pre: \
         &ObjectSnapshot) -> Result<InvertOutcome>`\""
    );
    assert_ne!(
        spec_invert, impl_invert,
        "if these agree, one of the two assertions above is measuring nothing"
    );
    println!("E_DR4626_1_CANON={spec_invert:?} IMPL={impl_invert:?}");
}

/// 🔴 **AC-S3 (DR-46-26)**: no adapter in this workspace chooses a read-set granularity.
///
/// `req/441` §4 is the rule -- "spill is the constructor's decision (`ReadSet::from_reads`); a form
/// in which the caller picks the variant makes the granularity tag a function of the caller's mood
/// rather than of the number of reads" -- and D24 built `ReadSet::from_reads` to hold it. DR-46-26
/// hands adapters a seat for read entries, which is the first window in which an adapter *could*
/// have reached for a variant. This is the scan that says none does.
///
/// It reads the adapter crates' sources rather than trusting the type: `InvertOutcome` carries a
/// `Vec<ReadEntry>` today and a later hand widening it would not fail any behavioural test.
#[test]
fn ac_s3_no_adapter_picks_a_read_set_granularity() {
    let mut named: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for crate_dir in [
        "gx-adapter-fs",
        "gx-adapter-git",
        "gx-adapter-mcp",
        "gx-adapter-postgres",
    ] {
        let src = repo_root().join("crates").join(crate_dir).join("src");
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("an adapter source is readable");
                scanned += 1;
                // Code lines only. A doc comment in `gx-adapter-mcp/src/invert.rs` names
                // `ReadSet::from_reads` in order to say that the *engine* owns it, and a scan that
                // could not tell a mention from a use would fail on the sentence that states the
                // rule it is checking.
                for (n, line) in text.lines().enumerate() {
                    let code = line.trim_start();
                    if code.starts_with("//") || code.starts_with("/*") || code.starts_with('*') {
                        continue;
                    }
                    for needle in [
                        "ReadSet::PerRead",
                        "ReadSet::PerEffectRoot",
                        "ReadSet::from_reads",
                    ] {
                        if code.contains(needle) {
                            named.push(format!("{}:{}: {needle}", path.display(), n + 1));
                        }
                    }
                }
            }
        }
    }
    println!(
        "AC_S3_ADAPTER_SOURCES={scanned} READSET_MENTIONS={}",
        named.len()
    );
    assert!(
        scanned >= 4,
        "the scan found almost no adapter sources ({scanned}); it is measuring nothing"
    );
    assert!(
        named.is_empty(),
        "an adapter names a `ReadSet` and so decides the granularity: {named:#?}"
    );
}

// ---------------------------------------------------------------------------
// E-M4-28: the bare `Result` of 41 §4, and whose it is
// ---------------------------------------------------------------------------

/// The seven signatures return **this crate's** `Result`, not gx-core's.
///
/// `req/38_ERRATA_2026-08-07.md` §30 M4H2-2, adopted (a), verbatim: "declare `gx_substrate::Error`+
/// `Result` (as the repo's 5 other crates already do) ... putting it **at the start of hand 3 rather
/// than hand 4** is to avoid the rework of building the conformance harness on the wrong `Result`
/// type and then swapping it out", and the reason it had to be a ruling is req/71 §2 M4H2-2:
/// `gx_core::Error` is ten variants of rejected argument, so an fs adapter's `snapshot` could not
/// report a file it failed to read with any of them.
///
/// # Why the import and not the signature
///
/// Because the signature does not move. `-> Result<ObjectSnapshot>` is the same eighteen characters
/// before and after the swap, so [`the_six_unamended_signatures_are_41_4_verbatim`] passes either
/// way -- it is comparing the canon's text with the source's text, and both say `Result`. What
/// changed is which type that name denotes, which lives one line above the trait. A hand that put
/// `gx_core::Result` back would leave every assertion in this file green, so the assertion has to be
/// about the line that decides.
#[test]
fn e_m4_28_the_bare_result_is_this_crates_own() {
    let source = adapter_rs();
    let imports: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("use "))
        .collect();

    let from_core: Vec<&&str> = imports
        .iter()
        .filter(|l| l.starts_with("use gx_core::") && l.contains("Result"))
        .collect();
    println!(
        "ADAPTER_RESULT_FROM_CRATE={} ADAPTER_RESULT_FROM_GX_CORE={}",
        imports
            .iter()
            .filter(|l| l.starts_with("use crate::") && l.contains("Result"))
            .count(),
        from_core.len()
    );
    assert!(
        from_core.is_empty(),
        "`adapter.rs` still imports gx-core's Result ({from_core:?}); after E-M4-28 the bare \
         `Result` of 41 §4 is `gx_substrate::Result`, and an adapter that cannot say \"could not be \
         read\" is the defect req/71 §2 M4H2-2 raised"
    );
    assert!(
        imports
            .iter()
            .any(|l| l.starts_with("use crate::") && l.contains("Result")),
        "`adapter.rs` imports no `Result` from this crate, so the seven signatures name a type this \
         file does not declare where it can be reviewed (E-M4-28)"
    );

    // The other half: the ruling is named where the swap happened, so a reader meeting
    // `Result<ObjectSnapshot>` can find out whose it is without a `git blame`.
    assert!(
        source.contains("E-M4-28"),
        "`adapter.rs` does not cite E-M4-28; the ruling that decides what every one of these seven \
         signatures returns is not written down where they are"
    );
}
