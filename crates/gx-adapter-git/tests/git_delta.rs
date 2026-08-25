// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The grammar, the locator and the scope bound: the three things a payload has to survive.
//!
//! Spec: 42 §2.1 (canonical DAG-CBOR), 42 §3.4 (the payload is opaque above the adapter), 42 §2.3
//! (the `≈` this crate's root declares), and **req/98** §3-4's reserved item 6 for the last section -- "git/mcp's
//! locator route refuses `ScopeTooLong` **at construction time**". (sem: SEM-gx-adapter-git-121)

mod support;

use gx_adapter_git::{
    normalize, GitDelta, GitOp, MAX_FORWARD_PAYLOAD_BYTES, MAX_OPS, MAX_PATH_DEPTH,
};
use gx_core::{Fingerprint, SubstrateKind, MAX_SCOPE_BYTES};
use gx_substrate::{elide_scope, SubstrateAdapter};
use support::{intent_for, GitFixture, BRANCH, GOAL};

// ---------------------------------------------------------------------------
// The grammar
// ---------------------------------------------------------------------------

/// **M4-07, adopted (c)**: concatenation of sequences is associative, which is what makes the monoid free. (sem: SEM-gx-adapter-git-122)
///
/// On the *sequences* and not on the bytes: **N-14** requires canonical DAG-CBOR and a CBOR array
/// carries its length in its head, so two payloads do not concatenate as bytes. `decode`, concatenate,
/// `encode`.
#[test]
fn concatenation_of_sequences_is_associative() {
    let a = GitOp::write("/r#refs/heads/a:x".to_string(), b"a".to_vec());
    let b = GitOp::remove("/r#refs/heads/b:x".to_string());
    let c = GitOp::reset("/r#refs/heads/c:x".to_string(), "0".repeat(40));

    let left = GitDelta::of(vec![a.clone(), b.clone()]);
    let right = GitDelta::of(vec![b.clone(), c.clone()]);
    let ab_c = GitDelta::of(
        left.ops()
            .iter()
            .cloned()
            .chain(std::iter::once(c.clone()))
            .collect(),
    );
    let a_bc = GitDelta::of(
        std::iter::once(a)
            .chain(right.ops().iter().cloned())
            .collect(),
    );
    assert_eq!(ab_c, a_bc);
    assert_eq!(
        ab_c.encode().expect("encodable"),
        a_bc.encode().expect("encodable"),
        "one value, one canonical form (42 §2.1)"
    );
    // The unit.
    let unit = GitDelta::of(Vec::new());
    let one = GitDelta::one(b);
    assert_eq!(
        GitDelta::of(
            unit.ops()
                .iter()
                .cloned()
                .chain(one.ops().iter().cloned())
                .collect()
        ),
        one
    );
}

/// The three refusals of [`GitDelta::decode`], each with its own word.
///
/// "unimplemented" (`Unimplemented`, which the shared harness reads as "none") and "this is not a payload I
/// wrote" (`PayloadUnreadable`) are different facts, and req/29 §4 forbids one word for both. (sem: SEM-gx-adapter-git-123)
#[test]
fn decode_refuses_three_different_things_in_three_different_words() {
    let op = GitOp::write("/r#refs/heads/a:x".to_string(), b"a".to_vec());

    let too_long = GitDelta::of(vec![op.clone(), op.clone()])
        .encode()
        .expect("encodable");
    let err = GitDelta::decode(&too_long).expect_err("v0.1 runs one operation");
    println!("DECODE_TOO_LONG kind={} MAX_OPS={MAX_OPS}", err.kind());
    assert_eq!(err.kind(), "Unimplemented");

    let empty = GitDelta::of(Vec::new()).encode().expect("encodable");
    let err = GitDelta::decode(&empty).expect_err("the unit is not a payload");
    println!("DECODE_EMPTY kind={}", err.kind());
    assert_eq!(err.kind(), "PayloadUnreadable");

    let err = GitDelta::decode(b"not cbor at all").expect_err("not this grammar");
    println!("DECODE_GARBAGE kind={}", err.kind());
    assert_eq!(err.kind(), "PayloadUnreadable");
}

/// A `reset` with content and a `content` with a target are payloads this grammar did not write.
///
/// The discriminant is a **field** rather than an inference precisely so that these two are
/// detectable: a grammar that read "no target" as "a content operation" would let a hand-made payload (sem: SEM-gx-adapter-git-124)
/// turn a branch move into a file change, or the reverse.
#[test]
fn an_operation_whose_kind_and_fields_disagree_is_unreadable() {
    let good = GitOp::reset("/r#refs/heads/a:x".to_string(), "0".repeat(40));
    assert!(good.well_formed().is_ok());

    // Built through the encoder, since the constructors cannot produce the malformed shapes.
    let bytes = GitDelta::one(good).encode().expect("encodable");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    println!("RESET_PAYLOAD_LEN={} printable={}", bytes.len(), text.len());

    // A `kind` this grammar does not write, reached the only way a caller could reach it: by decoding
    // bytes somebody else made. The payload is built by hand as a one-element array of the four keys.
    let forged = forged_op("elsewhere", None, "/r#refs/heads/a:x", None);
    let err = GitDelta::decode(&forged).expect_err("this grammar writes two kinds");
    println!("DECODE_FORGED_KIND kind={}", err.kind());
    assert_eq!(err.kind(), "PayloadUnreadable");

    let forged = forged_op(
        "reset",
        Some(b"body"),
        "/r#refs/heads/a:x",
        Some("0".repeat(40)),
    );
    let err = GitDelta::decode(&forged).expect_err("a reset carries no content");
    println!("DECODE_RESET_WITH_CONTENT kind={}", err.kind());
    assert_eq!(err.kind(), "PayloadUnreadable");

    let forged = forged_op(
        "content",
        Some(b"body"),
        "/r#refs/heads/a:x",
        Some("0".repeat(40)),
    );
    let err = GitDelta::decode(&forged).expect_err("a content operation carries no target");
    println!("DECODE_CONTENT_WITH_TARGET kind={}", err.kind());
    assert_eq!(err.kind(), "PayloadUnreadable");
}

/// One operation map, written by hand in the shape the grammar's fields have.
///
/// The keys are sorted, which is 42 §2.1-2's rule and is what serde's derive produces for this struct;
/// a hand-made payload that was not canonical would be refused for the wrong reason.
fn forged_op(kind: &str, content: Option<&[u8]>, locator: &str, target: Option<String>) -> Vec<u8> {
    #[derive(serde::Serialize)]
    struct Op<'a> {
        content: Option<&'a [u8]>,
        kind: &'a str,
        locator: &'a str,
        target: Option<String>,
    }
    gx_canon::cbor::encode(&vec![Op {
        content,
        kind,
        locator,
        target,
    }])
    .expect("a map of four keys has a canonical form")
}

/// **M4H5-4, adopted (b)**: the forward bound is declared **once**, and the declaration is the one that runs. (sem: SEM-gx-adapter-git-125)
#[test]
fn the_forward_ceiling_is_declared_in_one_place_and_is_enforced_in_plan() {
    let source = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/delta.rs"),
    )
    .expect("readable");
    let declarations = source
        .lines()
        .filter(|l| {
            l.trim_start()
                .starts_with("pub const MAX_FORWARD_PAYLOAD_BYTES")
        })
        .count();
    println!("FORWARD_CEILING_DECLARATIONS={declarations} VALUE={MAX_FORWARD_PAYLOAD_BYTES}");
    assert_eq!(
        declarations, 1,
        "one declaration, as §33 M4H5-4 (b) requires"
    );

    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = fixture.sandbox().locator_on(BRANCH);
    let pre = adapter.snapshot(&locator).expect("the entry is there");
    let oversized = vec![b'H'; MAX_FORWARD_PAYLOAD_BYTES + 1];
    let err = adapter
        .plan(&intent_for(&locator, &oversized), &pre)
        .expect_err("a change larger than the ceiling is not plannable");
    println!("FORWARD_CEILING_REFUSAL kind={}", err.kind());
    assert_eq!(err.kind(), "NotPlannable");

    // And a change under it plans, so the refusal above is the ceiling rather than the fixture.
    adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("a small change plans");
}

/// 🔴 **There is no escrow ceiling in this crate, and the absence is asserted rather than assumed.**
///
/// The crate root argues it: an fs inverse carries the whole old file (42 §5, M4-21) and a git inverse
/// carries an object id, so no input could reach a bound. A constant declared here would be a refusal
/// nobody asked for (52 contract 2), and -- worse -- one a reader would believe was reachable. This probe is (sem: SEM-gx-adapter-git-126)
/// what makes the day somebody adds one a red test rather than a silent widening of the vocabulary.
#[test]
fn this_adapter_declares_no_escrow_ceiling() {
    let source = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/delta.rs"),
    )
    .expect("readable");
    let declared = source
        .lines()
        .filter(|l| {
            l.trim_start()
                .starts_with("pub const MAX_INVERSE_PAYLOAD_BYTES")
        })
        .count();
    println!("ESCROW_CEILING_DECLARATIONS={declared}  (fs declares 1; git's inverse is a pointer)");
    assert_eq!(
        declared, 0,
        "an escrow ceiling here would be unreachable by construction: the inverse this adapter \
         builds is a `reset` naming a commit that is already in the object database"
    );
}

// ---------------------------------------------------------------------------
// The locator
// ---------------------------------------------------------------------------

/// L7's first half over every clause, including spellings this adapter would refuse to act on.
#[test]
fn normalisation_is_idempotent_and_folds_the_equivalent_spellings() {
    for spelling in [
        "/srv/repo#refs/heads/main:README.md",
        "/srv/./repo#main:README.md",
        "/srv/x/../repo#heads/main:/README.md",
        "/srv//repo#refs//heads/main:README.md/",
        "/srv/repo#tags/v1:README.md",
        "/../srv/repo#main:./README.md",
        "no separators here",
        "",
    ] {
        let once = normalize(spelling);
        assert_eq!(once, normalize(&once), "not idempotent for {spelling:?}");
    }

    let canonical = "/srv/repo#refs/heads/main:README.md";
    for equivalent in [
        "/srv/./repo#refs/heads/main:README.md",
        "/srv/x/../repo#main:README.md",
        "/srv//repo#heads/main:/README.md",
        "/srv/repo/#main:./README.md",
    ] {
        assert_eq!(
            normalize(equivalent),
            canonical,
            "{equivalent:?} names the same position"
        );
    }
    println!("LOCATOR_CLAUSES=4 CANONICAL={canonical}");
}

/// Clause 4: the bytes are the bytes. Two spellings that differ by case are two positions.
#[test]
fn the_normalisation_folds_no_case_and_no_unicode() {
    assert_ne!(
        normalize("/srv/repo#main:README.md"),
        normalize("/srv/repo#main:readme.md")
    );
    assert_ne!(
        normalize("/srv/repo#Main:README.md"),
        normalize("/srv/repo#main:README.md")
    );
}

/// What is not a position, and the word for it (**M4H5-5, adopted (b)**). (sem: SEM-gx-adapter-git-127)
#[test]
fn a_spelling_that_is_not_a_position_is_refused_as_one() {
    let adapter = gx_adapter_git::GitAdapter::new();
    for (spelling, why) in [
        ("/srv/repo", "no reference and no path"),
        ("/srv/repo#main", "no path"),
        ("relative/repo#main:x", "a relative repository"),
        ("/srv/repo#:x", "an empty reference"),
        ("/srv/repo#main:", "an empty path"),
        ("/srv/repo#main:a/b", "a nested path (MAX_PATH_DEPTH)"),
    ] {
        let err = adapter
            .snapshot(spelling)
            .expect_err("this is not a position");
        println!("NOT_A_POSITION {spelling:?} -> {} ({why})", err.kind());
        assert_eq!(err.kind(), "NotAPosition", "{spelling:?}: {why}");
    }
    println!("MAX_PATH_DEPTH={MAX_PATH_DEPTH}");
}

// ---------------------------------------------------------------------------
// The scope bound (req/98 §3-4's reserved item 6) (sem: SEM-gx-adapter-git-128)
// ---------------------------------------------------------------------------

/// 🔴 **Reserved item 6**: the git locator reaches `ScopeTooLong` through the **same one road** the fs adapter (sem: SEM-gx-adapter-git-129)
/// takes, and the refusal is at construction.
///
/// [`gx_core::Fingerprint::new`] is where the bound lives (**M4H1-2**: "gx-core also has a ceiling on the scope string
/// and turns it into a digest past it"), and [`elide_scope`] is the adapter-side fold that keeps a long scope's (sem: SEM-gx-adapter-git-130)
/// identity. Two halves are measured: a scope over the bound is refused **by the type** when it is not
/// elided, and the elision produces something the type accepts and that two reads agree on.
#[test]
fn an_over_long_scope_is_refused_at_construction_and_elides_to_one_line() {
    let long = format!("/{}#refs/heads/main", "d/".repeat(MAX_SCOPE_BYTES));
    assert!(long.len() > MAX_SCOPE_BYTES);

    let digest = gx_canon::cid::mint(gx_canon::cid::Domain::Leaf, &[b"x".as_slice()]);
    let refused = Fingerprint::new(SubstrateKind::Git, long.clone(), digest)
        .expect_err("gx-core refuses a scope past the bound at construction");
    println!(
        "SCOPE_REFUSED len={} MAX_SCOPE_BYTES={MAX_SCOPE_BYTES} kind={}",
        long.len(),
        refused.kind()
    );
    assert_eq!(refused.kind(), "ScopeTooLong");

    let elided = elide_scope(long.clone()).expect("a string always has a canonical form");
    assert!(elided.len() <= MAX_SCOPE_BYTES);
    assert_eq!(
        elided,
        elide_scope(long).expect("elision is a function of the text"),
        "two reads of one long scope have to elide to the same line, or a CAS check would refuse a \
         branch comparing with itself (E-M4-15)"
    );
    Fingerprint::new(SubstrateKind::Git, elided, digest)
        .expect("the elided line is under the bound");
}

/// 🔴 **A survivor's hole, closed** (M7 hand 1's battery point (f)).
///
/// The battery mutated [`gx_adapter_git::adapter`]'s `precondition` to scope the **entry** instead of
/// the branch and every one of the fifteen obligations stayed green. The reason is worth writing
/// down, because it is a gap in the shared harness rather than a weakness of this adapter: 51 §7 and
/// the L-list compare `precondition` with `precondition` (contract 3, L4), and `postcondition` with
/// `postcondition` (L2), and **never one with the other**. A `precondition` that named a different
/// scope than the `apply` beside it was therefore invisible — while being exactly the defect that
/// makes 42 §3.5's CAS unusable, since [`gx_core::Fingerprint::cas_eq`] refuses outright across two
/// scopes (**E-M4-15**) and an engine would read that refusal as "that comparison carries no meaning" about an (sem: SEM-gx-adapter-git-131)
/// object comparing with itself.
///
/// So the two are compared here, in both directions: the fingerprint `apply` observed is comparable
/// with a fresh `precondition` (one scope) and says the state did **not** move between them, and the
/// fingerprint taken **before** the apply is comparable and says it did. A mutation of either scope
/// makes the first assertion an `Err` rather than a `false`, which is the shape that distinguishes
/// "different state" from "different question". (sem: SEM-gx-adapter-git-132)
#[test]
fn the_postcondition_and_the_precondition_name_one_scope() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = fixture.sandbox().locator_on(BRANCH);

    let pre = adapter.snapshot(&locator).expect("the entry is there");
    let before = adapter.precondition(&pre).expect("a fingerprint");
    let delta = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans");
    let applied = adapter.apply(&delta).expect("the entry change applies");

    let after_snapshot = adapter
        .snapshot(&locator)
        .expect("the entry is still there");
    let after = adapter
        .precondition(&after_snapshot)
        .expect("a fingerprint");

    println!(
        "SCOPE_AGREEMENT postcondition={:?} precondition={:?}",
        applied.postcondition().scope(),
        after.scope()
    );
    assert_eq!(
        applied.postcondition().scope(),
        after.scope(),
        "the scope `apply` observed and the scope `precondition` names are one string, or the CAS \
         check of 41 §5-5b compares two questions"
    );
    assert_eq!(
        applied.postcondition().cas_eq(&after),
        Ok(true),
        "nothing moved between the apply and the read, so the two fingerprints are comparable and \
         equal — an `Err` here is two scopes and not two states (E-M4-15)"
    );
    assert_eq!(
        before.cas_eq(applied.postcondition()),
        Ok(false),
        "the branch moved, and the answer to 'did it move' is `Ok(false)` rather than a refusal (sem: SEM-gx-adapter-git-133)"
    );
}

/// And the adapter itself takes that road: a real position under a very deep repository path gets a
/// fingerprint rather than a refusal.
#[test]
fn the_adapter_elides_before_it_constructs() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let locator = fixture.sandbox().locator_on(BRANCH);
    let snap = adapter.snapshot(&locator).expect("the entry is there");
    let fingerprint = adapter.precondition(&snap).expect("a fingerprint");
    println!(
        "PRECONDITION scope_len={} substrate={:?}",
        fingerprint.scope().len(),
        fingerprint.substrate()
    );
    assert_eq!(fingerprint.substrate(), &SubstrateKind::Git);
    assert!(fingerprint.scope().len() <= MAX_SCOPE_BYTES);
}
