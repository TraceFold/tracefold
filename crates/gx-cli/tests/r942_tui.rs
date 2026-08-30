// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `req/942` — the terminal face's gates (g1..g10) and its adversarial probes (P1..P8).
//!
//! Everything here reads a **buffer**, not a picture. `ratatui`'s in-memory backend keeps the cells
//! a frame produced, so a terminal face can be checked more sharply than a browser one: symbol,
//! foreground, background and modifier are all readable per cell, and no screenshot is involved.
//!
//! # The three things this file is careful about
//!
//! 1. **Every gate is fired at least once in the red direction.** A gate that has never refused
//!    anything has not earned the right to say green (`req/942` §14-3). `plant_*` are the positive
//!    controls: they build a source tree with the defect in it and require the scanner to name the
//!    file, the line and the text.
//! 2. **Source scans skip comment lines.** A gate that fires on prose describing the gate teaches
//!    people to write worse prose, and grep on a codebase that documents itself is polluted by its
//!    own explanations.
//! 3. **`UNTESTABLE` is not `FAIL`.** Where a fact cannot be measured in this process it is printed
//!    as such and left out of the pass/fail counts rather than folded into the failing side.

#![cfg(feature = "tui")]

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gx_cli::tui::acts::{self, Act, View};
use gx_cli::tui::layout::{
    self, Priority, Recoverable, RegionRole, LAYOUT_ROLES, LEDGER_ADDRESS, LEDGER_COLUMNS,
    LEDGER_PAGE_KEYS, NO_ADDRESS_PHRASE, REGIONS,
};
use gx_cli::tui::renderer::{self, Tier};
use gx_cli::tui::tokens;
use gx_cli::tui::wire::{self, Coverage, Nothing, Screen, NOTHING_COVERAGE};

// ---------------------------------------------------------------------------------------------
// A server made of canned answers, and a record of what it was actually asked.
// ---------------------------------------------------------------------------------------------

const HEALTHZ: &str = r#"{"status":"ok","engine_version":"gx-engine 0.1.0","ledger_agrees":true,"journal_rows":0,"status_reason":null}"#;

/// Two rows chosen so that four of the six kinds of nothing come off the wire rather than out of a
/// fixture's imagination: `verdict` is `null` on the second row (measured, not knowable),
/// `created_at` is **missing** from it (never carried), `enforced` is `false` there, and
/// `journal_rows` above is `0`.
const TRANSFORMATIONS: &str = r#"{"items":[
{"transformation":"gx1:t3sto0000000001","state":"Committed","verdict":"Admit","enforced":true,"created_at":"2026-08-30T09:00:00Z","actor":"agent-a","scope":"src/lib.rs","inverse_status":"Escrowed","rollback":null,"superseded_by":null},
{"transformation":"gx1:t3sto0000000002","state":"Draft","verdict":null,"enforced":false,"actor":"agent-b","scope":"README.md","inverse_status":null,"rollback":null,"superseded_by":null}
],"next_cursor":null}"#;

const CANDIDATES: &str = r#"{"items":[],"next_cursor":null}"#;
const ESCALATIONS: &str = r#"{"items":[],"next_cursor":null}"#;

struct Fixture {
    base_url: String,
    seen: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
}

impl Fixture {
    fn start() -> Self {
        Self::spawn(false)
    }

    /// A server that answers, and refuses. `/v1/healthz` still answers `200` because it sits
    /// outside the Bearer guard by design; the other three answer `401` with a body that has no
    /// `items`. This is the shape a stale token produces, and it is the one that caught this face
    /// drawing `0` where the honest mark is `?`.
    fn start_refusing() -> Self {
        Self::spawn(true)
    }

    fn spawn(refuse: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        listener.set_nonblocking(true).expect("non-blocking");
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_seen, thread_stop) = (Arc::clone(&seen), Arc::clone(&stop));
        std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve_one(stream, &thread_seen, refuse),
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
                }
            }
        });
        Self {
            base_url,
            seen,
            stop,
        }
    }

    fn read(&self) -> Screen {
        Screen::read(&self.base_url, None)
    }

    /// Every request line this server received, so the membrane can be measured at run time and not
    /// only by reading the source.
    fn methods(&self) -> Vec<String> {
        self.seen
            .lock()
            .expect("fixture lock")
            .iter()
            .filter_map(|line| line.split_whitespace().next().map(str::to_string))
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn serve_one(mut stream: TcpStream, seen: &Arc<Mutex<Vec<String>>>, refuse: bool) {
    let mut raw = [0u8; 2048];
    let read = stream.read(&mut raw).unwrap_or(0);
    let head = String::from_utf8_lossy(&raw[..read]).to_string();
    let request_line = head.lines().next().unwrap_or_default().to_string();
    let path = request_line.split_whitespace().nth(1).unwrap_or_default();
    let guarded = path != "/v1/healthz";
    let (status, body) = if refuse && guarded {
        (401, r#"{"title":"unauthorized","gx_code":"UNAUTHORIZED"}"#)
    } else {
        (
            200,
            match path {
                "/v1/healthz" => HEALTHZ,
                "/v1/transformations" => TRANSFORMATIONS,
                "/v1/candidates" => CANDIDATES,
                "/v1/escalations" => ESCALATIONS,
                _ => "{}",
            },
        )
    };
    seen.lock().expect("fixture lock").push(request_line);
    let answer = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(answer.as_bytes());
}

/// A port nothing is listening on: the negative control's whole apparatus.
fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    let address = listener.local_addr().expect("local addr");
    drop(listener);
    format!("http://{address}")
}

// ---------------------------------------------------------------------------------------------
// The source scanner the layering gates are built on.
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Finding {
    file: String,
    line: usize,
    text: String,
    needle: &'static str,
    depth: u8,
}

/// How far below its own layer a name reaches. Three is a raw value, two a token type, one a role.
/// Reporting the depth is what makes "this is a layering breach" a measurement and not an opinion.
const RAW_VALUE: u8 = 3;
const TOKEN_TYPE: u8 = 2;

fn tui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} is not readable: {e}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    files
}

/// A comment line is prose. Scanning it counts the explanation of a rule as a breach of it.
fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
}

fn scan(dir: &Path, exempt: &[&str], needles: &[(&'static str, u8)]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for path in rust_files(dir) {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if exempt.contains(&name.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("source is readable");
        for (index, line) in text.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            for (needle, depth) in needles {
                if line.contains(needle) {
                    findings.push(Finding {
                        file: name.clone(),
                        line: index + 1,
                        text: line.trim().to_string(),
                        needle,
                        depth: *depth,
                    });
                }
            }
        }
    }
    findings
}

/// The names a raw value wears (g1).
const RAW_VALUE_NEEDLES: [(&str, u8); 3] = [
    ("Color::", RAW_VALUE),
    ("Style::", RAW_VALUE),
    ("Modifier::", RAW_VALUE),
];

/// The names the placement layer wears, and the medium itself (g5).
///
/// 🔴 `ratatui` is in the list on purpose. `req/942` §11-4 names five placement types; naming only
/// those would let a module bind itself to the medium in any other way and still pass — which the
/// first draft of this face did, by calling `ratatui::init()` from the module above the seam. The
/// gate is the stronger statement: **one file names the medium**.
const PLACEMENT_NEEDLES: [(&str, u8); 6] = [
    ("Constraint", TOKEN_TYPE),
    ("Rect", TOKEN_TYPE),
    ("Direction", TOKEN_TYPE),
    ("Layout", TOKEN_TYPE),
    ("ratatui::layout", TOKEN_TYPE),
    ("ratatui", TOKEN_TYPE),
];

/// The names a token wears when a component reaches past its role for it (g2).
const TOKEN_NEEDLES: [(&str, u8); 4] = [
    ("REGIONS", TOKEN_TYPE),
    ("LEDGER_COLUMNS", TOKEN_TYPE),
    ("min_rows", TOKEN_TYPE),
    ("Priority::", TOKEN_TYPE),
];

/// The methods that would make this face something other than a reader (g7).
const WRITING_METHODS: [&str; 4] = ["POST", "PUT", "PATCH", "DELETE"];

// ---------------------------------------------------------------------------------------------
// g1, g2, g5, g6 — the layering gates, and the plant that proves they can refuse.
// ---------------------------------------------------------------------------------------------

#[test]
fn g1_no_raw_value_is_named_outside_the_renderer() {
    let findings = scan(&tui_dir(), &["renderer.rs"], &RAW_VALUE_NEEDLES);
    println!("G1_FINDINGS={findings:?}");
    assert!(
        findings.is_empty(),
        "🔴 g1: a colour or a style is a value, and only the seam may spell one. \
         Everything above it names a role. {findings:?}"
    );
}

#[test]
fn g2_no_component_reaches_past_its_role_for_a_token() {
    let findings = scan(&tui_dir(), &["renderer.rs", "layout.rs"], &TOKEN_NEEDLES);
    println!("G2_FINDINGS={findings:?}");
    assert!(
        findings.is_empty(),
        "🔴 g2: a component names a role and asks for a plan. Reading a declaration's own numbers \
         is two layers down. {findings:?}"
    );
}

#[test]
fn g5_no_placement_type_is_named_outside_the_renderer() {
    let findings = scan(&tui_dir(), &["renderer.rs"], &PLACEMENT_NEEDLES);
    println!("G5_FINDINGS={findings:?}");
    assert!(
        findings.is_empty(),
        "🔴 g5 (`req/942` §11-4): placement lives behind the seam. A screen that spells a \
         rectangle cannot say what it dropped. {findings:?}"
    );
}

#[test]
fn g6_every_region_name_spelled_in_this_face_is_declared() {
    let mut spelled: BTreeSet<String> = BTreeSet::new();
    for path in rust_files(&tui_dir()) {
        let text = std::fs::read_to_string(&path).expect("source is readable");
        for line in text.lines() {
            if is_comment(line) {
                continue;
            }
            let mut rest = line;
            while let Some(at) = rest.find("\"region.") {
                rest = &rest[at + 1..];
                if let Some(end) = rest.find('"') {
                    spelled.insert(rest[..end].to_string());
                }
            }
        }
    }
    println!("G6_SPELLED={spelled:?}");
    let undeclared: Vec<&String> = spelled
        .iter()
        .filter(|name| !LAYOUT_ROLES.contains(&name.as_str()))
        .collect();
    assert!(
        undeclared.is_empty(),
        "🔴 g6: {undeclared:?} are region names this face spells and `LAYOUT_ROLES` does not declare"
    );
    assert_eq!(
        spelled.len(),
        LAYOUT_ROLES.len(),
        "🔴 g6: every declared role should be spelled somewhere; spelled {spelled:?} against \
         declared {LAYOUT_ROLES:?}"
    );
}

/// 🔴 **P3, the positive control.** A gate that has never refused anything is not green; it is
/// silent. This writes a source tree with both defects in it and requires the scanners to come back
/// with the file, the line, the text and the depth.
#[test]
fn p3_plant_the_defects_and_watch_both_gates_fire() {
    let planted = Path::new(env!("CARGO_TARGET_TMPDIR")).join("r942_plant");
    let _ = std::fs::remove_dir_all(&planted);
    std::fs::create_dir_all(&planted).expect("temp dir");
    let source = "// a comment naming Color::Rgb and Constraint must NOT be counted\n\
                  fn planted() {\n\
                  \x20   let style = Color::Rgb(255, 0, 170);\n\
                  \x20   let split = Constraint::Percentage(70);\n\
                  }\n";
    std::fs::write(planted.join("component.rs"), source).expect("plant is written");

    let raw = scan(&planted, &[], &RAW_VALUE_NEEDLES);
    let placement = scan(&planted, &[], &PLACEMENT_NEEDLES);
    println!("P3_RAW={raw:?}");
    println!("P3_PLACEMENT={placement:?}");

    assert_eq!(
        raw.len(),
        1,
        "🔴 g1 did not fire on a planted colour: {raw:?}"
    );
    assert_eq!(raw[0].file, "component.rs");
    assert_eq!(raw[0].line, 3, "the line number has to be the planted one");
    assert!(
        raw[0].text.contains("Color::Rgb(255, 0, 170)"),
        "the finding has to quote what it found: {:?}",
        raw[0].text
    );
    assert_eq!(raw[0].depth, RAW_VALUE);
    assert_eq!(
        raw[0].needle, "Color::",
        "the finding has to say which rule it broke, not only that one was broken"
    );

    assert_eq!(
        placement.len(),
        1,
        "🔴 g5 did not fire on a planted constraint: {placement:?}"
    );
    assert_eq!(placement[0].line, 4);
    assert!(placement[0].text.contains("Constraint::Percentage(70)"));
    assert_eq!(placement[0].depth, TOKEN_TYPE);
    assert_eq!(placement[0].needle, "Constraint");

    // 🔴 The negative half of the same control: the comment on line 1 names both and is not
    // counted. Without this the gate would be a prose detector.
    assert!(
        raw.iter().chain(placement.iter()).all(|f| f.line != 1),
        "a comment was counted as a breach: {raw:?} {placement:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// g3, g4, g10 — the declarations themselves.
// ---------------------------------------------------------------------------------------------

#[test]
fn g3_intent_to_role_is_total_and_injective() {
    let roles: BTreeSet<&str> = REGIONS.iter().map(|r| r.role.name()).collect();
    let intents: BTreeSet<&str> = REGIONS.iter().map(|r| r.intent.sentence()).collect();
    println!("G3_ROLES={roles:?} G3_INTENTS={intents:?}");
    assert_eq!(
        roles.len(),
        REGIONS.len(),
        "🔴 g3: two regions share a role, so a reader cannot tell them apart"
    );
    assert_eq!(
        intents.len(),
        REGIONS.len(),
        "🔴 g3: two intents collapse onto one look, which is the failure the injectivity \
         requirement exists to catch"
    );
    for role in [
        RegionRole::Subject,
        RegionRole::Apparatus,
        RegionRole::Provenance,
        RegionRole::Disclosure,
    ] {
        assert!(
            roles.contains(role.name()),
            "🔴 g3: {} is not declared, so the map is not total",
            role.name()
        );
    }
}

#[test]
fn g4_the_six_words_for_nothing_each_have_exactly_one_declared_cell() {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut unreachable = Vec::new();
    for (nothing, coverage) in NOTHING_COVERAGE {
        assert!(
            seen.insert(nothing.word()),
            "🔴 g4: {} is declared twice",
            nothing.word()
        );
        if let Coverage::Unreachable(why) = coverage {
            unreachable.push((nothing.word(), why));
        }
    }
    println!("G4_DECLARED={} G4_UNREACHABLE={unreachable:?}", seen.len());
    assert_eq!(
        seen.len(),
        Nothing::ALL.len(),
        "🔴 g4: the grid has an undeclared cell. An empty cell and an out-of-reach cell are \
         different values and neither may be left blank"
    );
    // 🔴 The marks have to be different from each other before any of this means anything.
    let marks: BTreeSet<&str> = Nothing::ALL.into_iter().map(Nothing::mark).collect();
    assert_eq!(
        marks.len(),
        Nothing::ALL.len(),
        "🔴 two of the six kinds of nothing are drawn the same, which is the collapse the six \
         words exist to prevent: {marks:?}"
    );
}

#[test]
fn g10_nothing_unrecoverable_sits_below_the_top_priority() {
    for region in REGIONS {
        if region.recoverable == Recoverable::Nowhere {
            assert_eq!(
                region.priority,
                Priority::One,
                "🔴 g10 (`req/942` §19-5-3): {} carries facts with no address, so dropping it \
                 destroys them. It cannot be below the top priority",
                region.role.name()
            );
        }
        println!(
            "G10 {} priority={:?} recoverable={:?}",
            region.role.name(),
            region.priority,
            region.recoverable
        );
    }
}

/// 🔴 The positive control for g10: a declaration with the defect in it has to be refused by the
/// same predicate the real table passes. Written as a predicate over a value so that the check is
/// the one the gate runs, not a second one that resembles it.
#[test]
fn p3b_plant_a_region_whose_facts_have_no_address_below_the_top_priority() {
    fn ok(priority: Priority, recoverable: Recoverable) -> bool {
        recoverable != Recoverable::Nowhere || priority == Priority::One
    }
    assert!(!ok(Priority::Two, Recoverable::Nowhere), "g10 did not fire");
    assert!(ok(Priority::One, Recoverable::Nowhere));
    assert!(ok(Priority::Three, Recoverable::Route("GET /v1/healthz")));
    for region in REGIONS {
        assert!(
            ok(region.priority, region.recoverable),
            "{} fails the predicate the plant proved can fail",
            region.role.name()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// g7 / P7 — the membrane, measured twice.
// ---------------------------------------------------------------------------------------------

#[test]
fn g7_no_writing_method_and_no_engine_internals_are_named_in_this_face() {
    let needles: Vec<(&'static str, u8)> = WRITING_METHODS
        .iter()
        .map(|method| (*method, RAW_VALUE))
        .chain([("gx_canon::", RAW_VALUE), ("gx_gate::", RAW_VALUE)])
        .collect();
    let findings = scan(&tui_dir(), &[], &needles);
    println!("G7_FINDINGS={findings:?}");
    assert!(
        findings.is_empty(),
        "🔴 g7: this face reads. A writing method or a road into the engine's own crates in \
         `src/tui/` is a hole in the membrane, and that is heavier than any drawing defect. \
         {findings:?}"
    );
}

/// 🔴 **Rule 2 held over a denominator that does not currently reach here.**
///
/// `probes/doubt/tests/m6_surface_doubt.rs::the_clock_and_the_entropy_source_are_each_read_once`
/// requires `SystemTime::now(` to appear exactly once in this crate, in `clock.rs`. Its
/// `cli_sources()` reads `crates/gx-cli/src` with `read_dir` and **does not recurse**, so
/// `src/tui/` — the first subdirectory under that path — is outside the count. That is a gap in
/// the floor that this lane's change is the first to expose, and it is reported rather than
/// repaired here (the instrument belongs to another lane; `req/942_artifacts/
/// build_lane_report.md` raises it).
///
/// What is repaired here is this face's own half: the rule is asserted directly over `src/tui/`, so
/// a second clock in this module is red now rather than after the denominator is widened.
#[test]
fn rule_2_holds_inside_this_face_even_though_the_shared_probe_does_not_reach_it() {
    let findings = scan(
        &tui_dir(),
        &[],
        &[
            ("SystemTime::now(", RAW_VALUE),
            ("RandomState::new(", RAW_VALUE),
        ],
    );
    println!("RULE2_TUI_FINDINGS={findings:?}");
    assert!(
        findings.is_empty(),
        "🔴 Rule 2: the binary learns the time in `clock.rs` and nowhere else. Two clocks in one \
         process are two answers to \"when\". {findings:?}"
    );
}

#[test]
fn p7_the_fixture_server_was_asked_with_nothing_but_get() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let methods = fixture.methods();
    println!("P7_METHODS={methods:?}");
    assert_eq!(
        methods.len(),
        4,
        "four routes were declared and this many were asked for: {methods:?}"
    );
    assert!(
        methods.iter().all(|method| method == "GET"),
        "🔴 P7: the source scan says there is no writing method and the wire has to agree. \
         {methods:?}"
    );
    assert!(
        screen.transformations.status == Some(200),
        "the fixture did not answer: {:?}",
        screen.transformations
    );
}

// ---------------------------------------------------------------------------------------------
// P1 — the negative control. A face that cannot tell "no engine" from "no rows" is a face that
// lies for free.
// ---------------------------------------------------------------------------------------------

#[test]
fn p1_a_dead_engine_draws_unknown_and_not_an_empty_table() {
    let fixture = Fixture::start();
    let live = renderer::render_to_buffer(&fixture.read(), 100, 24, Tier::Mono, false);
    let dead_screen = Screen::read(&closed_port_url(), None);
    let dead = renderer::render_to_buffer(&dead_screen, 100, 24, Tier::Mono, false);

    let live_text = renderer::buffer_text(&live);
    let dead_text = renderer::buffer_text(&dead);
    let live_unknown = count(&live_text, Nothing::Unknown.mark());
    let dead_unknown = count(&dead_text, Nothing::Unknown.mark());
    let differing = live_text
        .chars()
        .zip(dead_text.chars())
        .filter(|(a, b)| a != b)
        .count();
    println!(
        "P1_UNKNOWN_LIVE={live_unknown} P1_UNKNOWN_DEAD={dead_unknown} P1_DIFFERING_CELLS={differing}"
    );
    println!("--- dead ---\n{dead_text}");

    assert!(
        dead_screen.readings().iter().all(|r| r.status.is_none()),
        "the negative control reached something: {dead_screen:?}"
    );
    assert!(
        dead_unknown > live_unknown,
        "🔴 P1: with no engine the face has to say it does not know. unknown marks went \
         {live_unknown} -> {dead_unknown}"
    );
    assert!(
        differing >= 64,
        "🔴 P1: the two frames are nearly identical, so the face is not distinguishing \
         'no engine' from 'engine with nothing in it'. differing cells: {differing}"
    );
    // 🔴 And the sharper half: an engine that is there and holds nothing says `0`, which is a
    // different claim from `?`, and both are different from a blank cell.
    assert!(
        live_text.contains(Nothing::Zero.mark()),
        "the live frame should carry a zero (journal_rows is 0 in the fixture):\n{live_text}"
    );
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// 🔴 **P9 — a refusal is not an empty list.** The third negative control, and the only one that
/// was written *after* being caught rather than before.
///
/// A `401` is an answer: it has a status line, it has a body, and the body has no `items`. The
/// first build of this face asked "is the item list empty?" and drew `0` — telling a reader there
/// are no records when the truth is that this process was not allowed to see them. `zero` collapsed
/// into `unknown`, by the face, in the product whose first principle is that those are different.
///
/// Found by an unplanned restart on the machine, not by this suite: the token went stale, the
/// provenance line honestly read `status 200/401/401/401`, and every cell of the table read `0`.
#[test]
fn p9_a_refused_route_draws_unknown_and_not_zero() {
    let refusing = Fixture::start_refusing();
    let screen = refusing.read();
    let text = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        120,
        24,
        Tier::Mono,
        false,
    ));
    println!("--- 401 ---\n{text}");
    println!(
        "P9_STATUSES={:?}",
        screen
            .readings()
            .iter()
            .map(|r| r.status)
            .collect::<Vec<_>>()
    );

    assert_eq!(
        screen.healthz.status,
        Some(200),
        "healthz sits outside the guard by design; if the fixture refused it too, this probe is \
         measuring a different situation"
    );
    assert_eq!(screen.transformations.status, Some(401));
    assert!(
        screen.transformations.body.is_some(),
        "a refusal has a body -- that is exactly what makes it look like an empty list"
    );
    assert!(
        screen.transformations.items().is_empty(),
        "and the body has no items, which is the trap"
    );

    // 🔴 The assertion is scoped to the row of the **refused** route, and the first draft was not.
    // It forbade a bare `0` anywhere on the frame and went red on `journal_rows 0` -- which comes
    // from `/v1/healthz`, which answered `200`, and is a **true** zero. A probe that cannot tell a
    // real zero from a false one is measuring the wrong thing, which is the same defect it was
    // written to catch, one layer up.
    let lines: Vec<&str> = text.lines().collect();
    let header_at = lines
        .iter()
        .position(|line| line.trim_start().starts_with("transformation"))
        .expect("the subject table draws its header");
    let row = lines[header_at + 1];
    println!("P9_ROW={row:?}");

    assert!(
        row.contains(Nothing::Unknown.mark()),
        "🔴 P9: the row for a refused route has to say the answer is not knowable. It reads \
         {row:?}:\n{text}"
    );
    assert!(
        !row.split_whitespace()
            .any(|cell| cell == Nothing::Zero.mark()),
        "🔴 P9: a refused route is being drawn as `{}` (zero records). It is `{}`: there are \
         records and this process may not see them. Row: {row:?}\n{text}",
        Nothing::Zero.mark(),
        Nothing::Unknown.mark()
    );
    // 🔴 The other half, so this probe cannot be satisfied by removing `zero` from the vocabulary:
    // a genuine zero, on the route that did answer, still reads `0`.
    assert!(
        flat(&text).contains("journal_rows 0"),
        "🔴 P9: `/v1/healthz` answered 200 with `journal_rows: 0`, and a real zero has to survive \
         this repair:\n{text}"
    );
    // And the provenance still carries the codes one by one rather than `all 200`.
    let measured = renderer::measured(&screen);
    assert!(
        measured.statuses.contains("401"),
        "the provenance has to name the code: {:?}",
        measured.statuses
    );
}

/// Row breaks and cell padding are placement, not text.
///
/// 🔴 A phrase that has to appear on the screen can be word-wrapped across two rows, and a search
/// over the raw buffer would then report it missing — the gate would be measuring the width of the
/// terminal rather than the presence of the sentence. Wrapping breaks at spaces and padding is
/// spaces, so flattening the rows into one line and collapsing runs of spaces recovers exactly the
/// text the composer wrote.
fn flat(text: &str) -> String {
    text.replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------------------------
// P2 — the six words, in all four tiers.
// ---------------------------------------------------------------------------------------------

#[test]
fn p2_the_six_marks_stay_pairwise_distinct_in_every_tier() {
    for tier in Tier::ALL {
        let buffer = renderer::marks_buffer(tier);
        let mut rows: Vec<String> = Vec::new();
        for (index, _) in Nothing::ALL.iter().enumerate() {
            let y = index as u16;
            let mut row = String::new();
            for x in 0..8u16 {
                let cell = &buffer[(x, y)];
                // 🔴 On `mono` the comparison is the symbol alone. That is the whole point: a
                // meaning carried by colour is a meaning that does not survive this tier.
                if tier == Tier::Mono {
                    row.push_str(cell.symbol());
                } else {
                    row.push_str(&format!("{}/{:?}/{:?}", cell.symbol(), cell.fg, cell.bg));
                }
            }
            rows.push(row);
        }
        let distinct: BTreeSet<&String> = rows.iter().collect();
        println!("P2_TIER={} DISTINCT={}", tier.name(), distinct.len());
        assert_eq!(
            distinct.len(),
            Nothing::ALL.len(),
            "🔴 P2: two of the six kinds of nothing are indistinguishable in tier {}: {rows:?}",
            tier.name()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// P4 / AC-7 — degradation, and the disclosure of it.
// ---------------------------------------------------------------------------------------------

#[test]
fn p4_at_forty_by_ten_the_dropped_region_is_named_on_screen() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let measured = renderer::measured(&screen);
    let plan = layout::resolve(40, 10, &measured, false);
    let text = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        40,
        10,
        Tier::Mono,
        false,
    ));
    println!("P4_DROPPED={:?}", plan.dropped);
    println!("--- 40x10 ---\n{text}");

    assert!(
        !plan.dropped.is_empty(),
        "🔴 P4: forty by ten cannot hold everything; a plan that drops nothing is not measuring"
    );
    let one_line = flat(&text);
    for role in &plan.dropped {
        assert!(
            one_line.contains(role.short()),
            "🔴 P4: {} was dropped and the screen does not say so",
            role.name()
        );
    }
    // 🔴 The other direction, which catches the opposite lie: a region that is drawn must not be
    // announced as dropped.
    for (role, _) in &plan.rows {
        assert!(
            !plan.dropped.contains(role),
            "{} is both drawn and dropped",
            role.name()
        );
    }
    assert!(
        text.contains(&plan.dropped.len().to_string()),
        "the count of dropped regions is not on the screen:\n{text}"
    );
}

// ---------------------------------------------------------------------------------------------
// P5 — the labels are the wire's own words.
// ---------------------------------------------------------------------------------------------

#[test]
fn p5_every_column_label_drawn_is_a_key_the_wire_carried() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let mut wire_keys: BTreeSet<String> = BTreeSet::new();
    for item in screen.transformations.items() {
        if let Some(map) = item.as_object() {
            wire_keys.extend(map.keys().cloned());
        }
    }
    if let Some(map) = screen
        .transformations
        .body
        .as_ref()
        .and_then(|body| body.as_object())
    {
        wire_keys.extend(map.keys().cloned());
    }
    let measured = renderer::measured(&screen);
    let plan = layout::resolve(200, 24, &measured, false);
    let drawn: Vec<&str> = plan.columns.iter().map(|c| c.key).collect();
    println!("P5_DRAWN={drawn:?} P5_WIRE_KEYS={wire_keys:?}");
    let invented: Vec<&&str> = drawn
        .iter()
        .filter(|key| !wire_keys.contains(**key))
        .collect();
    assert!(
        invented.is_empty(),
        "🔴 P5: {invented:?} are words this face made up. The label a reader sees is the key the \
         engine used, or it is a second vocabulary for the same fact"
    );
    assert_eq!(
        drawn.len(),
        LEDGER_COLUMNS.len(),
        "two hundred columns wide should carry every declared column: {drawn:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// P6 — the codepoint budget. The terminal's version of the tofu defect.
// ---------------------------------------------------------------------------------------------

/// 🔴 The budget is a claim about **this face's own vocabulary**, not about the engine's data.
/// Substituting a character inside a wire value would be falsifying the record to satisfy a
/// drawing rule, which is the wrong trade in this product. So the offender set is "outside the
/// budget **and** not present in the bytes the wire sent" — everything the face itself chose has
/// to be in the budget, and everything the engine said is drawn as the engine said it.
#[test]
fn p6_every_codepoint_the_face_itself_chose_is_inside_the_declared_budget() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let from_wire: BTreeSet<char> = screen
        .readings()
        .iter()
        .filter_map(|reading| reading.body.as_ref())
        .flat_map(|body| body.to_string().chars().collect::<Vec<char>>())
        .collect();
    let mut offenders: Vec<(char, u32)> = Vec::new();
    for (width, height) in [(40u16, 10u16), (80, 24), (200, 40), (40, 6)] {
        let text = renderer::buffer_text(&renderer::render_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
        ));
        for character in text.chars() {
            if character == '\n' {
                continue;
            }
            if !(' '..='~').contains(&character) && !from_wire.contains(&character) {
                offenders.push((character, character as u32));
            }
        }
    }
    println!(
        "P6_OFFENDERS={offenders:?} P6_WIRE_CHARSET={}",
        from_wire.len()
    );
    assert!(
        offenders.is_empty(),
        "🔴 P6 (`req/942` §12-2): the budget is U+0020..=U+007E. A terminal draws a codepoint its \
         font is missing as a box, and the reader reads a box as 'this program is broken' — which \
         is the worst possible reading of the mark that means 'measured, and not knowable'. \
         {offenders:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// P8 / g9 — the fold, and the address the disclosure spells.
// ---------------------------------------------------------------------------------------------

#[test]
fn p8_when_the_provenance_cannot_be_a_region_it_is_folded_and_marked() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let measured = renderer::measured(&screen);

    // 🔴 `req/942` §19-5 asked for this at 40x10. At 40x10 this build still holds the provenance as
    // its own region -- dropping the apparatus is enough -- so the fold is measured at the size
    // where the mechanism actually fires, and both sizes are asserted rather than one being
    // quietly swapped for the other. The deviation is written down in
    // `req/942_artifacts/build_lane_report.md` rather than being absorbed here.
    let ten = layout::resolve(40, 10, &measured, false);
    assert!(
        !ten.provenance_folded,
        "at 40x10 the provenance still fits as a region: {ten:?}"
    );

    let plan = layout::resolve(40, 6, &measured, false);
    let text = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        40,
        6,
        Tier::Mono,
        false,
    ));
    // 🔴 The fold does not save rows -- it costs them, because saying the four facts takes more
    // room than a one-row region did. That is the correct trade and it is asserted rather than
    // hidden: what the fold buys is that the facts survive, not that the screen fits.
    assert!(
        plan.truncated,
        "at forty by six the floor does not fit and the plan has to say so: {plan:?}"
    );
    assert!(
        flat(&text).starts_with('!') || flat(&text).contains("! "),
        "🔴 a clipped screen has to admit it on the line that exists to say what is missing:\n{text}"
    );
    let one_line = flat(&text);
    println!("P8_PLAN_FOLDED={}", plan.provenance_folded);
    println!("--- 40x6 ---\n{text}");
    assert!(
        plan.provenance_folded,
        "🔴 P8: at forty by eight the provenance has no region and the fold is the only way its \
         four facts survive: {plan:?}"
    );
    assert!(
        one_line.contains(NO_ADDRESS_PHRASE),
        "🔴 P8: the folded facts have to be marked `{NO_ADDRESS_PHRASE}`. Without it a reader \
         takes them for facts with an address, which they are not:\n{text}"
    );
    // 🔴 The whole folded sentence, not four substrings. Asserting `"4"` and `"0"` separately
    // would pass on a screen that happens to contain a digit somewhere, which is not a
    // measurement of anything.
    assert!(
        one_line.contains(&measured.folded()),
        "🔴 P8: the folded provenance reads {:?} and the screen does not carry it:\n{text}",
        measured.folded()
    );
    assert!(
        !plan
            .rows
            .iter()
            .any(|(role, _)| *role == RegionRole::Provenance),
        "the provenance is folded and still has a region of its own: {plan:?}"
    );
}

#[test]
fn g9_the_disclosure_spells_an_address_that_answers_with_what_was_dropped() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let measured = renderer::measured(&screen);
    let plan = layout::resolve(80, 24, &measured, true);
    println!("G9_DISCLOSURE={}", plan.disclosure);
    println!("G9_DROPPED_FIELDS={:?}", plan.dropped_fields);

    assert!(
        !plan.dropped_fields.is_empty(),
        "eighty columns cannot hold ten of them; a plan that drops no field is not measuring"
    );
    assert!(
        plan.disclosure.contains(LEDGER_ADDRESS),
        "the disclosure does not spell its address: {}",
        plan.disclosure
    );
    // 🔴 `req/942` §19-3: only eight of the wire's fields come back from an id, so the id spelling
    // would be a lie for the rest. The route is the address, and this is the assertion that stops
    // the wrong spelling from coming back.
    assert!(
        !plan.disclosure.contains("gx show"),
        "🔴 g9: `gx show <id> --all` does not answer with these fields. {}",
        plan.disclosure
    );
    let answered: BTreeSet<&str> = LEDGER_COLUMNS
        .iter()
        .map(|column| column.key)
        .chain(LEDGER_PAGE_KEYS)
        .collect();
    for field in &plan.dropped_fields {
        assert!(
            answered.contains(field),
            "🔴 g9: {field:?} is named as dropped and {LEDGER_ADDRESS} does not answer with it"
        );
    }
    assert!(
        plan.disclosure
            .contains(&plan.dropped_fields.len().to_string()),
        "the count is not in the line: {}",
        plan.disclosure
    );
}

/// 🔴 The positive control for g9. Without it the check above is "the string we wrote contains the
/// string we wrote", which is not a measurement of anything.
///
/// The predicate is the one the gate runs: an address answers for a field when the route named is
/// one this face reads and the route's declared key set holds the field. `gx show <id>` fails it
/// for six of the eleven, which is exactly the wrong spelling `req/942` §19-3 caught.
#[test]
fn p3c_plant_the_wrong_address_in_the_disclosure_and_watch_g9_refuse_it() {
    fn answers(address: &str, field: &str) -> bool {
        if address != LEDGER_ADDRESS {
            return false;
        }
        LEDGER_COLUMNS
            .iter()
            .map(|column| column.key)
            .chain(LEDGER_PAGE_KEYS)
            .any(|key| key == field)
    }
    // The five an id really does answer for are still not answered for *by an id spelling*, because
    // the spelling names no route this face reads.
    assert!(!answers("gx show <id> --all", "state"), "g9 did not fire");
    assert!(
        !answers("GET /v1/candidates", "inverse_status"),
        "g9 did not fire"
    );
    assert!(!answers(LEDGER_ADDRESS, "a_field_the_wire_never_had"));
    assert!(answers(LEDGER_ADDRESS, "next_cursor"));

    let fixture = Fixture::start();
    let screen = fixture.read();
    let measured = renderer::measured(&screen);
    let plan = layout::resolve(80, 24, &measured, true);
    for field in &plan.dropped_fields {
        assert!(
            answers(LEDGER_ADDRESS, field),
            "{field:?} is named as dropped and the address does not answer for it"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// AC-1, AC-2 — the rows and the apparatus, on a frame.
// ---------------------------------------------------------------------------------------------

#[test]
fn ac1_and_ac2_the_frame_carries_a_real_id_the_engine_version_and_four_measurements() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let text = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        120,
        24,
        Tier::Mono,
        false,
    ));
    println!("--- 120x24 ---\n{text}");

    let id = screen.transformations.items()[0]["transformation"]
        .as_str()
        .expect("the fixture row has an id")
        .to_string();
    // 🔴 **A declared departure from AC-1 as written, and the reason.** An id is `gx1:` and a
    // digest; a real one is far wider than any column an eighty-cell screen can spare, so "the id
    // on the screen is the id the wire sent, character for character" cannot hold in a fixed grid
    // for any real row. What holds instead, and what is asserted here, is the strongest thing that
    // can: the drawn cell is a **prefix** of the wire id, character for character, and the cut is
    // **marked**. The unabbreviated value is at the route the disclosure line spells.
    // `req/942_artifacts/build_lane_report.md` carries this as a deviation rather than a pass.
    let expected = if id.chars().count() > 16 {
        format!("{}~", id.chars().take(15).collect::<String>())
    } else {
        id.clone()
    };
    assert!(
        flat(&text).contains(&expected),
        "🔴 AC-1: the frame does not carry {expected:?}, which is what the wire's {id:?} draws as \
         in a sixteen-cell column:\n{text}"
    );
    assert!(
        text.contains("Committed"),
        "🔴 a value that fits its column is drawn whole and unmarked:\n{text}"
    );
    assert!(
        text.contains("engine_version") && text.contains("gx-engine 0.1.0"),
        "🔴 AC-2: the page header carries the engine's own version:\n{text}"
    );
    for reading in screen.readings() {
        assert!(!reading.route.is_empty());
        assert!(!reading.read_at.is_empty(), "a reading with no read time");
        assert!(reading.status.is_some(), "a reading with no status");
    }
    // 🔴 The fourth mark: the second fixture row has no verdict, and a face that rounded it to one
    // of three would be inventing a judgement the engine did not make.
    assert!(
        text.contains(Nothing::Unknown.mark()),
        "🔴 the row whose verdict is null has to draw the fourth mark:\n{text}"
    );
    assert!(
        text.contains(Nothing::Absent.mark()),
        "🔴 the row with no `created_at` key at all has to draw a different mark from the one \
         whose value is null:\n{text}"
    );
    assert!(
        text.contains(Nothing::False.mark()),
        "🔴 `enforced: false` is not `unknown`:\n{text}"
    );
}

// ---------------------------------------------------------------------------------------------
// AC-10 — the measurement the browser face never made.
// ---------------------------------------------------------------------------------------------

#[test]
fn ac10_first_frame_and_redraw_are_measured_and_declared() {
    let fixture = Fixture::start();
    let screen = fixture.read();

    let started = Instant::now();
    let first = renderer::render_to_buffer(&screen, 120, 40, Tier::Truecolor, false);
    let first_us = started.elapsed().as_micros();

    let started = Instant::now();
    let rounds = 50;
    for _ in 0..rounds {
        let _ = renderer::render_to_buffer(&screen, 120, 40, Tier::Truecolor, false);
    }
    let redraw_us = started.elapsed().as_micros() / rounds;

    println!(
        "AC10_FIRST_FRAME_US={first_us} AC10_REDRAW_US={redraw_us} AC10_CELLS={}",
        120 * 40
    );
    assert!(
        first.area.width == 120 && first.area.height == 40,
        "the buffer is not the size that was asked for"
    );
    // 🔴 No threshold. `req/942` §15 refuses a target written before the first measurement,
    // because a number chosen in advance is a number the next lane will bend the measurement to
    // meet. What the acceptance criterion asks for is that the figure exists and is printed.
    assert!(
        redraw_us > 0,
        "the clock did not move at all, which is not a measurement"
    );
}

// ---------------------------------------------------------------------------------------------
// The wire vocabulary itself.
// ---------------------------------------------------------------------------------------------

#[test]
fn absent_and_unknown_and_false_and_zero_are_four_different_answers() {
    let object: serde_json::Value = serde_json::from_str(
        r#"{"present":"x","null_valued":null,"no":false,"count":0,"empty":[],"yes":true}"#,
    )
    .expect("fixture parses");
    assert_eq!(
        wire::cell(&object, "missing"),
        wire::Cell::Nothing(Nothing::Absent)
    );
    assert_eq!(
        wire::cell(&object, "null_valued"),
        wire::Cell::Nothing(Nothing::Unknown)
    );
    assert_eq!(
        wire::cell(&object, "no"),
        wire::Cell::Nothing(Nothing::False)
    );
    assert_eq!(
        wire::cell(&object, "count"),
        wire::Cell::Nothing(Nothing::Zero)
    );
    assert_eq!(
        wire::cell(&object, "empty"),
        wire::Cell::Nothing(Nothing::Zero)
    );
    assert_eq!(
        wire::cell(&object, "present"),
        wire::Cell::Value("x".to_string())
    );
    assert_eq!(
        wire::cell(&object, "yes"),
        wire::Cell::Value("yes".to_string())
    );
}

#[test]
fn the_routes_this_face_reads_are_the_four_the_browser_face_reads() {
    println!("ROUTES={:?}", wire::ROUTES);
    assert_eq!(wire::ROUTES.len(), 4);
    for route in wire::ROUTES {
        assert!(route.starts_with("/v1/"), "{route} is not a v1 route");
    }
}

// =============================================================================================
// `req/38` SS965 convert rows (a)..(d). Four cracks the audit named, and the gates that keep
// each one shut.
// =============================================================================================

// ---------------------------------------------------------------------------------------------
// (a) g11 / P10 — the membrane, over the whole of the engine rather than two of its crates.
// ---------------------------------------------------------------------------------------------

/// 🔴 **The gate g7 should have been.** g7 named `gx_canon::` and `gx_gate::` by hand, and the crack
/// the audit found was `gx_api::` — a crate nobody had thought to list. A hand-written list of the
/// roads that must stay shut is a list that goes stale the day a crate is added, so this one is
/// **derived**: every sibling of this crate in `crates/` is a road, and the only name this face may
/// spell is its own.
fn engine_crate_needles() -> Vec<(&'static str, u8)> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ is this crate's parent");
    let mut needles: Vec<(&'static str, u8)> = Vec::new();
    for entry in std::fs::read_dir(workspace).expect("crates/ is readable") {
        let entry = entry.expect("a directory entry");
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().replace('-', "_");
        if name == "gx_cli" {
            continue;
        }
        // Leaked so that the derived needle has the lifetime the shared scanner takes. A test
        // process that ends is the whole of the deallocation this needs.
        needles.push((Box::leak(format!("{name}::").into_boxed_str()), RAW_VALUE));
    }
    needles.sort_unstable();
    needles
}

#[test]
fn g11_no_engine_crate_is_named_in_this_face() {
    let needles = engine_crate_needles();
    println!(
        "G11_DENOMINATOR={} {:?}",
        needles.len(),
        needles.iter().map(|(n, _)| *n).collect::<Vec<_>>()
    );
    assert!(
        needles.len() > 5,
        "the denominator is derived from crates/ and cannot be this small: {needles:?}"
    );
    let findings = scan(&tui_dir(), &[], &needles);
    println!("G11_FINDINGS={findings:?}");
    assert!(
        findings.is_empty(),
        "🔴 g11 (`req/38` SS965 row (a)): this face reads a server over HTTP. A road into any of \
         the engine's crates makes it something else, and 'except for one date formatter' is the \
         shape every hole in a membrane has on the day it is made. {findings:?}"
    );
}

/// The positive control for g11: the exact line the audit found, put back in a plant.
#[test]
fn p10a_plant_the_engine_import_the_audit_found_and_watch_g11_fire() {
    let planted = Path::new(env!("CARGO_TARGET_TMPDIR")).join("r942_plant_engine");
    let _ = std::fs::remove_dir_all(&planted);
    std::fs::create_dir_all(&planted).expect("temp dir");
    std::fs::write(
        planted.join("wire.rs"),
        "// a comment naming gx_api::rfc3339 must NOT be counted\n\
         fn planted() {\n\
         \x20   let read_at = gx_api::rfc3339::of(crate::clock::now());\n\
         }\n",
    )
    .expect("plant is written");
    let findings = scan(&planted, &[], &engine_crate_needles());
    println!("P10A_FINDINGS={findings:?}");
    assert_eq!(
        findings.len(),
        1,
        "g11 did not fire on the plant: {findings:?}"
    );
    assert_eq!(findings[0].line, 3, "the comment on line 1 must not count");
    assert_eq!(findings[0].needle, "gx_api::");
}

/// 🔴 **The cost of closing the crack, measured rather than promised.** Row (a) buys the membrane
/// with a second implementation of one date format, and two implementations of a format drift. So
/// the two are run over the same instants and required to agree — the epoch, an instant before it,
/// both sides of a leap day, a leap second's neighbourhood, and the ends of the range an `i64`
/// nanosecond clock can carry.
#[test]
fn p10_the_faces_own_rfc3339_agrees_with_the_api_crates() {
    let instants: [i64; 12] = [
        0,
        1,
        -1,
        -86_400_000_000_000,
        1_756_543_200_123_456_789,
        951_782_400_000_000_000, // 2000-02-29, the leap day a wrong rule loses
        4_107_542_400_000_000_000, // 2100-03-01, on the far side of a century that is not a leap year
        1_234_567_890_000_000_000,
        -2_208_988_800_000_000_000,
        i64::MAX,
        i64::MIN + 1,
        i64::MIN,
    ];
    let mut disagreements: Vec<(i64, String, String)> = Vec::new();
    for nanos in instants {
        let mine = wire::rfc3339(nanos);
        let theirs = gx_api::rfc3339::of(gx_core::Timestamp(nanos));
        println!("P10 {nanos} -> {mine}");
        if mine != theirs {
            disagreements.push((nanos, mine, theirs));
        }
    }
    assert!(
        disagreements.is_empty(),
        "🔴 P10: this face's date and the API crate's date are the same fact spelled twice, and \
         they disagree: {disagreements:?}"
    );
    // The shape itself, so that a pair of formatters that agree on a wrong shape is still red.
    let sample = wire::rfc3339(1_756_543_200_123_456_789);
    assert!(
        sample.ends_with('Z') && sample.len() == "2026-08-30T00:00:00.000000000Z".len(),
        "the wire's date is RFC 3339 in UTC to the nanosecond: {sample}"
    );
}

// ---------------------------------------------------------------------------------------------
// (b) g13 / g14 — the paint ladder.
// ---------------------------------------------------------------------------------------------

/// A colour **value** written into the drawing code: `Color::` with a number in it. A colour read
/// out of an [`Ink`](gx_cli::tui::tokens::Ink) has no digits on the line, which is exactly the
/// difference this gate exists to measure.
fn colour_literals(dir: &Path) -> Vec<Finding> {
    scan(dir, &[], &[("Color::", RAW_VALUE)])
        .into_iter()
        .filter(|finding| finding.text.chars().any(|c| c.is_ascii_digit()))
        .collect()
}

#[test]
fn g13_no_colour_value_is_spelled_outside_the_token_table() {
    let findings = colour_literals(&tui_dir());
    println!("G13_FINDINGS={findings:?}");
    assert!(
        findings.is_empty(),
        "🔴 g13 (`req/38` SS965 row (b)): the renderer binds the medium and does not decide the \
         value. A hue with a number in it belongs in `tokens.rs`, where a role can reach it and a \
         reader can find every colour this face has. {findings:?}"
    );
    // The other half: the values do exist, and they exist there. A gate that passes because the
    // face draws nothing at all would be silent rather than green.
    let table =
        std::fs::read_to_string(tui_dir().join("tokens.rs")).expect("tokens.rs is readable");
    assert!(
        table.contains("214, 188, 106"),
        "the accent this face has always drawn is not in the token table"
    );
}

#[test]
fn p11_plant_a_hardcoded_hue_and_watch_g13_fire_without_catching_the_honest_line() {
    let planted = Path::new(env!("CARGO_TARGET_TMPDIR")).join("r942_plant_hue");
    let _ = std::fs::remove_dir_all(&planted);
    std::fs::create_dir_all(&planted).expect("temp dir");
    std::fs::write(
        planted.join("renderer.rs"),
        "// a comment naming Color::Rgb(1, 2, 3) must NOT be counted\n\
         fn planted() {\n\
         \x20   let hardcoded = Color::Rgb(214, 188, 106);\n\
         \x20   let resolved = Color::Rgb(red, green, blue);\n\
         \x20   let indexed = Color::Indexed(index);\n\
         }\n",
    )
    .expect("plant is written");
    let findings = colour_literals(&planted);
    println!("P11_FINDINGS={findings:?}");
    assert_eq!(
        findings.len(),
        1,
        "🔴 g13 has to fire on the value and stay quiet on the two lines that only type one: \
         {findings:?}"
    );
    assert_eq!(findings[0].line, 3);
    assert!(findings[0].text.contains("214, 188, 106"));
}

#[test]
fn g14_every_paint_role_resolves_to_a_value_and_every_token_keeps_a_role() {
    let mut names: BTreeSet<&str> = BTreeSet::new();
    let mut used: BTreeSet<&str> = BTreeSet::new();
    for role in tokens::ROLES {
        assert!(
            names.insert(role.name()),
            "🔴 g14: {} is declared twice",
            role.name()
        );
        let token = role.token();
        used.insert(token.name());
        assert!(
            tokens::TOKENS.contains(&token),
            "🔴 g14: {} names token {}, which the table does not declare",
            role.name(),
            token.name()
        );
        let coloured = tokens::ink(role, Tier::Truecolor).has_colour();
        for tier in Tier::ALL {
            let ink = tokens::ink(role, tier);
            let spellings = u8::from(ink.rgb.is_some())
                + u8::from(ink.c256.is_some())
                + u8::from(ink.c16.is_some());
            println!("G14 {} {} {ink:?}", role.name(), tier.name());
            assert!(
                spellings <= 1,
                "🔴 g14: {} in tier {} spells its colour twice; three spellings are one decision",
                role.name(),
                tier.name()
            );
            match tier {
                // 🔴 The whole of what `mono` means, asserted rather than described. Every mark in
                // this face is told from every other by its symbol (P2), so a tier with no hue
                // loses emphasis and never loses a meaning.
                Tier::Mono => assert!(
                    !ink.has_colour(),
                    "🔴 g14: {} spells a colour on mono",
                    role.name()
                ),
                Tier::Truecolor => assert_eq!(ink.rgb.is_some(), coloured),
                Tier::Ansi256 => assert_eq!(ink.c256.is_some(), coloured),
                Tier::Ansi16 => assert_eq!(ink.c16.is_some(), coloured),
            }
        }
    }
    let orphans: Vec<&str> = tokens::TOKENS
        .iter()
        .map(|token| token.name())
        .filter(|name| !used.contains(name))
        .collect();
    println!("G14_ROLES={} G14_TOKENS_USED={used:?}", names.len());
    assert!(
        orphans.is_empty(),
        "🔴 g14: {orphans:?} are values no role resolves to, and a value nobody reaches is a value \
         nobody maintains"
    );
}

// ---------------------------------------------------------------------------------------------
// (c) g12 — the acts, and the requirement that a declared one is a real one.
// ---------------------------------------------------------------------------------------------

/// 🔴 **The gate the audit asked for by name.** The build before this one declared nothing and bound
/// three keys, of which two did the same thing. Here every act in the declaration is fired through
/// the one reducer and has to move something: half-wired is the defect, and half-wired is invisible
/// to a test that only checks the acts it remembers to name.
#[test]
fn g12_every_declared_act_moves_the_state() {
    const ROWS: usize = 3;
    let starts = [
        View {
            selected: 0,
            open: false,
        },
        View {
            selected: 1,
            open: false,
        },
        View {
            selected: 2,
            open: true,
        },
        View {
            selected: 0,
            open: true,
        },
    ];
    let mut inert: Vec<&str> = Vec::new();
    for act in acts::ACTS {
        let moved = starts.iter().any(|start| {
            let (next, signal) = acts::apply(start, act, ROWS);
            next != *start || signal != acts::Signal::None
        });
        println!(
            "G12 {} effect={:?} keys={:?} moved={moved}",
            act.name(),
            act.effect(),
            act.keys()
        );
        if !moved {
            inert.push(act.name());
        }
    }
    assert!(
        inert.is_empty(),
        "🔴 g12 (`req/38` SS965 row (c)): {inert:?} are declared and do nothing. An act that is \
         announced and inert reads to a person as a broken program rather than as an unbound key"
    );
    assert_eq!(acts::ACTS.len(), 8, "the declared set is eight acts");

    // The reducer's own arithmetic, in the direction each act claims.
    let list = View {
        selected: 1,
        open: false,
    };
    assert_eq!(acts::apply(&list, Act::Prev, ROWS).0.selected, 0);
    assert_eq!(acts::apply(&list, Act::Next, ROWS).0.selected, 2);
    assert_eq!(acts::apply(&list, Act::Last, ROWS).0.selected, ROWS - 1);
    assert_eq!(acts::apply(&list, Act::First, ROWS).0.selected, 0);
    assert!(acts::apply(&list, Act::Open, ROWS).0.open);
    assert!(
        !acts::apply(
            &View {
                selected: 0,
                open: true
            },
            Act::Close,
            ROWS
        )
        .0
        .open
    );
    assert_eq!(acts::apply(&list, Act::Read, ROWS).1, acts::Signal::Read);
    assert_eq!(acts::apply(&list, Act::Leave, ROWS).1, acts::Signal::Leave);
    // The ends, and an empty list: a selection may not walk off either edge, and a list with
    // nothing in it has no record to attend to.
    assert_eq!(acts::apply(&View::default(), Act::Prev, ROWS).0.selected, 0);
    assert_eq!(
        acts::apply(
            &View {
                selected: ROWS - 1,
                open: false
            },
            Act::Next,
            ROWS
        )
        .0
        .selected,
        ROWS - 1
    );
    assert_eq!(acts::apply(&list, Act::Last, 0).0.selected, 0);
}

#[test]
fn g12b_every_act_has_keys_and_no_key_reaches_two_acts() {
    let mut bound: BTreeSet<&str> = BTreeSet::new();
    for act in acts::ACTS {
        assert!(!act.keys().is_empty(), "{} binds no key", act.name());
        for key in act.keys() {
            assert!(
                bound.insert(key),
                "🔴 g12b: {key} reaches two acts, so which one a reader gets is decided by the \
                 order of a table"
            );
            assert_eq!(
                acts::for_key(key),
                Some(act),
                "🔴 g12b: {key} is declared by {} and the one road from a key does not return it",
                act.name()
            );
        }
    }
    println!("G12B_KEYS={bound:?}");
    assert_eq!(acts::for_key("z"), None, "an unbound key binds nothing");
    assert_eq!(acts::for_key("Q"), None, "the binding is case-exact");
}

/// 🔴 The failure mode this repository is named after: a document that describes a program that
/// does not exist. `gx tui --help` lists the keys, so the help and the declaration are required to
/// say the same thing, measured over the source rather than trusted.
#[test]
fn g12c_the_help_text_names_every_declared_act() {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"))
        .expect("main.rs is readable");
    let flattened = flat(&source);
    let mut missing: Vec<String> = Vec::new();
    for act in acts::ACTS {
        let line = format!("{} {}", act.keys()[0], act.intent());
        if !flattened.contains(&line) {
            missing.push(line);
        }
    }
    println!("G12C_CHECKED={}", acts::ACTS.len());
    assert!(
        missing.is_empty(),
        "🔴 g12c: the help text does not spell {missing:?}. A key a person is not told about is a \
         capability that does not exist for them"
    );
}

/// The acts, on the screen rather than in the reducer: attending to a record marks it, and opening
/// one shows the members the grid has no column for.
#[test]
fn p12_the_view_reaches_the_frame() {
    let fixture = Fixture::start();
    let screen = fixture.read();

    // 🔴 `inverse_status` has no column at forty cells wide -- the disclosure says so -- and the
    // opened record has to carry it, or `open` is a bool that moves and shows nothing.
    let closed = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        40,
        24,
        Tier::Mono,
        false,
        &View {
            selected: 0,
            open: false,
        },
    ));
    let opened = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        40,
        24,
        Tier::Mono,
        false,
        &View {
            selected: 0,
            open: true,
        },
    ));
    println!("--- closed 40x24 ---\n{closed}\n--- opened 40x24 ---\n{opened}");
    assert!(
        !flat(&closed).contains("inverse_status Escrowed"),
        "the grid at forty cells has no column for it; the closed frame must not carry it"
    );
    assert!(
        flat(&opened).contains("inverse_status Escrowed"),
        "🔴 P12: `act.open` is declared as 'see everything this record carries' and the frame does \
         not carry it:\n{opened}"
    );
    assert!(
        flat(&opened).contains("10 of 10 members"),
        "the opened record has to say how much of itself is on the screen:\n{opened}"
    );
    // 🔴 And when it does not fit, the count says so rather than the screen quietly showing five.
    let cramped = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        40,
        14,
        Tier::Mono,
        false,
        &View {
            selected: 0,
            open: true,
        },
    ));
    println!("--- opened 40x14 ---\n{cramped}");
    let drawn = flat(&cramped)
        .split(" of 10 members")
        .next()
        .and_then(|head| head.split(' ').next_back().map(str::to_string))
        .expect("the note names how many members were drawn");
    let drawn: usize = drawn.parse().expect("the count is a number");
    assert!(
        drawn < 10 && flat(&cramped).contains(&format!("{drawn} of 10 members")),
        "🔴 P12: forty by fourteen cannot hold ten members and the note has to name the cut: \
         {drawn}\n{cramped}"
    );
    // 🔴 The note is wrapped rather than clipped: the line that says what was cut must not itself
    // be the thing that is cut. Its last word is the key that closes the record, and a clipped
    // note loses it.
    assert!(
        flat(&cramped).contains(&format!("close: {}", Act::Close.keys()[0])),
        "🔴 P12: the disclosure of the cut was itself cut:\n{cramped}"
    );

    // The attention mark: the attended row is drawn differently from the row above it, and by a
    // modifier rather than a hue, so it survives `mono`.
    let buffer = renderer::render_view_to_buffer(
        &screen,
        120,
        24,
        Tier::Mono,
        false,
        &View {
            selected: 1,
            open: false,
        },
    );
    let text = renderer::buffer_text(&buffer);
    let rows: Vec<&str> = text.lines().collect();
    // 🔴 By its `scope` and not by its id: the id column is sixteen cells wide and every id in the
    // fixture shares its first fifteen characters, so a search for one would find the row above it
    // — a probe that reads a truncated cell is measuring the padding.
    let first = rows
        .iter()
        .position(|row| row.contains("src/lib.rs"))
        .expect("the first record is drawn") as u16;
    let attended = first + 1;
    let mark = buffer[(0, attended)].modifier;
    println!("P12_ATTENDED_ROW={attended} MODIFIER={mark:?}");
    assert!(
        mark.contains(ratatui::style::Modifier::REVERSED),
        "🔴 P12: the attended record is not marked on a monochrome terminal"
    );
    assert!(
        !buffer[(0, first)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED),
        "every row is marked, which marks nothing"
    );
}

// ---------------------------------------------------------------------------------------------
// (d) g15 — the three verdicts, and the fourth mark the module documentation promised.
// ---------------------------------------------------------------------------------------------

#[test]
fn g15_the_fourth_mark_is_a_fourth_mark_and_never_one_of_the_three() {
    let kinds: Vec<String> = wire::VERDICT_KINDS
        .into_iter()
        .map(|kind| wire::verdict(&serde_json::json!({ "verdict": kind })).mark())
        .collect();
    let fourths: Vec<String> = Nothing::ALL
        .into_iter()
        .map(|nothing| wire::VerdictMark::None(nothing).mark())
        .collect();
    println!("G15_KINDS={kinds:?} G15_FOURTH={fourths:?}");
    let distinct: BTreeSet<&String> = kinds.iter().chain(fourths.iter()).collect();
    assert_eq!(
        distinct.len(),
        kinds.len() + fourths.len(),
        "🔴 g15: a verdict and a kind of nothing are drawn the same, which is the rounding the \
         fourth mark exists to refuse: {kinds:?} {fourths:?}"
    );

    // 🔴 Which kind of nothing is preserved. "the wire never carried a verdict" and "the wire
    // carried one and it was not knowable" are two facts, and the fourth mark carries both without
    // becoming a fourth verdict.
    let null = wire::verdict(&serde_json::json!({ "verdict": serde_json::Value::Null }));
    let missing = wire::verdict(&serde_json::json!({ "state": "Draft" }));
    assert_eq!(null, wire::VerdictMark::None(Nothing::Unknown));
    assert_eq!(missing, wire::VerdictMark::None(Nothing::Absent));
    assert_ne!(null.mark(), missing.mark());
    assert!(!null.is_kind() && !missing.is_kind());

    // 🔴 A word this face does not know is drawn as it arrived. An engine that grows a fourth kind
    // should make this face look out of date; it must not make it say `Deny`.
    let stranger = wire::verdict(&serde_json::json!({ "verdict": "Admitted" }));
    assert_eq!(stranger, wire::VerdictMark::Other("Admitted".to_string()));
    assert_eq!(stranger.mark(), "Admitted");
    assert!(!stranger.is_kind());
    assert!(
        !wire::VERDICT_KINDS.contains(&stranger.mark().as_str()),
        "the vocabulary swallowed a word it does not hold"
    );
}

/// The same claim, on the frame: the fixture's second record carries no verdict, and its verdict
/// cell has to be the fourth mark rather than one of the three.
#[test]
fn p13_the_screen_draws_the_fourth_mark_for_the_record_that_has_no_verdict() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let text = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        200,
        24,
        Tier::Mono,
        false,
    ));
    println!("--- 200x24 ---\n{text}");
    let column = LEDGER_COLUMNS
        .iter()
        .position(|column| column.key == wire::VERDICT_KEY)
        .expect("the verdict is a declared column");
    let cell_of = |needle: &str| -> String {
        text.lines()
            .find(|line| line.contains(needle))
            .map(|line| {
                line.split_whitespace()
                    .nth(column)
                    .unwrap_or_default()
                    .to_string()
            })
            .unwrap_or_else(|| panic!("{needle} is not on the screen:\n{text}"))
    };
    // The rows are named by their `scope`; the id column is cut at sixteen cells and every fixture
    // id agrees on its first fifteen characters.
    let admitted = cell_of("src/lib.rs");
    let no_verdict = cell_of("README.md");
    println!("P13_ADMITTED={admitted:?} P13_NO_VERDICT={no_verdict:?}");
    assert_eq!(admitted, "Admit", "the engine's own word, unchanged");
    assert_eq!(
        no_verdict,
        Nothing::Unknown.mark(),
        "🔴 P13: the record carrying `verdict: null` is drawn with one of the three, which is the \
         rounding `super::tui`'s own documentation says this face does not do"
    );
}
