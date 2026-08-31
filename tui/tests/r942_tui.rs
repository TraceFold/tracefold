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

// 🔴 **The `#![cfg(feature = "tui")]` that stood here is gone with the crate this suite left**
// (#188/#189, 2026-08-31). It existed because the face was a module of `gx-cli` behind an optional
// feature, and a test file compiled with the feature off would have named modules that did not
// exist. In `gx-tui` the face is the whole package: there is no configuration of this crate in
// which these gates do not apply, and a `cfg` that can never be false is a switch a reader has to
// rule out. The switch itself is not gone — it is `gx-cli`'s `tui = ["dep:gx-tui"]`, one level out.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gx_tui::tui::acts::{self, Act, View};
use gx_tui::tui::layout::{
    self, Priority, Recoverable, RegionRole, LAYOUT_ROLES, LEDGER_ADDRESS, LEDGER_COLUMNS,
    LEDGER_PAGE_KEYS, NO_ADDRESS_PHRASE, REGIONS,
};
use gx_tui::tui::renderer::{self, Tier};
use gx_tui::tui::tokens;
use gx_tui::tui::wire::{self, Coverage, Nothing, Screen, NOTHING_COVERAGE};

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

    // 🔴 **#188/#189 closes this crate's half of the gap the paragraph above reports.**
    //
    // The face used to borrow `gx-cli`'s clock (`crate::clock::now()`, a `gx_core::Timestamp`).
    // Extracting the package took that road out of the graph and put a clock *here*, in
    // `src/clock.rs` — outside `src/tui/`, exactly as `gx-cli`'s sits outside its own modules — so
    // the scan above is unchanged and still means what it meant. What would have been new is a
    // clock nobody counts: `m6_surface_doubt`'s `cli_sources()` reads `crates/gx-cli/src`, which
    // never reached `src/tui/` and reaches this package not at all.
    //
    // So the count is made here, over the crate root's own directory, and it is an **equality**
    // rather than an absence: exactly one site, in the file named for it. An absence check would
    // pass on a crate that had lost its clock as well as on one that keeps a single one.
    let crate_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let outside = scan(
        &crate_src,
        &["clock.rs"],
        &[
            ("SystemTime::now(", RAW_VALUE),
            ("RandomState::new(", RAW_VALUE),
        ],
    );
    let clock = std::fs::read_to_string(crate_src.join("clock.rs")).expect("src/clock.rs is here");
    let reads = clock
        .lines()
        .filter(|line| !is_comment(line))
        .filter(|line| line.contains("SystemTime::now("))
        .count();
    println!("RULE2_CRATE_OUTSIDE={outside:?} RULE2_CLOCK_READS={reads}");
    assert!(
        outside.is_empty(),
        "🔴 Rule 2, this crate: the clock is read in `src/clock.rs` and nowhere else. {outside:?}"
    );
    assert_eq!(
        reads, 1,
        "🔴 Rule 2, this crate: `src/clock.rs` holds exactly one read of the wall clock. \
         Nought is a crate that lost its clock and two is a crate with two answers to \"when\""
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

/// 🔴 **Amended 2026-08-31, design round 2 (`req/942`), by the ruling recorded in
/// `super::layout::REGIONS`: the apparatus declared `min_rows: 4` and drew two rows, at 46, 60, 80,
/// 100 and 120 cells wide against a live engine.** The fourth row was furniture, and it was
/// furniture in the region that is dropped first — so the row it hoarded was a row the screen went
/// looking for when it ran out. At three rows, forty by ten holds every region, and this probe's
/// original size no longer drops one.
///
/// The test is neither renamed nor deleted (a test name is a historical record). Forty by ten is
/// kept and now asserts the new fact in both directions, and the property this probe was written
/// for — *a region that is let go of is named on the screen* — is measured at forty by **eight**,
/// where the screen genuinely cannot hold the floor.
#[test]
fn p4_at_forty_by_ten_the_dropped_region_is_named_on_screen() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let measured = renderer::measured(&screen);

    // The amendment, pinned: at forty by ten nothing is dropped and every declared region is drawn.
    let roomy = layout::resolve(40, 10, &measured, false, layout::Subject::Grid);
    println!("P4_40x10_DROPPED={:?} ROWS={:?}", roomy.dropped, roomy.rows);
    println!(
        "--- 40x10 ---\n{}",
        renderer::buffer_text(&renderer::render_to_buffer(
            &screen,
            40,
            10,
            Tier::Mono,
            false
        ))
    );
    assert!(
        roomy.dropped.is_empty(),
        "🔴 P4: forty by ten held every region once the apparatus stopped hoarding a row. If it \
         drops one again, a region grew and nothing declared the growth: {:?}",
        roomy.dropped
    );
    assert_eq!(
        roomy.rows.len(),
        REGIONS.len(),
        "🔴 P4: forty by ten does not draw all four regions: {:?}",
        roomy.rows
    );

    let plan = layout::resolve(40, 8, &measured, false, layout::Subject::Grid);
    let text = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        40,
        8,
        Tier::Mono,
        false,
    ));
    println!("P4_DROPPED={:?}", plan.dropped);
    println!("--- 40x8 ---\n{text}");

    assert!(
        !plan.dropped.is_empty(),
        "🔴 P4: forty by eight cannot hold everything; a plan that drops nothing is not measuring"
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
    let plan = layout::resolve(200, 24, &measured, false, layout::Subject::Grid);
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
    let ten = layout::resolve(40, 10, &measured, false, layout::Subject::Grid);
    assert!(
        !ten.provenance_folded,
        "at 40x10 the provenance still fits as a region: {ten:?}"
    );

    let plan = layout::resolve(40, 6, &measured, false, layout::Subject::Grid);
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
    let plan = layout::resolve(80, 24, &measured, true, layout::Subject::Grid);
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
    let plan = layout::resolve(80, 24, &measured, true, layout::Subject::Grid);
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
///
/// 🔴 **#188/#189 widened the denominator by moving the face out of `crates/`.** While this suite
/// lived in `gx-cli`, its manifest directory *was* a sibling of the crates it derives from, and
/// `gx_cli` had to be skipped — the face was inside it, so naming it was naming itself. From
/// `tui/`, `crates/` is one hop up and **nothing is skipped**: `gx_cli::` is now among the roads
/// this face may not spell, which it could not have been before. The gate got stronger by being
/// moved, and that is the whole argument for the move in one line.
fn engine_crate_needles() -> Vec<(&'static str, u8)> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository root is this crate's parent")
        .join("crates");
    let mut needles: Vec<(&'static str, u8)> = Vec::new();
    for entry in std::fs::read_dir(&workspace).expect("crates/ is readable") {
        let entry = entry.expect("a directory entry");
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().replace('-', "_");
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

// 🔴 **`p10_the_faces_own_rfc3339_agrees_with_the_api_crates` is not here either** (#188/#189).
// It is a *differential* — this face's formatter against `gx_api::rfc3339::of` over twelve
// instants — so it needs both implementations in one test binary, and pulling `gx-api` and
// `gx-core` in as dev-dependencies of this package would put the engine's crates back into
// `gx-tui`'s graph on the very day the extraction took them out. A `[dev-dependencies]` edge does
// not ship, so `cargo tree -e normal` would have stayed clean and the membrane would have been
// broken somewhere the obvious measurement does not look — which is the failure shape the audit
// found the *first* time (`req/38` SS965 row (a): one date formatter, imported quietly).
//
// So it lives in `crates/gx-cli/tests/r942_tui_binding.rs`, where both crates are already
// dependencies and nothing new is admitted anywhere. The probe is unchanged and still runs on
// every `cargo test --workspace`.

/// The shape of this face's own date, which is a fact about this crate alone.
///
/// Split from the differential above it (#188/#189): the *agreement* between two implementations
/// needs both and moved to the consumer's suite; the *shape* needs only one and stays, so that a
/// pair of formatters agreeing on a wrong shape is still red from here.
#[test]
fn p10b_the_faces_own_rfc3339_has_the_shape_the_wire_promises() {
    let sample = wire::rfc3339(1_756_543_200_123_456_789);
    println!("P10B_SAMPLE={sample}");
    assert!(
        sample.ends_with('Z') && sample.len() == "2026-08-30T00:00:00.000000000Z".len(),
        "the wire's date is RFC 3339 in UTC to the nanosecond: {sample}"
    );
}

// ---------------------------------------------------------------------------------------------
// (b) g13 / g14 — the paint ladder.
// ---------------------------------------------------------------------------------------------

/// A colour **value** written into the drawing code: `Color::` with a number in it. A colour read
/// out of an [`Ink`](gx_tui::tui::tokens::Ink) has no digits on the line, which is exactly the
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

// ---------------------------------------------------------------------------------------------
// 🔴 **g12c and g23 are not here. They are in `crates/gx-cli/tests/r942_tui_binding.rs`** (#188/#189,
// 2026-08-31), and this note is left in the place they were rather than the two probes being
// quietly absent from a file that once held all of them.
//
// Both measure `gx tui --help` — text in `crates/gx-cli/src/main.rs`, written by the consumer —
// against this face's declarations. Keeping them here would have made this package's tests read
// `../crates/gx-cli/src/main.rs`: a crate whose suite fails when its consumer drifts and which
// cannot be tested in a tree that does not carry that consumer. `cargo tree` would show nothing;
// the coupling would live in the test directory, where the extraction's whole claim is invisible.
//
// The promise is made by the crate that prints the help, so the check sits there. The count did not
// move: 53 probes before, 51 here and 2 there.
// ---------------------------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------------------------
// (e) g16 and p14 — the round-2 audit's one real gap, and the header that stood over a record.
// ---------------------------------------------------------------------------------------------

/// 🔴 **The gap the round-2 audit found by name** (`req/38` SS971, INJ-B): re-point the role -> token
/// map — `mark.zero` to `thin`, say — and every one of the thirty-seven checks stayed green.
///
/// g14 is not the missing gate and was never claiming to be. It measures that the map is **total**
/// and that no declared value is an orphan, and a re-pointing breaks neither: `thin` is a declared
/// token, and `plain` keeps a role because `paint.body` still names it. What nothing measured is
/// *which* value a role takes — which is the whole of what the map decides.
///
/// Two halves, because a pin on its own records the map without saying why it is that map:
/// * **the pin** — all fourteen pairs by name, so a re-pointing is a red diff rather than a silent
///   one;
/// * **the reasons** — the four statements about nothing that the pin exists to protect. A later
///   edit has to argue with these rather than renumber them.
#[test]
fn g16_every_role_takes_the_value_it_was_declared_to_take() {
    use gx_tui::tui::tokens::{Role, Token};

    /// The map, spelled. Sorted the way `tokens::ROLES` is, so the two can be read side by side.
    ///
    /// 🔴 **Fifteenth pair added 2026-08-31 by `req/38` SS974 queue row Q4** — the ruling that gave
    /// an empty string a word of its own. The fourteen above it are untouched: a pin grows by a row
    /// when the declaration grows by a role, and the gate refusing to run until it does is the whole
    /// mechanism. This one landed red first (`DECLARED.len()` 14 against fifteen roles) and the red
    /// is what asked for this line.
    const DECLARED: [(Role, Token); 15] = [
        (Role::Head, Token::Accent),
        (Role::Quiet, Token::Thin),
        (Role::Body, Token::Plain),
        (Role::Attend, Token::Attend),
        (Role::MarkLoading, Token::Thin),
        (Role::MarkUnknown, Token::Thin),
        (Role::MarkAbsent, Token::Thin),
        (Role::MarkFalse, Token::Thin),
        (Role::MarkZero, Token::Plain),
        (Role::MarkDeleted, Token::Refuse),
        (Role::MarkEmpty, Token::Thin),
        (Role::VerdictAdmit, Token::Affirm),
        (Role::VerdictDeny, Token::Refuse),
        (Role::VerdictEscalate, Token::Accent),
        (Role::VerdictNone, Token::Thin),
    ];

    assert_eq!(
        DECLARED.len(),
        tokens::ROLES.len(),
        "🔴 g16: the pin holds {} pairs and the face declares {} roles; a role added without a pin \
         is a value nothing is watching",
        DECLARED.len(),
        tokens::ROLES.len()
    );

    let expect = |role: Role| -> Option<Token> {
        DECLARED
            .iter()
            .find(|(pinned, _)| *pinned == role)
            .map(|(_, token)| *token)
    };

    for role in tokens::ROLES {
        let taken = role.token();
        println!("G16 {} -> {}", role.name(), taken.name());
        assert_eq!(
            Some(taken),
            expect(role),
            "🔴 g16: {} resolves to {} and the pin says {:?}. If the new value is the right one, \
             move the pin in the same commit and say in the message which reading of the screen \
             changed — a value that moves without a sentence is a value nobody decided",
            role.name(),
            taken.name(),
            expect(role).map(Token::name)
        );
    }

    // The reasons. Each one is a fact about the screen, not about the table.
    assert_eq!(
        Role::MarkZero.token(),
        Role::Body.token(),
        "🔴 g16: a count of nought is a value the wire carried, and it is inked like every other \
         value the wire carried. Ink it like an absence and `0` starts reading as `nothing here`"
    );
    assert_ne!(
        Role::MarkZero.token(),
        Role::MarkAbsent.token(),
        "🔴 g16: zero and absent are told apart below the symbol layer as well as at it"
    );
    assert_ne!(
        Role::MarkDeleted.token(),
        Role::MarkAbsent.token(),
        "🔴 g16: a line that was written and struck is not a line that was never written"
    );
    // 🔴 The reason `req/38` SS974 row Q4 exists, one rung below the symbol. Repairing `cell` and
    // then giving the new mark `mark.zero`'s appearance would have moved the collapse rather than
    // removed it: two marks that differ only in their glyph are one mark to a reader scanning a
    // column.
    assert_ne!(
        Role::MarkEmpty.token(),
        Role::MarkZero.token(),
        "🔴 g16: a value that arrived with nothing in it is not a count of nought, and the two are \
         told apart in the appearance as well as in the symbol"
    );
    assert_ne!(
        Role::VerdictNone.token(),
        Role::VerdictDeny.token(),
        "🔴 g16: the worst collapse this face could make. `the engine has not answered` drawn in \
         the appearance of `the engine refused` is a verdict this face invented"
    );

    // 🔴 Negative control, because a check that cannot fail is a green that means nothing. The same
    // comparison, run against the map with the one pair INJ-B re-pointed, has to disagree — and
    // disagree about exactly one role, or it is finding something other than the injection.
    let injected: Vec<(Role, Token)> = DECLARED
        .iter()
        .map(|(role, token)| {
            if *role == Role::MarkZero {
                (*role, Token::Thin)
            } else {
                (*role, *token)
            }
        })
        .collect();
    let caught: Vec<&str> = tokens::ROLES
        .iter()
        .filter(|role| {
            injected
                .iter()
                .find(|(pinned, _)| pinned == *role)
                .map(|(_, token)| *token)
                != Some(role.token())
        })
        .map(|role| role.name())
        .collect();
    assert_eq!(
        caught,
        vec![Role::MarkZero.name()],
        "🔴 g16: the negative control did not fire. The comparison above is not comparing"
    );
}

/// 🔴 The grid's header stood over an opened record, naming columns that were not drawn.
///
/// Found on a real terminal rather than in a buffer: at 46x12 against the live engine the opened
/// record said `1 of 10 members`, and one of the two rows it had was spent on `transformation
/// verdict state` — a header for a table that was not on the screen. The reader is shown column
/// names and then key/value pairs that do not line up under them.
///
/// The check is the discriminator and not the wording: only the header carries the first two column
/// keys on one line, because a member line carries one key.
#[test]
fn p14_no_grid_header_stands_over_an_opened_record() {
    let fixture = Fixture::start();
    let screen = fixture.read();

    let frame = |open: bool| {
        renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            100,
            24,
            Tier::Mono,
            false,
            &View { selected: 0, open },
        ))
    };
    let header_rows = |text: &str| {
        text.lines()
            .filter(|line| line.contains("transformation") && line.contains("verdict"))
            .count()
    };

    let closed = frame(false);
    let opened = frame(true);
    println!("--- closed ---\n{closed}\n--- opened ---\n{opened}");

    // The control: the grid keeps its header, so this probe fails for the right reason when it
    // fails. Without it, deleting the header everywhere would pass.
    assert_eq!(
        header_rows(&closed),
        1,
        "🔴 P14: the grid lost the header that says which columns it is drawing:\n{closed}"
    );
    assert_eq!(
        header_rows(&opened),
        0,
        "🔴 P14: a column header is standing over an opened record, naming columns the frame does \
         not draw:\n{opened}"
    );

    // And the row it stopped taking went to the record. The record's own note is what says so, and
    // it is the number a reader acts on.
    let members = |text: &str| -> usize {
        flat(text)
            .split(" of 10 members")
            .next()
            .and_then(|head| head.split(' ').next_back().map(str::to_string))
            .expect("the note names how many members were drawn")
            .parse()
            .expect("the count is a number")
    };
    let cramped = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        46,
        12,
        Tier::Mono,
        false,
        &View {
            selected: 0,
            open: true,
        },
    ));
    println!("--- opened 46x12 ---\n{cramped}");
    assert_eq!(
        header_rows(&cramped),
        0,
        "🔴 P14: the size where the header cost the most still draws it:\n{cramped}"
    );
    assert!(
        members(&cramped) >= 2,
        "🔴 P14: the row the header gave back did not reach the record: {} members at 46x12\n{cramped}",
        members(&cramped)
    );
}

// ---------------------------------------------------------------------------------------------
// (f) g17 / g18 / p15 — the list's note, and the region that was hoarding rows while dropping text.
// ---------------------------------------------------------------------------------------------

/// 🔴 **g17 — every key the note spells comes out of the binding table.**
///
/// The defect this closes is not that the legend might be wrong today; it is that a legend is a
/// second place where a key is written down, and two spellings of one binding disagree the day one
/// of them is edited. `super::acts` is the declaration and `renderer::spelled` is required to be a
/// projection of it — the act's own declared name and the first of its own declared keys, nothing
/// composed by hand.
///
/// The negative control is a plausible hand-spelling (`open:Enter`), and what makes it a control is
/// that `acts::for_key` does not resolve it: this face binds the name `return`, and a legend that
/// said `Enter` would be teaching a key that does nothing.
#[test]
fn g17_every_key_the_note_spells_comes_from_the_binding_table() {
    for act in renderer::NOTE_ORDER
        .iter()
        .chain(renderer::NOTE_ORDER_EMPTY.iter())
    {
        let text = renderer::spelled(*act);
        let (name, key) = text
            .split_once(':')
            .expect("the note spells an act as name:key");
        println!("G17 {} -> {text:?}", act.name());
        assert_eq!(
            name,
            act.name().trim_start_matches("act."),
            "🔴 g17: the note invented a name for {}",
            act.name()
        );
        assert_eq!(
            key,
            act.keys()[0],
            "🔴 g17: the note spells a key that is not the act's first declared key"
        );
        assert!(
            !text.contains(' '),
            "🔴 g17: {text:?} holds a space, so `layout::wrap` can break between the act and its \
             key. A key severed from the act it produces reads as a typo, which is worse than no \
             legend at all"
        );
        assert_eq!(
            acts::for_key(key),
            Some(*act),
            "🔴 g17: the key {key:?} does not reach {} through the one binding table",
            act.name()
        );
    }

    // `act.close` is offered by neither list state: closing a record that is not open moves
    // nothing, and the opened record's own note is what names it (asserted by P12).
    assert!(!renderer::NOTE_ORDER.contains(&Act::Close));
    assert!(!renderer::NOTE_ORDER_EMPTY.contains(&Act::Close));
    assert_eq!(renderer::offered(0), renderer::NOTE_ORDER_EMPTY.as_slice());
    assert_eq!(renderer::offered(2), renderer::NOTE_ORDER.as_slice());
    for act in renderer::offered(2) {
        assert!(
            acts::ACTS.contains(act),
            "🔴 g17: the note offers {} and the declaration does not carry it",
            act.name()
        );
    }

    // The negative control, and the assertion that makes it one.
    let hand_spelled = "open:Enter";
    assert_ne!(
        renderer::spelled(Act::Open),
        hand_spelled,
        "the control has to differ from the real spelling or it measures nothing"
    );
    assert_eq!(
        acts::for_key("Enter"),
        None,
        "🔴 g17: the negative control names a key this face actually binds, so it is not a control"
    );
}

/// 🔴 **g18 — the note fits the rows it was given, and names what it folded away.**
///
/// A legend that quietly spells four of seven keys has taught the reader that there are four. So
/// every fold carries its own count and the address that holds the rest, and `g12c` is what makes
/// that address worth spelling: the help text names every declared act.
///
/// The head is a ladder for the same reason the disclosure has a long and a short form. The first
/// build of this note had one head, and at 46x12 against a live engine the head alone needed three
/// rows in the one row it had — so the line that says a record was let go of was itself cut,
/// mid-address. That is the defect P12 guards against for the opened record, reintroduced beside it.
#[test]
fn g18_the_note_fits_its_rows_and_names_the_keys_it_folded() {
    let offered = renderer::offered(2);
    let head = "record 1 of 2".to_string();
    // The ladder the renderer passes when nothing was let go of: the position, then no head at all.
    let heads = vec![head.clone(), String::new()];
    let mut by_rows: Vec<(u16, usize, usize)> = Vec::new();

    for width in [30u16, 40, 46, 60, 80, 100, 120, 200] {
        for rows in 1usize..=2 {
            let note = renderer::fold_note(&heads, offered, width, rows);
            let needed = layout::rows_needed(&note, width) as usize;
            let spelled = offered
                .iter()
                .filter(|act| note.contains(&renderer::spelled(**act)))
                .count();
            println!("G18 width={width} rows={rows} needed={needed} spelled={spelled} {note:?}");
            by_rows.push((width, rows, spelled));

            // The head is kept whenever it fits. When it does not, the rung below it drops the
            // head and not the keys — where the reader stands is also drawn by the attention mark,
            // and the keys are drawn nowhere else.
            assert!(
                note.starts_with(&head) || note.starts_with(&format!("{} keys", offered.len())),
                "🔴 g18: the fold produced a form that is on neither rung of the ladder:\n{note}"
            );
            // The floor is allowed not to fit — that is the named ceiling, and the screen is below
            // its own floor there. Anything that spells a key has to fit.
            if spelled > 0 {
                assert!(
                    needed <= rows,
                    "🔴 g18: the note spelled {spelled} keys into {rows} row(s) and needs \
                     {needed}:\n{note}"
                );
            }
            if spelled < offered.len() {
                // `more` only once there is something for it to be more than, so both spellings
                // count — what is asserted is the number and that it is a number *of keys*.
                let folded = offered.len() - spelled;
                assert!(
                    note.contains(&format!("{folded} keys"))
                        || note.contains(&format!("{folded} more keys")),
                    "🔴 g18: {folded} keys were folded away and the note does not say how \
                     many:\n{note}"
                );
                assert!(
                    note.contains(renderer::HELP_ADDRESS),
                    "🔴 g18: keys were folded away with no address for them:\n{note}"
                );
            }
        }
    }

    // Monotone in both arguments: a wider screen or a taller budget never teaches fewer keys.
    for (width, rows, spelled) in &by_rows {
        for (other_width, other_rows, other_spelled) in &by_rows {
            if other_width >= width && other_rows >= rows {
                assert!(
                    other_spelled >= spelled,
                    "🔴 g18: {other_width}x{other_rows} spells {other_spelled} keys and the \
                     smaller {width}x{rows} spells {spelled}"
                );
            }
        }
    }

    // The head ladder: when the long head does not fit, the short one is what is drawn.
    let long = format!("record 1 of 9 | +7 more rows | {LEDGER_ADDRESS}");
    let short = format!("+7 more rows | {LEDGER_ADDRESS}");
    let ladder = vec![long.clone(), short.clone()];
    assert!(
        layout::rows_needed(&renderer::note_line(&long, offered, 0), 46) as usize > 1,
        "the control is vacuous: the long head already fits one row at 46 cells"
    );
    let folded = renderer::fold_note(&ladder, offered, 46, 1);
    println!("G18_LADDER={folded:?}");
    assert!(
        folded.starts_with(&short),
        "🔴 g18: the long head did not fit and the short one was not reached:\n{folded}"
    );

    // The negative control: a fold that drops keys without counting them.
    let honest = renderer::note_line(&head, offered, 3);
    let silent = honest.replace(
        &format!(
            "{} more keys: {}",
            offered.len() - 3,
            renderer::HELP_ADDRESS
        ),
        "",
    );
    assert!(
        honest.contains("more keys") && !silent.contains("more keys"),
        "🔴 g18: the control did not change the line, so it is not measuring the count"
    );
}

/// 🔴 **p15 — the apparatus wraps its head instead of dropping it off the right edge.**
///
/// Found on a real terminal at 46x12: the region drew `engine_version 0.1.0 status ok
/// ledger_agrees` and the value of `ledger_agrees` and the whole of `journal_rows 3` were **gone**
/// — two of the engine's five facts about itself, cut with no mark and no line in the disclosure.
/// It was doing that while holding **two blank rows**, at every width measured from 46 to 120.
///
/// A face whose stated debt is disclosure cannot pay it and silently drop text off an edge, and a
/// region that hoards rows is hoarding them from the region that goes looking for rows when the
/// screen runs out — which is why the declaration went from four rows to three in the same change.
#[test]
fn p15_the_apparatus_head_is_wrapped_and_never_silently_clipped() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let pairs = [
        "engine_version gx-engine 0.1.0",
        "status ok",
        "ledger_agrees yes",
        "journal_rows 0",
    ];
    // The probe is only worth running where the head cannot fit on one row. Measured, not assumed.
    let head_width = pairs.join("  ").chars().count();
    println!("P15_HEAD_WIDTH={head_width}");

    for (width, height) in [(46u16, 24u16), (60, 24), (80, 24), (100, 24), (120, 24)] {
        let text = renderer::buffer_text(&renderer::render_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
        ));
        let one = flat(&text);
        let missing: Vec<&&str> = pairs.iter().filter(|pair| !one.contains(**pair)).collect();
        let vacuous = head_width <= width as usize;
        println!("P15 {width}x{height} missing={missing:?} one_row_would_fit={vacuous}");
        assert!(
            missing.is_empty(),
            "🔴 P15: at {width} cells the apparatus lost {missing:?} with no mark. The engine's \
             account of itself is not a place to drop text quietly:\n{text}"
        );
    }
    // And the discriminator: at the narrow end the head genuinely does not fit on one row, so the
    // frames above are passing because of the wrap and not because there was nothing to wrap.
    assert!(
        head_width > 46,
        "🔴 P15 is vacuous: the head is {head_width} cells and fits a 46-cell row without wrapping"
    );

    // The other half — a cut that cannot be avoided is **marked**, with the same trailing mark a
    // table cell is cut with, rather than performed silently.
    let narrow = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        24,
        24,
        Tier::Mono,
        false,
    ));
    println!("--- 24x24 ---\n{narrow}");
    let apparatus_rows: Vec<&str> = narrow
        .lines()
        .take_while(|line| !line.trim_start().starts_with("transformation"))
        .collect();
    let head_lost = pairs.iter().any(|pair| !flat(&narrow).contains(*pair));
    println!("P15_NARROW_HEAD_LOST={head_lost} ROWS={apparatus_rows:?}");
    if head_lost {
        assert!(
            apparatus_rows.iter().any(|line| line.contains('~')),
            "🔴 P15: the apparatus dropped part of its head at 24 cells and drew no cut mark:\n\
             {narrow}"
        );
    }
}

// =============================================================================================
// `req/38` SS974 — the three rows this lane was sent for: the subscription the round-2 audit
// named as this face's one gap against the browser (`Rust BEHIND: RT`), the queue row Q4
// collapse inside the classifier that exists to refuse collapses, and the reducer that
// disagreed with the screen about what opening means.
// =============================================================================================

use gx_tui::tui::live::{self, Link, LinkReport, Pulse};

/// A subscription report to draw with, spelled in one place so a probe does not become a second
/// declaration of what one looks like.
fn report(link: Link, events: u64, reconnects: u64) -> LinkReport {
    LinkReport {
        link,
        events,
        unreadable: 0,
        reconnects,
        // A connection that has been opened again `reconnects` times was attempted at least one
        // more time than it succeeded, which is coherent for every call site of this helper.
        attempts: reconnects + 1,
    }
}

/// An `std::io` result of the shape a socket read produces.
fn read_error(kind: std::io::ErrorKind) -> std::io::Result<usize> {
    Err(std::io::Error::new(kind, "a read window that passed"))
}

// ---------------------------------------------------------------------------------------------
// g19 / P16 — the subscription. Four states, and the one of them that must never wear `zero`.
// ---------------------------------------------------------------------------------------------

/// 🔴 **The gate the round-2 audit asked for by name.** `req/38` SS971 recorded the one place this
/// face was behind the browser one: `Rust BEHIND = RT purchase, no equivalent of the browser's g14`.
/// The browser face's version measures that the subscription's declaration is complete and that its
/// states are as many as it declares. This is the same question asked of this face's own
/// declaration. `req/38` SS988 made it five: `never` was carved out of `closed` (gate g22).
///
/// The load-bearing half is the third block. A subscription that has fallen over does not know
/// whether the ledger is moving, so it wears `unknown`; drawing `zero` there would say *nothing is
/// happening* about a question this process can no longer ask. That collapse is the one this
/// product exists to refuse, and a face that committed it in its own status line would be refuting
/// its own first principle on screen.
#[test]
fn g19_the_subscription_declares_five_states_and_closed_never_wears_zero() {
    // 1. Four states, named once each.
    let names: BTreeSet<&str> = live::LINKS.iter().map(|link| link.name()).collect();
    println!("G19_STATES={names:?}");
    assert_eq!(live::LINKS.len(), 5, "the declared set is five states");
    assert_eq!(
        names.len(),
        live::LINKS.len(),
        "🔴 g19: a state is declared twice: {names:?}"
    );

    // 2. As many marks as states, all different from each other. Five states drawn with four marks
    //    is four states with a footnote.
    let marks: BTreeSet<&str> = live::LINKS.iter().map(|link| link.mark()).collect();
    println!("G19_MARKS={marks:?}");
    assert_eq!(
        marks.len(),
        live::LINKS.len(),
        "🔴 g19: two of the five states of the connection are drawn the same: {marks:?}"
    );

    // 3. 🔴 The one that matters. `closed` is `unknown`, and no state anywhere in the map resolves
    //    to `zero`.
    assert_eq!(
        Link::Closed.nothing(),
        Some(Nothing::Unknown),
        "🔴 g19: a dropped subscription does not know what has happened since it dropped"
    );
    let zeroed: Vec<&str> = live::LINKS
        .iter()
        .filter(|link| link.nothing() == Some(Nothing::Zero))
        .map(|link| link.name())
        .collect();
    assert!(
        zeroed.is_empty(),
        "🔴 g19: {zeroed:?} would be drawn with the mark for a count of nought. `no events have \
         arrived` is a measurement and this state cannot make it: the answer is that nothing can \
         be measured, which is a different word"
    );
    assert_ne!(
        Link::Closed.mark(),
        Nothing::Zero.mark(),
        "🔴 g19: the symbol layer, checked separately from the map above it"
    );

    // 3b. 🔴 `req/38` SS988. `never` is a state of its own and it is `false` — *asked, and the
    //     answer is no* — rather than `unknown`, which is what it was folded into, or `absent`,
    //     which is `off`'s. The `zero` ban above already covers it because it sweeps the whole
    //     array; these are the arms that say which word it is instead.
    assert_eq!(
        Link::Never.nothing(),
        Some(Nothing::False),
        "🔴 g19: a connection that has never once been up is a measurement with a negative answer"
    );
    assert_ne!(
        Link::Never.nothing(),
        Link::Closed.nothing(),
        "🔴 g19: `never` and `closed` resolve to the same word, which is the collapse SS988 named"
    );
    assert_ne!(
        Link::Never.nothing(),
        Link::Off.nothing(),
        "🔴 g19: `never` wearing `off`'s mark merges a pair this map used to separate (g22)"
    );

    // 3c. The state an ended attempt lands in is a **function of the history** — of whether the
    //     stream has ever been up — and not of the reason the last attempt failed. Both arms fire
    //     without a socket, which is why it is a free function and not a branch in the worker.
    assert_eq!(live::after_attempt(0), Link::Never);
    assert_eq!(live::after_attempt(1), Link::Closed);
    assert_eq!(live::after_attempt(7), Link::Closed);
    // 🔴 An attempt is not an accomplishment. The counter this replaces was incremented on every
    //    pass of the retry loop, so an engine that had never been up reported re-openings of a
    //    connection that had never existed.
    assert_eq!(
        live::reopenings(0),
        0,
        "🔴 g19: a stream that was never up has been re-opened no times"
    );
    assert_eq!(
        live::reopenings(1),
        0,
        "🔴 g19: the first opening is not a re-opening"
    );
    assert_eq!(live::reopenings(3), 2);
    // and the report never spells a reconnect count for a state that has none.
    let never = report(Link::Never, 0, 0);
    assert!(
        !never.long().contains("reconnect") && !never.long().contains("closed after"),
        "🔴 g19: {:?} is the sentence of a connection that has been up",
        never.long()
    );

    // 4. `open` is not a kind of nothing, and the mark it therefore needs collides with none of the
    //    seven words that are.
    assert_eq!(
        Link::Open.nothing(),
        None,
        "🔴 g19: being connected is not an absence, so it does not borrow one of the words for one"
    );
    let words: BTreeSet<&str> = Nothing::ALL.into_iter().map(Nothing::mark).collect();
    assert!(
        !words.contains(live::OPEN_MARK),
        "🔴 g19: {} is already the mark for one of the kinds of nothing, so a connection that is up \
         would be drawn as an absence: {words:?}",
        live::OPEN_MARK
    );
    // The budget P6 measures over a frame, asserted here over the declaration so that a mark chosen
    // outside it is a red gate rather than a box on somebody's terminal.
    assert!(
        live::OPEN_MARK.chars().all(|c| (' '..='~').contains(&c)),
        "🔴 g19: {:?} leaves the declared codepoint budget",
        live::OPEN_MARK
    );

    // 5. The three states that **are** absences borrow the wire's vocabulary rather than spelling a
    //    second one. A parallel table of marks drifts from the first the day either is edited, and
    //    it drifts silently, because nothing compares them.
    for link in live::LINKS {
        if let Some(nothing) = link.nothing() {
            assert_eq!(
                link.mark(),
                nothing.mark(),
                "🔴 g19: {} spells its own mark instead of taking {}'s, which is a second vocabulary \
                 of absence",
                link.name(),
                nothing.word()
            );
        }
    }

    // 6. One sentence per state. A single sentence covering four states is where two of them go.
    let sentences: BTreeSet<&str> = live::LINKS.iter().map(|link| link.sentence()).collect();
    assert_eq!(
        sentences.len(),
        live::LINKS.len(),
        "🔴 g19: two states say the same sentence: {sentences:?}"
    );

    // 7. 🔴 What an event is allowed to do, as a word something can read. `apply` here would mean the
    //    stream had become a second source of truth beside the four routes.
    assert_eq!(
        live::ON_EVENT,
        "reread",
        "🔴 g19: an event says `go and look again`. Any other word here is a second thing on this \
         machine that claims to know what is true"
    );

    // 8. The wake is what notices that the debounce has expired, so it has to be shorter than it.
    assert!(
        renderer::WAKE < live::DEBOUNCE,
        "🔴 g19: the loop wakes every {:?} and the debounce is {:?}, so the debounce is really the \
         wake's period",
        renderer::WAKE,
        live::DEBOUNCE
    );

    // 9. 🔴 A read that returns nothing is not a disconnection. Measured against the live engine on
    //    `:8842`, which replays its history in one burst and then says nothing for as long as the
    //    connection is held; a classifier that read that silence as a close would report a broken
    //    engine twice a second.
    assert_eq!(live::pulse(&Ok(0)), Pulse::Ended);
    assert_eq!(live::pulse(&Ok(7)), Pulse::Bytes(7));
    for kind in [
        std::io::ErrorKind::WouldBlock,
        std::io::ErrorKind::TimedOut,
        std::io::ErrorKind::Interrupted,
    ] {
        assert_eq!(
            live::pulse(&read_error(kind)),
            Pulse::Idle,
            "🔴 g19: {kind:?} is how a read timeout arrives, and it means the window passed"
        );
    }
    assert_eq!(
        live::pulse(&read_error(std::io::ErrorKind::ConnectionReset)),
        Pulse::Ended
    );

    // 10. The route, spelled the way the wire spells it rather than the way the specification does.
    assert!(
        live::STREAM_ROUTE.starts_with("/v1/"),
        "🔴 g19: 44 §2.2 spells the stream `/stream` because it is declared inside the `/v1` nest. \
         Asking the running engine for `/stream` answers 404. {} is what the socket wants",
        live::STREAM_ROUTE
    );
}

/// The four states, on the screen rather than in the declaration — at every width this face can be
/// drawn at.
///
/// 🔴 The property is that **the mark is never the thing that falls off the edge**. A terminal cuts
/// from the right, and the provenance region gets exactly one row, so anything at the front of that
/// row survives every width. That is why the connection's mark leads the line and its counts trail
/// it: at a width where something has to go, the thing that goes is the count and not the state.
#[test]
fn g19b_the_state_of_the_connection_is_told_apart_at_every_width() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let mut cut: Vec<(u16, &str)> = Vec::new();
    for width in 20u16..=200 {
        for link in live::LINKS {
            let measured = renderer::measured_with_link(&screen, report(link, 14, 2));
            let plan =
                gx_tui::tui::layout::resolve(width, 24, &measured, false, layout::Subject::Grid);
            assert!(
                plan.provenance.starts_with(link.mark()),
                "🔴 g19b: at {width} cells the provenance line for {} is {:?}, which does not lead \
                 with the mark. The mark leading is the whole of why the state survives a narrow \
                 screen",
                link.name(),
                plan.provenance
            );
            // The ladder's contract: the line fits the row it was given, or it is the shortest rung
            // there is and the screen has nothing smaller to fall back to.
            let fits = gx_tui::tui::layout::rows_needed(&plan.provenance, width) <= 1;
            if !fits && plan.provenance != measured.bare() {
                cut.push((width, link.name()));
            }
        }
    }
    assert!(
        cut.is_empty(),
        "🔴 g19b: the provenance line was cut at a width where a shorter rung was available: \
         {cut:?}"
    );

    // And on a real buffer, through the same `draw` a live frame goes through.
    for width in [40u16, 80, 140] {
        let mut drawn: BTreeSet<String> = BTreeSet::new();
        for link in live::LINKS {
            drawn.insert(renderer::buffer_text(&renderer::render_live_to_buffer(
                &screen,
                width,
                24,
                Tier::Mono,
                false,
                &View::default(),
                report(link, 14, 2),
            )));
        }
        assert_eq!(
            drawn.len(),
            live::LINKS.len(),
            "🔴 g19b: at {width} cells two of the five states of the connection draw the same frame"
        );
    }

    // The words, at a width that carries them. Five states, five sentences on screen.
    for link in live::LINKS {
        let report = report(link, 14, 2);
        let text = renderer::buffer_text(&renderer::render_live_to_buffer(
            &screen,
            160,
            24,
            Tier::Mono,
            false,
            &View::default(),
            report,
        ));
        println!("G19B {} ---\n{text}", link.name());
        assert!(
            flat(&text).contains(&report.long()),
            "🔴 g19b: {} says {:?} and the screen does not carry it:\n{text}",
            link.name(),
            report.long()
        );
    }

    // 🔴 The negative half of the load-bearing claim, on the frame rather than in the table: a
    // closed connection draws `14 events` nowhere, because it does not know.
    let closed = flat(&renderer::buffer_text(&renderer::render_live_to_buffer(
        &screen,
        160,
        24,
        Tier::Mono,
        false,
        &View::default(),
        report(Link::Closed, 14, 2),
    )));
    assert!(
        closed.contains("closed after 14 events, 2 reconnects"),
        "🔴 g19b: the closed frame does not say what it counted before it closed: {closed}"
    );
    assert!(
        !closed.contains("| 14 events"),
        "🔴 g19b: a closed connection is reporting a live count: {closed}"
    );

    // 🔴 `req/38` SS988, on the frame. The collapse was **total**: a connection that opened once,
    // received nothing and dropped printed `closed after 0 events, 1 reconnects`, which is byte for
    // byte what an engine that was never once up printed. So the negative is that a `never` frame
    // carries neither the word `closed` nor a reconnect count nor an event count — none of the
    // three is a thing this state has measured — and the two frames differ.
    let frame = |link: Link, events: u64, reconnects: u64| {
        flat(&renderer::buffer_text(&renderer::render_live_to_buffer(
            &screen,
            160,
            24,
            Tier::Mono,
            false,
            &View::default(),
            report(link, events, reconnects),
        )))
    };
    let never = frame(Link::Never, 0, 0);
    println!("G19B_NEVER={never}");
    for banned in ["closed after", "reconnect", "0 events"] {
        assert!(
            !never.contains(banned),
            "🔴 g19b: the frame of a connection that has never been up says {banned:?}, which is a \
             claim about a connection that existed: {never}"
        );
    }
    assert_ne!(
        never,
        frame(Link::Closed, 0, 0),
        "🔴 g19b: `never` and `closed` draw the same frame at the counts that used to make them \
         byte-identical. That is the whole of SS988"
    );
}

/// 🔴 The plants. Every claim g19 makes, made again against a declaration with the defect in it, so
/// that the green above is a measurement rather than a comparison that cannot fail.
#[test]
fn p16_plant_the_collapses_the_subscription_exists_to_refuse() {
    // (a) `closed` wearing the mark for a count of nought. Written as a predicate over a map, so the
    //     check is the one g19 runs rather than a second one that resembles it.
    fn honest(map: impl Fn(Link) -> Option<Nothing>) -> bool {
        map(Link::Closed) != Some(Nothing::Zero)
    }
    assert!(
        honest(Link::nothing),
        "the shipped map fails the predicate its own plant is measured against"
    );
    let planted = |link: Link| {
        if link == Link::Closed {
            Some(Nothing::Zero)
        } else {
            link.nothing()
        }
    };
    assert!(
        !honest(planted),
        "🔴 P16(a): the predicate did not fire on a map that draws a dropped subscription as `no \
         events`. A gate that cannot refuse the defect is not watching it"
    );

    // (b) the mark for `open` colliding with one of the seven words for nothing.
    fn is_new(mark: &str) -> bool {
        !Nothing::ALL
            .into_iter()
            .any(|nothing| nothing.mark() == mark)
    }
    assert!(is_new(live::OPEN_MARK));
    assert!(
        !is_new(Nothing::Zero.mark()),
        "🔴 P16(b): the collision check does not detect a collision"
    );
    assert!(!is_new(Nothing::Unknown.mark()));

    // (c) a classifier that reads a read timeout as a close — the defect measured on the browser
    //     face, where one timeout meant for a list read was applied to a subscription and the
    //     subscription reported a broken engine every six seconds.
    fn as_a_close(result: &std::io::Result<usize>) -> Pulse {
        match result {
            Ok(0) | Err(_) => Pulse::Ended,
            Ok(count) => Pulse::Bytes(*count),
        }
    }
    let window = read_error(std::io::ErrorKind::WouldBlock);
    assert_eq!(as_a_close(&window), Pulse::Ended);
    assert_ne!(
        live::pulse(&window),
        as_a_close(&window),
        "🔴 P16(c): the shipped classifier and the planted one agree, so the plant is planting \
         nothing and the arm above proves nothing"
    );
    assert_eq!(live::pulse(&window), Pulse::Idle);
}

/// 🔴 The framing, at every boundary a socket can put one at.
///
/// Measured first, then written: `GET /v1/stream` answers `application/x-ndjson` **and**
/// `transfer-encoding: chunked`, so there are two layers of framing between the bytes and an event,
/// and neither layer's boundaries line up with a `read`. An implementation that dropped a line
/// straddling a boundary would pass almost every test and lose events in the field at a rate nobody
/// could reproduce — the worst schedule a defect can have — so the split is swept rather than
/// sampled.
#[test]
fn p17_no_event_is_lost_at_a_chunk_or_a_line_boundary() {
    let events = [
        r#"{"event":"candidate.created","cursor":"f39cc060"}"#,
        r#"{"event":"verdict.issued","cursor":"dad64121"}"#,
        r#"{"event":"committed","cursor":"ce730181"}"#,
    ];
    let expected: Vec<String> = events.iter().map(|event| (*event).to_string()).collect();

    // The framing the engine actually produces: one chunk per line, then the zero chunk.
    let mut sent: Vec<u8> = Vec::new();
    for event in events {
        let payload = format!("{event}\n");
        sent.extend_from_slice(format!("{:X}\r\n", payload.len()).as_bytes());
        sent.extend_from_slice(payload.as_bytes());
        sent.extend_from_slice(b"\r\n");
    }
    sent.extend_from_slice(b"0\r\n\r\n");

    // Every split, one at a time.
    for split in 0..=sent.len() {
        let mut frames = live::Frames::chunked();
        let mut lines: Vec<String> = Vec::new();
        for part in [&sent[..split], &sent[split..]] {
            for line in frames.push(part) {
                lines.push(String::from_utf8(line).expect("the fixture is text"));
            }
        }
        assert_eq!(
            lines,
            expected,
            "🔴 P17: split at byte {split} of {} loses or invents a line",
            sent.len()
        );
        assert_eq!(
            frames.partial(),
            0,
            "🔴 P17: bytes stranded at split {split}"
        );
        assert!(frames.finished(), "🔴 P17: the zero chunk was not read");
    }

    // One byte at a time, which is the worst a socket can do.
    let mut frames = live::Frames::chunked();
    let mut lines: Vec<String> = Vec::new();
    for byte in &sent {
        let one = [*byte];
        for line in frames.push(&one) {
            lines.push(String::from_utf8(line).expect("the fixture is text"));
        }
    }
    println!("P17_BYTEWISE={}", lines.len());
    assert_eq!(lines, expected, "🔴 P17: a byte at a time loses a line");

    // 🔴 One line spread over two chunks. The engine does not currently frame it this way, which is
    // exactly why a reader is not allowed to assume it will not: a proxy in front of the engine is
    // enough to change it, and the change would be invisible until events went missing.
    let whole = format!("{}\n", events[0]);
    let (head, tail) = whole.split_at(11);
    let mut straddled: Vec<u8> = Vec::new();
    for part in [head, tail] {
        straddled.extend_from_slice(format!("{:X}\r\n", part.len()).as_bytes());
        straddled.extend_from_slice(part.as_bytes());
        straddled.extend_from_slice(b"\r\n");
    }
    straddled.extend_from_slice(b"0\r\n\r\n");
    let mut frames = live::Frames::chunked();
    let straddling: Vec<String> = frames
        .push(&straddled)
        .into_iter()
        .map(|line| String::from_utf8(line).expect("the fixture is text"))
        .collect();
    assert_eq!(
        straddling,
        vec![events[0].to_string()],
        "🔴 P17: a line split across two chunks did not come back whole"
    );

    // A body with no transfer encoding, because the encoding is read off the headers rather than
    // assumed — and a body that ends in the middle of a line, which is counted and not dropped.
    let mut frames = live::Frames::plain();
    let lines = frames.push(b"{\"event\":\"a\"}\n{\"event\":\"b\"");
    assert_eq!(lines.len(), 1, "🔴 P17: the plain reader lost a whole line");
    assert!(
        frames.partial() > 0,
        "🔴 P17: the half of a line that arrived is being treated as though it never arrived, which \
         is the lie `nothing came`"
    );
}

/// 🔴 An event cannot write a row, measured over the source rather than promised in a paragraph.
///
/// The subscription hands the rest of the face a state and a boolean; no field of an event body
/// crosses that boundary. So **no module of this face spells the name of one** — and that is a
/// property a scan can check, which a sentence about intent is not. The day somebody reaches into
/// `data` to save a read, this gate goes red before the second source of truth exists.
#[test]
fn p19_no_module_of_this_face_names_a_field_of_an_event() {
    // The keys the engine's own events carry, read off `GET /v1/stream` against the running engine
    // on 2026-08-31 rather than out of a document.
    let needles: Vec<(&'static str, u8)> = vec![
        ("candidate.created", 1),
        ("verdict.issued", 1),
        ("canonicalized", 1),
        ("intent_digest", 1),
        ("proof_digest", 1),
        ("ledger_index", 1),
    ];
    let findings = scan(&tui_dir(), &[], &needles);
    println!("P19_FINDINGS={findings:?}");
    assert!(
        findings.is_empty(),
        "🔴 P19: this face names a field of an event body. An event says `look again` and nothing \
         else; a face that reads one is a second thing on this machine that claims to know what is \
         true, and the two disagree the first time a message is missed: {findings:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// g20 / P18 — `req/38` SS974 queue row Q4. The collapse inside the classifier that refuses
// collapses.
// ---------------------------------------------------------------------------------------------

/// 🔴 **The defect SS974 recorded by name.** `wire::cell` answered `zero` for an empty string, so a
/// value the engine carried as `""` was drawn with the mark that means *a count of nought* — the
/// same rounding the six words exist to prevent, committed by the function whose whole job is to
/// prevent it.
///
/// The repair is a seventh word rather than a `Cell::Value("")`, because drawing an empty string as
/// empty space is the other collapse: a blank cell and a cell that was never carried look identical.
#[test]
fn g20_an_empty_string_is_not_a_count_of_nought() {
    let object: serde_json::Value = serde_json::from_str(
        r#"{"empty_text":"","text":"x","count":0,"empty_list":[],"empty_object":{},"null_valued":null,"no":false}"#,
    )
    .expect("fixture parses");

    assert_eq!(
        wire::cell(&object, "empty_text"),
        wire::Cell::Nothing(Nothing::Empty),
        "🔴 g20: the wire carried the key and what it carried has no characters. That is not a count"
    );
    // 🔴 The other half, so this gate cannot be satisfied by deleting `zero` from the vocabulary: a
    // genuine count of nought still reads `0`.
    assert_eq!(
        wire::cell(&object, "count"),
        wire::Cell::Nothing(Nothing::Zero),
        "🔴 g20: a real zero has to survive the repair"
    );
    // 🔴 The declared range of the repair, asserted so that the range is a decision rather than a
    // thing nobody looked at. `[]` is nought items and `{}` is nought members; both of those are
    // counts, and the string is the one container with nothing to count.
    assert_eq!(
        wire::cell(&object, "empty_list"),
        wire::Cell::Nothing(Nothing::Zero)
    );
    assert_eq!(
        wire::cell(&object, "empty_object"),
        wire::Cell::Nothing(Nothing::Zero)
    );
    // The neighbours the repair must not have disturbed.
    assert_eq!(
        wire::cell(&object, "null_valued"),
        wire::Cell::Nothing(Nothing::Unknown)
    );
    assert_eq!(
        wire::cell(&object, "no"),
        wire::Cell::Nothing(Nothing::False)
    );
    assert_eq!(
        wire::cell(&object, "missing"),
        wire::Cell::Nothing(Nothing::Absent)
    );
    assert_eq!(
        wire::cell(&object, "text"),
        wire::Cell::Value("x".to_string())
    );

    // Told apart at the symbol layer and one rung below it.
    assert_ne!(Nothing::Empty.mark(), Nothing::Zero.mark());
    assert_ne!(Nothing::Empty.role(), Nothing::Zero.role());
    assert!(Nothing::Empty
        .mark()
        .chars()
        .all(|c| (' '..='~').contains(&c)));
    println!(
        "G20_MARK={:?} G20_WORDS={}",
        Nothing::Empty.mark(),
        Nothing::ALL.len()
    );
    // The vocabulary grew by one and every gate that sweeps it grew with it.
    assert_eq!(Nothing::ALL.len(), 7);
    assert_eq!(NOTHING_COVERAGE.len(), Nothing::ALL.len());

    // 🔴 The negative control: the classifier as it was, run over the same value. It has to disagree
    // with the shipped one on exactly this case, or the assertion at the top of this test is not
    // measuring a repair.
    let before = |value: &serde_json::Value| -> Option<Nothing> {
        match value {
            serde_json::Value::String(text) if text.is_empty() => Some(Nothing::Zero),
            _ => None,
        }
    };
    let empty = object.get("empty_text").expect("the fixture carries it");
    assert_eq!(before(empty), Some(Nothing::Zero));
    assert_ne!(
        wire::cell(&object, "empty_text"),
        wire::Cell::Nothing(before(empty).expect("the planted answer")),
        "🔴 g20: the shipped classifier still answers what the pre-repair one answered"
    );

    // 🔴 **What this repair is, stated once and measured rather than described.**
    //
    // `cell` is a map from the values the wire can carry onto a vocabulary, so it induces a
    // partition of those values — two values are in the same class when they are drawn the same.
    // Adding a word makes that partition **strictly finer**: the new map separates a pair the old
    // one merged, and it merges **no** pair the old one separated. One-directional refinement is
    // exactly what distinguishes a repair from a trade, and it is checkable.
    //
    // The old classifier is written here as the new one with the two classes glued back together
    // (`Empty` read as `Zero`) rather than as a second hand-written `match`, because a
    // re-implementation would be measuring this test's memory of the old code instead of the old
    // code. Gluing two classes of a partition is precisely the coarsening being undone.
    let classes: Vec<serde_json::Value> =
        serde_json::from_str(r#"["", "x", " ", 0, 1, [], [1], {}, {"k":1}, null, false, true]"#)
            .expect("the sample parses");
    let now = |value: &serde_json::Value| -> String {
        format!("{:?}", wire::cell(&serde_json::json!({ "k": value }), "k"))
    };
    let glued = |value: &serde_json::Value| -> String { now(value).replace("Empty", "Zero") };
    let mut strictly_finer = false;
    for left in &classes {
        for right in &classes {
            if now(left) == now(right) {
                assert_eq!(
                    glued(left),
                    glued(right),
                    "🔴 g20: {left} and {right} are drawn the same now and were drawn differently \
                     before. The repair merged a distinction, which is a trade and not a repair"
                );
            }
            if glued(left) == glued(right) && now(left) != now(right) {
                strictly_finer = true;
            }
        }
    }
    println!(
        "G20_SAMPLE={} G20_STRICTLY_FINER={strictly_finer}",
        classes.len()
    );
    assert!(
        strictly_finer,
        "🔴 g20: the new classification is not finer than the old one anywhere, so nothing was \
         repaired"
    );
}

/// The seventh word on the screen, through the same `draw` a live frame goes through.
///
/// 🔴 A classifier repaired in isolation is a repair nobody can see. The cell in the `state` column
/// has to come out as the new mark and not as `0`, in a frame.
#[test]
fn p18_the_screen_draws_an_empty_string_as_itself() {
    let healthz: serde_json::Value = serde_json::from_str(HEALTHZ).expect("fixture parses");
    let rows: serde_json::Value = serde_json::from_str(
        r#"{"items":[{"transformation":"gx1:t3sto0000000009","state":"","verdict":"Admit","enforced":true,"created_at":"2026-08-31T00:00:00Z","actor":"agent-a","scope":"src/lib.rs","inverse_status":"Escrowed","rollback":null,"superseded_by":null}],"next_cursor":null}"#,
    )
    .expect("fixture parses");
    let answered = |route: &str, body: serde_json::Value| wire::Reading {
        route: format!("GET {route}"),
        status: Some(200),
        read_at: "2026-08-31T00:00:00.000000000Z".to_string(),
        elapsed_ms: 1,
        body: Some(body),
        error: None,
    };
    let empty_list: serde_json::Value = serde_json::from_str(CANDIDATES).expect("fixture parses");
    let screen = Screen {
        healthz: answered(wire::ROUTES[0], healthz),
        transformations: answered(wire::ROUTES[1], rows),
        candidates: answered(wire::ROUTES[2], empty_list.clone()),
        escalations: answered(wire::ROUTES[3], empty_list),
    };

    let text = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        200,
        24,
        Tier::Mono,
        false,
    ));
    println!("--- P18 200x24 ---\n{text}");
    let column = LEDGER_COLUMNS
        .iter()
        .position(|column| column.key == "state")
        .expect("the state is a declared column");
    let row = text
        .lines()
        .find(|line| line.contains("src/lib.rs"))
        .unwrap_or_else(|| panic!("the record is not on the screen:\n{text}"));
    let cell = row
        .split_whitespace()
        .nth(column)
        .unwrap_or_default()
        .to_string();
    println!("P18_STATE_CELL={cell:?}");
    assert_eq!(
        cell,
        Nothing::Empty.mark(),
        "🔴 P18: the wire carried `state: \"\"` and the screen draws {cell:?}"
    );
    assert_ne!(
        cell,
        Nothing::Zero.mark(),
        "🔴 P18: the reader is being told the state is a count, and it is nought"
    );
}

// ---------------------------------------------------------------------------------------------
// g21 — the reducer and the screen, on what opening means.
// ---------------------------------------------------------------------------------------------

/// 🔴 **The disagreement design round 2 found and `renderer::offered` used to carry a paragraph
/// about.** `acts::apply` set `view.open` whatever the row count was; `renderer::subject` opens only
/// when there is a record to open. So the declaration said `act.open` moves the state on an empty
/// list and the screen said it does not, and a note derived from the declaration would have promised
/// a key that does nothing.
///
/// This gate is what keeps them from drifting apart again, and it asks the general question rather
/// than the one case: **every act a note offers at a given row count has to move something at that
/// row count.**
#[test]
fn g21_the_reducer_and_the_screen_agree_about_what_opening_means() {
    let shut = View::default();

    // The repair itself.
    let (after, signal) = acts::apply(&shut, Act::Open, 0);
    assert!(
        !after.open,
        "🔴 g21: the reducer opened a record on a list that has none"
    );
    assert_eq!(signal, acts::Signal::None);
    assert!(
        acts::apply(&shut, Act::Open, 1).0.open,
        "🔴 g21: and it still opens when there is something to open"
    );
    // A list that shrank to nothing between two reads closes what was open, rather than leaving an
    // opened record pointing at a row that is gone.
    let opened = View {
        selected: 0,
        open: true,
    };
    assert!(!acts::apply(&opened, Act::Open, 0).0.open);

    // 🔴 **The invariant, stated as one rather than as a case.** An empty list is a **fixed point**
    // of the reducer's state: `View::default()` is carried to itself by every declared act, and the
    // attention is carried to `0` from anywhere. Written over `ACTS` rather than over `Act::Open`,
    // so an act added later is measured by it without anybody remembering to add a line.
    for act in acts::ACTS {
        assert_eq!(
            acts::apply(&View::default(), act, 0).0,
            View::default(),
            "🔴 g21: {} moves the state on a list with nothing in it. The empty list is a fixed \
             point of this reducer or the screen and the declaration are describing two programs",
            act.name()
        );
    }
    for start in [
        shut,
        opened,
        View {
            selected: 9,
            open: true,
        },
    ] {
        for act in acts::ACTS {
            let (next, _) = acts::apply(&start, act, 0);
            assert_eq!(
                next,
                View::default(),
                "🔴 g21: {} carried {start:?} somewhere other than the one state a list of nothing \
                 has",
                act.name()
            );
        }
    }

    // 🔴 The general property, and the one that found the second defect. `renderer::offered` is the
    // note's declaration of what a reader can do at this row count, and every entry in it has to be
    // true of the reducer at that row count.
    //
    // 🔴 It went red at `rows = 1` on its first run: the note named `act.next` on a list of one
    // record, where the attention has nowhere to go. The repair was a third rung in `offered`, not
    // a narrower question here — a probe that skips the row count it fails at is measuring the
    // probe's comfort. Every row count from nought to four is swept, so a rung added later is
    // measured without anybody remembering to add a line.
    for rows in [0usize, 1, 2, 3, 4] {
        let starts = [
            View {
                selected: 0,
                open: false,
            },
            View {
                selected: 0,
                open: true,
            },
            View {
                selected: rows.saturating_sub(1),
                open: false,
            },
        ];
        for act in renderer::offered(rows) {
            let moved = starts.iter().any(|start| {
                let (next, signal) = acts::apply(start, *act, rows);
                next != *start || signal != acts::Signal::None
            });
            println!("G21 rows={rows} {} moved={moved}", act.name());
            assert!(
                moved,
                "🔴 g21: the note offers {} on a list of {rows} and the reducer says it does \
                 nothing. A key a reader is told about and which does nothing reads as a broken \
                 program",
                act.name()
            );
        }
    }

    // And on the frame: with nothing to open, the two views draw the same screen, which is what
    // `open` meaning nothing here amounts to.
    let fixture = Fixture::start_refusing();
    let refused = fixture.read();
    let with = renderer::buffer_text(&renderer::render_view_to_buffer(
        &refused,
        80,
        24,
        Tier::Mono,
        false,
        &opened,
    ));
    let without = renderer::buffer_text(&renderer::render_view_to_buffer(
        &refused,
        80,
        24,
        Tier::Mono,
        false,
        &shut,
    ));
    assert_eq!(
        with, without,
        "🔴 g21: the flag changes the screen on a list with nothing in it"
    );

    // 🔴 The negative control: the reducer as it was, which the assertion at the top has to disagree
    // with. Without this the top of this test is a green that cannot go red.
    let unconditional = |view: &View| View {
        open: true,
        ..*view
    };
    assert!(unconditional(&shut).open);
    assert_ne!(
        acts::apply(&shut, Act::Open, 0).0,
        unconditional(&shut),
        "🔴 g21: the shipped reducer still does what the pre-repair one did"
    );
}

// ---------------------------------------------------------------------------------------------
// g24 — `req/964` §16 `[T-r3-view-in-resolve]`. The disclosure describes the screen that is
// actually drawn, and the two named ceilings that stood in the way are closed.
// ---------------------------------------------------------------------------------------------

/// 🔴 **Two named ceilings, both of them the same shape: a line that says what is missing, composed
/// from something other than what was drawn.**
///
/// 1. `renderer::subject` drew an opened record — every member the wire carried, one per row —
///    while the disclosure went on reporting the **grid's** dropped columns. `4 of 11 fields not
///    drawn` stood under a region that was drawing all eleven.
/// 2. `layout::resolve` chose the provenance's rung **after** composing the disclosure, so the
///    bottom rung could give up the connection's counts — which are `Recoverable::Nowhere`, so
///    losing them destroys them — with no line on the screen admitting it.
///
/// Both are closed by handing the subject's shape to `resolve` and deciding the rung inside its
/// loop. The shape comes from `layout::subject_shape`, which is also what the renderer reads, so
/// there is one answer to "which shape is this" rather than two that can drift.
#[test]
fn g24_the_disclosure_describes_the_screen_that_was_actually_drawn() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let measured = renderer::measured(&screen);
    let shut = View::default();
    let opened = View {
        selected: 0,
        open: true,
    };
    let items = screen.transformations.items();
    assert!(
        !items.is_empty(),
        "the fixture has to carry a record or neither half of this gate is measuring anything"
    );

    // 1. One classifier, and it answers differently for the two views.
    assert_eq!(
        layout::subject_shape(&screen.transformations, &shut),
        layout::Subject::Grid
    );
    assert_eq!(
        layout::subject_shape(&screen.transformations, &opened),
        layout::Subject::Record
    );
    // 🔴 And it is the **only** answer: the renderer does not decide the shape a second time.
    //    Measured over the source, because "we will remember to call the classifier" is a promise
    //    and this is a property. The day somebody writes `view.open` back into the drawing code,
    //    the disclosure and the region can disagree again, and this goes red first.
    let renderer_source =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui/renderer.rs"))
            .expect("renderer.rs is readable");
    assert!(
        !renderer_source.contains("view.open"),
        "🔴 g24: the drawing code decides for itself whether a record is open. There is one \
         classifier (`layout::subject_shape`) and the disclosure reads it too; a second answer here \
         is how the line that says what is missing comes to describe a different screen"
    );

    // An **empty** list keeps its grid even when the view says open, because an empty grid is still
    // a grid and its header is what says which columns found nothing.
    let empty = Fixture::start_refusing();
    let refused = empty.read();
    assert_eq!(
        layout::subject_shape(&refused.transformations, &opened),
        layout::Subject::Grid,
        "🔴 g24: a refused read has no record to open, and drawing one would be inventing it"
    );

    // 2. The grid drops fields by width; the record does not drop any by width at all.
    let grid = layout::resolve(80, 24, &measured, false, layout::Subject::Grid);
    let record = layout::resolve(80, 24, &measured, false, layout::Subject::Record);
    println!("G24_GRID={}", grid.disclosure);
    println!("G24_RECORD={}", record.disclosure);
    assert!(
        !grid.dropped_fields.is_empty(),
        "eighty cells cannot hold eleven fields; a grid that drops none is not measuring"
    );
    assert!(
        record.dropped_fields.is_empty(),
        "🔴 g24: a record draws every member the wire carried, so nothing is dropped by width: \
         {:?}",
        record.dropped_fields
    );
    assert!(
        grid.disclosure.contains("fields not drawn"),
        "the grid's disclosure has to keep saying what the columns cut: {}",
        grid.disclosure
    );
    assert!(
        !record.disclosure.contains("fields not drawn"),
        "🔴 g24: the disclosure is reporting the grid's dropped columns over a region that is not \
         drawing a grid: {}",
        record.disclosure
    );
    assert!(
        record.disclosure.contains("a record is open"),
        "🔴 g24: the line that says what is not on the screen does not say which screen it is \
         describing: {}",
        record.disclosure
    );

    // 3. 🔴 The plant: the composer as it was, which counted the grid whatever was drawn. The
    //    shipped answer has to differ from it on exactly this case, or the assertions above are
    //    comparing a string to itself.
    let before = layout::resolve(80, 24, &measured, false, layout::Subject::Grid).disclosure;
    assert_ne!(
        record.disclosure, before,
        "🔴 g24: the shipped composer still says what the pre-repair one said about an open record"
    );

    // 4. And on a frame, through the same `draw` a live frame goes through: the members the record
    //    shows are on the screen **and** the bottom line is not calling them undrawn.
    let drawn = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        80,
        24,
        Tier::Mono,
        false,
        &opened,
    )));
    println!("G24_FRAME={drawn}");
    assert!(
        drawn.contains("members"),
        "the record's own line is what counts them, and it is not on the frame: {drawn}"
    );
    assert!(
        !drawn.contains("fields not drawn"),
        "🔴 g24: the frame draws a record and its bottom line reports the grid's cut columns: \
         {drawn}"
    );

    // 5. The second ceiling: the rung is a decision the plan carries, and the rung that gives up
    //    the connection's counts says so.
    let wide_plan = layout::resolve(120, 24, &measured, false, layout::Subject::Grid);
    let narrow = layout::resolve(24, 24, &measured, false, layout::Subject::Grid);
    println!(
        "G24_RUNGS wide={:?} narrow={:?}\nG24_NARROW_DISCLOSURE={}",
        wide_plan.provenance_rung, narrow.provenance_rung, narrow.disclosure
    );
    assert!(
        wide_plan.provenance_rung.carries_counts(),
        "🔴 g24: a hundred and twenty cells give up the connection's counts"
    );
    assert_eq!(
        wide_plan.provenance,
        measured.long(),
        "the rung the plan names and the text it carries are the same decision"
    );
    assert_eq!(
        narrow.provenance_rung,
        layout::Rung::Bare,
        "🔴 g24: at twenty-four cells the short form does not fit, so the bare rung is the one"
    );
    assert_eq!(narrow.provenance, measured.bare());
    assert!(
        !narrow.provenance_rung.carries_counts(),
        "the bare rung is the one that gives the counts up"
    );
    assert!(
        narrow.disclosure.contains(NO_ADDRESS_PHRASE),
        "🔴 g24: the connection's counts were dropped and they are measured here — no route \
         returns them — so dropping them without a line saying so destroys them: {}",
        narrow.disclosure
    );
    assert!(
        !wide_plan.disclosure.contains("counts cut")
            && !wide_plan.disclosure.contains("counts are not drawn"),
        "🔴 g24: the wide plan claims a drop it did not make: {}",
        wide_plan.disclosure
    );
}

// ---------------------------------------------------------------------------------------------
// g22 — `req/38` SS988. `never` is not `closed`, and the word it takes is chosen with the
// partition rather than with a sentence.
// ---------------------------------------------------------------------------------------------

/// 🔴 **The defect SS988 recorded, stated more strongly than SS988 stated it.** SS988 said the two
/// cases were told apart by the trailing `events` count. They were not: a connection that opened
/// once, received nothing and dropped spelled `closed after 0 events, 1 reconnects`, byte for byte
/// what an engine that had never once been up spelled. The counts separated them at no width. What
/// made it look as though they did is the accident that the engine on `:8842` replays fourteen
/// events on connect.
///
/// The repair is a fifth state, and this gate is what makes the choice of its word a measurement
/// rather than an opinion — written g20's way, so the "before" map is the shipped map with the two
/// classes glued back together and never a second hand-written table.
#[test]
fn g22_never_is_carved_out_of_closed_and_the_partition_is_only_ever_finer() {
    /// What a map of states onto the vocabulary draws for one state.
    fn drawn(map: &dyn Fn(Link) -> Option<Nothing>, link: Link) -> &'static str {
        map(link).map_or(live::OPEN_MARK, Nothing::mark)
    }

    /// Is `map` a **strict refinement** of the map that had no `never` in it?
    ///
    /// The map before the repair is this one with `Never`'s class glued into `Closed`'s — gluing
    /// two classes is precisely the coarsening being undone, so the control cannot drift from the
    /// thing it controls. Refinement is one-directional: the new map may separate a pair the old
    /// one merged and may merge **no** pair the old one separated. A change that does both is a
    /// trade and not a repair.
    fn refines(map: &dyn Fn(Link) -> Option<Nothing>) -> bool {
        let glued = |link: Link| {
            if link == Link::Never {
                drawn(map, Link::Closed)
            } else {
                drawn(map, link)
            }
        };
        let mut separates_something = false;
        for left in live::LINKS {
            for right in live::LINKS {
                if drawn(map, left) == drawn(map, right) && glued(left) != glued(right) {
                    // A pair the old map told apart is drawn the same now.
                    return false;
                }
                if glued(left) == glued(right) && drawn(map, left) != drawn(map, right) {
                    separates_something = true;
                }
            }
        }
        separates_something
    }

    /// Does this map keep every state off the mark for a count of nought? (g19's sweep, as a
    /// predicate, so the plant below is measured by the check g19 runs rather than by a second one
    /// that resembles it.)
    fn no_zero(map: &dyn Fn(Link) -> Option<Nothing>) -> bool {
        !live::LINKS
            .iter()
            .any(|link| map(*link) == Some(Nothing::Zero))
    }

    let shipped = |link: Link| link.nothing();
    println!(
        "G22_MARKS={:?}",
        live::LINKS
            .iter()
            .map(|link| (link.name(), link.mark()))
            .collect::<Vec<_>>()
    );
    assert!(
        refines(&shipped),
        "🔴 g22: the shipped map is not a strict refinement of the one that had `never` folded into \
         `closed`, so either nothing was repaired or something was traded away"
    );
    assert!(no_zero(&shipped));

    // 🔴 The plant that carries the whole argument against `--`, which is the obvious first reach.
    // `absent` is already `off`'s mark, so giving it to `never` **merges** `{off, never}` — a pair
    // the old map separated (`off` drew `--`, the never-case drew `?`). The predicate has to
    // refuse it, and it refusing is what makes the sentence in the module documentation a
    // measurement instead of a preference.
    let as_absent = |link: Link| {
        if link == Link::Never {
            Some(Nothing::Absent)
        } else {
            link.nothing()
        }
    };
    assert!(
        !refines(&as_absent),
        "🔴 g22: the refinement predicate accepts a map that draws `never` and `off` identically. A \
         gate that cannot refuse the obvious wrong answer is not choosing the right one"
    );

    // 🔴 And the plant for the other wrong answer. `zero` would say *no events ever arrived*, which
    // is a measurement this process never made. **This one the refinement predicate does not
    // catch** — `0` collides with no other state's mark, so the partition is still strictly finer —
    // and that is why there are two predicates here and not one. Written down rather than left to
    // be discovered: a single check would have passed a map that draws a connection that never
    // existed as a ledger that never moved.
    let as_zero = |link: Link| {
        if link == Link::Never {
            Some(Nothing::Zero)
        } else {
            link.nothing()
        }
    };
    assert!(
        refines(&as_zero),
        "the refinement predicate is expected to be silent about this one; if it now fires, the two \
         predicates below are no longer independent and the comment above is wrong"
    );
    assert!(
        !no_zero(&as_zero),
        "🔴 g22: the `zero` sweep does not fire on a map that draws a connection that has never been \
         up as a count of nought"
    );

    // 🔴 The collapse itself, at the layer it was visible on: the sentences. Under the shipped map
    // the two states say different things at the counts that used to make them identical; under the
    // map before the repair — `never` spelled as `closed`, which is literally what the code did —
    // they are byte-equal.
    assert_ne!(
        report(Link::Never, 0, 0).long(),
        report(Link::Closed, 0, 0).long(),
        "🔴 g22: `never` and `closed` say the same sentence"
    );
    let before = |link: Link, events: u64, reconnects: u64| {
        let folded = if link == Link::Never {
            Link::Closed
        } else {
            link
        };
        report(folded, events, reconnects).long()
    };
    assert_eq!(
        before(Link::Never, 0, 1),
        before(Link::Closed, 0, 1),
        "🔴 g22: the pre-repair spelling is supposed to be the collapse. If these differ, this test \
         is not measuring the defect SS988 named"
    );
    println!(
        "G22_NEVER={:?} G22_CLOSED={:?} G22_BEFORE={:?}",
        report(Link::Never, 0, 0).long(),
        report(Link::Closed, 0, 0).long(),
        before(Link::Never, 0, 1)
    );

    // 🔴 The second defect of the same family, found in the same read: `reconnects` was a counter
    // incremented on every pass of the retry loop, so an engine that had never been up reported a
    // growing number of re-openings of a connection that had never existed. It is now derived from
    // the number of times the stream was actually up. The plant is the counter as it was — the
    // number of attempts standing in for the number of openings.
    let counted = |opens: u64| opens; // an attempt reported as an accomplishment
    assert_eq!(live::reopenings(0), 0);
    assert_ne!(
        live::reopenings(1),
        counted(1),
        "🔴 g22: the shipped derivation still counts the first opening as a re-opening"
    );
    for opens in 0..8u64 {
        assert!(
            live::reopenings(opens) < opens.max(1),
            "🔴 g22: {opens} openings cannot be {} re-openings",
            live::reopenings(opens)
        );
    }

    // The short form carries the distinction too, because the long one is the first thing a narrow
    // screen gives up.
    let shorts: BTreeSet<String> = live::LINKS
        .iter()
        .map(|link| format!("{}{}", link.mark(), report(*link, 0, 0).short()))
        .collect();
    println!("G22_SHORTS={shorts:?}");
    assert_eq!(
        shorts.len(),
        live::LINKS.len(),
        "🔴 g22: two states are indistinguishable once the line is shortened: {shorts:?}"
    );
}

// =============================================================================================
// `req/38` SS996 — the three rows of round 4: the arm nothing can reach, the ruling that was
// only prose, and the declaration table nothing was holding closed.
// =============================================================================================

/// 🔴 **g25 — the report for a record this face cannot read exists, and says `?`.**
///
/// `Subscription::report` has an arm for a poisoned lock. Reading every critical section in
/// `live` settles that **nothing on this code can reach it**: `set`, `record`, `due`, `report` and
/// the one line in `start` hold the guard across an assignment or an addition and nothing else, so
/// there is no panic under a guard and therefore no poisoning. That is a reason to *name* the arm,
/// not a reason to leave it unnamed — an arm nothing can fire and nothing spells is what a later
/// edit to those critical sections turns into a silent lie.
///
/// So the arm is [`live::unreadable_record`] and this gate is its existence proof, fired every run.
/// The first half is the precondition, measured rather than assumed: a mutex whose holder panicked
/// answers `Err` for the rest of the process's life. **The panic printed by that thread is the
/// point of the test, not a failure of it.**
#[test]
fn g25_the_arm_for_a_record_this_face_cannot_read_exists_and_says_unknown() {
    // The precondition class is real and reproducible, which is what makes the arm a branch rather
    // than a decoration.
    let lock = Arc::new(Mutex::new(0u8));
    let other = Arc::clone(&lock);
    println!("G25: the panic below is deliberate — it is how a lock becomes poisoned.");
    let died = std::thread::spawn(move || {
        let _held = other
            .lock()
            .expect("the lock is clean before this thread takes it");
        panic!("g25 poisons the lock on purpose");
    })
    .join();
    assert!(
        died.is_err(),
        "🔴 g25: the thread that had to panic did not"
    );
    assert!(
        lock.lock().is_err(),
        "🔴 g25: a mutex whose holder panicked answered Ok, so the arm's precondition is not what \
         this gate says it is"
    );

    let report = live::unreadable_record();
    println!(
        "G25_UNREADABLE={:?} MARK={:?} LONG={:?}",
        report.link,
        report.link.mark(),
        report.long()
    );
    assert_eq!(
        report.link,
        Link::Closed,
        "🔴 g25: a record that cannot be read is being reported as something other than closed"
    );
    assert_eq!(
        report.link.nothing(),
        Some(Nothing::Unknown),
        "🔴 g25: the state for an unreadable record stopped wearing unknown"
    );
    // The plant: the two neighbours it must not collapse into. `never` would be a claim about a
    // history this process can no longer read, and `zero` would be the count it is not entitled to.
    for wrong in [Link::Never.mark(), Link::Off.mark(), Nothing::Zero.mark()] {
        assert_ne!(
            report.link.mark(),
            wrong,
            "🔴 g25: the unreadable record is drawn as {wrong}, which is a different sentence"
        );
    }
}

/// 🔴 **g26 — the note is paid for out of spare rows, and the one shape where it vanishes is held
/// where it is.**
///
/// The ruling lived only in a comment: the list's legend is paid for out of rows the records did not
/// take, never out of a record, because the first build of it turned a list of three into a list of
/// two at 46x12 to print a legend with no room for a key. `renderer::note_rows` is that sentence as
/// a function and this is the sentence as a gate, fired over every shape of list and body.
///
/// 🔴 It also **bounds a defect this round decided not to close** (`req/964` §16). Where the records
/// fill the body exactly there is no spare row, so the note is not drawn and nothing says so. The
/// two ways out are a reversal of the ruling above — which reinstates the exact screen the ruling
/// was measured from — and telling `layout::resolve` how many rows the region actually drew, which
/// is the order inversion §16 named. Neither was taken, so the set of shapes where the legend
/// disappears is pinned to exactly the diagonal: it cannot grow, and a reversal fires this gate
/// instead of passing quietly.
#[test]
fn g26_the_note_is_paid_from_spare_rows_and_its_silent_drop_is_exactly_the_diagonal() {
    let mut silent: Vec<(usize, usize)> = Vec::new();
    for body in 0usize..=24 {
        for occupied in 0usize..=24 {
            let rows = renderer::note_rows(occupied, body);
            assert!(
                rows <= body,
                "🔴 g26: {rows} rows of note budgeted into a body of {body}"
            );
            assert!(rows <= 2, "🔴 g26: a legend of {rows} rows is furniture");
            if occupied <= body {
                assert!(
                    rows <= body - occupied,
                    "🔴 g26: the note took a row from a record: {occupied} rows of content in a \
                     body of {body} and {rows} rows of legend"
                );
            } else {
                assert_eq!(
                    rows,
                    usize::from(body > 0),
                    "🔴 g26: a list that is already being cut pays for the note out of the row the \
                     count was on, and only when there is a row at all"
                );
            }
            if rows == 0 && body > 0 && occupied > 0 {
                silent.push((occupied, body));
            }
        }
    }
    println!("G26_SILENT={silent:?}");
    let diagonal: Vec<(usize, usize)> = (1..=24).map(|n| (n, n)).collect();
    assert_eq!(
        silent, diagonal,
        "🔴 g26: the shapes where the legend disappears with nothing saying so are no longer \
         exactly the ones where the records fill the body"
    );

    // The plant is the reversal: let the note take a record's row when the body is full, and the
    // ruling assertion above is what refuses it. Fired here so this gate is known to say no.
    let reversed = |occupied: usize, body: usize| {
        if occupied >= body {
            1
        } else {
            (body - occupied).min(2)
        }
    };
    assert!(
        reversed(3, 3) > 3usize.saturating_sub(3),
        "🔴 g26: the plant for this gate no longer breaks the rule the gate enforces"
    );
    assert_ne!(
        reversed(3, 3),
        renderer::note_rows(3, 3),
        "🔴 g26: the shipped budget and the reversal of it agree, so this gate is a tautology"
    );
}

/// 🔴 **g27 — the declared acts are closed, and no two of them answer the same key.**
///
/// `acts` argues that a `match` on a key code in the drawing loop is wiring no gate can read, and
/// replaces it with a declaration. The declaration has two holes of its own and neither was held by
/// anything: `ACTS` is an array beside the enum, so a ninth `Act` compiles without ever reaching
/// `apply` or `for_key`; and `for_key` resolves a key by scanning `ACTS` in order, so a key declared
/// twice is answered by whichever act is written first — one table with two rows for the same key,
/// which is the defect the module exists to prevent, one level down.
///
/// The slot table below is the closure: a ninth variant has to be given an arm, the arm has to name
/// an index, and the index has to be one `ACTS` actually holds.
#[test]
fn g27_the_declared_acts_are_closed_and_no_key_answers_twice() {
    let slot = |act: Act| match act {
        Act::Prev => 0usize,
        Act::Next => 1,
        Act::First => 2,
        Act::Last => 3,
        Act::Open => 4,
        Act::Close => 5,
        Act::Read => 6,
        Act::Leave => 7,
    };
    for act in acts::ACTS {
        assert_eq!(
            acts::ACTS.get(slot(act)),
            Some(&act),
            "🔴 g27: {} is declared and the table does not hold it at its own slot",
            act.name()
        );
    }
    assert_eq!(
        acts::ACTS.len(),
        8,
        "🔴 g27: the table grew or shrank without the slots being redrawn"
    );
    // 🔴 The loop above walks the **table**, so on its own it cannot see an act that was left out of
    // it — the first version of this gate could not, and the plant that dropped `act.prev` was
    // caught by the key check below instead, which is luck rather than coverage. The slots the table
    // fills are what close it: eight entries occupying eight distinct slots is the same statement as
    // "every act the enum declares is in here exactly once", and it fails on a table that repeats
    // one act to make room for the one it dropped.
    let filled: BTreeSet<usize> = acts::ACTS.into_iter().map(slot).collect();
    assert_eq!(
        filled,
        (0..8).collect::<BTreeSet<usize>>(),
        "🔴 g27: the table does not hold every declared act exactly once — slots filled: {filled:?}"
    );

    let mut seen: Vec<&str> = Vec::new();
    for act in acts::ACTS {
        assert!(
            !act.keys().is_empty(),
            "🔴 g27: {} declares no key, so nothing can produce it and `spelled` would panic on it",
            act.name()
        );
        for key in act.keys() {
            assert!(
                !seen.contains(key),
                "🔴 g27: {key} is declared by two acts, and `for_key` answers with whichever is \
                 written first"
            );
            seen.push(key);
            assert_eq!(
                acts::for_key(key),
                Some(act),
                "🔴 g27: {key} is declared by {} and the one road from a key to an act does not \
                 take it there",
                act.name()
            );
        }
    }
    println!("G27_KEYS={seen:?}");

    // The plant: the same lookup over a table with a key on two acts. The shipped table must not
    // look like this one, and the check above is what tells them apart.
    let doubled = [(Act::Open, "l"), (Act::Close, "l")];
    let first = doubled
        .iter()
        .find(|(_, key)| *key == "l")
        .map(|(act, _)| *act);
    assert_eq!(
        first,
        Some(Act::Open),
        "🔴 g27: the plant does not demonstrate the silent first-wins the gate refuses"
    );
    assert_ne!(
        doubled[0].1, "",
        "🔴 g27: the plant is empty and proves nothing"
    );
}
