// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/316` H-01, L-01 and L-02** (`req/317` §1 items 1 and 6, `req/38` §227 rulings 1, 2
//! and 4) — one question, one function, and every gate that asks it.
//!
//! # What three consecutive audits measured
//!
//! ```text
//! A21  the sentence a gate prints and the predicate it runs ask different questions
//! A22  the `$cas_read` gate asks about `restores` **keys** and never about `restored_by` values
//! A23  A23_GATES escrow_road_inverse_face_accepted=true
//!      A23_GATES cas_road_inverse_face_accepted=false
//!      A23_GATES escrow_road_effect_face_accepted=false
//!      A23_ESCROW object_before="the paragraph a person wrote before any agent touched it\n"
//!      A23_ESCROW verdict=refused-before-verdict object_after="" client_calls=0
//! ```
//!
//! R23 repaired the `$cas_read` gate. The twenty-third audit then wrote the **same** contradiction
//! into `read_by` — the escrow road's face — and gx accepted it, called it from the escrow before
//! any verdict existed, and emptied a paragraph a person had written. The agent's own effect was
//! never sent. `req/38` §227 ruling 1 named the diagnosis: it is not that three sites were spelled
//! wrongly, it is that **the same question is spelled at three sites at all**.
//!
//! So the arms here come in two kinds, and the second kind is the point:
//!
//! * behavioural — both gates refuse a read face this file names as an inverse, and the remedy
//!   each prints cannot be followed into the harm it names;
//! * **structural** — the number of places in `catalogue.rs` that ask "does this file say this tool
//!   writes" is held at one, so a fourth gate spelled by hand is red here before an audit finds it.
//!
//! # Red-first
//!
//! No symbol this lane created is named. Sentences are spelled as needles and the structural arms
//! read `src/catalogue.rs` as text, so this file compiles at `e944d74` and fails on its assertions.

use std::path::{Path, PathBuf};

use gx_adapter_mcp::Catalogue;

const WRITE_TOOL: &str = "notes.write";
const RESTORE_TOOL: &str = "notes.restore";

/// The sentence `gx_adapter_mcp::CAS_READ_FACE_IS_AN_INVERSE` carries, spelled rather than imported
/// so this file builds at the branch point (`req/299` §0's rule).
const INVERSE_NEEDLE: &str = "the inverse of one";

/// R17's wording rule: every refusal in this family carries a remedy.
const REMEDY: &str = "What to fix:";

fn adapter_src(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()))
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// 🔴 **`req/320` M-03 (R25)** — the source with its comment lines removed.
///
/// The repaired file quotes the spellings it removed inside comments — the record of why each gate
/// changed — and a scan that counted those would be a scan that punishes writing the reason down
/// (`req/316` §5 self-admission 1).
fn code_of(src: &str) -> Vec<(usize, String)> {
    src.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .map(|(n, line)| (n + 1, line.to_string()))
        .collect()
}

/// 🔴 **`req/320` M-03 (R25)** — which `fn` each line of `catalogue.rs` sits in.
///
/// `impl` blocks in this file are formatted by `rustfmt`, so a method opens at four spaces and
/// closes on a line that is exactly `    }`. Attribution is by that shape rather than by a
/// character window: R24's sweep arm took 3,500 characters forward from a name and the
/// twenty-fourth audit measured the margin at **132 characters of prose**, which is a gate held up
/// by a comment's length.
fn enclosing_fn(src: &str, line_no: usize) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut current = "<file scope>".to_string();
    for (n, line) in lines.iter().enumerate() {
        if n + 1 > line_no {
            break;
        }
        if *line == "    }" {
            current = "<impl scope>".to_string();
            continue;
        }
        let trimmed = line.trim_start();
        let mut rest = trimmed;
        for prefix in ["pub(crate) ", "pub ", "const ", "async ", "unsafe "] {
            rest = rest.strip_prefix(prefix).unwrap_or(rest);
        }
        if let Some(after) = rest.strip_prefix("fn ") {
            current = after
                .split(['(', '<'])
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
        }
    }
    current
}

/// The body of one method, from its `pub fn` line to the `    }` that closes it.
fn body_of(src: &str, name: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with(&format!("    pub fn {name}")))
        .unwrap_or_else(|| panic!("`{name}` is a method of this file"));
    let end = lines[start..]
        .iter()
        .position(|line| *line == "    }")
        .unwrap_or_else(|| panic!("`{name}` closes on a `    }}` line"));
    lines[start..=start + end].join("\n")
}

/// Every read of the private `restores` map, with the function it sits in.
///
/// 🔴 The whole point of counting the **field** rather than a predicate's spelling: any question
/// about what this file says a tool is has to reach this map, in whatever words. See
/// `the_question_whether_this_file_says_a_tool_writes_is_spelled_once`.
fn restores_field_reads(src: &str) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for (n, line) in code_of(src) {
        let count = line.matches("self.restores").count();
        if count == 0 {
            continue;
        }
        let owner = enclosing_fn(src, n);
        match out.iter_mut().find(|(name, _)| *name == owner) {
            Some((_, total)) => *total += count,
            None => out.push((owner, count)),
        }
    }
    out.sort();
    out
}

/// Every **bare** `restores` in code — the map named without `self.`, which is only possible while
/// `from_json` is building it and on the struct's own field line.
fn bare_restores_lines(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (_, line) in code_of(src) {
        let bytes = line.as_bytes();
        let mut at = 0usize;
        while let Some(found) = line[at..].find("restores") {
            let start = at + found;
            let end = start + "restores".len();
            at = end;
            if line[..start].ends_with("self.") {
                continue;
            }
            // A word boundary on the left, and a `.` / `,` / `:` on the right -- which is what
            // *using* the map looks like, as opposed to naming it in a sentence.
            let left_ok = start == 0 || {
                let c = bytes[start - 1];
                !(c.is_ascii_alphanumeric() || c == b'_')
            };
            let right_ok = matches!(bytes.get(end), Some(b'.') | Some(b',') | Some(b':'));
            if left_ok && right_ok {
                out.push(line.trim().to_string());
                break;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// H-01 — the sibling gate
// ---------------------------------------------------------------------------

/// The audit's catalogue, verbatim apart from the resource prefix: the read face of `notes.write`
/// is the tool this same file names as `notes.write`'s inverse.
fn inverse_as_escrow_read_face() -> String {
    format!(
        r#"{{ "{WRITE_TOOL}": {{
            "restored_by": "{RESTORE_TOOL}",
            "read_by": {{ "by_tool": "{RESTORE_TOOL}",
                          "arguments": {{ "uri": {{ "forward": "uri" }} }},
                          "identity": ["doc:", {{ "answer": "/id" }}] }},
            "arguments": {{ "uri": {{ "forward": "uri" }}, "contents": "prior_contents_utf8" }} }} }}"#
    )
}

/// The same contradiction on the road R23 repaired, for the discriminator below.
fn inverse_as_cas_read_face() -> String {
    format!(
        r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
             "$cas_read": {{ "doc://": {{ "by_tool": "{RESTORE_TOOL}" }} }} }}"#
    )
}

/// And the spelling R18 closed, which has never been accepted: a read face that is an **effect**.
fn effect_as_escrow_read_face() -> String {
    format!(
        r#"{{ "{WRITE_TOOL}": {{
            "restored_by": "{RESTORE_TOOL}",
            "read_by": {{ "by_tool": "{WRITE_TOOL}",
                          "arguments": {{ "uri": {{ "forward": "uri" }} }},
                          "identity": ["doc:", {{ "answer": "/id" }}] }},
            "arguments": {{ "uri": {{ "forward": "uri" }}, "contents": "prior_contents_utf8" }} }} }}"#
    )
}

/// 🔴 `req/316` H-01: the escrow road's read face may not be a tool this file names as an inverse.
#[test]
fn an_escrow_read_face_this_file_names_as_an_inverse_is_refused() {
    let parsed = Catalogue::from_json(inverse_as_escrow_read_face().as_bytes());
    let why = parsed.as_ref().err().cloned().unwrap_or_default();
    println!("R24_H01 accepted={} why={why}", parsed.is_ok());
    assert!(
        parsed.is_err(),
        "🔴 `req/316` H-01: this catalogue parsed. gx then called {RESTORE_TOOL:?} from the escrow \
         road — which carries no admission and runs before any verdict — and the paragraph a \
         person had written was empty afterwards, with the agent's own effect never sent \
         (`client_calls: 0`). A restore tool writes by construction, and `restored_by` is this \
         file saying so"
    );
    assert!(
        why.contains(INVERSE_NEEDLE),
        "and the sentence names the fact the reader has to act on — that this file declares that \
         tool as an inverse — rather than the one about effects, which is true of a spelling this \
         file does not use: {why}"
    );
    assert!(why.contains(REMEDY), "R17's wording rule: {why}");
}

/// 🔴 `req/38` §227 ruling 1: the two gates that ask this question give the same answer.
///
/// This is the discriminator the audit printed. Two of the three rows were already right; the
/// finding is that they were right **separately**, and the third row is what one gate knowing
/// something the other does not costs.
#[test]
fn the_two_read_face_gates_agree_about_an_inverse() {
    let rows = [
        ("escrow road, inverse face", inverse_as_escrow_read_face()),
        ("cas road,    inverse face", inverse_as_cas_read_face()),
        ("escrow road, effect  face", effect_as_escrow_read_face()),
    ];
    let mut accepted: Vec<&str> = Vec::new();
    for (what, json) in &rows {
        let parsed = Catalogue::from_json(json.as_bytes());
        println!("R24_GATES {what} accepted={}", parsed.is_ok());
        if parsed.is_ok() {
            accepted.push(what);
        } else {
            let why = parsed.err().unwrap_or_default();
            assert!(
                why.contains(REMEDY),
                "each refusal carries a remedy: {what}: {why}"
            );
        }
    }
    println!("R24_GATES_ACCEPTED={accepted:?}");
    assert!(
        accepted.is_empty(),
        "🔴 `req/316` H-01: the same tool name, written into the `$cas_read` face, is refused, and \
         written into the `read_by` face is accepted. `docs/LIMITS.md` v0.5-j told a buyer the \
         soundness gate *\"asks about **both** sets a catalogue file declares\"* in the singular, \
         and a reader has no way to learn that the sentence is about one of two gates: {accepted:?}"
    );
}

/// 🔴 The remedy the escrow gate prints, followed **verbatim**, must not reach the harm it names.
///
/// This is the shape of the finding rather than one of its instances. `req/279` M-04's sentence
/// ends *"name a read face this catalogue does not declare as an effect"*, and `notes.restore` is
/// not declared as an effect: it is declared as an inverse. Following the printed instruction to
/// the letter arrived at a declaration that emptied the object.
#[test]
fn the_remedy_the_escrow_gate_prints_no_longer_leads_to_the_harm_it_names() {
    let effect_face = Catalogue::from_json(effect_as_escrow_read_face().as_bytes());
    let effect_why = effect_face.as_ref().err().cloned().unwrap_or_default();
    println!("R24_REMEDY effect_face_refused={}", effect_face.is_err());
    assert!(
        effect_face.is_err(),
        "the premise: the effect spelling is refused, which is the refusal that prints the remedy"
    );
    assert!(
        effect_why.contains("does not declare as an effect"),
        "and it is the remedy this arm is about: {effect_why}"
    );
    let followed = Catalogue::from_json(inverse_as_escrow_read_face().as_bytes());
    println!("R24_REMEDY followed_accepted={}", followed.is_ok());
    assert!(
        followed.is_err(),
        "🔴 `req/316` H-01: a deployment that reads the sentence above and does exactly what it \
         says arrives here. `req/312` H-01 was the same walk on the other road, and the repair for \
         it did not reach this one: {effect_why}"
    );
}

/// The control the repair must not take with it: a read face that is a **read** tool still parses.
///
/// Without this arm, "refuse every `read_by`" satisfies all three arms above and takes DR-46-9 A-3
/// with it.
#[test]
fn a_read_face_this_file_names_neither_way_still_parses() {
    let sound = format!(
        r#"{{ "{WRITE_TOOL}": {{
            "restored_by": "{RESTORE_TOOL}",
            "read_by": {{ "by_tool": "notes.get",
                          "arguments": {{ "uri": {{ "forward": "uri" }} }},
                          "identity": ["doc:", {{ "answer": "/id" }}] }},
            "arguments": {{ "uri": {{ "forward": "uri" }}, "contents": "prior_contents_utf8" }} }} }}"#
    );
    let parsed = Catalogue::from_json(sound.as_bytes());
    println!("R24_CONTROL accepted={}", parsed.is_ok());
    assert!(
        parsed.is_ok(),
        "a tool this file names neither as an effect nor as an inverse is what the remedy asks \
         for, and it has to be reachable: {:?}",
        parsed.err()
    );
}

/// The other control: every catalogue this repository ships still parses.
#[test]
fn every_shipped_catalogue_still_parses_under_the_one_predicate() {
    let mut checked: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(fixtures()).expect("the fixtures directory");
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !name.ends_with("-catalogue.json") && !name.ends_with("-catalogue-cas.json") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{name} readable: {e}"));
        let outcome = Catalogue::from_json(&bytes);
        println!("R24_FIXTURE {name} accepted={}", outcome.is_ok());
        assert!(
            outcome.is_ok(),
            "🔴 the unified predicate refuses a catalogue this repository ships: {name}: {:?}",
            outcome.err()
        );
        checked.push(name);
    }
    assert!(
        checked.len() >= 4,
        "a scan that found nothing would satisfy the loop above: {checked:?}"
    );
}

// ---------------------------------------------------------------------------
// 🔴 §227 ruling 1 and 2 — the structural half: one site, and a sweep that stays swept
// ---------------------------------------------------------------------------

/// 🔴 **`req/38` §227 ruling 1** — "does this file say this tool writes" is spelled **once**.
///
/// # Why the arm counts text rather than testing behaviour
///
/// Every behavioural arm above passes on a tree where the question is spelled correctly at three
/// separate sites. That tree is exactly the tree the twenty-third audit found: R22's repair was
/// correct, R23's repair was correct, and the defect was that a fourth gate could be — and was —
/// written without either. A count of spellings is the only thing that is red *before* the fourth
/// one is wrong.
///
/// Two spellings are allowed and both are named here:
///
/// * the one inside the shared predicate itself;
/// * `declared_reversibility`, which reads the same map to answer a **different** question — *is
///   there a declared inverse for this tool* — and folding it in would be the same mistake with the
///   sign reversed.
#[test]
fn the_question_whether_this_file_says_a_tool_writes_is_spelled_once() {
    let src = adapter_src("catalogue.rs");
    let reads = restores_field_reads(&src);
    let total: usize = reads.iter().map(|(_, n)| n).sum();
    println!("R25_FIELD_READS total={total} {reads:?}");
    // The scan is looking at the file it thinks it is (`req/316` §5 self-admission 2).
    assert!(
        total > 0 && src.contains("cas_read_soundness") && src.contains("entry_fault"),
        "an arm that measures an empty file gives the right answer for the wrong reason"
    );
    let declared: Vec<(String, usize)> = [
        // The two builders and the two convenience readers, none of which is a gate.
        ("declared", 1usize),
        ("declared_reversibility", 1),
        ("entry_fault", 1),
        ("restore_for", 1),
        ("soundness", 1),
        ("spec_for", 1),
        ("with_prior_read", 1),
        ("with_restore", 1),
        ("with_restore_template", 1),
        // 🔴 The shared predicate itself: the key half and the value half, and nowhere else.
        ("writes_per_this_file", 2),
    ]
    .iter()
    .map(|(name, n)| ((*name).to_string(), *n))
    .collect();
    assert_eq!(
        reads, declared,
        "🔴 `req/38` §227 ruling 1, held by `req/320` M-03's repair: every read of the private \
         `restores` map in this file is listed here, with the function it sits in. A gate that \
         asks *does this file say this tool writes* has to reach this map in some spelling, so a \
         third one is a row that is not in this list — whatever words it is written in. R24 \
         counted two **spellings** instead (`restores.contains_key` and `.restored_by() == `) and \
         the twenty-fourth audit walked past it in one line: `restores.get(t).is_some()` and \
         `tool == s.restored_by()` ask the identical question and were counted zero times"
    );
    let bare = bare_restores_lines(&src);
    println!("R25_BARE_RESTORES {bare:?}");
    assert_eq!(
        bare,
        vec![
            "restores: BTreeMap<String, RestoreSpec>,".to_string(),
            "restores.insert(tool, spec);".to_string(),
            "restores,".to_string(),
        ],
        "🔴 and the map named **without** `self.` — the other way a gate could reach it and not \
         appear in the list above. The three lines allowed are the struct's own field and the two \
         `from_json` uses that fill the map and move it into `Self`, before there is a `Self` to \
         ask. (A `let` that merely binds the name is not a use of it: the scan asks for a `.`, `,` \
         or `:` on the right, which is what reaching the map looks like.)"
    );
    // The two halves stay two halves: the value scan is not folded into the key lookup, because
    // which set a face fell into is what the two refusal sentences differ by.
    let predicate = body_of(&src, "writes_per_this_file");
    assert!(
        predicate.contains("contains_key") && predicate.contains("restored_by()"),
        "the shared predicate still asks both halves of the question: {predicate}"
    );
}

/// 🔴 **`req/38` §227 ruling 2 (the sibling sweep, as a standing gate)** — each gate's body names
/// the shared predicate.
///
/// The count above holds the number of places the map is reached. This holds *which functions*
/// stopped reaching it, so a lane that deletes one gate's call and adds a private helper elsewhere
/// in the file is red rather than merely differently arranged.
///
/// 🔴 **`req/320` M-03 (R25)** — the body is the body now. R24 took **3,500 characters** forward
/// from the gate's name, and the twenty-fourth audit measured the margin before that window reached
/// the *next* function at 132 characters of comment prose: a lane tidying a comment would have
/// turned this arm green by accident. `body_of` ends the body where `rustfmt` ends it.
#[test]
fn both_read_face_gates_ask_the_shared_predicate() {
    let src = adapter_src("catalogue.rs");
    let mut missing: Vec<&str> = Vec::new();
    for gate in ["entry_fault", "cas_read_soundness"] {
        let body = body_of(&src, gate);
        let asks = code_of(&body)
            .iter()
            .any(|(_, line)| line.contains("writes_per_this_file"));
        println!(
            "R24_SWEEP gate={gate} asks_the_shared_predicate={asks} body_chars={}",
            body.len()
        );
        if !asks {
            missing.push(gate);
        }
    }
    assert!(
        missing.is_empty(),
        "🔴 `req/38` §227 ruling 2: these gates ask the question themselves rather than asking the \
         one function. That is the shape the twenty-first, twenty-second and twenty-third audits \
         each found one instance of: {missing:?}"
    );
    // 🔴 The negative control the character window could not have: the body ends before the next
    // method begins, so a call sitting in the *neighbour* does not answer for this one.
    {
        let (gate, neighbour) = ("entry_fault", "cas_read_soundness");
        let body = body_of(&src, gate);
        assert!(
            !body.contains(&format!("pub fn {neighbour}")),
            "🔴 `req/320` M-03: `{gate}`'s body reaches into `{neighbour}`, which is the window \
             this repair replaced"
        );
    }
}

// ---------------------------------------------------------------------------
// L-01 — a declaration is a JSON object
// ---------------------------------------------------------------------------

/// 🔴 `req/316` L-01: the positional spellings serde accepted are refused, and the refusal says so.
#[test]
fn a_declaration_written_as_a_json_array_is_refused() {
    let spellings = [
        (
            "$cas_read entry",
            r#"{ "$cas_read": { "doc://": ["doc.read", {"uri": "resource"}] } }"#.to_string(),
        ),
        (
            "$cas_read entry, one element",
            r#"{ "$cas_read": { "doc://": ["doc.read"] } }"#.to_string(),
        ),
        (
            "restore entry",
            format!(r#"{{ "{WRITE_TOOL}": ["{RESTORE_TOOL}", null, null] }}"#),
        ),
        (
            "restore entry, one element",
            format!(r#"{{ "{WRITE_TOOL}": ["{RESTORE_TOOL}"] }}"#),
        ),
    ];
    let mut accepted: Vec<&str> = Vec::new();
    for (what, json) in &spellings {
        let parsed = Catalogue::from_json(json.as_bytes());
        println!("R24_POSITIONAL {what:<28} accepted={}", parsed.is_ok());
        if parsed.is_ok() {
            accepted.push(what);
        } else {
            let why = parsed.err().unwrap_or_default();
            assert!(
                why.contains(REMEDY),
                "the refusal carries a remedy: {what}: {why}"
            );
        }
    }
    println!("R24_POSITIONAL_ACCEPTED={accepted:?}");
    assert!(
        accepted.is_empty(),
        "🔴 `req/316` L-01: `serde`'s derive reads a struct as a sequence as well as a map, so \
         these parse and the slots are decided by field order in a Rust struct the operator \
         approving this file cannot see. The audit measured the first of them reaching the real \
         read road (`A23_POSITIONAL_ROAD ok=true tool_reads=1`). `deny_unknown_fields` exists \
         because a misspelt member name is a declaration nobody made; a spelling with no member \
         names is the same argument: {accepted:?}"
    );
}

/// The control: the object spelling of the same declarations is unchanged.
#[test]
fn the_object_spelling_of_the_same_declarations_still_parses() {
    let cas = r#"{ "$cas_read": { "doc://": { "by_tool": "doc.read" } } }"#;
    let restore = format!(r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }} }}"#);
    for (what, json) in [("$cas_read", cas.to_string()), ("restore", restore)] {
        let parsed = Catalogue::from_json(json.as_bytes());
        println!("R24_POSITIONAL_CONTROL {what} accepted={}", parsed.is_ok());
        assert!(
            parsed.is_ok(),
            "refusing the array spelling must not refuse the object spelling: {what}: {:?}",
            parsed.err()
        );
    }
}

// ---------------------------------------------------------------------------
// L-02 — the sets are drawn by bytes, and one axis of that is closed
// ---------------------------------------------------------------------------

/// 🔴 `req/316` L-02: a tool name with whitespace at an edge is refused wherever it is spelled.
///
/// The audit's own reproduction is the first row: `"notes.restore "` as a `$cas_read` face against
/// `notes.restore` as an inverse. The other three are the sibling sweep of the same question —
/// `req/38` §227 ruling 2 — because a name is spelled in four places and an axis closed in one of
/// them is what `req/312` M-02 already was.
#[test]
fn a_tool_name_with_whitespace_at_an_edge_is_refused_in_every_position() {
    let padded = format!("{RESTORE_TOOL} ");
    let places = [
        (
            "$cas_read by_tool",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}" }},
                     "$cas_read": {{ "doc://": {{ "by_tool": "{padded}" }} }} }}"#
            ),
        ),
        (
            "restores key",
            format!(r#"{{ "{padded}": {{ "restored_by": "{RESTORE_TOOL}" }} }}"#),
        ),
        (
            "restored_by",
            format!(r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{padded}" }} }}"#),
        ),
        (
            "read_by.by_tool",
            format!(
                r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}",
                     "read_by": {{ "by_tool": "{padded}",
                                   "arguments": {{ "uri": {{ "forward": "uri" }} }},
                                   "identity": ["doc:", {{ "answer": "/id" }}] }} }} }}"#
            ),
        ),
    ];
    let mut accepted: Vec<&str> = Vec::new();
    for (place, json) in &places {
        let parsed = Catalogue::from_json(json.as_bytes());
        println!("R24_EDGE_WS {place:<20} accepted={}", parsed.is_ok());
        if parsed.is_ok() {
            accepted.push(place);
        } else {
            let why = parsed.err().unwrap_or_default();
            assert!(
                why.contains("whitespace") && why.contains(REMEDY),
                "the refusal names the fault and a remedy: {place}: {why}"
            );
        }
    }
    println!("R24_EDGE_WS_ACCEPTED={accepted:?}");
    assert!(
        accepted.is_empty(),
        "🔴 `req/316` L-02: the soundness sets are compared by bytes, so `{padded:?}` and \
         `{RESTORE_TOOL:?}` are two names to the gate and one name to the reader — and the first \
         one walks past the gate that refuses the second: {accepted:?}"
    );
}

/// The control: a name with an interior space, and the ordinary names, are unchanged.
#[test]
fn a_tool_name_without_edge_whitespace_is_unchanged() {
    for (what, name) in [("ordinary", "notes.get"), ("interior space", "notes get")] {
        let json = format!(
            r#"{{ "{WRITE_TOOL}": {{ "restored_by": "{RESTORE_TOOL}",
                 "read_by": {{ "by_tool": "{name}",
                               "arguments": {{ "uri": {{ "forward": "uri" }} }},
                               "identity": ["doc:", {{ "answer": "/id" }}] }},
                 "arguments": {{ "uri": {{ "forward": "uri" }}, "contents": "prior_contents_utf8" }} }} }}"#
        );
        let parsed = Catalogue::from_json(json.as_bytes());
        println!("R24_EDGE_WS_CONTROL {what} accepted={}", parsed.is_ok());
        assert!(
            parsed.is_ok(),
            "the gate is about an edge a reader cannot see, not about the character: {what}: {:?}",
            parsed.err()
        );
    }
}

/// 🔴 The **residue**, measured rather than described: canonical equivalence is still open.
///
/// `req/38` §224 ruling 2 put the widening of these sets to Unicode-equivalence behind a queued
/// lane (it needs a normalisation table this crate does not carry, which is a direct dependency and
/// a `Cargo.lock` change). The audit's second spelling — U+212B ANGSTROM SIGN against U+00C5 — is
/// still accepted, and this arm says so out loud so that the day a lane closes it, this file and
/// `docs/LIMITS.md` move together instead of the page quietly becoming false.
#[test]
fn canonically_equivalent_spellings_of_one_name_are_still_two_names() {
    let effect = "notes.wr\u{212b}ite";
    let face = "notes.wr\u{c5}ite";
    let json = format!(
        r#"{{ "{effect}": {{ "restored_by": "{RESTORE_TOOL}" }},
             "$cas_read": {{ "doc://": {{ "by_tool": "{face}" }} }} }}"#
    );
    let parsed = Catalogue::from_json(json.as_bytes());
    println!(
        "R24_EQUIV_RESIDUE accepted={} (U+212B effect vs U+00C5 face)",
        parsed.is_ok()
    );
    assert!(
        parsed.is_ok(),
        "this is a **declared residue**, not a repair: if a lane closes it, `docs/LIMITS.md`'s \
         sentence about which axis is closed has to move in the same commit, and this arm is what \
         makes that happen: {:?}",
        parsed.err()
    );
}
