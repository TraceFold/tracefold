// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/324` M-03 / M-04 (`req/38` §231 ruling 2)** — the axis is a class, not a list.
//!
//! # What broke
//!
//! The gate declares its axis in words: *an edge a reader cannot see*. R24 implemented it as
//! `char::is_whitespace`; R25 widened it to `char::is_whitespace` **plus five enumerated scalars**
//! and `docs/LIMITS.md` wrote *"the width is exactly these five, and they are enumerated rather
//! than described"*. The twenty-fifth audit walked twelve more scalars that render as nothing at
//! an edge — soft hyphen, the Arabic letter mark, the Hangul fillers, the Mongolian vowel
//! separator, the directional marks and isolates, an invisible operator, the interlinear
//! annotation anchor, a language tag — through all five positions a declaration spells a name in.
//! **The edge gate stopped none of the sixty cells.** Twelve were stopped by an unrelated fault,
//! forty-eight parsed; and a `$cas_read` prefix padded with any of them parsed and then governed
//! **no locator at all**, which is the one fault in that file that produces no error at the moment
//! it matters.
//!
//! Inside the enumeration there was a second hole: a name made **only** of the five enumerated
//! scalars is the *unnamed* fault, and `names_nothing` is asked at **three** sites while a
//! declaration spells a name in **five** slots. Ten of twenty-five such cells were accepted.
//!
//! # What this file requires
//!
//! One predicate over a **class** — the scalars Unicode itself calls default-ignorable, plus the
//! format category — asked in all five positions for the edge question and in all five slots for
//! the unnamed question.
//!
//! # 🔴 The negative control is the load-bearing half
//!
//! A class predicate is one careless range away from refusing legitimate multilingual names, which
//! would be a worse defect than the one it repairs: `req/325` item 4 makes the negative control
//! **HARD**. Real Japanese, Chinese, Korean, Arabic and Hebrew tool names are driven through all
//! five positions here and must parse. Widen the class until this file's positive arms are green;
//! if the negative arms go red, the class is wrong, not the names.

use gx_adapter_mcp::Catalogue;

const WRITE_TOOL: &str = "notes.write";
const RESTORE_TOOL: &str = "notes.restore";

/// The scalars the shipped enumeration already carried — the positive control.
const THE_ENUMERATION: [(&str, char); 5] = [
    ("U+200B ZERO WIDTH SPACE", '\u{200B}'),
    ("U+200C ZERO WIDTH NON-JOINER", '\u{200C}'),
    ("U+200D ZERO WIDTH JOINER", '\u{200D}'),
    ("U+2060 WORD JOINER", '\u{2060}'),
    ("U+FEFF ZERO WIDTH NO-BREAK SPACE", '\u{FEFF}'),
];

/// 🔴 The twelve `req/324` M-03 measured passing the gate in all five positions.
///
/// Two families on purpose, and the second is why "format character" is not the right class on its
/// own: the Hangul fillers are `Lo` — letters — and are still blank on the page an operator
/// approves.
const OUTSIDE_THE_ENUMERATION: [(&str, char); 12] = [
    ("U+00AD SOFT HYPHEN", '\u{00AD}'),
    ("U+061C ARABIC LETTER MARK", '\u{061C}'),
    ("U+115F HANGUL CHOSEONG FILLER", '\u{115F}'),
    ("U+1160 HANGUL JUNGSEONG FILLER", '\u{1160}'),
    ("U+17B4 KHMER VOWEL INHERENT AQ", '\u{17B4}'),
    ("U+180E MONGOLIAN VOWEL SEPARATOR", '\u{180E}'),
    ("U+200E LEFT-TO-RIGHT MARK", '\u{200E}'),
    ("U+2061 FUNCTION APPLICATION", '\u{2061}'),
    ("U+2066 LEFT-TO-RIGHT ISOLATE", '\u{2066}'),
    ("U+3164 HANGUL FILLER", '\u{3164}'),
    ("U+FFF9 INTERLINEAR ANNOTATION ANCHOR", '\u{FFF9}'),
    ("U+E0001 LANGUAGE TAG", '\u{E0001}'),
];

/// 🔴 **The negative control's material** — names an operator in Tokyo, Beijing, Seoul, Cairo or
/// Tel Aviv would actually write, none of which carries an invisible edge.
///
/// Hangul syllables sit beside the Hangul **fillers** in the positive list above and must not share
/// their fate; an Arabic name sits beside the Arabic **letter mark**; a right-to-left script is not
/// a right-to-left *override*.
const REAL_MULTILINGUAL_NAMES: [(&str, &str); 7] = [
    // 🔴 Japanese and Chinese are written as code points rather than as glyphs, for the reason
    // `probes/doubt/tests/cjk_doubt.rs` writes its own ranges that way: the repository's public
    // face is measured for CJK lines and a probe about CJK must measure zero under its own rule.
    // The scalars driven are exactly the glyphs -- \u{30E1}\u{30E2} "memo", \u{7B46}\u{8A18}
    // "notes" -- and every one of them is visible on the page an operator approves.
    (
        "Japanese",
        "\u{30E1}\u{30E2}.\u{66F8}\u{304D}\u{8FBC}\u{3080}",
    ),
    ("Chinese", "\u{7B14}\u{8BB0}.\u{5199}\u{5165}"),
    ("Korean", "노트.쓰기"),
    ("Arabic (RTL)", "ملاحظات.كتابة"),
    ("Hebrew (RTL)", "הערות.כתיבה"),
    ("Cyrillic", "заметки.записать"),
    ("Devanagari", "नोट.लिखें"),
];

/// The five positions a declaration file spells a name in, with `name` substituted.
///
/// 🔴 `$cas_read`'s `by_tool` may **not** be padded on `notes.restore`: this catalogue declares
/// that tool as the inverse of `notes.write`, and `req/312` H-01's gate refuses a read face that is
/// an inverse — a cell refused by *that* gate would score a refusal this axis did not earn, which
/// is the fail-open shape `req/324` §9-2 caught in its own first draft.
fn five_positions(name: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "$cas_read prefix",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
                     "$cas_read": {{ "{name}": {{ "by_tool": "notes.fetch" }} }} }}"#
            ),
        ),
        (
            "$cas_read by_tool",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
                     "$cas_read": {{ "doc://": {{ "by_tool": "{name}" }} }} }}"#
            ),
        ),
        (
            "restores key",
            format!(r#"{{ "{name}": {{ "restored_by": "{RESTORE_TOOL}" }} }}"#),
        ),
        (
            "restored_by",
            format!(r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{name}" }} }}"#),
        ),
        (
            // 🔴 The entry carries a top-level `arguments` template, which `req/324`'s own bed did
            // not. Without it every cell in this position is refused by `req/279` H-01's soundness
            // gate — *a read face without a template is not a declaration* — and that refusal is
            // one this axis did not earn. It is why the audit could only report this position as
            // "stopped by another gate × 12" (`A25_WS_OTHER_GATE`) and could not tell whether the
            // edge gate would have stopped it. Sound here, so the cell measures the axis.
            "read_by.by_tool",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}",
                     "arguments": {{ "uri": {{ "forward": "uri" }},
                                     "contents": "prior_contents_utf8" }},
                     "read_by": {{ "by_tool": "{name}",
                                   "arguments": {{ "uri": {{ "forward": "uri" }} }},
                                   "identity": ["doc:", {{ "answer": "/id" }}] }} }} }}"#
            ),
        ),
    ]
}

/// The unpadded name each position is spelled with.
fn clean_name(position: &str) -> &'static str {
    match position {
        "$cas_read prefix" => "doc://",
        "$cas_read by_tool" => "notes.fetch",
        _ => RESTORE_TOOL,
    }
}

/// The padded name for a position: the clean name with one invisible scalar at its trailing edge.
fn padded(position: &str, scalar: char) -> String {
    format!("{}{scalar}", clean_name(position))
}

/// The word the edge gate's own refusals carry, in both of its sentences.
const EDGE_REFUSAL_NEEDLE: &str = "whitespace or with a zero-width";

/// The words the *unnamed* sentences carry — a different fault with a different remedy than the
/// edge gate's.
///
/// Two spellings because two of the five slots are **keys** rather than tool names: a `$cas_read`
/// prefix names no *tool*, it names nothing. Both sentences carry `What to fix:` and neither is the
/// edge gate's, which is the property this arm is about.
const UNNAMED_NEEDLES: [&str; 2] = ["names no tool", "names nothing"];

fn parse_position(position: &str, name: &str) -> core::result::Result<Catalogue, String> {
    let json = five_positions(name)
        .into_iter()
        .find(|(p, _)| *p == position)
        .expect("the position is one of the five")
        .1;
    Catalogue::from_json(json.as_bytes())
}

fn positions() -> Vec<&'static str> {
    five_positions("x").into_iter().map(|(p, _)| p).collect()
}

// ---------------------------------------------------------------------------------------------
// Bed control -- every count below is worthless if the cells do not reach the gate being measured
// ---------------------------------------------------------------------------------------------

/// 🔴 **The bed control.** Each of the five positions is shown to reach the edge gate, by being
/// refused *in the edge gate's own words* when padded with a scalar the shipped enumeration
/// already carried.
#[test]
fn every_one_of_the_five_positions_reaches_the_edge_gate() {
    let mut reached: Vec<(&str, usize)> = Vec::new();
    for position in positions() {
        let mut n = 0usize;
        for (_, scalar) in THE_ENUMERATION {
            if let Err(why) = parse_position(position, &padded(position, scalar)) {
                if why.contains(EDGE_REFUSAL_NEEDLE) {
                    n += 1;
                }
            }
        }
        reached.push((position, n));
    }
    println!("R26_BED reached_by_the_edge_gate={reached:?}");
    for (position, n) in &reached {
        assert_eq!(
            *n, 5,
            "🔴 the bed is broken before anything is measured: position {position:?} was refused \
             in the edge gate's own words for only {n} of the five scalars the shipped \
             enumeration already carries. Every count in this file is a count of cells that reach \
             this gate; a cell stopped by some other fault scores a refusal this axis did not earn."
        );
    }
}

// ---------------------------------------------------------------------------------------------
// M-03 -- the axis outside the enumeration
// ---------------------------------------------------------------------------------------------

/// 🔴 **`req/324` M-03** — sixty cells the audit measured the gate letting through.
#[test]
fn an_invisible_edge_is_refused_in_all_five_positions_for_the_whole_class() {
    let mut accepted: Vec<String> = Vec::new();
    let mut stopped_by_another_gate: Vec<String> = Vec::new();
    let mut cells = 0usize;
    for position in positions() {
        for (label, scalar) in OUTSIDE_THE_ENUMERATION {
            cells += 1;
            let cell = format!("{position} / {label}");
            match parse_position(position, &padded(position, scalar)) {
                Ok(_) => accepted.push(cell),
                Err(why) if why.contains(EDGE_REFUSAL_NEEDLE) => {}
                Err(_) => stopped_by_another_gate.push(cell),
            }
        }
    }
    println!(
        "R26_WS cells={cells} accepted={} stopped_by_another_gate={} stopped_by_the_edge_gate={}",
        accepted.len(),
        stopped_by_another_gate.len(),
        cells - accepted.len() - stopped_by_another_gate.len()
    );
    println!("R26_WS_ACCEPTED={accepted:?}");
    println!("R26_WS_OTHER_GATE={stopped_by_another_gate:?}");
    assert_eq!(cells, 60, "bed: twelve scalars in five positions");
    assert!(
        accepted.is_empty() && stopped_by_another_gate.is_empty(),
        "🔴 `req/324` M-03 (`req/38` §231 ruling 2): the gate declares its axis as *an edge a \
         reader cannot see* and implements it as a list. {} of {cells} cells parsed and {} were \
         stopped by an unrelated fault -- a refusal this axis did not earn and which moves the \
         moment that other fault is fixed. accepted={accepted:?} other={stopped_by_another_gate:?}",
        accepted.len(),
        stopped_by_another_gate.len()
    );
}

/// 🔴 **`req/324` M-03, the quiet half** — a `$cas_read` prefix with an invisible edge governed
/// nothing rather than being refused, which is the one fault in this file that produces no error
/// at the moment it matters.
#[test]
fn a_prefix_with_an_invisible_edge_never_reaches_the_road_where_it_governs_nothing() {
    let mut governs_nothing: Vec<String> = Vec::new();
    for (label, scalar) in OUTSIDE_THE_ENUMERATION {
        let name = padded("$cas_read prefix", scalar);
        if let Ok(catalogue) = parse_position("$cas_read prefix", &name) {
            if catalogue.cas_read_for("doc://page/1").is_none() {
                governs_nothing.push(label.to_string());
            }
        }
    }
    println!(
        "R26_PREFIX accepted_and_governs_nothing={}",
        governs_nothing.len()
    );
    assert!(
        governs_nothing.is_empty(),
        "🔴 `req/324` M-03: {} prefixes parsed and then matched no locator at all, so this file's \
         declared read road is silently not taken and every read falls back to `resources/read` on \
         a deployment that believes it opted in: {governs_nothing:?}",
        governs_nothing.len()
    );
}

// ---------------------------------------------------------------------------------------------
// M-04 -- inside the enumeration: the unnamed question is asked in three slots of five
// ---------------------------------------------------------------------------------------------

/// 🔴 **`req/324` M-04** — a name made only of invisible scalars is the *unnamed* fault in every
/// slot a name is declared in, not in three of the five.
#[test]
fn a_name_of_only_invisible_scalars_is_the_unnamed_fault_in_all_five_slots() {
    let all: Vec<(&str, char)> = THE_ENUMERATION
        .iter()
        .chain(OUTSIDE_THE_ENUMERATION.iter())
        .copied()
        .collect();
    let mut accepted: Vec<String> = Vec::new();
    let mut cells = 0usize;
    for position in positions() {
        for (label, scalar) in &all {
            cells += 1;
            let name: String = std::iter::repeat_n(*scalar, 3).collect();
            if parse_position(position, &name).is_ok() {
                accepted.push(format!("{position} / {label}"));
            }
        }
    }
    println!(
        "R26_UNNAMED cells={cells} accepted={} accepted_list={accepted:?}",
        accepted.len()
    );
    assert_eq!(cells, 85, "bed: seventeen scalars in five slots");
    assert!(
        accepted.is_empty(),
        "🔴 `req/324` M-04 (`req/38` §231 ruling 2): a declared name that is nothing but invisible \
         scalars was accepted in {} of {cells} cells. `names_nothing` is asked at three sites and \
         a declaration spells a name in five slots, so the two slots nobody swept -- the `restores` \
         key and the `$cas_read` prefix -- still take a name that is blank on the page an operator \
         approved: {accepted:?}",
        accepted.len()
    );
}

/// 🔴 The *unnamed* fault keeps its own sentence: a reader whose file says `"restored_by": "\u{200b}"`
/// must be told to write a name, not to trim one they never wrote.
///
/// 🔴 Widened to **all five slots and all seventeen scalars** because `docs/LIMITS.md` v0.5-m says
/// the property at that width (*"each in the sentence a reader can act on rather than in the edge
/// gate's"*). Asserting a page's claim at a narrower width than the page states it is `req/324`
/// H-01's own defect, one file over, so the arm is widened rather than the sentence softened.
#[test]
fn the_unnamed_fault_is_refused_in_its_own_words_and_not_the_edge_gates() {
    let all: Vec<(&str, char)> = THE_ENUMERATION
        .iter()
        .chain(OUTSIDE_THE_ENUMERATION.iter())
        .copied()
        .collect();
    let mut wrong_sentence: Vec<String> = Vec::new();
    let mut cells = 0usize;
    for position in positions() {
        for (label, scalar) in &all {
            cells += 1;
            let name: String = std::iter::repeat_n(*scalar, 3).collect();
            match parse_position(position, &name) {
                // The unnamed sentence, and *not* the edge gate's: a reader who wrote nothing must
                // be told to write a name, not to trim an edge they never typed.
                Err(why)
                    if UNNAMED_NEEDLES.iter().any(|n| why.contains(n))
                        && !why.contains(EDGE_REFUSAL_NEEDLE) => {}
                other => wrong_sentence.push(format!("{position} / {label} -> {other:?}")),
            }
        }
    }
    println!(
        "R26_UNNAMED_SENTENCE cells={cells} wrong={}",
        wrong_sentence.len()
    );
    assert_eq!(cells, 85, "bed: seventeen scalars in five slots");
    assert!(
        wrong_sentence.is_empty(),
        "🔴 `req/320` L-01's property has to survive the widening, in every slot: an all-invisible \
         name is the *unnamed* fault and reaches the sentence that asks for a name, not the edge \
         gate's. {wrong_sentence:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// 🔴 The negative controls (`req/325` item 4, HARD)
// ---------------------------------------------------------------------------------------------

/// 🔴 **HARD negative control** — real multilingual tool names parse in all five positions.
///
/// This is the arm that fails if the class is widened past the axis it declares. A gate that
/// refuses a Japanese or an Arabic tool name has not closed a hole, it has broken the product
/// for everyone who does not write in ASCII. (The names themselves are in
/// [`REAL_MULTILINGUAL_NAMES`]; they are not repeated here because this file measures zero CJK
/// lines by the same rule `cjk_doubt.rs` holds itself to.)
#[test]
fn real_multilingual_names_are_not_refused_in_any_position() {
    let mut refused: Vec<String> = Vec::new();
    let mut cells = 0usize;
    for position in positions() {
        for (language, name) in REAL_MULTILINGUAL_NAMES {
            cells += 1;
            // The `$cas_read` prefix is matched against resource URIs, so a bare script name is
            // spelled the way a locator is; the point of the cell is the script, not the syntax.
            let spelled = if position == "$cas_read prefix" {
                format!("doc://{name}/")
            } else {
                name.to_string()
            };
            if let Err(why) = parse_position(position, &spelled) {
                refused.push(format!("{position} / {language} / {name:?}: {why}"));
            }
        }
    }
    println!("R26_NEGATIVE cells={cells} refused={}", refused.len());
    assert!(
        refused.is_empty(),
        "🔴 `req/325` item 4's HARD negative control: {} of {cells} cells refused a name a real \
         operator would write. The class is wrong, not the name. Every scalar in these names is \
         visible on the page an operator approves, which is the axis the gate declares: {refused:?}",
        refused.len()
    );
}

/// 🔴 The second negative control: a plain prefix still governs the locator it names, so
/// "governs nothing" cannot pass by nothing being declared.
#[test]
fn a_plain_prefix_still_governs_its_locator() {
    let catalogue = parse_position("$cas_read prefix", "doc://").expect("a plain prefix parses");
    let governed = catalogue.cas_read_for("doc://page/1");
    println!("R26_NEGATIVE plain_governs={}", governed.is_some());
    assert!(
        governed.is_some(),
        "🔴 the bed for `a_prefix_with_an_invisible_edge_never_reaches_the_road_where_it_governs \
         _nothing` is that a prefix without an invisible edge *does* govern. Without this the \
         arm above passes on a build where nothing governs anything."
    );
}

/// 🔴 The third negative control: a visible scalar that is not invisible at an edge -- a full-width
/// ideographic full stop, a hyphen -- is not swept up by the class.
#[test]
fn visible_scalars_at_an_edge_are_not_the_edge_fault() {
    let mut refused: Vec<String> = Vec::new();
    for (label, scalar) in [
        ("U+3002 IDEOGRAPHIC FULL STOP", '\u{3002}'),
        ("U+002D HYPHEN-MINUS", '-'),
        ("U+05D0 HEBREW LETTER ALEF", '\u{05D0}'),
        ("U+0627 ARABIC LETTER ALEF", '\u{0627}'),
        ("U+AC00 HANGUL SYLLABLE GA", '\u{AC00}'),
        ("U+1F4DD MEMO", '\u{1F4DD}'),
    ] {
        for position in positions() {
            let name = padded(position, scalar);
            if let Err(why) = parse_position(position, &name) {
                refused.push(format!("{position} / {label}: {why}"));
            }
        }
    }
    println!("R26_NEGATIVE visible_edge_refused={}", refused.len());
    assert!(
        refused.is_empty(),
        "🔴 a scalar a reader *can* see at an edge is not this fault: {refused:?}"
    );
}
