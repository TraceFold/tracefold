// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R39 / `req/533` M-02(b)** — the two claims `docs/LIMITS.md` made about the frozen 2026-08-18
//! specimen that no instrument in this tree was measuring.
//!
//! # What audit 38 counted
//!
//! `docs/LIMITS.md` §"A receipt this product issued in August 2026 does not verify against this
//! build" carried two numbers in public prose:
//!
//! 1. "`gx receipt verify` answers exit **7** for this document" — no test ran the binary against
//!    the specimen at all. `crates/gx-witness` cannot: it has no `[[bin]]` dependency, so
//!    `CARGO_BIN_EXE_gx` does not exist there, and the corpus suite lives in that crate.
//! 2. "the re-encoding is still 25 bytes longer" — the test named
//!    `the_re_encoding_gap_is_not_only_the_two_required_members` performs **no re-encoding**. The
//!    string `re_encoded_bytes` appears nowhere in the tree. The number was taken on a build that
//!    was then withdrawn.
//!
//! A public page carrying a number that nothing re-derives is a number that becomes false in
//! silence, which is the same shape §294's declaration/alarm pair exists to prevent one file over.
//!
//! # What this suite does about each
//!
//! Claim 1 is measurable and is measured: this crate **does** have the binary, so the specimen is
//! handed to `gx receipt verify` as a third party would hand it over, and the exit is asserted. A
//! control that changes one byte of the specimen is asserted to answer differently, so that a
//! green here cannot be produced by a suite that never opened the file.
//!
//! Claim 2 is **not** measurable, and `req/540` R-2d named that outcome a legitimate landing before
//! this lane started. The re-encoding it describes is `decode → load into today's `ReceiptPayload`
//! → canonical-encode → compare lengths`, and step one does not exist: the document does not
//! decode, which is the limit itself. There is no build in the tree that produced 25 and no build
//! this lane can reach that would. So the number came off the page and what replaced it is what
//! this file measures: the specimen carries eleven members and this build's payload names twenty.
//!
//! 🔴 **`req/901` (2026-08-26): "fifteen" was stale — the count is seventeen, and it had been wrong
//! since `confinement` and `catalogue_hash` landed.** The reason it went stale silently is worth
//! keeping: this file's dynamic check asserts `named > carried`, which stays green at any number
//! above eleven, so the sentence and the assertion could disagree for as long as they liked. The
//! assertion is **not** changed here — that needs `cargo`, held by another lane — but
//! `tools/receipt_generation_gate.mjs` now counts the struct and fails on a stale statement of the
//! count, from outside the test suite. `req/901` §5 item 2 carries the equality repair.
//!
//! 🔴 **`req/919` W5 (2026-08-29): seventeen to eighteen — `payload_version` (F7, `req/868`
//! R-868-6) landed.** Same discipline: the sentence above is edited to the live count, this
//! paragraph is the additive record of the move.
//!
//! # The specimen is not copied here
//!
//! `req/540` R-2b: one specimen, one copy. This reads the fixture out of `crates/gx-witness` across
//! the crate boundary rather than duplicating it, because two copies of a frozen artefact are two
//! things that can rot apart, and the one that rots is the one nobody looks at.

mod support;

use std::path::{Path, PathBuf};

use support::run;

/// The frozen artefacts, read where `crates/gx-witness` keeps them.
fn frozen(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("gx-witness")
        .join("tests")
        .join("fixtures")
        .join("frozen_receipts")
        .join("issued_2026_08_18")
        .join(name)
}

/// 44 §1.4's 7: "refuted".
const REFUTED: i32 = 7;

/// 🔴 **`req/540` AC-7 / R-2a** — `gx receipt verify` answers exit 7 for the frozen document, and
/// the page may say so because this ran.
///
/// The invocation is a third party's: `--offline`, the frozen checkpoint as the anchor, the frozen
/// public key, and no project anywhere near it. That is the shape `docs/LIMITS.md`'s sentence is
/// about, so it is the shape asserted.
#[test]
fn ac7_the_frozen_specimen_is_refuted_by_the_shipped_binary() {
    let receipt = frozen("receipt.json");
    let checkpoint = frozen("checkpoint.json");
    let key = frozen("key.pub.json");
    for path in [&receipt, &checkpoint, &key] {
        assert!(
            path.is_file(),
            "the bed failed before the product did: {} is not there. The fixture lives in \
             `crates/gx-witness` and is read across the crate boundary on purpose (`req/540` R-2b)",
            path.display()
        );
    }

    let out = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&receipt)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint)
        .arg("--key")
        .arg(&key));
    println!(
        "R39_FROZEN_CLI verify={} stdout={} stderr={}",
        out.code,
        out.stdout.trim(),
        out.stderr.trim()
    );
    assert_eq!(
        out.code, REFUTED,
        "🔴 `docs/LIMITS.md` says this document answers exit 7 against this build. If it answers \
         something else the page is now wrong — update the page, and check whether the leaf \
         derivation moved (`req/519` §7-5), because a receipt can start decoding and still compute \
         a leaf that is not the leaf that was committed"
    );
}

/// 🔴 **`req/540` AC-7's control** — and the answer comes from the file, not from the invocation.
///
/// A suite that never opened the specimen would pass the assertion above by accident, because the
/// binary answers 7 for a great many things. This copies the specimen into the scratch directory,
/// flips one byte inside the signed payload, and asserts the **answer about it** changes.
///
/// 🔴 What changes is not the exit code, and that is worth saying out loud rather than working
/// around: `gx receipt verify` answers 7 for the untouched specimen and 7 for the tampered one, so
/// the exit alone does not tell "this build cannot read the document" from "somebody edited it".
/// What separates them is the `checks.signature` member of the JSON on stdout, which is `true` for
/// the specimen (the signature over bytes nobody touched still holds — the reason
/// `frozen_receipt_corpus.rs` asserts it first and separately) and `false` for the control. So the
/// discrimination this control needs is asserted where it exists. The copy is a control and not a
/// second specimen: it is written under `CARGO_TARGET_TMPDIR` and goes with the target directory.
#[test]
fn ac7_control_a_specimen_with_one_byte_changed_answers_differently() {
    let dir = support::scratch("r39_frozen_control");
    let good = std::fs::read(frozen("receipt.json")).expect("read the frozen receipt");
    let mut tampered = good.clone();
    let at = tampered
        .iter()
        .position(|b| *b == b'A')
        .expect("the base64 payload holds an `A`");
    tampered[at] = b'B';
    let path = dir.join("tampered.json");
    std::fs::write(&path, &tampered).expect("write the control");

    let out = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(frozen("checkpoint.json"))
        .arg("--key")
        .arg(frozen("key.pub.json")));
    let untouched = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(frozen("receipt.json"))
        .arg("--offline")
        .arg("--checkpoint")
        .arg(frozen("checkpoint.json"))
        .arg("--key")
        .arg(frozen("key.pub.json")));
    let signature_of = |out: &support::Run| out.json()["checks"]["signature"].clone();
    println!(
        "R39_FROZEN_CLI control byte_at={at} tampered_exit={} tampered_signature={} \
         untouched_exit={} untouched_signature={}",
        out.code,
        signature_of(&out),
        untouched.code,
        signature_of(&untouched),
    );
    assert_eq!(
        signature_of(&untouched),
        serde_json::json!(true),
        "the bed failed before the product did: the frozen specimen's signature holds, which is \
         what makes the refusal a fact about this build"
    );
    assert_eq!(
        signature_of(&out),
        serde_json::json!(false),
        "🔴 `req/540` AC-7 control: a specimen with one byte changed inside the signed payload \
         answered exactly what the untouched one answered, so the probe above is not reading the \
         file it names"
    );
}

/// 🔴 **`req/540` AC-8 / R-2b** — the specimen exists once in this tree.
///
/// Two copies of a frozen artefact are two things that can be edited apart, and the instruction in
/// `frozen_receipt_corpus.rs` — "regenerating these files defeats the entire probe" — is only
/// enforceable if there is one place to regenerate. The count is taken over the whole repository
/// rather than over the two crates this lane touched, because a third copy anywhere is the failure.
#[test]
fn ac8_the_frozen_specimen_has_exactly_one_copy_in_the_tree() {
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if path.is_dir() {
                // `target/` holds copies this suite's own controls made, and `.git` holds every
                // version of everything. Neither is the tree.
                if name != "target" && name != ".git" && name != "node_modules" {
                    walk(&path, found);
                }
            } else if name == "receipt.json"
                && path
                    .parent()
                    .and_then(|p| p.file_name())
                    .is_some_and(|p| p == "issued_2026_08_18")
            {
                found.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-cli");
    let mut found = Vec::new();
    walk(root, &mut found);
    println!("R39_FROZEN_CLI copies={} {found:?}", found.len());
    assert_eq!(
        found.len(),
        1,
        "🔴 `req/540` AC-8: the frozen specimen is in {} places. One of them will be edited and the \
         other will not",
        found.len()
    );
}

/// 🔴 **`req/540` AC-9 / R-2c → R-2d** — the re-encoding gap, attempted and found unmeasurable, and
/// the size the limit does have instead.
///
/// The attempt is the first half of this test and it is kept rather than described: `payload()` is
/// the only road from the signed bytes to a value this build can re-encode, and it refuses. There
/// is no second road — `ReceiptPayload::ledger_digest` re-encodes the struct, so without the struct
/// there is nothing to re-encode. The number 25 was measured on a build R38 withdrew and cannot be
/// re-derived here, which is why `docs/LIMITS.md` no longer carries it (`req/540` R-2d).
///
/// What replaces it is measurable without a decoder: CBOR writes a map's member count into the head
/// byte, so the specimen says how many members it carries, and this build's `ReceiptPayload` says
/// how many it names. The gap has a size again, in members rather than in bytes, and every part of
/// it is re-derived on every run.
#[test]
fn ac9_the_re_encoding_cannot_be_measured_and_the_member_gap_can() {
    let text = std::fs::read_to_string(frozen("receipt.json")).expect("the frozen receipt is here");
    let receipt: gx_witness::Receipt =
        serde_json::from_str(&text).expect("the frozen receipt is a `Receipt` document");
    let signed = &receipt.envelope.payload;

    // The attempt. Not a formality: if this ever succeeds, the re-encoding **is** measurable and
    // `req/540` R-2c is back on, so the panic below says so.
    let refusal = match receipt.payload() {
        Ok(_) => panic!(
            "🔴 the frozen specimen now decodes, so the re-encoding `docs/LIMITS.md` describes can \
             be performed and this test should be replaced by one that performs it: decode, \
             canonical-encode, and print the byte difference against the {} signed bytes",
            signed.len()
        ),
        Err(e) => e.to_string(),
    };

    // CBOR major type 5 (a map) with the count in the low five bits, which is how
    // `frozen_receipt_corpus.rs`'s own note reads `0xab` as "a map of eleven". Counts of 24 and up
    // are written in following bytes; this refuses rather than guessing, because a specimen whose
    // head byte changed is not this specimen.
    let head = signed[0];
    assert_eq!(
        head & 0xe0,
        0xa0,
        "the bed failed before the product did: the signed payload does not begin with a CBOR map \
         (head byte {head:#04x})"
    );
    let carried = usize::from(head & 0x1f);
    assert!(
        carried < 24,
        "the bed failed before the product did: this reader only handles a count written in the \
         head byte, and this specimen's is {carried}"
    );

    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../gx-witness/src/receipt.rs"),
    )
    .expect("the payload's own module is here");
    let at = source
        .find("pub struct ReceiptPayload {")
        .expect("`ReceiptPayload` is declared in receipt.rs");
    let declaration = &source[at..at + source[at..].find("\n}").expect("it closes")];
    let named = declaration
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("pub ") && t.contains(": ") && t.ends_with(',')
        })
        .count();

    println!(
        "R39_FROZEN_CLI signed_bytes={} carried_members={carried} payload_members={named} \
         member_gap={} refusal={refusal:?}",
        signed.len(),
        named.saturating_sub(carried)
    );
    assert!(
        named > carried,
        "🔴 `req/540` AC-9: the specimen carries {carried} members and this build's payload names \
         {named}. If the payload ever names no more than the specimen carries, the limit \
         `docs/LIMITS.md` declares has closed and the page has to say so"
    );
}

/// 🔴 **`req/540` AC-9's other half** — and the number that cannot be re-derived is not on the page.
///
/// `req/540` R-2d: "measured, or gone" — what is not allowed is a number in public prose that no
/// instrument produces. This is the gone half, asserted where the page can be read rather than left
/// to a reviewer's eye.
#[test]
fn ac9_limits_carries_no_byte_count_for_a_re_encoding_nothing_performs() {
    let limits = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate sits at <root>/crates/gx-cli")
            .join("docs/LIMITS.md"),
    )
    .expect("the page is here");
    let offending: Vec<&str> = limits
        .lines()
        .filter(|l| l.contains("25 bytes longer"))
        .collect();
    println!("R39_FROZEN_CLI limits_25_bytes_lines={}", offending.len());
    assert!(
        offending.is_empty(),
        "🔴 `req/540` AC-9: `docs/LIMITS.md` carries a byte count for a re-encoding no instrument \
         in this tree performs. It was measured on a build R38 withdrew. Either restore a probe \
         that re-derives it or take the number off the page: {offending:?}"
    );
}
