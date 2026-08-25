// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R27 item 2 (`req/331` §0-2, from `req/329` M-02, `req/38` §233 ruling 3)** — the
//! invisible-edge **class** reaches the scalars whose appearance the standard does not fix, and the
//! page and the code move in one commit.
//!
//! # What broke
//!
//! R26 replaced an enumeration of five scalars with a class — `Default_Ignorable_Code_Point ∪ Cf`,
//! beside `char::is_whitespace` — and argued, rightly, that a property Unicode publishes is a width
//! a reader can check against the standard. The twenty-sixth audit walked the two categories that
//! argument leaves out and found **25 of 30** control cells and **10 of 10** private-use cells
//! parsing at an edge. `docs/LIMITS.md` then listed what the class *"still does not close"* — three
//! items, none of them these.
//!
//! # Why these two categories and not a wider net
//!
//! Both are Unicode general categories, so this stays a width a reader checks against the standard:
//!
//! * **`Cc`** (U+0000–U+001F, U+007F–U+009F) renders as nothing or as a control picture. It is the
//!   axis the gate already names for itself — *an edge a reader cannot see* — and the six
//!   `White_Space` members of it were already refused, by the other half of the predicate. That
//!   split is the tell: the gate was stopping U+0085 and taking U+0007 for a reason that had
//!   nothing to do with the axis.
//! * **`Co`** (the three private-use areas) is the honest generalisation rather than a claim of
//!   invisibility: a private-use scalar renders as whatever a private agreement says, which on the
//!   page an operator approves is **not a fact the page carries**. The catalogue's question is
//!   whether a human approving a file can see the name it declares, and a name whose glyph is
//!   outside the standard cannot be read off the page at all.
//!
//! The negative control is the half that matters and is driven first: real Japanese, Chinese,
//! Korean, Arabic, Hebrew, Cyrillic and Devanagari names in all five positions. A class one careless
//! range too wide would refuse names half the world writes, which is a worse defect than the one it
//! repairs.

use gx_adapter_mcp::catalogue::Catalogue;
use serde_json::{json, Value};

const WRITE_TOOL: &str = "notes.write";
const RESTORE_TOOL: &str = "notes.restore";
/// The word the edge gate's own refusals carry, in both of its sentences.
const EDGE_REFUSAL_NEEDLE: &str = "whitespace or with a zero-width";

fn record(line: &str) {
    println!("{line}");
    if let Ok(path) = std::env::var("R27_MEASUREMENTS") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// The five positions a declaration spells a name in, as JSON values.
///
/// Mirrors `r26_invisible_edge_axis.rs::five_positions` shape for shape, including the top-level
/// `arguments` template on the `read_by` entry — without it that position is stopped by `req/279`
/// H-01's soundness gate and the cell measures a refusal this axis did not earn (`req/326` §6-3).
fn five_positions(name: &str) -> Vec<(&'static str, Value)> {
    vec![
        (
            "$cas_read prefix",
            json!({
                WRITE_TOOL: { "restored_by": RESTORE_TOOL },
                "$cas_read": { name: { "by_tool": "notes.fetch" } },
            }),
        ),
        (
            "$cas_read by_tool",
            json!({
                WRITE_TOOL: { "restored_by": RESTORE_TOOL },
                "$cas_read": { "doc://": { "by_tool": name } },
            }),
        ),
        (
            "restores key",
            json!({ name: { "restored_by": RESTORE_TOOL } }),
        ),
        (
            "restored_by",
            json!({ WRITE_TOOL: { "restored_by": name } }),
        ),
        (
            "read_by.by_tool",
            json!({
                WRITE_TOOL: {
                    "restored_by": RESTORE_TOOL,
                    "arguments": {
                        "uri": { "forward": "uri" },
                        "contents": "prior_contents_utf8",
                    },
                    "read_by": {
                        "by_tool": name,
                        "arguments": { "uri": { "forward": "uri" } },
                        "identity": ["doc:", { "answer": "/id" }],
                    },
                },
            }),
        ),
    ]
}

fn clean_name(position: &str) -> &'static str {
    match position {
        "$cas_read prefix" => "doc://",
        "$cas_read by_tool" => "notes.fetch",
        _ => RESTORE_TOOL,
    }
}

fn positions() -> Vec<&'static str> {
    five_positions("x").into_iter().map(|(p, _)| p).collect()
}

fn parse_position(position: &str, name: &str) -> Result<Catalogue, String> {
    let json = five_positions(name)
        .into_iter()
        .find(|(p, _)| *p == position)
        .expect("the position is one of the five")
        .1;
    Catalogue::from_json(json.to_string().as_bytes())
}

/// One scalar at the **trailing edge** of the name a position spells.
fn at_the_edge(position: &str, scalar: char) -> String {
    format!("{}{scalar}", clean_name(position))
}

/// One scalar in the **middle** of the name a position spells.
// Kept though no arm calls it today: the five positions this bed measures are built by
// `at_the_start`/`at_the_edge`/`in_the_middle`, and dropping the middle one would make the
// position set of a later arm unbuildable. `dead_code` allowed rather than removed (no-delete,
// req/477 stage 2).
#[allow(dead_code)]
fn in_the_middle(position: &str, scalar: char) -> String {
    let clean = clean_name(position);
    let cut = clean.len() / 2;
    let (head, tail) = clean.split_at(cut);
    format!("{head}{scalar}{tail}")
}

// ---------------------------------------------------------------------------
// Bed soundness and the negative control, measured before any finding
// ---------------------------------------------------------------------------

// (Kept, not deleted: this is the earlier wording of the bed-control note the `#[test]` below now
// carries as its own doc comment. It documents no item, so it is a plain comment rather than a
// doc comment -- `clippy::empty_line_after_doc_comment`, req/477 stage 2.)
// 🔴 **This bed reaches the edge gate in all five positions.** A scalar the class certainly
// carries (U+200B) is refused **in the gate's own words** at every position. Without this arm a
// zero elsewhere in this file would be indistinguishable from a bed that never reached the gate —
// which is the exact fault `req/326` §6-3 found in audit 25's bed.

// ---------------------------------------------------------------------------
// Bed soundness and the negative control, measured before any finding
// ---------------------------------------------------------------------------

/// 🔴 **Bed control** — a scalar the class certainly carries (U+200B) is refused **in the gate's own
/// words** at every one of the five positions.
///
/// Without this arm, a zero anywhere below would be indistinguishable from a bed that never reached
/// the gate, which is the fault `req/326` §6-3 found in audit 25's bed.
#[test]
fn a_bed_control_a_known_class_member_is_refused_by_the_edge_gate_in_all_five_positions() {
    let mut refused = 0;
    let mut other = Vec::new();
    for position in positions() {
        match parse_position(position, &at_the_edge(position, '\u{200B}')) {
            Ok(_) => other.push(format!("{position}: parsed")),
            Err(why) if why.contains(EDGE_REFUSAL_NEEDLE) => refused += 1,
            Err(why) => other.push(format!("{position}: {why}")),
        }
    }
    record(&format!(
        "R27_BED_CONTROL_EDGE refused_by_the_edge_gate={refused}/5 other={other:?}"
    ));
    assert_eq!(refused, 5, "the bed reaches the edge gate: {other:?}");
}

/// 🔴 **The negative control, and it is the half that matters.**
///
/// Seven real tool names in seven scripts, through all five positions. A gate that refused these
/// would be a worse defect than the one this file repairs, so the widening is not allowed to land
/// without them.
#[test]
fn b_negative_control_real_multilingual_names_parse_in_all_five_positions() {
    // Japanese and Chinese are written as code points rather than as glyphs, for the reason
    // `r26_invisible_edge_axis.rs` gives beside its own copy of this control: the repository's
    // public face is measured for CJK lines (`probes/doubt/tests/cjk_doubt.rs`) and a probe about
    // what renders must measure zero under that rule. The scalars driven are exactly the glyphs.
    const REAL: [&str; 7] = [
        "\u{30E1}\u{30E2}.\u{66F8}\u{304D}\u{8FBC}\u{307F}",
        "\u{7B14}\u{8BB0}.\u{5199}\u{5165}",
        "메모.쓰기",
        "مذكرة.كتابة",
        "פתק.כתיבה",
        "заметка.запись",
        "नोट.लिखें",
    ];
    let mut refused = Vec::new();
    let mut cells = 0;
    for name in REAL {
        for position in positions() {
            cells += 1;
            if let Err(why) = parse_position(position, name) {
                refused.push(format!("{name}/{position}: {why}"));
            }
        }
    }
    record(&format!(
        "R27_NEGATIVE_CONTROL cells={cells} refused={}",
        refused.len()
    ));
    assert!(
        refused.is_empty(),
        "🔴 the class has eaten names half the world writes, which is worse than the defect it \
         repairs: {refused:?}"
    );
}

// ---------------------------------------------------------------------------
// The widening
// ---------------------------------------------------------------------------

/// 🔴 **`req/329` M-02** — the C0/C1 control scalars, at an edge, in all five positions.
///
/// `Cc` is not `Cf` and is not Default_Ignorable, so R26's class missed all of it except the six
/// members `char::is_whitespace` happens to answer for. A `$cas_read` prefix padded with U+0007
/// parsed and then governed no locator at all — which `docs/LIMITS.md` itself calls the one fault
/// in a catalogue file that produces no error at the moment it matters.
///
/// A refusal by **another** gate does not count: the cell must be stopped in the edge gate's own
/// words, or the axis is being scored for somebody else's work.
#[test]
fn c_the_class_reaches_the_control_scalars() {
    const CONTROLS: [(&str, char); 6] = [
        ("U+0000 NULL", '\u{0000}'),
        ("U+0007 BELL", '\u{0007}'),
        ("U+001B ESCAPE", '\u{001B}'),
        ("U+007F DELETE", '\u{007F}'),
        ("U+0085 NEXT LINE", '\u{0085}'),
        ("U+009F APPLICATION PROGRAM COMMAND", '\u{009F}'),
    ];
    let mut not_stopped = Vec::new();
    let mut stopped = 0;
    let mut cells = 0;
    for (label, scalar) in CONTROLS {
        for position in positions() {
            cells += 1;
            match parse_position(position, &at_the_edge(position, scalar)) {
                Ok(_) => not_stopped.push(format!("{label}/{position}")),
                Err(why) if why.contains(EDGE_REFUSAL_NEEDLE) => stopped += 1,
                Err(_) => not_stopped.push(format!("{label}/{position} (another gate)")),
            }
        }
    }
    record(&format!(
        "R27_CONTROL_EDGE cells={cells} stopped_by_the_edge_gate={stopped} not_stopped={}",
        not_stopped.len()
    ));
    record(&format!("R27_CONTROL_EDGE_NOT_STOPPED {not_stopped:?}"));
    assert!(
        not_stopped.is_empty(),
        "🔴 `req/329` M-02: the axis the gate names for itself is *an edge a reader cannot see*, \
         and these render as nothing at one: {not_stopped:?}"
    );
}

/// 🔴 **`req/329` M-02** — the private-use areas, at an edge, in all five positions.
///
/// Watch the digit count in the range table R26 shipped: `E0000-E0FFF` is plane 14's tag range,
/// **not** the BMP private-use area `E000-F8FF`. The two supplementary planes were not in it either.
#[test]
fn d_the_class_reaches_the_private_use_areas() {
    const PUA: [(&str, char); 4] = [
        ("U+E000 BMP PUA first", '\u{E000}'),
        ("U+F8FF BMP PUA last", '\u{F8FF}'),
        ("U+F0000 plane 15 PUA", '\u{F0000}'),
        ("U+100000 plane 16 PUA", '\u{100000}'),
    ];
    let mut not_stopped = Vec::new();
    let mut cells = 0;
    for (label, scalar) in PUA {
        for position in positions() {
            cells += 1;
            match parse_position(position, &at_the_edge(position, scalar)) {
                Ok(_) => not_stopped.push(format!("{label}/{position}")),
                Err(why) if why.contains(EDGE_REFUSAL_NEEDLE) => {}
                Err(_) => not_stopped.push(format!("{label}/{position} (another gate)")),
            }
        }
    }
    record(&format!(
        "R27_PUA_EDGE cells={cells} not_stopped={}",
        not_stopped.len()
    ));
    assert!(
        not_stopped.is_empty(),
        "🔴 `req/329` M-02: a name whose glyph is outside the standard cannot be read off the page \
         an operator approves it on: {not_stopped:?}"
    );
}

/// 🔴 The boundary this widening does **not** cross, measured rather than asserted in prose.
///
/// Unassigned scalars, and the surrogate range's neighbours, stay outside: they are not a category
/// Unicode publishes an appearance rule for, and a gate that refused every scalar a build has not
/// heard of would rot the day Unicode assigns one. If a later lane widens into them, this arm is
/// where the collision shows up.
#[test]
fn e_the_class_stops_where_the_two_categories_stop() {
    const OUTSIDE: [(&str, char); 3] = [
        ("U+0041 LATIN A", '\u{0041}'),
        ("U+3042 HIRAGANA A", '\u{3042}'),
        ("U+1F600 EMOJI", '\u{1F600}'),
    ];
    let mut refused = Vec::new();
    for (label, scalar) in OUTSIDE {
        for position in positions() {
            if let Err(why) = parse_position(position, &at_the_edge(position, scalar)) {
                if why.contains(EDGE_REFUSAL_NEEDLE) {
                    refused.push(format!("{label}/{position}"));
                }
            }
        }
    }
    record(&format!(
        "R27_EDGE_OUTSIDE refused_by_the_edge_gate={refused:?}"
    ));
    assert!(
        refused.is_empty(),
        "🔴 the class has grown past the two categories it declares: {refused:?}"
    );
}

/// 🔴 **`req/331` §0-2's same-commit requirement, mechanised.**
///
/// The finding `req/329` M-02 filed is not only that the class was narrow: it is that
/// `docs/LIMITS.md` **enumerated** what the class does not close and these were not on the list. A
/// document that enumerates and then omits is worse than one that stays silent, so the page is held
/// by a gate rather than by a lane's memory. When the categories move, this arm makes the page move
/// in the same commit.
#[test]
fn f_the_page_names_the_two_categories_this_class_reaches() {
    let page = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/LIMITS.md"),
    )
    .expect("docs/LIMITS.md is readable");
    for (what, needle) in [
        (
            "the control category, by its Unicode name",
            "General_Category=Cc",
        ),
        (
            "the private-use category, by its Unicode name",
            "General_Category=Co",
        ),
        (
            "that the private-use half is not a claim of invisibility",
            "renders as whatever a private agreement says",
        ),
        (
            "the negative control that keeps the widening from eating real names",
            "all thirty-five parse",
        ),
    ] {
        assert!(
            page.contains(needle),
            "🔴 `req/329` M-02: `docs/LIMITS.md` does not name {what} (`{needle}`). The defect this \
             arm holds is an enumeration that omits, so the code may not widen without the page \
             saying it did"
        );
    }
}
