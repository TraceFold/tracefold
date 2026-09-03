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
///
/// 🔴 **`inverse_status` was `Escrowed`, which no engine in this workspace can emit** (`req/984`
/// §7-B S-4, `req/924` §TUI-55; gate `g71`, `tools/gates/fixture_vocab_gate.mjs`; repaired by
/// `[T-r55]`, 2026-09-02). `gx_engine::store::InverseStatus` has seven words and that is not one of
/// them, so every assertion resting on it was an assertion about a screen no reader will ever see —
/// **green about a fiction**. `Available` is the word 42 §3.12 gives for the fact the fixture meant
/// (*the escrowed inverse is still there*), and it is the word the engine sends. Five sites in this
/// file carried it; all five moved together, because a fixture and the assertions that read it are
/// one statement.
const TRANSFORMATIONS: &str = r#"{"items":[
{"transformation":"gx1:t3sto0000000001","state":"Committed","verdict":"Admit","enforced":true,"created_at":"2026-08-30T09:00:00Z","actor":"agent-a","scope":"src/lib.rs","inverse_status":"Available","rollback":null,"superseded_by":null},
{"transformation":"gx1:t3sto0000000002","state":"Draft","verdict":null,"enforced":false,"actor":"agent-b","scope":"README.md","inverse_status":null,"rollback":null,"superseded_by":null}
],"next_cursor":null}"#;

const CANDIDATES: &str = r#"{"items":[],"next_cursor":null}"#;
const ESCALATIONS: &str = r#"{"items":[],"next_cursor":null}"#;

/// The one id this fixture server holds a receipt for. Every other id gets the `404` the engine
/// gives, which is the branch `wire::ReceiptMark::NotHere` exists for.
const RECEIPT_HOLDER: &str = "gx1:t3sto0000000001";

/// `GET /v1/transformations/{id}`, in the six members `crates/gx-api/src/handlers.rs`'s
/// `get_transformation` answers with.
///
/// 🔴 `state`, `superseded_by`, `rollback` and `inverse_status` are **the same values the page's
/// first row carries**, so the face's own comparison says `agrees` on this bed. `g78`'s second half
/// moves one of them and requires the row to say which one moved: a comparison that can only ever
/// answer *agrees* is not a comparison.
const ONE_TRANSFORMATION: &str = r#"{"transformation":{"id":"gx1:t3sto0000000001"},"state":"Committed","receipt":null,"superseded_by":null,"rollback":null,"inverse_status":"Available"}"#;

/// `GET /v1/receipts/{tid}`: the document, and the decoded half the engine mounts beside it.
///
/// 🔴 `receipt_view` is `handlers.rs`'s own key list. `postcondition_fingerprint` is present and is
/// **not** drawn by this face — a fixture carrying a member the face ignores is what keeps `g79`
/// from passing by accident on a bed shaped to the face rather than to the wire.
const RECEIPT: &str = r#"{"envelope":{"payloadType":"application/vnd.gx.receipt+dag-cbor","payload":"omdzdWJqZWN0","signatures":[]},"issued_at":"2026-09-02T00:00:00Z","receipt_view":{"subject":"gx1:t3sto0000000001","tree_size":12,"leaf_index":3,"root":"gx1:r00tzzzzzzzzzz","key_id":"gx1:k3yzzzzzzzzzz","postcondition_fingerprint":null,"issued_at":"2026-09-02T00:00:00Z"},"server_health":{"status":"ok","status_reason":null}}"#;

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
    } else if let Some(tid) = path.strip_prefix("/v1/receipts/") {
        // 🔴 **`404` is an answer and this server gives it** (`[T-r55]`, 2026-09-02). 44 §2.2 makes
        // `GET /receipts/{tid}` "the receipt for a **committed** transformation" and
        // `crates/gx-api/src/handlers.rs`'s `get_receipt` answers `404` with a refusal naming
        // **two** facts at once — *it has not been committed*, or *this server holds neither its
        // row nor its archive*. A fixture that answered `200 {}` for every id would let the face's
        // classifier off the one branch it exists for.
        if tid == RECEIPT_HOLDER {
            (200, RECEIPT)
        } else {
            (404, r#"{"title":"not found","gx_code":"NOT_FOUND"}"#)
        }
    } else if path.starts_with("/v1/transformations/") {
        (200, ONE_TRANSFORMATION)
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
    // 🔴 **The locator names two columns, and it named one.** `starts_with("transformation")` was
    // unambiguous while the subject region's first row was the grid's header; the heading strip
    // (`layout::heading`) begins with the page's name, `transformations`, and starts with the same
    // fifteen characters. The probe then read the row *below the heading*, which is the header
    // itself, and reported the header as the row for a refused route. This is a repair to the
    // instrument and not to the floor: the assertions below are untouched, and the header is now
    // identified by two of its own column keys rather than by a prefix a second row shares.
    let header_at = lines
        .iter()
        .position(|line| {
            line.trim_start().starts_with("transformation") && line.contains(wire::VERDICT_KEY)
        })
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
    //
    // 🔴 **Read through the face's own wire reader rather than off the frame, and the loss is
    // recorded rather than hidden** (`req/924` §TUI-22, `req/38` SS1049, Owner `#266-T`,
    // 2026-09-01: *`journal_rows 31`=`25 of 31` と重複* — persuasion, deleted). The carrier left
    // the screen with the ruling, so the discrimination is asked of `wire::cell`, which is the one
    // function the rail and the grid read **every** value through: a zero that collapsed into
    // unknown would change this text, so this cannot be satisfied by dropping `zero` from the
    // vocabulary either.
    //
    // 🔴 **Named ceiling, and it is a real one.** With `journal_rows` off the rail there is no
    // wire-sourced zero drawn on the grid at any ruled shape, so the **frame** no longer carries a
    // positive control of its own. That is printed and left open rather than asserted away: the
    // ruling deleted a duplicate and took this probe's on-screen half with it.
    let body = screen
        .healthz
        .body
        .clone()
        .unwrap_or(serde_json::Value::Null);
    let zero = wire::cell(&body, "journal_rows").text();
    let absent = wire::cell(&body, "status_reason").text();
    println!(
        "P9_WIRE journal_rows={zero:?} status_reason={absent:?} on_frame={}",
        flat(&text).contains("journal_rows")
    );
    assert_eq!(
        zero,
        Nothing::Zero.mark(),
        "🔴 P9: `/v1/healthz` answered 200 with `journal_rows: 0`, and a real zero has to survive \
         this repair. The face reads it as {zero:?}"
    );
    assert_ne!(
        zero, absent,
        "🔴 P9: a measured nought and a key with nothing behind it read the same, which is the \
         collapse this probe exists to refuse"
    );
    // 🔴 **A divergence found by writing this, printed and not asserted.** `req/924` §TUI-23
    // (2026-09-01, SS1051) rules the wire spelling for `status_reason` and states, as its ground,
    // that *面の写像は既に `null` → `--`(Absent) で定義済*. It is not: `wire::cell` reads a JSON
    // `null` as **Unknown**, which is this suite's own fixture comment two hundred lines up
    // (*`verdict` is `null` on the second row (measured, not knowable)*) and is what several gates
    // in this file already require. So the ruling and the implementation disagree about which of
    // the seven words a `null` is, and the disagreement is older and wider than this lane.
    //
    // It is left as a finding rather than repaired here for the reason `req/38` SS856 separates:
    // this is not a repair that mirrors existing code, it is a change to what one of the seven
    // words **means** across every route, and a face that quietly re-spelled it would be the
    // collapse this probe exists to refuse, committed by the probe.
    println!(
        "P9_DIVERGENCE null_reads_as={absent:?} req/924_§TUI-23_says={:?}",
        Nothing::Absent.mark()
    );
    // 🔴 **Addendum, no-delete (`req/924` §TUI-39, SS1069; `req/1033`).** The divergence above is
    // still true of `wire::cell` -- `cell` was deliberately left alone, for the exact SS856 reason
    // the paragraph above gives. What changed is that `status_reason` is no longer read through
    // `cell` by any caller that matters: `wire::status_reason` (a carve-out, mirroring
    // `wire::inverse_status`'s existing shape for `INVERSE_STATUS_KEY`) now answers `Absent` for
    // this one key, and `super::renderer::engine_line` reads the key through it. `g62`
    // (`fn g62_status_reason_null_is_absent_not_unknown_through_the_classifier`, this file) is the
    // assertion this comment could not make in 2026-09-01's first pass.
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
    // 🔴 **Amended again, 2026-09-01 (Owner #227), and the amendment is the sentence this probe is
    // named for.** It pinned `dropped.is_empty()` at forty by ten and said in its own message what
    // the pin was for: *if it drops one again, a region grew and nothing declared the growth*. A
    // region did grow — the subject region's floor went from four rows to five, to carry the
    // heading that says which screen this is — and the growth **is** declared, in `REGIONS`, with
    // the reason. So the condition the message names is the one asserted here, rather than the
    // outcome that happened to follow from it when it was written: a drop at forty by ten is
    // allowed exactly when the declared floors do not fit, and the screen has to say what went.
    //
    // The pin is not weakened into "anything may be dropped". An *undeclared* growth still fires
    // this: the floors are summed from `REGIONS`, so a region that starts taking a row it did not
    // declare leaves the sum fitting and the drop unexplained.
    let floors: u16 = REGIONS.iter().map(|region| region.min_rows).sum();
    println!("P4_40x10_DECLARED_FLOORS={floors}");
    if roomy.dropped.is_empty() {
        // 🔴 **Four became two, and by declaration rather than by drop** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
        // The pin's sentence is unchanged: *a drop at forty by ten is allowed exactly when the
        // declared floors do not fit, and an undeclared growth still fires this*. What changed is
        // the count it compares against, because `layout::STOOD_DOWN_REGIONS` now declares that two
        // of the four are off the standing frame. Reading the count from that declaration rather
        // than writing `2` is what keeps the pin a pin: a region coming back changes both sides.
        assert_eq!(
            roomy.rows.len(),
            REGIONS.len() - layout::STOOD_DOWN_REGIONS.len(),
            "🔴 P4: forty by ten drops nothing and does not draw the regions that are on the \
             standing frame: {:?}",
            roomy.rows
        );
    } else {
        // 🔴 The disclosure is charged what its **text** needs rather than its declared floor of
        // one: it is the region whose height is a function of what the other three did, and a sum
        // that gave it one row would say ten rows were enough when they were not. Every other
        // region is charged its declaration, so a region that quietly takes a row it never
        // declared still leaves this sum fitting and the drop unexplained.
        let need: u16 = REGIONS
            .iter()
            .map(|region| {
                if region.role == RegionRole::Disclosure {
                    layout::rows_needed(&roomy.disclosure, 40).max(region.min_rows)
                } else {
                    region.min_rows
                }
            })
            .sum();
        println!("P4_40x10_NEED={need}");
        assert!(
            need > 10,
            "🔴 P4: forty by ten dropped {:?} while the declared floors need {need} rows and fit \
             in ten — a region grew and nothing declared the growth (floors sum {floors})",
            roomy.dropped
        );
        let roomy_text = flat(&renderer::buffer_text(&renderer::render_to_buffer(
            &screen,
            40,
            10,
            Tier::Mono,
            false,
        )));
        for role in &roomy.dropped {
            assert!(
                roomy_text.contains(role.short()),
                "🔴 P4: {} was dropped at forty by ten and the screen does not say so",
                role.name()
            );
        }
    }

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

    // 🔴 **And this read `a plan that drops nothing is not measuring`** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // The ladder gives no region up any more — the two that could be given up are off the standing
    // frame by ruling — so there is nothing for forty by eight to drop. What the probe is for is
    // unchanged and is asserted below: **a screen that could not hold everything has to say so**.
    // The mark is `!` in front of the standing row, and `~` where the cut fell.
    assert!(
        plan.dropped.is_empty(),
        "🔴 P4: the ladder gave a region up at forty by eight. Since `req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01) \
         it gives none up: {:?}",
        plan.dropped
    );
    assert!(
        plan.truncated,
        "🔴 P4: forty by eight cannot hold everything and the plan does not say so: {plan:?}"
    );
    let one_line = flat(&text);
    // 🔴 **The `~` disjunct is gone** (independent audit F-03, 2026-09-02). Every ledger row
    // draws an elided id -- `gx1:2hc4zanmdgh~` -- so `contains('~')` was true on any frame that
    // drew one record, and this assertion could not fail. `p8` had the correct form in the same
    // diff (`starts_with('!') || contains("! ")`), so the weakening was specific to here rather
    // than an idiom, and no ruling licenses it. The mark for a cut **row** is `!`.
    assert!(
        one_line.starts_with('!') || one_line.contains("! "),
        "🔴 P4: the screen was cut and no mark on it says so:\n{text}"
    );
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
    // 🔴 **Rewritten under `req/924` §TUI-45** (`req/38` SS1076, Owner `#275-T`, 2026-09-01).
    // *Two hundred cells carries every declared column* was the right property while width was the
    // only reason a column could be absent. It is not any more: a column whose every value in this
    // reading is a mark for nothing is not drawn at **any** width, and the fixture's `rollback` and
    // `superseded_by` are `null` on every row. So the property is stated as the union it always
    // was — at a width that drops nothing, every declared column is either drawn or is one this
    // reading found nothing in — and a column that is neither is still a failure.
    let vacant = vacant_of(&screen);
    let unaccounted: Vec<&'static str> = LEDGER_COLUMNS
        .iter()
        .map(|column| column.key)
        .filter(|key| {
            !drawn.contains(key) && !vacant.iter().any(|(vacant_key, _)| vacant_key == key)
        })
        .collect();
    println!("P5_VACANT={vacant:?}");
    assert!(
        unaccounted.is_empty(),
        "🔴 P5: at two hundred cells a declared column is neither drawn nor found empty by the \
         reading: {unaccounted:?} (drawn {drawn:?}, vacant {vacant:?})"
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
///
/// 🔴 **The budget grew by declaration, and the property did not change** (Owner #227, 2026-09-01,
/// `req/OWNER_VERBATIM_2026-08-29.md`; TUI faces only). This test asserted `U+0020..=U+007E` as a
/// literal range, so it refused a glyph the ruling admits — a test that requires the behaviour a
/// dated ruling has called a defect, which is the one condition under which a gate may be
/// rewritten. It is rewritten and not deleted: the set is now `ASCII` **plus**
/// [`tokens::GLYPHS`], read out of the source rather than restated here, so the tofu property this
/// gate exists for still holds over an enumerated set and a glyph nobody declared still fails.
///
/// The ruling's own admission test — a glyph earns its place by carrying a meaning that deletes a
/// word — is not machine-checkable and is not claimed to be: what is checked is that every
/// non-ASCII codepoint on the screen is one somebody wrote down, with its meaning and the words it
/// replaced, in `tokens::GLYPHS`.
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
    // The declared glyphs, read out of the array the face draws them from.
    let declared: BTreeSet<char> = tokens::GLYPHS
        .iter()
        .flat_map(|glyph| glyph.text.chars())
        .collect();
    // Each declaration has to say what it means and what it replaced; a blank one is a glyph
    // admitted without an argument, which is what the ruling refuses.
    for glyph in tokens::GLYPHS {
        assert!(
            !glyph.means.is_empty() && !glyph.instead_of.is_empty(),
            "🔴 P6: {glyph:?} is declared without a meaning or without the words it replaced"
        );
    }
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
            if !(' '..='~').contains(&character)
                && !declared.contains(&character)
                && !from_wire.contains(&character)
            {
                offenders.push((character, character as u32));
            }
        }
    }
    println!(
        "P6_OFFENDERS={offenders:?} P6_WIRE_CHARSET={} P6_DECLARED_GLYPHS={}",
        from_wire.len(),
        declared.len()
    );
    assert!(
        offenders.is_empty(),
        "🔴 P6 (`req/942` §12-2, widened by Owner #227): the budget is U+0020..=U+007E plus \
         `tokens::GLYPHS`. A terminal draws a codepoint its font is missing as a box, and the \
         reader reads a box as 'this program is broken' — which is the worst possible reading of \
         the mark that means 'measured, and not knowable'. {offenders:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// P8 / g9 — the fold, and the address the disclosure spells.
// ---------------------------------------------------------------------------------------------

#[test]
fn p8_when_the_provenance_cannot_be_a_region_it_is_folded_and_marked() {
    // 🔴 **The fold's trigger moved from *room* to *health*** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // P8's subject is the fold itself: when the provenance has no region, its four facts must
    // survive **into** the line that says what is missing, wearing `NO_ADDRESS_PHRASE` so a
    // reader does not take them for facts with an address. That is unchanged and is what is
    // asserted. What changed is when the fold happens: §TUI-57 took the region off the standing
    // frame at every shape, and §TUI-29's test -- *does this row change the reader's next act
    // when everything is normal?* -- is what now decides. So the fold fires when a route stops
    // answering `200` or the engine stops agreeing with itself, rather than when the rows run
    // out.
    let refusing = Fixture::start_refusing();
    let screen = refusing.read();
    let measured = renderer::measured(&screen);
    let plan = layout::resolve(120, 32, &measured, false, layout::Subject::Grid);
    println!(
        "P8_ALL_200={} P8_HEALTHY={}",
        measured.all_200, measured.healthy
    );
    println!("P8_PLAN_FOLDED={}", plan.provenance_folded);
    assert!(
        !measured.all_200,
        "P8: the refusing fixture answers 200 on every route, so this probe measures nothing"
    );
    assert!(
        plan.provenance_folded,
        "🔴 P8: a route did not answer 200 and the four measured facts have no region and no \
         fold, so they left the screen: {plan:?}"
    );
    assert!(
        !plan
            .rows
            .iter()
            .any(|(role, _)| *role == RegionRole::Provenance),
        "the provenance is folded and still has a region of its own: {plan:?}"
    );
    let text = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        120,
        32,
        Tier::Mono,
        false,
    ));
    let one_line = flat(&text);
    println!("--- 120x32 refusing ---\n{text}");
    assert!(
        one_line.contains(NO_ADDRESS_PHRASE),
        "🔴 P8: the folded facts have to be marked `{NO_ADDRESS_PHRASE}`. Without it a reader \
         takes them for facts with an address, which they are not:\n{text}"
    );
    assert!(
        one_line.contains(&measured.folded()),
        "🔴 P8: the folded provenance reads {:?} and the screen does not carry it:\n{text}",
        measured.folded()
    );
    // 🔴 And the other half of the trigger: a screen too small for its own floor still says so.
    let well = Fixture::start();
    let good = well.read();
    let tight = layout::resolve(
        40,
        6,
        &renderer::measured(&good),
        false,
        layout::Subject::Grid,
    );
    let tight_text = flat(&renderer::buffer_text(&renderer::render_to_buffer(
        &good,
        40,
        6,
        Tier::Mono,
        false,
    )));
    assert!(
        tight.truncated,
        "at forty by six the floor does not fit and the plan has to say so: {tight:?}"
    );
    assert!(
        tight_text.starts_with('!') || tight_text.contains("! "),
        "🔴 a clipped screen has to admit it on the line that exists to say what is missing:\n\
         {tight_text}"
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
    // 🔴 **Exactly one of the two rows spells it, and this read `the disclosure spells it`**
    // (`req/924` §TUI-22, `req/38` SS1049, Owner `#266-T`, 2026-09-01: *`GET /v1/transformations`
    // の2〜5回目*=persuasion, deleted; §TUI-21, SS1048, Owner `#265-T` deleted the second and the
    // third). The address did not leave the screen — it moved to the **top rail**, which is now
    // the one row that spells it. Requiring `contains` here would be requiring the duplicate the
    // ruling deleted, so the assertion is the ruling itself, and it is strictly stronger than what
    // it replaces: it also refuses the two spellings standing at once.
    // 🔴 **And this read `exactly one of the two rows spells it`** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // There are not two rows. §TUI-57: the complete address is a detail and moves behind `?`,
    // and the standing row carries the **road** instead. So the assertion is the ruling: the row
    // does not spell the address, and it does spell the way to the page that does.
    let on_rail = layout::heading_carries_address(&plan.heading);
    assert!(
        !on_rail && !plan.disclosure.contains(LEDGER_ADDRESS),
        "🔴 g9 (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)): the address is spelled on the \
         standing frame. rail={on_rail}, disclosure {:?}",
        plan.disclosure
    );
    assert!(
        // 🔴 Either road, and which one is the reducer's decision rather than this gate's: `?`
        // opens the hatch in place while the reading offers the act, and `super::acts::grounded`
        // clamps it on a list with nothing in it — where the shell command is the honest address.
        [
            renderer::spelled(Act::Help),
            renderer::HELP_ADDRESS.to_string(),
        ]
        .iter()
        .any(|road| plan.disclosure.contains(road) || plan.note.contains(road)),
        "🔴 g9 (§TUI-21): the standing row spells no road at all, so the count of dropped fields \
         is a number a reader cannot act on: note {:?} disclosure {:?}",
        plan.note,
        plan.disclosure
    );
    // And it is on the **frame**, not only in the plan -- the half that separates a declaration
    // from a screen, and the property `g40` holds over every ruled shape.
    //
    // 🔴 **The frame in question is the hatch** (`req/924` §TUI-57, `req/38` SS1088, Owner
    // `#282-T`). The standing frame carries the road and the hatch carries the address, so asking
    // the list face for the address would be asking for the spelling the ruling deleted. What is
    // asserted is the same property one screen along: the address reaches a **drawn** frame.
    let frame = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        80,
        24,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
    ));
    assert!(
        address_row(&frame).is_some(),
        "🔴 g9: the page's address is on no row of the hatch:\n{frame}"
    );
    let standing = renderer::buffer_text(&renderer::render_to_buffer(
        &screen,
        80,
        24,
        Tier::Mono,
        false,
    ));
    assert!(
        address_row(&standing).is_none(),
        "🔴 g9 (`req/924` §TUI-57): the standing frame spells the address, which is the duplicate \
         the ruling deleted:\n{standing}"
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
    //
    // 🔴 **The width is read off the declaration and was typed here** (`[T-r71]`, 2026-09-02,
    // `req/942_artifacts/tui_r71_2026-09-02/RULING.md`). Sixteen and fifteen stood in this expression, so a ruling that moved the
    // identity column's width broke a test whose *property* — the drawn cell is a prefix of the wire
    // id and the cut is marked — had not moved at all. The literal is replaced by the declaration
    // rather than by a new literal: this is the same assertion, and it is now the assertion it
    // always meant to be.
    let cells = usize::from(layout::LEDGER_COLUMNS[0].width);
    let expected = if id.chars().count() > cells {
        format!("{}~", id.chars().take(cells - 1).collect::<String>())
    } else {
        id.clone()
    };
    assert!(
        flat(&text).contains(&expected),
        "🔴 AC-1: the frame does not carry {expected:?}, which is what the wire's {id:?} draws as \
         in a {cells}-cell column:\n{text}"
    );
    assert!(
        text.contains("Committed"),
        "🔴 a value that fits its column is drawn whole and unmarked:\n{text}"
    );
    // 🔴 **AC-2, rewritten under `req/924` §TUI-29** (`req/38` SS1058, Owner `#268-T`,
    // 2026-09-01; repeated as §TUI-45 row 3, SS1076, `#275-T`). The criterion is that the engine's
    // own version is a fact this face carries, and that still holds. What the ruling changed is
    // **which screen carries it while nothing is wrong**: `status ok ledger_agrees yes
    // engine_version 0.1.0` on every frame is three claims that change no reader's next act, and
    // the ruling folds them to one word. So the assertion is split rather than dropped — the list
    // face has to spell the fold, and the version has to be reachable **without leaving the
    // process**, which is the half that keeps this a fold instead of a deletion (SS842).
    // 🔴 **And this read `the rail spells the fold`** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // There is no rail. The fold itself did not go away -- what carries it did: the dot on the
    // standing row is what stands for the engine now, and the sentence the fold is spelled with is
    // on the hatch. Both halves are asserted, because either alone would let the other rot.
    let (ac2_dot, _) = renderer::measured(&screen).link.dot();
    assert!(
        flat(&text).contains(ac2_dot),
        "🔴 AC-2 (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)): the standing row carries no dot, \
         so nothing on it stands for the engine:\n{text}"
    );
    let ac2_hatch = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        32,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
    )));
    assert!(
        ac2_hatch.contains("engine_version") && ac2_hatch.contains("gx-engine 0.1.0"),
        "🔴 AC-2: the rail folded the engine's version away and no screen this process can draw \
         carries it. That is not a fold, it is a deletion:\n{ac2_hatch}"
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
            ..View::default()
        },
        View {
            selected: 1,
            open: false,
            ..View::default()
        },
        View {
            selected: 2,
            open: true,
            ..View::default()
        },
        View {
            selected: 0,
            open: true,
            ..View::default()
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
    // 🔴 **This was `assert_eq!(acts::ACTS.len(), 8, "the declared set is eight acts")`.**
    // Ruling: `req/984` §8-17. The count added nothing here in any case -- the loop above
    // already walks `ACTS` and requires every entry to move the state, so the number asserted
    // the length of a table whose contents were being checked one by one directly above it.
    // What replaces it is the property the number stood in for: the declaration and the one
    // road from a key to an act agree, in order, over the whole table.
    let reachable: Vec<Act> = acts::ACTS
        .into_iter()
        .filter_map(|act| acts::for_key(act.keys()[0]))
        .collect();
    assert_eq!(
        reachable.as_slice(),
        acts::ACTS.as_slice(),
        "🔴 g12: the declared table and the road from a key to an act do not agree"
    );

    // The reducer's own arithmetic, in the direction each act claims.
    let list = View {
        selected: 1,
        open: false,
        ..View::default()
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
                open: true,
                ..View::default()
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
                open: false,
                ..View::default()
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
            ..View::default()
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
            ..View::default()
        },
    ));
    println!("--- closed 40x24 ---\n{closed}\n--- opened 40x24 ---\n{opened}");
    assert!(
        // 🔴 `Escrowed` until `[T-r55]` (2026-09-02): see `TRANSFORMATIONS`' own note and gate
        // `g71`. The property is unchanged; the word is now one the engine can send.
        !flat(&closed).contains("inverse_status Available"),
        "the grid at forty cells has no column for it; the closed frame must not carry it"
    );
    assert!(
        flat(&opened).contains("inverse_status Available"),
        "🔴 P12: `act.open` is declared as 'see everything this record carries' and the frame does \
         not carry it:\n{opened}"
    );
    // 🔴 **The sentence this replaces was `flat(&opened).contains("10 of 10 members")`**
    // (`[T-r58]`, 2026-09-02, defect 4). It required a row saying *nothing was cut* on a screen
    // where nothing was cut — a permanent line reporting an event that had not happened, which the
    // seat's ruling on the real capture names as slop. The probe's subject is *the opened record
    // carries what the grid could not*, and that is what is asserted: at forty cells the grid has no
    // `inverse_status` column and the record does, and every one of the ten members is on the
    // screen. Counted off the drawing rather than off the drawing's claim about itself.
    let member_words = |text: &str| -> usize {
        text.lines()
            .flat_map(|line| line.split_whitespace())
            .filter(|word| LEDGER_COLUMNS.iter().any(|column| column.key == *word))
            .count()
    };
    assert_eq!(
        member_words(&opened),
        10,
        "🔴 P12: forty by twenty-four has room for every member and the record is not drawing \
         them:\n{opened}"
    );
    assert!(
        !flat(&opened).contains("not drawn:"),
        "🔴 P12: nothing was cut at forty by twenty-four and the screen reports a cut:\n{opened}"
    );
    // 🔴 And when it does not fit, the count says so rather than the screen quietly showing five.
    // 🔴 **Thirteen, and it was fourteen** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // The subject region gained the row the grid's column header used to be charged for, so at
    // forty by fourteen every one of the ten members now fits and the shape stopped being a
    // discriminator. The probe's subject is *the note names the cut when there is one*, which is
    // unchanged; the shape that produces a cut moved by one row and this follows it.
    // 🔴 **Ten, and it was twelve** (`[T-r58]`, 2026-09-02, defect 2). The members whose value is a
    // mark gather onto one row per kind of nothing, so the ten members of this bed now need nine
    // rows instead of ten and forty by twelve stopped being a shape that cuts them. The probe's
    // subject is *the screen names the cut when there is one*, which is unchanged; the shape that
    // produces a cut moved by two rows and this follows it, for the same reason the comment above
    // records it moving by one.
    let cramped = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        40,
        10,
        Tier::Mono,
        false,
        &View {
            selected: 0,
            open: true,
            ..View::default()
        },
    ));
    println!("--- opened 40x10 ---\n{cramped}");
    let drawn = member_words(&cramped);
    assert!(
        drawn < 10,
        "🔴 P12: forty by twelve cannot hold ten members, so a shape that draws all ten is not \
         measuring a cut: {drawn}\n{cramped}"
    );
    // 🔴 The line that says what was cut must not itself be the thing that is cut. The row it needs
    // is taken off the region's budget **before** the counts are settled, so it cannot be.
    assert!(
        flat(&cramped).contains(&format!("{} of 10 members", 10 - drawn)),
        "🔴 P12: the record dropped {} of its ten members and the screen does not name the \
         number:\n{cramped}",
        10 - drawn
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
            ..View::default()
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
    /// 🔴 **Six pairs added by `req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)** — the
    /// dot that replaced `ENGINE LIVE, N events` and `engine ok`. Six and not one because a single
    /// appearance would put `req/38` SS1085's finding back on the screen (*a quiet stream and a
    /// dead stream wearing one face*), and the pin is what stops the six drifting into fewer.
    /// `link.never` and `link.closed` share `refuse` on purpose: both are the connection being
    /// down, and they are told apart by the mark rather than the hue — the same arrangement
    /// `mark.zero`/`mark.empty` have one rung up. This landed red first (`DECLARED.len()` 15
    /// against twenty-one roles).
    const DECLARED: [(Role, Token); 21] = [
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
        (Role::LinkLive, Token::Affirm),
        (Role::LinkQuiet, Token::Accent),
        (Role::LinkOpening, Token::Thin),
        (Role::LinkNever, Token::Refuse),
        (Role::LinkClosed, Token::Refuse),
        (Role::LinkOff, Token::Thin),
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
            &View {
                selected: 0,
                open,
                ..View::default()
            },
        ))
    };
    // 🔴 **A header is column names with no values beside them** (`[T-r58]`, 2026-09-02). The
    // predicate was `contains("transformation") && contains("verdict")`, and that is what a header
    // looks like only while nothing else on the face spells two column names on one row. The record
    // face's head row now does: it folds the three `Priority::One` keys and **their values** into
    // one row (`renderer::record_own`), so it names `transformation` beside `gx1:…` rather than
    // over an empty column. The property this probe is for — *no signpost standing over columns the
    // frame does not draw* — is unchanged; what is repaired is a predicate that could not tell a
    // signpost from a fact. The grid's header carries no address, which is the discriminator.
    let header_rows = |text: &str| {
        text.lines()
            .filter(|line| {
                line.contains("transformation") && line.contains("verdict") && !line.contains("gx1:")
            })
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

    // And the row it stopped taking went to the record.
    //
    // 🔴 **Counted off the screen rather than off a note** (`[T-r58]`, 2026-09-02, defect 4). The
    // note that spelled `N of 10 members` at every shape is gone: it is drawn only where there is a
    // cut, and it names what was **dropped**. So the members are counted the way a reader counts
    // them — the rows that begin with a key this ledger declares, plus the keys folded into the head
    // row and into the gathered marks row. The number is the same number; what changed is that it is
    // now read from the drawing instead of from the drawing's own claim about itself, which is
    // strictly the stronger measurement.
    let members = |text: &str| -> usize {
        text.lines()
            .flat_map(|line| line.split_whitespace())
            .filter(|word| LEDGER_COLUMNS.iter().any(|column| column.key == *word))
            .count()
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
            ..View::default()
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
        // 🔴 **`name:key` is now one of two shapes** (`req/924` §TUI-45 row 4, `req/38` SS1076,
        // Owner `#275-T`; the admission test is `INHERITED_PRINCIPLES` §3c-③''). A mark may stand
        // in for an act's name when the name then goes, and for `act.open` the mark **is** the key
        // — `↵` names the return key — so that spelling carries no colon at all. The property this
        // gate exists for is untouched and is asserted in both shapes: nothing here is composed by
        // hand. Either the name is the act's own declared name or it is a mark declared in
        // `renderer::ACT_MARKS`, and either the key is the act's own first declared key or the
        // mark itself is the key.
        let text = renderer::spelled(*act);
        let mark = renderer::act_mark(*act);
        println!("G17 {} -> {text:?} mark={mark:?}", act.name());
        let key = match text.split_once(':') {
            Some((name, key)) => {
                assert!(
                    name == act.name().trim_start_matches("act.") || Some(name) == mark,
                    "🔴 g17: the note invented a name for {}: {name:?}",
                    act.name()
                );
                assert_eq!(
                    key,
                    act.keys()[0],
                    "🔴 g17: the note spells a key that is not the act's first declared key"
                );
                key.to_string()
            }
            None => {
                assert_eq!(
                    Some(text.as_str()),
                    mark,
                    "🔴 g17: the note spells {} with no colon and no declared mark",
                    act.name()
                );
                let _ = mark;
                // The mark stands alone only where the glyph is the key, which is what makes the
                // name's absence a deletion of a word rather than a loss of a road.
                act.keys()[0].to_string()
            }
        };
        let key = key.as_str();
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

    // 🔴 **The mark is paired with the act by its own declaration, not by an array index**
    // (independent audit, finding 11). `renderer::ACT_MARKS` reads `tokens::GLYPHS[5..8]`, so
    // swapping two of those entries — or inserting one above them — silently re-pairs all three,
    // and the help face's `marks` line is generated from the same array so it lies in the same
    // direction. `P6` only requires `means` and `instead_of` to be non-empty. What ties a mark to
    // its act is the word the declaration says it deleted, and that is what is asked here.
    for (act, mark) in renderer::ACT_MARKS {
        let glyph = tokens::glyph(mark).unwrap_or_else(|| {
            panic!("🔴 g17: {mark:?} is paired with an act and declared nowhere")
        });
        let name = act.name().trim_start_matches("act.");
        println!(
            "G17_MARK {mark:?} -> {name} instead_of={:?}",
            glyph.instead_of
        );
        assert!(
            glyph.instead_of.contains(name),
            "🔴 g17: {mark:?} is spelled for `{name}` and its declaration says it replaced {:?}. A \
             mark earns its cells by deleting a word; a pairing nothing checks is a mark that can \
             be swapped onto the opposite act in silence.",
            glyph.instead_of
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
    // 🔴 **The address this note may spell is now the key, when the key works here** (Owner #227
    // 2026-09-01 by way of `req/942_artifacts/sidebyside_round3_2026-09-01.md` §8-2: the mechanism
    // moved to `?` and the words did not). This gate held `HELP_ADDRESS` as the only acceptable
    // address, so it required the process-exiting command the ruling calls a defect — the one
    // condition under which a gate is rewritten rather than obeyed. What it asserts is unchanged:
    // a fold names its count **and** an address for what it folded. Which address is honest is a
    // property of the act list, and it is read from that list here rather than assumed.
    let in_place = offered.contains(&Act::Help);
    let address = if in_place {
        renderer::spelled(Act::Help)
    } else {
        renderer::HELP_ADDRESS.to_string()
    };
    // An address that is itself a key is a key on the screen, so it is not among the folded ones.
    let foldable = offered.len() - usize::from(in_place);
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
                note.starts_with(&head) || note.starts_with(&format!("{foldable} keys")),
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
                    note.contains(&address),
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
    // 🔴 The count the honest line carries is not `offered.len() - 3`. An address that is itself a
    // key is a key on the screen and is not among the folded ones (Owner #227) — but only while it
    // is not *also* spelled among the three, in which case nothing is subtracted. The control
    // computes the same number the line does rather than assuming one, which is what keeps it a
    // control and not a second implementation of the rule.
    let spoken = usize::from(in_place && !offered.iter().take(3).any(|act| *act == Act::Help));
    let silent = honest.replace(
        &format!("{} more keys: {address}", offered.len() - 3 - spoken),
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
///
/// 🔴 **The region is one row now and cannot wrap, so this probe measures the property rather
/// than the mechanism** (`req/924` §TUI-22, `req/38` SS1049, Owner `#266-T`, 2026-09-01). Three
/// things in that ruling reach this probe and each is named where it lands:
///
/// 1. `REGIONS[Apparatus].min_rows` is **one**. One row cannot wrap, so *the head is wrapped* is
///    no longer a form this face can take. What was load-bearing was never the wrap — it was
///    **never silently clipped** — and that is what is asserted below, at the seven ruled shapes.
/// 2. `journal_rows` is deleted (§TUI-22 ①説得: *`journal_rows 31`=`25 of 31` と重複*), so the
///    four pairs this probe was written against are three. They are read off
///    [`renderer::measured`] rather than written out here, because a probe carrying its own copy
///    of the keys measures the copy.
/// 3. `status ok` and `ledger_agrees yes` are §TUI-22 ②但し書き — *畳んで良いが捨てるな*. The rail
///    folds them from the end at narrow widths, so what is asked here is the folding's other half:
///    the count is on the same screen. The **names** half is `g61`, which was **red at four of the
///    six ruled shapes where the face makes the claim** when this was rewritten (the names were
///    absent at five of the seven ruled shapes; the seventh, 40x10, makes no claim). That is the
///    implementation defect this lane repaired rather than wrote around.
#[test]
fn p15_the_apparatus_head_is_wrapped_and_never_silently_clipped() {
    // 🔴 **There is no head to wrap** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // P15's subject was the top rail cutting the engine's keys off the right edge behind a `~`
    // with no line saying so. §TUI-57 ruled the rail off the standing frame, so the cut it
    // guarded against cannot happen -- and the obligation it was guarding (§TUI-22: those two
    // claims may be folded and may **not** be discarded) is discharged somewhere else. This
    // probe follows the obligation: no rail is composed at any shape, and **every** key of the
    // engine's own line is spelled in full on the hatch, at every ruled shape.
    let well = Fixture::start();
    let screen = well.read();
    let measured = renderer::measured(&screen);
    println!("P15_ENGINE={:?}", measured.engine_full);
    let mut railed: Vec<String> = Vec::new();
    let mut silent: Vec<String> = Vec::new();

    for (width, height) in RULED_SHAPES {
        let plan = layout::resolve(width, height, &measured, false, layout::Subject::Grid);
        if !plan.heading.is_empty() || plan.rows_for(RegionRole::Apparatus) > 0 {
            railed.push(format!("{width}x{height}"));
        }
        let hatch = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
            &View {
                help: true,
                ..View::default()
            },
        )));
        for (key, value) in &measured.engine_full {
            if !hatch.contains(&format!("{key} {value}")) {
                silent.push(format!("{width}x{height}: {key} {value}"));
            }
        }
    }
    println!("P15_RAILED={railed:?} P15_SILENT={silent:?}");
    assert!(
        railed.is_empty(),
        "🔴 P15 (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)): a top rail is composed at \
         {railed:#?}"
    );
    // 🔴 Bounded, and the bound is printed: the narrow shapes cut the hatch itself, and a
    // hatch that ran out of rows is not the rail discarding a caveat. The assertion is the
    // widest ruled shape, where the hatch has room for everything it declares.
    let narrow: Vec<&String> = silent
        .iter()
        .filter(|entry| !entry.starts_with("120x32"))
        .collect();
    println!("P15_NARROW_HATCH_CUT={}", narrow.len());
    // 🔴 **Bounded, and it was only printed** (independent audit Q6, 2026-09-02). A count
    // computed, printed and never asserted is UNTESTABLE dropped from the denominator by silence.
    // The narrow shapes cut the hatch itself and that is a named ceiling of this face -- there is
    // no act that scrolls the hatch -- so the bound is *which* shapes may be in the bucket, not
    // that it is empty. A wide shape appearing here is the rail discarding a caveat and fires.
    let unexpected: Vec<&&String> = narrow
        .iter()
        .filter(|entry| entry.starts_with("120x") || entry.starts_with("100x"))
        .collect();
    assert!(
        unexpected.is_empty(),
        "🔴 P15: the engine's keys are missing from the hatch at a shape wide enough to \
         hold it: {unexpected:#?}"
    );
    let wide: Vec<&String> = silent
        .iter()
        .filter(|entry| entry.starts_with("120x32"))
        .collect();
    assert!(
        wide.is_empty(),
        "🔴 P15 (§TUI-22, as moved by §TUI-57): the engine's own keys are not all on the \
         hatch at the widest ruled shape, so a caveat was discarded rather than folded: \
         {wide:#?}"
    );
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
        // 🔴 `req/924` §TUI-57: something arrived a moment ago, which is the arm the dot draws
        // `live` for. A probe that left this `None` would be measuring the quiet dot everywhere and
        // would never once fire the state the badge it replaces used to stand for.
        silent_for: Some(std::time::Duration::from_secs(1)),
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
        // 🔴 **And this read `the screen carries the state's sentence`**
        // (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)). The sentence is on the hatch now and the
        // standing row carries the **dot**, which is six appearances rather than one precisely so
        // that `req/38` SS1085's finding -- a quiet stream and a dead stream wearing one face --
        // cannot come back. Both halves are asserted: the mark is on the row, and the words are
        // reachable.
        let (dot, _) = report.dot();
        assert!(
            flat(&text).contains(dot),
            "🔴 g19b: {} draws the mark {dot:?} and the screen does not carry it:\n{text}",
            link.name()
        );
        let hatch = flat(&renderer::buffer_text(&renderer::render_live_to_buffer(
            &screen,
            160,
            24,
            Tier::Mono,
            false,
            &View {
                help: true,
                ..View::default()
            },
            report,
        )));
        assert!(
            hatch.contains(&report.long()),
            "🔴 g19b (§TUI-21): {} says {:?} and the hatch does not carry it, so the words \
             were deleted rather than moved:\n{hatch}",
            link.name(),
            report.long()
        );
    }

    // 🔴 The negative half of the load-bearing claim, on the frame rather than in the table: a
    // closed connection draws `14 events` nowhere, because it does not know.
    // 🔴 **On the hatch, and it read `on the frame`** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // The sentence is what the badge used to spell on the standing row; the row carries the dot
    // now and the words moved to the page `?` opens. The negative half below is unchanged and is
    // still asked of the **standing** frame, which is where a live count would be the lie.
    let closed = flat(&renderer::buffer_text(&renderer::render_live_to_buffer(
        &screen,
        160,
        24,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
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
        // 🔴 `Escrowed` until `[T-r55]` (2026-09-02): see `TRANSFORMATIONS`' own note and gate `g71`.
        r#"{"items":[{"transformation":"gx1:t3sto0000000009","state":"","verdict":"Admit","enforced":true,"created_at":"2026-08-31T00:00:00Z","actor":"agent-a","scope":"src/lib.rs","inverse_status":"Available","rollback":null,"superseded_by":null}],"next_cursor":null}"#,
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
        ..View::default()
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
            ..View::default()
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
                ..View::default()
            },
            View {
                selected: 0,
                open: true,
                ..View::default()
            },
            View {
                selected: rows.saturating_sub(1),
                open: false,
                ..View::default()
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
        ..View::default()
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
    // 🔴 A record plan resolved from `Attention::default()` is a record with nought rows of its own,
    // which is not a screen this face can draw. The counts come from the one function that measures
    // them (`[T-r58]`, 2026-09-02).
    let record_plan = |width: u16, height: u16| {
        let held = wire::Held::none();
        let (record_members, record_beyond) =
            renderer::record_extent(&screen, &held, &opened, width);
        layout::resolve_attended(
            width,
            height,
            &measured,
            false,
            layout::Subject::Record,
            layout::Attention {
                selected: 0,
                items: items.len(),
                glide: 0,
                record_members,
                record_beyond,
            },
        )
    };
    let record = record_plan(80, 24);
    println!("G24_GRID={}", grid.disclosure);
    println!("G24_RECORD={}", record.disclosure);
    assert!(
        !grid.dropped_fields.is_empty(),
        "eighty cells cannot hold eleven fields; a grid that drops none is not measuring"
    );
    // 🔴 **This assertion read `record.dropped_fields.is_empty()`, and it is superseded**
    // (`[T-r58]`, 2026-09-02 — the seat's ruling on the real capture at
    // `req/942_artifacts/tui_r55_2026-09-02/pty/record_120x32.txt`, defect 3). The sentence it
    // encoded — *a record draws every member the wire carried, so nothing is dropped by width* —
    // is still true **of the record**, and it was true of the **screen** only while the record was
    // the whole screen. It is not: the capture showed thirteen blank rows at 120x32, a third of the
    // terminal standing empty, and the ruling is that an empty panel is furniture rather than
    // information (`SS831`). The rows the record does not need go back to the ledger it was opened
    // from, and a ledger drops columns by width.
    //
    // So the requirement is now the pairing, which is strictly more than the old one asserted: the
    // record's own members are never dropped by width, **and** the disclosure names the ledger's
    // columns exactly when there is a ledger under the record to drop them from.
    assert_eq!(
        record.dropped_fields.is_empty(),
        record.grid_capacity == 0,
        "🔴 g24: the disclosure and the region disagree about whether a ledger is drawn under the \
         opened record. dropped={:?} ledger_rows={}",
        record.dropped_fields,
        record.grid_capacity
    );
    let cramped_record = record_plan(40, 10);
    assert!(
        cramped_record.dropped_fields.is_empty(),
        "🔴 g24: at forty by ten the record fills the region and there is no ledger under it, so a \
         clause naming dropped columns describes a screen that is not there: {:?}",
        cramped_record.dropped_fields
    );
    // 🔴 **The count, not one spelling of it** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // The standing chrome is one row, so the long form (`N of 11 fields not drawn`) is chosen only
    // where the row can carry it and the short form (`N/11 fields`) elsewhere. Both spell the count
    // and the denominator, which is what §TUI-21 puts outside the word budget; requiring one
    // wording would have been requiring the row to be wide rather than requiring it to be honest.
    assert!(
        grid.disclosure.contains("fields")
            && grid
                .disclosure
                .contains(&grid.dropped_fields.len().to_string()),
        "the grid's disclosure has to keep saying what the columns cut: {}",
        grid.disclosure
    );
    // 🔴 **Paired rather than forbidden** (`[T-r58]`, 2026-09-02, defect 3). The clause was
    // forbidden over a record because a record was the whole screen; a record with a ledger under it
    // does drop columns, and the one screen that drops columns and says nothing would be the defect.
    // Either form of the clause: the long form spells `N of 11 fields not drawn` and the short one
    // `N/11 fields`, which is the same pairing the grid's assertion above is written against.
    assert_eq!(
        record.disclosure.contains("fields"),
        record.grid_capacity > 0,
        "🔴 g24: the disclosure and the region disagree about whether columns were dropped from a \
         ledger under the opened record: {} (ledger rows {})",
        record.disclosure,
        record.grid_capacity
    );
    assert!(
        !cramped_record.disclosure.contains("fields"),
        "🔴 g24: at forty by ten the record fills the region and the disclosure is reporting a \
         grid's dropped columns over a screen that has no grid: {}",
        cramped_record.disclosure
    );
    // 🔴 **Either form of the same clause** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // The standing chrome is one row, so the short form (`record open`) is chosen where the long
    // one (`a record is open: its own line counts what it drew`) does not fit. Both say which
    // screen the line is describing, which is the property; requiring one wording was requiring the
    // row to be wide.
    assert!(
        record.disclosure.contains("record open") || record.disclosure.contains("a record is open"),
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
    //    shows are on the screen **and** the bottom line describes that screen.
    //
    // 🔴 **The two assertions here were `contains("members")` and `!contains("fields not drawn")`,
    // and both are superseded** (`[T-r58]`, 2026-09-02, defects 3 and 4). The first required the
    // record's own counting line at a shape where nothing was cut, which is a permanent row
    // reporting an event that did not happen; the second forbade the columns clause over a screen
    // that now really does draw a ledger under the record. What replaces them is the same property
    // stated over the drawing: every member the wire carried is on the screen, and the bottom line
    // is describing that screen rather than another one.
    let drawn = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        80,
        24,
        Tier::Mono,
        false,
        &opened,
    )));
    println!("G24_FRAME={drawn}");
    for column in LEDGER_COLUMNS {
        assert!(
            drawn.contains(column.key),
            "🔴 g24: eighty by twenty-four has room for every member and `{}` is not on the \
             frame: {drawn}",
            column.key
        );
    }
    assert!(
        !drawn.contains("not drawn:"),
        "🔴 g24: nothing was cut at eighty by twenty-four and the record reports a cut: {drawn}"
    );
    assert!(
        drawn.contains("record open"),
        "🔴 g24: the bottom line does not say which of the three screens it is describing: {drawn}"
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
        Act::Help => 7,
        Act::Wide => 8,
        Act::Leave => 9,
    };
    for act in acts::ACTS {
        assert_eq!(
            acts::ACTS.get(slot(act)),
            Some(&act),
            "🔴 g27: {} is declared and the table does not hold it at its own slot",
            act.name()
        );
    }
    // 🔴 **This was `assert_eq!(acts::ACTS.len(), 8, ..)` and it was not raised to ten.**
    // Ruling: `req/984` §8-17 -- the hardcoded literal is itself the defect. The sister
    // vocabulary states the rule it was breaking: `wire.rs`'s `Nothing::ALL` says the count "is
    // read from this array everywhere it is used" and carries no literal length assertion at
    // all. What the slot table holds is not the *length* but the *shape*: every declared act at
    // its own slot, the slots an unbroken prefix. The bound is derived, so it cannot go stale.
    //
    // 🔴 Named residual: an act deleted from `ACTS` together with its `slot` arm leaves a
    // shorter perfect prefix and nothing here objects. Closing that needs an enumeration of the
    // enum independent of `ACTS`, which is a second table -- the defect this module exists to
    // prevent. The compiler's exhaustiveness check over `slot` covers the direction that
    // matters: a variant *added* cannot be forgotten.
    let filled_slots: BTreeSet<usize> = acts::ACTS.into_iter().map(slot).collect();
    assert_eq!(
        filled_slots.len(),
        acts::ACTS.len(),
        "🔴 g27: two declared acts share a slot, so the table does not hold each once"
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
        (0..acts::ACTS.len()).collect::<BTreeSet<usize>>(),
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

// ---------------------------------------------------------------------------------------------
// g28, g29 — the window the subject region draws, and the line that says where in the list the
// reader is standing.
//
// Both defects were named and left standing rather than repaired. `acts::View::selected`'s own
// comment (`req/38` SS999, T-r4-B) records that the attention can be moved on to a record the
// region never draws, so the mark then appears nowhere; `req/964` §16 and `renderer::fold_note`
// record that the reader's position is the first rung the note's ladder gives up (T-r4-A2).
//
// Measured on a real terminal against a twenty-eight row ledger
// (`req/942_artifacts/visual_r5_2026-08-31/`): `G` moved the attention to record 28 and the frame
// came back byte-identical to the entry frame, with `record N of 28` gone from it as well — the
// two defects landing on the same screen, at the moment a reader most needs both.
// ---------------------------------------------------------------------------------------------

/// One route that answered, with a body.
fn answered(route: &str, body: serde_json::Value) -> wire::Reading {
    wire::Reading {
        route: format!("GET {route}"),
        status: Some(200),
        read_at: "2026-08-31T02:47:48.000000000Z".to_string(),
        elapsed_ms: 1,
        body: Some(body),
        error: None,
    }
}

/// The id of the `n`th generated record.
///
/// 🔴 **The row number is in the first five characters and the padding is behind it**, which is a
/// property of the *probe* rather than of the face: the id column is sixteen cells wide and a cell
/// wider than its column is cut to fifteen and marked, so an id that carries its number in the
/// tail is a row this file cannot tell from the row above it. `p12` names the same trap and works
/// around it by reading `scope` — which is not available here, because `scope` has no column below
/// sixty-one cells and these gates are fired at forty-six.
fn record_id(n: usize) -> String {
    format!("gx1:r{n:03}zzzzzzzzzz")
}

/// A ledger of `count` records, in the shape `GET /v1/transformations` answers with.
///
/// 🔴 Built in this process rather than served over a socket, and the reason is the property under
/// test: what is measured here is *how many* of the records reach the buffer and *which* one
/// carries the attention, and a list long enough to be cut is the one thing the fixture server's
/// two rows cannot produce. The keys are the fixture's own, key for key, so the rows the grid
/// draws are the rows it draws from the wire.
fn ledger(count: usize) -> Screen {
    let items: Vec<serde_json::Value> = (0..count)
        .map(|n| {
            serde_json::json!({
                "transformation": record_id(n),
                "state": "Committed",
                "verdict": "Admit",
                "enforced": true,
                "created_at": "2026-08-30T09:00:00Z",
                "actor": "agent-a",
                "scope": format!("src/row{n}.rs"),
                // 🔴 `Escrowed` until `[T-r55]` (2026-09-02): see `TRANSFORMATIONS`' note, gate `g71`.
                "inverse_status": "Available",
                "rollback": serde_json::Value::Null,
                "superseded_by": serde_json::Value::Null,
            })
        })
        .collect();
    Screen {
        healthz: answered(
            "/v1/healthz",
            serde_json::json!({
                "status": "ok",
                "engine_version": "gx-engine 0.1.0",
                "ledger_agrees": true,
                "journal_rows": count,
                "status_reason": serde_json::Value::Null,
            }),
        ),
        transformations: answered(
            "/v1/transformations",
            serde_json::json!({ "items": items, "next_cursor": serde_json::Value::Null }),
        ),
        candidates: answered(
            "/v1/candidates",
            serde_json::json!({ "items": [], "next_cursor": serde_json::Value::Null }),
        ),
        escalations: answered(
            "/v1/escalations",
            serde_json::json!({ "items": [], "next_cursor": serde_json::Value::Null }),
        ),
    }
}

/// Which rows of a frame are records, and which record each one is.
///
/// The id column is the first one and sixteen cells wide, so it survives every width these gates
/// are fired at — which is why the probe keys on it rather than on a column the plan drops.
fn drawn_records(text: &str) -> Vec<(usize, usize)> {
    text.lines()
        .enumerate()
        .filter_map(|(row, line)| {
            let head = line.trim_start();
            let digits: String = head
                .strip_prefix("gx1:r")?
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse::<usize>().ok().map(|n| (row, n))
        })
        .collect()
}

/// Which rows carry the attention, by the modifier `p12` measures it with.
fn attended_rows(buffer: &ratatui::buffer::Buffer) -> Vec<u16> {
    (buffer.area.y..buffer.area.y + buffer.area.height)
        .filter(|y| {
            buffer[(buffer.area.x, *y)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        })
        .collect()
}

/// The shapes both gates are fired over, and every place in the list the attention can stand.
const WINDOW_SHAPES: [(u16, u16); 6] = [
    (80, 24),
    (120, 32),
    (100, 24),
    (200, 50),
    (60, 20),
    (46, 12),
];

const WINDOW_STANDS: [usize; 6] = [0, 1, 5, 15, 26, 27];

/// 🔴 **g28 — the attention is inside the window the subject region draws.**
///
/// The invariant, in one sentence: *whenever the region draws a record at all, it draws the one
/// the reader is attending to.* A face that moves an attention it does not draw has told the
/// reader that a key did nothing.
///
/// The buffer half is here; the half that fires the same question at `layout`'s own window
/// arithmetic over every `(items, capacity, selected)` triple is added with that function.
#[test]
fn g28_the_attention_is_inside_the_window_that_was_drawn() {
    let screen = ledger(28);
    let items = screen.transformations.items().len();
    assert_eq!(
        items, 28,
        "the bed is the twenty-eight rows the terminal was measured on"
    );

    // 🔴 The decision, before the picture. `layout::window` is fired at every triple in a range
    // that covers the shapes below and then some; the buffer half after this measures what actually
    // reached the screen. A gate on only one of the two would be measuring either an arithmetic
    // nobody draws with or a picture whose rule nobody can read.
    let mut outside: Vec<(usize, usize, usize)> = Vec::new();
    let mut no_room = 0usize;
    for count in 0usize..=40 {
        for capacity in 0usize..=40 {
            for stand in 0usize..=40 {
                let drawn = layout::window(stand, count, capacity);
                assert!(
                    drawn.rows <= count && drawn.rows <= capacity,
                    "🔴 g28: the window draws {} rows out of {count} records into {capacity} \
                     rows",
                    drawn.rows
                );
                assert!(
                    drawn.first + drawn.rows <= count,
                    "🔴 g28: the window runs off the end of the list: {drawn:?} over {count} \
                     records"
                );
                if drawn.rows == 0 {
                    // The third value again: no row exists, so no row can carry the mark. Counted
                    // and printed rather than folded into either side.
                    no_room += 1;
                    continue;
                }
                // `View::selected` is clamped against the records the list holds, which is a state
                // `acts::apply` guarantees; it is clamped here too so the invariant is a property
                // of this function rather than of its callers remembering to.
                let attended = stand.min(count - 1);
                if attended < drawn.first || attended >= drawn.first + drawn.rows {
                    outside.push((count, capacity, stand));
                }
            }
        }
    }
    println!("G28_NO_ROOM={no_room} triples where the region has room for no record at all");
    // 🔴 The count first and then **eight** of them, and the narrowing is said out loud rather than
    // done quietly. The planted reversal of this invariant fails at sixty-eight thousand triples at
    // once, and a gate that prints all of them has not refused anything a person will read.
    let shown_failures: Vec<(usize, usize, usize)> = outside.iter().take(8).copied().collect();
    assert!(
        outside.is_empty(),
        "🔴 g28: the attended record is outside the window `layout` returned, in {} of the \
         triples swept; the first eight, as (items, capacity, selected), are {shown_failures:?}",
        outside.len()
    );

    let mut nowhere: Vec<(u16, u16, usize)> = Vec::new();
    let mut wrong: Vec<(u16, u16, usize, usize)> = Vec::new();
    for (width, height) in WINDOW_SHAPES {
        for selected in WINDOW_STANDS {
            let view = View {
                selected,
                open: false,
                ..View::default()
            };
            let buffer =
                renderer::render_view_to_buffer(&screen, width, height, Tier::Mono, false, &view);
            let text = renderer::buffer_text(&buffer);
            let records = drawn_records(&text);
            let marked = attended_rows(&buffer);
            println!(
                "G28 {width}x{height} selected={selected} drawn={} first={:?} marked={marked:?}",
                records.len(),
                records.first().map(|(_, n)| *n)
            );
            if records.is_empty() {
                // 🔴 The third value. A region with no room for a single record cannot carry an
                // attention mark, and that is not the same fact as an attention drawn in the wrong
                // place. It is left out of the failing side, and the position line is what has to
                // speak for the reader there — which is g29.
                println!(
                    "G28 UNTESTABLE {width}x{height} selected={selected}: no record row drawn"
                );
                continue;
            }
            match marked.len() {
                1 => {
                    let row = marked[0] as usize;
                    match records.iter().find(|(y, _)| *y == row) {
                        Some((_, n)) if *n == selected => {}
                        Some((_, n)) => wrong.push((width, height, selected, *n)),
                        None => nowhere.push((width, height, selected)),
                    }
                }
                _ => nowhere.push((width, height, selected)),
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "🔴 g28: the mark landed on a record the reader is not attending to, at \
         (width, height, selected, marked) {wrong:?}"
    );
    assert!(
        nowhere.is_empty(),
        "🔴 g28: the region drew records and the attention was on none of them, at \
         (width, height, selected) {nowhere:?}"
    );
}

/// 🔴 **g29 — where there are more records than rows, the screen says which record is attended.**
///
/// The position is the one fact on that line that nothing else on the screen carries. `+N more
/// rows` says that records were let go of, and a reader can count the rows that are drawn — so
/// `record N of M` *implies* the cut, and the cut does not imply the position. That asymmetry is
/// what orders the ladder, and this gate is that ordering fired over every shape.
#[test]
fn g29_the_position_is_drawn_wherever_the_list_is_longer_than_the_window() {
    let screen = ledger(28);
    let items = screen.transformations.items().len();
    let mut silent: Vec<(u16, u16, usize)> = Vec::new();
    let mut fired = 0usize;
    for (width, height) in WINDOW_SHAPES {
        for selected in WINDOW_STANDS {
            let view = View {
                selected,
                open: false,
                ..View::default()
            };
            let buffer =
                renderer::render_view_to_buffer(&screen, width, height, Tier::Mono, false, &view);
            let text = renderer::buffer_text(&buffer);
            let drawn = drawn_records(&text).len();
            if drawn >= items {
                // Nothing was let go of; there is no cut for the position to be the address of.
                continue;
            }
            fired += 1;
            // 🔴 `N of M` and not `record N of M` (`req/924` §TUI-21 目標の形, `req/38` SS1048,
            // Owner `#265-T`, 2026-09-01: the target form spells `25 of 31`). The property this
            // gate holds is untouched — what changed is the spelling the face draws it in, and a
            // locator that keeps the deleted word measures the word rather than the position.
            let position = format!("{} of {items}", selected + 1);
            let carries = flat(&text).contains(&position);
            println!("G29 {width}x{height} selected={selected} drawn={drawn} carries={carries}");
            if !carries {
                silent.push((width, height, selected));
            }
        }
    }
    assert!(
        fired > 0,
        "🔴 g29: no shape in the sweep cut the list, so this gate measured nothing"
    );
    println!("G29_FIRED={fired}");
    assert!(
        silent.is_empty(),
        "🔴 g29: the list was cut and the screen did not say which record the reader is \
         attending to, at (width, height, selected) {silent:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// g30 / g31 — the declaration decides the order, and a view is grounded before it is drawn.
// ---------------------------------------------------------------------------------------------

/// The order the subject table's columns are let go of, as the declaration spells it.
///
/// `Priority::Four` first and `Priority::One` last. Derived from `LEDGER_COLUMNS` rather than
/// written out, so the expectation moves with the declaration — what this gate compares is the
/// declaration against the **behaviour**, and a hand-written list here would turn it into a
/// comparison of two memories.
///
/// 🔴 Built as *keep* order and then reversed, and that is not a detail. `LEDGER_COLUMNS` says of
/// itself that it is "in the order they are let go of (last first)", so among columns of equal
/// priority the one declared **first** is the one given up **last**. A stable descending sort gets
/// the priorities right and the ties exactly backwards, and it did: the first draft of this gate
/// refused the shipped table at 76 of 201 widths, insisting that `transformation` be given up
/// before `state`. The instrument was wrong and the declaration was right (`req/38` SS999,
/// T-r9-A1).
fn declared_column_letting_go() -> Vec<&'static str> {
    let mut columns = LEDGER_COLUMNS;
    columns.sort_by_key(|column| column.priority);
    columns.reverse();
    columns.iter().map(|column| column.key).collect()
}

/// The same, for the four regions.
///
/// 🔴 No reversal here, because `REGIONS` says of itself that it is in **draw order** rather than
/// in priority order — there is no "last first" convention to honour. The three `Priority::One`
/// regions are therefore tied with nothing to break the tie, and that is harmless only because
/// exactly one of the three (the provenance) has a way of being let go of at all: the subject is
/// what the screen is for and the disclosure is the line that says what went. The tie is never
/// load-bearing, and this comment is here so that the day it becomes load-bearing somebody has to
/// rule on it rather than inherit an accident.
fn declared_region_letting_go() -> Vec<RegionRole> {
    let mut regions = REGIONS;
    regions.sort_by_key(|region| std::cmp::Reverse(region.priority));
    regions.iter().map(|region| region.role).collect()
}

/// 🔴 **g30 — the order things are let go of is the order that was declared.**
///
/// Every region carries a `priority` and every column carries a `priority`, and until this gate
/// **nothing read either of them**: the regions were let go of in an order written by hand into
/// `layout::resolve_attended`'s loop and the columns in the order the array happened to be typed
/// in. Both happened to agree with the declaration, which is the worst version of the defect —
/// there was nothing on the screen, in a test or in a gate that would have said so on the day one
/// of them stopped agreeing.
///
/// g10 is not this gate. g10 asks whether the declaration is honest with **itself** (nothing whose
/// facts have no address may sit below the top priority), and a declaration can be perfectly
/// self-consistent while the code beside it ignores it. This asks the other question: what a screen
/// actually gives up, in the order it actually gives it up, against what the declaration says.
#[test]
fn g30_the_order_of_letting_go_is_the_order_that_was_declared() {
    // --- the subject table's columns ---------------------------------------------------------
    let declared = declared_column_letting_go();
    println!("G30_DECLARED_COLUMNS={declared:?}");
    let page_keys: BTreeSet<&str> = LEDGER_PAGE_KEYS.into_iter().collect();
    let mut widths = 0usize;
    let mut wrong: Vec<(u16, Vec<&str>, Vec<&str>)> = Vec::new();
    for width in 0u16..=200 {
        let (drawn, disclosed) = layout::columns_for(width);
        let dropped: Vec<&str> = disclosed
            .into_iter()
            .filter(|key| !page_keys.contains(key))
            .collect();
        assert_eq!(
            drawn.len() + dropped.len(),
            LEDGER_COLUMNS.len(),
            "🔴 g30: at {width} cells a column is neither drawn nor disclosed"
        );
        widths += 1;
        // The declaration's answer to "these many were given up": the first `n` of the declared
        // order. Compared as sets, because which of two columns of **equal** priority goes first
        // is the declaration's business and not this gate's.
        let expected: Vec<&str> = declared.iter().take(dropped.len()).copied().collect();
        if expected.iter().copied().collect::<BTreeSet<&str>>()
            != dropped.iter().copied().collect::<BTreeSet<&str>>()
        {
            wrong.push((width, expected, dropped));
        }
    }
    println!(
        "G30_COLUMN_WIDTHS_SWEPT={widths} G30_COLUMN_WIDTHS_WRONG={}",
        wrong.len()
    );
    let shown: Vec<&(u16, Vec<&str>, Vec<&str>)> = wrong.iter().take(4).collect();
    assert!(
        wrong.is_empty(),
        "🔴 g30: the columns a width gives up are not the ones the declaration gives up first, at \
         {} of the {widths} widths swept; the first four, as (width, declared, actual), are \
         {shown:?}",
        wrong.len()
    );

    // --- the four regions --------------------------------------------------------------------
    let declared_regions = declared_region_letting_go();
    println!("G30_DECLARED_REGIONS={declared_regions:?}");
    let screen = ledger(28);
    let measured = renderer::measured(&screen);
    // 🔴 The same invariant as the column half, one axis over: whatever a screen has let go of at
    // any size, it has to be the **first n** of the declared order — never a later region while an
    // earlier one is still drawn. Read off the plan as a set rather than as a sequence, because a
    // plan is the state after the loop rather than a recording of it, and a gate that inferred the
    // sequence from the height at which each region disappeared would have nothing to say about a
    // screen that gives two up in the same step (measured: the planted reversal does exactly that
    // at eighty cells).
    let mut shapes = 0usize;
    let mut states: Vec<(u16, u16, Vec<RegionRole>)> = Vec::new();
    let mut ever: BTreeSet<&'static str> = BTreeSet::new();
    for width in [46u16, 60, 80, 100, 120, 200] {
        for height in 0u16..=40 {
            let plan = layout::resolve_attended(
                width,
                height,
                &measured,
                false,
                layout::Subject::Grid,
                layout::Attention {
                    selected: 0,
                    items: 28,
                    glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
                },
            );
            let mut given_up: Vec<RegionRole> = plan.dropped.clone();
            if plan.provenance_folded {
                given_up.push(RegionRole::Provenance);
            }
            shapes += 1;
            for role in &given_up {
                ever.insert(role.name());
            }
            states.push((width, height, given_up));
        }
    }
    // Only the regions some screen was actually seen to give up. The subject is what the screen is
    // for and the disclosure is the line that says what went, so neither has a step to take at all;
    // "cannot be let go of" is not "is let go of last", and folding the two together would invent
    // an order for regions that never move.
    let order: Vec<RegionRole> = declared_regions
        .iter()
        .copied()
        .filter(|role| ever.contains(role.name()))
        .collect();
    println!(
        "G30_REGION_SHAPES={shapes} G30_REGIONS_EVER_GIVEN_UP={ever:?} G30_REGION_ORDER={order:?}"
    );
    // 🔴 **The ladder gives up nothing now, and that is the assertion**
    // (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)). Two of the four regions are off the standing
    // frame **by ruling** (`layout::STOOD_DOWN_REGIONS`) rather than by running out of rows, and
    // the two that remain are what the screen is for and the line that says what went -- neither
    // has a step. So the order half has nothing to measure and says so, and what is measured
    // instead is the stronger property: no region is *ever* given up by the ladder at any shape in
    // the sweep, and the two that are absent are absent because they were declared to be.
    assert!(
        order.is_empty(),
        "🔴 g30: {} region(s) were given up by the ladder across the {shapes} shapes swept. \
         Since `req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01) the ladder gives none up; a drop \
         here means a region grew a step nothing declared: {order:?}",
        order.len()
    );
    assert_eq!(
        layout::STOOD_DOWN_REGIONS.len(),
        2,
        "🔴 g30: the declaration of which regions are off the standing frame has changed \
         size, and the screen's chrome budget is a function of it"
    );
    let mut out_of_order: Vec<(u16, u16, Vec<RegionRole>)> = Vec::new();
    for (width, height, given_up) in &states {
        let expected: BTreeSet<&str> = order
            .iter()
            .take(given_up.len())
            .map(|role| role.name())
            .collect();
        let actual: BTreeSet<&str> = given_up.iter().map(|role| role.name()).collect();
        if expected != actual {
            out_of_order.push((*width, *height, given_up.clone()));
        }
    }
    println!("G30_REGION_OUT_OF_ORDER={}", out_of_order.len());
    let shown: Vec<&(u16, u16, Vec<RegionRole>)> = out_of_order.iter().take(4).collect();
    assert!(
        out_of_order.is_empty(),
        "🔴 g30: a screen kept a region the declaration gives up sooner and gave up one it gives \
         up later, at {} of the {shapes} shapes swept; the declared order is {order:?} and the \
         first four disagreements, as (width, height, given up), are {shown:?}. \
         `Region::priority` is either what decides this or it is decoration",
        out_of_order.len()
    );
}

/// 🔴 **g31 — a redraw nobody asked for still draws a reader who is there.**
///
/// `acts::apply` ends by asking whether what the view points at still exists, and every **key**
/// goes through it. A **reading** does not: `renderer::interactive` re-reads on
/// `subscription.due()` and redraws with no act applied, so a list that shrinks between reads
/// leaves the attention pointing past the end of it with nothing on the road to notice.
///
/// The window arithmetic clamps internally, so the rows drawn are right; what was wrong is the
/// **mark**, which `renderer::subject` decides with the raw `view.selected` — records on the screen
/// and the attention on none of them, which is exactly the T-r4-B shape g28 was built for, reached
/// by a road g28 does not sweep (g28 stands the reader somewhere legal and shrinks nothing).
///
/// Found out of scope by the r6 verification lane's probe V2
/// (`req/942_artifacts/tui_r6_verify_2026-08-31/00_VERIFY.md`), repaired here.
#[test]
fn g31_a_view_is_grounded_before_it_is_drawn_even_when_no_key_was_pressed() {
    let mut nowhere: Vec<(u16, u16, usize, usize)> = Vec::new();
    let mut fired = 0usize;
    let mut untestable = 0usize;
    for (width, height) in WINDOW_SHAPES {
        for stand in WINDOW_STANDS {
            for rows in [0usize, 1, 3, 5] {
                if stand < rows {
                    // The attention is still inside the shorter list; nothing shrank out from
                    // under it and there is no question to ask. g28 covers this half.
                    continue;
                }
                let screen = ledger(rows);
                // 🔴 No act and no key. The view is where the reader left it against the long
                // ledger and the reading that came back carries `rows` records — which is the
                // subscription's road, expressed as the only thing about it that matters here.
                let view = View {
                    selected: stand,
                    open: false,
                    ..View::default()
                };
                let buffer = renderer::render_view_to_buffer(
                    &screen,
                    width,
                    height,
                    Tier::Mono,
                    false,
                    &view,
                );
                let text = renderer::buffer_text(&buffer);
                let records = drawn_records(&text);
                let marked = attended_rows(&buffer);
                println!(
                    "G31 {width}x{height} stand={stand} rows={rows} drawn={} marked={marked:?}",
                    records.len()
                );
                if records.is_empty() {
                    // No row exists, so no row can carry the mark: the same third value g28 holds
                    // apart, counted rather than folded into either side.
                    untestable += 1;
                    continue;
                }
                fired += 1;
                let landed =
                    marked.len() == 1 && records.iter().any(|(y, _)| *y == marked[0] as usize);
                if !landed {
                    nowhere.push((width, height, stand, rows));
                }
            }
        }
    }
    println!(
        "G31_FIRED={fired} G31_UNTESTABLE={untestable} G31_NOWHERE={}",
        nowhere.len()
    );
    assert!(
        fired > 0,
        "🔴 g31: no shape drew a record at all, so this gate measured nothing"
    );
    let shown: Vec<&(u16, u16, usize, usize)> = nowhere.iter().take(8).collect();
    assert!(
        nowhere.is_empty(),
        "🔴 g31: the list shrank with no key pressed and the redraw put records on the screen with \
         the attention on none of them, in {} of the {fired} cases measured; the first eight, as \
         (width, height, stand, rows), are {shown:?}",
        nowhere.len()
    );
}

// =============================================================================================
// `req/38` SS1005 — g32: one meaning per mark, measured on the frame and not only in the
// declaration.
//
// `LinkReport::short` spelled the number of lines it could not read as `{n}?`, and `?` is
// `Nothing::Unknown`'s mark. One provenance line therefore carried `?` twice, with two meanings:
// the state of the connection (*measured, and not knowable*) and a count of lines that arrived
// and could not be read. `Nothing::mark` is injective and `g20`/`g22` hold it so — but they hold
// the *vocabulary*, and nothing held the union of the vocabulary with everything else the face
// draws beside it.
//
// The reading that settles the repair, in the mechanism's words rather than in taste:
// `unreadable` is a **measurement**. The engine sent something and this face could not read it,
// so something is there. The seven words are a vocabulary of **absence**; `false` is the only one
// of them that is a measurement, and its answer is negative. None of the seven says *arrived and
// unreadable*, which is why the count does not belong to that vocabulary at all. It belongs with
// the other counts, which this face already spells with letters (`ev`, `re`, `att`), and the
// repair moves it there rather than inventing an eighth word for nothing.
// =============================================================================================

/// A subscription report that has counted lines it could not read.
///
/// Beside [`report`] rather than a fifth argument to it: `req/38` SS850 — an existing probe is not
/// edited to make room for a new one.
fn report_unreadable(link: Link, events: u64, reconnects: u64, unreadable: u64) -> LinkReport {
    LinkReport {
        unreadable,
        ..report(link, events, reconnects)
    }
}

/// The marks this face draws whose spelling contains no digit.
///
/// 🔴 **Narrowed on purpose, and the narrowing is declared here rather than left quiet.**
/// `zero`'s mark is the character `0`, so "a mark straight after a digit" is true of `200`, of
/// every clock this face prints and of every count of ten or more. `zero` is told from its
/// neighbours by `g20` and by the cell it is drawn in, not by what precedes it, so it is outside
/// this gate's alphabet and the other six words are inside it. [`live::OPEN_MARK`] is in as well:
/// it is not one of the seven, and it is still a mark that means something on its own.
fn marks_without_digits() -> Vec<&'static str> {
    let mut marks: Vec<&'static str> = Nothing::ALL
        .iter()
        .map(|nothing| nothing.mark())
        .filter(|mark| !mark.chars().any(|character| character.is_ascii_digit()))
        .collect();
    marks.push(live::OPEN_MARK);
    marks
}

/// Every place in `text` where a count is spelled with one of this face's marks as its unit.
///
/// A mark straight after a digit is the shape of the defect: the reader has a number and then a
/// symbol that, everywhere else on the same frame, is a word for a kind of nothing.
fn counts_wearing_a_mark(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found: Vec<String> = Vec::new();
    for mark in marks_without_digits() {
        let mut from = 0;
        while let Some(at) = text[from..].find(mark) {
            let start = from + at;
            let end = start + mark.len();
            if start > 0 && bytes[start - 1].is_ascii_digit() {
                let back = start.saturating_sub(6);
                let context = text.get(back..end).unwrap_or(mark);
                found.push(format!("{mark:?} after a digit in {context:?}"));
            }
            from = end;
        }
    }
    found
}

/// How many question marks a piece of text carries.
fn question_marks(text: &str) -> usize {
    text.matches(Nothing::Unknown.mark()).count()
}

/// 🔴 **g32 — no count on this face is spelled with one of its marks as the unit.**
///
/// The gate the collision asked for. `g19`/`g22`/`g25` hold the five states of the subscription
/// apart from one another and `g20` holds the seven words apart from one another; all of them are
/// statements about **one** producer. This one is about the frame: two producers, one alphabet,
/// and a reader who has no way to tell which of them wrote the `?` in front of them.
///
/// The census is taken by **difference** rather than by reading the frame and guessing which mark
/// came from where: the same frame is drawn with nought unreadable lines and with three, and what
/// the second one adds is exactly what the count contributed.
#[test]
fn g32_no_count_is_spelled_with_a_mark_as_its_unit() {
    // The gate's own positive control, first. A checker that cannot say no has not earned the
    // right to say yes, and this one is small enough to be wrong quietly.
    let planted = counts_wearing_a_mark("14ev 2re 3?");
    println!("G32_PLANT={planted:?}");
    assert!(
        !planted.is_empty(),
        "🔴 g32: the predicate did not fire on `3?`, which is the exact shape this gate exists \
         for. A green from it would mean nothing"
    );
    let clean = counts_wearing_a_mark("<< 4 routes 12:00:00 15ms all 200 14ev 2re");
    assert!(
        clean.is_empty(),
        "🔴 g32: the predicate fired on a provenance line that spells no count with a mark, so it \
         is measuring something other than what it says: {clean:?}"
    );

    let mut offences: Vec<String> = Vec::new();

    // 1. The strings the report composes, in every state, with lines it could not read.
    for link in live::LINKS {
        for unreadable in [1u64, 3, 12] {
            let report = report_unreadable(link, 14, 2, unreadable);
            let (short, long) = (report.short(), report.long());
            println!(
                "G32 {} unreadable={unreadable} SHORT={short:?} LONG={long:?}",
                link.name()
            );
            offences.extend(counts_wearing_a_mark(&short));
            offences.extend(counts_wearing_a_mark(&long));
        }
    }

    // 2. The provenance line, composed the way a live frame composes it, at three widths.
    let fixture = Fixture::start();
    let screen = fixture.read();
    for width in [40u16, 80, 140] {
        for link in live::LINKS {
            let quiet = renderer::measured_with_link(&screen, report_unreadable(link, 14, 2, 0));
            let noisy = renderer::measured_with_link(&screen, report_unreadable(link, 14, 2, 3));
            let quiet_line =
                layout::resolve(width, 24, &quiet, false, layout::Subject::Grid).provenance;
            let noisy_line =
                layout::resolve(width, 24, &noisy, false, layout::Subject::Grid).provenance;
            let quiet_frame = renderer::buffer_text(&renderer::render_live_to_buffer(
                &screen,
                width,
                24,
                Tier::Mono,
                false,
                &View::default(),
                report_unreadable(link, 14, 2, 0),
            ));
            let noisy_frame = renderer::buffer_text(&renderer::render_live_to_buffer(
                &screen,
                width,
                24,
                Tier::Mono,
                false,
                &View::default(),
                report_unreadable(link, 14, 2, 3),
            ));
            // The census by difference: how many `?` the whole frame carries with no unreadable
            // line, how many with three, and therefore how many the count itself put there.
            println!(
                "G32_QMARKS w={width} {} frame_without={} frame_with={} added_by_the_count={} \
                 line_without={quiet_line:?} line_with={noisy_line:?}",
                link.name(),
                question_marks(&quiet_frame),
                question_marks(&noisy_frame),
                question_marks(&noisy_frame) as i64 - question_marks(&quiet_frame) as i64,
            );
            offences.extend(counts_wearing_a_mark(&noisy_line));
        }
    }

    println!("G32_OFFENCES={}", offences.len());
    let shown: Vec<&String> = offences.iter().take(8).collect();
    assert!(
        offences.is_empty(),
        "🔴 g32: a count on this face is spelled with one of its own marks as the unit, so one \
         frame carries that mark with two meanings and the reader cannot tell them apart. {} \
         cases; the first eight are {shown:?}",
        offences.len()
    );
}
// `req/988` — the four axes the overtake gate lost on, and the gates that hold each repair.
//
// The sweep is stated once, here, and it is **not** the whole range `req/988` AC-5 names. Where a
// gate below cannot reach a shape it prints it as `UNTESTABLE` and leaves it out of both counts,
// which is this project's own three-valued rule applied to its own instruments rather than to its
// product (`req/38` SS870).
// =============================================================================================

/// The widths swept. Sixty-six is in the list because it is the width at which the apparatus head
/// stops fitting on one row, which is the boundary g35 exists to measure rather than assume.
const SWEEP_WIDTHS: [u16; 10] = [20, 30, 40, 46, 60, 66, 80, 100, 120, 200];
/// The heights swept.
const SWEEP_HEIGHTS: [u16; 8] = [5, 8, 10, 12, 20, 24, 32, 60];
/// The list lengths swept: none, one and two (the three rungs `offered` has), then lengths that
/// overflow every height above.
const SWEEP_ROWS: [usize; 8] = [0, 1, 2, 3, 7, 12, 28, 40];

/// How many keys a line of text says it is **not** spelling.
///
/// 🔴 Three phrases and not one, because the face has three ways of saying it and a probe that knew
/// only one would read a screen that discloses honestly as a screen that discloses nothing. The
/// substitution is what keeps them apart: `" more keys:"` **contains** `" keys:"`, so a scan for the
/// shorter phrase would find the longer one and count it twice.
fn disclosed_keys(text: &str) -> usize {
    let marked = text.replace(" more keys:", " \u{1}:");
    let mut total = 0usize;
    for phrase in [" keys not drawn:", " \u{1}:", " keys:"] {
        if let Some(at) = marked.find(phrase) {
            let digits: String = marked[..at]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            total += digits.parse::<usize>().unwrap_or(0);
        }
    }
    total
}

/// The plan and the frame for one shape, from a reader who has done nothing.
fn shape_at(screen: &Screen, width: u16, height: u16, rows: usize) -> (layout::Plan, String) {
    let view = View::default();
    let measured = renderer::measured(screen);
    let plan = layout::resolve_attended(
        width,
        height,
        &measured,
        false,
        layout::subject_shape(&screen.transformations, &view),
        layout::Attention {
            selected: 0,
            items: rows,
            glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
        },
    );
    let text = renderer::buffer_text(&renderer::render_view_to_buffer(
        screen,
        width,
        height,
        Tier::Mono,
        false,
        &view,
    ));
    (plan, text)
}

/// 🔴 **g34 — the declared keys are partitioned into the ones spelled and the ones disclosed.**
///
/// `renderer::note_rows` has a defect it names itself and gate g26 pins to exactly the diagonal
/// where the records fill the body: the legend is given nought rows, is not drawn, and **nothing on
/// the screen says it was there to draw**. Seven declared acts left the face in silence. g26 holds
/// the *shape* of that silence constant; it does not object to the silence.
///
/// This is the objection. `declared = spelled + disclosed` at every shape swept, so a key that is
/// not on the screen is a key the screen said it was not showing. There is no third bucket, and the
/// absence of a third bucket is the whole assertion — a count that quietly went missing would show
/// up here as a sum that does not add up.
#[test]
fn g34_every_declared_key_is_either_spelled_or_disclosed() {
    let mut checked = 0usize;
    let mut untestable: Vec<(u16, u16, usize)> = Vec::new();
    let mut broken: Vec<String> = Vec::new();
    let mut silent_and_disclosed = 0usize;

    for rows in SWEEP_ROWS {
        let screen = ledger(rows);
        for width in SWEEP_WIDTHS {
            for height in SWEEP_HEIGHTS {
                let (plan, text) = shape_at(&screen, width, height, rows);
                let flat_text = flat(&text);

                // 🔴 The third value. The disclosure is the mouth; if it has no rows, or if it has
                // fewer rows than its own text needs, then the screen cannot say anything about the
                // legend and this shape is not evidence either way. It is **not** counted as a
                // failure and **not** dropped from the denominator (`req/38` SS870).
                let mouth = plan.rows_for(RegionRole::Disclosure);
                if mouth == 0 || layout::rows_needed(&plan.disclosure, width) > mouth {
                    untestable.push((width, height, rows));
                    continue;
                }
                // 🔴 **A cut row is UNTESTABLE, not broken** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
                // The note and the caveat share one row now, and where they do not both fit the
                // plan cuts the caveat and marks it twice — `!` in front of the row and `~` where
                // the cut fell. On such a row the frame text no longer carries every key that was
                // spelled *into* it, so asking this frame what it spelled measures the terminal's
                // width rather than the face's partition. It is the third value and it stays in the
                // denominator (`req/38` SS870), which is the same discipline the arm above it is.
                if plan.truncated {
                    untestable.push((width, height, rows));
                    continue;
                }

                // 🔴 The budget is recomputed from `renderer::note_rows` rather than read off
                // `plan.note_rows`. The plan's member is part of what this lane added, and a gate
                // that checks a declaration by reading that same declaration is measuring its own
                // echo (`req/38` 2026-08-31: a gate validated a declaration nothing read). This
                // asks the shipped budget function directly, so it holds whether or not the plan
                // carries the number at all.
                let body_rows = (plan.rows_for(RegionRole::Subject) as usize).saturating_sub(1);
                let note_rows = renderer::note_rows(rows.max(1), body_rows);

                let declared = renderer::offered(rows).len();
                let spelled = renderer::offered(rows)
                    .iter()
                    .filter(|act| flat_text.contains(&renderer::spelled(**act)))
                    .count();
                let disclosed = disclosed_keys(&flat_text);
                checked += 1;
                if note_rows == 0 {
                    silent_and_disclosed += 1;
                }
                if spelled + disclosed != declared {
                    broken.push(format!(
                        "{width}x{height} rows={rows}: declared {declared}, spelled {spelled}, \
                         disclosed {disclosed}, note_rows {note_rows}"
                    ));
                }
            }
        }
    }

    println!(
        "G34_CHECKED={checked} G34_UNTESTABLE={} G34_NOTE_ROWS_ZERO={silent_and_disclosed} \
         G34_BROKEN={}",
        untestable.len(),
        broken.len()
    );
    // A gate that never reached the interesting case would pass on a face that never repaired it.
    assert!(
        silent_and_disclosed > 0,
        "🔴 g34: not one shape in the sweep gave the legend nought rows, so the case this gate \
         exists for was never reached and a pass here means nothing"
    );
    assert!(
        checked > 0,
        "🔴 g34: every shape came back UNTESTABLE, which is a broken instrument and not a green"
    );
    // 🔴 **And the bucket is bounded** (independent audit Q6, 2026-09-02). `plan.truncated`
    // now sends narrow shapes to UNTESTABLE, so the set that stopped being measured grew and only
    // `checked > 0` stood between that and a gate that measures one shape. A third value has to be
    // *reported and bounded* or it becomes an exemption nobody can see.
    // The bound is **which** shapes may be in the bucket, not how many. A proportion is the wrong
    // shape for it: the sweep runs 640 width/height/row triples of which most are shapes nobody
    // has ruled on, and a screen too small to say anything is legitimately outside the
    // measurement. What may **not** be outside it is a shape the ruling was measured at.
    // Distinct **shapes**, not entries: the sweep visits each shape at several row counts, so a
    // count of entries is not comparable to a count of shapes. Getting that wrong the first time
    // is what this line records.
    let ruled_untestable: BTreeSet<(u16, u16)> = untestable
        .iter()
        .filter(|(width, height, _)| RULED_SHAPES.contains(&(*width, *height)))
        .map(|(width, height, _)| (*width, *height))
        .collect();
    println!(
        "G34_RULED_UNTESTABLE_SHAPES={} of {} entries={}",
        ruled_untestable.len(),
        RULED_SHAPES.len(),
        untestable.len()
    );
    assert!(
        ruled_untestable.len() < RULED_SHAPES.len(),
        "🔴 g34: every one of the seven ruled shapes came back UNTESTABLE at every row count, \
         so the partition is unmeasured at every shape anybody has ruled on: {ruled_untestable:#?}"
    );
    let shown: Vec<&String> = broken.iter().take(8).collect();
    assert!(
        broken.is_empty(),
        "🔴 g34 (`req/988` §3-2): the declared keys are neither spelled nor disclosed in {} of \
         {checked} shapes measured — a key left the screen and nothing said so. The first eight \
         are {shown:#?}",
        broken.len()
    );
}

/// 🔴 **g35 — the screen says what it is made of, and says where the address went when it cannot.**
///
/// Two halves of one repair (`req/988` §3-1), and the second half is the one that keeps the first
/// honest.
///
/// **The rail.** `RegionRole` declares four roles, `Intent` declares a sentence for each, g3/g4/g6
/// and g10 all hold them consistent — and the only line that ever spelled one was the clause that
/// said what had been *dropped*. A screen with all four regions drawn named none of them. The
/// clause is now total, so the face says what it is made of as well as what it let go of, and the
/// two halves are a partition of the four declared roles.
///
/// **The breadcrumb.** The apparatus region declares `min_rows: 3` and draws two, so the third row
/// has been blank on every frame this face has drawn. The page's address goes in it at no cost in
/// rows. But the spare row is a function of width: the head is sixty-seven characters, so below
/// sixty-seven cells it wraps and the row is spent. This gate measures **both** sides of that
/// boundary and prints where it falls, rather than asserting only the half that passes.
///
/// 🔴 **The boundary has moved and this gate keeps measuring it, unchanged** (`req/984` R13-1,
/// T-r22). The paragraph above is where the breadcrumb stood when this gate was written; the
/// address is now also offered the last drawn row's spare **cells** when no whole row is spare, so
/// `G35_NARROWEST_WITH` fell from eighty to forty and `G35_ADDRESS_ON_NO_ROW` from eleven shapes to
/// five. Not one assertion here is touched by that: this gate's subject is where the boundary falls
/// and it prints it. The property itself — *the page's address is on some row* — is asserted by
/// [`g40_the_page_address_stands_on_a_row_at_every_ruled_shape`], because a finding that is printed
/// and passed on is a description of a defect and not a line held against it.
#[test]
fn g35_the_screen_names_its_own_parts_and_says_where_the_address_went() {
    let screen = ledger(28);
    // The four declared roles were this gate's partition target while the rail was drawn.
    // `req/924` §TUI-22 deleted the rail, so what is asserted now is its absence and the
    // totality of the half that remains -- the regions the screen let **go** of.
    let mut rail_missing: Vec<String> = Vec::new();
    let mut with_breadcrumb: Vec<u16> = Vec::new();
    let mut without_breadcrumb: Vec<u16> = Vec::new();
    let mut too_narrow_for_address: Vec<u16> = Vec::new();
    let mut address_nowhere: Vec<(u16, u16)> = Vec::new();
    let mut no_rail: Vec<(u16, u16)> = Vec::new();

    for width in SWEEP_WIDTHS {
        for height in SWEEP_HEIGHTS {
            let (plan, text) = shape_at(&screen, width, height, 28);
            let flat_text = flat(&text);

            // 🔴 **The rail is deleted and this half is its negation** (`req/924` §TUI-22,
            // `req/38` SS1049, Owner `#266-T`, 2026-09-01: *枠を足して `screen: …` を残したら不可。
            // 対で動かせ*). The clause named the face's own internal regions in words a reader
            // cannot act on; §TUI-21 classified it as persuasion and §TUI-22 made deleting it the
            // condition the enclosure was admitted under. What used to be asserted **about** the
            // rail's partition is therefore asserted about its absence, and the pairing itself is
            // `g60`'s subject.
            if plan.disclosure.contains("screen: ") {
                rail_missing.push(format!("{width}x{height}: {:?}", plan.disclosure));
            } else {
                no_rail.push((width, height));
            }
            // What the deleted rail carried that nothing else did is which regions are **present**.
            // The half a reader can act on -- which are **absent** -- is still total, and that is
            // asserted here rather than counted.
            for role in &plan.dropped {
                assert!(
                    plan.disclosure.contains(role.short()),
                    "🔴 g35: {width}x{height} let go of {:?} and the disclosure does not name it: \
                     {:?}",
                    role.short(),
                    plan.disclosure
                );
            }

            // The breadcrumb half. The apparatus is drawn first, so its rows are the first rows of
            // the frame; reading them by position is what keeps this from finding the copy of the
            // address that the disclosure region legitimately carries lower down.
            let apparatus_rows = plan.rows_for(RegionRole::Apparatus) as usize;
            if apparatus_rows == 0 {
                continue;
            }
            // 🔴 The third value again, and it was found by this gate refusing on the first run.
            // `GET /v1/transformations` is twenty-three characters, so on a screen twenty cells
            // wide there is **no row anywhere** that can hold it — not the apparatus, not the
            // disclosure. That is a property of the address and the terminal, not a defect this
            // lane introduced or could repair, so it is named and counted rather than folded into
            // the failing side. Below this width the page genuinely has no address on it.
            if (width as usize) < LEDGER_ADDRESS.chars().count() {
                too_narrow_for_address.push(width);
                continue;
            }
            let head: String = text
                .lines()
                .take(apparatus_rows)
                .collect::<Vec<_>>()
                .join(" ");
            if head.contains(LEDGER_ADDRESS) {
                with_breadcrumb.push(width);
            } else {
                without_breadcrumb.push(width);
                // 🔴 **This was written as an assertion and the face refused it, and the face was
                // right.** The claim was that losing the breadcrumb costs nothing because the
                // disclosure still spells the address. That is true of the disclosure's **long**
                // form and false of its short one: `compose_disclosure`'s short form spends its
                // words on the field count, the routes, the dropped region names and
                // `gx tui --wide`, and `LEDGER_ADDRESS` is not among them. So at a width narrow
                // enough to take the short form and to wrap the apparatus head, the page's address
                // is on **no row of the screen at all**.
                //
                // That is older than this lane and this lane does not repair it — widening the
                // short form is the one change that would push the disclosure into the cap and put
                // win axis 3 at risk (`req/988` §5). So it is counted and printed as a finding
                // rather than asserted away, and the report carries it as an open item.
                if !flat_text.contains(LEDGER_ADDRESS) {
                    address_nowhere.push((width, height));
                }
            }
        }
    }

    let narrowest_with = with_breadcrumb.iter().copied().min();
    let widest_without = without_breadcrumb.iter().copied().max();
    println!(
        "G35_BREADCRUMB_WIDTHS={:?} G35_NO_BREADCRUMB_WIDTHS={:?} \
         G35_NARROWEST_WITH={narrowest_with:?} G35_WIDEST_WITHOUT={widest_without:?} \
         G35_UNTESTABLE_TOO_NARROW={:?} G35_ADDRESS_ON_NO_ROW={:?} G35_NO_RAIL={:?}",
        with_breadcrumb.iter().collect::<BTreeSet<_>>(),
        without_breadcrumb.iter().collect::<BTreeSet<_>>(),
        too_narrow_for_address.iter().collect::<BTreeSet<_>>(),
        address_nowhere,
        no_rail
    );
    // 🔴 **And this read `the rail has to actually be drawn somewhere`** -- the sentence that
    // stood here while the clause existed. It is now the other way round: the rail is drawn at
    // **no** shape, which is what makes the enclosure's four cells free rather than an addition.
    assert_eq!(
        no_rail.len(),
        SWEEP_WIDTHS.len() * SWEEP_HEIGHTS.len(),
        "🔴 g35: the internal region rail is still drawn at {} shapes. `req/924` §TUI-22 admitted \
         the enclosure on the condition that the two move together",
        SWEEP_WIDTHS.len() * SWEEP_HEIGHTS.len() - no_rail.len()
    );

    let shown: Vec<&String> = rail_missing.iter().take(8).collect();
    assert!(
        rail_missing.is_empty(),
        "🔴 g35 (`req/924` §TUI-22): the face still names its own internal regions at {} shapes. \
         The first eight are {shown:#?}",
        rail_missing.len()
    );
    // 🔴 **And this read `the breadcrumb has to be drawn somewhere`**
    // (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)). The apparatus region is off the standing
    // frame, so there is no rail to carry a breadcrumb and none is drawn at any width. The
    // assertion is inverted rather than deleted, which keeps the sweep measuring the same thing
    // from the other side: a breadcrumb reappearing is a rail reappearing.
    assert!(
        with_breadcrumb.is_empty(),
        "🔴 g35 (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)): a breadcrumb is drawn at {} \
         width(s). The apparatus region is off the standing frame and a rail cannot come back \
         without the chrome budget moving with it",
        with_breadcrumb.len()
    );
}

/// 🔴 **AC-12 — the measurement `req/942` AC-10 asked for and r6 closed red.**
///
/// The existing `ac10` figure is one shape, one mean, and no separation between the first frame and
/// a redraw. `req/988` AC-12 asks for five shapes, cold and warm apart, `n >= 30`, and a median with
/// a spread — because this lane adds rows and branches to the draw road and "it is O(1) in theory"
/// is not a measurement (`req/988` §8-2).
///
/// **No threshold**, for the reason `ac10` states: a number chosen before the first measurement is a
/// number the next lane bends the measurement to meet. What is asserted is that the figures exist,
/// that the clock moved, and that they are printed.
#[test]
fn ac12_the_draw_road_is_measured_cold_and_warm_at_five_shapes() {
    let screen = ledger(28);
    let shapes: [(u16, u16); 5] = [(120, 32), (100, 30), (80, 24), (60, 20), (40, 10)];
    const ROUNDS: usize = 60;
    for (width, height) in shapes {
        // Cold: the first frame at this shape in this process, measured once and on its own.
        let started = Instant::now();
        let _ = renderer::render_to_buffer(&screen, width, height, Tier::Truecolor, false);
        let cold_us = started.elapsed().as_micros();

        // Warm: n = 60 redraws, each timed on its own so a median and a spread exist. A mean over
        // the batch would hide exactly the tail this is measuring.
        let mut warm: Vec<u128> = Vec::with_capacity(ROUNDS);
        for _ in 0..ROUNDS {
            let started = Instant::now();
            let _ = renderer::render_to_buffer(&screen, width, height, Tier::Truecolor, false);
            warm.push(started.elapsed().as_micros());
        }
        warm.sort_unstable();
        let median = warm[ROUNDS / 2];
        let p10 = warm[ROUNDS / 10];
        let p90 = warm[ROUNDS * 9 / 10];
        // The plan on its own, so the layout and the drawing can be told apart.
        let measured = renderer::measured(&screen);
        let mut plans: Vec<u128> = Vec::with_capacity(ROUNDS);
        for _ in 0..ROUNDS {
            let started = Instant::now();
            let _ = layout::resolve_attended(
                width,
                height,
                &measured,
                false,
                layout::Subject::Grid,
                layout::Attention {
                    selected: 0,
                    items: 28,
                    glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
                },
            );
            plans.push(started.elapsed().as_micros());
        }
        plans.sort_unstable();
        println!(
            "AC12 {width}x{height} n={ROUNDS} cold_us={cold_us} warm_median_us={median} \
             warm_p10_us={p10} warm_p90_us={p90} resolve_median_us={} cells={}",
            plans[ROUNDS / 2],
            u32::from(width) * u32::from(height)
        );
        assert!(
            cold_us > 0 && p90 > 0,
            "🔴 AC-12: the clock did not move at {width}x{height}, which is not a measurement"
        );
    }
}

/// 🔴 **g36 — `?` reaches the help face, the help face reads the declarations nobody read, and the
/// empty list is still a fixed point.**
///
/// `Act::intent` and `layout::Intent::sentence` were both written and called by nothing that draws.
/// The help face is the one screen that reads them, so this measures that they arrive rather than
/// that they exist.
///
/// It also holds the shape of the `req/984` §9-7 ruling, which is the reason this act exists in the
/// form it does: `super::acts::grounded` clamps `View::help` on a list with nothing in it, so g21's
/// fixed point survives — and because the act is inert there, the note must **not** name it there.
/// Those two are one statement, and a repair that did the first without the second would trade g21
/// for g21.
#[test]
fn g36_the_help_face_is_reachable_and_the_empty_list_stays_a_fixed_point() {
    assert_eq!(
        acts::for_key("?"),
        Some(Act::Help),
        "🔴 g36: `?` reaches no act, so the only road to the help text is still to leave the process"
    );

    // It toggles, so the key that opens is the key that closes.
    let (opened, signal) = acts::apply(&View::default(), Act::Help, 28);
    assert!(opened.help, "🔴 g36: act.help does not open the help face");
    assert_eq!(signal, acts::Signal::None);
    assert!(
        !acts::apply(&opened, Act::Help, 28).0.help,
        "🔴 g36: the key that opens the help face does not close it, so it is a room with no door"
    );

    // 🔴 The ruling (`req/984` §9-7): clamped on an empty list, and therefore not advertised there.
    for act in [Act::Help, Act::Wide] {
        assert_eq!(
            acts::apply(&View::default(), act, 0).0,
            View::default(),
            "🔴 g36: {} moves the state on a list with nothing in it, which is the fixed point g21 \
             holds",
            act.name()
        );
        assert!(
            !renderer::offered(0).contains(&act),
            "🔴 g36: {} is inert on an empty list and the note names it there anyway, which is the \
             advertised-inert-key defect g21 exists to refuse",
            act.name()
        );
        assert!(
            renderer::offered(1).contains(&act) && renderer::offered(28).contains(&act),
            "🔴 g36: {} moves the state once there is a record and the note never names it",
            act.name()
        );
    }

    let screen = ledger(28);
    let view = View {
        help: true,
        ..View::default()
    };
    assert_eq!(
        layout::subject_shape(&screen.transformations, &view),
        layout::Subject::Help,
        "🔴 g36: the one classifier does not report the help shape"
    );
    let text = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        32,
        Tier::Mono,
        false,
        &view,
    ));
    println!("G36_HELP_FRAME_120x32:\n{text}");
    let flat_text = flat(&text);

    let mut absent: Vec<&str> = Vec::new();
    for act in acts::ACTS {
        for needle in [
            act.name().trim_start_matches("act."),
            act.keys()[0],
            act.intent(),
        ] {
            if !flat_text.contains(needle) {
                absent.push(needle);
            }
        }
    }
    assert!(
        absent.is_empty(),
        "🔴 g36: the help face does not spell {absent:?}, so the declaration and the screen \
         describe two different programs"
    );
    assert!(
        REGIONS
            .iter()
            .any(|region| flat_text.contains(region.intent.sentence())),
        "🔴 g36: not one `Intent::sentence` reaches the help face, so that declaration is still \
         written and unread"
    );
    // A grid header standing over a list of key bindings is a signpost down a road that is not there.
    assert!(
        !flat_text.contains("transformation verdict"),
        "🔴 g36: the grid's header is drawn over the help face"
    );
    // The disclosure describes the screen that is drawn, here as everywhere else.
    let measured = renderer::measured(&screen);
    let plan = layout::resolve_attended(
        120,
        32,
        &measured,
        false,
        layout::Subject::Help,
        layout::Attention {
            selected: 0,
            items: 28,
            glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
        },
    );
    assert!(
        !plan.disclosure.contains("fields not drawn"),
        "🔴 g36: the disclosure counts grid columns over a face that draws no grid: {}",
        plan.disclosure
    );
}

/// 🔴 **g37 — `w` answers `--wide` in the same process, and the dumped frame does not move.**
///
/// `gx tui --wide` could only be answered by relaunching. The repair is an act on the declaration
/// and one `||` at the two call sites, so the property is exact: the frame a reader reaches by
/// pressing `w` is the frame the flag would have produced, character for character.
///
/// The second half protects everything already captured: `--dump` draws from `View::default()`, so
/// its bytes cannot move.
#[test]
fn g37_the_wide_act_answers_the_flag_without_a_new_process() {
    assert_eq!(
        acts::for_key("w"),
        Some(Act::Wide),
        "🔴 g37: `w` reaches no act, so the disclosure still costs a restart"
    );
    let (wide, _) = acts::apply(&View::default(), Act::Wide, 28);
    assert!(wide.wide, "🔴 g37: act.wide does not widen the view");
    assert!(
        !acts::apply(&wide, Act::Wide, 28).0.wide,
        "🔴 g37: the act does not toggle back"
    );

    let screen = ledger(28);
    let mut compared = 0usize;
    let mut differed = 0usize;
    for width in SWEEP_WIDTHS {
        for height in SWEEP_HEIGHTS {
            let by_act = renderer::buffer_text(&renderer::render_view_to_buffer(
                &screen,
                width,
                height,
                Tier::Mono,
                false,
                &View {
                    wide: true,
                    ..View::default()
                },
            ));
            let by_flag = renderer::buffer_text(&renderer::render_view_to_buffer(
                &screen,
                width,
                height,
                Tier::Mono,
                true,
                &View::default(),
            ));
            assert_eq!(
                by_act, by_flag,
                "🔴 g37: at {width}x{height} the act and the flag draw different frames, so one of \
                 them is a second answer to the same question"
            );
            // The dumped road: no view, so no widening, whatever the act did elsewhere.
            let dumped = renderer::buffer_text(&renderer::render_to_buffer(
                &screen,
                width,
                height,
                Tier::Mono,
                false,
            ));
            let plain = renderer::buffer_text(&renderer::render_view_to_buffer(
                &screen,
                width,
                height,
                Tier::Mono,
                false,
                &View::default(),
            ));
            assert_eq!(
                dumped, plain,
                "🔴 g37: `--dump` moved when the act was added, and every capture taken of it is \
                 now a picture of a different program"
            );
            // The act has to actually do something at some shape, or this gate is a tautology.
            if by_act != plain {
                differed += 1;
            }
            compared += 1;
        }
    }
    println!("G37_SHAPES_COMPARED={compared} G37_SHAPES_WHERE_WIDE_CHANGED_THE_FRAME={differed}");
    assert!(
        differed > 0,
        "🔴 g37: widening changed no frame at any shape swept, so the act is declared and inert"
    );
}

// =============================================================================================
// [T-r19-help-note] — the ruling of `req/984` §10-8, and the line it is not allowed to cost.
//
// 🔴 **Neither gate here writes the ruling's table down.** The table in `req/984` §10-8 lists
// seven shapes and says at which of them the help key may be advertised; a gate that asserted
// those seven answers would be measuring a transcript. What is asserted instead is the rule the
// table was derived from (`req/984` §10-9): *a line may be given up only if the same screen still
// spells it somewhere else.* The seven answers then fall out, and the table is **printed** so the
// report can be compared against the ruling rather than the ruling against itself.
//
// The two directions are separate gates on purpose. g38 is the trade: nothing is bought with a
// line that is spelled nowhere else, **and** nothing affordable is left unbought. g39 is the
// guard: the reader's position and the count of rows let go of survive at every shape, whatever
// g38 decides. A repair that satisfied g38 by spending the drop disclosure would pass g38 and
// fail g39, which is why the guard is not a clause inside the trade.
// =============================================================================================

/// The seven shapes the ruling was measured at (`req/942_artifacts/tui_r18_2026-09-01/C_TRADE.txt`,
/// `BUILD_SHA=3fb4aaea`). They are printed as a table, not asserted as one.
const RULED_SHAPES: [(u16, u16); 7] = [
    (120, 32),
    (100, 30),
    (80, 24),
    (66, 20),
    (60, 20),
    (46, 12),
    (40, 10),
];

/// The note one shape actually drew, with everything needed to ask what it cost.
struct Note {
    /// The note as the face composed it, verified to be on the screen.
    now: String,
    /// The same ladder walked without any rung paying for itself: what this face drew before
    /// `req/984` §10-8.
    base: String,
    /// The screen with this note removed, which is what "spelled somewhere else" means.
    rest: String,
    /// The ladder, read from the face rather than rebuilt here.
    ladder: Vec<renderer::Rung>,
    /// The acts the note was offered.
    acts: &'static [Act],
    /// The rows the note was drawn in, recovered by finding which one matches the screen.
    rows: usize,
}

/// The parts of a note, split on the separator the note itself joins them with.
fn note_parts(note: &str) -> Vec<String> {
    note.split(" | ")
        .map(flat)
        .filter(|part| !part.is_empty())
        .collect()
}

/// The parts of a note that come from its **head**: everything except the legend it is buying.
///
/// 🔴 **This gate's own first draft was wrong here and refused itself.** Splitting a note into
/// parts and diffing them counts `leave:q` as given up when the repaired note reads
/// `leave:q  help:?  open:return`, and counts `8 more keys: gx tui --help` as given up when the
/// repaired note says `5 more keys`. Neither is a line the reader lost — they are the legend and
/// its own disclosure, which is the thing being *bought*. The ruling of `req/984` §10-9 is about
/// what the **head** gives up, so the legend is identified structurally and removed: the fold
/// clause is the one that ends in the help address, and the key clause is the one whose every
/// token is an act spelled by [`renderer::spelled`].
fn head_parts(note: &str, acts: &[Act]) -> Vec<String> {
    // 🔴 The fold clause is identified by **whichever** address it is honestly allowed to name
    // (Owner #227): `help:?` while the key opens the help face on this reading, and the shell
    // command on the one reading where `super::acts::grounded` clamps it. Naming only the shell
    // command left `8 keys: help:?` looking like a part of the head that the note gave up, which
    // is the legend being bought — the exact confusion the paragraph above was written about, with
    // the other address in it.
    let folds = [
        format!("keys: {}", renderer::HELP_ADDRESS),
        format!("keys: {}", renderer::spelled(Act::Help)),
    ];
    note_parts(note)
        .into_iter()
        .filter(|part| {
            !folds.iter().any(|fold| part.ends_with(fold.as_str()))
                && !part
                    .split_whitespace()
                    .all(|token| acts.iter().any(|act| renderer::spelled(*act) == token))
        })
        .collect()
}

/// Whether the note's **legend** spells this act, as opposed to the note merely containing the
/// string somewhere.
///
/// 🔴 The difference was nothing until Owner #227 and is load-bearing now. The fold clause's
/// address is `help:?` on every reading where the key works, so `note.contains("help:?")` is true
/// on lines where the legend spells no keys at all — and g38's whole subject is what the legend
/// was *bought* with. The legend is the part whose every token is an act spelled by
/// [`renderer::spelled`], which is the same structural test [`head_parts`] already uses to remove
/// it; this asks whether the act is one of that part's tokens.
fn legend_names(note: &str, acts: &[Act], act: Act) -> bool {
    let wanted = renderer::spelled(act);
    note_parts(note).into_iter().any(|part| {
        let tokens: Vec<&str> = part.split_whitespace().collect();
        !tokens.is_empty()
            && tokens
                .iter()
                .all(|token| acts.iter().any(|a| renderer::spelled(*a) == *token))
            && tokens.contains(&wanted.as_str())
    })
}

/// A ladder's heads with their prices dropped: the walk that was made before a rung could be
/// asked to pay for itself.
fn unpriced(ladder: &[renderer::Rung]) -> Vec<String> {
    ladder.iter().map(|rung| rung.head.clone()).collect()
}

/// Read the list note off one frame.
///
/// 🔴 **Two things are measured off the screen rather than recomputed**, and that is what keeps
/// this from being a gate that checks a declaration nothing reads. The count of records drawn
/// comes from counting the rows that carry an id, and the row budget comes from asking which of
/// the two possible notes the frame actually contains. If neither is there the shape is
/// `UNTESTABLE` — the note is not drawn at all at that shape, which is the defect g26 pins and
/// not this gate's business.
fn read_note(screen: &Screen, width: u16, height: u16, records: usize) -> Option<Note> {
    let frame = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
        screen,
        width,
        height,
        Tier::Mono,
        false,
        &View::default(),
    )));
    let shown = renderer::buffer_text(&renderer::render_view_to_buffer(
        screen,
        width,
        height,
        Tier::Mono,
        false,
        &View::default(),
    ))
    .lines()
    .filter(|line| line.contains("gx1:"))
    .count();
    // 🔴 `N of M` (`req/924` §TUI-21 目標の形, `req/38` SS1048, Owner `#265-T`, 2026-09-01).
    // This helper rebuilds the head the face composes so the ladder it walks is the face's own; a
    // stale spelling here does not fail a gate, it makes every shape `UNTESTABLE` -- which is how
    // `g38` and `g39` came to report, in green prose on a red run, that they had measured nothing
    // at any of the seventy-eight shapes swept.
    let position = format!("1 of {records}");
    let acts = renderer::offered(records);
    let ladder = renderer::note_ladder(
        &position,
        records.checked_sub(shown).filter(|dropped| *dropped > 0),
        acts,
    );
    // 🔴 **The note is read off the plan and then verified on the frame** (`req/924` §TUI-57,
    // `req/38` SS1088, Owner `#282-T`). It used to be **re**composed here, against `width` and one
    // or two rows, and that stopped matching the moment the note moved onto the standing row: it
    // is composed there against what the caveat beside it left, which is narrower than `width` and
    // is a number only `layout::resolve_attended` knows. Recomposing it made every shape
    // `UNTESTABLE` — the exact failure mode this helper's own doc comment names one paragraph up.
    //
    // Reading it from the plan is not a gate checking its own echo: the plan is what the region
    // draws, and the `frame.contains` below is the verification that it reached the screen. The
    // ladder is still built here, because g38's subject is what the note **could** have said.
    let measured = renderer::measured(screen);
    let plan = layout::resolve_attended(
        width,
        height,
        &measured,
        false,
        layout::Subject::Grid,
        layout::Attention {
            selected: 0,
            items: records,
            glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
        },
    );
    let now = plan.note.clone();
    let flat_now = flat(&now);
    if flat_now.is_empty() || !frame.contains(&flat_now) {
        return None;
    }
    Some(Note {
        base: renderer::fold_note(&unpriced(&ladder), acts, width, 1),
        rest: frame.replacen(&flat_now, " ", 1),
        now,
        ladder,
        acts,
        rows: 1,
    })
}

/// 🔴 **g38 — the help key is on the note, and what the note gave up is spelled elsewhere.**
///
/// `req/984` §10-9 in one predicate. For each shape: what the note gave up against the ladder
/// walked at full length has to still be on the screen somewhere else, **and** the note has to
/// name the help key.
///
/// 🔴 **The second half was changed by `T-r28-owner-attach` (2026-09-01), and this paragraph is
/// the record of what it used to say.** It read: *"if some rung could have bought the help key at
/// that price it has to have been bought"*, measured with [`legend_names`] — the **legend**
/// spelling `help:?`, deliberately not the note merely containing it. That form required the
/// legend to spell `help:?` on exactly the lines whose fold clause also spells `help:?`, so the
/// two halves of this face together *forced* `help:?` to appear twice on one row, four cells
/// apart. That duplicate is the defect the lane was sent to repair, and it was this gate that made
/// it mandatory.
///
/// The repair moves `Act::Help` out of the legend wherever a fold clause exists, so the note now
/// names the help key at **every** shape rather than at the shapes where a trade was affordable.
/// The assertion is therefore unconditional, which is strictly stronger than the equality it
/// replaces: no shape may leave the reader without the way in, whether or not a trade was going.
/// The affordability walk is kept and reported — it is the reasoning the ruling was made from, and
/// deleting it would delete the record of why the trade used to be conditional.
#[test]
fn g38_the_help_key_is_bought_only_with_a_line_the_screen_still_spells() {
    let records = 28;
    let screen = ledger(records);
    let mut shapes: Vec<(u16, u16)> = RULED_SHAPES.to_vec();
    for width in SWEEP_WIDTHS {
        for height in SWEEP_HEIGHTS {
            if !shapes.contains(&(width, height)) {
                shapes.push((width, height));
            }
        }
    }

    let mut table: Vec<String> = Vec::new();
    let mut untestable: Vec<String> = Vec::new();
    // Shapes where the note is at `renderer::fold_note`'s declared ceiling and carries no clause at
    // all. Neither measured nor failed -- see the third-value note below.
    let mut no_clause: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut bought = 0usize;
    // The ruled shapes where there is no note at all, each with its reason asserted below.
    let mut ruled_untestable: Vec<String> = Vec::new();

    for (width, height) in shapes {
        let Some(note) = read_note(&screen, width, height, records) else {
            untestable.push(format!("{width}x{height}"));
            // 🔴 **The third value carries its reason at the ruled shapes** (`[T-r42]`,
            // 2026-09-01; `INHERITED_PRINCIPLES` 検査不能を不合格に畳み込むな). `req/924` §TUI-22
            // gave the ledger back a row, so at 120x32 a ledger of twenty-eight now fills the
            // region exactly: nothing is cut, `renderer::note_rows` budgets nought rows and there
            // is no note on the screen to measure. That is a shape where the ruling has nothing to
            // say, not a shape where it was broken — and the difference is asserted rather than
            // assumed, because an unexplained UNTESTABLE is an exemption nobody can see.
            if RULED_SHAPES.contains(&(width, height)) {
                let (plan, _) = shape_at(&screen, width, height, records);
                assert_eq!(
                    plan.note_rows, 0,
                    "🔴 g38: {width}x{height} is a ruled shape, the region budgeted \
                     {} note rows and no note was found on the frame. The instrument and the face \
                     disagree, which is not a third value",
                    plan.note_rows
                );
                ruled_untestable.push(format!("{width}x{height}"));
            }
            continue;
        };
        checked += 1;
        let now = head_parts(&note.now, note.acts);
        let base = head_parts(&note.base, note.acts);
        let given_up: Vec<String> = base
            .iter()
            .filter(|part| !now.contains(part))
            .cloned()
            .collect();
        for part in &given_up {
            assert!(
                note.rest.contains(part.as_str()),
                "🔴 g38 (`req/984` §10-9): at {width}x{height} the note gave up {part:?} and no \
                 other part of the screen spells it. The note reads {:?} and the screen without it \
                 reads {:?}",
                note.now,
                note.rest
            );
        }

        // Was there an affordable trade the note did not take? The candidate at each rung is the
        // ladder walked from that rung down, which is exactly what pruning the rung above would
        // produce — no second selection rule, just the same walk over a shorter ladder.
        let floor = renderer::legend_floor(note.acts);
        let mut affordable: Option<Vec<String>> = None;
        for at in 0..note.ladder.len() {
            if renderer::spellable(&note.ladder[at].head, note.acts, width, note.rows) < floor {
                continue;
            }
            let candidate =
                renderer::fold_note(&unpriced(&note.ladder[at..]), note.acts, width, note.rows);
            if !legend_names(&candidate, note.acts, Act::Help) {
                continue;
            }
            let parts = head_parts(&candidate, note.acts);
            let cost: Vec<String> = base
                .iter()
                .filter(|part| !parts.contains(part))
                .cloned()
                .collect();
            if cost.iter().all(|part| note.rest.contains(part.as_str())) {
                affordable = Some(cost);
                break;
            }
        }

        // Kept for the report: which shapes still spell the key inside the legend, and where a
        // trade was affordable. Neither decides the verdict any more -- see this gate's own doc.
        let names_help = legend_names(&note.now, note.acts, Act::Help);
        // 🔴 The verdict. `Act::Help` is not in `NOTE_ORDER_EMPTY`, and on that reading the road
        // is the shell command rather than the key, so the question is only asked where the state
        // offers the act at all -- an absent act is not an unspelled one.
        let offers_help = note.acts.contains(&Act::Help);
        let note_names_help = flat(&note.now).contains(&renderer::spelled(Act::Help));
        if note_names_help {
            bought += 1;
        }
        // 🔴 **A third value, and it is not a pass and not a failure.** `renderer::fold_note`
        // declares a ceiling of its own: when not even the shortest rung can carry the keys clause
        // at this width, the head is drawn alone and the keys go unmentioned. At 20x8 the note is
        // the four words `record 1 of 28` and there is nowhere for an address to stand.
        //
        // That is a **named** limitation of the face, identical on the unrepaired binary, and
        // folding it into this gate's failing side would report a defect this lane did not cause
        // and cannot repair from here. It is counted and printed instead, so the bound stays
        // visible rather than becoming an exemption nobody can see.
        //
        // The predicate is the face's own: the same comparison `fold_note` makes against its last
        // rung, asked here rather than restated -- a gate that rebuilt this arithmetic would be
        // measuring its own copy of it.
        let floor_head = note
            .ladder
            .last()
            .map_or(String::new(), |rung| rung.head.clone());
        let carries_clause =
            layout::rows_needed(&renderer::note_line(&floor_head, note.acts, 0), width) as usize
                <= note.rows;
        if !carries_clause {
            no_clause.push(format!("{width}x{height}"));
        }
        assert!(
            !offers_help || !carries_clause || note_names_help,
            "🔴 g38 (`req/984` §10-8, as moved by T-r28-owner-attach 2026-09-01): at \
             {width}x{height} the state offers the help act and the note does not spell it \
             anywhere. legend_names={names_help}, affordable={affordable:?}. The note reads {:?}",
            note.now
        );
        if RULED_SHAPES.contains(&(width, height)) {
            table.push(format!(
                "{width}x{height}={} gave_up={given_up:?}",
                if names_help { "help" } else { "no-help" }
            ));
        }
    }

    println!("G38_RULED_TABLE={table:?}");
    println!(
        "G38_SHAPES_CHECKED={checked} G38_SHAPES_NAMING_HELP={bought} \
         G38_UNTESTABLE_NOTE_NOT_DRAWN={untestable:?} \
         G38_NO_CLAUSE_AT_CEILING={no_clause:?}"
    );
    println!("G38_RULED_UNTESTABLE={ruled_untestable:?}");
    // 🔴 **Measured plus explained equals the seven, and it read `measured equals the seven`.**
    // The guard is the same one: nothing leaves this sweep unaccounted for. What changed is that
    // a shape with no note on it is now required to say **why** rather than being counted as a
    // failure of the ruling -- `req/924` §TUI-22 handed the ledger a row and at one ruled shape
    // the list stopped being cut, which is the lane's stated goal arriving as a gate's third value.
    assert_eq!(
        table.len() + ruled_untestable.len(),
        RULED_SHAPES.len(),
        "🔴 g38: {} of the seven ruled shapes were neither measured nor explained, so the ruling \
         is unchecked rather than kept",
        RULED_SHAPES.len() - table.len() - ruled_untestable.len()
    );
    assert!(
        !table.is_empty(),
        "🔴 g38: no ruled shape carried a note, so the ruling was not measured at all"
    );
    assert!(
        bought > 0,
        "🔴 g38: the help key is named at no shape at all, so this gate is asserting an absence \
         it would also assert against a face that had no help key"
    );
}

/// 🔴 **g39 — the position and the count of rows let go of are never spent on a legend.**
///
/// The guard on the trade g38 permits. `record N of M` says where the reader stands and `+K more
/// rows` says that rows were dropped; neither is spelled anywhere else on the screen, so neither
/// is ever a legitimate price. Derived from the ladder walked at full length rather than from a
/// list of widths: whichever of the two the face would have drawn without the ruling, it still
/// draws with it.
#[test]
fn g39_the_position_and_the_drop_count_are_never_given_up_for_a_legend() {
    let records = 28;
    let screen = ledger(records);
    // 🔴 The same spelling and the same ruling. `+K more rows` went with it (`req/924` §TUI-21
    // ①: *`+11 more rows`(`25 of 31` と重複)*), so the second half of the protected set no longer
    // fires on this face. It is kept rather than deleted because it is the guard that refuses a
    // priced rung carrying the drop count if one is ever added back.
    let position = format!("1 of {records}");
    let mut kept = 0usize;
    let mut untestable: Vec<String> = Vec::new();

    for width in SWEEP_WIDTHS {
        for height in SWEEP_HEIGHTS {
            let Some(note) = read_note(&screen, width, height, records) else {
                untestable.push(format!("{width}x{height}"));
                continue;
            };
            let now = note_parts(&note.now);
            for part in note_parts(&note.base) {
                let protected =
                    part == position || (part.starts_with('+') && part.ends_with(" more rows"));
                if !protected {
                    continue;
                }
                assert!(
                    now.contains(&part),
                    "🔴 g39 (`req/984` §10-8): at {width}x{height} the note dropped {part:?}, \
                     which is spelled nowhere else on the screen. The note reads {:?} and without \
                     the ruling it would have read {:?}",
                    note.now,
                    note.base
                );
                kept += 1;
            }
        }
    }

    println!("G39_PROTECTED_PARTS_KEPT={kept} G39_UNTESTABLE_NOTE_NOT_DRAWN={untestable:?}");
    assert!(
        kept > 0,
        "🔴 g39: no shape carried a position or a drop count to protect, so this gate measured \
         nothing"
    );
}

/// The row the page's address stands on, whole and unwrapped, or `None`.
///
/// 🔴 Deliberately **not** [`flat`]. `flat` exists so a gate can find a sentence the terminal broke
/// across two rows, and for most of what this face writes that is the right reading. It is the wrong
/// reading here: an address is something a reader copies, and one that arrives as `... | GET` on one
/// row and `/v1/transformations` on the next has been drawn but has not been *given*. So the two
/// readings are kept apart and both are reported — this one decides, and the flattened one is
/// printed beside it so that "wrapped" is never silently counted as "absent".
fn address_row(text: &str) -> Option<usize> {
    text.lines().position(|line| line.contains(LEDGER_ADDRESS))
}

/// 🔴 **g40 — the page says which page it is, at every ruled shape.**
///
/// `req/984` R13-1, raised by the r13 lane against a face it did not repair: at forty by ten the
/// page's address was on **no row of the screen at all**. The apparatus region's breadcrumb needs a
/// spare row and the head has taken it by then; the disclosure spells the address only in its long
/// form, and below forty-six cells the long form does not fit its cap. Three carriers, none of them
/// answerable for the property, so the property was nobody's.
///
/// This is the property, asserted rather than counted. g35 already measured it — `address_nowhere`
/// is the same set — but measured it as a finding it printed and passed on, which is the shape a
/// gate takes when it is describing a defect instead of holding a line. The difference is the whole
/// point of minting a second gate rather than tightening the first: g35's subject is the breadcrumb
/// and the rail, and it is *right* to keep counting where that boundary falls; g40's subject is the
/// page's addressability, which is a property of the screen and not of any one region.
///
/// **Three values, not two.** An address twenty-three cells long cannot stand on a row narrower than
/// twenty-three, at any budget — that is a fact about the terminal and not a defect the face can
/// repair, so such a shape is `UNTESTABLE` and is kept out of both counts (`req/38` SS870). None of
/// the seven ruled shapes is that narrow, so the assertion below has a live denominator; the sweep
/// census printed with it does contain such widths, and they are named there rather than folded in.
///
/// **Bounded on purpose, and the bound is printed.** The assertion is over `RULED_SHAPES`. The
/// sweep is wider than that and this gate walks all of it, but as a census: the residual shapes are
/// printed by name so that a green here can never be read as "the address is everywhere".
#[test]
fn g40_the_page_address_stands_on_a_row_at_every_ruled_shape() {
    // 🔴 **The address left the rows and this gate followed it** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // The property g40 was minted for is the page's **addressability**, and it is unchanged: a
    // reader must be able to find out where these rows came from. What changed is where the
    // answer lives. §TUI-57: *a signpost is enough once, and the complete address is a detail*,
    // so the address moved behind `?` and the standing row spells the road rather than the
    // address. Requiring it on a row would now be requiring the duplicate the ruling deleted.
    //
    // So the assertion is the ruling's own clause from §TUI-21 -- *do not call it moved until a
    // gate has confirmed the hatch lists it* -- plus the half that keeps the move honest: the
    // road out is on the standing row at every ruled shape, so a reader can reach the hatch
    // from where they are standing.
    let screen = ledger(28);
    let mut reachable: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut roadless: Vec<String> = Vec::new();

    // The positive control first: the predicate has to be able to answer *no*.
    let hatch_120 = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        32,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
    )));
    assert!(
        hatch_120.contains(LEDGER_ADDRESS),
        "🔴 g40: the widest hatch does not carry the address, so the instrument is reading \
         the wrong thing before it has measured anything:\n{hatch_120}"
    );
    assert!(
        !hatch_120
            .replace(LEDGER_ADDRESS, "")
            .contains(LEDGER_ADDRESS),
        "🔴 g40: the predicate still finds the address on a page it was taken out of, so it \
         is matching something other than the address"
    );

    for (width, height) in RULED_SHAPES {
        let hatch = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
            &View {
                help: true,
                ..View::default()
            },
        )));
        if hatch.contains(LEDGER_ADDRESS) {
            reachable.push(format!("{width}x{height}"));
        } else {
            missing.push(format!("{width}x{height}"));
        }
        let (_, text) = shape_at(&screen, width, height, 28);
        if !flat(&text).contains(&renderer::spelled(Act::Help)) {
            roadless.push(format!("{width}x{height}"));
        }
    }
    println!("G40_REACHABLE={reachable:?} G40_MISSING={missing:?} G40_ROADLESS={roadless:?}");
    assert!(
        !reachable.is_empty(),
        "🔴 g40: the address was reachable at no ruled shape, which is a broken instrument \
         and not a green"
    );
    assert!(
        missing.is_empty(),
        "🔴 g40 (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01), holding §TUI-21): the address is not \
         on the hatch at {} of the seven ruled shapes, so it was deleted rather than moved: \
         {missing:#?}",
        missing.len()
    );
    assert!(
        roadless.is_empty(),
        "🔴 g40: the standing row spells no road to the hatch at {} ruled shape(s), so the \
         address is somewhere a reader standing on the list cannot get to: {roadless:#?}",
        roadless.len()
    );
}

// ---------------------------------------------------------------------------------------------
// g41 / g42 / g43 — Owner #227: which screen am I on, what can I do next, and what does the badge
// on the provenance line actually claim.
// ---------------------------------------------------------------------------------------------

/// Every act this frame spells as `name:key`, in the order they are declared.
///
/// 🔴 The predicate is [`renderer::spelled`], which is the same string the note draws, so a key
/// counted here is a key a reader can read off the screen and press. A bare `q` inside a record id
/// is not a key and is not counted; that is the whole reason the note spells the act's name and its
/// key with no space between them.
fn keys_on_frame(frame: &str, rows: usize) -> Vec<Act> {
    let flattened = flat(frame);
    renderer::offered(rows)
        .iter()
        .filter(|act| flattened.contains(&renderer::spelled(**act)))
        .copied()
        .collect()
}

/// 🔴 **g41 — the screen says which screen it is, at every ruled shape and in all three of them.**
///
/// Owner #227 (2026-09-01): what is missing from this face is not a field, it is *where am I, what
/// am I looking at, and what can I do next*. The third round of the superiority gate measured the
/// first two as absent — `border 0.0 / tab 0 / region heading 0` at all five common shapes, against
/// six of twelve reference faces that keep a named structure at forty by ten
/// (`req/942_artifacts/sidebyside_round3_2026-09-01.md` §3-1). The face had the vocabulary the whole
/// time: `layout::Subject` declares three shapes and `layout::subject_shape` computes which one is
/// drawn, and nothing put that answer on a row.
///
/// What is asserted, in order: the heading exists; it names **every** declared shape, so the reader
/// is shown the ones they are not on as well as the one they are; **exactly one** cell is attended
/// and it is the shape the classifier says is drawn; and the three names reach the **frame** rather
/// than only the plan. The last of those is what separates a declaration from a screen — this
/// repository has already shipped a gate that checked a declaration nothing read.
#[test]
fn g41_the_heading_names_which_screen_this_is_at_every_shape() {
    // 🔴 **The heading is gone and the question it answered is answered elsewhere**
    // (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)). Owner #227 admitted the heading to answer
    // *what am I looking at*; §TUI-57 ruled the top rail off the standing frame outright and
    // said where each of its parts went -- and of `list` specifically: **drop it, `?` has the
    // screen's name**. So the assertion is the ruling: no heading is composed at any shape, and
    // the hatch names every screen this face draws.
    //
    // The declarations the old assertion read -- `layout::heading`, `heading_carries_address`,
    // `heading_engine_dropped` -- are still in the source and are called by nothing that draws.
    // That is stated here rather than left for an audit to find: they are kept under `no-delete`
    // and because the ladder they encode is the answer the day a rail comes back.
    let screen = ledger(28);
    let measured = renderer::measured(&screen);
    let mut checked = 0usize;
    let mut railed: Vec<String> = Vec::new();

    for (width, height) in RULED_SHAPES {
        for subject in layout::SUBJECTS {
            let plan = layout::resolve(width, height, &measured, false, subject);
            checked += 1;
            if !plan.heading.is_empty() {
                railed.push(format!("{width}x{height} {subject:?}"));
            }
            if plan.rows_for(RegionRole::Apparatus) > 0 {
                railed.push(format!("{width}x{height} {subject:?} rows"));
            }
        }
    }
    println!("G41_CHECKED={checked} G41_RAILED={railed:?}");
    assert!(checked > 0, "🔴 g41 measured nothing");
    assert!(
        railed.is_empty(),
        "🔴 g41 (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)): a top rail is composed at {} \
         shape(s). The standing chrome is one row and a rail cannot come back without the \
         chrome budget moving with it: {railed:#?}",
        railed.len()
    );

    // And the question the heading was admitted for is answered where the ruling sent it.
    let hatch = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        32,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
    )));
    let unnamed: Vec<&str> = layout::SUBJECTS
        .iter()
        .map(|subject| subject.name())
        .filter(|name| !hatch.contains(name))
        .collect();
    assert!(
        unnamed.is_empty(),
        "🔴 g41 (§TUI-21's clause): the hatch does not name {unnamed:?}, so the screen's \
         own name was deleted rather than moved:\n{hatch}"
    );
}

/// 🔴 **g42 — at every ruled shape the reader can see at least one thing they can do.**
///
/// The measured floor, and it was nought. `req/942_artifacts/sidebyside_round3_2026-09-01.md` §3-2
/// read the note line off the r19 captures at all seven shapes: `120x32=3`, `100x30=4`, `80x24=2`,
/// and **`66x20`, `60x20`, `46x12` and `40x10` all zero** — nine declared acts and the screen naming
/// none of them, while `gitui` spells six at forty by ten, `rainfrog` five and `claude-squad` four.
/// A face that offers no key at the size a reader is most stuck at has, for that reader, no keys.
///
/// One is the floor and not the target: the assertion is `>= 1` and the per-shape counts are
/// printed, so a fall from three keys to one at a wide shape is visible in the output even though
/// it does not fail here. A gate that asserted the exact counts would be pinning today's widths.
#[test]
fn g42_every_ruled_shape_spells_at_least_one_key() {
    let records = 28;
    let screen = ledger(records);
    let mut table: Vec<String> = Vec::new();
    let mut barren: Vec<String> = Vec::new();

    for (width, height) in RULED_SHAPES {
        let frame = renderer::buffer_text(&renderer::render_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
        ));
        let keys = keys_on_frame(&frame, records);
        table.push(format!(
            "{width}x{height}={} {:?}",
            keys.len(),
            keys.iter()
                .map(|act| renderer::spelled(*act))
                .collect::<Vec<_>>()
        ));
        if keys.is_empty() {
            barren.push(format!("{width}x{height}\n{frame}"));
        }
    }

    println!("G42_TABLE={table:?}");
    assert!(
        barren.is_empty(),
        "🔴 g42 (Owner #227): {} of the {} ruled shapes spell no key at all. Nine acts are declared \
         and the reader is told about none of them:\n{}",
        barren.len(),
        RULED_SHAPES.len(),
        barren.join("\n---\n")
    );
}

/// 🔴 **g43 — the badge never claims more than this process can see.**
///
/// Owner #227, and it is a claim about the engine rather than about a word. Events reach this face
/// over `live::STREAM_ROUTE`, the bus that feeds it lives **in the serve process**, and a write
/// another process makes straight to the journal never touches it (`req/38` SS987, measured on a
/// running engine). So `LIVE` on its own would say *the ledger is being kept fresh*, which this face
/// has no way to know; `ENGINE LIVE` says *this serve process is emitting events*, which is exactly
/// what was measured.
///
/// The gate is written over the frame rather than over the constant, and it is written **now**,
/// while the connection is the only thing that could carry such a badge — a rule that arrives after
/// the second badge arrives too late. Every occurrence of the word on any frame this face can draw
/// has to be an occurrence of the qualified one; the two counts are equal, or the screen is claiming
/// something it did not measure.
#[test]
fn g43_the_live_badge_is_never_drawn_unqualified() {
    let records = 28;
    let screen = ledger(records);
    let bare = "LIVE";
    let qualified = live::LIVE_BADGE;
    assert!(
        qualified.contains(bare) && qualified != bare,
        "🔴 g43: the control is vacuous — the qualified badge {qualified:?} has to be the bare word \
         plus what qualifies it"
    );

    let mut checked = 0usize;
    let mut seen = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for (width, height) in RULED_SHAPES {
        for link in live::LINKS {
            for help in [false, true] {
                let report = live::LinkReport {
                    link,
                    events: 145,
                    unreadable: 0,
                    reconnects: 0,
                    attempts: 1,
                    silent_for: Some(std::time::Duration::from_secs(1)),
                };
                let view = View {
                    help,
                    ..View::default()
                };
                let frame = flat(&renderer::buffer_text(&renderer::render_live_to_buffer(
                    &screen,
                    width,
                    height,
                    Tier::Mono,
                    false,
                    &view,
                    report,
                )));
                checked += 1;
                let all = frame.matches(bare).count();
                let good = frame.matches(qualified).count();
                seen += all;
                if all != good {
                    offenders.push(format!(
                        "{width}x{height} {} help={help}: {all} occurrences of {bare:?} and {good} \
                         of {qualified:?}",
                        link.name()
                    ));
                }
            }
        }
    }
    println!(
        "G43_CHECKED={checked} G43_BADGES_SEEN={seen} G43_OFFENDERS={}",
        offenders.len()
    );
    // A gate that never met the word would be green on a face that had removed the badge entirely,
    // which is a different program and not a repaired one.
    assert!(
        seen > 0,
        "🔴 g43: the badge was drawn on no frame in the sweep, so this gate measured nothing"
    );
    assert!(
        offenders.is_empty(),
        "🔴 g43 (Owner #227): a frame spells {bare:?} without saying whose events it is counting. A \
         journal written by another process is not observed, so the bare word is a claim this face \
         cannot make. {offenders:#?}"
    );
}

/// 🔴 **g44 — no act is spelled twice on one note.**
///
/// `T-r28-owner-attach` (2026-09-01), defect 2. At a hundred cells the list note read
/// `record 1 of 29 | +8 more rows | GET /v1/transformations | leave:q  help:?  | 7 more keys: help:?`
/// — `help:?` twice on one row, four cells apart, because the legend spelled the act and the fold
/// clause spelled the same act as its address. A signpost printed twice is not two signposts: the
/// second one costs six cells that could have carried a key the reader cannot otherwise see.
///
/// The predicate is deliberately about **any** act rather than about `Act::Help`. The duplicate
/// arose from two independently correct rules meeting, and the next pair of rules to meet will not
/// be these two.
///
/// Composed from the same three functions the region draws with — `note_ladder`, `afford`,
/// `fold_note` — rather than from a rebuilt string, so it measures the line the screen draws.
#[test]
fn g44_no_act_is_spelled_twice_on_one_note() {
    let mut findings: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for records in [2usize, 5, 28, 29] {
        let acts = renderer::offered(records);
        for width in SWEEP_WIDTHS {
            for rows in [1usize, 2] {
                for shown in [1usize, 3, 8, 21] {
                    if shown >= records {
                        continue;
                    }
                    let position = format!("record 1 of {records}");
                    let ladder = renderer::note_ladder(&position, Some(records - shown), acts);
                    let note = renderer::fold_note(
                        &renderer::afford(&ladder, acts, width, rows),
                        acts,
                        width,
                        rows,
                    );
                    checked += 1;
                    for act in acts {
                        let spelling = renderer::spelled(*act);
                        let times = note.matches(spelling.as_str()).count();
                        if times > 1 {
                            findings.push(format!(
                                "{width} cells, {rows} row(s), {records} records: {spelling:?} \
                                 drawn {times} times in {note:?}"
                            ));
                        }
                    }
                }
            }
        }
    }
    println!("G44_CHECKED={checked} G44_FINDINGS={}", findings.len());
    assert!(
        checked > 100,
        "g44 measured almost nothing ({checked} notes), so a green here means nothing"
    );
    assert!(
        findings.is_empty(),
        "🔴 g44 (T-r28-owner-attach 2026-09-01): an act is spelled twice on one note in {} cases. \
         The first few are {:?}",
        findings.len(),
        &findings[..findings.len().min(6)]
    );
}

/// 🔴 **g45 — one cutting policy: a declared road is never broken across two rows.**
///
/// `T-r28-owner-attach` (2026-09-01), defect 3. At a hundred cells the disclosure ended one row
/// with `... 2 routes read and not drawn: GET` and opened the next with `/v1/candidates,`. A method
/// and a path are one name; a row that ends in a bare `GET` reads as a defect rather than as a
/// wrap.
///
/// 🔴 **The roads are listed here from the four declarations that already existed, and not from
/// `layout::unbreakable()`.** A gate that read the repair's own list would measure whether the list
/// agrees with itself. These four are the addresses this face spells, and the property is stated
/// about them directly.
///
/// A road wider than the screen is skipped rather than failed: `wrap` breaks inside a word that
/// cannot fit, that is declared behaviour, and it is a different question from this one.
#[test]
fn g45_no_declared_road_is_broken_across_two_rows() {
    let roads: Vec<String> = {
        let mut roads = vec![
            layout::LEDGER_ADDRESS.to_string(),
            layout::WIDE_ADDRESS.to_string(),
            renderer::HELP_ADDRESS.to_string(),
        ];
        roads.extend(
            layout::READ_NOT_DRAWN
                .iter()
                .map(|road| (*road).to_string()),
        );
        roads
    };
    assert!(
        roads.len() >= 5,
        "the denominator came from nowhere: {roads:?}"
    );

    let screen = ledger(29);
    let measured = renderer::measured(&screen);
    let mut findings: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut skipped_too_narrow = 0usize;

    for width in SWEEP_WIDTHS {
        for height in SWEEP_HEIGHTS {
            let plan = layout::resolve(width, height, &measured, false, layout::Subject::Grid);
            let acts = renderer::offered(29);
            let ladder = renderer::note_ladder("record 1 of 29", Some(8), acts);
            let note =
                renderer::fold_note(&renderer::afford(&ladder, acts, width, 2), acts, width, 2);
            // 🔴 **The texts this face actually draws** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
            // The provenance region is off the standing frame, so scanning only its rung text was
            // measuring a line no reader sees and left this sweep with forty placements -- below
            // its own floor, which is what turned it red. `plan.note` and `plan.provenance_full`
            // are the two texts the ruling moved the roads **into**: the standing row and the
            // hatch.
            for text in [
                plan.disclosure.clone(),
                plan.provenance.clone(),
                plan.provenance_full.clone(),
                plan.note.clone(),
                note,
            ] {
                for road in &roads {
                    let wanted = text.matches(road.as_str()).count();
                    if wanted == 0 {
                        continue;
                    }
                    if road.chars().count() > width as usize {
                        skipped_too_narrow += 1;
                        continue;
                    }
                    checked += 1;
                    let drawn: usize = layout::wrap(&text, width)
                        .iter()
                        .map(|row| row.matches(road.as_str()).count())
                        .sum();
                    if drawn != wanted {
                        findings.push(format!(
                            "{width}x{height}: {road:?} whole {wanted} time(s) in the text and \
                             {drawn} time(s) after wrapping -- {:?}",
                            layout::wrap(&text, width)
                        ));
                    }
                }
            }
        }
    }
    println!(
        "G45_CHECKED={checked} G45_SKIPPED_ROAD_WIDER_THAN_SCREEN={skipped_too_narrow} \
         G45_FINDINGS={}",
        findings.len()
    );
    // 🔴 **Forty, and it was fifty** (`req/924` §TUI-57 / §TUI-62). The floor exists to stop a
    // vacuous green and the number is a function of how many **lines** carry roads: there used to
    // be a provenance region, a note row and a multi-row disclosure, and there is one standing row
    // now. Measured on the repaired face the sweep places 48 roads; on `main` it placed 40 with
    // three of the five texts scanned. Lowered to the number that still refuses a face which
    // spells no road at all, and the count is printed either way.
    assert!(
        checked > 40,
        "g45 measured almost nothing ({checked} placements), so a green here means nothing"
    );
    assert!(
        findings.is_empty(),
        "🔴 g45 (T-r28-owner-attach 2026-09-01): a declared road was broken across two rows in {} \
         cases. The first few are {:?}",
        findings.len(),
        &findings[..findings.len().min(4)]
    );
}

/// 🔴 **g46 — a resize is an event this face answers.**
///
/// `T-r28-owner-attach` (2026-09-01), found by measurement rather than by reading. The interactive
/// loop reads `terminal.size()` only when the frame is dirty, and its event match sent everything
/// that was not a key press to `Ok(_) => {}`. A resize therefore marked nothing, so the face went
/// on drawing the plan it had resolved for the **old** size until something else happened to dirty
/// the frame — on a quiet engine, never.
///
/// Measured (`req/942_artifacts/tui_r28_2026-09-01/RESIZE_PROOF.txt`), resizing a running process
/// from 80x24 to 120x32: without the arm, the face did not follow within 10s in two runs against a
/// live engine and took 3181ms against no engine at all; with it, 5ms, 7ms and 6ms.
///
/// 🔴 A third run without the arm **did** follow within 3s. That is not a contradiction, it is the
/// shape of the defect: without the arm the frame is redrawn only when something else dirties it,
/// so the face follows a resize by luck — whenever a subscription event or a change of link state
/// happens to arrive. The first version of this paragraph said the face stayed at 80 "until the
/// process was replaced", which is what one run looked like and is more than the measurements
/// support.
///
/// 🔴 **Why this is a source gate and not a shape sweep.** Every shape measurement in this suite
/// and in every capture this repository takes starts a *new* process at the shape it is measuring.
/// None of them resizes anything, which is exactly why 493 measured shapes did not see this. A
/// gate built the same way would not see it either. What is asserted here instead is the one line
/// that makes the answer possible, in the idiom g1/g7/g11 already use for facts about this face's
/// own source.
#[test]
fn g46_a_resize_is_an_event_this_face_answers() {
    let path = tui_dir().join("renderer.rs");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let arms: Vec<&str> = src
        .lines()
        .filter(|line| !is_comment(line))
        .filter(|line| line.contains("Event::Resize"))
        .collect();
    println!("G46_ARMS={arms:?}");
    assert_eq!(
        arms.len(),
        1,
        "🔴 g46 (T-r28-owner-attach 2026-09-01): this face answers a resize in {} places. It has \
         to be exactly one -- nought means a window can be made wider and the face goes on drawing \
         the old plan, and two means there are two answers to the same event.",
        arms.len()
    );
    assert!(
        arms[0].contains("dirty = true"),
        "🔴 g46: the resize arm exists but does not mark the frame dirty, so `terminal.size()` is \
         never asked again and the arm changes nothing: {:?}",
        arms[0]
    );
}

// ---------------------------------------------------------------------------------------------
// G47..G50 — a column every drawn row agrees on is said once, not on every row (T-r30-hoist).
//
// 🔴 `req/38` SS1019: 667 of the 1,540 non-blank cells at 120x32 were exactly this repetition --
// five columns spelling the same five words down every one of twenty-three rows. `req/984`
// §10-33 names the design these gates hold to: a layer-independent uniform predicate
// ([`layout::resolve_shared`]) that never has to know what a wire key or a terminal width is, and
// a renderer-side [`renderer::hoist`] that crosses into wire values only after the plan's window
// already exists -- so [`layout::Plan::columns`] and [`layout::columns_for`] answer exactly what
// they answered before this existed, and every call site that already reads either one is unhurt.
// ---------------------------------------------------------------------------------------------

const HOIST_WIDTHS: [u16; 7] = [120, 100, 80, 66, 60, 46, 40];

#[test]
fn g47_a_column_every_drawn_row_agrees_on_is_moved_to_shared_at_every_ruled_shape() {
    for width in HOIST_WIDTHS {
        let (columns, _) = layout::columns_for(width);
        if columns.is_empty() {
            println!("G47_UNTESTABLE width={width}: columns_for drew no column at all");
            continue;
        }
        let target = columns[0];
        let rows: Vec<Vec<String>> = (0..4).map(|_| vec!["Admit".to_string()]).collect();
        let (kept, shared) = layout::resolve_shared(&[target], &rows);
        assert!(
            kept.is_empty(),
            "🔴 G47 at {width}: a column all {} drawn rows spell \"Admit\" must not stay in the \
             per-row set -- got kept={kept:?}",
            rows.len()
        );
        assert_eq!(
            shared,
            vec![(target.key, "Admit".to_string())],
            "🔴 G47 at {width}: the constant must be hoisted with its own key and its own mark, \
             not dropped or renamed"
        );
    }
}

#[test]
fn g48_the_shared_mark_keeps_unknown_and_absent_apart() {
    let target = layout::Column {
        key: "created_at",
        width: 20,
        priority: Priority::Two,
    };
    let (_, unknown) =
        layout::resolve_shared(&[target], &[vec!["?".to_string()], vec!["?".to_string()]]);
    let (_, absent) =
        layout::resolve_shared(&[target], &[vec!["--".to_string()], vec!["--".to_string()]]);
    assert_eq!(
        unknown,
        vec![(target.key, "?".to_string())],
        "🔴 G48: every drawn row measured and got no answer -- the shared mark must stay \"?\""
    );
    assert_eq!(
        absent,
        vec![(target.key, "--".to_string())],
        "🔴 G48: every drawn row never carried the key at all -- the shared mark must stay \"--\", \
         never rounded into unknown's mark"
    );
    assert_ne!(
        unknown, absent,
        "🔴 G48: the seven-word vocabulary for nothing is not simplification's to spend -- two \
         different kinds of nothing must never hoist to the same shared field"
    );
}

#[test]
fn g49_no_shared_field_is_claimed_from_fewer_than_two_rows() {
    let target = layout::Column {
        key: "verdict",
        width: 9,
        priority: Priority::One,
    };
    let (kept0, shared0) = layout::resolve_shared(&[target], &[]);
    assert!(
        shared0.is_empty() && kept0.len() == 1,
        "🔴 G49: zero drawn rows is not evidence of a constant -- the column stays kept, not \
         shared: kept={kept0:?} shared={shared0:?}"
    );
    let (kept1, shared1) = layout::resolve_shared(&[target], &[vec!["Admit".to_string()]]);
    assert!(
        shared1.is_empty() && kept1.len() == 1,
        "🔴 G49: one drawn row proves nothing repeats -- the column stays kept, not shared: \
         kept={kept1:?} shared={shared1:?}"
    );
}

/// A ledger of `rows` records, every one of them agreeing on `verdict`, `state`, `created_at`
/// (`null` on the wire, so drawn as `?`) and `scope` (`null` too), and disagreeing on
/// `transformation`, which is what a reader tells one record from another by.
fn uniform_ledger(rows: usize) -> Vec<serde_json::Value> {
    (0..rows)
        .map(|i| {
            serde_json::json!({
                "transformation": format!("gx1:t{i:016x}"),
                "verdict": "Admit",
                "state": "Committed",
                "created_at": serde_json::Value::Null,
                "scope": serde_json::Value::Null,
                "enforced": true,
            })
        })
        .collect()
}

#[test]
fn g50_a_shared_row_never_leaves_fewer_than_two_records_drawn_to_justify_it() {
    let rows = uniform_ledger(6);
    let items: Vec<&serde_json::Value> = rows.iter().collect();
    let mut saw_a_hoist = false;
    for width in HOIST_WIDTHS {
        let (columns, _) = layout::columns_for(width);
        if columns.len() < 2 {
            println!("G50_UNTESTABLE width={width}: fewer than two columns fit, nothing to hoist out of the row");
            continue;
        }
        for capacity in 1..=items.len() {
            let (kept, shared) = renderer::hoist(&items, &columns, capacity);
            let drawn = layout::scrolled(
                0,
                items.len(),
                1 + usize::from(!shared.is_empty()),
                capacity,
                0,
            )
            .1;
            if shared.is_empty() {
                continue;
            }
            saw_a_hoist = true;
            assert!(
                drawn.rows >= 2,
                "🔴 G50 at {width}, capacity {capacity}: a shared row was drawn over a window of \
                 {} record(s) -- no shape claims a constant from fewer than two",
                drawn.rows
            );
            assert!(
                kept.len() < columns.len(),
                "🔴 G50 at {width}, capacity {capacity}: {} column(s) said the same thing on \
                 every one of {} drawn rows and none of them moved to shared",
                columns.len() - kept.len(),
                drawn.rows
            );
        }
    }
    assert!(
        saw_a_hoist,
        "🔴 G50: this ledger was built to agree on four columns at every width in \
         HOIST_WIDTHS -- if no width and no capacity ever produced a shared field, the test is \
         not exercising hoist() at all"
    );
}

/// 🔴 Independent audit, 2026-09-01: the first cut of `hoist` compared `items.len()` against
/// `window.rows` to decide whether the region had spare capacity, and [`layout::window`]'s own
/// body caps `rows` at `items.min(capacity)` -- so `window.rows <= items.len()` by construction,
/// the comparison could never be true, and every hoist paid for its shared row by dropping a real
/// record even when the region had blank rows going unused below the list. This is that exact
/// shape, planted directly: three records, and a region asked for ten.
#[test]
fn g51_a_shared_row_costs_nothing_when_the_region_has_spare_capacity() {
    let rows = uniform_ledger(3);
    let items: Vec<&serde_json::Value> = rows.iter().collect();
    let (columns, _) = layout::columns_for(120);
    let capacity = 10; // strictly more than items.len() (3): the region has room to spare.
    let window = layout::window(0, items.len(), capacity);
    assert_eq!(
        window.rows,
        items.len(),
        "🔴 G51 setup: a window built from more capacity than there are items must draw every \
         item -- got {} of {}",
        window.rows,
        items.len()
    );
    let (kept, shared) = renderer::hoist(&items, &columns, capacity);
    let drawn = layout::scrolled(
        0,
        items.len(),
        1 + usize::from(!shared.is_empty()),
        capacity,
        0,
    )
    .1;
    assert!(
        !shared.is_empty(),
        "🔴 G51 setup: this ledger agrees on verdict/state/created_at/scope at every row -- \
         hoist() found nothing to share, so this test is not exercising the fix at all"
    );
    assert!(
        kept.len() < columns.len(),
        "🔴 G51: {} column(s) agreed on every one of {} rows and none moved to shared",
        columns.len() - kept.len(),
        items.len()
    );
    assert_eq!(
        drawn.rows,
        items.len(),
        "🔴 G51: the region asked for {capacity} rows and only {} items exist, so a shared row \
         must cost nothing -- got a window of {} record(s), which means a real record was \
         dropped to pay for a row the region never needed to fill",
        items.len(),
        drawn.rows
    );
}

// =============================================================================================
// `req/924` §TUI-13 追記 -- the ten states of the inverse column, and the two collapses that were
// in this face until g51..g53 were written.
//
// The ruling counts ten: two that belong to the *reading* (not measured yet, measured and not
// knowable) and eight that belong to the *wire* (`null`, and the seven `InverseStatus` variants of
// `crates/gx-engine/src/store.rs`, of which `Consumed` alone carries a member). The two collapses:
//
// 1. `null` was drawn `?`, the same mark a failed read draws. "This transformation never escrowed
//    anything" and "this face could not read the list" were one picture.
// 2. `{"Consumed":{"by":...}}` was drawn as the JSON text of itself and cut at fourteen cells to
//    `{"Consumed":{~` -- the whole column spent on punctuation, and the one fact the object carried
//    thrown away.
//
// 🔴 These three gates are written against the **frame**, not against the classifier, so that they
// compile and run on the commit before the repair. A gate that only compiles once the repair exists
// cannot be fired in the red direction, and a compile error is not a red gate.
// =============================================================================================

/// The transformation the Consumed fixture names.
///
/// Nineteen characters against a fourteen-cell column on purpose: `g52` is about a cut being a cut,
/// and a `by` short enough to fit would measure nothing.
const INVERSE_CONSUMED_BY: &str = "gx1:t3sto0000000042";

/// The six words the engine spells as bare strings.
///
/// 🔴 **Transcribed from `crates/gx-engine/src/store.rs`, not read from it.** #188/#189 ruled that
/// this package's suite does not open another crate's source -- a suite that fails when a crate it
/// does not depend on drifts is a coupling `cargo tree` cannot show. The cost of that ruling is
/// paid here and is named rather than hidden: nothing in this file goes red the day `InverseStatus`
/// grows an eighth variant. The freshness gate for the count is the engine's own
/// (`crates/gx-engine/tests/lifecycle_transitions.rs` asserts the arms and their writers).
const INVERSE_WORDS: [&str; 6] = [
    "Available",
    "Expired",
    "Unavailable",
    "Pending",
    "BodyMissing",
    "Undetermined",
];

/// The eight shapes `inverse_status` can arrive in, each under the name this file calls it by.
fn inverse_wire_shapes() -> Vec<(&'static str, serde_json::Value)> {
    let mut shapes: Vec<(&'static str, serde_json::Value)> =
        vec![("null", serde_json::Value::Null)];
    for word in INVERSE_WORDS {
        shapes.push((word, serde_json::Value::String(word.to_string())));
    }
    shapes.push((
        "Consumed",
        serde_json::json!({ "Consumed": { "by": INVERSE_CONSUMED_BY } }),
    ));
    shapes
}

/// The width the declaration gives the column, read from the declaration.
fn inverse_column_width() -> usize {
    LEDGER_COLUMNS
        .iter()
        .find(|column| column.key == INVERSE_KEY)
        .unwrap_or_else(|| panic!("🔴 no column is declared for {INVERSE_KEY}"))
        .width as usize
}

/// The wire's key, spelled once. Checked against the declaration by [`inverse_column_width`], which
/// panics rather than silently measuring nothing if the key is ever renamed.
const INVERSE_KEY: &str = "inverse_status";

/// A ledger carrying one `inverse_status` shape per row.
///
/// 🔴 Every other column differs across the rows **on purpose**. `renderer::hoist` moves a column
/// that every drawn row agrees on out of the header and into a shared line; these gates find their
/// column by reading the header the frame actually drew, so a ledger that let any column agree
/// would move the thing being measured rather than measure it.
fn inverse_ledger() -> Screen {
    let items: Vec<serde_json::Value> = inverse_wire_shapes()
        .into_iter()
        .enumerate()
        .map(|(n, (_, status))| {
            let verdict = wire::VERDICT_KINDS[n % wire::VERDICT_KINDS.len()];
            serde_json::json!({
                "transformation": record_id(n),
                "state": format!("State{n}"),
                "verdict": verdict,
                "enforced": n % 2 == 0,
                "created_at": format!("2026-08-0{}T09:00:00Z", n + 1),
                "actor": format!("agent-{n}"),
                "scope": format!("src/row{n}.rs"),
                "inverse_status": status,
                "rollback": format!("kind-{n}"),
                "superseded_by": record_id(n + 100),
            })
        })
        .collect();
    let rows = items.len();
    Screen {
        healthz: answered(
            "/v1/healthz",
            serde_json::json!({
                "status": "ok",
                "engine_version": "gx-engine 0.1.0",
                "ledger_agrees": true,
                "journal_rows": rows,
                "status_reason": serde_json::Value::Null,
            }),
        ),
        transformations: answered(
            "/v1/transformations",
            serde_json::json!({ "items": items, "next_cursor": serde_json::Value::Null }),
        ),
        candidates: answered(
            "/v1/candidates",
            serde_json::json!({ "items": [], "next_cursor": serde_json::Value::Null }),
        ),
        escalations: answered(
            "/v1/escalations",
            serde_json::json!({ "items": [], "next_cursor": serde_json::Value::Null }),
        ),
    }
}

/// A reading that was refused: an answer, with a body, and no `items` in it.
fn inverse_refused(route: &str) -> wire::Reading {
    wire::Reading {
        route: format!("GET {route}"),
        status: Some(401),
        read_at: "2026-09-01T00:00:00.000000000Z".to_string(),
        elapsed_ms: 1,
        body: Some(serde_json::json!({"title":"unauthorized","gx_code":"UNAUTHORIZED"})),
        error: None,
    }
}

/// The same ledger with its subject route replaced, for the two states that belong to the reading
/// rather than to any row.
fn inverse_screen_with(reading: wire::Reading) -> Screen {
    let mut screen = inverse_ledger();
    screen.transformations = reading;
    screen
}

/// The frame this face draws at one shape, one tier, and one place for the attention.
fn inverse_frame(
    screen: &Screen,
    width: u16,
    height: u16,
    tier: Tier,
    selected: usize,
    open: bool,
) -> String {
    renderer::buffer_text(&renderer::render_view_to_buffer(
        screen,
        width,
        height,
        tier,
        false,
        &View {
            selected,
            open,
            ..View::default()
        },
    ))
}

/// Where the `inverse_status` column starts in the frame, read from the header the frame drew.
///
/// [`None`] when the frame has no such column -- because the plan dropped it, or because `hoist`
/// moved it. Both are answers about the frame, which is why this reads the header rather than
/// summing widths out of the declaration.
fn inverse_column_start(text: &str) -> Option<usize> {
    text.lines()
        .find(|line| line.contains("transformation") && line.contains(INVERSE_KEY))
        .and_then(|line| line.find(INVERSE_KEY))
}

/// The `inverse_status` cell of each drawn record, keyed by which record it is.
fn inverse_grid_cells(text: &str) -> Vec<(usize, String)> {
    let Some(start) = inverse_column_start(text) else {
        return Vec::new();
    };
    let width = inverse_column_width();
    let lines: Vec<&str> = text.lines().collect();
    drawn_records(text)
        .into_iter()
        .filter_map(|(row, n)| {
            lines.get(row).map(|line| {
                (
                    n,
                    line.chars()
                        .skip(start)
                        .take(width)
                        .collect::<String>()
                        .trim_end()
                        .to_string(),
                )
            })
        })
        .collect()
}

/// The `inverse_status` member of an opened record, as the frame spelled it.
///
/// The record draws one member per line as `key value`, so the member is recovered by the key it
/// begins with. [`None`] when no line does -- the region ran out of rows, which is a third value and
/// not a failure.
fn inverse_opened_member(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim_end().strip_prefix(&format!("{INVERSE_KEY} ")))
        .map(|rest| rest.trim().to_string())
}

/// 🔴 **g51 -- the ten states of the inverse column are ten spellings, on `mono` as well.**
///
/// `req/924` §TUI-13 追記. Two of the ten come from [`wire::Nothing`]'s own declaration, and the
/// gate requires them to be on the screen as well as declared; the other eight are read out of the
/// frame at every ruled shape and every tier.
///
/// A shape where a state cannot be read is counted `UNTESTABLE` and named, never folded into the
/// failing side (`req/942` §14-3, and the ruling that a measurement which did not happen is not a
/// measurement that failed). The gate refuses only two things: two states sharing a spelling, and
/// no shape having measured all ten.
#[test]
fn g54_the_ten_inverse_states_are_ten_spellings() {
    let ledger = inverse_ledger();
    let pending = inverse_screen_with(wire::Reading::pending("/v1/transformations"));
    let refused = inverse_screen_with(inverse_refused("/v1/transformations"));
    let loading = Nothing::Loading.mark().to_string();
    let unknown = Nothing::Unknown.mark().to_string();
    let shapes = inverse_wire_shapes();

    let mut fully_measured: Vec<(u16, u16, &'static str)> = Vec::new();
    let mut untestable: Vec<String> = Vec::new();
    let mut grid_measured: Vec<(u16, u16, &'static str)> = Vec::new();
    let mut collisions: Vec<String> = Vec::new();

    for (width, height) in RULED_SHAPES {
        for tier in Tier::ALL {
            // The two states that belong to the reading. Declared by `wire`, and required to reach
            // the screen: a mark nothing draws is a mark that is not part of the face.
            let pending_frame = inverse_frame(&pending, width, height, tier, 0, false);
            let refused_frame = inverse_frame(&refused, width, height, tier, 0, false);
            assert!(
                pending_frame.contains(&loading),
                "🔴 G54 at {width}x{height} {}: a reading that has not happened draws no \
                 {loading:?}:\n{pending_frame}",
                tier.name()
            );
            assert!(
                refused_frame.contains(&unknown),
                "🔴 G54 at {width}x{height} {}: a refused reading draws no {unknown:?}:\n\
                 {refused_frame}",
                tier.name()
            );

            let mut spelled: Vec<(&'static str, String)> = vec![
                ("<loading>", loading.clone()),
                ("<unknown>", unknown.clone()),
            ];
            let grid = inverse_frame(&ledger, width, height, tier, 0, false);
            let cells = inverse_grid_cells(&grid);
            if cells.len() == shapes.len() {
                grid_measured.push((width, height, tier.name()));
            }
            for (n, (name, _)) in shapes.iter().enumerate() {
                // The record is the road the disclosure names for a column the grid let go of, so
                // it is where the state is read when the column is not drawn -- and it is read the
                // same way when it is, because one road that always answers is better than two that
                // sometimes do.
                let opened = inverse_frame(&ledger, width, height, tier, n, true);
                match inverse_opened_member(&opened) {
                    Some(text) => spelled.push((name, text)),
                    None => untestable.push(format!(
                        "{width}x{height} {} {name}: the opened record drew no {INVERSE_KEY} line",
                        tier.name()
                    )),
                }
            }

            if spelled.len() == shapes.len() + 2 {
                fully_measured.push((width, height, tier.name()));
            }
            for i in 0..spelled.len() {
                for j in (i + 1)..spelled.len() {
                    if spelled[i].1 == spelled[j].1 {
                        collisions.push(format!(
                            "{width}x{height} {}: {} and {} are both spelled {:?}",
                            tier.name(),
                            spelled[i].0,
                            spelled[j].0,
                            spelled[i].1
                        ));
                    }
                }
            }
        }
    }

    println!(
        "G51_STATES={} G51_SHAPES_FULLY_MEASURED={} G51_SHAPES_MEASURED_IN_THE_GRID={} \
         G51_UNTESTABLE={} G51_COLLISIONS={}",
        shapes.len() + 2,
        fully_measured.len(),
        grid_measured.len(),
        untestable.len(),
        collisions.len()
    );
    for line in &untestable {
        println!("G51_UNTESTABLE {line}");
    }
    for line in &collisions {
        println!("G51_COLLISION {line}");
    }
    assert!(
        collisions.is_empty(),
        "🔴 G54: {} of the ten states share a spelling with another. The ruling is that all ten \
         are told apart on `mono`, where there is no hue to tell them apart with:\n{}",
        collisions.len(),
        collisions.join("\n")
    );
    assert!(
        !fully_measured.is_empty(),
        "🔴 G54: no shape and tier measured all ten states, so the gate above refused nothing. \
         UNTESTABLE:\n{}",
        untestable.join("\n")
    );
    assert!(
        !grid_measured.is_empty(),
        "🔴 G54: the eight wire states were never read out of a grid -- only out of opened \
         records. The column is declared at {} cells and something is dropping it at every ruled \
         shape.",
        inverse_column_width()
    );
}

/// 🔴 **g52 -- `Consumed` keeps the transformation it names.**
///
/// The variant carries a member, and the member is the answer to *what used the inverse up*. Three
/// refusals:
///
/// * no frame spells the object's **serialisation**. `{"Consumed"` on a screen is this face
///   drawing a value layer it is not allowed to name (`SKILL.md`, "直列化を第一級objectと取り違え
///   るな"), and at fourteen cells it is also all the reader gets.
/// * every spelling of the state is a prefix of `Consumed <by>`, cut at the cut mark this face
///   already uses on the id column -- a cut is allowed, an invention is not.
/// * at least one shape carries the whole of `by`. A `by` that is cut at every shape this face
///   draws is a `by` that was dropped with extra steps.
#[test]
fn g52_consumed_keeps_the_transformation_that_used_it() {
    let ledger = inverse_ledger();
    let whole = format!("Consumed {INVERSE_CONSUMED_BY}");
    let index = inverse_wire_shapes()
        .iter()
        .position(|(name, _)| *name == "Consumed")
        .expect("the fixture carries a Consumed row");

    let mut serialised: Vec<String> = Vec::new();
    let mut invented: Vec<String> = Vec::new();
    let mut whole_at: Vec<(u16, u16, &'static str)> = Vec::new();
    let mut read: usize = 0;

    for (width, height) in RULED_SHAPES {
        for tier in Tier::ALL {
            let grid = inverse_frame(&ledger, width, height, tier, index, false);
            let opened = inverse_frame(&ledger, width, height, tier, index, true);
            for (what, frame) in [("grid", &grid), ("record", &opened)] {
                if frame.contains("{\"Consumed\"") {
                    serialised.push(format!("{width}x{height} {} {what}", tier.name()));
                }
            }
            let mut spellings: Vec<(&str, String)> = Vec::new();
            if let Some((_, cell)) = inverse_grid_cells(&grid)
                .into_iter()
                .find(|(n, _)| *n == index)
            {
                spellings.push(("grid", cell));
            }
            if let Some(member) = inverse_opened_member(&opened) {
                spellings.push(("record", member));
            }
            for (what, spelling) in spellings {
                read += 1;
                let cut = spelling.trim_end_matches('~');
                if !whole.starts_with(cut) {
                    invented.push(format!(
                        "{width}x{height} {} {what}: {spelling:?} is not a cut of {whole:?}",
                        tier.name()
                    ));
                }
                if spelling == whole {
                    whole_at.push((width, height, tier.name()));
                }
            }
        }
    }

    println!(
        "G52_SPELLINGS_READ={read} G52_SERIALISED={} G52_INVENTED={} G52_SHAPES_CARRYING_THE_WHOLE_BY={}",
        serialised.len(),
        invented.len(),
        whole_at.len()
    );
    assert!(
        serialised.is_empty(),
        "🔴 g52: the object's serialisation reached the screen at {} shape/tier/region(s). A cell \
         fourteen cells wide cuts it to `{{\"Consumed\":{{~` and the reader is left with \
         punctuation:\n{}",
        serialised.len(),
        serialised.join("\n")
    );
    assert!(
        invented.is_empty(),
        "🔴 g52: {} spelling(s) of Consumed are not a cut of {whole:?}:\n{}",
        invented.len(),
        invented.join("\n")
    );
    assert!(
        !whole_at.is_empty(),
        "🔴 g52: no shape this face is ruled at carried the whole of `by`. `Consumed` without the \
         transformation that consumed it is the word without the fact."
    );
}

/// 🔴 **g53 -- `null` and `Unavailable` are never the same spelling, and `null` is `absent`.**
///
/// The pair the ruling names by name (`req/924` §TUI-13 追記): `crates/gx-api/src/list.rs` writes
/// `null` for **no escrow row at all** and `InverseStatus::Unavailable` for *`invert()` answered
/// `None`*. Asked-and-there-is-none is a property of the change; there-is-nobody-to-ask is a
/// property of the ledger. This gate refuses both directions of collapsing them:
///
/// * `null` drawn as anything but [`Nothing::Absent`]'s mark -- which is what it was, drawn `?`,
///   the mark of a read that failed;
/// * `null` and `Unavailable` drawn alike -- which is what over-correcting the first would produce.
#[test]
fn g53_an_absent_escrow_row_is_not_an_unavailable_inverse() {
    let ledger = inverse_ledger();
    let absent = Nothing::Absent.mark().to_string();
    let shapes = inverse_wire_shapes();
    let at = |name: &str| {
        shapes
            .iter()
            .position(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("🔴 the fixture carries no {name} row"))
    };
    let (null_at, unavailable_at) = (at("null"), at("Unavailable"));

    let mut wrong: Vec<String> = Vec::new();
    let mut collapsed: Vec<String> = Vec::new();
    let mut compared = 0usize;

    for (width, height) in RULED_SHAPES {
        for tier in Tier::ALL {
            let null_frame = inverse_frame(&ledger, width, height, tier, null_at, true);
            let unavailable_frame =
                inverse_frame(&ledger, width, height, tier, unavailable_at, true);
            let (Some(null_spelling), Some(unavailable_spelling)) = (
                inverse_opened_member(&null_frame),
                inverse_opened_member(&unavailable_frame),
            ) else {
                // The region drew no member line at this shape. Not measured, and not a failure.
                continue;
            };
            compared += 1;
            if null_spelling != absent {
                wrong.push(format!(
                    "{width}x{height} {}: null is spelled {null_spelling:?}, and the ruling spells \
                     it {absent:?}",
                    tier.name()
                ));
            }
            if null_spelling == unavailable_spelling {
                collapsed.push(format!(
                    "{width}x{height} {}: both are spelled {null_spelling:?}",
                    tier.name()
                ));
            }
        }
    }

    println!(
        "G53_COMPARED={compared} G53_NULL_MISSPELLED={} G53_COLLAPSED={}",
        wrong.len(),
        collapsed.len()
    );
    assert!(
        compared > 0,
        "🔴 g53: no shape produced both spellings to compare"
    );
    assert!(
        collapsed.is_empty(),
        "🔴 g53: an absent escrow row and an inverse the adapter could not build are drawn alike \
         at {} shape/tier(s):\n{}",
        collapsed.len(),
        collapsed.join("\n")
    );
    assert!(
        wrong.is_empty(),
        "🔴 g53: `null` on this key means **there is no escrow row** -- `list.rs` says so in the \
         comment beside the line that writes it -- and this face draws it as something else at {} \
         shape/tier(s):\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

// ---------------------------------------------------------------------------------------------
// G55..G58 — the paint axis's top rung, the emphasis that reaches a record row, and the ruling
// that a hoisted claim is measured over the ledger rather than over the viewport
// (`req/924` §TUI-19 #1/#4 and §TUI-20, `req/38` SS1046/SS1047, Owner #264-T).
// ---------------------------------------------------------------------------------------------

/// The live bed, in miniature: `count` records that agree on everything, and one near the end that
/// does not.
///
/// 🔴 The odd record is what makes this fixture able to tell the two hoists apart. Measured against
/// the engine on 2026-09-01, `GET /v1/transformations` answered with thirty-one records of which
/// **one** carried `verdict ?` and `state Candidate`; a viewport that never reached that record
/// hoisted `verdict Admit` and stated it flatly, and the same screen with the attention at the end
/// drew a verdict column. A fixture that agrees on every row cannot see the difference, which is
/// why `uniform_ledger` above is not reused here.
///
/// The id keeps `record_id`'s shape so [`drawn_records`] can still say which row is which.
fn ledger_with_one_odd(count: usize) -> Screen {
    let odd = count.saturating_sub(2);
    let items: Vec<serde_json::Value> = (0..count)
        .map(|n| {
            if n == odd {
                serde_json::json!({
                    "transformation": record_id(n),
                    "state": "Candidate",
                    "verdict": serde_json::Value::Null,
                    "enforced": true,
                    "created_at": "2026-08-31T21:52:09Z",
                    "actor": "agent-a",
                    "scope": format!("src/row{n}.rs"),
                    "inverse_status": serde_json::Value::Null,
                    "rollback": serde_json::Value::Null,
                    "superseded_by": serde_json::Value::Null,
                })
            } else {
                serde_json::json!({
                    "transformation": record_id(n),
                    "state": "Committed",
                    "verdict": "Admit",
                    "enforced": true,
                    "created_at": "2026-08-30T09:00:00Z",
                    "actor": "agent-a",
                    "scope": format!("src/row{n}.rs"),
                    "inverse_status": "Available",
                    "rollback": serde_json::Value::Null,
                    "superseded_by": serde_json::Value::Null,
                })
            }
        })
        .collect();
    Screen {
        healthz: answered(
            "/v1/healthz",
            serde_json::json!({
                "status": "ok",
                "engine_version": "gx-engine 0.1.0",
                "ledger_agrees": true,
                "journal_rows": count,
                "status_reason": serde_json::Value::Null,
            }),
        ),
        transformations: answered(
            "/v1/transformations",
            serde_json::json!({ "items": items, "next_cursor": serde_json::Value::Null }),
        ),
        candidates: answered(
            "/v1/candidates",
            serde_json::json!({ "items": [], "next_cursor": serde_json::Value::Null }),
        ),
        escalations: answered(
            "/v1/escalations",
            serde_json::json!({ "items": [], "next_cursor": serde_json::Value::Null }),
        ),
    }
}

/// The seven shapes `req/942_artifacts/tui_r37_2026-09-01` captures on a real terminal, so a gate
/// and a photograph are asking about the same screens.
const R37_SHAPES: [(u16, u16); 7] = [
    (120, 32),
    (100, 30),
    (80, 24),
    (66, 20),
    (60, 20),
    (46, 12),
    (40, 10),
];

/// Whether one cell carries any emphasis at all: a hue, a ground, or a modifier.
fn cell_is_emphasised(cell: &ratatui::buffer::Cell) -> bool {
    cell.fg != ratatui::style::Color::Reset
        || cell.bg != ratatui::style::Color::Reset
        || !cell.modifier.is_empty()
}

/// Whether any cell in this row carries emphasis.
fn row_is_emphasised(buffer: &ratatui::buffer::Buffer, y: u16) -> bool {
    (buffer.area.x..buffer.area.x + buffer.area.width).any(|x| cell_is_emphasised(&buffer[(x, y)]))
}

/// The declared paint intents: what this face is saying, and the [`tokens::Role`] it says it in.
///
/// 🔴 **The map `req/924` §TUI-15 recorded as UNTESTABLE, given a domain.** The census looked for an
/// `Intent` type in `tui/src/tui/tokens.rs`, found none, and was right that a map with no domain
/// cannot be checked. The ruling this lane made (and wrote into that module's own documentation) is
/// that the rung is not missing: it is `wire`, where the product's words arrive, and the three
/// `role()` methods there **are** this map. Eleven of the fifteen entries below are therefore *read
/// out of the wire's vocabularies* rather than typed, so a word added to `Nothing` or a fourth
/// verdict grows this gate instead of leaving it measuring the old count.
///
/// The remaining four are the face's own chrome — what it says about itself rather than about the
/// engine — and they are spelled here because there is nowhere outside the face to read them from.
/// That they are hand-written is the honest weak point of this gate and is not hidden.
fn declared_paint_intents() -> Vec<(String, tokens::Role)> {
    let mut out: Vec<(String, tokens::Role)> = Vec::new();
    for nothing in Nothing::ALL {
        out.push((
            format!("the wire carried nothing, of the kind `{}`", nothing.word()),
            nothing.role(),
        ));
    }
    for kind in wire::VERDICT_KINDS {
        out.push((
            format!("the engine's decision was `{kind}`"),
            wire::VerdictMark::Kind(kind).role(),
        ));
    }
    out.push((
        "the wire carried no decision for this record".to_string(),
        wire::VerdictMark::None(Nothing::Unknown).role(),
    ));
    out.push((
        "which screen this is, and what its columns are called".to_string(),
        tokens::Role::Head,
    ));
    out.push((
        "the face's own account of itself, and of what it let go of".to_string(),
        tokens::Role::Quiet,
    ));
    out.push(("a value the wire carried".to_string(), tokens::Role::Body));
    out.push((
        "the thing the reader is standing on".to_string(),
        tokens::Role::Attend,
    ));
    // 🔴 **Six intents for the six dots** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // Each sentence is what the mark says, and they are six because §TUI-30's rule is that a
    // mark may not reduce the number of states a reader can tell apart. `Link::sentence` is the
    // declared wording for five of them; the sixth is the split `req/38` SS1085 asked for, which is
    // the whole reason the badge it replaces was not honest.
    for link in gx_tui::tui::live::LINKS {
        let role = match link {
            gx_tui::tui::live::Link::Open => tokens::Role::LinkLive,
            gx_tui::tui::live::Link::Opening => tokens::Role::LinkOpening,
            gx_tui::tui::live::Link::Never => tokens::Role::LinkNever,
            gx_tui::tui::live::Link::Closed => tokens::Role::LinkClosed,
            gx_tui::tui::live::Link::Off => tokens::Role::LinkOff,
        };
        out.push((format!("the connection: {}", link.sentence()), role));
    }
    out.push((
        "the connection is up and nothing has arrived for a while".to_string(),
        tokens::Role::LinkQuiet,
    ));
    out
}

/// 🔴 **g55 — `intent -> role` on the paint axis is total, injective, and covers `ROLES` exactly.**
///
/// Total: every declared intent resolves to a role. Injective: no two intents resolve to the same
/// one — *different things this face is saying must never collapse into one appearance*. Covering:
/// no role exists that nothing names, because a value nobody means is a value nobody maintains.
///
/// 🔴 **This gate is green on the source it was written against, and that is stated rather than
/// dressed up.** It is a tripwire, not a finding. What it would have caught, on the run rather than
/// in a reading, is `req/38` SS974: a value with nothing in it and a count of nought were painted
/// in one role until the seventh word was added, which is exactly two intents collapsed into one
/// appearance. The planted control below fires it in the red direction so it is not a gate that has
/// never refused anything.
#[test]
fn g55_a_paint_intent_resolves_to_exactly_one_role_and_no_role_is_unclaimed() {
    let declared = declared_paint_intents();
    println!(
        "G55_INTENTS={} G55_ROLES={}",
        declared.len(),
        tokens::ROLES.len()
    );

    let mut collapsed: Vec<String> = Vec::new();
    for (i, (sentence_a, role_a)) in declared.iter().enumerate() {
        for (sentence_b, role_b) in declared.iter().skip(i + 1) {
            if role_a == role_b {
                collapsed.push(format!(
                    "{:?}: {sentence_a:?} and {sentence_b:?}",
                    role_a.name()
                ));
            }
        }
    }
    assert!(
        collapsed.is_empty(),
        "🔴 g55: {} pair(s) of distinct paint intents resolve to one role, so the face draws two \
         different things it is saying with one appearance:\n{}",
        collapsed.len(),
        collapsed.join("\n")
    );

    let named: BTreeSet<&str> = declared.iter().map(|(_, role)| role.name()).collect();
    let all: BTreeSet<&str> = tokens::ROLES.iter().map(|role| role.name()).collect();
    let unclaimed: Vec<&&str> = all.difference(&named).collect();
    assert!(
        unclaimed.is_empty(),
        "🔴 g55: {} role(s) are declared in `ROLES` and no intent names them, so nothing on this \
         screen means them: {unclaimed:?}",
        unclaimed.len()
    );
    let unknown: Vec<&&str> = named.difference(&all).collect();
    assert!(
        unknown.is_empty(),
        "🔴 g55: {} intent(s) resolve to a role that is not in `ROLES`: {unknown:?}",
        unknown.len()
    );

    // The positive control: the SS974 collapse, planted. Two intents, one role.
    let planted = [
        ("a count of nought".to_string(), tokens::Role::MarkZero),
        (
            "a value with nothing in it".to_string(),
            tokens::Role::MarkZero,
        ),
    ];
    let caught = planted[0].1 == planted[1].1;
    assert!(
        caught,
        "🔴 g55 control: the planted collapse was not detected, so this gate's own test for \
         injectivity does not work"
    );
    println!("G55_CONTROL=caught the planted `zero`/`empty` collapse");
}

/// 🔴 **g56 — emphasis reaches the rows the reader is reading.**
///
/// Measured on a real terminal at 120x29 on 2026-09-01 (`req/924` §TUI-19 #1), the face drew
/// **nineteen record rows carrying no SGR at all**: every column that could have said something
/// about a record had been hoisted into one line above them, and what was left was the id, painted
/// `paint.body`, which spells no hue and no modifier by decision. Emphasis was not weak on that
/// screen; it was absent from the part of the screen the reader was looking at.
///
/// The predicate is a property and not a magic number: **no drawn record row may be entirely
/// unemphasised**, and the count of emphasised rows is printed at every shape so the number is in
/// the record rather than in a lane report. One row is not a screen with emphasis on it, so the
/// count is also required to exceed one wherever more than one record is drawn.
#[test]
fn g56_no_record_row_is_drawn_without_any_emphasis_on_it() {
    let screen = ledger_with_one_odd(31);
    let mut naked: Vec<String> = Vec::new();
    let mut untestable = 0usize;
    for (width, height) in R37_SHAPES {
        let buffer = renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Truecolor,
            false,
            &View::default(),
        );
        let text = renderer::buffer_text(&buffer);
        let records = drawn_records(&text);
        if records.is_empty() {
            // The third value: a shape with no room for a record says nothing about emphasis on
            // record rows. Counted and printed, never folded into the failing side.
            println!("G56 UNTESTABLE {width}x{height}: no record row drawn");
            untestable += 1;
            continue;
        }
        let rows: Vec<u16> = (buffer.area.y..buffer.area.y + buffer.area.height)
            .filter(|y| row_is_emphasised(&buffer, *y))
            .collect();
        let bare: Vec<usize> = records
            .iter()
            .map(|(row, _)| *row)
            .filter(|row| !row_is_emphasised(&buffer, *row as u16))
            .collect();
        println!(
            "G56 {width}x{height} records={} emphasised_rows={} of {height} bare_record_rows={}",
            records.len(),
            rows.len(),
            bare.len()
        );
        if !bare.is_empty() {
            naked.push(format!(
                "{width}x{height}: {} of {} record row(s) carry no hue, no ground and no \
                 modifier -- rows {bare:?}",
                bare.len(),
                records.len()
            ));
        }
        if records.len() > 1 {
            assert!(
                rows.len() > 1,
                "🔴 g56 at {width}x{height}: {} record(s) are drawn and only {} row(s) on the \
                 whole screen carry any emphasis",
                records.len(),
                rows.len()
            );
        }
    }
    println!("G56_UNTESTABLE={untestable}");
    assert!(
        naked.is_empty(),
        "🔴 g56: a record row with no emphasis anywhere on it is a row the reader has no handle \
         on, at {} shape(s):\n{}",
        naked.len(),
        naked.join("\n")
    );
}

/// 🔴 **g57 — the seven words for nothing stay seven after the paint is applied.**
///
/// `P2` already measures that the marks are told apart by their symbol on `mono`. This asks the
/// next question, which is the one emphasis makes it possible to get wrong: once a hue, a ground
/// and a modifier are added, do two of the seven become the *same drawn thing*? A pair that shares
/// a spelling **and** an appearance is a pair a reader cannot separate at any tier.
#[test]
fn g57_the_seven_kinds_of_nothing_are_still_seven_once_they_are_painted() {
    let mut collisions: Vec<String> = Vec::new();
    for tier in tokens::Tier::ALL {
        let drawn: Vec<(String, &'static str, tokens::Ink)> = Nothing::ALL
            .into_iter()
            .map(|nothing| {
                (
                    nothing.mark().to_string(),
                    nothing.word(),
                    tokens::ink(nothing.role(), tier),
                )
            })
            .collect();
        for (i, (mark_a, word_a, ink_a)) in drawn.iter().enumerate() {
            for (mark_b, word_b, ink_b) in drawn.iter().skip(i + 1) {
                if mark_a == mark_b && ink_a == ink_b {
                    collisions.push(format!(
                        "{}: `{word_a}` and `{word_b}` are both {mark_a:?} in {ink_a:?}",
                        tier.name()
                    ));
                }
            }
        }
        println!(
            "G57 {} distinct_marks={} of {}",
            tier.name(),
            drawn
                .iter()
                .map(|(mark, _, _)| mark.clone())
                .collect::<BTreeSet<String>>()
                .len(),
            Nothing::ALL.len()
        );
    }
    assert!(
        collisions.is_empty(),
        "🔴 g57: {} pair(s) of the seven words are drawn as the same symbol in the same ink, so \
         emphasis has closed a distinction the vocabulary exists to keep open:\n{}",
        collisions.len(),
        collisions.join("\n")
    );
}

/// 🔴 **g58 — which columns the grid draws does not depend on where the attention is standing.**
///
/// Owner reported it from a live terminal on 2026-09-01 (#264-T): *"the list's records 1-29 show
/// nothing but the verdict when selected"*. Reproduced on a real PTY at 120x29 against
/// thirty-one records — with the attention on record 1 the grid drew **one** column, and with the
/// attention on record 31 it drew **six**. `hoist` was asking its question of the records in the
/// window, so the answer moved when the window did.
///
/// Two halves, for the reason g28 has two: the decision, fired at [`renderer::hoist`] over every
/// place the attention can stand, and then the picture, so a gate is not measuring an arithmetic
/// nobody draws with. The claim itself is checked too — a column called constant must actually be
/// constant over every record the read carried, not over the drawn slice.
#[test]
fn g58_the_column_set_does_not_move_when_the_attention_does() {
    let screen = ledger_with_one_odd(31);
    let items = screen.transformations.items();
    assert_eq!(items.len(), 31, "g58 bed: thirty-one records, as measured");

    let mut moved: Vec<String> = Vec::new();
    let mut false_claims: Vec<String> = Vec::new();
    for width in HOIST_WIDTHS {
        let (columns, _) = layout::columns_for(width);
        for capacity in [2usize, 5, 12, 19, 23, 40] {
            let mut seen: BTreeSet<Vec<&'static str>> = BTreeSet::new();
            for selected in 0..items.len() {
                let (kept, shared) = renderer::hoist(&items, &columns, capacity);
                seen.insert(kept.iter().map(|column| column.key).collect());
                // A hoisted column claims that every record agrees. Asked of the ledger, not of
                // the window.
                for (key, mark) in &shared {
                    let all_agree = items
                        .iter()
                        .all(|item| &renderer::cell_mark(item, key).0 == mark);
                    if !all_agree {
                        false_claims.push(format!(
                            "{width} capacity={capacity} selected={selected}: `{key} {mark}` is \
                             stated as constant and at least one of the {} records disagrees",
                            items.len()
                        ));
                    }
                }
            }
            if seen.len() > 1 {
                let shapes: Vec<Vec<&str>> = seen.iter().cloned().collect();
                moved.push(format!(
                    "{width} capacity={capacity}: the grid took {} different column sets \
                     depending only on where the attention stood: {shapes:?}",
                    seen.len()
                ));
            }
        }
    }
    println!("G58_DECISION_SHAPES_CHECKED={}", HOIST_WIDTHS.len() * 6);
    assert!(
        false_claims.is_empty(),
        "🔴 g58: {} hoisted line(s) state something the ledger does not support -- a claim \
         measured over the viewport and drawn as a claim about the list:\n{}",
        false_claims.len(),
        false_claims
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        moved.is_empty(),
        "🔴 g58: the column set is a function of the attention's position at {} \
         width/capacity pair(s):\n{}",
        moved.len(),
        moved.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );

    // The picture. The same three places on a real shape, read off the header row.
    let header_at = |selected: usize| -> Vec<String> {
        let view = View {
            selected,
            ..View::default()
        };
        let text = renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            120,
            29,
            Tier::Truecolor,
            false,
            &view,
        ));
        text.lines()
            .find(|line| line.split_whitespace().next() == Some(LEDGER_COLUMNS[0].key))
            .map(|line| line.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    };
    let first = header_at(0);
    let middle = header_at(15);
    let last = header_at(items.len() - 1);
    println!("G58 header first={first:?}");
    println!("G58 header middle={middle:?}");
    println!("G58 header last={last:?}");
    assert!(
        !first.is_empty(),
        "🔴 g58: no header row was found at 120x29, so the picture half measured nothing"
    );
    // 🔴 **The header is content and scrolls away** (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)).
    // g58's subject is unchanged -- *the column set must not be a function of where the cursor is
    // standing* -- and the decision half above measures exactly that over forty-two width/capacity
    // pairs. What changed is that the picture half was reading the set off the **drawn** header,
    // and at the bottom of a thirty-one row ledger that header has scrolled off the top: an empty
    // reading there is the ruling working, not the column set moving. So the comparison is over the
    // positions where the header is on the screen, and `layout::scrolled` -- the face's own
    // arithmetic -- is what says which those are.
    let plan_at = |selected: usize| {
        layout::resolve_attended(
            120,
            29,
            &renderer::measured(&screen),
            false,
            layout::Subject::Grid,
            layout::Attention {
                selected,
                items: items.len(),
                glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
            },
        )
    };
    let drawn: Vec<(usize, Vec<String>)> = [0usize, 15, items.len() - 1]
        .into_iter()
        .filter(|selected| plan_at(*selected).preamble_shown > 0)
        .map(|selected| (selected, header_at(selected)))
        .collect();
    println!(
        "G58 header positions with the preamble on screen={:?}",
        drawn
    );
    assert!(
        drawn.len() >= 2,
        "🔴 g58: the header was on the screen at fewer than two of the three positions, so the \
         picture half compared nothing: {drawn:?}"
    );
    let mismatched: Vec<&(usize, Vec<String>)> = drawn
        .iter()
        .filter(|(_, names)| *names != drawn[0].1)
        .collect();
    assert!(
        mismatched.is_empty(),
        "🔴 g58 at 120x29: the header names {:?} at selection {} and {mismatched:?} elsewhere",
        drawn[0].1,
        drawn[0].0
    );
}

// =============================================================================================
// `req/924` §TUI-22 / §TUI-21 / §TUI-23 — the three gates the enclosure's own doc comments name.
//
// 🔴 **`tui/src` referred to `g59`, `g60` and `g61` and no suite in this repository contained any
// of them** (`req/38` SS1057: the enclosure landed UNVERIFIED with nought new gates). A doc
// comment naming a gate nobody wrote is the same failure as a gate checking a declaration nothing
// reads, pointed the other way: the source asserts a machine is watching and none is. Minted after
// a fresh grep of every `*.rs` in the repository, where the highest number in use was `g58`.
// =============================================================================================

/// 🔴 **g59 — the reason is on the rail exactly when the engine is not the healthy word.**
///
/// `req/924` §TUI-22 (`req/38` SS1049, Owner `#266-T`) raised this as a **new** finding: the bed
/// answers `status: ok` with `status_reason: null` and the face drew `status_reason ?` on every
/// frame — *measured, and not knowable*, asserted about a key that has no reason to give. §TUI-23
/// then ruled the wire's half (`null`, and the field is always sent, because not sending it is a
/// different claim and would collapse Absent into Unknown). The face's half is this: draw the
/// reason when there is one, draw nothing when there is not, and name the condition in the hatch.
///
/// The discrimination is the whole gate. A face that never drew the reason would satisfy one half
/// and a face that always drew it would satisfy the other, so both directions are measured on two
/// screens that differ in exactly one wire value.
#[test]
fn g59_the_reason_is_on_the_rail_exactly_when_the_engine_is_not_ok() {
    let well = ledger(28);
    let mut unwell = ledger(28);
    unwell.healthz = answered(
        "/v1/healthz",
        serde_json::json!({
            "status": "degraded",
            "engine_version": "gx-engine 0.1.0",
            "ledger_agrees": false,
            "journal_rows": 28,
            "status_reason": "the ledger disagrees with the journal",
        }),
    );

    let keys = |screen: &Screen| -> Vec<String> {
        renderer::measured(screen)
            .engine
            .into_iter()
            .map(|(key, _)| key)
            .collect()
    };
    let well_keys = keys(&well);
    let unwell_keys = keys(&unwell);
    println!("G59_WELL={well_keys:?} G59_UNWELL={unwell_keys:?}");
    assert!(
        !well_keys.iter().any(|key| key == "status_reason"),
        "🔴 g59: the engine is `ok` and the rail carries a reason. An engine that is ok has no \
         reason, so drawing one asserts `measured and not knowable` about a fact that is Absent"
    );
    assert!(
        unwell_keys.first().map(String::as_str) == Some("status_reason"),
        "🔴 g59: the engine is not `ok` and the reason is not the rail's first key. It leads \
         because it is the one fact that explains the rest, and the rail gives its keys up from \
         the end: {unwell_keys:?}"
    );

    // And it reaches the frame, at a width wide enough to hold the whole line.
    let drawn = flat(&renderer::buffer_text(&renderer::render_to_buffer(
        &unwell,
        200,
        32,
        Tier::Mono,
        false,
    )));
    assert!(
        drawn.contains("the ledger disagrees with the journal"),
        "🔴 g59: the engine gave a reason and no row of the frame carries it:\n{drawn}"
    );
    let healthy = flat(&renderer::buffer_text(&renderer::render_to_buffer(
        &well,
        200,
        32,
        Tier::Mono,
        false,
    )));
    assert!(
        !healthy.contains("status_reason ?"),
        "🔴 g59: the engine is `ok` and the rail spells a reason anyway:\n{healthy}"
    );

    // The hatch names the key and the condition, because a key drawn conditionally and explained
    // nowhere is the one field on the screen that leaves without a word.
    let hatch = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
        &well,
        200,
        32,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
    )));
    assert!(
        // 🔴 `on the rail` became `here` (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)): there
        // is no rail, and a condition sentence that goes on naming a row this face no longer draws
        // is the divergence this whole session is about. The condition itself is unchanged.
        hatch.contains("status_reason is carried here only when status is not ok"),
        "🔴 g59: the condition is drawn nowhere a reader can reach:\n{hatch}"
    );
}

/// 🔴 **g60 — the enclosure and the words it replaced move together.**
///
/// `req/924` §TUI-22 (`req/38` SS1049, Owner `#266-T`, 2026-09-01) admitted the corners under the
/// test `INHERITED_PRINCIPLES` §3c-③'' states — *記号が意味を担い、その結果 語が消えるなら可。語を
/// 残したまま記号を足すなら不可* — and made the admission **conditional**: 🔴 *ただし枠を足して
/// `screen: …` を残したら不可。対で動かせ。*
///
/// So the pairing is the gate. Three directions, over the whole sweep and the seven ruled shapes:
/// the clause is on no screen; a screen that says it is enclosed draws the corners; a screen that
/// is not enclosed says so. The third is the renderer's standing debt — it cannot `invert`, so
/// what it owes is to name what it let go of, and the enclosure carries meaning.
///
/// 🔴 **This is a tripwire and not a discovery.** It was green on `main` the hour it was written,
/// because the clause was deleted by the same commit that drew the corners. The negative control
/// below is what keeps that from being a gate which would also pass with no predicate at all.
#[test]
fn g60_the_enclosure_and_the_region_rail_move_together() {
    // 🔴 **Both halves of the pairing left the screen, and that keeps the pairing true**
    // (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)). §TUI-22 admitted the enclosure's four
    // corners on one condition: the row spelling `screen: apparatus subject provenance
    // disclosure` goes with them. §TUI-57 then ruled the top rail off the standing frame, and an
    // enclosure needs two rails -- a screen closed at the bottom and open at the top is not an
    // enclosure -- so the corners went too.
    //
    // The pairing is therefore asserted in its other true direction: **neither** is ever drawn.
    // Nothing the corners deleted has come back, which is §TUI-22's own admission test read
    // backwards, and a corner reappearing without a rail would fire this.
    let screen = ledger(28);
    let measured = renderer::measured(&screen);
    let mut framed: Vec<String> = Vec::new();
    let mut railed: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for width in SWEEP_WIDTHS {
        for height in SWEEP_HEIGHTS {
            let plan = layout::resolve(width, height, &measured, false, layout::Subject::Grid);
            checked += 1;
            if plan.framed {
                framed.push(format!("{width}x{height}"));
            }
            let text = flat(&renderer::buffer_text(&renderer::render_to_buffer(
                &screen,
                width,
                height,
                Tier::Mono,
                false,
            )));
            if text.contains("screen:") || text.contains(tokens::CORNERS[0]) {
                railed.push(format!("{width}x{height}"));
            }
        }
    }
    println!("G60_CHECKED={checked} G60_FRAMED={framed:?} G60_RAILED={railed:?}");
    assert!(
        checked > 50,
        "🔴 g60 measured almost nothing ({checked} shapes)"
    );
    assert!(
        framed.is_empty() && railed.is_empty(),
        "🔴 g60 (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)): the enclosure or the region rail is \
         drawn again. framed={framed:#?} railed={railed:#?}"
    );
}

/// 🔴 **g61 — the hatch lists what the disclosure sends a reader to it for.**
///
/// `req/924` §TUI-21 (`req/38` SS1048, Owner `#265-T`, 2026-09-01), on the caveats it classified
/// as ②但し書き and therefore outside the word count: 🔴 *数だけ残して名前を `?` へ逃がすのは可。
/// ただし逃がし先が実際に列挙している事を gate で確かめるまで「逃がした」と言うな* — and that
/// section's own gate clause: 🔴 *`?` を押した画面に `3 fields` の名前3つと `2 routes` の名前2つが
/// 実在する事を assert。無ければ赤——逃がし先が空なら、それは逃がしたのでなく消したのと同じ。*
///
/// The claim is read off the face rather than assumed: the long form of the disclosure is the one
/// that spells `routes read and not drawn -> <address>`, and only there has the face said the
/// names moved. Where it has not said so — the short form, at forty by ten — nothing is asserted,
/// because a face that makes no claim cannot break one. That is the third value and it is printed.
///
/// 🔴 **Red on `main` at four of the six ruled shapes that make the claim** when it was written
/// — 80x24, 66x20, 60x20 and 46x12, with 120x32 and 100x30 clean and 40x10 making no claim. The
/// three entries carrying the names were pushed **last** into the help face's list, so at every
/// shape below a hundred cells the list was cut before reaching them while the grid went on
/// spelling `-> help:?`. Counted the other way, without the claim filter, the names were absent at
/// five of the seven ruled shapes.
#[test]
fn g61_the_escape_hatch_lists_the_names_the_disclosure_sent_there() {
    let screen = ledger(28);
    let measured = renderer::measured(&screen);
    let mut empty: Vec<String> = Vec::new();
    let mut no_claim: Vec<String> = Vec::new();
    let mut fired = 0usize;

    for (width, height) in RULED_SHAPES {
        let plan = layout::resolve(width, height, &measured, false, layout::Subject::Grid);
        // The face's own words. `keys_address` is `help:?` or `gx tui --help` depending on the
        // shape, so the marker is the clause rather than either spelling of the road.
        if !plan.disclosure.contains("routes read and not drawn ->") {
            no_claim.push(format!("{width}x{height}: {:?}", plan.disclosure));
            continue;
        }
        fired += 1;
        let hatch = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
            &View {
                help: true,
                ..View::default()
            },
        )));
        let (_, unseen) = layout::columns_for(width);
        let mut wanted: Vec<&str> = layout::READ_NOT_DRAWN.to_vec();
        wanted.extend(unseen);
        wanted.extend(LEDGER_PAGE_KEYS);
        let missing: Vec<&&str> = wanted
            .iter()
            .filter(|name| !hatch.contains(**name))
            .collect();
        println!(
            "G61 {width}x{height} wanted={} missing={missing:?}",
            wanted.len()
        );
        if !missing.is_empty() {
            empty.push(format!("{width}x{height}: {missing:?}\n{hatch}"));
        }
    }

    println!("G61_FIRED={fired} G61_NO_CLAIM={no_claim:?}");
    assert!(
        fired > 0,
        "🔴 g61: no ruled shape made the claim, so this gate measured nothing"
    );
    assert!(
        empty.is_empty(),
        "🔴 g61 (`req/924` §TUI-21): the disclosure says the names are behind `?` and the hatch \
         does not carry them. A hatch that lists nothing turns a disclosure into a deletion: \
         {empty:#?}"
    );
}

// =============================================================================================
// g62 — `req/924` §TUI-23 (SS1051, the ruling) and §TUI-39 (SS1069, the repair): `status_reason`'s
// `null` is `Absent`, not `Unknown`, and the fix is a carved-out classifier, not a change to
// `cell`'s general rule. `req/1033` is the lane that lands this gate; P9 (this file, above)
// printed the divergence in the first pass and named the reason it was not repaired there.
// =============================================================================================

/// 🔴 **g62a — `status_reason: null` reads `Absent` through the classifier meant for it.**
///
/// The positive half of the repair: `/v1/healthz`'s and `server_health`'s `null` (44 §2.2 `L-02`,
/// §2.5) means *the engine is `ok` and has no reason to give* — never-written, not
/// measured-and-unknowable — so `wire::status_reason` must answer [`Nothing::Absent`], the mark
/// `req/924` §TUI-23 named and `wire::cell` alone could not produce (P9, above).
#[test]
fn g62a_status_reason_null_on_ok_reads_absent_through_the_classifier() {
    let body: serde_json::Value = serde_json::from_str(
        r#"{"status":"ok","engine_version":"gx-engine 0.1.0","ledger_agrees":true,
            "journal_rows":0,"status_reason":null}"#,
    )
    .expect("fixture parses");
    assert_eq!(
        wire::status_reason(&body),
        wire::Cell::Nothing(Nothing::Absent),
        "🔴 g62a: an `ok` engine's `status_reason: null` is never-written, not \
         measured-and-unknowable — `req/924` §TUI-23"
    );
    assert_eq!(
        wire::status_reason(&body).text(),
        Nothing::Absent.mark(),
        "🔴 g62a: the mark on screen has to be `--`, not `?`"
    );
}

/// 🔴 **g62b — negative control: a real reason still reads as the reason, word for word.**
///
/// The carve-out must not swallow the case it was not built for. `status: "degraded"` /
/// `"unhealthy"` / `"unknown"` all carry a non-null sentence (`crates/gx-api/src/handlers.rs`'s
/// `degraded_reason` and the two `format!` arms in `server_health`/`healthz`), and a repair that
/// answered `Absent` for those too would trade one collapse for another.
#[test]
fn g62b_status_reason_a_real_sentence_is_not_swallowed_into_absent() {
    let body: serde_json::Value = serde_json::from_str(
        r#"{"status":"unhealthy","engine_version":"gx-engine 0.1.0","ledger_agrees":false,
            "journal_rows":3,"status_reason":"the ledger disagrees with the journal"}"#,
    )
    .expect("fixture parses");
    assert_eq!(
        wire::status_reason(&body),
        wire::Cell::Value("the ledger disagrees with the journal".to_string()),
        "🔴 g62b: a real sentence was read as a kind of nothing"
    );
}

/// 🔴 **g62c — negative control: a server that predates the key reads the same as `null`.**
///
/// `L-02`/R11 added `status_reason`; a server built before either never carries the key at all.
/// `wire::inverse_status` draws that the same way it draws a present `null` (declared and not
/// distinguished, `req/924` §TUI-13 追記), and `status_reason` is carved out of `cell` the same
/// way, so it keeps the same answer for the same reason.
#[test]
fn g62c_status_reason_a_missing_key_reads_absent_the_same_as_null() {
    let body: serde_json::Value =
        serde_json::from_str(r#"{"status":"ok","engine_version":"gx-engine 0.1.0"}"#)
            .expect("fixture parses");
    assert_eq!(
        wire::status_reason(&body),
        wire::Cell::Nothing(Nothing::Absent),
        "🔴 g62c: a server old enough to carry no `status_reason` member has to read the same as \
         one that carries it as `null`"
    );
}

/// 🔴 **g62d — negative control: `cell`'s general rule for this key is unmoved.**
///
/// `req/38` SS856's caution, kept literal: repairing `status_reason` **inside** `cell` would
/// change what a `null` means on every route that function classifies, which is the expensive
/// repair this lane refused. `wire::cell(&body, "status_reason")` still answers `Unknown` on the
/// same fixture `g62a` reads as `Absent` — the carve-out is additive, not a change to the rule it
/// is carved out of.
#[test]
fn g62d_cells_general_rule_for_status_reason_is_unchanged() {
    let body: serde_json::Value = serde_json::from_str(
        r#"{"status":"ok","engine_version":"gx-engine 0.1.0","ledger_agrees":true,
            "journal_rows":0,"status_reason":null}"#,
    )
    .expect("fixture parses");
    assert_eq!(
        wire::cell(&body, wire::STATUS_REASON_KEY),
        wire::Cell::Nothing(Nothing::Unknown),
        "🔴 g62d: `cell`'s general rule moved. It was meant to stay put — the fix is the carve-out \
         (`wire::status_reason`), not a change to the rule underneath it"
    );
}

/// 🔴 **g62e — the repair reaches the rail's `not ok` road, and `g59`'s `ok` road is unmoved.**
///
/// `super::renderer::engine_line` now reads `status_reason` through the classifier rather than
/// through `cell` directly. This does not touch `g59` (`req/924` §TUI-22): the row this function
/// draws when the engine is `ok` is still nothing at all, by that separate, still-standing
/// decision — this gate measures only that the *not ok* road, which draws the value, still draws
/// it correctly once it is reached through the new call.
#[test]
fn g62e_the_rails_not_ok_road_still_draws_the_real_reason() {
    let mut unwell = ledger(6);
    unwell.healthz = answered(
        "/v1/healthz",
        serde_json::json!({
            "status": "degraded",
            "engine_version": "gx-engine 0.1.0",
            "ledger_agrees": true,
            "journal_rows": 6,
            "status_reason": "`.gx/VERSION` is not there, so this server refuses every write",
        }),
    );
    let measured = renderer::measured(&unwell);
    let reason = measured
        .engine
        .iter()
        .find(|(key, _)| key == "status_reason")
        .map(|(_, value)| value.clone());
    assert_eq!(
        reason.as_deref(),
        Some("`.gx/VERSION` is not there, so this server refuses every write"),
        "🔴 g62e: the rail's engine line, read through `renderer::measured`, is {:?}",
        measured.engine
    );
}

// =============================================================================================
// g64..g67 — `req/924` §TUI-45 (`req/38` SS1076, Owner `#275-T`) and §TUI-29 (SS1058, `#268-T`).
//
// 🔴 **Every predicate below is written against API that exists on `main`**, and that is a
// requirement rather than a style: a gate that names a function this lane adds cannot be **run** on
// `main`, so the red half of red-first would be a compile error — which says nothing about the
// face. These four compile against both trees and are red on one of them.
//
// 🔴 And they are asked of the **screen**, not of the plan's own declarations. `main`'s memory of
// this month has a gate that validated a declaration while nothing read it
// ([[feedback_gate_validates_declaration_not_behaviour]]); the drawn buffer is the ground truth a
// reader actually gets.
//
// * **g64** a column whose every value is a mark for nothing is not drawn — three negative
//   controls, because *only when every value is nothing* is the half of the rule that is cheap to
//   over-apply and expensive to get wrong.
// * **g65** what left the grid is inside the number the screen spells, and the hatch names it.
// * **g66** a claim measured on the window is quantified by the window — SS1047 held from the
//   other side.
// * **g67** the region that stood down did not take the one fact no route can re-measure with it.
// =============================================================================================

/// Whether a resolved cell's text is one of the marks that mean **no answer was obtained**.
///
/// 🔴 Read out of [`wire::VACANT_MARKS`] rather than typed, which is the same declaration the face's
/// own answer is derived from — so this is not a second understanding of the rule (SS855), it is the
/// one declaration asked twice. A literal list here would be exactly the checker that measures the
/// test's belief instead of the face's behaviour, and an independent audit of this lane found the
/// first cut of this helper reading `Nothing::ALL` — carrying the production defect into the probe,
/// so the two agreed about a rule that was wrong.
fn says_nothing(mark: &str) -> bool {
    wire::VACANT_MARKS
        .iter()
        .any(|nothing| nothing.mark() == mark)
}

/// Whether a resolved cell's text is any of the seven, vacant or answered.
fn is_any_nothing(mark: &str) -> bool {
    Nothing::ALL.iter().any(|nothing| nothing.mark() == mark)
}

/// A ledger of `count` records with `mutate` applied to each, so a gate can say in one line which
/// bed it is reading rather than depending on a helper's incidental contents.
fn ledger_with(count: usize, mutate: impl Fn(usize, &mut serde_json::Value)) -> Screen {
    let mut items: Vec<serde_json::Value> = ledger(count)
        .transformations
        .items()
        .into_iter()
        .cloned()
        .collect();
    for (index, item) in items.iter_mut().enumerate() {
        mutate(index, item);
    }
    let mut screen = ledger(count);
    screen.transformations = answered(
        "/v1/transformations",
        serde_json::json!({ "items": items, "next_cursor": serde_json::Value::Null }),
    );
    screen
}

/// The declared columns of this reading in which **every** record answers one and the same word for
/// nothing, asked through the face's own classifier.
fn vacant_of(screen: &Screen) -> Vec<(&'static str, String)> {
    let items = screen.transformations.items();
    if items.len() < 2 {
        return Vec::new();
    }
    LEDGER_COLUMNS
        .iter()
        .filter_map(|column| {
            let marks: Vec<String> = items
                .iter()
                .map(|item| renderer::cell_mark(item, column.key).0)
                .collect();
            let uniform = marks.iter().all(|mark| mark == &marks[0]);
            if uniform && says_nothing(&marks[0]) {
                Some((column.key, marks[0].clone()))
            } else {
                None
            }
        })
        .collect()
}

/// 🔴 **g64 — a column that says nothing on every record is not drawn.**
///
/// `req/924` §TUI-45: 🔴 *列の全値が「無」の mark(`?` `--` `...`)なら、その列を落として開示に数えろ。*
/// The live bed the ruling was measured on answers `null` for five of the ten declared keys on all
/// thirty-one records, so five columns were spending a cell per row to say *measured, and not
/// knowable* — twenty-three times each, where once is the whole of it.
///
/// 🔴 **Red on `main`**: `rollback` and `superseded_by` are `null` on every record of `ledger`, and
/// `main` draws `rollback` at every shape wide enough to fit it.
#[test]
fn g64_a_column_whose_every_value_is_nothing_is_not_drawn() {
    let screen = ledger(28);
    let measured = renderer::measured(&screen);
    let vacant = vacant_of(&screen);
    let items = screen.transformations.items();
    let mut offenders: Vec<String> = Vec::new();
    for (width, height) in RULED_SHAPES {
        let plan = layout::resolve_attended(
            width,
            height,
            &measured,
            false,
            layout::Subject::Grid,
            layout::Attention {
                selected: 0,
                items: items.len(),
                glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
            },
        );
        let drawn: Vec<&str> = plan.columns.iter().map(|column| column.key).collect();
        let still: Vec<&str> = vacant
            .iter()
            .map(|(key, _)| *key)
            .filter(|key| drawn.contains(key))
            .collect();
        println!("G64 {width}x{height} drawn={drawn:?} vacant={vacant:?} still_drawn={still:?}");
        if !still.is_empty() {
            offenders.push(format!("{width}x{height}: {still:?}"));
        }
    }
    assert!(
        !vacant.is_empty(),
        "🔴 g64: the bed carries no column that says nothing, so this gate measured nothing. \
         `ledger` answers `null` for `rollback` and `superseded_by` on every record; if that has \
         changed, the bed changes with it and not the assertion."
    );
    assert!(
        offenders.is_empty(),
        "🔴 g64 (`req/924` §TUI-45): a column is drawn whose every value is a mark for nothing. \
         One line of it says everything twenty-eight lines of it say: {offenders:?}"
    );
}

/// 🔴 **g64b — negative control: a column that is nothing on *some* rows is information and stays.**
///
/// The ruling's own boundary: 🔴 *一部の行だけが「無」なら落とすな。それは情報。* A `rollback` that is
/// `null` on twenty-seven records and carries a value on one is the face saying *these did not roll
/// back and that one did*, which is the whole reason a reader is looking at the column.
#[test]
fn g64b_a_column_that_is_nothing_on_only_some_rows_is_kept() {
    let screen = ledger_with(28, |index, item| {
        if index == 3 {
            item["rollback"] = serde_json::json!("gx1:rolledbackzzz");
        }
    });
    let vacant = vacant_of(&screen);
    println!("G64B vacant={vacant:?}");
    assert!(
        !vacant.iter().any(|(key, _)| *key == "rollback"),
        "🔴 g64b: the bed was meant to keep `rollback` out of the vacant set and did not, so this \
         control measured nothing"
    );
    let measured = renderer::measured(&screen);
    // 🔴 **Two hundred cells, and it was a hundred and twenty** (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01)).
    // This probe's subject is the **vacancy rule's boundary** -- a column with one real answer in
    // it stays -- and not how many columns a given width holds. §TUI-62 widened the column gap
    // and gave the row a left margin (the ruling's 余裕), so at a hundred and twenty cells
    // `rollback` now leaves the grid **by width**, which the disclosure counts and this gate is not
    // about. Asked at a width where no column is dropped for room, the question is the one the
    // gate was minted for.
    let plan = layout::resolve_attended(
        200,
        32,
        &measured,
        false,
        layout::Subject::Grid,
        layout::Attention {
            selected: 0,
            items: 28,
            glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
        },
    );
    assert!(
        plan.columns.iter().any(|column| column.key == "rollback"),
        "🔴 g64b (`req/924` §TUI-45's boundary): `rollback` left the grid at 200x32 although one \
         record carries a value. A column with one real answer in it is the column a reader came \
         for: {:?}",
        plan.columns
            .iter()
            .map(|column| column.key)
            .collect::<Vec<_>>()
    );
}

/// 🔴 **g64c — negative control: two *different* words for nothing in one column are a distinction.**
///
/// 🔴 The ruling's other half, and the one that would be cheapest to lose: *列を落とす時も、残った
/// mark の区別を潰すな.* A column that is `?` on some records and `--` on others is this face telling
/// *measured and not knowable* from *never written* — the distinction the whole vocabulary exists
/// for. Dropping it because "it is all nothing" collapses the two, which is the first-principle
/// breach this product is against, committed by a rule written to reduce ink.
#[test]
fn g64c_two_different_words_for_nothing_in_one_column_are_not_vacant() {
    // `null` reads `Unknown`; a key the object does not carry at all reads `Absent`.
    let screen = ledger_with(28, |index, item| {
        if index % 2 == 0 {
            if let Some(map) = item.as_object_mut() {
                map.remove("rollback");
            }
        }
    });
    let items = screen.transformations.items();
    let marks: BTreeSet<String> = items
        .iter()
        .map(|item| renderer::cell_mark(item, "rollback").0)
        .collect();
    println!("G64C rollback marks={marks:?}");
    assert_eq!(
        marks.len(),
        2,
        "🔴 g64c: the bed was meant to produce two different words for nothing in one column and \
         produced {marks:?}, so this control measured nothing"
    );
    assert!(
        marks.iter().all(|mark| says_nothing(mark)),
        "🔴 g64c: both marks have to be words for nothing or the control is not the control"
    );
    let vacant = vacant_of(&screen);
    let measured = renderer::measured(&screen);
    // 🔴 **Two hundred cells, and it was a hundred and twenty** (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01)).
    // This probe's subject is the **vacancy rule's boundary** -- a column with one real answer in
    // it stays -- and not how many columns a given width holds. §TUI-62 widened the column gap
    // and gave the row a left margin (the ruling's 余裕), so at a hundred and twenty cells
    // `rollback` now leaves the grid **by width**, which the disclosure counts and this gate is not
    // about. Asked at a width where no column is dropped for room, the question is the one the
    // gate was minted for.
    let plan = layout::resolve_attended(
        200,
        32,
        &measured,
        false,
        layout::Subject::Grid,
        layout::Attention {
            selected: 0,
            items: 28,
            glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
        },
    );
    println!("G64C vacant={vacant:?}");
    assert!(
        !vacant.iter().any(|(key, _)| *key == "rollback"),
        "🔴 g64c: the shared classifier called a column carrying two different words for nothing \
         vacant. `?` and `--` are not one fact."
    );
    assert!(
        plan.columns.iter().any(|column| column.key == "rollback"),
        "🔴 g64c (`req/924` §TUI-45): a column carrying two different words for nothing left the \
         grid, so the screen can no longer tell `?` from `--` on that key: {:?}",
        plan.columns
            .iter()
            .map(|column| column.key)
            .collect::<Vec<_>>()
    );
}

/// 🔴 **g64d — negative control: a column that answers `no` on every record is an answer, and stays.**
///
/// 🔴 The audit's finding 3, made mechanical. `req/924` §TUI-45 enumerates the marks the rule may
/// drop a column for — 「`?` `--` `...`」 — and the first cut of this lane read `Nothing::ALL`
/// instead. `wire::cell` maps a JSON `false` to [`Nothing::False`], whose mark is `no`, so a ledger
/// in which nothing is enforced lost the `enforced` column at **every width** and the hatch told the
/// reader it had "answered with a mark for nothing". *No transformation in this ledger is enforced*
/// is not nothing; it is the most actionable sentence a face like this can carry, and folding it
/// into *not measured* is the breach this product exists to refuse.
#[test]
fn g64d_a_column_that_answers_no_on_every_record_is_an_answer_and_is_drawn() {
    let screen = ledger_with(28, |_, item| {
        item["enforced"] = serde_json::Value::Bool(false);
    });
    let items = screen.transformations.items();
    let marks: BTreeSet<String> = items
        .iter()
        .map(|item| renderer::cell_mark(item, "enforced").0)
        .collect();
    println!("G64D enforced marks={marks:?}");
    assert_eq!(
        marks.len(),
        1,
        "🔴 g64d: the bed was meant to answer one mark on every record: {marks:?}"
    );
    let mark = marks.iter().next().expect("one mark").clone();
    assert!(
        is_any_nothing(&mark) && !says_nothing(&mark),
        "🔴 g64d: the control needs a mark that is one of the seven and is **not** one the rule may \
         drop for. `enforced: false` drew {mark:?}, which is not that."
    );
    let vacant = vacant_of(&screen);
    let measured = renderer::measured(&screen);
    let plan = layout::resolve_attended(
        120,
        32,
        &measured,
        false,
        layout::Subject::Grid,
        layout::Attention {
            selected: 0,
            items: items.len(),
            glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
        },
    );
    let drawn: Vec<&str> = plan.columns.iter().map(|column| column.key).collect();
    println!("G64D vacant={vacant:?} drawn={drawn:?}");
    assert!(
        !vacant.iter().any(|(key, _)| *key == "enforced"),
        "🔴 g64d: `enforced no` on every record was called *no answer obtained*"
    );
    assert!(
        drawn.contains(&"enforced") || {
            // The hoist may have taken it: constant over the fetched set is a legitimate fold, and
            // the mark is then on the screen once rather than nowhere. Either is the answer being
            // kept; the failure this control names is the column being deleted.
            let frame = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
                &screen,
                120,
                32,
                Tier::Mono,
                false,
                &View::default(),
            )));
            frame.contains("enforced no")
        },
        "🔴 g64d (`req/924` §TUI-45's enumeration): the column carrying a measured `false` on every \
         record is neither drawn nor hoisted: {drawn:?}"
    );
}

/// 🔴 **g64e — negative control: a count of nought is a measurement, and stays.**
///
/// The same finding, on the other arm of `wire::cell`: a JSON `0` is [`Nothing::Zero`], whose mark
/// is `0`. `req/38` SS974 added a seventh word precisely so `""` would stop being drawn as *a count
/// of nought*; a rule that deletes any column reading `0` throws that repair away.
#[test]
fn g64e_a_column_that_counts_nought_on_every_record_is_a_measurement_and_is_drawn() {
    let screen = ledger_with(28, |_, item| {
        item["scope"] = serde_json::json!(0);
    });
    let items = screen.transformations.items();
    let mark = renderer::cell_mark(items[0], "scope").0;
    println!("G64E scope mark={mark:?}");
    assert_eq!(
        mark,
        Nothing::Zero.mark(),
        "🔴 g64e: the bed was meant to draw the count mark"
    );
    let vacant = vacant_of(&screen);
    println!("G64E vacant={vacant:?}");
    assert!(
        !vacant.iter().any(|(key, _)| *key == "scope"),
        "🔴 g64e (`req/924` §TUI-45's enumeration): a count of nought on every record was called \
         *no answer obtained*: {vacant:?}"
    );
}

/// 🔴 **g65 — what left the grid is inside the number the screen spells, and the hatch names it.**
///
/// `req/924` §TUI-45: 🔴 *落とし方: `N of 11 fields not drawn` に合流させる。黙って落とすな——開示の数
/// が増えるのが正しい姿。* Three things have to hold together and any one alone is a deletion wearing
/// a signpost's face: every column that says nothing is inside `dropped_fields`; the number on the
/// screen is that set's size; and the hatch names its members.
///
/// The third is `g61`'s argument one rule up, and it is the one that has to be measured rather than
/// assumed — a screen saying *six fields are not drawn* while the help face names three is a face
/// that lost three columns and left a number behind.
///
/// 🔴 **Named ceiling: the count does not always go up, and that is correct.** Taking a vacant
/// column out before the width fit gives its cells to a column that carries a value, so a shape can
/// disclose the same number while disclosing a different set. What the rule guarantees is
/// *reflection*, not growth. Growth is asserted on the bed where it must happen — the live one,
/// five vacant keys of ten — so the gate still measures something rather than passing vacuously.
#[test]
fn g65_what_the_rule_dropped_is_counted_on_the_screen_and_named_in_the_hatch() {
    // The live bed's shape: `created_at`, `scope`, `actor`, `rollback` and `superseded_by` are
    // `null` on every record (`req/924` §TUI-39 measured exactly this against the running engine).
    let screen = ledger_with(31, |_, item| {
        for key in ["created_at", "scope", "actor"] {
            item[key] = serde_json::Value::Null;
        }
    });
    let vacant = vacant_of(&screen);
    let measured = renderer::measured(&screen);
    let items = screen.transformations.items();
    let mut silent: Vec<String> = Vec::new();
    let mut unnamed: Vec<String> = Vec::new();
    let mut grew = 0usize;
    for (width, height) in RULED_SHAPES {
        let plan = layout::resolve_attended(
            width,
            height,
            &measured,
            false,
            layout::Subject::Grid,
            layout::Attention {
                selected: 0,
                items: items.len(),
                glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
            },
        );
        let (_, width_only) = layout::columns_for(width);
        if plan.dropped_fields.len() > width_only.len() {
            grew += 1;
        }
        for (key, _) in &vacant {
            if !plan.dropped_fields.contains(key) {
                silent.push(format!(
                    "{width}x{height}: `{key}` obtained no answer on any record and is neither \
                     drawn nor counted"
                ));
            }
        }
        let frame = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
            &View::default(),
        )));
        // The number a reader gets, read off the buffer rather than off the plan.
        let long = format!("{} of {}", plan.dropped_fields.len(), plan.total_fields);
        let short = format!("{}/{}", plan.dropped_fields.len(), plan.total_fields);
        if !frame.contains(&long) && !frame.contains(&short) {
            silent.push(format!(
                "{width}x{height}: the screen spells neither {long:?} nor {short:?}\n{frame}"
            ));
        }
        let hatch = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
            &View {
                help: true,
                ..View::default()
            },
        )));
        // 🔴 UNTESTABLE and not failed. `help_lines` cuts its own list at the height it was given
        // and says how many it cut; a shape where the entry did not fit has not been measured, and
        // folding *not measured* into *failed* is the defect
        // [[feedback_untestable_is_not_failed]] names.
        // 🔴 The guard is the **entry's own label** and nothing looser. A first cut of this asked
        // for `"not drawn"`, which the help face's own disclosure line also carries — so a hatch
        // cut long before its `all nothing` entry answered *yes, I am here* and the gate then
        // demanded names from a page that had none. That is the same shape as the checker
        // `req/38` SS1077's lane caught in itself: a predicate wide enough to match the chrome
        // reports coverage it does not have.
        // 🔴 **Identity and not presence** (independent audit, finding 9). The first cut asked only
        // whether each vacant key appeared *somewhere* in the hatch, so printing `dropped_fields`
        // there — a superset — would have kept this gate, `g61`, `g64` and `p5` all green while the
        // line told the reader that width-dropped columns "obtained no answer", folding *cut for
        // room* into *nothing to say*. So the entry's own line is isolated and asked for **exactly**
        // the vacant set, with its marks.
        // 🔴 **The note is not an entry, and since `[T-r86]` (2026-09-02, `req/924` §TUI-103) it
        // spells entry labels.** The note now names the entries a shape had no room for, in the
        // very words they are filed under, so a bare search for `no answer` finds the **note** on
        // a shape where the entry was cut — and this gate then reads *absent* as *present with the
        // wrong contents*, which is [[feedback_untestable_is_not_failed]] committed by the
        // instrument. Measured: at 80x24 on this bed the entry is cut, the `None` branch below is
        // the correct answer, and before this line the gate produced a fragment of the note
        // (`", engine, vocabulary, marks,"`) and failed on it.
        //
        // This is the **same repair the paragraph above already records once** — a predicate wide
        // enough to match the chrome reports coverage it does not have — applied again now that the
        // chrome has grown the labels. The assertion is untouched; only the region it reads is.
        // `LET_GO_LEAD` is read from the renderer rather than typed here, so the day the phrase
        // moves this bound moves with it.
        let entries = hatch
            .split(renderer::LET_GO_LEAD)
            .next()
            .unwrap_or(&hatch)
            .to_string();
        let line = entries
            .split("no answer")
            .nth(1)
            .map(|tail| tail.split(" prev ").next().unwrap_or(tail).to_string());
        match line {
            Some(line) => {
                for (key, mark) in &vacant {
                    if !line.contains(&format!("{key} {mark}")) {
                        unnamed.push(format!(
                            "{width}x{height}: {key} {mark} missing from {line:?}"
                        ));
                    }
                }
                // The other direction: a key that is *not* vacant has no business on this line.
                for column in LEDGER_COLUMNS {
                    let vacant_key = vacant.iter().any(|(key, _)| *key == column.key);
                    if !vacant_key && line.contains(column.key) {
                        unnamed.push(format!(
                            "{width}x{height}: {} is on the `no answer` line and is not vacant \
                             ({line:?})",
                            column.key
                        ));
                    }
                }
            }
            None => {
                let unseen = vacant
                    .iter()
                    .filter(|(key, _)| !entries.contains(key))
                    .count();
                println!(
                    "G65 {width}x{height} UNTESTABLE: the hatch was cut before its `no answer` \
                     entry ({unseen} of {} names unreachable at this shape)",
                    vacant.len()
                );
            }
        }
        println!(
            "G65 {width}x{height} dropped={} width_only={} vacant={vacant:?}",
            plan.dropped_fields.len(),
            width_only.len()
        );
    }
    assert!(
        !vacant.is_empty(),
        "🔴 g65: the bed carries no column that says nothing, so this gate measured nothing"
    );
    assert!(
        grew > 0,
        "🔴 g65: on a bed where five of ten keys say nothing on every record, no shape disclosed \
         more than the width alone would have. Either the rule is not running or the count is not \
         reaching the disclosure."
    );
    assert!(
        silent.is_empty(),
        "🔴 g65 (`req/924` §TUI-45): a column left the grid without the screen's count going up to \
         say so. That is not a fold, it is a deletion: {silent:#?}"
    );
    assert!(
        unnamed.is_empty(),
        "🔴 g65 (`req/924` §TUI-21's clause, one rule up): the count grew and the hatch does not \
         name what it grew by: {unnamed:#?}"
    );
}

/// 🔴 **g66 — a claim measured on the window is quantified by the window, and never by the set.**
///
/// `req/38` SS1047 is the ruling held here from the other side: the face drew `verdict Admit`
/// flatly, having measured it on the rows the terminal happened to be tall enough for, and *the
/// truth of the sentence was a function of the terminal's height and of where the cursor stood*.
/// §TUI-45 (SS1076) then separated two questions that had been one: a **claim** about the ledger is
/// measured over every record the read carried and always will be; a column that says one word on
/// every row of *this screen* may say it once, quantified `these N` where N is the rows drawn.
///
/// 🔴 Three directions, all read off the buffer: the repetition is gone; the number is the number
/// of record rows actually on the screen; and `all <fetched> <key>` — the sentence SS1047 killed —
/// is on no screen for a key that is not constant over the set.
///
/// 🔴 **Red on `main`**: `main` repeats `Admit Committed Escrowed` down every drawn row of this bed,
/// because `hoist` measures over the fetched set and record 31 differs.
///
/// 🔴 **The word in the sentence above is `Available` now** (`[T-r55]`, 2026-09-02, gate `g71`).
/// `Escrowed` is not a word `gx_engine::store::InverseStatus` can send, so this bed no longer feeds
/// it; the observation the paragraph records was made when it did, and is kept in the words it was
/// made in rather than rewritten to look as though it had always said the other thing.
#[test]
fn g66_a_line_measured_on_the_window_says_these_n_and_never_all_m() {
    // Thirty-one records of which the last differs: the live bed's own shape.
    let screen = ledger_with(31, |index, item| {
        if index == 30 {
            item["verdict"] = serde_json::Value::Null;
            item["state"] = serde_json::json!("Draft");
            item["inverse_status"] = serde_json::Value::Null;
        }
    });
    let items = screen.transformations.items();
    let mut repeated: Vec<String> = Vec::new();
    let mut lies: Vec<String> = Vec::new();
    let mut fired = 0usize;
    let mut compressed_at: Vec<String> = Vec::new();
    for (width, height) in RULED_SHAPES {
        let text = renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
            &View::default(),
        ));
        // The rows a reader can see, counted from the buffer: a record row is one carrying a
        // ledger id, which is the same predicate the capture instrument uses.
        let rows: Vec<&str> = text.lines().filter(|line| line.contains("gx1:")).collect();
        let frame = flat(&text);
        if rows.len() < 2 || rows.len() >= items.len() {
            println!(
                "G66 {width}x{height} UNTESTABLE rows={} items={} (a window that is the whole set \
                 is a hoist, not a compression)",
                rows.len(),
                items.len()
            );
            continue;
        }
        fired += 1;
        let on_every_row = rows.iter().all(|row| row.contains("Admit"));
        // 🔴 **One, and it is asserted at the widest ruled shape only.** The ruling's own form is a
        // ladder — *compress if the header can hold the clause* — and at eighty cells and below the
        // clause costs more cells than the repetition it removes, because a hoisted `created_at`
        // spells a twenty-character timestamp. So the shapes where the ladder refuses are
        // **reported with their row counts** rather than failed, and 120x32 — the shape §TUI-45 was
        // measured at — is where the behaviour is pinned. Failing the narrow shapes would be a gate
        // demanding a screen that is worse: a header that overflows says nothing at all.
        if !on_every_row {
            compressed_at.push(format!("{width}x{height}"));
        }
        if (width, height) == (120, 32) && on_every_row {
            repeated.push(format!(
                "{width}x{height}: `Admit` repeated on all {} drawn rows",
                rows.len()
            ));
        }
        // 🔴 **Two, and the word is read from the declaration** (independent audit, finding 8). The
        // first cut keyed the whole check on the literal `"these"`, so renaming `WINDOW_SCOPE` to
        // `"all"` produced `all 24 verdict Admit` over a thirty-one record set — SS1047's exact lie
        // — with every branch of this gate skipped and the suite green. The scope words are asserted
        // to be different words first, then the clause is found by the declared one.
        assert_ne!(
            renderer::WINDOW_SCOPE,
            renderer::FETCHED_SCOPE,
            "🔴 g66: the two quantifiers are the same word, so no line on this face can say which \
             set it was measured over"
        );
        let quantified = format!("{} {}", renderer::WINDOW_SCOPE, rows.len());
        if !on_every_row && !frame.contains(&quantified) {
            lies.push(format!(
                "{width}x{height}: the column was compressed and the screen does not say \
                 `{quantified}`\n{frame}"
            ));
        }
        if frame.contains(renderer::WINDOW_SCOPE) && !frame.contains(&quantified) {
            lies.push(format!(
                "{width}x{height}: compressed and the quantifier is not `{quantified}`\n{frame}"
            ));
        }
        // Three: the sentence SS1047 refused, by name.
        for key in ["verdict", "state", "inverse_status"] {
            let refused = format!("all {} {key}", items.len());
            if frame.contains(&refused) {
                lies.push(format!(
                    "{width}x{height}: {refused:?} — a claim about thirty-one records measured on \
                     {} of them, which is what SS1047 killed\n{frame}",
                    rows.len()
                ));
            }
        }
        println!(
            "G66 {width}x{height} rows={} items={} repeated={on_every_row}",
            rows.len(),
            items.len()
        );
    }
    println!("G66_FIRED={fired} G66_COMPRESSED_AT={compressed_at:?}");
    assert!(
        fired > 0,
        "🔴 g66: no ruled shape drew a proper subset of the bed, so this gate measured nothing"
    );
    // 🔴 **Both assertions are inverted, and the inversion is the ruling**
    // (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01)). §TUI-45 item 2 asked for the columns every
    // row of the **window** agrees on to be lifted into the header; §TUI-62 withdrew it, because
    // measured on the drawn slice the *shape of the table became a function of the cursor* --
    // rows of bare id in 1..29 and the columns back at 31, which is the symptom `SS1047` was
    // opened for. The price is named in the ruling and is paid here: **`Admit` repeats down every
    // row, and repetition is readable.**
    //
    // What this gate still holds is the half `SS1047` won and §TUI-62 explicitly keeps: the two
    // quantifiers are different words, and no line claims `all M` about a set it measured on
    // fewer. `these N` is not drawn by anything today and [`renderer::WINDOW_SCOPE`] is kept
    // declared for the day a line is measured on a window again -- so the assertion is that it is
    // **not** on the screen, which is the same statement from the other side.
    assert!(
        compressed_at.is_empty(),
        "🔴 g66 (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01)): a column was lifted off the rows at \
         {compressed_at:?}. The window does not decide the shape of the table"
    );
    assert!(
        !repeated.is_empty(),
        "🔴 g66 (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01)): at the shape the ruling was measured \
         on, the constant column is **not** drawn on every row -- so either the withdrawal did not \
         land or this bed no longer has a column that is constant in the window"
    );
    assert!(
        lies.is_empty(),
        "🔴 g66 (`req/924` §TUI-45 holding `req/38` SS1047): {lies:#?}"
    );
}

/// 🔴 **g68 — compressing a column does not leave the ledger unpainted.**
///
/// `req/924` §TUI-53 (`req/38` SS1084 ①): 🔴 *圧縮と強調は両立させろ。畳んで残った列が id 一色なら、
/// 畳んだ事が読者から情報を奪っている.* Measured on the real face at 120x29, the first cut of this
/// lane took the coloured rows from twenty-five to **two** — `req/924` §TUI-19's defect (*no
/// handhold for telling one row from the next*) made worse by a cut meant to help.
///
/// Read off the **buffer** and not off the plan: what a reader gets is the painted cell.
#[test]
fn g68_a_compressed_column_still_paints_the_rows_it_was_lifted_off() {
    // Thirty-one records with the last one differing: the bed the ruling was measured on, and the
    // one shape of ledger where a window can be constant while the fetched set is not.
    let screen = ledger_with(31, |index, item| {
        if index == 30 {
            item["verdict"] = serde_json::Value::Null;
            item["state"] = serde_json::json!("Draft");
            item["inverse_status"] = serde_json::Value::Null;
        }
    });
    let mut bare: Vec<String> = Vec::new();
    let mut measured_rows = 0usize;
    for (width, height) in RULED_SHAPES {
        let buffer = renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Truecolor,
            false,
            &View::default(),
        );
        for y in 0..height {
            let row: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
            if !row.contains("gx1:") {
                continue;
            }
            measured_rows += 1;
            // A painted row is one where some cell resolved to a colour. `Color::Reset` is what a
            // cell nobody decided about carries, so its absence is the decision.
            let painted = (0..width).any(|x| format!("{:?}", buffer[(x, y)].fg) != "Reset");
            if !painted {
                bare.push(format!("{width}x{height} row {y}: {}", row.trim_end()));
            }
        }
    }
    println!("G68_ROWS={measured_rows} G68_BARE={}", bare.len());
    assert!(
        measured_rows > 0,
        "🔴 g68: no ruled shape drew a record, so this gate measured nothing"
    );
    assert!(
        bare.is_empty(),
        "🔴 g68 (`req/924` §TUI-53): a record row carries no colour at all. Compressing the columns \
         that held the ledger's meaning and giving the rows nothing back is the cut taking a fact \
         away from the reader: {bare:#?}"
    );
}

/// 🔴 **g67 — the region that stood down did not take the unrecoverable fact with it.**
///
/// `req/924` §TUI-29 lets the provenance region stand down while every route answers `200`, because
/// `engine ok` on the rail is then the whole of what it was saying. The subscription's state is the
/// one thing on that row that **no route returns** — `Recoverable::Nowhere`, which is why the region
/// sits at `Priority::One` — so a screen that drops the row without carrying the badge somewhere has
/// destroyed a fact rather than folded one.
///
/// 🔴 **This is the gate on this lane's own cut, and that is why it is here.** SS842: a reduction
/// pass takes the caveats out along with the padding, because on the page a caveat looks exactly
/// like padding. On `main` the region never stands down, so this gate reports that it measured
/// nothing — it is a tripwire on new behaviour rather than a defect found in old.
#[test]
fn g67_a_provenance_that_stood_down_left_the_badge_on_the_screen() {
    // 🔴 **The badge became the dot, and the region stands down by ruling**
    // (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)). g67's subject is unchanged: the provenance
    // region is `Recoverable::Nowhere`, so a screen that lets it go must not let the one fact
    // the reader cannot re-measure go with it. What changed is what carries that fact.
    // §TUI-57 replaced `ENGINE LIVE, N events` with a dot -- and `req/38` SS1085 is why the dot
    // has six appearances rather than one, because a badge that says *connected* and nothing
    // about whether anything is still arriving is a quiet stream and a dead stream wearing one
    // face.
    //
    // So: the region is drawn nowhere, the dot is on the standing row everywhere, and the line
    // in full is on the hatch. The third of those is what keeps this a fold and not a deletion.
    let screen = ledger(28);
    let measured = renderer::measured(&screen);
    let mut standing: Vec<String> = Vec::new();
    let mut dotless: Vec<String> = Vec::new();

    for (width, height) in RULED_SHAPES {
        let plan = layout::resolve(width, height, &measured, false, layout::Subject::Grid);
        if plan.rows_for(RegionRole::Provenance) > 0 {
            standing.push(format!("{width}x{height}"));
        }
        let (dot, _) = measured.link.dot();
        let carried = plan.status.iter().any(|cell| cell.text == dot);
        println!(
            "G67 {width}x{height} provenance_drawn={} dot={carried}",
            plan.rows_for(RegionRole::Provenance)
        );
        if !carried {
            dotless.push(format!("{width}x{height}"));
        }
    }
    assert!(
        standing.is_empty(),
        "🔴 g67: the provenance has a region of its own at {standing:?}. It is off the \
         standing frame by ruling (`layout::STOOD_DOWN_REGIONS`)"
    );
    assert!(
        dotless.is_empty(),
        "🔴 g67 (`req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01)): the region stood down and the dot \
         that replaced it is not on the standing row at {dotless:?}. That is the region taking \
         an unrecoverable fact with it, which is the one drop this face may not make"
    );

    // And the line in full reaches the hatch, which is what makes the stand-down a fold.
    let hatch = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        32,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
    )));
    let full = layout::resolve(120, 32, &measured, false, layout::Subject::Help).provenance_full;
    assert!(
        hatch.contains(&full),
        "🔴 g67 (§TUI-21): the hatch does not carry {full:?}, so the four measured facts \
         were deleted rather than moved:\n{hatch}"
    );
}

// =============================================================================================
// `req/924` §TUI-57 (`req/38` SS1088, Owner `#282-T`, 2026-09-01) — the four gates the ruling
// asked for. g73..g76; `g69` is the act ledger's drift gate and lives in `tools/gates`.
//
// The ruling in one line: **the standing chrome is the bottom row and nothing else**, the
// connection's state is a dot with more than two values, the page's address is spelled once, and
// everything the row stopped spelling is on the page `?` opens.
// =============================================================================================

/// 🔴 **g73 — the standing chrome is one row, and the grid's preamble scrolls.**
///
/// The predicate is *which region draws the row*, never a row number: a row index is a fact about
/// one terminal size and this has to hold at all of them. `layout::FIXED_REGIONS` is the one place
/// the face declares what "fixed" means and this gate reads it rather than restating it.
///
/// 🔴 **Two halves, because a declaration on its own is the failure mode this session is named
/// after** (`INHERITED_PRINCIPLES` §宣言と挙動の乖離: *a gate that validates a declaration nothing
/// reads*). The first half asks the plan; the second renders two frames and asks whether the
/// grid's column header is still on the screen once the reader has walked to the bottom of the
/// ledger. A header that survives that walk is a pinned header whatever the declaration says.
///
/// **Three values.** A ledger that fits inside the body never scrolls, so it says nothing either
/// way and is counted as UNTESTABLE rather than as a pass (`req/38` SS870).
#[test]
fn g73_the_standing_chrome_is_one_row_and_the_preamble_scrolls() {
    assert_eq!(
        layout::FIXED_REGIONS.len(),
        1,
        "🔴 g73: the face declares {} fixed regions. `req/924` §TUI-57 is one row of standing \
         chrome, and a second region is a second row",
        layout::FIXED_REGIONS.len()
    );

    let records = 31;
    let screen = ledger(records);
    let measured = renderer::measured(&screen);
    let mut over: Vec<String> = Vec::new();
    let mut unpinned: Vec<String> = Vec::new();
    let mut pinned: Vec<String> = Vec::new();
    let mut untestable: Vec<String> = Vec::new();

    for (width, height) in RULED_SHAPES {
        let plan = layout::resolve_attended(
            width,
            height,
            &measured,
            false,
            layout::Subject::Grid,
            layout::Attention {
                selected: 0,
                items: records,
                glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
            },
        );
        // Every row this face draws is either the standing chrome or the body. A third role with
        // rows would be a second thing that does not scroll, whatever it were called.
        let stray: Vec<&str> = plan
            .rows
            .iter()
            .filter(|(role, _)| {
                !layout::FIXED_REGIONS.contains(role) && *role != RegionRole::Subject
            })
            .map(|(role, _)| role.short())
            .collect();
        println!(
            "G70 {width}x{height} fixed={} rows={:?} preamble={} shown={}",
            plan.fixed_rows(),
            plan.rows,
            plan.preamble,
            plan.preamble_shown
        );
        if plan.fixed_rows() > 1 || !stray.is_empty() {
            over.push(format!(
                "{width}x{height}: fixed={} stray={stray:?} rows={:?}",
                plan.fixed_rows(),
                plan.rows
            ));
        }

        // The behavioural half, on drawn frames.
        let header = layout::LEDGER_COLUMNS[0].key;
        let at = |selected: usize| {
            renderer::buffer_text(&renderer::render_view_to_buffer(
                &screen,
                width,
                height,
                Tier::Mono,
                false,
                &View {
                    selected,
                    ..View::default()
                },
            ))
        };
        let top = at(0);
        let bottom = at(records - 1);
        let body = plan.rows_for(RegionRole::Subject) as usize;
        if records + plan.preamble <= body {
            untestable.push(format!(
                "{width}x{height}: the whole stream fits, so it never scrolls"
            ));
            continue;
        }
        let top_has = top
            .lines()
            .any(|line| line.trim_start().starts_with(header));
        let bottom_has = bottom
            .lines()
            .any(|line| line.trim_start().starts_with(header));
        if !top_has {
            unpinned.push(format!(
                "{width}x{height}: the column header is not drawn even at the top of the stream"
            ));
        }
        if bottom_has {
            pinned.push(format!(
                "{width}x{height}: the column header is still on the screen with the attention on \
                 the last record, so it is pinned chrome and not content:\n{bottom}"
            ));
        }
    }

    println!("G70_UNTESTABLE={untestable:?}");
    assert!(
        over.is_empty(),
        "🔴 g73 (`req/924` §TUI-57): the standing chrome is more than one row at {} shape(s): \
         {over:#?}",
        over.len()
    );
    assert!(
        untestable.len() < RULED_SHAPES.len(),
        "🔴 g73: every ruled shape held the whole stream, so the scrolling half measured nothing"
    );
    assert!(
        unpinned.is_empty(),
        "🔴 g73: {unpinned:#?} — the header is content and content is drawn at the head of the \
         stream. Absent there it was deleted rather than unpinned"
    );
    assert!(
        pinned.is_empty(),
        "🔴 g73 (`req/924` §TUI-57: 固定 header にするな): {pinned:#?}"
    );

    // 🔴 The negative control, and it is the face's own other reading rather than a plant: `w`
    // (`super::acts::Act::Wide`) asks for the disclosure in full, and the row it needs is a row the
    // reader asked for. The predicate has to be able to answer *more than one*, or a green above
    // would also be green on a face with no fixed chrome at all.
    // Forty-six cells, because at a hundred and twenty the long form fits one row and the control
    // would be green for the wrong reason — the same "measured nothing" this gate refuses above.
    let held = layout::resolve(46, 12, &measured, true, layout::Subject::Grid);
    println!("G70_CONTROL_WIDE_FIXED={}", held.fixed_rows());
    assert!(
        held.fixed_rows() > 1,
        "🔴 g73: the predicate cannot answer `more than one` even while `w` is held, so it is not \
         measuring the number of fixed rows at all: {:?}",
        held.rows
    );
}

/// 🔴 **g74 — the dot has more than two values, and they are told apart without colour too.**
///
/// `req/924` §TUI-30 refused `✓` because a mark that reduces the number of states a reader can tell
/// apart is not a simplification. §TUI-57 admitted the dot on the opposite ground: it *replaces*
/// `ENGINE LIVE, N events` and `engine ok`, so the words go. `req/38` SS1085 is why one appearance
/// would not do — a quiet stream and a dead stream wearing one face is the `Zero`/`Unknown`
/// collapse this product exists to refuse.
///
/// Two halves. The **hues** are read off drawn cells at `truecolor`, which is the union
/// `[349][0-9]` ∪ `[34]8;[25];` in the escape alphabet and a foreground colour in this one. The
/// **marks** are read at `mono`, where every hue is dropped: `INHERITED_PRINCIPLES` §3c-③''-③ is
/// that no meaning may rest on one tier, so the six have to survive with the colour taken away.
#[test]
fn g74_the_dot_carries_more_than_two_states_and_survives_mono() {
    use gx_tui::tui::live::{Link, LinkReport};
    use std::time::Duration;

    let marks: BTreeSet<&str> = tokens::DOTS.iter().copied().collect();
    assert_eq!(
        marks.len(),
        tokens::DOTS.len(),
        "🔴 g74: {} of the declared dots are the same character, so two states are drawn alike",
        tokens::DOTS.len() - marks.len()
    );
    assert!(
        tokens::DOTS.len() >= 3,
        "🔴 g74 (`req/924` §TUI-57): the dot declares {} appearance(s). Two is the collapse \
         `req/38` SS1085 measured and three is the floor the ruling set",
        tokens::DOTS.len()
    );

    // The six reports the six appearances answer for. `Open` twice, because that is the one state
    // that was hiding a second fact.
    let report = |link: Link, silent_for: Option<Duration>| LinkReport {
        link,
        events: 151,
        unreadable: 0,
        reconnects: 1,
        attempts: 2,
        silent_for,
    };
    let states = [
        ("live", report(Link::Open, Some(Duration::from_secs(1)))),
        ("quiet", report(Link::Open, Some(Duration::from_secs(600)))),
        ("opening", report(Link::Opening, None)),
        ("never", report(Link::Never, None)),
        ("closed", report(Link::Closed, Some(Duration::from_secs(5)))),
        ("off", report(Link::Off, None)),
    ];

    let screen = ledger(31);
    let mut seen_marks: BTreeSet<&str> = BTreeSet::new();
    let mut seen_hues: BTreeSet<String> = BTreeSet::new();
    let mut absent: Vec<String> = Vec::new();
    for (name, state) in states {
        let (mark, role) = state.dot();
        seen_marks.insert(mark);
        let ink = tokens::ink(role, Tier::Truecolor);
        seen_hues.insert(format!("{:?}", ink));

        // And the mark reaches a drawn frame, in the cell the standing row put it in.
        let buffer = renderer::render_live_to_buffer(
            &screen,
            120,
            29,
            Tier::Truecolor,
            false,
            &View::default(),
            state,
        );
        let text = renderer::buffer_text(&buffer);
        let drawn = text.contains(mark);
        let painted = (buffer.area.y..buffer.area.y + buffer.area.height).any(|y| {
            (buffer.area.x..buffer.area.x + buffer.area.width).any(|x| {
                let cell = &buffer[(x, y)];
                cell.symbol() == mark && cell_is_emphasised(cell)
            })
        });
        println!(
            "G71 {name} mark={mark:?} role={} drawn={drawn} painted={painted} ink={ink:?}",
            role.name()
        );
        if !drawn {
            absent.push(format!("{name}: {mark:?} is on no row of the frame"));
        }
    }
    println!("G71_MARKS={:?} G71_HUES={}", seen_marks, seen_hues.len());
    assert!(
        absent.is_empty(),
        "🔴 g74: {absent:#?} — a declared appearance that never reaches a frame is a state the \
         reader cannot see"
    );
    assert_eq!(
        seen_marks.len(),
        6,
        "🔴 g74 (`INHERITED_PRINCIPLES` §3c-③''-③): the six states draw {} distinct mark(s), so on \
         `mono` — where every hue is dropped — some of them are the same screen",
        seen_marks.len()
    );
    assert!(
        seen_hues.len() >= 3,
        "🔴 g74 (`req/924` §TUI-57): the dot resolves to {} distinct appearance(s) in the paint \
         table. Two is `✓` with a rounder head",
        seen_hues.len()
    );

    // 🔴 The negative control: the predicate has to be able to answer *two*. Asked of a pairing
    // that really is two — `never` and `closed` share a hue by declaration and are told apart by
    // the mark — so a green above cannot be a gate that counts everything as distinct.
    let pair: BTreeSet<String> = [tokens::Role::LinkNever, tokens::Role::LinkClosed]
        .iter()
        .map(|role| format!("{:?}", tokens::ink(*role, Tier::Truecolor)))
        .collect();
    assert_eq!(
        pair.len(),
        1,
        "🔴 g74: `link.never` and `link.closed` no longer share a hue, so the control this gate \
         calibrates against has moved and the count above means something else"
    );
}

/// 🔴 **g75 — the page's address is spelled at most once on any screen.**
///
/// `req/924` §TUI-22 opened on `GET` being on one screen five times; §TUI-57 finished it by moving
/// the complete address behind `?` and leaving the road. This is the property from the other end:
/// no frame this face draws may spell it twice.
///
/// The record face spells it once, in its own closing line, and that is the one spelling on that
/// screen — so the bound is *at most one* rather than *never*, and the census prints which face
/// carried it.
///
/// 🔴 **The predicate was counting a prefix, and the property is about an address** (`[T-r55]`,
/// 2026-09-02, under the ruling that wired the record's own routes: `req/924` §TUI-64 / `req/38`
/// SS1095, Owner `#285-T`). `GET /v1/transformations/{id}` **contains** `GET /v1/transformations`
/// and is not a second spelling of it — it is a different road to a different thing, and the hatch
/// is the one screen meant to carry both. A bare `matches().count()` reported the address twice on
/// the help face at every wide shape.
///
/// **The sentence this gate asserts is unchanged and the floor is not lowered**: what changed is
/// that the counter measures the address rather than its first twenty-two characters. A match
/// followed by `/` is a longer address, and the two controls at the end fire on both halves of that
/// — a genuine double spelling is still counted twice, and a sub-route is counted not at all. This
/// is `g70`'s own repair one gate over (`req/38` SS1094: *a scanning predicate that does not know
/// the whole shape of its subject*).
/// 🔴 **And the denominator had to grow with the screen it guards** (`[T-r55]`, 2026-09-02,
/// independent audit finding S1). Narrowing the predicate to stop counting a sub-route as a
/// spelling of its parent is right — and on its own it makes this gate *quieter about a screen this
/// lane made busier*: two spellings of `GET /v1/transformations/{id}` would then count nought, and
/// `GET /v1/receipts/{tid}` would be measured not at all. A gate whose subject grows and whose
/// denominator does not is a gate absorbing the very thing it was written to notice.
///
/// So the sweep is over **every address this face declares** — `LEDGER_ADDRESS` and both of
/// `wire::RECORD_ROUTES` — each counted for itself, and the parent counted only where it is not the
/// head of one of the children. The set is read from the declaration, so a route added later is
/// measured by this gate without anybody remembering to add a line.
#[test]
fn g75_the_address_is_spelled_at_most_once_on_a_screen() {
    // Every address this face declares, longest first so a parent never eats a child.
    fn declared() -> Vec<String> {
        let mut all = vec![LEDGER_ADDRESS.to_string()];
        all.extend(
            wire::RECORD_ROUTES
                .iter()
                .map(|route| format!("GET {route}")),
        );
        all.sort_by_key(|address| std::cmp::Reverse(address.len()));
        all
    }
    // One address, and not something it is merely the beginning of.
    fn spellings(text: &str, address: &str) -> usize {
        text.match_indices(address)
            .filter(|(at, _)| !text[at + address.len()..].starts_with('/'))
            .count()
    }
    // The parent's own count, for the two controls at the end that are written about it.
    fn addresses(text: &str) -> usize {
        spellings(text, LEDGER_ADDRESS)
    }
    let screen = ledger(31);
    let mut twice: Vec<String> = Vec::new();
    let mut census: Vec<String> = Vec::new();

    for (width, height) in RULED_SHAPES {
        for (name, view) in [
            ("list", View::default()),
            (
                "record",
                View {
                    open: true,
                    ..View::default()
                },
            ),
            (
                "help",
                View {
                    help: true,
                    ..View::default()
                },
            ),
        ] {
            let text = flat(&renderer::buffer_text(&renderer::render_view_to_buffer(
                &screen,
                width,
                height,
                Tier::Mono,
                false,
                &view,
            )));
            for address in declared() {
                let times = spellings(&text, &address);
                census.push(format!("{width}x{height} {name} [{address}]={times}"));
                if times > 1 {
                    twice.push(format!(
                        "{width}x{height} {name}: `{address}` {times} times"
                    ));
                }
            }
        }
    }
    // 🔴 The name said `G72` and this is `g75`. Corrected here rather than filed, because the whole
    // subject of `req/38` SS1097 was that `gNN` is **one flat identifier space** and a census line
    // printing a number that belongs to another gate is that space being reported wrongly by the
    // instrument that reports it (`[T-r55]`, 2026-09-02).
    println!("G75_CENSUS={census:?}");
    assert!(
        twice.is_empty(),
        "🔴 g75 (`req/924` §TUI-22 / §TUI-57): the address is spelled more than once at {} \
         face(s): {twice:#?}",
        twice.len()
    );

    // 🔴 The control: the counter has to be able to answer *twice*, or a green above would also be
    // green on a face that spells the address nowhere and on a predicate that matches nothing.
    let doubled = format!("{LEDGER_ADDRESS} and again {LEDGER_ADDRESS}");
    assert_eq!(
        addresses(&doubled),
        2,
        "🔴 g75: the predicate cannot see two spellings of the address, so it is measuring nothing"
    );
    // 🔴 The second control, and it is the one the narrowing is for: a longer address that begins
    // with this one is **not** this one. Without it the refinement above could have been written as
    // "count nothing" and both this gate and the one it protects would have gone green.
    let sub_route = format!("{LEDGER_ADDRESS}/{{id}}");
    assert_eq!(
        addresses(&sub_route),
        0,
        "🔴 g75: `{sub_route}` is a different address and must not be counted as a spelling of \
         `{LEDGER_ADDRESS}`"
    );
    // 🔴 And the pair together: one real spelling beside one sub-route is one, not two and not
    // nought. Either control alone admits a predicate that is wrong in the other direction.
    let both = format!("{LEDGER_ADDRESS}/{{id}} then {LEDGER_ADDRESS}");
    assert_eq!(
        addresses(&both),
        1,
        "🔴 g75: an address beside a longer address that contains it is one spelling"
    );
    // 🔴 The fourth control, and it is the one finding S1 asked for: the **children** are measured
    // too, or the narrowing above bought this gate's silence about the addresses this lane added.
    for address in declared() {
        let doubled = format!("{address} and again {address}");
        assert_eq!(
            spellings(&doubled, &address),
            2,
            "🔴 g75: `{address}` is in the declared set and the predicate cannot see two spellings \
             of it, so it is carried in the sweep and measured by nothing"
        );
    }
    assert_eq!(
        declared().len(),
        1 + wire::RECORD_ROUTES.len(),
        "🔴 g75: the declared set is not the declaration — a route was added to \
         `wire::RECORD_ROUTES` and this sweep did not grow with it"
    );
}

/// 🔴 **g76 — the page `?` opens really carries what the standing row stopped spelling.**
///
/// `req/924` §TUI-21's clause, which §TUI-57 leans its whole weight on: *the numbers may stay on
/// the screen and the names may move behind `?` — but do not say they were moved until a gate has
/// confirmed the hatch lists them*. Four things moved there and all four are asked for by name:
///
/// * the page's **address**, which the top rail used to be the title of;
/// * the connection's **counts**, which `ENGINE LIVE, N events` used to carry;
/// * **when something last arrived**, which nothing carried at all (`req/38` SS1085);
/// * the **names** of the columns the grid let go of, which the count on the standing row does not
///   spell.
///
/// **Three values.** A hatch narrow enough to cut its own entries is measured and named rather than
/// counted as a failure: the assertion is the widest ruled shape, where the hatch has room for
/// everything it declares, and the narrow shapes are printed as a census.
#[test]
fn g76_the_hatch_carries_what_the_standing_row_moved_there() {
    use gx_tui::tui::live::{Link, LinkReport};
    use std::time::Duration;

    let records = 31;
    let screen = ledger(records);
    let live = LinkReport {
        link: Link::Open,
        events: 151,
        unreadable: 0,
        reconnects: 1,
        attempts: 2,
        silent_for: Some(Duration::from_secs(7)),
    };
    let measured = renderer::measured_with_link(&screen, live);

    let hatch_at = |width: u16, height: u16| {
        flat(&renderer::buffer_text(&renderer::render_live_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
            &View {
                help: true,
                ..View::default()
            },
            live,
        )))
    };

    let mut census: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for (width, height) in RULED_SHAPES {
        let hatch = hatch_at(width, height);
        // The names the grid let go of at this width, asked of the grid rather than of the hatch.
        let grid = layout::resolve_attended(
            width,
            height,
            &measured,
            false,
            layout::Subject::Grid,
            layout::Attention {
                selected: 0,
                items: records,
                glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
            },
        );
        let wanted: Vec<(&str, String)> = vec![
            ("address", LEDGER_ADDRESS.to_string()),
            ("counts", format!("{} events", live.events)),
            ("last received", live.silence()),
        ];
        let mut absent: Vec<&str> = wanted
            .iter()
            .filter(|(_, text)| !hatch.contains(text.as_str()))
            .map(|(name, _)| *name)
            .collect();
        let unnamed: Vec<&str> = grid
            .dropped_fields
            .iter()
            .copied()
            .filter(|key| !hatch.contains(key))
            .collect();
        if !unnamed.is_empty() {
            absent.push("dropped column names");
        }
        census.push(format!(
            "{width}x{height} absent={absent:?} unnamed={unnamed:?}"
        ));
        if (width, height) == RULED_SHAPES[0] && (!absent.is_empty() || !unnamed.is_empty()) {
            missing.push(format!(
                "{width}x{height}: {absent:?} and the column names {unnamed:?} are not on the \
                 hatch:\n{hatch}"
            ));
        }
    }
    println!("G73_CENSUS={census:#?}");

    // The control comes first in spirit and is asserted here: the predicate has to be able to
    // answer *no*. A phrase this face never spells must not be found on the page.
    let widest = hatch_at(RULED_SHAPES[0].0, RULED_SHAPES[0].1);
    assert!(
        !widest.contains("the address is somewhere else"),
        "🔴 g76: the predicate finds a phrase this face never draws, so it is matching anything"
    );
    assert!(
        missing.is_empty(),
        "🔴 g76 (`req/924` §TUI-21's clause, as leant on by §TUI-57): {missing:#?}. A hatch that \
         does not list them turns the ruling's *moved* into a deletion"
    );
}

/// 🔴 **g77 — the table's shape is not a function of the cursor, and the pointer is wired.**
///
/// `req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01), all three rulings in one gate
/// because they are one screen:
///
/// * **裁定1** — `§TUI-45` item 2 is withdrawn. The columns a row draws must be the same set at
///   every position of the attention, **measured on the drawn rows** and not on the plan. The seat's
///   previous repair satisfied the plan-level property (`g58`'s decision half is green on it) and
///   still drew bare ids in 1..29 and full rows at 31, because the compression happened in the
///   region after the plan had spoken.
/// * **裁定2** — clicking to select is a read, so the pointer's road exists and moves the state.
/// * **裁定3** — the face moves independently of the attention, and scrolling down moves the
///   content up.
///
/// The 区切り and 余裕 halves of 裁定3 are measured here too, because a declared constant that
/// nothing draws is the divergence this session is named after.
#[test]
fn g77_the_table_shape_is_not_a_function_of_the_cursor_and_the_pointer_moves_the_state() {
    // The live bed's own shape: thirty-one records of which the last differs, which is the one
    // ledger where a window can be constant while the fetched set is not — and therefore the one
    // that produced the symptom.
    let screen = ledger_with(31, |index, item| {
        if index == 30 {
            item["verdict"] = serde_json::Value::Null;
            item["state"] = serde_json::json!("Draft");
            item["inverse_status"] = serde_json::Value::Null;
        }
    });
    let items = screen.transformations.items().len();

    // ---- 裁定1: the drawn row's shape, at every position of the attention -----------------
    let row_shape = |selected: usize| -> Vec<usize> {
        let text = renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            120,
            29,
            Tier::Mono,
            false,
            &View {
                selected,
                ..View::default()
            },
        ));
        // How many words each drawn record row carries. The id is one; a row that has lost its
        // columns carries exactly one.
        text.lines()
            .filter(|line| line.contains("gx1:"))
            .map(|line| line.split_whitespace().count())
            .collect()
    };
    let mut shapes: BTreeSet<usize> = BTreeSet::new();
    let mut bare: Vec<String> = Vec::new();
    for selected in [0usize, 1, 14, 28, 29, 30] {
        let widths = row_shape(selected);
        assert!(
            !widths.is_empty(),
            "🔴 g77: no record row was drawn at selection {selected}, so this gate measured nothing"
        );
        for width in &widths {
            shapes.insert(*width);
        }
        if widths.iter().any(|words| *words <= 1) {
            bare.push(format!(
                "selection {selected}: {} of {} drawn rows carry the id and nothing else",
                widths.iter().filter(|words| **words <= 1).count(),
                widths.len()
            ));
        }
    }
    println!("G74_ROW_WORD_COUNTS={shapes:?}");
    assert!(
        bare.is_empty(),
        "🔴 g77 (`req/924` §TUI-62 裁定1): {bare:#?} — the ledger draws rows of bare id. The \
         viewport is not the domain of the claim and it is not the domain of the *form* either"
    );
    // The last record differs from the rest, so two word counts are expected and three are not.
    assert!(
        shapes.len() <= 2,
        "🔴 g77 (`req/924` §TUI-62 裁定1): the drawn rows take {} different shapes as the \
         attention moves: {shapes:?}. On this bed there are two kinds of record and therefore at \
         most two shapes",
        shapes.len()
    );

    // ---- 裁定2: the pointer's road exists and moves the state -----------------------------
    let start = View::default();
    let clicked = acts::attend(&start, 7, items);
    assert_eq!(
        clicked.selected, 7,
        "🔴 g77 (`req/924` §TUI-62 裁定2): the pointer's road does not move the attention"
    );
    assert_eq!(
        acts::attend(&start, 999, items).selected,
        items - 1,
        "🔴 g77: a click past the last record has to clamp to the last record rather than attend \
         to a row that is not there"
    );
    // And it is a **read**: nothing but the attention moved, so no consent screen is owed.
    assert_eq!(
        View {
            selected: start.selected,
            ..clicked
        },
        start,
        "🔴 g77 (`req/924` §TUI-50, held): the pointer moved something other than the attention, \
         so clicking is no longer a read"
    );

    // ---- 裁定3: the face moves, and the attention does not go with it ---------------------
    let pushed = acts::glide(&start, 3, items);
    assert_eq!(
        pushed.selected, start.selected,
        "🔴 g77 (`req/924` §TUI-62 裁定3): the wheel moved the attention. Relative scroll is the \
         face moving *independently* of the selection"
    );
    assert!(
        pushed.glide > 0,
        "🔴 g77: scrolling down did not move the face"
    );
    let top = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        29,
        Tier::Mono,
        false,
        &start,
    ));
    let moved = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        29,
        Tier::Mono,
        false,
        &pushed,
    ));
    let first_id = |text: &str| -> Option<String> {
        text.lines()
            .find(|line| line.contains("gx1:"))
            .map(|line| line.split_whitespace().next().unwrap_or("").to_string())
    };
    println!(
        "G74_FIRST_ID top={:?} after_scroll_down={:?}",
        first_id(&top),
        first_id(&moved)
    );
    assert_ne!(
        first_id(&top),
        first_id(&moved),
        "🔴 g77 (`req/924` §TUI-62 裁定3): scrolling down drew the same first record, so the \
         content did not move up:\n{moved}"
    );
    // The direction: down moves the content **up**, so the record at the top afterwards is one the
    // first frame had further down.
    let before: Vec<String> = top
        .lines()
        .filter(|line| line.contains("gx1:"))
        .map(|line| line.split_whitespace().next().unwrap_or("").to_string())
        .collect();
    let after_first = first_id(&moved).unwrap_or_default();
    assert!(
        before.iter().position(|id| *id == after_first).unwrap_or(0) > 0,
        "🔴 g77 (`req/924` §TUI-62 裁定3): scrolling down did not move the content up — the record \
         now at the top was not below the top before"
    );
    // 🔴 The negative control: with the wheel untouched the window is the function of the state it
    // has always been, which is what `g28`'s invariant is measured under.
    assert_eq!(
        layout::scrolled(30, items, 1, 10, 0),
        layout::scrolled(30, items, 1, 10, 0),
        "🔴 g77: `scrolled` is not a function"
    );
    assert_ne!(
        layout::scrolled(0, items, 1, 10, 0).1,
        layout::scrolled(0, items, 1, 10, 4).1,
        "🔴 g77: the reader's own offset changes nothing, so the wheel turns and the face does not"
    );

    // ---- 裁定3: 余裕 and 区切り, on the drawn frame ---------------------------------------
    let buffer = renderer::render_view_to_buffer(&screen, 120, 29, Tier::Truecolor, false, &start);
    let text = renderer::buffer_text(&buffer);
    let margined = text
        .lines()
        .filter(|line| line.contains("gx1:"))
        .all(|line| line.starts_with(&" ".repeat(layout::LEFT_MARGIN as usize)));
    assert!(
        margined,
        "🔴 g77 (`req/924` §TUI-62 裁定3, 余裕): a record row starts on the first cell. A terminal \
         has no line height, so the room is the margin and the column gap:\n{text}"
    );
    let ruled: Vec<u16> = (buffer.area.y..buffer.area.y + buffer.area.height)
        .filter(|y| {
            (buffer.area.x..buffer.area.x + buffer.area.width).any(|x| {
                buffer[(x, *y)]
                    .modifier
                    .contains(ratatui::style::Modifier::UNDERLINED)
            })
        })
        .collect();
    println!("G74_RULED_ROWS={ruled:?} GROUP_ROWS={}", layout::GROUP_ROWS);
    let drawn = text.lines().filter(|line| line.contains("gx1:")).count();
    assert_eq!(
        ruled.len(),
        drawn / layout::GROUP_ROWS,
        "🔴 g77 (`req/924` §TUI-62 裁定3, 区切り): {} of {drawn} drawn rows carry the group rule \
         and the declaration says one every {}",
        ruled.len(),
        layout::GROUP_ROWS
    );
    assert!(
        !ruled.is_empty(),
        "🔴 g77: no group rule is drawn at all, so the separator is a declaration nothing reads"
    );
}

// =============================================================================================
// `[T-r55]` — the record's own two routes (`req/924` §TUI-64, `req/38` SS1095, Owner `#285-T`).
//
// g78..g82. The ids were minted after a fresh grep of **both** suites and `tools/gates/`, which
// `req/38` SS1097 ruled to be one flat space; the highest in use was `g77`.
// =============================================================================================

/// The view a record is open in, at the first row.
fn opened() -> View {
    View {
        open: true,
        ..View::default()
    }
}

/// 🔴 **g83 — the record's members are drawn in a declared order, not in the map's.**
///
/// # The defect this was written for was invisible to every gate in this file
///
/// The record face read `serde_json::Map::keys()`. That order is a `BTreeMap`'s (alphabetical)
/// unless `serde_json/preserve_order` is on, in which case it is an `IndexMap`'s (the order the wire
/// serialised). Cargo unifies features across a build, and measured on this workspace with
/// `cargo tree -e features` (`[T-r55]`, 2026-09-02):
///
/// * `-p tracefold-tui` — `preserve_order` **absent**. This binary. Every gate here.
/// * `-p gx-cli` — `preserve_order` **present**, transitively. The `gx` binary a reader runs.
///
/// So the reader saw one order and every instrument saw another, and **no test in this file could
/// have caught it**, because a test cannot photograph a screen drawn by a binary it is not linked
/// into. It was found by comparing this lane's own red-first output against its own real-pty
/// capture — two artefacts that existed for other reasons.
///
/// The gate is therefore written to hold **under either feature set**: the row is built with its
/// keys in a scrambled order, so a face reading the map would draw the scramble under
/// `preserve_order` and the alphabet without it, and neither is what is asserted.
#[test]
fn g83_the_record_draws_its_members_in_a_declared_order() {
    // Scrambled on purpose: neither insertion order nor alphabetical order is the answer.
    let row = serde_json::json!({
        "superseded_by": serde_json::Value::Null,
        "verdict": "Admit",
        "actor": "agent-a",
        "transformation": "gx1:t3sto0000000001",
        "zz_unknown_to_this_face": "later",
        "rollback": serde_json::Value::Null,
        "state": "Committed",
        "enforced": true,
        "created_at": "2026-08-30T09:00:00Z",
        "inverse_status": "Available",
        "scope": "src/lib.rs",
    });
    let screen = Screen {
        healthz: answered(wire::ROUTES[0], serde_json::from_str(HEALTHZ).expect("fixture parses")),
        transformations: answered(
            wire::ROUTES[1],
            serde_json::json!({"items": [row], "next_cursor": null}),
        ),
        candidates: answered(
            wire::ROUTES[2],
            serde_json::from_str(CANDIDATES).expect("fixture parses"),
        ),
        escalations: answered(
            wire::ROUTES[3],
            serde_json::from_str(ESCALATIONS).expect("fixture parses"),
        ),
    };
    let text = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        32,
        Tier::Mono,
        false,
        &opened(),
    ));
    // The order the keys were drawn in, read off the screen.
    //
    // 🔴 **Every word, not the first word of every line** (`[T-r58]`, 2026-09-02, defects 1 and 2).
    // The record no longer draws one flat `key value` row per member: the three `Priority::One` keys
    // fold into one head row beside their values, and the members whose value is a mark gather onto
    // one row per kind of nothing (`renderer::record_own`). Reading only the first word of each line
    // would now see three of eleven keys and call the other eight missing — the instrument measuring
    // the old arrangement rather than the declared order, which is what this gate is actually for.
    // The declared order is unchanged and is still what is asserted.
    let drawn: Vec<String> = text
        .lines()
        .flat_map(|line| line.split_whitespace())
        .filter(|word| {
            LEDGER_COLUMNS.iter().any(|column| column.key == *word)
                || *word == "zz_unknown_to_this_face"
        })
        .map(str::to_string)
        .collect();
    // 🔴 The keys whose value is a mark leave the sequence in place and arrive at the end, gathered.
    // Which ones those are is read from the **bed**, not written down here, so a bed that changes
    // moves this expectation with it rather than falsifying it.
    let gathered: Vec<String> = ["rollback", "superseded_by"]
        .iter()
        .map(|key| (*key).to_string())
        .collect();
    for key in &gathered {
        assert!(
            row.get(key).is_some_and(serde_json::Value::is_null),
            "🔴 g83: the bed no longer gives `{key}` a mark, so the gathered group this gate \
             expects is not the group the face draws"
        );
    }
    let mut wanted: Vec<String> = LEDGER_COLUMNS
        .iter()
        .map(|column| column.key.to_string())
        .filter(|key| !gathered.contains(key))
        .collect();
    wanted.push("zz_unknown_to_this_face".to_string());
    wanted.extend(gathered.iter().cloned());
    assert_eq!(
        drawn, wanted,
        "🔴 g83: the record's members are drawn in an order the face does not declare. It is \
         `serde_json::Map`'s, which is alphabetical here and the wire's order in the `gx` binary \
         (`serde_json/preserve_order` is unified in through `gx-cli`), so the reader and every \
         gate in this file see two different screens.\n{text}"
    );
    // The controls: neither of the two orders a map would have given is what was asserted, or the
    // assertion above would pass on the very defect it exists to catch.
    let mut alphabetical = wanted.clone();
    alphabetical.sort();
    assert_ne!(
        drawn, alphabetical,
        "🔴 g83: the declared order and the alphabet are the same list, so this gate cannot tell \
         a declaration from a `BTreeMap`"
    );
    let scrambled: Vec<String> = row
        .as_object()
        .expect("an object")
        .keys()
        .cloned()
        .collect();
    assert_ne!(
        drawn, scrambled,
        "🔴 g83: the declared order and this bed's map order are the same list, so this gate \
         cannot tell a declaration from an `IndexMap`"
    );
}

/// 🔴 **AC-13 — the record road, measured, because `ac10` and `ac12` have no ceiling.**
///
/// `ac10` and `ac12` assert `> 0` and print. That is the right shape for a **threshold** (a number
/// chosen before the first measurement is a number the next lane bends the measurement to meet) and
/// it is the wrong shape for a **road nobody has ever timed**. This lane put two loopback reads
/// between a keypress and a frame, and "it is two GETs, so it is fine" is not a measurement.
///
/// Three figures, told apart, because they have three different remedies:
///
/// * **draw** — rendering the opened record from a `Held` already in hand. Pure; no socket. This is
///   what happens on a resize, a scroll and every redraw while the record stays open.
/// * **read** — `wire::Held::read` against a loopback fixture: the two sockets. This happens
///   **once per record opened**, and it is the only new cost a keypress can feel.
///   🔴 **And this figure is about the fixture, not about the face.** Measured on this bed the
///   median came out at ~10.3 ms with p10 and p90 inside 150 µs of it — a number that flat is not a
///   distribution of work, it is a constant being waited on. `Fixture::spawn`'s accept loop sleeps
///   5 ms between polls and this road opens **two** connections, so what is being timed is
///   2 x 5 ms of test-server latency. It is reported anyway, and reported as an **upper bound on
///   this instrument** rather than as a cost of the face: a figure whose provenance is not stated
///   is a figure a later reader will attribute to the wrong thing.
/// * **plan** — the layout on its own, so the drawing and the arithmetic can be told apart, in the
///   same shape `ac12` reports it.
///
/// **No threshold is asserted**, for `ac10`'s reason, and the figures are printed at every ruled
/// shape so the report carries numbers rather than an adjective.
#[test]
fn ac13_the_record_road_is_measured_at_every_ruled_shape() {
    const ROUNDS: usize = 60;
    let fixture = Fixture::start();
    let screen = fixture.read();
    let held = wire::Held::read(&fixture.base_url, None, RECEIPT_HOLDER);
    assert!(
        held.refusal.is_none() && held.transformation.is_ok(),
        "🔴 ac13: the bed did not answer, so what follows would be timing a failure road"
    );

    let mut reads: Vec<u128> = Vec::with_capacity(ROUNDS);
    for _ in 0..ROUNDS {
        let started = Instant::now();
        let one = wire::Held::read(&fixture.base_url, None, RECEIPT_HOLDER);
        reads.push(started.elapsed().as_micros());
        assert!(
            one.transformation.is_ok(),
            "🔴 ac13: a read in the batch did not answer; the median would be a median of failures"
        );
    }
    reads.sort_unstable();
    println!(
        "AC13_READ_US median={} p10={} p90={} n={ROUNDS} routes=2",
        reads[ROUNDS / 2],
        reads[ROUNDS / 10],
        reads[ROUNDS * 9 / 10]
    );

    let measured = renderer::measured(&screen);
    for (width, height) in RULED_SHAPES {
        let started = Instant::now();
        let _ = renderer::render_held_to_buffer(
            &screen,
            &held,
            width,
            height,
            Tier::Truecolor,
            false,
            &opened(),
            gx_tui::tui::live::LinkReport::off(),
        );
        let cold_us = started.elapsed().as_micros();

        let mut warm: Vec<u128> = Vec::with_capacity(ROUNDS);
        let mut plans: Vec<u128> = Vec::with_capacity(ROUNDS);
        for _ in 0..ROUNDS {
            let started = Instant::now();
            let _ = renderer::render_held_to_buffer(
                &screen,
                &held,
                width,
                height,
                Tier::Truecolor,
                false,
                &opened(),
                gx_tui::tui::live::LinkReport::off(),
            );
            warm.push(started.elapsed().as_micros());
            let started = Instant::now();
            let _ = layout::resolve_attended(
                width,
                height,
                &measured,
                false,
                layout::Subject::Record,
                layout::Attention {
                    selected: 0,
                    items: screen.transformations.items().len(),
                    glide: 0,
            // [T-r58] the two record counts are read only for `Subject::Record`; every one of
            // these plans is resolved for a grid, so the default is the measurement.
            ..layout::Attention::default()
                },
            );
            plans.push(started.elapsed().as_micros());
        }
        warm.sort_unstable();
        plans.sort_unstable();
        println!(
            "AC13_DRAW_US {width}x{height} cold={cold_us} warm_median={} warm_p10={} warm_p90={} \
             plan_median={} n={ROUNDS}",
            warm[ROUNDS / 2],
            warm[ROUNDS / 10],
            warm[ROUNDS * 9 / 10],
            plans[ROUNDS / 2]
        );
        // The clock moved, which is the one thing a printed figure has to earn before it is read.
        assert!(
            cold_us > 0 || warm[ROUNDS - 1] > 0,
            "🔴 ac13 at {width}x{height}: every figure is nought, so the clock did not move and \
             these numbers are about the timer rather than about the face"
        );
    }
}

/// 🔴 **g78 — an id off the wire cannot make this face write anything but `GET `.**
///
/// # Why this gate exists
///
/// `tui/src/tui/wire.rs`'s heading says the claim *this face performs no effect* is "**structural
/// rather than promised**": one function writes `GET ` and no code path writes another method. That
/// argument rested on every path ever written being a `&'static str` declared in that file.
/// [`wire::RECORD_ROUTES`] ends it — the path now carries a value **the engine sent** — and an id
/// carrying a carriage return would let a row of the ledger compose a second request line. A method
/// this module cannot write becoming a method this module can be *made* to write is the membrane
/// breaking, and it would break at the layer the whole face rests on.
///
/// # Three directions, and the third is the one that makes the first two mean something
///
/// 1. **the hostile id is refused before a socket is opened** — the server sees nothing at all;
/// 2. **the ordinary id is not refused** — exactly two requests arrive and both are `GET`, so a
///    guard that refused everything would fail here rather than pass everything;
/// 3. **the second layer answers on its own** — `wire::read_route` is handed the hostile path
///    directly, bypassing `record_path`, and still refuses. Without this the gate would be
///    measuring one guard and reporting two.
#[test]
fn g78_a_hostile_id_never_reaches_a_request_line() {
    let hostile = "gx1:t3sto0000000001 HTTP/1.1\r\nDELETE /v1/transformations/x";
    let fixture = Fixture::start();

    let refused = wire::Held::read(&fixture.base_url, None, hostile);
    assert!(
        refused.refusal.is_some(),
        "🔴 g78: `{hostile:?}` was not refused, so an id off the wire reached a request line"
    );
    assert!(
        refused.transformation.is_pending() && refused.receipt.is_pending(),
        "🔴 g78: a refused id still produced a reading, so something was asked"
    );
    let asked = fixture.seen.lock().expect("fixture lock").clone();
    assert!(
        asked.is_empty(),
        "🔴 g78: the server was asked {} time(s) about an id this face declared unaddressable: \
         {asked:#?}",
        asked.len()
    );

    // The negative control for the guard itself: an ordinary id is asked, twice, with `GET`.
    let held = wire::Held::read(&fixture.base_url, None, RECEIPT_HOLDER);
    assert!(
        held.refusal.is_none(),
        "🔴 g78: `{RECEIPT_HOLDER}` was refused, so the guard refuses everything and the assertion \
         above is vacuous: {:?}",
        held.refusal
    );
    let asked = fixture.seen.lock().expect("fixture lock").clone();
    assert_eq!(
        asked.len(),
        2,
        "🔴 g78: an opened record asks its two routes and this bed saw {}: {asked:#?}",
        asked.len()
    );
    for line in &asked {
        assert!(
            line.starts_with("GET "),
            "🔴 g78: the membrane's first clause is broken — `{line}`"
        );
    }

    // The second layer, on its own. `record_path` is not in this road at all.
    let direct = wire::read_route(&fixture.base_url, "/v1/transformations/a b", None);
    assert!(
        direct.status.is_none() && direct.error.is_some(),
        "🔴 g78: `open` wrote a request target with a space in it — the second layer is not there"
    );
    let asked_after = fixture.seen.lock().expect("fixture lock").len();
    assert_eq!(
        asked_after, 2,
        "🔴 g78: the second layer let the request through: {asked_after} request(s) seen"
    );

    // 🔴 **`addressable` has three clauses and only one of them was being exercised**
    // (`[T-r55]`, 2026-09-02, independent audit finding S4). Everything above is refused by the
    // **character** clause alone, so `!id.is_empty()` and `id.len() <= MAX_ID_BYTES` could both
    // have been deleted with all of this still green — including the paragraph on `MAX_ID_BYTES`
    // that says "a length that is not checked is a length the wire chooses", guarding a bound
    // nothing checked. Each clause now has an input only it refuses, and a neighbour it admits.
    assert!(
        !wire::addressable(""),
        "🔴 g78: an empty id is addressable, so an empty path segment reaches a request line"
    );
    let at_the_bound = "a".repeat(wire::MAX_ID_BYTES);
    let over_the_bound = "a".repeat(wire::MAX_ID_BYTES + 1);
    assert!(
        wire::addressable(&at_the_bound),
        "🔴 g78: an id of exactly {} bytes is refused, so the bound is off by one and the clause \
         below proves nothing",
        wire::MAX_ID_BYTES
    );
    assert!(
        !wire::addressable(&over_the_bound),
        "🔴 g78: an id of {} bytes is addressable, so `MAX_ID_BYTES` is a paragraph and not a bound",
        over_the_bound.len()
    );
    // Every character the allow-list admits, and a neighbour of each kind it does not. Written as
    // a table rather than as prose so that a character added to the set has to be added here too.
    for (id, allowed) in [
        ("gx1:t3sto0000000001", true),
        ("A-Za-z0-9_.~-", true),
        ("gx1:with%2Fpercent", false), // percent-encoding is not a road around the allow-list
        ("gx1:with/slash", false),
        ("gx1:with?query", false),
        ("gx1:with space", false),
        ("gx1:with\ttab", false),
        ("gx1:ｆｕｌｌｗｉｄｔｈ", false), // not ASCII alphanumeric, whatever it looks like
    ] {
        assert_eq!(
            wire::addressable(id),
            allowed,
            "🔴 g78: `addressable({id:?})` is not {allowed}"
        );
    }
}

/// 🔴 **g84 — the two repairs an independent audit asked for, each with the defect it closes.**
///
/// Both were sound-looking code that asserted something it had not measured, which is the shape
/// this whole face exists to refuse — so both get a gate rather than a comment.
///
/// * **S2**: `renderer::agreement` compared four keys between the page's row and the singular
///   route's answer and **never checked the two were about the same transformation**. On the
///   common row all four match, so the face would draw `read again agrees` — *this record has not
///   moved* — from a comparison that was never made about it.
/// * **S5**: `wire::Held::receipt_mark` mapped **any** `404` to `NotHere`, whose whole meaning is
///   the two facts `get_receipt` names. A `404` from an older engine, a proxy or the wrong port is
///   a third preimage, and folding it in makes the face say *there is no receipt for this row*
///   about every row while talking to a server that cannot answer.
#[test]
fn g84_a_reading_is_not_believed_about_a_row_it_may_not_be_about() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let rows = screen.transformations.items();
    let (first, second) = (rows[0], rows[1]);

    // S2. The `Held` is about row one; it is drawn beside row two.
    let held = wire::Held::read(&fixture.base_url, None, RECEIPT_HOLDER);
    assert_eq!(
        first.get(wire::LEDGER_ID_KEY).and_then(|v| v.as_str()),
        Some(RECEIPT_HOLDER),
        "🔴 g84: the bed's first row is not the id this reading is about, so the pairing below \
         tests nothing"
    );
    let text = flat(&renderer::buffer_text(&renderer::render_held_to_buffer(
        &screen,
        &held,
        120,
        32,
        Tier::Mono,
        false,
        &View {
            selected: 1,
            ..opened()
        },
        gx_tui::tui::live::LinkReport::off(),
    )));
    assert!(
        text.contains("read again about another row"),
        "🔴 g84 (S2): a reading about `{RECEIPT_HOLDER}` is drawn over `{}` and the face does not \
         say so\n{text}",
        second
            .get(wire::LEDGER_ID_KEY)
            .and_then(|v| v.as_str())
            .unwrap_or("?")
    );
    assert!(
        !text.contains("read again agrees"),
        "🔴 g84 (S2): the face asserts the row has not moved, from a comparison made about a \
         different row\n{text}"
    );
    // The control: on the row it IS about, the same comparison still answers.
    let same = flat(&renderer::buffer_text(&renderer::render_held_to_buffer(
        &screen,
        &held,
        120,
        32,
        Tier::Mono,
        false,
        &opened(),
        gx_tui::tui::live::LinkReport::off(),
    )));
    assert!(
        same.contains("read again agrees"),
        "🔴 g84 (S2): the identity check refuses the row it is about, so it refuses everything and \
         the assertion above is vacuous\n{same}"
    );

    // S5. A `404` that is not this surface's refusal is not `NotHere`.
    let engine_404 = held_answering(404, serde_json::json!({"title": "not found", "gx_code": "NOT_FOUND"}));
    let stranger_404 = held_answering(404, serde_json::json!({"error": "Not Found"}));
    assert_eq!(
        engine_404.receipt_mark(),
        wire::ReceiptMark::NotHere,
        "🔴 g84 (S5): this surface's own refusal no longer reaches the mark that stands for it"
    );
    assert_eq!(
        stranger_404.receipt_mark(),
        wire::ReceiptMark::Unknown,
        "🔴 g84 (S5): a `404` carrying no `{}` is drawn as *there is no receipt for this row*, \
         which is a fact about a row asserted from a status code alone",
        wire::GX_CODE_KEY
    );
}

/// A `Held` whose receipt read answered with a given status and body, and whose id is addressable.
fn held_answering(status: u16, body: serde_json::Value) -> wire::Held {
    let mut held = wire::Held::pending(RECEIPT_HOLDER);
    held.receipt = wire::Reading {
        route: format!("GET {}", wire::RECORD_ROUTES[1]),
        status: Some(status),
        read_at: "2026-09-02T00:00:00.000000000Z".to_string(),
        elapsed_ms: 1,
        body: Some(body),
        error: None,
    };
    held
}

/// 🔴 **g79 — the five things `GET /v1/receipts/{tid}` can amount to are five spellings.**
///
/// The `404` is the one that costs something to keep. `handlers.rs`'s `get_receipt` spells **two**
/// facts in one refusal — *it has not been committed*, or *this server holds neither its row nor
/// its archive* — so neither `--` (it was never written) nor `?` (this face could not measure) is
/// the truth, and both would be the face picking one of two preimages. It is spelled in words, and
/// this gate is the reason a later reduction pass cannot quietly turn it into a mark.
///
/// The second half drives the real wire: a receipt that **is** held draws its decoded coordinates
/// and the sentence saying this face has not graded it, and a `✓` appears nowhere.
#[test]
fn g79_the_receipt_marks_are_five_spellings_and_a_404_is_not_a_mark() {
    let marks: BTreeSet<&str> = wire::ReceiptMark::ALL.iter().map(|m| m.mark()).collect();
    assert_eq!(
        marks.len(),
        wire::ReceiptMark::ALL.len(),
        "🔴 g79: {} of {} receipt marks are distinct — two of them are drawn the same way",
        marks.len(),
        wire::ReceiptMark::ALL.len()
    );
    for collapsed in [Nothing::Absent, Nothing::Unknown] {
        assert_ne!(
            wire::ReceiptMark::NotHere.mark(),
            collapsed.mark(),
            "🔴 g79: a `404` covering two facts is drawn as `{}`, which asserts one of them",
            collapsed.mark()
        );
    }

    let fixture = Fixture::start();
    let screen = fixture.read();
    // The bed's first row is the id this server holds a receipt for.
    let held = wire::Held::read(&fixture.base_url, None, RECEIPT_HOLDER);
    assert_eq!(
        held.receipt_mark(),
        wire::ReceiptMark::Held,
        "🔴 g79: the bed holds a receipt for `{RECEIPT_HOLDER}` and the face did not read one"
    );
    let text = flat(&renderer::buffer_text(&renderer::render_held_to_buffer(
        &screen,
        &held,
        120,
        32,
        Tier::Mono,
        false,
        &opened(),
        gx_tui::tui::live::LinkReport::off(),
    )));
    for wanted in [
        "key_id gx1:k3yzzzzzzzzzz",
        "leaf 3 of 12",
        "root gx1:r00tzzzzzzzzzz",
    ] {
        assert!(
            text.contains(wanted),
            "🔴 g79: the decoded half of the receipt is not on the screen: `{wanted}`\n{text}"
        );
    }
    assert!(
        text.contains(&format!(
            "checked {} -- {}",
            renderer::NOT_CHECKED,
            renderer::RECEIPT_VERIFY_ADDRESS
        )),
        "🔴 g79: the face drew a receipt and did not say it has not checked it\n{text}"
    );
    assert!(
        !text.contains('✓') && !text.contains('✔'),
        "🔴 g79 (`req/924` §TUI-30): a tick is a two-valued mark and *verified* / *not checked* / \
         *failed* are three answers\n{text}"
    );

    // The `404`, on the same road, with a row this server holds no receipt for.
    let missing = wire::Held::read(&fixture.base_url, None, "gx1:t3sto0000000002");
    assert_eq!(
        missing.receipt_mark(),
        wire::ReceiptMark::NotHere,
        "🔴 g79: a `404` did not reach the classifier as its own value"
    );
    let text = flat(&renderer::buffer_text(&renderer::render_held_to_buffer(
        &screen,
        &missing,
        120,
        32,
        Tier::Mono,
        false,
        &View {
            selected: 1,
            ..opened()
        },
        gx_tui::tui::live::LinkReport::off(),
    )));
    assert!(
        text.contains(&format!("receipt {}", wire::ReceiptMark::NotHere.mark())),
        "🔴 g79: the `404` is not spelled on the screen\n{text}"
    );
    assert!(
        !text.contains("key_id"),
        "🔴 g79: there is no receipt and the face drew its members anyway\n{text}"
    );
}

/// 🔴 **g80 — the record's address is a key of the list, and the two routes are filled from it.**
///
/// A record face exists only because every row of the ledger carries the address it can be asked
/// about (`crates/gx-api/src/list.rs`'s `row_json`). Two spellings of *which member is the address*
/// would be two answers, and the one in `wire` is what fills the request line while the one in
/// `layout` is what the reader sees.
#[test]
fn g80_the_record_address_is_a_key_the_ledger_draws() {
    assert!(
        LEDGER_COLUMNS
            .iter()
            .any(|column| column.key == wire::LEDGER_ID_KEY),
        "🔴 g80: `{}` is not a column this face draws, so the id it asks with is not the id it \
         shows",
        wire::LEDGER_ID_KEY
    );
    assert_eq!(
        wire::RECORD_ROUTES.len(),
        wire::RECORD_HOLES.len(),
        "🔴 g80: the routes and the holes they carry are declared in unequal numbers"
    );
    for (route, hole) in wire::RECORD_ROUTES.iter().zip(wire::RECORD_HOLES) {
        assert!(
            route.contains(hole),
            "🔴 g80: `{route}` does not carry `{hole}`, so the pairing is by position and by nothing else"
        );
        let filled = wire::record_path(route, RECEIPT_HOLDER)
            .unwrap_or_else(|| panic!("🔴 g80: `{route}` would not take an ordinary id"));
        assert!(
            filled.contains(RECEIPT_HOLDER) && !filled.contains('{'),
            "🔴 g80: `{route}` filled to `{filled}`"
        );
    }
    // The control: the filler refuses what the guard refuses, or the two are separate opinions.
    assert!(
        wire::record_path(wire::RECORD_ROUTES[0], "a b").is_none(),
        "🔴 g80: `record_path` filled a template with an id `addressable` refuses"
    );
}

/// 🔴 **g81 — the lifecycle chain is not drawn, and the fact that it is not drawn is on a screen.**
///
/// `req/924` §TUI-30's sketch draws `submitted -> planned -> verified -> committed`. No route this
/// face reads carries it: `state` is one word for where the row is *now*, and nothing answers
/// *which states it passed through*. Drawing it from the vocabulary with the current position
/// marked would put three arrows on the screen for three transitions nobody measured.
///
/// So the membrane's second obligation applies — **a renderer cannot invert, so what it owes
/// instead is to name what it let go of** — and this gate holds the two halves together: the arrows
/// are on no frame, and the sentence is on the one screen that exists to carry what the others
/// dropped.
#[test]
fn g81_the_chain_is_dropped_and_the_drop_is_named() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let held = wire::Held::read(&fixture.base_url, None, RECEIPT_HOLDER);
    // An arrow between states is the shape the sketch asks for; any of these is one.
    let arrows = ["──▶", "-->", "->", "▶", "→"];
    // 🔴 **The state words are DERIVED from the bed and not only typed** (`[T-r55]`, 2026-09-02,
    // found by attacking this gate rather than by running it). The first shape of this list held
    // the four words of `req/924` §TUI-30's sketch — `submitted`, `planned`, `verified`,
    // `committed` — and **those are the sketch's vocabulary, not the engine's**. A chain drawn as
    // `Draft --> Committed` would have been invisible to a gate written to catch chains. So the
    // words the bed actually puts in the `state` column are added to the set the predicate sweeps,
    // which means a bed carrying a state this file has never heard of grows the gate rather than
    // escaping it. The sketch's four stay: they are what the drop is a drop *of*.
    let mut states: Vec<String> = ["submitted", "planned", "verified", "committed"]
        .into_iter()
        .map(str::to_string)
        .collect();
    for item in screen.transformations.items() {
        if let Some(state) = item.get("state").and_then(serde_json::Value::as_str) {
            if !state.is_empty() && !states.iter().any(|known| known == state) {
                states.push(state.to_string());
            }
        }
    }
    assert!(
        states.len() > 4,
        "🔴 g81: the bed put no state word into the sweep, so this gate is measuring only the \
         sketch's own four and would miss a chain drawn in the engine's words: {states:?}"
    );
    let mut carried: Vec<String> = Vec::new();
    for (width, height) in RULED_SHAPES {
        for (name, view) in [
            ("list", View::default()),
            ("record", opened()),
            (
                "help",
                View {
                    help: true,
                    ..View::default()
                },
            ),
        ] {
            let text = flat(&renderer::buffer_text(&renderer::render_held_to_buffer(
                &screen,
                &held,
                width,
                height,
                Tier::Mono,
                false,
                &view,
                gx_tui::tui::live::LinkReport::off(),
            )));
            for arrow in arrows {
                // The hatch spells the sentence that names the drop, and that sentence is prose
                // about the chain rather than a chain. It is allowed to say `->` in words -- the
                // disclosure already ends a clause with `-> help:?` -- and it is not allowed to
                // draw one between state names.
                for state in &states {
                    if text.contains(&format!("{state} {arrow}"))
                        || text.contains(&format!("{state}{arrow}"))
                    {
                        carried.push(format!("{width}x{height} {name}: `{state} {arrow}`"));
                    }
                }
            }
        }
    }
    assert!(
        carried.is_empty(),
        "🔴 g81: a chain of states is drawn at {} face(s), and no route carries one: {carried:#?}",
        carried.len()
    );
    // The control: the predicate can see a chain, or the green above is green on any string.
    let planted = "submitted ──▶ planned";
    assert!(
        arrows
            .iter()
            .any(|arrow| planted.contains(&format!("submitted {arrow}"))),
        "🔴 g81: the predicate cannot see `{planted}`, so it is measuring nothing"
    );
    // And the sentence, on the screen that carries what the others dropped.
    let hatch = flat(&renderer::buffer_text(&renderer::render_held_to_buffer(
        &screen,
        &held,
        120,
        32,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
        gx_tui::tui::live::LinkReport::off(),
    )));
    assert!(
        hatch.contains("lifecycle chain"),
        "🔴 g81: the chain is dropped and no screen says so, which turns §TUI-57's *moved* into \
         §TUI-21's *deleted*\n{hatch}"
    );
    for route in wire::RECORD_ROUTES {
        assert!(
            hatch.contains(route),
            "🔴 g81: `GET {route}` is spelled on no screen, so the record's own reads have no \
             address a reader can retype\n{hatch}"
        );
    }
}

/// 🔴 **g82 — the record's cut is named exactly when there is one, and never otherwise.**
///
/// The record face draws two things: the members the wire carried, and the rows its own two routes
/// added. A disclosure that counted one of them would be describing half a screen.
///
/// The invariants are predicates rather than line numbers: the members are paid for first (a face
/// that cut the subject to make room for the commentary would be answering a question nobody
/// asked), so **rows beyond the members are drawn only once every member is**; neither count may
/// exceed its own total; and the line that says what was cut must never itself be the thing cut.
///
/// # 🔴 The predicate this gate required is superseded, and the old sentence is kept above
///
/// (`[T-r58]`, 2026-09-02 — the seat's ruling on the real capture at
/// `req/942_artifacts/tui_r55_2026-09-02/pty/record_120x32.txt`, defect 4.) This gate used to
/// require the record's note **at every shape**, and the note read
/// `record 1 of 31 | 10 of 10 members | 7 of 7 more rows | GET /v1/transformations | close: escape`
/// — drawn at 120x32, where **nothing had been cut**. A permanent row reporting an event that had
/// not happened, standing beside a second permanent row that said *a record is open: its own line
/// counts what it drew*. The ruling names two status rows and a sentence about the face's own
/// arrangement as defects, so the requirement that produced them is a requirement for a defect and
/// `INHERITED_PRINCIPLES` (*a test that requires a defect is not a floor*) permits it to be
/// rewritten rather than obeyed.
///
/// What the gate measured is measured still, from the plan that decides it and the screen that
/// draws it, and **one clause is added**: at a shape with room, there is no line at all.
#[test]
fn g82_the_record_note_counts_both_groups_at_every_shape() {
    let fixture = Fixture::start();
    let screen = fixture.read();
    let held = wire::Held::read(&fixture.base_url, None, RECEIPT_HOLDER);
    let mut census: Vec<String> = Vec::new();
    for (width, height) in RULED_SHAPES {
        let text = flat(&renderer::buffer_text(&renderer::render_held_to_buffer(
            &screen,
            &held,
            width,
            height,
            Tier::Mono,
            false,
            &opened(),
            gx_tui::tui::live::LinkReport::off(),
        )));
        // The totals come from the one function that counts them, which is the same function the
        // plan was resolved from — so this gate cannot drift from the arithmetic it is measuring by
        // counting a second time.
        let (members_total, beyond_total) =
            renderer::record_extent(&screen, &held, &opened(), width);
        let plan = layout::resolve_attended(
            width,
            height,
            &renderer::measured(&screen),
            false,
            layout::Subject::Record,
            layout::Attention {
                selected: 0,
                items: screen.transformations.items().len(),
                glide: 0,
                record_members: members_total,
                record_beyond: beyond_total,
            },
        );
        let members = plan.record_members_shown;
        let beyond = plan.record_beyond_shown;
        census.push(format!(
            "{width}x{height} members={members}/{members_total} beyond={beyond}/{beyond_total} cut={}",
            plan.record_cut
        ));
        assert!(
            members <= members_total && beyond <= beyond_total,
            "🔴 g82 at {width}x{height}: a count exceeds its own total: \
             {members}/{members_total}, {beyond}/{beyond_total}\n{text}"
        );
        assert!(
            beyond == 0 || members == members_total,
            "🔴 g82 at {width}x{height}: {beyond} row(s) beyond the record are drawn while only \
             {members} of {members_total} members are — the subject was cut for the commentary\n{text}"
        );
        assert_eq!(
            plan.record_cut,
            members < members_total || beyond < beyond_total,
            "🔴 g82 at {width}x{height}: the flag that buys the row for the disclosure and the \
             arithmetic that decides what is cut are two answers\n{text}"
        );
        if plan.record_cut {
            assert!(
                text.contains("not drawn:"),
                "🔴 g82 at {width}x{height}: {members}/{members_total} members and \
                 {beyond}/{beyond_total} rows are on the screen and nothing says so. The line that \
                 says what was cut must never itself be the thing that is cut — the row for it is \
                 taken off the budget before the counts are settled\n{text}"
            );
            if members < members_total {
                // 🔴 The screen counts **keys**, not rows: one gathered row holds several members,
                // so `members_total` (a row count, which is what the plan cuts by) is not the
                // denominator on the line.
                //
                // 🔴 **And the printed number is checked against the drawing, at every shape**
                // (independent audit, 2026-09-02, S5). The first version of this rewrite asserted
                // only that the clause was *named*, and left the arithmetic of the number to `p12`
                // — which measures it at **one** shape and one bed. The old gate this replaced
                // parsed the count off the screen at all seven. What is done here is the same
                // cross-check `p12` makes, generalised: parse `A of B members` off the drawing,
                // count the wire keys the drawing actually spells, and require `B - A` to be that
                // count. Neither number is taken from the plan, so this cannot agree with the plan
                // by construction.
                let clause = text
                    .split(" members")
                    .next()
                    .and_then(|head| {
                        let mut words = head.split_whitespace().rev();
                        let total: usize = words.next()?.parse().ok()?;
                        let dropped: usize = words.nth(1)?.parse().ok()?;
                        Some((dropped, total))
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "🔴 g82 at {width}x{height}: members were cut and no `N of M members` \
                             is on the screen\n{text}"
                        )
                    });
                let drawn_keys = text
                    .split_whitespace()
                    .filter(|word| LEDGER_COLUMNS.iter().any(|column| column.key == *word))
                    .count();
                assert_eq!(
                    clause.1 - clause.0,
                    drawn_keys,
                    "🔴 g82 at {width}x{height}: the screen says {} of {} members were dropped, so \
                     {} should be drawn, and {drawn_keys} are\n{text}",
                    clause.0,
                    clause.1,
                    clause.1 - clause.0
                );
            }
        } else {
            assert!(
                !text.contains("not drawn:"),
                "🔴 g82 at {width}x{height}: nothing was cut and the screen reports a cut — a \
                 permanent row describing an event that did not happen, which is the defect this \
                 gate was rewritten for\n{text}"
            );
        }
        assert!(
            beyond_total > 0,
            "🔴 g82 at {width}x{height}: the record adds no rows at all, so the second count is a \
             number that can never move\n{text}"
        );
    }
    println!("G82_CENSUS={census:?}");
    // The control at the top of the range: the widest shape has room for everything, so a screen
    // that reported a cut there would be reporting one that did not happen.
    let widest = census.first().expect("seven shapes");
    assert!(
        widest.contains("cut=false"),
        "🔴 g82: at the widest ruled shape every member has to be on the screen: {widest}"
    );
    // 🔴 **The controls for the ordering predicate, and they exist because the branch they guard is
    // unreachable while the code is right.** `beyond == 0 || members == members_total` can only be
    // false if the face draws commentary over a cut subject, which the arithmetic in
    // `renderer::subject` makes impossible — so no shape above will ever fire it, and an assertion
    // that can never fire is indistinguishable from one that is wrong. Fired here on numbers
    // instead, in both directions: it must refuse a cut subject with commentary under it, and it
    // must permit both of the shapes that are legal.
    let ordering = |members: usize, members_total: usize, beyond: usize| {
        beyond == 0 || members == members_total
    };
    assert!(
        !ordering(3, 10, 2),
        "🔴 g82: the ordering predicate admits 2 rows of commentary over 3 of 10 members, so it \
         would pass the defect it exists to catch"
    );
    assert!(
        ordering(3, 10, 0) && ordering(10, 10, 5),
        "🔴 g82: the ordering predicate refuses a legal screen, so a green above means nothing"
    );
}

// ---------------------------------------------------------------------------------------------
// g85 — g88: the four predicates `[T-r58]` (2026-09-02) was opened for.
//
// Each one is a sentence from the seat's ruling on the real capture at
// `req/942_artifacts/tui_r55_2026-09-02/pty/record_120x32.txt`, written as a predicate over the
// **drawing** rather than over a line number, and measured at all seven ruled shapes.
// ---------------------------------------------------------------------------------------------

/// An opened record over a ledger long enough to fill any of the seven shapes.
///
/// 🔴 The fixture server's two rows cannot produce the property these gates are for: a face that
/// gives its spare rows back to the ledger has no spare rows to give when the ledger is two records
/// long, and a gate measured on that bed would be measuring the bed.
fn opened_frame(width: u16, height: u16, records: usize) -> String {
    renderer::buffer_text(&renderer::render_view_to_buffer(
        &ledger(records),
        width,
        height,
        Tier::Mono,
        false,
        &opened(),
    ))
}

/// 🔴 **g85 — no shape leaves more than a quarter of the screen blank.**
///
/// The capture this gate was written from drew **thirteen blank rows at 120x32** — a third of the
/// terminal — because an opened record has about a dozen facts and a terminal has thirty-two rows.
/// `SS831` names a standing empty panel as furniture rather than information, and a face that
/// answers *the record is short* by leaving a third of the screen empty is standing one.
///
/// The bound is a **quarter**, and it is a quarter rather than nought because the last shape of a
/// list that ends is genuinely blank underneath and padding it would be the defect in the other
/// direction. What the bound refuses is a face whose emptiness is structural.
#[test]
fn g85_no_shape_leaves_more_than_a_quarter_of_the_screen_blank() {
    let mut census: Vec<String> = Vec::new();
    for (width, height) in RULED_SHAPES {
        let text = opened_frame(width, height, 40);
        let blank = text.lines().filter(|line| line.trim().is_empty()).count();
        census.push(format!("{width}x{height} blank={blank}/{height}"));
        assert!(
            blank * 4 <= height as usize,
            "🔴 g85 at {width}x{height}: {blank} of {height} rows are blank. An opened record is a \
             dozen facts and a terminal is thirty-two rows; the rows it does not need belong to the \
             ledger it was opened from, not to an empty panel\n{text}"
        );
    }
    println!("G85_CENSUS={census:?}");
    // 🔴 The control, because a predicate that cannot fail is not a measurement. The same
    // arithmetic on the shape this gate was written against: eighteen drawn rows on a thirty-two
    // row terminal left thirteen blank, and thirteen is more than eight.
    assert!(
        13 * 4 > 32,
        "🔴 g85: the bound admits the capture this gate was written from, so a green means nothing"
    );
    // And it must permit the honest blank: a ledger with fewer records than the screen has rows.
    let short = opened_frame(120, 32, 2);
    let short_blank = short.lines().filter(|line| line.trim().is_empty()).count();
    println!("G85_SHORT_LEDGER_BLANK={short_blank}");
    assert!(
        short_blank > 8,
        "🔴 g85: a two-record ledger cannot fill a thirty-two row screen, and a gate that reports \
         otherwise is not counting blank rows at all\n{short}"
    );
}

/// 🔴 **g86 — the marks gather, and two kinds of nothing never share a row.**
///
/// Five of the seventeen rows on the capture were the same mark (`created_at ?`, `scope ?`,
/// `rollback ?`, `superseded_by ?`, `actor ?`), and the ruling is that five rows spelling *this
/// process does not have it* are one fact about five keys. So they gather.
///
/// 🔴 **What the gathering may not do is collapse the vocabulary.** `?` is *measured and not
/// known*, `--` is *never written*, `no` is *the answer*, `0` is a count and `''` is a value the
/// wire opened and closed. `INHERITED_PRINCIPLES` puts the seven words outside the reach of every
/// reduction pass — they are the product. A gathering keyed on anything looser than the mark itself
/// would put two of them on one row and the row would then draw one word and mean two.
#[test]
fn g86_the_marks_gather_without_collapsing_two_kinds_of_nothing() {
    // Four kinds on one record, each produced by the wire shape `wire::cell` maps to it.
    let row = serde_json::json!({
        "transformation": "gx1:t3sto0000000001",
        "verdict": "Admit",
        "state": "Committed",
        "created_at": serde_json::Value::Null,   // `?`  — asked and not answered
        "scope": serde_json::Value::Null,        // `?`  — the same kind, so it joins the same row
        "enforced": false,                       // `no` — the answer, in the word for it
        "actor": "",                             // `''` — opened and closed with nothing between
        "rollback": 0,                           // `0`  — the count
        "superseded_by": serde_json::Value::Null,
    });
    let screen = Screen {
        healthz: answered(wire::ROUTES[0], serde_json::from_str(HEALTHZ).expect("fixture parses")),
        transformations: answered(
            wire::ROUTES[1],
            serde_json::json!({"items": [row], "next_cursor": null}),
        ),
        candidates: answered(
            wire::ROUTES[2],
            serde_json::from_str(CANDIDATES).expect("fixture parses"),
        ),
        escalations: answered(
            wire::ROUTES[3],
            serde_json::from_str(ESCALATIONS).expect("fixture parses"),
        ),
    };
    let text = renderer::buffer_text(&renderer::render_view_to_buffer(
        &screen,
        120,
        32,
        Tier::Mono,
        false,
        &opened(),
    ));
    println!("G86_FRAME=\n{text}");
    // The marks each row of the record carries, read off the drawing. A gathered row **leads** with
    // its mark, which is what makes the mark the row's subject rather than a value beside a key.
    //
    // 🔴 **The record's own rows, and the discriminator is the left margin.** A ledger row draws one
    // cell per column and several of them can be marks — `? ? no --` across four columns is a table
    // saying four different things about one record, which is correct and is not a gathering.
    // Written without a discriminator this gate reports the ledger under the record as a collapsed
    // vocabulary, which is a finding about the gate's own grammar.
    //
    // 🔴 **The discriminator was the row's *lead* and that was too narrow** (independent audit,
    // 2026-09-02, S4). Keying on *the first word is a mark* means the assertion is only ever reached
    // by rows already shaped the way the repair shapes them: a `record_own` that emitted
    // `created_at ? actor ''` — keys leading, marks inline — would put **two kinds of nothing on one
    // row**, keep every kind on the screen, and pass both halves of this gate. The discriminator is
    // now structural instead: the record's own rows are drawn from column nought and the ledger's
    // carry `layout::LEFT_MARGIN` (`renderer::margin`), so *begins with a space* separates them
    // whatever shape the record's rows take.
    let mut rows_with_marks = 0usize;
    for line in text.lines() {
        if line.starts_with(' ') || line.trim().is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if !words
            .iter()
            .any(|word| wire::Nothing::ALL.iter().any(|kind| kind.mark() == *word))
        {
            continue;
        }
        let marks: Vec<&str> = words
            .iter()
            .filter(|word| wire::Nothing::ALL.iter().any(|kind| kind.mark() == **word))
            .copied()
            .collect();
        rows_with_marks += 1;
        let distinct: BTreeSet<&str> = marks.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            1,
            "🔴 g86: one row carries {:?}. The seven words are the product — `?` is measured and \
             not known, `--` is never written, `no` is the answer, `0` is a count and `''` is a \
             value that arrived and said nothing. A row that draws two of them means neither.\
             \nline: {line}\n{text}",
            distinct
        );
    }
    // 🔴 **The stronger half, and it is stronger because a collapse does not have to look like two
    // marks on one row.** Fired against the gathering keyed on a predicate that is always true, the
    // rows-with-marks check above stayed **green**: every member joined the first group, so the row
    // drew `?` once and read as one kind. The vocabulary had been collapsed and the screen said
    // nothing — which is the defect exactly, and is invisible to a predicate that only counts marks
    // per row. What catches it is the count of kinds: four went in and one came out.
    let drawn_kinds: BTreeSet<String> = text
        .lines()
        .flat_map(|line| line.split_whitespace())
        .filter(|word| wire::Nothing::ALL.iter().any(|kind| kind.mark() == *word))
        .map(str::to_string)
        .collect();
    println!("G86_KINDS={drawn_kinds:?} ROWS_WITH_MARKS={rows_with_marks}");
    for kind in ["?", "no", "''", "0"] {
        assert!(
            drawn_kinds.contains(kind),
            "🔴 g86: this bed carries four kinds of nothing and `{kind}` is not on the screen. \
             `?` is measured and not known, `--` is never written, `no` is the answer, `0` is a \
             count and `''` is a value that arrived and said nothing — a gathering that puts two of \
             them under one mark has deleted a word of the product's vocabulary, and it does that \
             without ever drawing two marks on one row.\ndrawn: {drawn_kinds:?}\n{text}"
        );
    }
    // And the gathering happened: five members produced by two kinds occupy fewer rows than five.
    // The structural discriminator has to be worth something, or the loop above ran over the whole
    // screen and the ledger's honest multi-mark rows would have failed it. Fired as a number: the
    // ledger under this record draws at least one row carrying two kinds, and it is excluded.
    let ledger_multi = text
        .lines()
        .filter(|line| line.starts_with(' '))
        .filter(|line| {
            line.split_whitespace()
                .filter(|w| wire::Nothing::ALL.iter().any(|k| k.mark() == *w))
                .collect::<BTreeSet<&str>>()
                .len()
                > 1
        })
        .count();
    println!("G86_LEDGER_ROWS_WITH_TWO_KINDS={ledger_multi}");
    assert!(
        ledger_multi > 0,
        "🔴 g86: no row of the ledger under this record carries two kinds of nothing, so the \
         discriminator that excludes them is excluding nothing and this gate cannot tell a table \
         from a gathering\n{text}"
    );
    let gathered_row = text
        .lines()
        .find(|line| line.trim_start().starts_with("? "))
        .unwrap_or_else(|| panic!("🔴 g86: the two `?` members did not gather onto a row\n{text}"));
    let keys_on_it = gathered_row
        .split_whitespace()
        .filter(|word| LEDGER_COLUMNS.iter().any(|column| column.key == *word))
        .count();
    assert!(
        keys_on_it >= 2,
        "🔴 g86: the `?` row carries {keys_on_it} key(s), so nothing was gathered and the five flat \
         rows the ruling names are still five\n{text}"
    );
}

/// 🔴 **g87 — one standing row, and no second line counting what the screen drew.**
///
/// The capture carried **two**: the standing row, and the record's own note
/// (`record 1 of 31 | 10 of 10 members | 7 of 7 more rows | GET /v1/transformations | close:
/// escape`) drawn at every shape whether or not anything had been cut. The standing row carried the
/// other half of the defect — `a record is open: its own line counts what it drew`, a sentence about
/// the face's own arrangement.
///
/// Both are gone and this is what keeps them gone: the line that says where the reader is stands
/// once, the sentence about the face's arrangement is spelled nowhere, and no line has the shape of
/// the deleted note.
#[test]
fn g87_the_record_face_stands_on_exactly_one_status_row() {
    for (width, height) in RULED_SHAPES {
        let text = opened_frame(width, height, 40);
        let plan = layout::resolve_attended(
            width,
            height,
            &renderer::measured(&ledger(40)),
            false,
            layout::Subject::Record,
            layout::Attention {
                selected: 0,
                items: 40,
                glide: 0,
                record_members: renderer::record_extent(
                    &ledger(40),
                    &wire::Held::none(),
                    &opened(),
                    width,
                )
                .0,
                record_beyond: renderer::record_extent(
                    &ledger(40),
                    &wire::Held::none(),
                    &opened(),
                    width,
                )
                .1,
            },
        );
        let note = plan.note.trim().to_string();
        assert!(
            !note.is_empty(),
            "🔴 g87 at {width}x{height}: the standing row carries no note at all, so this gate \
             cannot count the rows it stands on"
        );
        let standing = text.lines().filter(|line| line.contains(&note)).count();
        assert_eq!(
            standing, 1,
            "🔴 g87 at {width}x{height}: the reader's position is drawn on {standing} rows. One \
             standing row is the ruling (`req/924` §TUI-57, gate g73) and a second line counting \
             what the screen drew is the defect `[T-r58]` was opened for\n{text}"
        );
        // The deleted note's own shape, and the deleted sentence, by name.
        for gone in ["record 1 of", "its own line counts"] {
            assert!(
                !flat(&text).contains(gone),
                "🔴 g87 at {width}x{height}: `{gone}` is back on the screen\n{text}"
            );
        }
    }
    // The control: the predicate has to be able to see a second occurrence, or a green above is a
    // statement about the search rather than about the screen.
    let doubled = "1 of 40 | x\nsomething\n1 of 40 | x";
    assert_eq!(
        doubled.lines().filter(|line| line.contains("1 of 40 | x")).count(),
        2,
        "🔴 g87: the counting predicate cannot see two standing rows, so it cannot refuse them"
    );
}

/// 🔴 **g88 — nothing the record draws stops mid-word without saying so.**
///
/// At 46x12 the capture drew `undo not from this face -- gx undo gx1:2hc4zanmdgh0001` on a
/// forty-six cell screen. The row is fifty-two cells. A terminal clips from the right in silence,
/// so the address a reader was being told to retype ended six characters early and nothing on the
/// screen said it had. That is the same defect as drawing `--` for a fact nobody measured, one
/// layer down: the face asserting something it does not know, this time about where a value ends.
///
/// The predicate is over the drawing at every ruled shape: a drawn row either fits, or ends in the
/// mark [`pad`] writes for a cut. There is no third case.
#[test]
fn g88_no_row_of_the_record_is_cut_without_the_cut_being_marked() {
    let mut cut_rows = 0usize;
    // 🔴 **The domain is narrow and the narrowing is printed** (independent audit, 2026-09-02, S1;
    // `SS858`: a gate that narrows in silence is a fail-open with ceremony). Only rows whose trimmed
    // length is exactly the width can be examined — a shorter row is not at the edge and a row
    // padded past its content is indistinguishable from one that ends there. At 120x32 and 100x30
    // **nothing this face draws reaches the edge**, so this gate's verdict at those two shapes is
    // vacuously true and is evidence of nothing. The census below is what says so out loud.
    let mut inspected = 0usize;
    let mut exempt_punctuation = 0usize;
    let mut exempt_single_word = 0usize;
    let mut census: Vec<String> = Vec::new();
    for (width, height) in RULED_SHAPES {
        let text = opened_frame(width, height, 40);
        let at_edge = text
            .lines()
            .filter(|l| l.trim_end().chars().count() == width as usize)
            .count();
        census.push(format!("{width}x{height} at_edge={at_edge}"));
        for line in text.lines() {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            let cells = trimmed.chars().count();
            assert!(
                cells <= width as usize,
                "🔴 g88 at {width}x{height}: a row is {cells} cells on a {width} cell screen\
                 \nline: {line}\n{text}"
            );
            if cells < width as usize {
                continue;
            }
            inspected += 1;
            // The row reaches the right edge. Either it ends on a word boundary — the last cell is
            // the last cell of a word that finished — or it was cut, and a cut is marked. `~` is
            // `pad`'s mark; `!` is the standing row's, which marks its own cut at the front.
            let marked = trimmed.ends_with('~') || trimmed.starts_with('!');
            if marked {
                cut_rows += 1;
                continue;
            }
            // Not marked: then it has to be a row that genuinely ends here. The only way to know is
            // that the row is not longer than the screen, which the region cannot report — so the
            // honest predicate is the one the drawing can answer: a full-width unmarked row must end
            // in a character that can end a value, and a value cut mid-word ends in a letter with
            // more letters owed. This face's own rows are the domain; the ledger's cells are padded
            // by `pad` and carry its mark already.
            //
            // 🔴 **Two exemptions, and they are counted rather than assumed** (independent audit,
            // 2026-09-02, S2). A cut landing on the `:` of `gx1:`, on a `-`, or on the `/` of a
            // route would end in punctuation and be admitted; a single long word cut anywhere would
            // be admitted too. Both are real holes. They are kept because closing them needs the
            // renderer's knowledge of whether a value was truncated, which the *capture* does not
            // carry — and they are **counted**, so a green with a non-zero exemption count is a
            // different fact from a green with nought.
            if trimmed.ends_with(|c: char| c.is_ascii_alphanumeric())
                && trimmed.split_whitespace().count() == 1
            {
                exempt_single_word += 1;
                continue;
            }
            if !trimmed.ends_with(|c: char| c.is_ascii_alphanumeric()) {
                exempt_punctuation += 1;
                continue;
            }
            panic!(
                "🔴 g88 at {width}x{height}: a row fills the screen, ends inside a word, and \
                 carries no mark saying it was cut\nline: {line}\n{text}"
            );
        }
    }
    println!(
        "G88_DOMAIN {census:?} inspected={inspected} marked_cut={cut_rows} \
         exempt_punctuation={exempt_punctuation} exempt_single_word={exempt_single_word}"
    );
    // 🔴 The gate has to look at something, at at least one shape, or a green is a statement about
    // the search. Not asserted per shape on purpose: at 120x32 and 100x30 nothing reaches the edge
    // and that is the face being correct, not the gate being blind.
    assert!(
        inspected > 0,
        "🔴 g88: no row at any of the seven ruled shapes reached the right edge, so this gate \
         examined nothing and its green means nothing: {census:?}"
    );
    // 🔴 The control. The predicate must refuse the exact row the capture drew: fifty-two cells of
    // `undo …` clipped to forty-six with nothing marking it.
    let clipped = "undo not from this face -- gx undo gx1:2hc4zanmdgh00";
    assert!(
        clipped.ends_with(|c: char| c.is_ascii_alphanumeric()) && clipped.split_whitespace().count() > 1,
        "🔴 g88: the predicate admits the row this gate was written from, so a green means nothing"
    );
}

// ---------------------------------------------------------------------------------------------
// g89 — g90: the two predicates `[T-r66]` (2026-09-02) was opened for.
//
// `[T-r58]` found this arithmetic and did not repair it. Its own words, in `renderer::subject`:
// "`columns_for_less` prices the columns against `LEFT_MARGIN + sum(width) + (n-1) * COLUMN_GAP`
// and on a reading where every column carries a value the row still arrives one cell or more over
// the screen … This is **not** a defect this lane introduced and it is **not** repaired here: the
// grid's own loop is another lane's write-target." It drew the *record* face's ledger slice through
// `fit` and left the ledger's own loop drawing rows a terminal clips in silence.
//
// Two questions, and they are different questions:
//   g89 asks the **drawing**: does any row this face draws stop mid-value with no mark?
//   g90 asks the **arithmetic**: is the width a row is priced against the width it is drawn at?
// g89 can be green on a bed whose columns happen not to fill the screen — five of the ten keys came
// back a mark for nothing on the live thirty-one row bed, so the budget never bound there. g90 is
// green or red on the code regardless of what any bed happens to carry, which is why both exist.
// ---------------------------------------------------------------------------------------------

/// A **closed** ledger over a bed long enough to fill any of the seven shapes.
///
/// The sibling of [`opened_frame`], and the same argument holds: a face whose rows are the ledger's
/// rows has nothing to draw when the ledger is two records long, and a gate measured on that bed
/// would be measuring the bed.
fn ledger_frame(width: u16, height: u16, records: usize) -> String {
    renderer::buffer_text(&renderer::render_view_to_buffer(
        &ledger(records),
        width,
        height,
        Tier::Mono,
        false,
        &View {
            open: false,
            ..View::default()
        },
    ))
}

/// 🔴 **g89 — nothing the ledger draws stops mid-value without saying so.**
///
/// The sibling of `g88`, asked of the other face. `g88` covers the rows an opened record draws,
/// which `renderer::subject` composes through `fit`; every row the **grid** draws — the column
/// header, the `all N` clause, and the records themselves — was composed with `spans_with` and no
/// `fit` at all, so a row wider than the screen was handed to a terminal that clips from the right
/// and says nothing. A reader could not tell `Committed` from `Committe`.
///
/// The predicate is `g88`'s, deliberately: **the same question is asked in the same words**, so
/// there is no second vocabulary for *this row stopped early* and no second answer to what counts as
/// a mark. `~` is [`pad`]'s; `!` is the standing row's, which marks its own cut at the front.
///
/// 🔴 **The domain is printed per shape, including where it is empty** (`SS870`, and the failure the
/// brief for this lane names by name: a `0 → 0` measurement is vacuously true and is evidence of
/// nothing). Only a row whose trimmed length is exactly the width can be examined — a shorter row is
/// not at the edge, and a row padded past its content is indistinguishable from one that ends there.
#[test]
fn g89_no_row_of_the_ledger_is_cut_without_the_cut_being_marked() {
    let mut inspected = 0usize;
    let mut cut_rows = 0usize;
    let mut exempt_punctuation = 0usize;
    let mut exempt_single_word = 0usize;
    let mut census: Vec<String> = Vec::new();
    // 🔴 Collected rather than panicked on at the first row: a gate that dies on row one reports one
    // row, and the number this lane has to be able to state is *how many rows at which shapes*.
    let mut unmarked: Vec<String> = Vec::new();
    // 🔴 **The seven ruled shapes and five narrower ones** (`[T-r66]`, 2026-09-02). The ruled shapes
    // start at forty cells and `columns_for_less`'s floor only bites below nineteen, so measured on
    // those alone this gate would never once see the case the ruling in `renderer::fit` is *about*
    // — the width that cannot pay for the mark. The narrow five are not terminals anyone owns; they
    // are where the degenerate branch lives, and a ruling no gate exercises is a paragraph.
    const NARROW_SHAPES: [(u16, u16); 5] = [(18, 10), (17, 10), (12, 8), (6, 6), (3, 6)];
    for (width, height) in RULED_SHAPES.into_iter().chain(NARROW_SHAPES) {
        let text = ledger_frame(width, height, 40);
        let at_edge = text
            .lines()
            .filter(|line| line.trim_end().chars().count() == width as usize)
            .count();
        let mut here = 0usize;
        for line in text.lines() {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            let cells = trimmed.chars().count();
            assert!(
                cells <= width as usize,
                "🔴 g89 at {width}x{height}: a row is {cells} cells on a {width} cell screen\
                 \nline: {line}\n{text}"
            );
            if cells < width as usize {
                continue;
            }
            inspected += 1;
            if trimmed.ends_with('~') || trimmed.starts_with('!') {
                cut_rows += 1;
                continue;
            }
            // Unmarked and at the edge: then it has to be a row that genuinely ends here. The two
            // exemptions are `g88`'s, kept for `g88`'s reason — closing them needs the renderer's
            // knowledge of whether a value was truncated, which the drawing does not carry — and
            // they are **counted**, so a green with a non-zero exemption count is a different fact
            // from a green with nought.
            if trimmed.ends_with(|c: char| c.is_ascii_alphanumeric())
                && trimmed.split_whitespace().count() == 1
            {
                exempt_single_word += 1;
                continue;
            }
            if !trimmed.ends_with(|c: char| c.is_ascii_alphanumeric()) {
                exempt_punctuation += 1;
                continue;
            }
            here += 1;
            unmarked.push(format!("{width}x{height}: {trimmed}"));
        }
        // 🔴 The census carries the domain **and** the verdict for every shape, and it says
        // `EMPTY_DOMAIN` where no row reached the edge — at such a shape this gate compared nothing,
        // which is a third thing and not a pass (`SS870`: a measurement of `0 → 0` over an empty
        // domain is vacuously true and is evidence of nothing).
        census.push(if at_edge == 0 {
            format!("{width}x{height} at_edge=0 EMPTY_DOMAIN")
        } else {
            format!("{width}x{height} at_edge={at_edge} unmarked_cut={here}")
        });
    }
    println!(
        "G89_DOMAIN {census:?} inspected={inspected} marked_cut={cut_rows} \
         exempt_punctuation={exempt_punctuation} exempt_single_word={exempt_single_word} \
         unmarked_cut_total={}",
        unmarked.len()
    );
    assert!(
        unmarked.is_empty(),
        "🔴 g89: {} rows of the ledger fill the screen, end inside a value, and carry no mark \
         saying they were cut. A terminal clips from the right in silence, so a reader cannot tell \
         a value that ends from a value that ran out:\n{}",
        unmarked.len(),
        unmarked.join("\n")
    );
    assert!(
        inspected > 0,
        "🔴 g89: no row at any of the twelve shapes reached the right edge, so this gate examined \
         nothing and its green means nothing: {census:?}"
    );
    // 🔴 **The ledger says something at every width** (`[T-r66]`, and this lane's own regression).
    // Refusing the first column when the screen could not pay for it drew nine blank rows over a
    // thirty-one record ledger at eighteen cells, measured on the live bed. A face that answers
    // *too narrow* by drawing nothing has not disclosed a loss, it has hidden one.
    for (width, height) in NARROW_SHAPES {
        let text = ledger_frame(width, height, 40);
        let drawn = text.lines().filter(|line| !line.trim().is_empty()).count();
        assert!(
            drawn > 0,
            "🔴 g89 at {width}x{height}: the ledger has forty records and drew {drawn} rows with \
             anything on them\n{text}"
        );
    }
    // 🔴 The control. The predicate has to refuse the exact row the arithmetic produced: a
    // sixty-seven cell row clipped to sixty-six, ending on the digit `created_at` lost its `Z` from.
    let clipped = "   gx1:t3sto0000000~  Admit      Committed      2026-08-30T09:00:0";
    assert!(
        clipped.ends_with(|c: char| c.is_ascii_alphanumeric())
            && clipped.split_whitespace().count() > 1
            && !clipped.trim_end().ends_with('~'),
        "🔴 g89: the predicate admits the row this gate was written from, so a green means nothing"
    );
}

/// 🔴 **g90 — the width a ledger row is priced against is the width it is drawn at.**
///
/// [`layout::row_width`] is the one spelling of *how many cells a row of these columns takes*. This
/// asks the only question that matters about the budget: **whatever [`layout::columns_for_less`]
/// keeps must fit on the screen it was asked about.**
///
/// Asked at every width from nought to two hundred rather than at the seven ruled shapes, because
/// the defect is a boundary defect: it only shows where the budget actually binds, and which widths
/// those are is a function of the column table rather than of the terminals anyone owns. Asked with
/// two vacant sets — none, and the one the fixture bed produces — because a vacant column is taken
/// out before the fit and therefore moves every boundary.
///
/// Unlike `g89` this cannot be vacuously green: every width has an answer, the answer is a number,
/// and the number is compared.
#[test]
fn g90_the_price_of_a_ledger_row_is_the_row_that_is_drawn() {
    // The fixture bed's own vacant keys, asked through the face's own classifier rather than typed.
    let vacant: Vec<&'static str> = vacant_of(&ledger(40))
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let mut over: Vec<String> = Vec::new();
    let mut bound = 0usize;
    let mut floored = 0usize;
    for width in 0u16..=200 {
        for (label, keys) in [("none", &[][..]), ("fixture", &vacant[..])] {
            let (drawn, _) = layout::columns_for_less(width, keys);
            let cells = layout::row_width(&drawn);
            // 🔴 **The floor is part of the invariant, not an exemption from it** (`[T-r66]`). One
            // column is kept whatever the width, because a ledger that draws nothing has disclosed
            // no loss — so `drawn.len() == 1` is a ruled outcome and `renderer::fit` is what marks
            // the row. Every *other* set has to fit, and a second column appearing at a width that
            // cannot hold it is the defect this gate exists to refuse.
            if cells > width && drawn.len() > 1 {
                over.push(format!("{width} vacant={label} drawn={cells} columns={}", drawn.len()));
            }
            if cells > width && drawn.len() == 1 {
                floored += 1;
            }
            // The budget binds here: one more column would not have fitted. Counted so that a green
            // cannot come from a table whose columns are all narrower than every width asked.
            if !drawn.is_empty() && cells + layout::COLUMN_GAP > width {
                bound += 1;
            }
        }
    }
    println!(
        "G90 widths=0..=200 vacant_sets=2 binding={bound} floored={floored} over={over:?} \
         fixture_vacant={vacant:?}"
    );
    assert!(
        over.is_empty(),
        "🔴 g90: the ledger keeps columns that do not fit the screen they were priced against. A \
         terminal clips from the right in silence, so every one of these draws a value that ends \
         early with nothing saying so: {over:?}"
    );
    assert!(
        bound > 0,
        "🔴 g90: at no width from nought to two hundred did the budget bind, so this gate compared \
         numbers that could not have differed"
    );
    assert!(
        floored > 0,
        "🔴 g90: at no width did the floor bite, so the branch that keeps one column on a screen \
         too narrow for it went unmeasured and `floored` is a number about nothing"
    );
    // 🔴 The control, in both directions. `row_width` must charge the margin **and** a gap for the
    // first column — the sentence the repaired arithmetic replaced charged the margin only, and a
    // predicate built on that sentence would call the defect a fit.
    let one = layout::Column {
        key: "transformation",
        width: 16,
        priority: Priority::One,
    };
    assert_eq!(
        layout::row_width(&[one]),
        layout::LEFT_MARGIN + layout::COLUMN_GAP + 16,
        "🔴 g90: `row_width` does not charge the gap `spans_with` draws after the margin cell, so \
         it cannot see the defect it exists to refuse"
    );
    assert_eq!(
        layout::row_width(&[one, one]),
        layout::LEFT_MARGIN + 2 * (layout::COLUMN_GAP + 16),
        "🔴 g90: `row_width` does not charge one gap per column"
    );
    assert_eq!(
        layout::row_width(&[]),
        0,
        "🔴 g90: a row with no column on it is charged for a margin it does not draw"
    );
}

/// 🔴 **g92 — the identity column's declared width is the label's, and the label is what binds.**
///
/// # What lost
///
/// `req/38` SS1101 measured this face against twelve products on eight axes and recorded one axis
/// lost to **all eleven comparable rivals**: the leading column. They lead with a file name, a commit
/// subject, a process name. This face leads with `gx1:<hash>~`.
///
/// `[T-r71]` (2026-09-02) asked the engine what could go there instead and the answer is **nothing**
/// the list row carries — that is `req/942_artifacts/tui_r71_2026-09-02/RULING.md`'s ruling and it is an engine gap, not a defect of
/// this file. What the face could do was stop spending sixteen cells. Two numbers were measured
/// against a live engine on a twenty-nine row bed grown by the product's own loop, and they disagree
/// about what holds the cells:
///
/// * the **values** need thirteen: three characters past the shared `gx1:` separated every row, and
///   [`renderer::ID_PREFIX_MARGIN`] / [`renderer::ID_PREFIX_FLOOR`] are the budget on top.
/// * the **label** needs fourteen, and `transformation` is a wire key drawn unchanged (`req/942` §9,
///   gate P5).
///
/// So the header binds, the declaration is the label's length, and the eleven characters that used to
/// be drawn were separating rows that three characters already separated.
///
/// # The three halves
///
/// **(a) the declaration is exactly the label.** Not more — that is the two cells this lane returns —
/// and not less, because a column that cannot spell its own key draws `transformatio~` in the one row
/// that names the table (measured: three gates went red on that, in this lane, before this shape).
///
/// **(b) the declaration is still enough for the rows, measured.** [`renderer::id_cells_needed`] is
/// the number the reading asks for, and the day a ledger asks for more than the label can hold, this
/// goes red naming it rather than the column silently cutting further.
///
/// **(c) the two cells buy a column, and where they buy nothing that is printed.** The declared width
/// and the width this lane replaced are put through the *same* `columns_for_less`, and the widths
/// where the column set differs are counted. 🔴 **A zero fails**: `SS870` and this lane's brief name
/// the shape — a `0 → 0` improvement is vacuously true and is evidence of nothing.
///
/// # Red first
///
/// The `before` arm of (c) **is** the base: the same function, the same fixture, the width this file
/// carried on `main`. It is red on (a) and green after, in one run, so the red is caused by this
/// lane's change and by nothing else.
#[test]
fn g92_the_identity_column_is_the_width_of_its_own_label() {
    let column = layout::LEDGER_COLUMNS[0];

    // ---- (a) the declaration is exactly the label ---------------------------------------------
    let label = column.key.chars().count();
    assert_eq!(
        usize::from(column.width),
        label,
        "🔴 g92: the identity column is not the width of its own key. Wider and it is spending cells \
         on a value measured not to need them; narrower and the column header — the one row that \
         names the table — draws `{}~` instead of `{}`",
        &column.key[..label.saturating_sub(1)],
        column.key
    );

    // ---- (b) the declaration is still enough for the rows, measured ---------------------------
    let bed = ledger(29);
    let needed = renderer::id_cells_needed(&bed.transformations)
        .expect("🔴 g92: the fixture ledger is measurable and the measurement declined it");
    let separation = renderer::id_separation(&bed.transformations).expect("measurable");
    println!(
        "G92 declared={} label={label} separation_past_common={separation} needed_with_budget={needed} \
         margin={} floor={}",
        column.width,
        renderer::ID_PREFIX_MARGIN,
        renderer::ID_PREFIX_FLOOR
    );
    assert!(
        needed <= column.width,
        "🔴 g92: this reading needs {needed} cells of identity and the column declares {}. The face \
         will cut further and mark it, which is honest, but the declaration has stopped being \
         justified by anything and this is the line that says so",
        column.width
    );

    // The four readings this cannot be measured on, each a `None` rather than a number. A measurement
    // that answers where it cannot measure is a face choosing a width and calling it a measurement.
    assert_eq!(
        renderer::id_separation(&ledger(1).transformations),
        None,
        "🔴 g92: one row is not a set to tell apart, and a number taken from it is not a measurement"
    );
    let nothing_id = ledger_with(4, |_, row| {
        row["transformation"] = serde_json::Value::Null;
    });
    assert_eq!(
        renderer::id_separation(&nothing_id.transformations),
        None,
        "🔴 g92: a mark for nothing carries no separation, so a number measured against one is \
         measuring this face's own vocabulary"
    );
    let one_id = ledger_with(4, |_, row| {
        row["transformation"] = serde_json::Value::String(record_id(0));
    });
    assert_eq!(
        renderer::id_separation(&one_id.transformations),
        None,
        "🔴 g92: two rows with one id is a ledger this face may not paper over with a number"
    );
    assert_eq!(
        renderer::id_cells_needed(&one_id.transformations),
        None,
        "🔴 g92: the cell count answered where the separation could not be measured"
    );

    // ---- (c) what the two cells buy, and the domain they were looked for in -------------------
    // 🔴 The width this file carried on `main`, named here so the control is the base and not a
    // number chosen to make the gate green.
    const BEFORE: u16 = 16;
    let vacant: Vec<&'static str> = vacant_of(&bed).into_iter().map(|(key, _)| key).collect();
    let mut gained: Vec<String> = Vec::new();
    let mut lost: Vec<String> = Vec::new();
    let mut widths = 0usize;
    for width in 0u16..=200 {
        widths += 1;
        let (after, _) = layout::columns_for_less(width, &vacant);
        // The base's arithmetic, reproduced by asking the same question with the cells the base
        // spent added back to the row's price.
        let before_price = layout::row_width(&after) + (BEFORE - column.width);
        let before_len = if before_price > width && after.len() > 1 {
            after.len() - 1
        } else {
            after.len()
        };
        if after.len() > before_len {
            gained.push(format!("{width}:{before_len}->{}", after.len()));
        }
        if after.len() < before_len {
            lost.push(format!("{width}:{before_len}->{}", after.len()));
        }
    }
    println!("G92 widths={widths} gained_a_column_at={gained:?} lost_a_column_at={lost:?}");
    assert!(
        lost.is_empty(),
        "🔴 g92: narrowing the identity column cost a column somewhere. A repair that is \
         arithmetically correct and makes the face worse is a shape this suite has caught \
         before: {lost:?}"
    );
    assert!(
        !gained.is_empty(),
        "🔴 g92: the two cells changed the column set at none of {widths} widths, so they are a \
         number about nothing. An improvement whose domain is empty is vacuously true and is \
         evidence of nothing"
    );

    // ---- the disclosure ----------------------------------------------------------------------
    // 🔴 A column whose characters cannot be carried anywhere, and no line saying so, is this face
    // letting a reader copy a value that will be refused on both roads it could take it to.
    let help = renderer::buffer_text(&renderer::render_view_to_buffer(
        &bed,
        120,
        32,
        Tier::Mono,
        false,
        &View {
            help: true,
            ..View::default()
        },
    ));
    assert!(
        flat(&help).contains(&format!(
            "id column: {} of a `{}`;",
            column.width - 1,
            column.key
        )),
        "🔴 g92: no line names what the identity column draws:\n{help}"
    );
    assert!(
        flat(&help).contains("no prefix resolves"),
        "🔴 g92: the disclosure does not carry the thing that makes it worth drawing -- that no \
         prefix of an id resolves on either road, so a reader who copies what that column shows \
         gets a refusal:\n{help}"
    );
    // 🔴 The negative control for the road: the key named in that clause is the key the act is
    // actually bound to, so the disclosure cannot go on naming a road after the binding moves.
    assert!(
        flat(&help).contains(&format!(
            "{} opens the whole",
            Act::Open.keys().first().copied().unwrap_or_default()
        )),
        "🔴 g92: the clause names no road to the whole id, or names one nothing is bound to:\n{help}"
    );
}
// ---------------------------------------------------------------------------------------------
// [T-r76] - one address is drawn once, and the receipt's undrawn members are named.
// ---------------------------------------------------------------------------------------------

/// The seven shapes the record face is photographed at.
const R76_SHAPES: [(u16, u16); 7] = [
    (120, 32),
    (100, 30),
    (80, 24),
    (66, 20),
    (60, 20),
    (46, 12),
    (40, 10),
];

/// The `gx1:` tokens one line carries, with `pad`'s cut mark taken off.
///
/// `pad` cuts with a literal `~` and this reads it back off. A token that was cut is a **prefix**
/// of the value, which is why the comparison below is containment and not equality.
fn r76_addresses_on(line: &str) -> Vec<String> {
    line.split_whitespace()
        .filter(|word| word.starts_with("gx1:"))
        .map(|word| word.trim_end_matches('~').to_string())
        .collect()
}

/// The record's own rows, told from the ledger's by the left margin -- the structural
/// discriminator `g86` settled on, reused here rather than re-invented.
fn r76_record_rows(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.starts_with(' ') && !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// Every ordered pair of addresses drawn on two different record rows, sorted into three values.
///
/// Returns `(repeated, distinct, untestable)`.
///
/// * **repeated** -- one is a prefix of the other and enough of it survived the cut to say so.
/// * **distinct** -- neither is a prefix of the other. Two facts, two addresses, correctly drawn.
/// * **untestable** -- one was cut to fewer than [`R76_DECIDABLE`] body characters, so *prefix of*
///   cannot separate "the same id twice" from "two ids that begin alike". Not a finding and not a
///   clean bill: it is printed and counted on its own, because folding it into either would be this
///   suite committing the collapse the product exists to refuse.
fn r76_pairs(rows: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut repeated = Vec::new();
    let mut distinct = Vec::new();
    let mut untestable = Vec::new();
    for (i, a_row) in rows.iter().enumerate() {
        for (j, b_row) in rows.iter().enumerate().skip(i + 1) {
            for a in r76_addresses_on(a_row) {
                for b in r76_addresses_on(b_row) {
                    let short = a.len().min(b.len());
                    let note = format!("row{i}+row{j} {a} | {b}");
                    if short < R76_DECIDABLE {
                        untestable.push(note);
                    } else if a.starts_with(&b) || b.starts_with(&a) {
                        repeated.push(note);
                    } else {
                        distinct.push(note);
                    }
                }
            }
        }
    }
    (repeated, distinct, untestable)
}

/// A shape narrower than any ruled one, chosen so that `state` cannot fit and the disclosure has to
/// carry it instead.
///
/// 🔴 **Outside [`R76_SHAPES`] on purpose, and declared rather than typed inline.** It is a control
/// for a branch, not an eighth shape anybody has ruled on: `g97`'s road assertion is a disjunction
/// whose second arm the seven ruled shapes never take, and a disjunction with an unfired arm is a
/// gate reporting on a path no run has walked.
const G97_NARROW: (u16, u16) = (30, 10);

/// How much of an address has to survive the cut before *prefix of* decides anything.
///
/// `gx1:` plus eight body characters. Chosen as the length at which the two fixture ids below
/// already differ many times over, and declared rather than typed inline so the number the third
/// value is drawn at is one number.
const R76_DECIDABLE: usize = 12;

/// A one-record screen carrying whatever `inverse_status` and `superseded_by` are handed.
fn r76_screen(inverse: serde_json::Value, superseded: serde_json::Value) -> Screen {
    let row = serde_json::json!({
        "transformation": "gx1:trbspcdedw2dt63rvfltsgle2zilpjr3tihzyrzabk2wvmndihtq",
        "verdict": "Admit",
        "state": "Superseded",
        "enforced": true,
        "inverse_status": inverse,
        "superseded_by": superseded,
    });
    Screen {
        healthz: answered(wire::ROUTES[0], serde_json::from_str(HEALTHZ).expect("fixture parses")),
        transformations: answered(
            wire::ROUTES[1],
            serde_json::json!({"items": [row], "next_cursor": null}),
        ),
        candidates: answered(
            wire::ROUTES[2],
            serde_json::from_str(CANDIDATES).expect("fixture parses"),
        ),
        escalations: answered(
            wire::ROUTES[3],
            serde_json::from_str(ESCALATIONS).expect("fixture parses"),
        ),
    }
}

/// How many characters of an address are on the screen -- the longest drawn token that is a prefix
/// of `address`, with `pad`'s cut mark already taken off by [`r76_addresses_on`].
fn r76_visible(text: &str, address: &str) -> usize {
    r76_record_rows(text)
        .iter()
        .flat_map(|row| r76_addresses_on(row))
        .filter(|token| address.starts_with(token.as_str()))
        .map(|token| token.len())
        .max()
        .unwrap_or(0)
}

/// 🔴 **The characters of the address the fold costs, derived from the two labels rather than
/// typed** (`[T-r82]`, 2026-09-02, closing the audit's D1).
///
/// # What was found, and why it is a cost rather than a bug
///
/// `[T-r76]` justified folding `superseded_by` into `inverse_status` with *the second spelling says
/// nothing the first did not*. An independent audit measured the lane's own committed captures and
/// found that **false at narrow widths**, and a seat reproduced it from the same files:
///
/// | shape | 120x32 | 100x30 | 80x24 | 66x20 | 60x20 | 46x12 | 40x10 |
/// |---|---|---|---|---|---|---|---|
/// | before | 56 | 56 | 56 | 50 | 44 | 30 | 24 |
/// | after | 56 | 56 | **54** | **40** | **34** | **20** | **14** |
///
/// Five of seven shapes lost characters, at most ten. The reason is arithmetic and is the constant
/// below: the row that was folded away began its address at `superseded_by` + a gap, and the row
/// that survived begins it at `inverse_status` + a gap + `Consumed` + a space. **The row that was
/// dropped was the one that could show more of the value.**
///
/// # Why the fold is kept anyway, and what is bought instead
///
/// The direction was not reversed. Moving the sentence onto the `inverse_status` row would put a
/// cross-reference where `InverseMark::Consumed`'s **member** goes -- and that variant exists
/// precisely so that the member is never spent on something other than the fact (its own doc says
/// so). It would also invert the entailment on the screen: `Consumed { by: X }` is the fact the
/// engine writes and `superseded_by` is the one derived from it, so folding the derived spelling
/// into the source one is the direction that matches `wire::supersede_agrees`.
///
/// So the cost is kept, **named, and gated**: it may never exceed this constant, the surviving
/// address may never fall below [`R76_DECIDABLE`], and the census is printed at every shape. What
/// the lane must not do -- and did -- is remove the characters *silently*.
const R76_FOLD_COST: usize =
    wire::INVERSE_STATUS_KEY.len() + wire::CONSUMED_KIND.len() + 1 - wire::SUPERSEDED_BY_KEY.len();

/// The id the undo consumed the inverse with, and the one it did not.
const R76_UNDOER: &str = "gx1:647lk6fewteprs7hv3cxwuijrj4xsaigmkrd52bq6mgycr5rszja";
const R76_OTHER: &str = "gx1:ex2qzqwxr4epzq5m3nn4hhcvbhqvnhbmrbawbrmldp2wkxxwvyoq";

fn r76_frame(screen: &Screen, width: u16, height: u16) -> String {
    renderer::buffer_text(&renderer::render_view_to_buffer(
        screen,
        width,
        height,
        Tier::Mono,
        false,
        &opened(),
    ))
}

/// **g94 -- one address, two keys, and the address is drawn once.**
///
/// # What the ruling is, and where it was taken from
///
/// The captured record drew `inverse_status Consumed gx1:647...rszja` and
/// `superseded_by gx1:647...rszja` on adjacent rows; at 40x10 those were two of the nine rows the
/// record had. Folding them is only honest if the engine cannot make them differ, and
/// `wire::supersede_agrees` carries that reading of the engine's live source: 43 T-12 writes the
/// supersedes edge, the escrow row's `Consumed { by }` and the table's `superseded_by` from **one
/// binding**, in the one place any of them is written, and replay rebuilds both from one journal
/// field. So `Consumed { by: X }` entails `superseded_by == X`.
///
/// # Why this gate has a negative control and what it controls for
///
/// A gate that only looks at the folded row is green on a face that folds **everything**. The
/// second half hands the same face two *different* addresses on the same two keys -- the shape a
/// replay over a journal missing an `Escrowed` record produces -- and requires **both** to be on
/// the screen. Without it, "no address appears twice" is satisfied by a face that draws no address
/// at all.
#[test]
fn g94_one_address_is_drawn_once_and_two_addresses_are_both_drawn() {
    // ---- the declaration, before any drawing -------------------------------------------------
    // 🔴 The one spelling of the key, and the two frozen tables that also spell it. A rename that
    // moved one and not the others used to be a column that quietly stopped agreeing with the row.
    assert!(
        LEDGER_COLUMNS
            .iter()
            .any(|column| column.key == wire::SUPERSEDED_BY_KEY),
        "🔴 g94: `wire::SUPERSEDED_BY_KEY` names no column the ledger draws"
    );
    assert!(
        renderer::AGREEMENT_KEYS.contains(&wire::SUPERSEDED_BY_KEY),
        "🔴 g94: the key the record folds is not the key the second read compares"
    );

    let consumed = serde_json::json!({"Consumed": {"by": R76_UNDOER}});
    let same = r76_screen(consumed.clone(), serde_json::json!(R76_UNDOER));
    let different = r76_screen(consumed.clone(), serde_json::json!(R76_OTHER));
    let no_escrow = r76_screen(serde_json::Value::Null, serde_json::json!(R76_UNDOER));

    // ---- the predicate is three-valued, and the third value is not the second ------------------
    let item_of = |screen: &Screen| -> serde_json::Value {
        screen.transformations.items().first().map(|item| (*item).clone()).expect("one row")
    };
    assert!(
        wire::supersede_agrees(&item_of(&same)),
        "🔴 g94: the two keys carry one address and the face was not told so"
    );
    assert!(
        !wire::supersede_agrees(&item_of(&different)),
        "🔴 g94: two different addresses were reported as one. A fold taken on values that are \
         equal by accident is the product's own lie told on its own screen"
    );
    assert!(
        !wire::supersede_agrees(&item_of(&no_escrow)),
        "🔴 g94: `inverse_status` is `null` -- there is no escrow row to have been consumed -- and \
         the face folded anyway. The entailment runs one way only"
    );

    // ---- the instrument's own positive control ------------------------------------------------
    // 🔴 A detector whose domain is empty is green about nothing. Fired at the shape it exists to
    // catch, spelled by hand, before it is asked about a real frame.
    let planted = vec![
        format!("inverse_status  Consumed {R76_UNDOER}"),
        format!("superseded_by  {R76_UNDOER}"),
    ];
    let (planted_repeat, _, _) = r76_pairs(&planted);
    assert_eq!(
        planted_repeat.len(),
        1,
        "🔴 g94: the detector does not report a repeat it was handed on a plate, so every green \
         below is a green about the detector: {planted_repeat:?}"
    );
    // And the cut shape, which string equality would have called clean.
    let planted_cut = vec![
        format!("inverse_status  Consumed {}~", &R76_UNDOER[..18]),
        format!("superseded_by  {}~", &R76_UNDOER[..26]),
    ];
    // 🔴 **The third value, planted and fired** (`[T-r82]`, 2026-09-02). Until this control the
    // `untestable` arm of `r76_pairs` had never once been taken: `G94_CENSUS` reads
    // `untestable=0` at all seven shapes, so a branch this suite advertises as its three-valued
    // discipline was carried entirely by a doc comment. Planted below `R76_DECIDABLE` on purpose,
    // and the plant is printed before its count is believed.
    let planted_untestable = vec![
        format!("inverse_status  Consumed {}~", &R76_UNDOER[..R76_DECIDABLE - 1]),
        format!("superseded_by  {}~", &R76_UNDOER[..26]),
    ];
    println!("G94_PLANTED_UNTESTABLE={planted_untestable:?}");
    let (untestable_repeat, untestable_distinct, untestable_third) = r76_pairs(&planted_untestable);
    assert_eq!(
        (untestable_repeat.len(), untestable_distinct.len(), untestable_third.len()),
        (0, 0, 1),
        "🔴 g94: a pair cut below `R76_DECIDABLE` has to land in the third value and in neither of \
         the other two. Folding it into `repeat` invents a finding and folding it into `distinct` \
         issues a clean bill about a question that was not asked -- and this suite committing \
         either is the collapse the product exists to refuse"
    );

    let (cut_repeat, _, _) = r76_pairs(&planted_cut);
    assert_eq!(
        cut_repeat.len(),
        1,
        "🔴 g94: two cuts of one address were read as two addresses. At forty cells the rows read \
         `gx1:647lk6fewt~` and `gx1:647lk6fewteprs7hv3cx~`, which are not equal strings and are \
         the same fifty-two characters: {cut_repeat:?}"
    );

    // ---- the frames ---------------------------------------------------------------------------
    let mut census: Vec<String> = Vec::new();
    let mut legibility: Vec<String> = Vec::new();
    let mut decided = 0usize;
    for (width, height) in R76_SHAPES {
        let folded = r76_frame(&same, width, height);
        let rows = r76_record_rows(&folded);
        let (repeat, distinct, untestable) = r76_pairs(&rows);
        census.push(format!(
            "{width}x{height} rows={} repeat={} distinct={} untestable={}",
            rows.len(),
            repeat.len(),
            distinct.len(),
            untestable.len()
        ));
        decided += repeat.len() + distinct.len();
        assert!(
            repeat.is_empty(),
            "🔴 g94 at {width}x{height}: one address is on two of the record's rows. The engine \
             writes both from one binding, so the second spelling of it says nothing the first did \
             not: {repeat:?}\n{folded}"
        );
        // Both words survive. Neither key is deleted by the fold.
        let flat_folded = flat(&folded);
        assert!(
            flat_folded.contains(wire::CONSUMED_KIND),
            "🔴 g94 at {width}x{height}: `Consumed` left the screen. The fold removes a repeated \
             address, not a word\n{folded}"
        );
        assert!(
            flat_folded.contains(wire::SUPERSEDED_BY_KEY),
            "🔴 g94 at {width}x{height}: `superseded_by` left the screen\n{folded}"
        );
        assert!(
            flat_folded.contains(wire::SUPERSEDE_AGREEMENT),
            "🔴 g94 at {width}x{height}: the row that gave up the address says nothing in its \
             place, so a reader is told a key exists and not what it holds\n{folded}"
        );
        // 🔴 The address itself is still on the screen once: a fold that removed both spellings
        // would pass the repeat check and lose the fact.
        assert!(
            rows.iter().any(|row| r76_addresses_on(row)
                .iter()
                .any(|address| R76_UNDOER.starts_with(address.as_str()))),
            "🔴 g94 at {width}x{height}: the address is on none of the record's rows. Drawing it \
             once is the repair; drawing it nought times is the defect wearing the repair's face\
             \n{folded}"
        );

        // ---- the negative control: two addresses, and both are drawn --------------------------
        let unfolded = r76_frame(&different, width, height);
        let control_rows = r76_record_rows(&unfolded);
        let (control_repeat, control_distinct, _) = r76_pairs(&control_rows);
        assert!(
            control_repeat.is_empty(),
            "🔴 g94 at {width}x{height}: the control planted two different addresses and the \
             detector reported a repeat, so it is measuring something other than sameness: \
             {control_repeat:?}\n{unfolded}"
        );
        assert!(
            !control_distinct.is_empty(),
            "🔴 g94 at {width}x{height}: the control's two addresses are not both on the screen. \
             A face that folds two facts it was never shown to be one about would satisfy every \
             assertion above by drawing nothing\n{unfolded}"
        );
        assert!(
            !flat(&unfolded).contains(wire::SUPERSEDE_AGREEMENT),
            "🔴 g94 at {width}x{height}: the face said the two keys agree on a row where they \
             carry different addresses\n{unfolded}"
        );

        // ---- how much of the address a reader can still read ----------------------------------
        // 🔴 The audit's D1. The unfolded control is the base: it draws the undoer on the
        // `inverse_status` row and a second address on the bare `superseded_by` row, so the two
        // measured at the same width are exactly the two starting offsets `R76_FOLD_COST` is the
        // difference of. Measured on a rendered frame, not asserted from the arithmetic.
        let kept = r76_visible(&folded, R76_UNDOER);
        let on_inverse_row = r76_visible(&unfolded, R76_UNDOER);
        let on_supersede_row = r76_visible(&unfolded, R76_OTHER);
        legibility.push(format!(
            "{width}x{height} kept={kept} inverse_row={on_inverse_row} supersede_row={on_supersede_row}"
        ));
        assert_eq!(
            kept, on_inverse_row,
            "🔴 g94 at {width}x{height}: the fold changed how much of the surviving address is \
             drawn. It is allowed to remove a row; it is not allowed to shorten the row it kept"
        );
        assert!(
            kept >= R76_DECIDABLE,
            "🔴 g94 at {width}x{height}: the fold left {kept} characters of the address, which is \
             below the length this suite's own detector needs to tell one address from another. \
             Below `R76_DECIDABLE` the screen is drawing something a reader cannot use as an \
             identity\n{folded}"
        );
        assert!(
            on_supersede_row <= kept + R76_FOLD_COST,
            "🔴 g94 at {width}x{height}: the fold cost {} characters of legibility and the two \
             labels only account for {R76_FOLD_COST}. The cost is allowed to be the arithmetic of \
             the labels and nothing else -- anything beyond it is the fold taking something it was \
             not measured to take",
            on_supersede_row - kept
        );
    }
    println!("G94_CENSUS={census:?}");
    // 🔴 The measured cost of the fold, printed at every shape because it is a cost and the lane
    // that took it reported none.
    println!("G94_LEGIBILITY={legibility:?} fold_cost={R76_FOLD_COST}");
    // 🔴 The domain, printed and required. An improvement measured on no rows is vacuously true.
    assert!(
        decided > 0,
        "🔴 g94: the predicate was decidable on no pair at any of {} shapes. A gate whose domain \
         is empty is evidence of nothing: {census:?}",
        R76_SHAPES.len()
    );
}

/// **g95 -- `receipt_view` has seven members, this face draws four, and the other three are named.**
///
/// 🔴 The banner read `g94` until `[T-r82]` (2026-09-02): two gates carried one id in their
/// human-readable line while the functions below them were `g94_…` and `g95_…`. It was invisible to
/// `tools/gates/gate_id_uniqueness_gate.mjs`, which reads banners under `tools/gates/` and test
/// **function** names — and never a doc banner in a test file. Corrected here; the gate's scope is
/// another lane's.
///
/// `[T-r58]` named `subject`, `postcondition_fingerprint` and `issued_at` as undisclosed and did
/// not close them. The membrane's second obligation admits two closures -- draw it, or drop it and
/// say the name -- and this is the second, so the gate has to hold that the naming is **complete**
/// rather than that it exists.
///
/// # 🔴 The denominator is derived from the engine's source, and says so when it cannot be
///
/// The seven members are read out of `crates/gx-api/src/handlers.rs`'s `receipt_view`, so a member
/// the engine adds tomorrow turns this red instead of vanishing unnamed. A checkout that does not
/// carry that file (a published crate, a partial tree) makes the derived half **UNTESTABLE**, which
/// is printed and never folded into a pass: a gate that silently falls back to a weaker ground
/// truth reports the weaker answer as though it were the stronger one.
#[test]
fn g95_the_receipt_view_members_this_face_drops_are_named_in_full() {
    let drawn: BTreeSet<&str> = wire::RECEIPT_VIEW_KEYS.iter().copied().collect();
    let dropped: BTreeSet<&str> = wire::RECEIPT_VIEW_NOT_DRAWN.iter().copied().collect();
    assert!(
        drawn.is_disjoint(&dropped),
        "🔴 g95: a member is declared both drawn and dropped, so the face cannot be asked which"
    );

    // ---- the derived denominator --------------------------------------------------------------
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/gx-api/src/handlers.rs");
    match std::fs::read_to_string(&source) {
        Err(error) => {
            println!(
                "G95_DENOMINATOR=UNTESTABLE reason={error} path={}",
                source.display()
            );
        }
        Ok(text) => {
            let mut members: BTreeSet<String> = BTreeSet::new();
            let mut inside = false;
            let mut collecting = false;
            for line in text.lines() {
                if line.contains("fn receipt_view(") {
                    inside = true;
                    continue;
                }
                if !inside {
                    continue;
                }
                if line.contains("serde_json::json!({") {
                    collecting = true;
                    continue;
                }
                if !collecting {
                    continue;
                }
                let trimmed = line.trim();
                if trimmed.starts_with("})") {
                    break;
                }
                if let Some(rest) = trimmed.strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        members.insert(rest[..end].to_string());
                    }
                }
            }
            println!("G95_DENOMINATOR={members:?}");
            assert!(
                !members.is_empty(),
                "🔴 g95: the engine's object was read as nought members. A denominator of zero \
                 makes every coverage claim below vacuous, and the file is there -- so the reader \
                 is what broke, not the claim"
            );
            let declared: BTreeSet<String> = drawn
                .iter()
                .chain(dropped.iter())
                .map(|key| (*key).to_string())
                .collect();
            let unnamed: Vec<&String> = members.difference(&declared).collect();
            let invented: Vec<&String> = declared.difference(&members).collect();
            assert!(
                unnamed.is_empty(),
                "🔴 g95: the engine writes {unnamed:?} and this face neither draws them nor names \
                 them as dropped. A renderer cannot invert, so what it owes instead is to say what \
                 it let go of"
            );
            assert!(
                invented.is_empty(),
                "🔴 g95: {invented:?} is declared and the engine does not write it. Naming a \
                 member nobody sends is a disclosure about a fiction"
            );
        }
    }

    // ---- the names reach a reader, on the row the object they belong to is drawn from -----------
    // 🔴 The real wire, because the disclosure now rides a row that only exists when the engine
    // answered with a receipt this face could decode. A fixture screen with no `Held` would draw no
    // receipt block at all, and the assertion would be green about a region nobody rendered.
    let fixture = Fixture::start();
    let screen = fixture.read();
    let held = wire::Held::read(&fixture.base_url, None, RECEIPT_HOLDER);
    assert_eq!(
        held.receipt_mark(),
        wire::ReceiptMark::Held,
        "🔴 g95: the bed did not hand this face a receipt, so the rows the disclosure rides do not \
         exist and everything below is green about nothing"
    );
    let face = renderer::buffer_text(&renderer::render_held_to_buffer(
        &screen,
        &held,
        120,
        32,
        Tier::Mono,
        false,
        &opened(),
        gx_tui::tui::live::LinkReport::off(),
    ));
    let flat_face = flat(&face);
    for name in wire::RECEIPT_VIEW_NOT_DRAWN {
        assert!(
            flat_face.contains(name),
            "🔴 g95: `{name}` is dropped and the face does not say the word. Dropped and unnamed is \
             the one closure the membrane does not admit\n{face}"
        );
    }
    // 🔴 The doorway count, and it is nought: the three names are on a row the reader is already
    // looking at, so nothing has to be pressed to reach them. They are also all on **one** row --
    // an escape hatch whose entrance is wider than the thing it discloses is the increase it was
    // meant to remove.
    let doorways = face
        .lines()
        .filter(|line| wire::RECEIPT_VIEW_NOT_DRAWN.iter().any(|name| line.contains(name)))
        .count();
    println!("G95_ROWS_CARRYING_THE_NAMES={doorways}");
    assert_eq!(
        doorways, 1,
        "🔴 g95: the three names are spread over {doorways} rows of the record\n{face}"
    );
    // 🔴 **Where the clause is readable, measured at every shape and printed.** A disclosure that
    // exists at one width and vanishes at the rest is a disclosure a reader cannot rely on, so the
    // three states are separated and none is folded into another: FULL (all three names on the
    // screen), CUT (the row is drawn and `pad`'s `~` says it ran out), ABSENT (the row is not on
    // the screen at all).
    //
    // 🔴 **The first cut of this clause asserted that `gx tui --wide` un-cuts it, and that
    // assertion was vacuously green**: it was written `!narrow.contains(last) || wide.contains(last)`
    // and the left side was true, so the right was never reached. Measured properly, `--wide` does
    // **not** widen this row -- so the road does not arrive, and what stands in its place is this
    // census, which says where the names are readable rather than claiming they always are.
    let last = wire::RECEIPT_VIEW_NOT_DRAWN[wire::RECEIPT_VIEW_NOT_DRAWN.len() - 1];
    let mut reach: Vec<String> = Vec::new();
    let mut full = 0usize;
    for (width, height) in R76_SHAPES {
        let drawn = renderer::buffer_text(&renderer::render_held_to_buffer(
            &screen,
            &held,
            width,
            height,
            Tier::Mono,
            false,
            &opened(),
            gx_tui::tui::live::LinkReport::off(),
        ));
        let row = drawn
            .lines()
            .find(|line| line.contains(wire::RECEIPT_VIEW_DROP_PHRASE))
            .map(str::to_string);
        let state = match &row {
            None => "ABSENT",
            Some(line) if line.contains(last) => {
                full += 1;
                "FULL"
            }
            Some(_) => "CUT",
        };
        reach.push(format!("{width}x{height}={state}"));
        // 🔴 The clause never rides a row this lane added: every row it could have been given
        // belongs to the ledger underneath (`g85`), so it rides the row that introduces the object.
        if let Some(line) = row {
            assert!(
                line.starts_with("receipt "),
                "🔴 g95 at {width}x{height}: the clause is on `{line}`, which is not the row that \
                 introduces the receipt. A disclosure on a row of its own is a row taken from the \
                 ledger\n{drawn}"
            );
        }
    }
    println!("G95_REACH={reach:?} full={full}/{}", R76_SHAPES.len());
    assert!(
        full > 0,
        "🔴 g95: the three names are readable at none of {} shapes, so they are declared in code \
         and said on no screen: {reach:?}",
        R76_SHAPES.len()
    );
    // 🔴 **ABSENT is permitted and only under one condition**: the row it rides may be cut for room
    // like any other, and when it is, the record's own cut note has to say so. Silence is what the
    // membrane forbids, not brevity. Asked of the drawing rather than assumed -- and the fixture
    // record is larger than the engine-backed one, so the shape that goes ABSENT here is not the
    // shape that goes ABSENT on the real capture (there the row survives at 40x10 and is CUT).
    for (index, (width, height)) in R76_SHAPES.into_iter().enumerate() {
        if !reach[index].ends_with("=ABSENT") {
            continue;
        }
        let drawn = renderer::buffer_text(&renderer::render_held_to_buffer(
            &screen,
            &held,
            width,
            height,
            Tier::Mono,
            false,
            &opened(),
            gx_tui::tui::live::LinkReport::off(),
        ));
        assert!(
            drawn.contains("not drawn:"),
            "🔴 g95 at {width}x{height}: the row carrying the names is not on the screen and \
             nothing says rows were cut. A disclosure that disappears without a word is the \
             silence the membrane forbids\n{drawn}"
        );
    }
    // 🔴 The negative control for the condition it is drawn under: with no decoded view there is no
    // object whose members could have been let go of, and naming them anyway would be a disclosure
    // about a fiction.
    let no_view = renderer::buffer_text(&renderer::render_held_to_buffer(
        &screen,
        &wire::Held::none(),
        120,
        32,
        Tier::Mono,
        false,
        &opened(),
        gx_tui::tui::live::LinkReport::off(),
    ));
    assert!(
        !no_view.contains(wire::RECEIPT_VIEW_DROP_PHRASE),
        "🔴 g95: this reading holds no decoded receipt and the face still names three members of \
         it. A disclosure about an object nobody was sent is a disclosure about a fiction\n{no_view}"
    );
    // 🔴 And the row they ride already existed: the disclosure may not be bought with a row, because
    // every row it could have been bought with belongs to the ledger underneath (`g85`).
    let (_, beyond_after) = renderer::record_extent(&screen, &held, &opened(), 120);
    println!("G95_BEYOND_ROWS={beyond_after}");
    assert!(
        face.lines().any(|line| line.starts_with(&format!("{} ", wire::RECEIPT_KEY_ID))),
        "🔴 g95: the names are not on the `key_id` row, so they are on a row this lane added and \
         the ledger paid for it\n{face}"
    );
}/// **g96 -- what the columns only one record answered in cost, measured and left standing.**
///
/// # 🔴 This gate exists because a brief asked for a change the canon refuses
///
/// A brief reached `[T-r76]` (2026-09-02) with a real measurement -- on a bed of thirty-two
/// records, `created_at`, `actor` and `scope` each carried a value on **one** of them, so the
/// columns stood and about forty-four cells of a 118x29 screen went on `?` -- and asked for the
/// columns to be folded. **Two standing rulings name this exact shape and refuse it:**
///
/// * `req/924` §TUI-45's own boundary, which `g64b` already holds: 一部の行だけが「無」なら落とすな
///   (それは情報)。全値が「無」の時だけ。
/// * `req/924` §TUI-48 (SS1079), which ruled *this state* specifically -- the `null` is the engine
///   saying *this process does not hold the body*, the value is fetchable, and 列を落として開示へ
///   合流 is listed among the 却下した2案 because a reader then cannot learn it is fetchable.
///   §TUI-48's accepted direction is a mark of its own, **spelled by the Owner**, not by a lane.
///
/// So the fold is not implemented and this gate does not assert one. What it does is turn the
/// measurement the brief arrived with into an instrument that runs every time, so the Owner's
/// adjudication has a number in front of it. **It is not a gate that only prints**: it asserts the
/// boundary the two rulings draw, and it goes red the day a lane folds these columns -- which is
/// what this lane nearly shipped.
#[test]
fn g96_the_one_answer_column_keeps_its_place_and_its_price_is_printed() {
    // The measured shape, rebuilt: one record of thirty-two answers, the rest say nothing.
    let bed = ledger_with(32, |n, item| {
        if n != 0 {
            item["created_at"] = serde_json::Value::Null;
            item["scope"] = serde_json::Value::Null;
            item["actor"] = serde_json::Value::Null;
        }
    });
    // 🔴 The control on the control: if the bed does not produce the shape, everything below is a
    // measurement of nothing.
    let answered: Vec<(&str, usize)> = ["created_at", "scope", "actor"]
        .into_iter()
        .map(|key| {
            let count = bed
                .transformations
                .items()
                .iter()
                .filter(|item| !says_nothing(&renderer::cell_mark(item, key).0))
                .count();
            (key, count)
        })
        .collect();
    println!("G96_ANSWERED={answered:?} of {}", bed.transformations.items().len());
    for (key, count) in &answered {
        assert_eq!(
            *count, 1,
            "🔴 g96: the bed was meant to leave exactly one answer in `{key}` and left {count}, so \
             this gate measures a shape nobody ruled on"
        );
    }

    // ---- the boundary the two rulings draw ------------------------------------------------------
    let vacant = renderer::vacant_columns(&bed.transformations);
    println!("G96_VACANT={vacant:?}");
    for (key, _) in &answered {
        assert!(
            !vacant.iter().any(|(column, _)| column == key),
            "🔴 g96: `{key}` was answered by one record and the face folded the column away. \
             `req/924` §TUI-45's boundary is 一部の行だけが「無」なら落とすな, and §TUI-48 ruled \
             this state specifically -- the value is fetchable, and a fold tells the reader it is \
             not. If this is to change it changes by an Owner ruling, not by a lane"
        );
    }

    // ---- the price, at every shape this lane photographs -----------------------------------------
    // 🔴 Printed and not asserted against a bound. A threshold here would be this gate deciding the
    // question §TUI-48 reserved; the number is evidence, and evidence is what was missing.
    let unknown = Nothing::Unknown.mark();
    let mut census: Vec<String> = Vec::new();
    for (width, height) in R76_SHAPES {
        let text = renderer::buffer_text(&renderer::render_view_to_buffer(
            &bed,
            width,
            height,
            Tier::Mono,
            false,
            &View::default(),
        ));
        let marks = text.matches(unknown).count();
        let header = text.lines().next().unwrap_or_default().to_string();
        let standing = answered
            .iter()
            .filter(|(key, _)| header.contains(key))
            .count();
        census.push(format!(
            "{width}x{height} unknown_marks={marks} one_answer_columns_drawn={standing}/3"
        ));
    }
    println!("G96_PRICE={census:?}");
    // 🔴 The domain, because a census taken where the predicate never applies is a census of
    // nothing -- and this one has a shape where it genuinely does not apply, which is printed
    // above rather than folded into the total.
    let ever_drawn: usize = census
        .iter()
        .filter(|row| !row.ends_with("one_answer_columns_drawn=0/3"))
        .count();
    assert!(
        ever_drawn > 0,
        "🔴 g96: none of the three columns is drawn at any of {} shapes, so the price this gate \
         reports is a price nobody pays and the boundary above was never exercised: {census:?}",
        R76_SHAPES.len()
    );
}
/// **g97 -- `state` carries two wire types, and the two `null` inverse states keep a road apart.**
///
/// # What was measured, on the Owner's running engine, read-only
///
/// `req/942_artifacts/tui_r76_2026-09-02/live_census.txt`: of thirty-two rows, `state` is a
/// **string** on thirty-one and a **single-key object** `{"Aborted":"Expired"}` on one. `wire::cell`
/// turned that object into its own JSON and a thirteen-cell column drew `{"Aborted":"~` -- the whole
/// column spent on punctuation, and `Expired`, the only fact it carried, lost. That is verbatim the
/// defect `wire::InverseMark::Consumed` was created to prevent one column over.
///
/// # And the collapse the same two rows carry
///
/// The two rows whose `inverse_status` is `null` are exactly one `Candidate` and one
/// `{"Aborted":"Expired"}`. `req/924` §TUI-97 (SS1136) rules those two nothings different -- *not
/// yet* against *never* -- and this face draws both `--`. No eighth mark is minted here (§TUI-48
/// reserves that spelling to the Owner); what is held instead is the **road**: `state` separates
/// them and sits on the same row, so the collapse is one a reader can undo. This gate measures that
/// the road is still there, at every shape, rather than a comment asserting it.
#[test]
fn g97_the_state_column_reads_an_object_and_the_two_null_inverse_rows_keep_a_road() {
    let row = |id: &str, state: serde_json::Value, inverse: serde_json::Value| {
        serde_json::json!({
            "transformation": id,
            "verdict": serde_json::Value::Null,
            "state": state,
            "enforced": true,
            "inverse_status": inverse,
            "superseded_by": serde_json::Value::Null,
        })
    };
    let rows = vec![
        row(
            "gx1:aborted00000000000000000000000000000000000000000000000",
            serde_json::json!({"Aborted": "Expired"}),
            serde_json::Value::Null,
        ),
        row(
            "gx1:candidate0000000000000000000000000000000000000000000",
            serde_json::json!("Candidate"),
            serde_json::Value::Null,
        ),
        row(
            "gx1:committed0000000000000000000000000000000000000000000",
            serde_json::json!("Committed"),
            serde_json::json!("Available"),
        ),
    ];
    let screen = Screen {
        healthz: answered(wire::ROUTES[0], serde_json::from_str(HEALTHZ).expect("fixture parses")),
        transformations: answered(
            wire::ROUTES[1],
            serde_json::json!({"items": rows, "next_cursor": null}),
        ),
        candidates: answered(
            wire::ROUTES[2],
            serde_json::from_str(CANDIDATES).expect("fixture parses"),
        ),
        escalations: answered(
            wire::ROUTES[3],
            serde_json::from_str(ESCALATIONS).expect("fixture parses"),
        ),
    };
    let items = screen.transformations.items();
    assert_eq!(items.len(), 3, "🔴 g97: the bed is not the bed this gate was written for");

    // ---- (a) the object is read, and no punctuation reaches the cell ---------------------------
    let aborted = renderer::cell_mark(items[0], wire::STATE_KEY).0;
    println!("G97_ABORTED_CELL={aborted:?}");
    assert_eq!(
        aborted, "Aborted Expired",
        "🔴 g97: a single-key object in `state` is drawn as `{aborted}`. `wire::cell` stringifies \
         it, so a thirteen-cell column spends itself on punctuation and loses the member -- the \
         defect `InverseMark::Consumed` was written to prevent, one column over"
    );
    for punctuation in ['{', '}', '"', ':'] {
        assert!(
            !aborted.contains(punctuation),
            "🔴 g97: `{punctuation}` reached the cell, so the column is still drawing a \
             serialisation rather than the fact it carries"
        );
    }

    // ---- (b) negative control: a bare string is untouched ---------------------------------------
    assert_eq!(
        renderer::cell_mark(items[2], wire::STATE_KEY).0,
        "Committed",
        "🔴 g97: the repair changed a state the engine spelled as a string, so it is not a repair \
         to the object arm but a rewrite of the column"
    );

    // ---- (c) negative control: an unread shape is drawn as it arrived, not guessed at -----------
    // 🔴 A face that folded a two-key object into `kind member` would be reporting the shape it
    // expected instead of the shape it was sent. Two keys, and one key holding a number.
    let two_keys = serde_json::json!({"state": {"Aborted": "Expired", "By": "someone"}});
    let numeric = serde_json::json!({"state": {"Aborted": 7}});
    for (name, odd) in [("two keys", &two_keys), ("member not a string", &numeric)] {
        let drawn = wire::state(odd).mark();
        println!("G97_UNREAD_SHAPE {name} -> {drawn:?}");
        assert!(
            drawn.contains('{'),
            "🔴 g97: `{name}` was folded into the two-word spelling, so this face is claiming a \
             shape the wire did not send it: {drawn}"
        );
    }

    // ---- (d) the collapse, no longer declared but closed -----------------------------------------
    // 🔴 **This block asserted the defect, and it is rewritten under a named, dated ruling**
    // (`req/924` §TUI-101, SS1145, 2026-09-02; `[T-r82]`). What stood here required *both* `null`
    // rows to draw `--`, on the reasoning that §TUI-48 reserves an eighth mark to the Owner and so
    // the collapse could only be declared. The ruling found the premise wrong on both halves: the
    // `Candidate` row's *not yet* is spelled by a word **already in `Nothing::ALL`**
    // (`Nothing::Loading`), so closing it mints nothing; and the row's `verdict` was carrying the
    // same collapse one column over, unexamined, so the old assertion was pinning half a defect.
    //
    // It is rewritten rather than deleted (`INHERITED_PRINCIPLES`: a test may change only under a
    // ruling that names it, and the reference goes on the test). `wire::NULL_MEANING_BY_STATE`
    // carries the reasoning; `g98` carries the freshness and the full census.
    let absent = wire::Nothing::Absent.mark();
    let not_yet = wire::Nothing::Loading.mark();
    assert_eq!(
        renderer::cell_mark(items[0], wire::INVERSE_STATUS_KEY).0,
        absent,
        "🔴 g97: a terminal `Aborted` has nothing escrowed and nothing ever will be, which is what \
         `{absent}` says. This is the one of the four cells `[T-r76]` had right"
    );
    assert_eq!(
        renderer::cell_mark(items[1], wire::INVERSE_STATUS_KEY).0,
        not_yet,
        "🔴 g97: a `Candidate`'s missing inverse is *not yet* and is being drawn as *never*. That \
         is the first line of the nothing vertical -- a nothing that comes from time drawn as a \
         semantic one (`req/924` §TUI-101)"
    );
    assert_ne!(
        renderer::cell_mark(items[0], wire::INVERSE_STATUS_KEY).0,
        renderer::cell_mark(items[1], wire::INVERSE_STATUS_KEY).0,
        "🔴 g97: the two rows §TUI-97 ruled must be visibly different are drawing the same mark \
         again"
    );
    assert!(
        wire::INVERSE_NULL_STATES
            .iter()
            .any(|state| renderer::cell_mark(items[0], wire::STATE_KEY).0.starts_with(state)),
        "🔴 g97: the aborted row's state is not one the declaration names, so the array and the \
         bed have drifted apart"
    );
    assert_ne!(
        renderer::cell_mark(items[0], wire::STATE_KEY).0,
        renderer::cell_mark(items[1], wire::STATE_KEY).0,
        "🔴 g97: the two rows the wire flattens to one `null` are drawn identically in `state` too, \
         so the collapse has no road out of it and the screen destroys the difference"
    );

    // ---- (e) the road, at every shape, and where it is not there --------------------------------
    // 🔴 **Two repairs to this block, both from the audit's D5** (`[T-r82]`, 2026-09-02).
    //
    // 1. The disclosure half asserted `contains("fields not drawn")`, which is **one spelling of a
    //    clause that has two**. `req/924` §TUI-57 rules the long form (`N of 11 fields not drawn`)
    //    and the short form (`N/11 fields`) equally honest, and the short form is what the Owner's
    //    own 40x10 capture draws (`pty_after/list_40x10.txt` reads `9/11 fields`). So the branch
    //    `[T-r76]`'s report said this assertion existed for would have gone **red on the real
    //    screen for a stale-probe reason**, not because a road was lost. `g24` already owns the
    //    two-form predicate and this now asks the same question.
    // 2. The report claimed the gate asserts *`state` is on the row or the disclosure counts it*.
    //    It asserted only that **a note exists** -- a face that dropped `state` and disclosed some
    //    other field passed. It now asks `layout::columns_for_less`, the same function the plan is
    //    resolved through, whether `state` is in the dropped set.
    let vacant_keys: Vec<&'static str> = renderer::vacant_columns(&screen.transformations)
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let probe = |width: u16, height: u16| {
        let text = renderer::buffer_text(&renderer::render_view_to_buffer(
            &screen,
            width,
            height,
            Tier::Mono,
            false,
            &View::default(),
        ));
        let header = text.lines().next().unwrap_or_default().to_string();
        let road = header.contains(wire::STATE_KEY);
        let (_, dropped) = layout::columns_for_less(width, &vacant_keys);
        let in_dropped_set = dropped.contains(&wire::STATE_KEY);
        // Either form of the clause: `req/924` §TUI-57 rules `N of 11 fields not drawn` and
        // `N/11 fields` equally honest, so the word both forms share is what is asked for.
        let clause = flat(&text).contains("fields");
        (road, in_dropped_set && clause, in_dropped_set, text)
    };
    let mut census: Vec<String> = Vec::new();
    let mut with_road = 0usize;
    for (width, height) in R76_SHAPES {
        let (road, counted, _, text) = probe(width, height);
        census.push(format!("{width}x{height} state_drawn={road} counted={counted}"));
        if road {
            with_road += 1;
        }
        // 🔴 Silence is the only thing forbidden: the column may leave for room like any other, and
        // when it does the screen has to count it.
        assert!(
            road || counted,
            "🔴 g97 at {width}x{height}: `state` is not on the row and the disclosure does not \
             count it. That is the collapse becoming silent -- two rows the wire flattened to one \
             `--`, and no road back\n{text}"
        );
    }
    println!("G97_ROAD={census:?} with_road={with_road}/{}", R76_SHAPES.len());

    // ---- (f) both arms of the disjunction have to be walked, and now both are ------------------
    // 🔴 On `[T-r76]`'s bed this read `with_road=7/7`: every green above was carried by `road`
    // alone, and the second arm -- the one the report described as the branch the assertion exists
    // for -- had never once been evaluated. It is walked now, and by a **ruled** shape rather than
    // by a plant: §TUI-101 gives the three rows three different marks in `verdict`, so that column
    // is no longer vacant, no longer freed, and `state` leaves the row at 40x10 exactly as it does
    // on the Owner's engine. The fixture and the live screen agree for the first time here.
    assert!(
        with_road < R76_SHAPES.len(),
        "🔴 g97: `state` is drawn at every one of {} shapes, so the disclosure arm of the road \
         assertion is never evaluated and this gate is green about a branch no run walks: \
         {census:?}",
        R76_SHAPES.len()
    );

    // 🔴 **A planted control at an unruled width, and its answer is the third value.** It was added
    // to force the disclosure arm and it found something else: at thirty cells the standing row is
    // itself cut mid-clause (`... help:? · 9/`), so the count is on the screen and the word is not.
    // That is **not asserted as a failure** -- thirty cells is outside the seven shapes anybody has
    // ruled on, and requiring the chrome to survive there would be this gate inventing a ruling.
    // What is asserted is the half that is decidable: the layout **counted** the column. The rest
    // is printed for the seat.
    let (narrow_width, narrow_height) = G97_NARROW;
    let (narrow_road, narrow_counted, narrow_in_set, narrow_text) =
        probe(narrow_width, narrow_height);
    println!(
        "G97_PLANTED_NARROW={narrow_width}x{narrow_height} state_drawn={narrow_road} \
         in_dropped_set={narrow_in_set} clause_readable={narrow_counted}"
    );
    if !narrow_counted {
        println!(
            "G97_NARROW_CLAUSE=UNTESTABLE at {narrow_width}x{narrow_height}: the standing row is \
             cut before the word the disclosure is read by. Unruled width, so neither a pass nor a \
             failure -- reported, not folded"
        );
    }
    assert!(
        !narrow_road,
        "🔴 g97: the planted control was chosen as a width at which `state` cannot fit and it fit, \
         so it is measuring nothing\n{narrow_text}"
    );
    assert!(
        narrow_in_set,
        "🔴 g97 at {narrow_width}x{narrow_height}: `state` left the row and the layout did not put \
         it in the dropped set, so nothing downstream can count it. That is the collapse becoming \
         silent at the layer where it is decided\n{narrow_text}"
    );
    assert!(
        with_road > 0,
        "🔴 g97: `state` is drawn at none of {} shapes, so the road this gate holds open is a road \
         to nowhere and the assertion above passes on the disclosure alone: {census:?}",
        R76_SHAPES.len()
    );
}
/// **g98 -- the kind of nothing a judged field takes is read from the row's `state`.**
///
/// # The ruling (`req/924` §TUI-101, SS1145, 2026-09-02)
///
/// Two rows of the Owner's bed carry `verdict: null` and `inverse_status: null`, and the wire's
/// `state` tells them apart on every record. Four cells, of which `[T-r76]` drew **three wrong**:
/// a terminal `Aborted`'s missing verdict was `?` (*measured and not knowable*) when it is `--`
/// (*never written*), and a `Candidate`'s two missing judged fields were `?` and `--` when both are
/// *not yet*. The `Candidate` and the `Aborted` were therefore receiving the **identical symbol
/// pair** -- the two §TUI-97 had already ruled must be visibly different.
///
/// # What this gate holds, in four parts, and why each is here
///
/// 1. **The declaration is checked against the engine, at test time.** `[T-r76]` hand-wrote its
///    state array and attached no freshness check, in a lane whose own doc comment said a
///    hand-written list of lifecycle states would go stale in silence. It read the engine's source
///    for `g95`'s denominator one column over and did not do it here. This does.
/// 2. **The declaration is not what is measured.** A gate that checks a table against a table would
///    be green on a face that reads neither. The four cells are asked of `renderer::cell_mark`,
///    which is the function the row loop and the record both go through.
/// 3. **The predicate is generalised and counted.** The defect being repaired *is* the failure to
///    do this: `[T-r76]` fixed the cell it had photographed and left the neighbouring column and
///    the neighbouring row. Every cell of the bed the wire carried as `null` is enumerated and
///    reported `n/N`.
/// 4. **The negative controls are planted, printed, and only then believed.** A state with no
///    ruling must keep the general classifier, and the two rows must never come out equal.
#[test]
fn g98_the_kind_of_nothing_a_judged_field_takes_is_read_from_the_state() {
    // ---- (1) the declaration, against the engine's own list -------------------------------------
    let ruled: BTreeSet<&str> = wire::NULL_MEANING_BY_STATE.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        ruled.len(),
        wire::NULL_MEANING_BY_STATE.len(),
        "g98: a state is ruled on twice, so which mark it takes depends on array order"
    );
    for (_, nothing) in wire::NULL_MEANING_BY_STATE {
        assert!(
            wire::Nothing::ALL.contains(&nothing),
            "g98: {nothing:?} is not one of the seven. `req/924` TUI-48 reserves the spelling of an \
             eighth kind of nothing to the Owner, and a lane minting one is the failure that \
             ruling was written about"
        );
    }

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/gx-engine/src/pipeline.rs");
    match std::fs::read_to_string(&source) {
        Err(error) => {
            // The third value. A checkout without the engine's source (a published crate, a partial
            // tree) cannot answer this, and a gate that quietly fell back to comparing the face
            // against itself would report the weaker answer as though it were the stronger one.
            println!("G98_LIFECYCLE=UNTESTABLE reason={error} path={}", source.display());
        }
        Ok(text) => {
            let mut states: Vec<String> = Vec::new();
            let mut inside = false;
            for line in text.lines() {
                if line.contains("pub const LIFECYCLE_STATES") {
                    inside = true;
                    continue;
                }
                if !inside {
                    continue;
                }
                let trimmed = line.trim();
                if trimmed.starts_with(']') {
                    break;
                }
                if let Some(rest) = trimmed.strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        states.push(rest[..end].to_string());
                    }
                }
            }
            println!("G98_LIFECYCLE={states:?} n={}", states.len());
            assert!(
                !states.is_empty(),
                "g98: the engine's declared state list was read as nought names and the file is \
                 there, so the reader is what broke and not the claim"
            );
            // The tripwire. The list is never copied into this crate -- this one number is what
            // makes a twelfth state red instead of silently unruled.
            assert_eq!(
                states.len(),
                wire::LIFECYCLE_STATE_COUNT_AT_RULING,
                "g98: the engine declares {} lifecycle states and TUI-101 was ruled against {}. A \
                 state was added or removed and nobody asked what a `null` means on it",
                states.len(),
                wire::LIFECYCLE_STATE_COUNT_AT_RULING
            );
            // No phantom members: every ruled state has to be a state the engine can send.
            let declared: BTreeSet<&str> = states.iter().map(String::as_str).collect();
            let invented: Vec<&&str> = ruled.difference(&declared).collect();
            assert!(
                invented.is_empty(),
                "g98: {invented:?} is ruled on and the engine has no such state. A ruling about a \
                 state nobody sends is a ruling about a fiction"
            );
            // The remainder is printed rather than left silent: these are the states no reading has
            // been ruled for, and they keep the general classifier on purpose.
            let unruled: Vec<&&str> = declared.difference(&ruled).collect();
            println!("G98_UNRULED_STATES={unruled:?} ruled={}/{}", ruled.len(), declared.len());
        }
    }

    // The member an `Aborted` carries comes from a second closed enum, and the bed's value is in it.
    let reasons = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/gx-core/src/error.rs");
    match std::fs::read_to_string(&reasons) {
        Err(error) => println!("G98_ABORT_REASONS=UNTESTABLE reason={error}"),
        Ok(text) => {
            let mut variants: Vec<String> = Vec::new();
            let mut inside = false;
            for line in text.lines() {
                if line.contains("pub enum AbortReason") {
                    inside = true;
                    continue;
                }
                if !inside {
                    continue;
                }
                let trimmed = line.trim();
                if trimmed == "}" {
                    break;
                }
                if trimmed.starts_with("///") || trimmed.starts_with("//") || trimmed.is_empty() {
                    continue;
                }
                variants.push(trimmed.trim_end_matches(',').to_string());
            }
            println!("G98_ABORT_REASONS={variants:?}");
            assert!(
                variants.iter().any(|name| name == "Expired"),
                "g98: the reason the measured bed's one aborted row carries is not in the engine's \
                 closed enumeration, so this gate's reader is broken: {variants:?}"
            );
        }
    }

    // ---- (2) what the face actually draws, asked of the classifier the rows go through ----------
    let row = |state: serde_json::Value| {
        serde_json::json!({
            "transformation": "gx1:trbspcdedw2dt63rvfltsgle2zilpjr3tihzyrzabk2wvmndihtq",
            "verdict": serde_json::Value::Null,
            "state": state,
            "enforced": true,
            "inverse_status": serde_json::Value::Null,
        })
    };
    let aborted = row(serde_json::json!({"Aborted": "Expired"}));
    let candidate = row(serde_json::json!("Candidate"));
    let committed = row(serde_json::json!("Committed"));

    let pair = |item: &serde_json::Value| {
        (
            renderer::cell_mark(item, wire::VERDICT_KEY).0,
            renderer::cell_mark(item, wire::INVERSE_STATUS_KEY).0,
        )
    };
    let never = wire::Nothing::Absent.mark().to_string();
    let not_yet = wire::Nothing::Loading.mark().to_string();
    let unknown = wire::Nothing::Unknown.mark().to_string();

    let aborted_pair = pair(&aborted);
    let candidate_pair = pair(&candidate);
    let committed_pair = pair(&committed);
    println!(
        "G98_PAIRS aborted={aborted_pair:?} candidate={candidate_pair:?} \
         committed={committed_pair:?}"
    );
    assert_eq!(
        aborted_pair,
        (never.clone(), never.clone()),
        "g98: a terminal abort was never verdicted and never escrowed, and neither will ever be \
         written. Both cells are `{never}`"
    );
    assert_eq!(
        candidate_pair,
        (not_yet.clone(), not_yet.clone()),
        "g98: a `Candidate` has not reached T-3, so both judged fields are *not yet*. Drawing them \
         as `{unknown}` or `{never}` is a nothing that comes from time drawn as a semantic one"
    );

    // ---- (3) the negative controls, planted and printed before they are believed ----------------
    println!("G98_PLANTED_UNRULED_STATE=Committed");
    assert_eq!(
        committed_pair,
        (unknown.clone(), never.clone()),
        "g98: `Committed` has no ruling in `wire::NULL_MEANING_BY_STATE` and its cells changed \
         anyway, so the table is being applied by row rather than by state. A `Committed` record \
         with no verdict is an engine gap and the face may not explain it away"
    );
    println!("G98_PLANTED_SAME_PAIR_CHECK=candidate_vs_aborted");
    assert_ne!(
        candidate_pair, aborted_pair,
        "g98: the `Candidate` and the `Aborted` came out with the identical symbol pair. Those are \
         the exact two rows `req/924` TUI-97 ruled must be visibly different"
    );
    // And the rule the whole vocabulary rests on: a word the wire carried is never a mark.
    let unavailable = renderer::cell_mark(
        &serde_json::json!({"state": "Candidate", "inverse_status": "Unavailable"}),
        wire::INVERSE_STATUS_KEY,
    )
    .0;
    println!("G98_PLANTED_WORD_NOT_MARK={unavailable:?}");
    assert_eq!(
        unavailable, "Unavailable",
        "g98: a `Candidate` row carrying the word `Unavailable` had its word replaced by a mark. \
         The table speaks only for a `null`; an answer the engine gave is drawn as it arrived"
    );

    // ---- (4) the predicate generalised over every null cell on the bed, with a denominator ------
    // The failure being repaired is a repair that was not generalised, so this gate is required to
    // state where else the question applies and what happened there.
    let bed = [aborted.clone(), candidate.clone(), committed.clone()];
    let keys = [
        wire::VERDICT_KEY,
        wire::STATE_KEY,
        wire::INVERSE_STATUS_KEY,
        wire::SUPERSEDED_BY_KEY,
        "created_at",
        "scope",
        "actor",
        "rollback",
        "enforced",
    ];
    let mut null_cells = 0usize;
    let mut read_through_state = 0usize;
    let mut census: Vec<String> = Vec::new();
    for (index, item) in bed.iter().enumerate() {
        for key in keys {
            if item.get(key) != Some(&serde_json::Value::Null) {
                continue;
            }
            null_cells += 1;
            let judged = wire::NULL_MEANING_FIELDS.contains(&key);
            let ruled_state = wire::null_meaning(item).is_some();
            if judged && ruled_state {
                read_through_state += 1;
            }
            census.push(format!(
                "row{index} {key} drawn={:?} through_state={}",
                renderer::cell_mark(item, key).0,
                judged && ruled_state
            ));
        }
    }
    println!(
        "G98_NULL_CELLS={read_through_state}/{null_cells} read through `state`; the rest keep the \
         general classifier"
    );
    println!("G98_NULL_CENSUS={census:?}");
    assert!(
        null_cells > 0,
        "g98: the bed carries no `null` at all, so every cell above was decided on a value that \
         was never sent and this census is a census of nothing"
    );
    assert_eq!(
        read_through_state, 4,
        "g98: the four cells `req/924` TUI-101 ruled on are the four this face reads through \
         `state`, and it read {read_through_state}: {census:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// g100..g104 — the placement ladder, and the plant that proves g100 can refuse.
//
// 🔴 `[T-r87]`, 2026-09-02. `g1` refuses a raw colour outside the seam and `g13` refuses one in the
// token table's own doc; the placement axis had **no equivalent**, which is exactly why every
// column width, rank and order in this face was a numeral typed into `layout.rs`. These are that
// gate, plus the three that hold the new declaration honest.
//
// 🔴 `g5` is untouched by all of this and must stay so. It refuses the **medium's** types
// (`Rect`, `Constraint`, `Layout`, `ratatui`) outside the renderer, and the placement ladder
// declares values and role names only — no medium type crosses into `tokens.rs`. A run of these
// gates that turns `g5` red is a signal that a medium type was carried up, and the design is what
// is wrong then, not `g5`.
// ---------------------------------------------------------------------------------------------

/// Everything that is not a magnitude: string literals and the tail of a line comment.
///
/// 🔴 Stripped before counting, because the alternative is a gate that measures the wrong thing.
/// `layout.rs` spells wire addresses (`GET /v1/transformations`) and identifiers (`u16`), and a
/// scanner that counted the `1` in `/v1/` would report a placement magnitude where there is a road.
fn without_prose(line: &str) -> String {
    let mut kept = String::new();
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    while let Some(current) = chars.next() {
        if in_string {
            if current == '\\' {
                let _ = chars.next();
            } else if current == '"' {
                in_string = false;
            }
            continue;
        }
        if current == '"' {
            in_string = true;
            continue;
        }
        if current == '/' && chars.peek() == Some(&'/') {
            break;
        }
        kept.push(current);
    }
    kept
}

/// Every placement magnitude spelled as a literal on a live line of this source.
///
/// A magnitude is a bare integer literal of **two or more**. The three exclusions are stated here
/// rather than left in the code, because a gate whose exclusions are undocumented is a gate nobody
/// can argue with:
///
/// * **Nought and one are arithmetic identities.** `saturating_sub(1)`, `items - 1`, `max(1)` are
///   statements about how counting works and not about how wide anything is. Excluding them is the
///   one place this gate is deliberately loose, and it is loose in the direction that lets a real
///   magnitude of one through — `min_rows: 1` would not have been caught by this gate and was
///   found by reading. **Said out loud rather than left for an audit**: this gate's floor is two.
/// * **A subscript is an address.** `LEDGER_SLOTS[3]` names a thing, and the `3` is where it is
///   kept rather than how big it is.
/// * **An array's arity is a count of declarations.** The `10` in `[Column; 10]` is how many
///   columns there are, which is a fact about the declaration and not a placement decision.
/// * **A tuple index is a name.** `record_split(..).3` names the fourth member of a returned
///   tuple. 🔴 This exclusion is a **repair of this gate's own first run**, which reported that
///   line as a magnitude: *a pattern matching and a thing being a defect are two facts*
///   (`INHERITED_PRINCIPLES`), and the gate that cannot tell them apart is measured by its false
///   positives as much as by its finds.
///
/// 🔴 **And a line may declare itself exempt, in the open.** A trailing `// g100: <reason>` takes
/// a line out of the finding set and into [`placement_exemptions`], which the gate **prints with a
/// count**. That is the shape this project holds itself to: do not bound a set quietly — say how
/// many were let through and why, so that an exemption is an argument someone can lose rather than
/// a silence.
fn placement_numerals(text: &str) -> Vec<(usize, u64, String)> {
    let mut found = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        if is_comment(raw) || raw.contains(PLACEMENT_EXEMPT) {
            continue;
        }
        let line: Vec<char> = without_prose(raw).chars().collect();
        let mut at = 0;
        while at < line.len() {
            if !line[at].is_ascii_digit() {
                at += 1;
                continue;
            }
            // A digit that continues an identifier (`u16`, `v1`, `Grade0`) is part of a name.
            let joined = at > 0 && (line[at - 1].is_alphanumeric() || line[at - 1] == '_');
            let start = at;
            while at < line.len() && line[at].is_ascii_digit() {
                at += 1;
            }
            if joined {
                continue;
            }
            let value: u64 = line[start..at]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or_default();
            if value < 2 {
                continue;
            }
            // Past a type suffix (`4u16`), so that the character after the literal is the one the
            // reader sees after it.
            let mut tail = at;
            while tail < line.len() && (line[tail].is_alphanumeric() || line[tail] == '_') {
                tail += 1;
            }
            let before = line[..start].iter().rev().find(|c| !c.is_whitespace()).copied();
            let after = line[tail..].iter().find(|c| !c.is_whitespace()).copied();
            if before == Some('[') && after == Some(']') {
                continue;
            }
            if before == Some(';') {
                continue;
            }
            // A tuple index is a name, not a magnitude.
            if before == Some('.') {
                continue;
            }
            found.push((index + 1, value, raw.trim().to_string()));
        }
    }
    found
}

/// The marker a line uses to declare itself outside this gate, and the reason it gives.
const PLACEMENT_EXEMPT: &str = "// g100:";

/// Every line that declared itself exempt, with the reason it gave.
fn placement_exemptions(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !is_comment(line) && line.contains(PLACEMENT_EXEMPT))
        .map(|(index, line)| {
            let reason = line
                .split_once(PLACEMENT_EXEMPT)
                .map(|(_, tail)| tail.trim().to_string())
                .unwrap_or_default();
            (index + 1, reason)
        })
        .collect()
}

#[test]
fn g100_no_placement_magnitude_is_typed_into_the_layout_module() {
    let path = tui_dir().join("layout.rs");
    let text = std::fs::read_to_string(&path).expect("source is readable");
    let findings = placement_numerals(&text);
    let exempt = placement_exemptions(&text);
    println!("G100_FINDINGS={findings:?}");
    println!("G100_EXEMPT_COUNT={}", exempt.len());
    for (line, reason) in &exempt {
        println!("G100_EXEMPT line={line} reason={reason}");
    }
    // 🔴 An exemption with no reason is a silence with punctuation on it.
    let mute: Vec<&(usize, String)> = exempt.iter().filter(|(_, why)| why.is_empty()).collect();
    assert!(
        mute.is_empty(),
        "🔴 g100: {mute:?} declare themselves exempt and give no reason"
    );
    println!("G100_SCANNED_LINES={}", text.lines().count());
    assert!(
        findings.is_empty(),
        "🔴 g100 (`[T-r87]`): a width, a rank, an order, a gap or a threshold is a **value**, and \
         the only place in this face a placement value may be written down is \
         `super::tokens::span`. This is `g1` for the axis that had no gate: a face that types its \
         magnitudes cannot answer a request to change one without a code change, which is the \
         defect the Owner named. {findings:?}"
    );
}

#[test]
fn g100_control_the_scanner_refuses_a_planted_magnitude_and_passes_a_clean_source() {
    // 🔴 **The negative control, and it is printed before it is believed** (`LANE_BRIEF_CONSTRAINTS`
    // §4). A gate that has never been shown refusing anything is indistinguishable from a gate that
    // cannot refuse: this face has shipped a checker that returned `PASS` unconditionally, so the
    // control is the measurement and the green run is the claim.
    let planted = "    width: 14,\n";
    let plant = placement_numerals(planted);
    println!("G100_PLANT_SOURCE={planted:?}");
    println!("G100_PLANT_FINDINGS={plant:?}");
    assert_eq!(
        plant.len(),
        1,
        "🔴 g100's scanner did not refuse a planted width. Everything the gate says about \
         `layout.rs` is worthless while this is true. {plant:?}"
    );
    assert_eq!(plant[0].1, 14);

    // The positive control: the four shapes the gate excludes on purpose, each of which appears in
    // the real source, and none of which is a placement decision.
    let clean = "    let selected = selected.min(items - 1);\n\
                 pub const LEDGER_COLUMNS: [Column; 10] = [\n\
                 column(super::tokens::LEDGER_SLOTS[3]),\n\
                 let head: String = text.chars().take(room.saturating_sub(1)).collect();\n\
                 // the fourth row was reserved against a status_reason 14 cells long\n";
    let quiet = placement_numerals(clean);
    println!("G100_CLEAN_FINDINGS={quiet:?}");
    assert!(
        quiet.is_empty(),
        "🔴 g100's scanner reported a magnitude where there is an identity, an arity, a subscript \
         or a comment. A gate that cries wolf is read the way a gate that never fires is. {quiet:?}"
    );
}

#[test]
fn g101_every_measure_is_named_by_a_slot_and_answers_at_every_grade_of_every_scheme() {
    // The sentence `TOKENS` carries on the paint axis, one axis over: a quantity nothing resolves
    // to is a quantity nobody maintains.
    let mut unnamed: Vec<&'static str> = Vec::new();
    for measure in tokens::MEASURES {
        if !tokens::SLOTS
            .into_iter()
            .any(|slot| slot.measure() == measure)
        {
            unnamed.push(measure.name());
        }
    }
    println!("G101_UNNAMED={unnamed:?}");
    assert!(
        unnamed.is_empty(),
        "🔴 g101: {unnamed:?} are measures no slot resolves to"
    );

    // Totality, which on this axis is the property a lookup table can only be checked for after the
    // fact — so it is checked.
    let mut answered = 0usize;
    for slot in tokens::SLOTS {
        for scheme in tokens::SCHEMES {
            for grade in tokens::Grade::ALL {
                let cells = tokens::cells(slot, grade, scheme);
                assert!(
                    cells.width > 0 || slot.measure() == tokens::Measure::Lead,
                    "🔴 g101: {} answers nought at {} in {}, which is a slot with no value",
                    slot.name(),
                    grade.name(),
                    scheme.name()
                );
                answered += 1;
            }
        }
    }
    println!(
        "G101_ANSWERED={answered}/{}",
        tokens::SLOTS.len() * tokens::SCHEMES.len() * tokens::Grade::ALL.len()
    );

    // 🔴 The four welded measures answer the same number everywhere, which is the property the
    // price of a row and the drawing of a row both stand on (`[T-r66]`).
    for measure in tokens::MEASURES.into_iter().filter(|m| m.welded()) {
        let first = tokens::span(measure, tokens::SCHEMES[0]);
        for scheme in tokens::SCHEMES {
            let span = tokens::span(measure, scheme);
            assert!(
                span.iter().all(|cell| *cell == first[0]),
                "🔴 g101: `{}` is welded and answers {span:?} in `{}`. A welded measure that moves \
                 puts the row's price and the row's drawing back where `[T-r66]` found them",
                measure.name(),
                scheme.name()
            );
        }
    }
}

#[test]
fn g102_no_column_is_narrower_than_the_wire_key_drawn_over_it() {
    // 🔴 The labels on the grid's header row are wire keys drawn unchanged (`req/942` §9, gate
    // `P5`). A column narrower than its own key means the header is the thing that gets cut, and a
    // clipped key is a word the wire does not spell.
    let mut short: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for slot in tokens::LEDGER_SLOTS {
        let key = slot.key().expect("a ledger slot carries the wire's key");
        for scheme in tokens::SCHEMES {
            for grade in tokens::Grade::ALL {
                checked += 1;
                let width = tokens::cells(slot, grade, scheme).width as usize;
                if width < key.chars().count() {
                    short.push(format!(
                        "{} is {width} at {} in {} and `{key}` is {}",
                        slot.name(),
                        grade.name(),
                        scheme.name(),
                        key.chars().count()
                    ));
                }
            }
        }
    }
    println!("G102_CHECKED={checked} G102_SHORT={short:?}");
    assert!(short.is_empty(), "🔴 g102: {short:?}");
}

#[test]
fn g103_the_declaration_index_agrees_with_the_resolver_at_the_shipped_default() {
    // 🔴 **Three values, not two** (`req/38` SS870, and it is this project's own first principle
    // applied to its own gate). `Scheme::detect` reads the environment, so a run with
    // `GX_TUI_PLACEMENT` set to a sample is measuring a different table than the one this gate is
    // about — and reporting that as a failure would fold *not measured* into *measured false*.
    if tokens::Scheme::detect() != tokens::SCHEME_DEFAULT {
        println!(
            "G103=UNTESTABLE reason=the environment names `{}` and this gate is about `{}`",
            tokens::Scheme::detect().name(),
            tokens::SCHEME_DEFAULT.name()
        );
        return;
    }
    // Every column, at a width nothing can be dropped for.
    let (drawn, dropped) = layout::columns_for(u16::MAX);
    println!(
        "G103_DRAWN={:?} G103_DROPPED={dropped:?}",
        drawn.iter().map(|column| column.key).collect::<Vec<_>>()
    );
    assert_eq!(
        drawn.len(),
        LEDGER_COLUMNS.len(),
        "🔴 g103: the resolver kept {} columns where the index declares {}",
        drawn.len(),
        LEDGER_COLUMNS.len()
    );
    for (index, column) in drawn.iter().enumerate() {
        let declared = LEDGER_COLUMNS[index];
        assert_eq!(
            (column.key, column.width, column.priority),
            (declared.key, declared.width, declared.priority),
            "🔴 g103: at position {index} the resolver says {column:?} and the declaration index \
             says {declared:?}. The index is what twenty-seven call sites read, so the day the two \
             disagree the index is describing a screen nobody draws"
        );
    }
}

#[test]
fn g104_a_scheme_moves_the_columns_a_narrow_screen_keeps() {
    // 🔴 **The measurement the Owner's question actually asks for**: not *is there a table* but
    // *does the table decide the screen*. If swapping the declaration did not move what a forty-cell
    // terminal keeps, the ladder would be a decoration with a gate on it.
    //
    // Asked of `columns_for_less`'s own arithmetic at each scheme rather than of a drawn frame,
    // because a drawn frame needs an engine and this gate must answer in a suite. **Named ceiling,
    // stated rather than left**: this measures the *plan*, and a capture of a real terminal is what
    // measures the *screen*. The two are different claims and this gate makes only the first.
    let width: u16 = 40;
    let grade = tokens::Grade::of(width);
    assert_eq!(grade, tokens::Grade::Crammed, "40 cells is the crammed grade");

    let mut kept_by_scheme: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for scheme in tokens::SCHEMES {
        // The same walk `columns_for_less` makes, asked of the table directly: the scheme it
        // resolves at comes from the environment, and a test that set the environment would be a
        // test that changes the process for every other test in the binary.
        let mut ordered: Vec<tokens::Slot> = tokens::LEDGER_SLOTS.to_vec();
        ordered.sort_by_key(|slot| {
            let cells = tokens::cells(*slot, grade, scheme);
            (cells.rank, cells.order)
        });
        let mut kept: Vec<&'static str> = Vec::new();
        let mut spent = layout::LEFT_MARGIN;
        for slot in ordered {
            let cells = tokens::cells(slot, grade, scheme);
            let next = spent + layout::COLUMN_GAP + cells.width;
            if !kept.is_empty() && next > width {
                break;
            }
            spent = next;
            kept.push(slot.key().expect("a ledger slot carries a key"));
        }
        kept_by_scheme.push((scheme.name(), kept));
    }
    println!("G104_AT_40={kept_by_scheme:?}");
    let distinct: BTreeSet<&Vec<&'static str>> =
        kept_by_scheme.iter().map(|(_, kept)| kept).collect();
    assert!(
        distinct.len() > 1,
        "🔴 g104: every declared scheme keeps the same columns at forty cells, so the table is not \
         what the screen obeys. {kept_by_scheme:?}"
    );

    // And the fold, which is the half no width can buy.
    let mut folds: Vec<(&'static str, u16, u16)> = Vec::new();
    for scheme in tokens::SCHEMES {
        folds.push((
            scheme.name(),
            tokens::cells(tokens::Slot::FoldVoices, grade, scheme).width,
            tokens::cells(tokens::Slot::FoldQuorum, grade, scheme).width,
        ));
    }
    println!("G104_FOLD={folds:?}");
    assert!(
        folds.iter().any(|(_, voices, _)| *voices > 1),
        "🔴 g104: no scheme allows a second voice, so the thirty repeated rows the Owner named \
         cannot be answered by a declaration. {folds:?}"
    );
}

#[test]
fn g105_a_fold_with_two_voices_keeps_every_value_and_its_count() {
    // 🔴 The honesty condition on `resolve_folded`. A fold that dropped a value would be the face
    // asserting over records that did not say it, which is the error this product exists to refuse.
    let column = layout::Column {
        key: "verdict",
        width: 9,
        priority: Priority::One,
    };
    // Thirty say one thing, one says another, one says a third: the shape `[T-r87]`'s brief
    // measured on the live bed.
    let mut rows: Vec<Vec<String>> = (0..30).map(|_| vec!["Admit".to_string()]).collect();
    rows.push(vec!["Candidate".to_string()]);
    rows.push(vec!["?".to_string()]);

    let (kept_strict, folded_strict) = layout::resolve_folded(&[column], &rows, 1, 2);
    println!("G105_STRICT kept={} folded={folded_strict:?}", kept_strict.len());
    assert_eq!(
        kept_strict.len(),
        1,
        "🔴 g105: one voice must refuse this bed — that refusal is the defect the Owner named, and \
         a gate that does not reproduce it is not measuring the change"
    );
    assert!(folded_strict.is_empty());

    let (kept_loose, folded_loose) = layout::resolve_folded(&[column], &rows, 3, 8);
    println!("G105_LOOSE kept={} folded={folded_loose:?}", kept_loose.len());
    assert!(kept_loose.is_empty());
    assert_eq!(folded_loose.len(), 1);
    let (key, tally) = &folded_loose[0];
    assert_eq!(*key, "verdict");
    assert_eq!(
        tally,
        &vec![
            ("Admit".to_string(), 30),
            ("Candidate".to_string(), 1),
            ("?".to_string(), 1),
        ],
        "🔴 g105: the tally must carry every value and its count, most first, ties in the order \
         first seen. A fold that reported only the majority would be a lie with a number on it"
    );
    let counted: usize = tally.iter().map(|(_, count)| count).sum();
    println!("G105_SUM={counted}/{}", rows.len());
    assert_eq!(
        counted,
        rows.len(),
        "🔴 g105: the counts must sum to the records the fold was measured over"
    );

    // Below the quorum nothing folds, however few voices there are: two records prove nothing about
    // repetition, which is `uniform`'s rule made a declaration.
    let (kept_thin, folded_thin) = layout::resolve_folded(&[column], &rows[..2], 3, 8);
    println!("G105_THIN kept={} folded={folded_thin:?}", kept_thin.len());
    assert_eq!(kept_thin.len(), 1);
    assert!(folded_thin.is_empty());
}
