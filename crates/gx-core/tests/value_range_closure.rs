// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **E-M3-18** (H-1) — every door that hands out a `Transformation` closes the value range.
//!
//! `req/38_ERRATA_2026-08-07.md` §25, verbatim (quoted in SEM-gx-core-199): "🔴 **H-1 (adopted =
//! erratum E-M3-18; implementation is a mandatory DoD at the start of M4)**: make `identity` return
//! a `Result` too and **close the value range at every constructor**. The ground for infallibility
//! (the error arm being unreachable) became false with D-6 -- as long as one unchecked door
//! remains, the invariant 'a gx-core Transformation is within range' cannot be stated in the
//! type".
//!
//! # Why two instruments and not one
//!
//! "close the value range" (sem: SEM-gx-core-200) is two claims, and one test cannot hold both.
//!
//! * **The shape**: no public function of this crate hands back a bare `Transformation`. That is a
//!   fact about the source, so it is read out of the source -- a `match` or a call list would be
//!   updated in the same edit that opened a fourth door and would therefore never notice one. This
//!   is the A-1 shape (`req/61` §2): a behavioural half that a mutation can walk past, beside a
//!   source-order half that it cannot.
//! * **The behaviour**: each of those doors actually refuses **E-M3-13**'s ① and ②. A door that
//!   returns `Result` and never returns `Err` is the same lie one level in, which is exactly what
//!   this hand found `identity` doing under the old signature: `compose_range.rs`'s
//!   `identity_is_a_third_door_and_this_hand_did_not_close_it` pinned that behaviour on purpose so
//!   that changing it would be a deliberate act. This hand is that act, and the pin is rewritten
//!   there rather than deleted.
//!
//! `UNCHECKED_DOORS` is printed by the first and `DOORS_REFUSING_OUT_OF_RANGE` by the second, so a
//! reader of the log sees both numbers rather than a single word.
//!
//! # What the source scan cannot see
//!
//! It reads the return type as the text between the first `->` of a signature and the `where` or
//! the body brace. A parameter written as `impl Fn(..) -> ..` would be read as a return type; this
//! crate writes those bounds in `where` clauses, so no signature here has an arrow before its own.
//! If one is added, this note is the place that says why the scan starts answering wrongly.

mod conformance;

use conformance::{cid, metadata, snapshot, World};
use gx_core::{
    compose, identity, CompositionMetadata, Error, IntentId, Subject, Timestamp, Transformation,
    TransformationId,
};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<repo>/crates/gx-core`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

/// Every `.rs` file of this crate's `src/`, read once.
fn sources() -> Vec<(String, String)> {
    let dir = repo_root().join("crates/gx-core/src");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("gx-core/src is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|x| x == "rs") {
            let name = path
                .file_name()
                .expect("named")
                .to_string_lossy()
                .into_owned();
            out.push((name, std::fs::read_to_string(&path).expect("readable")));
        }
    }
    out.sort();
    assert!(
        out.len() >= 14,
        "gx-core declared fourteen modules at M3 and lib.rs makes fifteen files; found {}",
        out.len()
    );
    out
}

/// A public function of this crate, and the type it hands back.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Door {
    file: String,
    name: String,
    returns: String,
}

/// The first identifier of `text`.
fn ident(text: &str) -> String {
    text.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// What `Self` means inside `impl <this> {`.
///
/// `impl serde::Serialize for Cid {` names `Cid`; `impl Transformation {` names itself. The token
/// after the last ` for ` is the subject when there is one, and the first token otherwise.
fn impl_subject(rest: &str) -> String {
    let tail = match rest.rsplit_once(" for ") {
        Some((_, after)) => after,
        None => rest,
    };
    ident(tail.trim_start_matches(['<', '\'']).trim())
}

/// The return type of a signature that has been joined onto one line, or `None` while the
/// signature is still open.
fn return_type(signature: &str) -> Option<String> {
    let at = signature.find("->")?;
    let after = &signature[at + 2..];
    let end = after
        .find(" where")
        .into_iter()
        .chain(after.find('{'))
        .min()
        .unwrap_or(after.len());
    Some(after[..end].trim().to_string())
}

/// Every `pub fn` in this crate's `src/`, with `Self` resolved to the type it stands for.
fn public_functions() -> Vec<Door> {
    let mut out = Vec::new();
    for (file, source) in sources() {
        let mut subject = String::new();
        let mut open: Option<(String, String)> = None;
        for raw in source.lines() {
            let line = raw.trim();
            // Documentation carries whole functions inside `compile_fail` blocks (F-2's is in
            // `transformation.rs`), so comment lines are not source for this purpose.
            if line.starts_with("//") {
                continue;
            }
            if let Some(rest) = line.strip_prefix("impl ") {
                subject = impl_subject(rest);
            }
            match open.as_mut() {
                Some((_, signature)) => {
                    signature.push(' ');
                    signature.push_str(line);
                }
                None => {
                    if let Some(rest) = line.strip_prefix("pub fn ") {
                        open = Some((ident(rest), rest.to_string()));
                    }
                }
            }
            let Some((name, signature)) = open.clone() else {
                continue;
            };
            if let Some(returns) = return_type(&signature) {
                out.push(Door {
                    file: file.clone(),
                    name,
                    returns: returns.replace("Self", &subject),
                });
                open = None;
            } else if signature.contains('{') {
                // A signature with no arrow at all returns `()`; nothing here is a door.
                open = None;
            }
        }
    }
    out
}

/// Does this type expression name `token` as a type of its own, rather than as a prefix?
///
/// `Vec<TransformationId>` does not name `Transformation`, and reading it as though it did would
/// make [`gx_core::ancestors`] a door.
fn names(type_expression: &str, token: &str) -> bool {
    type_expression
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|t| t == token)
}

// ---------------------------------------------------------------------------
// The shape: no bare `Transformation` leaves this crate
// ---------------------------------------------------------------------------

/// **E-M3-18**, as a fact about the source.
///
/// A door is a public function whose return type names `Transformation`. An unchecked door is one
/// that does so without a `Result` around it: a caller of such a function cannot be handed a
/// refusal, so no predicate the crate holds can be enforced there. The count is printed whether it
/// is zero or not, because "the value range is closed" (sem: SEM-gx-core-201) is a number a reader
/// should see rather than infer
/// from a green run.
#[test]
fn every_door_that_hands_out_a_transformation_returns_a_result() {
    let doors: Vec<Door> = public_functions()
        .into_iter()
        .filter(|d| names(&d.returns, "Transformation"))
        .collect();
    let unchecked: Vec<&Door> = doors
        .iter()
        .filter(|d| !names(&d.returns, "Result"))
        .collect();

    println!(
        "TRANSFORMATION_DOORS={} UNCHECKED_DOORS={} ({:?})",
        doors.len(),
        unchecked.len(),
        doors
            .iter()
            .map(|d| format!("{}::{} -> {}", d.file, d.name, d.returns))
            .collect::<Vec<_>>()
    );

    assert!(
        unchecked.is_empty(),
        "these public functions hand out a `Transformation` with no way to refuse one: \
         {unchecked:?}. E-M3-18: 'as long as one unchecked door remains, the invariant \"a gx-core \
         Transformation is within range\" cannot be stated in the type' (sem: SEM-gx-core-202)"
    );

    let mut named: Vec<String> = doors.iter().map(|d| d.name.clone()).collect();
    named.sort();
    assert_eq!(
        named,
        vec!["compose", "identity", "new"],
        "the three doors E-M3-13 and E-M3-18 are about; a fourth is a decision somebody writes down"
    );
}

// ---------------------------------------------------------------------------
// The behaviour: each door refuses E-M3-13's (1) and (2)
// ---------------------------------------------------------------------------

/// A door, as a callable: metadata in, refusal or arrow out.
///
/// The three signatures differ, so the fixtures they need differ; what is shared is the metadata,
/// which is the argument E-M3-13's two predicates are about. Written as a list so that the count
/// below is the number of doors exercised rather than the number of assertions typed.
fn call_every_door(
    meta: CompositionMetadata,
) -> Vec<(&'static str, Result<Transformation, Error>)> {
    let world = World::new(1, 2, 3);
    let x = snapshot(1, 4);
    let y = snapshot(2, 4);

    vec![
        (
            "new",
            Transformation::new(
                TransformationId(cid(5)),
                0,
                Subject::Object(*x.id()),
                Some(*y.digest()),
                Vec::new(),
                meta.clone(),
            ),
        ),
        (
            "compose",
            compose(&world.f, &world.g, world.resolve(), meta.clone(), |_| {
                TransformationId(cid(99))
            }),
        ),
        ("identity", identity(&x, meta, |_| TransformationId(cid(5)))),
    ]
}

/// "① `created_at ≥ 0`" at all three doors, and "② `intent_id ≠ the all-zero placeholder`" at all
/// three (sem: SEM-gx-core-203).
///
/// The count is what makes this more than three assertions: `DOORS_REFUSING_OUT_OF_RANGE=3/3` and
/// `UNCHECKED_DOORS=0` are the two halves of "close the value range at every constructor" (sem:
/// SEM-gx-core-204), and a reader of the log has
/// both numbers without opening this file.
#[test]
fn every_door_refuses_the_two_predicates_e_m3_13_names() {
    let mut refusing = 0usize;
    let mut doors = 0usize;

    let mut below_epoch = metadata(1);
    below_epoch.created_at = Timestamp(-1);
    for (name, built) in call_every_door(below_epoch) {
        doors += 1;
        match built {
            Err(Error::CreatedAtNegative { got: -1 }) => refusing += 1,
            other => panic!("`{name}` admitted a created_at below the epoch: {other:?}"),
        }
    }

    let mut unset_intent = metadata(1);
    unset_intent.intent_id = IntentId(gx_core::Cid([0u8; 32]));
    for (name, built) in call_every_door(unset_intent) {
        match built {
            Err(Error::IntentIdUnset) => {}
            other => panic!("`{name}` admitted the all-zero intent id: {other:?}"),
        }
    }

    println!("DOORS_REFUSING_OUT_OF_RANGE={refusing}/{doors}");
    assert_eq!(refusing, 3, "three doors, all refusing");
}

/// The control the refusals above need: metadata inside the range still builds at every door.
///
/// req/29 §2 in one line -- a suite whose refusals are all it asserts cannot tell a working check
/// from a check that refuses everything. `compose_range.rs` states the same rule for its own
/// predicates; this is that rule asked of all three doors at once.
#[test]
fn every_door_still_builds_an_arrow_inside_the_range() {
    for (name, built) in call_every_door(metadata(1)) {
        built.unwrap_or_else(|e| panic!("`{name}` refused metadata that is inside the range: {e}"));
    }
}

/// The refusal a door gives is the same refusal the others give, named the same way.
///
/// H-3's table is what makes "same way" (sem: SEM-gx-core-205) checkable: [`gx_core::Error::kind`]
/// is the word, and two
/// doors that refused the same value under two names would be two vocabularies. This is the join
/// between E-M3-18 and E-M2-23, and it is one line because both were done in one hand.
#[test]
fn the_three_doors_refuse_under_one_name() {
    let mut below_epoch = metadata(1);
    below_epoch.created_at = Timestamp(-1);
    let kinds: Vec<&'static str> = call_every_door(below_epoch)
        .into_iter()
        .map(|(name, built)| {
            built
                .expect_err(&format!("`{name}` is out of range here"))
                .kind()
        })
        .collect();
    assert_eq!(kinds, ["CreatedAtNegative"; 3]);
    for kind in kinds {
        assert!(gx_core::ERROR_KINDS.contains(&kind));
    }
}
