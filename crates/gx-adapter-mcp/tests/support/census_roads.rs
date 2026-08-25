// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R27 item 3 (`req/331` §0-3, from `req/329` M-03, `req/38` §233 ruling 4)** — the roads to
//! the question *does this file say this tool writes*, **derived from `catalogue.rs` on every run**.
//!
//! # Why this module exists
//!
//! Three releases in a row repaired the same defect: a census that counts the spellings a lane
//! thought of, and a third gate written in a spelling it did not. R22 counted two strings and a
//! third spelling walked past. R25 counted the private field and argued privacy closed the crate;
//! the four `pub` accessors were roads it did not count. R26 widened the count to five roads and
//! wrote, on `docs/LIMITS.md`, that it *"counts every road to the question"*. The twenty-sixth
//! audit derived the roads from the source instead of from that sentence and found **seven**:
//! `declared`, `entry_fault` and `soundness` were uncounted, and `entry_fault` is the accessor
//! `catalogue.rs`'s own doc calls **"the question"**.
//!
//! Each repair was a longer list. A longer list has the same failure mode as a shorter one, so this
//! release stops writing the list. The roads are computed from the file itself every time the suite
//! runs: when an eighth accessor is added it is a road that day, with nobody remembering to add it.
//!
//! # What counts as a road, and what deliberately does not
//!
//! A road is the **private field** plus every `pub fn` that takes `&self` and whose body touches
//! `self.restores`. Readers only. A **builder** — `with_restore`, `with_prior_read`,
//! `with_restore_template` — also touches the map, and a gate cannot be written on one: it
//! constructs a catalogue rather than answering a question about a tool. Counting builders would
//! inflate the census with cells that are not roads, which is the shape of overclaim the audit that
//! found this defect refused in its own instrument (`req/329` §9-3).

use std::path::Path;

/// `catalogue.rs`, read from the crate this module is compiled into.
#[must_use]
pub fn catalogue_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/catalogue.rs");
    std::fs::read_to_string(path).expect("this crate's catalogue.rs is readable")
}

/// Source with `//` lines dropped, so a road named in prose is not counted as a road.
#[must_use]
pub fn code_of(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The **whole signature** of the item whose header starts at `start`, and the index of the line
/// that opens its body.
///
/// 🔴 **R28 / `req/334` M-02** — the reason this exists.
///
/// R27's predicate tested `&self` against the header's **one physical line**. rustfmt puts one
/// argument per line the moment the list stops fitting, and at that moment the header becomes
/// `pub fn name(` with `&self` on the line below — so the road stops being a road. This is not a
/// hypothetical spelling: `catalogue.rs` already writes **nine** signatures in exactly that shape
/// (`resolve`, `resolve_from_observation`, `resolve_split`, `new`, `arguments_from_forward`,
/// `resource_from`, `with_restore_template`, …), which is to say it is the spelling this file uses
/// every time an argument list grows. A census whose domain is *single-line headers* has the same
/// failure mode as the hand-written list it replaced: it counts the spellings someone thought of.
///
/// The signature is everything from the header to the line that opens the body, which is the first
/// line carrying `{`. A Rust type may not contain a bare `{`, so that scan is not ambiguous within
/// a signature; the walk is capped so that a malformed file cannot make this loop the whole file.
fn signature_at(lines: &[&str], start: usize) -> (String, usize) {
    let mut signature = String::new();
    for (offset, line) in lines.iter().enumerate().skip(start).take(24) {
        signature.push_str(line);
        signature.push('\n');
        if line.contains('{') {
            return (signature, offset);
        }
    }
    (signature, start)
}

/// Every road in `catalogue.rs` that takes `&self` and whose body reaches `self.restores`.
///
/// 🔴 **R28 / `req/334` M-02** — two spellings of the same road that R27's derivation could not
/// see, and now can.
///
/// * **A wrapped signature.** See [`signature_at`]: `&self` is tested against the whole signature
///   rather than against the header's first physical line.
/// * **A method behind a trait.** A method in an `impl <Trait> for <Type>` block is written
///   `fn name(&self)` with **no `pub`** — the visibility is the trait's — and R27 required the line
///   to start with `pub`. R25's lesson was "what is private is the field, not the question"; a
///   trait implemented on the catalogue is that same sentence one indirection along, and
///   `catalogue.rs` already contains trait impls.
///
/// The narrowing that R27 got right is kept exactly: a **builder** is not a road. Builders take
/// `mut self` (`with_restore`, `with_prior_read`, `with_restore_template` all do), so requiring
/// `&self` still excludes them — including `with_restore_template`, whose signature wraps and which
/// this widening therefore had to be checked against rather than assumed past.
///
/// 🔴 **Declared over-reach**: a method behind a *private* trait is counted, because this walk does
/// not resolve the trait's visibility. A census that must not undercount is allowed to err in the
/// direction of counting one road too many; erring the other way is the defect three releases in a
/// row have been a repair of. `d_the_domain_the_derivation_does_hold` holds the other edge.
///
/// Returned in source order, without the trailing `(` — see [`roads_to_the_question`] for the form
/// a census matches against.
#[must_use]
pub fn reader_accessors(src: &str) -> Vec<String> {
    let code = code_of(src);
    let lines: Vec<&str> = code.lines().collect();
    let mut found: Vec<String> = Vec::new();
    // The indentation of the `impl <Trait> for <Type>` block being walked, if any.
    let mut trait_impl: Option<usize> = None;
    for i in 0..lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some(open) = trait_impl {
            if trimmed.starts_with('}') && indent <= open {
                trait_impl = None;
            }
        }
        if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
            // ` for ` is what tells a trait impl from an inherent one, and it is read off the whole
            // header for the same reason the signature is: `impl<A, B>` headers wrap too.
            if signature_at(&lines, i).0.contains(" for ") {
                trait_impl = Some(indent);
            }
            continue;
        }
        let inherent_pub = trimmed.starts_with("pub fn ") || trimmed.starts_with("pub const fn ");
        let via_trait = trait_impl.is_some()
            && (trimmed.starts_with("fn ") || trimmed.starts_with("const fn "));
        if !inherent_pub && !via_trait {
            continue;
        }
        let (signature, opens_at) = signature_at(&lines, i);
        // Readers only: a builder takes `self` or `&mut self` and answers no question about a tool.
        if !signature.contains("&self") {
            continue;
        }
        let Some(name) = trimmed
            .split("fn ")
            .nth(1)
            .and_then(|rest| rest.split(['(', '<']).next())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        // The body: from the line after the one that opens it, to the next line at the header's
        // indentation starting `}`.
        let mut body = String::new();
        for candidate in lines.iter().skip(opens_at + 1) {
            let c_trim = candidate.trim_start();
            let c_indent = candidate.len() - c_trim.len();
            if c_trim.starts_with('}') && c_indent <= indent {
                break;
            }
            body.push_str(candidate);
            body.push('\n');
        }
        if body.contains("self.restores") && !found.contains(&name) {
            found.push(name);
        }
    }
    found
}

/// The strings a census matches source text against: the private field, then one per reader
/// accessor.
///
/// The field first and by its bare name, because a body inside `catalogue.rs` reaches the map
/// directly; the accessors with their opening parenthesis, because that is how a call is spelled
/// and a bare name would also match the definition.
#[must_use]
pub fn roads_to_the_question(src: &str) -> Vec<String> {
    let mut roads = vec!["restores".to_string()];
    for name in reader_accessors(src) {
        roads.push(format!("{name}("));
    }
    roads
}

/// How many roads to the question a piece of source text takes.
///
/// The census itself, as a pure function of two texts, so it can be fired at text that is not the
/// shipped file — which is what makes the mutation arms mutations rather than descriptions.
#[must_use]
pub fn reaches_the_question(code: &str, roads: &[String]) -> usize {
    roads
        .iter()
        .map(|road| code.matches(road.as_str()).count())
        .sum()
}
