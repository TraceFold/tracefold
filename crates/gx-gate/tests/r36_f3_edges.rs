// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/476` M-01, red-first** (`req/38` §271 ruling 5 item 2; reqdef `req/479` §0-2).
//!
//! # What audit 35 measured
//!
//! `declares_its_default` counts permits by the **spelling at the start of a line**:
//!
//! ```text
//! let permits = statements.iter().filter(|line| line.starts_with("permit")).count();
//! ```
//!
//! So a pack that writes its `@id` annotation and its `permit` on **one line** ships a permit and
//! is read as having none. Cedar parses that text (it is ordinary Cedar), the pack declares no
//! permit default, and F3 — the clause whose entire purpose is that a pack must pick its default
//! out loud — accepts it. Audit 35's seven-shape table (`C:/work/a35_logs/f3_edges2.log`, verbatim
//! for the row that carries the finding):
//!
//! ```text
//! A35F3 shape=permit_on_the_same_line_as_its_id_annotation f3_accepted=true cedar_parses=true
//!       ids=["fs-quiet"] detail=None
//! ```
//!
//! Three more rows in that table are shape 2's **unchecked conjuncts**. `PACK_FORMAT` F3 shape 2 is
//! a conjunction of three — no permit statement, *declared as such in the pack's header*, and at
//! least one `deny_by_no_policy` conformance case — and the shipped check tests only the first.
//! A pack with an empty source and no ids was accepted as "has declared its default".
//!
//! # Why every shape is also handed to Cedar
//!
//! Audit 35 §7-9 is the reason and it is worth carrying: the audit's first four shapes did not
//! parse as Cedar at all, and reporting those as holes in F3 would have been four fabricated
//! findings. A shape only matters if the parser this product uses accepts it, so both answers are
//! measured on every row and printed side by side.
//!
//! # Red-first (`req/38` §226)
//!
//! Every shape is built from `ShippedPack`'s own public fields and handed to the shipped
//! `declares_its_default` and to the shipped `PolicyEngine::parse`. No symbol this lane creates is
//! named here, so this suite compiles at the commit before the repair and fails on its assertions.

use gx_gate::packs::{declares_its_default, ShippedPack};
use gx_gate::policy::PolicyEngine;

/// One row of audit 35's table: what F3 answered, and what Cedar answered, about the same text.
struct Shape {
    name: &'static str,
    pack: ShippedPack,
    /// What F3 must answer once shape 2's conjuncts are checked.
    must_be_accepted: bool,
    /// Why, in the words of the clause.
    because: &'static str,
}

fn shapes() -> Vec<Shape> {
    vec![
        // ---- The two negative controls: the mechanism can refuse. -------------------------
        Shape {
            name: "permit_with_no_permit_default_id",
            pack: ShippedPack {
                path: "policies/r36/edge.cedar",
                source: "@id(\"fs-something-else\")\npermit ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
                policy_ids: &["fs-something-else"],
                substrate: "fs",
            },
            must_be_accepted: false,
            because: "shape 1 with no `-permit-default` id: the control that says the gate can refuse",
        },
        Shape {
            name: "no_permit_but_id_names_permit_default",
            pack: ShippedPack {
                path: "policies/r36/edge.cedar",
                source: "@id(\"fs-permit-default\")\nforbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
                policy_ids: &["fs-permit-default"],
                substrate: "fs",
            },
            must_be_accepted: false,
            because: "ids and statements disagree: the second control, and it also refuses today",
        },
        // ---- M-01 proper: a permit hidden behind its own annotation. -----------------------
        Shape {
            name: "permit_on_the_same_line_as_its_id_annotation",
            pack: ShippedPack {
                path: "policies/r36/edge.cedar",
                source: "@id(\"fs-quiet\") permit ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
                policy_ids: &["fs-quiet"],
                substrate: "fs",
            },
            must_be_accepted: false,
            because: "req/476 M-01: this pack ships a permit and declares no permit default. F3 \
                      exists for exactly this state and a newline is not what decides it",
        },
        // ---- Shape 2's second and third conjuncts, unchecked. ------------------------------
        Shape {
            name: "no_permit_id_names_deny_default_nothing_declared",
            pack: ShippedPack {
                path: "policies/r36/edge.cedar",
                source: "@id(\"fs-deny-default\")\nforbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
                policy_ids: &["fs-deny-default"],
                substrate: "fs",
            },
            must_be_accepted: false,
            because: "shape 2 requires the header to declare the deny default; an id is not a header",
        },
        Shape {
            name: "no_permit_nothing_declared_at_all",
            pack: ShippedPack {
                path: "policies/r36/edge.cedar",
                source: "@id(\"fs-forbid-something\")\nforbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
                policy_ids: &["fs-forbid-something"],
                substrate: "fs",
            },
            must_be_accepted: false,
            because: "the pack says nothing about its default, which is the state F3 forbids",
        },
        Shape {
            name: "empty_source_empty_ids",
            pack: ShippedPack {
                path: "policies/r36/edge.cedar",
                source: "",
                policy_ids: &[],
                substrate: "fs",
            },
            must_be_accepted: false,
            because: "a file with nothing in it was answered `has declared its default`",
        },
        Shape {
            name: "header_claims_permit_default_over_a_pack_with_no_permit",
            pack: ShippedPack {
                path: "policies/r36/edge.cedar",
                source: "// This pack declares a **permit-default**.\n@id(\"fs-forbid-something\")\nforbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
                policy_ids: &["fs-forbid-something"],
                substrate: "fs",
            },
            must_be_accepted: false,
            because: "the header and the statements say different things and nothing compared them",
        },
        // ---- The shape that must keep passing: the deny-default pack as shipped. -----------
        Shape {
            name: "no_permit_and_the_header_declares_the_deny_default",
            pack: ShippedPack {
                path: "policies/r36/edge.cedar",
                source: "// # This pack declares a **deny-default**. It has no permit statement, and that is the ruling.\n@id(\"fs-forbid-something\")\nforbid ( principal, action, resource )\n  when { resource.substrate == \"fs\" };\n",
                policy_ids: &["fs-forbid-something"],
                substrate: "fs",
            },
            must_be_accepted: true,
            because: "shape 2, taken out loud: this is the form `policies/postgres/` already ships",
        },
    ]
}

/// 🔴 `req/476` M-01 and shape 2's unchecked conjuncts, on one table.
#[test]
fn r36_the_deny_default_branch_of_f3_is_checked() {
    println!("R36F3_BEGIN");
    let mut wrong: Vec<&'static str> = Vec::new();
    let mut not_cedar: Vec<&'static str> = Vec::new();
    for shape in shapes() {
        let answered = declares_its_default(&shape.pack);
        let accepted = answered.is_ok();
        let cedar_parses = PolicyEngine::parse(shape.pack.source).is_ok();
        println!(
            "R36F3 shape={} f3_accepted={accepted} cedar_parses={cedar_parses} ids={:?} detail={:?}",
            shape.name,
            shape.pack.policy_ids,
            answered.err().map(|e| e.to_string())
        );
        // A shape Cedar itself rejects is not a hole in F3, and reporting one as such is how an
        // audit fabricates findings (audit 35 §7-9, where the first four shapes carried no `@id`
        // and did not parse). Every row here is ordinary Cedar, and this arm is what says so.
        if !cedar_parses {
            not_cedar.push(shape.name);
        }
        if accepted != shape.must_be_accepted {
            println!("R36F3_WRONG shape={} because={}", shape.name, shape.because);
            wrong.push(shape.name);
        }
    }
    println!("R36F3_SUMMARY wrong={wrong:?} not_cedar={not_cedar:?}");
    assert!(
        not_cedar.is_empty(),
        "instrument: {not_cedar:?} are not Cedar, so what F3 answers about them proves nothing \
         (audit 35 §7-9 nearly reported four of these as findings)"
    );
    assert!(
        wrong.is_empty(),
        "req/476 M-01: {wrong:?} — F3 answered the opposite of PACK_FORMAT F3. Shape 2 is a \
         conjunction of three clauses and the shipped check reads the first one off the start of a \
         line, so `@id(..) permit` on one line ships a permit that the count does not see"
    );
}

/// 🔴 The four packs that actually ship must keep passing, and the reason is not tautological: a
/// repair that tightened F3 into refusing `policies/postgres/` would take the CLI down, because
/// [`gx_gate::packs::shipped_pack_set`] is what a deployment composes at start-up.
#[test]
fn r36_every_shipped_pack_still_declares_its_default() {
    let mut refused: Vec<(&'static str, String)> = Vec::new();
    for pack in &gx_gate::packs::SHIPPED_PACKS {
        if let Err(why) = declares_its_default(pack) {
            refused.push((pack.path, why.to_string()));
        }
    }
    println!("R36F3_SHIPPED refused={refused:?}");
    assert!(
        refused.is_empty(),
        "the repair refuses a pack this repository ships, which would stop every CLI that composes \
         the shipped set: {refused:?}"
    );
    assert!(
        gx_gate::packs::shipped_pack_set().is_ok(),
        "the composed set is what a deployment starts with"
    );
}
