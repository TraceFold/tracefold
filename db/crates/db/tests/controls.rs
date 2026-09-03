// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_db");
static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Run {
    exit: i32,
    out: String,
}

impl Run {
    fn says(&self, needle: &str) -> bool {
        self.out.contains(needle)
    }
    fn pair(&self, needle: &str) -> String {
        format!("({}, {})", self.exit, if self.says(needle) { needle } else { "ABSENT" })
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("n10_4layer")
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create target directory");
    for entry in fs::read_dir(from).expect("read source directory") {
        let entry = entry.expect("read directory entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

fn fresh_db(label: &str) -> PathBuf {
    let serial = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("db_control_{}_{}_{}", std::process::id(), serial, label));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear the previous copy");
    }
    copy_tree(&fixture(), &dir);
    dir
}

fn run(db: &Path, args: &[&str]) -> Run {
    let mut command = Command::new(BIN);
    command.arg("--db").arg(db);
    for argument in args {
        command.arg(argument);
    }
    let output = command.output().expect("the db binary should run");
    let mut out = String::from_utf8_lossy(&output.stdout).to_string();
    out.push_str(&String::from_utf8_lossy(&output.stderr));
    let exit = match output.status.code() {
        Some(code) => code,
        None => -1,
    };
    Run { exit, out }
}

fn compiled(label: &str) -> PathBuf {
    let db = fresh_db(label);
    let first = run(&db, &["compile"]);
    assert_eq!(first.exit, 0, "compile should answer on the fixture: {}", first.out);
    db
}

fn admitted(label: &str) -> PathBuf {
    let db = compiled(label);
    let pushed = run(&db, &["push"]);
    assert_eq!(pushed.exit, 0, "push should answer: {}", pushed.out);
    db
}

fn digest_line(text: &str, key: &str) -> String {
    for line in text.split('\n') {
        if line.starts_with(key) {
            return line.trim().to_string();
        }
    }
    panic!("no line starting with {} in:\n{}", key, text);
}

fn mutate(path: &Path, edit: impl Fn(String) -> String) {
    let before = fs::read_to_string(path).expect("read the file about to be mutated");
    let after = edit(before.clone());
    assert_ne!(
        before, after,
        "the mutation changed nothing, so the control would have measured the unmutated file"
    );
    fs::write(path, after).expect("write the mutated file");
}

#[test]
fn compile_is_deterministic_and_the_index_is_regenerable() {
    let db = compiled("determinism");
    let first = run(&db, &["compile"]);
    let source_one = digest_line(&first.out, "source_digest");
    let table_one = digest_line(&first.out, "table_digest");

    let second = run(&db, &["compile"]);
    assert_eq!(source_one, digest_line(&second.out, "source_digest"));
    assert_eq!(table_one, digest_line(&second.out, "table_digest"));

    let raw_one = digest_line(&first.out, "raw_digest");
    assert_eq!(raw_one, digest_line(&second.out, "raw_digest"));

    let index = db.join("build").join("index").join("semantic.sqlite");
    let bytes_before = fs::read(&index).expect("the index should exist after compile");
    let build = db.join("build");
    fs::remove_dir_all(&build).expect("delete everything regenerable");
    assert!(!build.exists());

    let third = run(&db, &["compile"]);
    assert_eq!(third.exit, 0);
    assert_eq!(source_one, digest_line(&third.out, "source_digest"));
    assert_eq!(raw_one, digest_line(&third.out, "raw_digest"));
    assert_eq!(
        table_one,
        digest_line(&third.out, "table_digest"),
        "deleting build/ and compiling again must return the same tables"
    );
    let bytes_after = fs::read(&index).expect("the index should exist again");
    assert_eq!(
        bytes_before.len(),
        bytes_after.len(),
        "a regenerated index should be the same size as the one it replaced"
    );
}

#[test]
fn the_raw_tier_holds_the_source_bytes_under_their_own_digest() {
    let db = compiled("raw");
    let listing = fs::read_to_string(db.join("build").join("raw").join("INDEX"))
        .expect("the raw tier should carry a listing");
    let rows: Vec<&str> = listing.trim_end_matches('\n').split('\n').collect();
    assert_eq!(rows.len(), 2, "one row per declared document: {}", listing);

    for row in &rows {
        let (digest, address) = row.split_once(' ').expect("digest and address");
        let (band, path) = address.split_once('/').expect("band and path");
        let copy = db.join("build").join("raw").join(&digest[..2]).join(digest);
        let original = db.join("bands").join(band).join(path);
        assert_eq!(
            fs::read(&copy).expect("the addressed copy"),
            fs::read(&original).expect("the source"),
            "the raw tier is a copy of the source bytes, not a rendering of them"
        );
    }
}

#[test]
fn gate_is_untestable_before_the_journal_exists_and_passes_after() {
    let db = compiled("journal");
    let before = run(&db, &["gate"]);
    assert_eq!(
        (before.exit, before.says("JOURNAL_ABSENT")),
        (2, true),
        "a chain over zero events is UNKNOWN, never a pass: {}",
        before.out
    );
    assert!(before.says("unknown"), "the third value must be printed by name");

    let pushed = run(&db, &["push"]);
    assert_eq!(pushed.exit, 0, "{}", pushed.out);

    let after = run(&db, &["gate"]);
    assert_eq!((after.exit, after.says("0 fail, 0 UNKNOWN")), (0, true), "{}", after.out);
}

#[test]
fn g_s2_sees_a_document_no_manifest_claims() {
    let db = admitted("orphan");
    let clean = run(&db, &["gate"]);
    assert_eq!(clean.exit, 0, "positive control: {}", clean.out);

    fs::write(db.join("bands").join("arch").join("stray.md"), "# Stray\n")
        .expect("write the orphan");
    let dirty = run(&db, &["gate"]);
    assert_eq!(
        (dirty.exit, dirty.pair("ORPHAN_MD")),
        (1, "(1, ORPHAN_MD)".to_string()),
        "{}",
        dirty.out
    );
}

#[test]
fn g_s5_covers_the_last_journal_record() {
    let db = admitted("chain");
    let journal = db.join("journal").join("semantic.journal.jsonl");
    let text = fs::read_to_string(&journal).expect("read the journal");
    let lines: Vec<&str> = text.trim_end_matches('\n').split('\n').collect();
    assert!(lines.len() > 1, "the control needs more than one event");

    mutate(&journal, |body| body.replace("\"executor\":\"lane\"", "\"executor\":\"owner\""));
    let tampered = run(&db, &["gate"]);
    assert_eq!(
        (tampered.exit, tampered.says("HEAD_MISMATCH") || tampered.says("CHAIN_BREAK")),
        (1, true),
        "editing any record, including the last, must break the fold: {}",
        tampered.out
    );
}

#[test]
fn g_i1_recomputes_the_digest_instead_of_asking_the_index() {
    let db = admitted("digest");
    let document = db.join("bands").join("arch").join("01_overview.md");
    mutate(&document, |body| body.replace("two bands", "TWO bands"));

    let stale = run(&db, &["gate"]);
    assert_eq!(
        (stale.exit, stale.pair("DIGEST_MISMATCH")),
        (1, "(1, DIGEST_MISMATCH)".to_string()),
        "{}",
        stale.out
    );

    let rebuilt = run(&db, &["push"]);
    assert_eq!(rebuilt.exit, 0, "{}", rebuilt.out);
    let fresh = run(&db, &["gate"]);
    assert_eq!(fresh.exit, 0, "after push the digest agrees again: {}", fresh.out);
}

#[test]
fn a_cap_below_one_is_refused_and_the_manifest_is_left_alone() {
    let db = compiled("cap");
    let manifest = db.join("db.toml");
    let before = fs::read(&manifest).expect("read the manifest");
    mutate(&manifest, |body| body.replace("L0 = 100", "L0 = 0"));
    let mutated = fs::read(&manifest).expect("read the mutated manifest");

    let refused = run(&db, &["ls"]);
    assert_eq!(refused.exit, 2, "{}", refused.out);
    assert!(refused.says("cap below 1"), "{}", refused.out);

    let after = fs::read(&manifest).expect("read the manifest again");
    assert_eq!(mutated, after, "a refused manifest is never rewritten");
    assert_ne!(before, after, "the control did mutate the file it claims to protect");
}

#[test]
fn a_manifest_that_is_not_toml_is_refused_rather_than_replaced() {
    let db = compiled("prose");
    let manifest = db.join("db.toml");
    mutate(&manifest, |_| "this file is prose, not a manifest\n".to_string());
    let refused = run(&db, &["gate"]);
    assert_eq!(refused.exit, 2, "{}", refused.out);
    let after = fs::read_to_string(&manifest).expect("read the manifest again");
    assert_eq!(after, "this file is prose, not a manifest\n");
}

#[test]
fn ls_binds_an_exit_and_a_reason_to_each_control() {
    let db = compiled("ls");
    let controls: Vec<(&str, Vec<&str>, i32, &str)> = vec![
        ("negative", vec!["ls", "--layer", "L9"], 2, "UNKNOWN_FILTER_VALUE"),
        ("negative", vec!["ls", "--band", "nowhere"], 2, "UNKNOWN_FILTER_VALUE"),
        ("negative", vec!["ls", "--role", "gossip"], 2, "UNKNOWN_FILTER_VALUE"),
        ("vacuous", vec!["ls", "--band", "arch", "--layer", "L2"], 2, "EMPTY"),
        ("positive", vec!["ls", "--band", "evidence", "--layer", "L2"], 0, "rows"),
    ];
    for (kind, args, exit, needle) in controls {
        let outcome = run(&db, &args);
        assert_eq!(
            (outcome.exit, outcome.says(needle)),
            (exit, true),
            "{} control {:?} expected ({}, {}): {}",
            kind,
            args,
            exit,
            needle,
            outcome.out
        );
    }
}

#[test]
fn a_cursor_is_the_whole_id_and_never_a_prefix() {
    let db = compiled("cursor");
    let listed = run(&db, &["ls", "--band", "evidence", "--layer", "L2"]);
    assert_eq!(listed.exit, 0, "{}", listed.out);

    let first = listed
        .out
        .split('\n')
        .nth(1)
        .expect("a row line")
        .split_whitespace()
        .next()
        .expect("a short id")
        .to_string();
    assert_eq!(first.len(), 12, "the headline prints a twelve character short id");

    let prefix = run(&db, &["ls", "--band", "evidence", "--layer", "L2", "--cursor", &first]);
    assert_eq!(
        (prefix.exit, prefix.pair("BAD_CURSOR")),
        (2, "(2, BAD_CURSOR)".to_string()),
        "a short id is a prefix of the row id, and a prefix is refused: {}",
        prefix.out
    );

    let begin = run(&db, &["ls", "--band", "evidence", "--layer", "L2", "--cursor", "begin"]);
    assert_eq!(begin.exit, 0, "begin is the one word that is not an id: {}", begin.out);
}

#[test]
fn find_returns_addresses_and_refuses_an_empty_result() {
    let db = compiled("find");
    let hit = run(&db, &["find", "regenerable"]);
    assert_eq!((hit.exit, hit.says("hit(s)")), (0, true), "{}", hit.out);
    assert!(
        !hit.says("derived from the source and the journal"),
        "find returns the address and one line, never the body: {}",
        hit.out
    );

    let miss = run(&db, &["find", "chartreuse"]);
    assert_eq!((miss.exit, miss.pair("EMPTY")), (2, "(2, EMPTY)".to_string()), "{}", miss.out);

    let nothing = run(&db, &["find", "   "]);
    assert_eq!(nothing.exit, 2, "an empty needle asks nothing: {}", nothing.out);
}

#[test]
fn show_refuses_a_prefix_and_answers_an_exact_address() {
    let db = compiled("show");
    let exact = run(&db, &["show", "Overview"]);
    assert_eq!((exact.exit, exact.says("Overview")), (0, true), "{}", exact.out);

    let missing = run(&db, &["show", "not-an-anchor"]);
    assert_eq!(
        (missing.exit, missing.pair("EMPTY")),
        (2, "(2, EMPTY)".to_string()),
        "{}",
        missing.out
    );
}

#[test]
fn an_empty_corpus_is_untestable_and_never_a_pass() {
    let db = fresh_db("vacuous");
    fs::write(
        db.join("bands").join("arch").join("01_overview.md"),
        "",
    )
    .expect("empty the document");
    fs::write(db.join("bands").join("evidence").join("run.md"), "").expect("empty the document");
    let outcome = run(&db, &["compile"]);
    assert_eq!(
        (outcome.exit, outcome.says("UNTESTABLE")),
        (2, true),
        "zero atoms is UNTESTABLE, never a pass: {}",
        outcome.out
    );
}

#[test]
fn selftest_refuses_a_directory_with_nothing_to_read() {
    let empty = std::env::temp_dir().join(format!("db_control_empty_{}", std::process::id()));
    fs::create_dir_all(&empty).expect("create an empty directory");
    let outcome = Command::new(BIN)
        .arg("selftest")
        .arg("--path")
        .arg(&empty)
        .output()
        .expect("run selftest");
    let text = String::from_utf8_lossy(&outcome.stdout).to_string();
    assert_eq!(outcome.status.code(), Some(2), "{}", text);
    assert!(text.contains("NO_SOURCE_READ"), "{}", text);
}

#[test]
fn selftest_finds_a_comment_that_is_not_the_header() {
    let dir = std::env::temp_dir().join(format!("db_control_comment_{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create the directory");
    let clean = dir.join("clean.rs");
    fs::write(
        &clean,
        "// SPDX-License-Identifier: Apache-2.0\n// Copyright (c) 2026 Glovrex\n\npub fn one() -> usize {\n    1\n}\n",
    )
    .expect("write the clean file");
    let positive = Command::new(BIN)
        .arg("selftest")
        .arg("--path")
        .arg(&dir)
        .output()
        .expect("run selftest");
    assert_eq!(
        positive.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&positive.stdout)
    );

    fs::write(
        dir.join("dirty.rs"),
        "// SPDX-License-Identifier: Apache-2.0\n// Copyright (c) 2026 Glovrex\n\n// this line explains the code\npub fn two() -> usize {\n    2\n}\n",
    )
    .expect("write the dirty file");
    let negative = Command::new(BIN)
        .arg("selftest")
        .arg("--path")
        .arg(&dir)
        .output()
        .expect("run selftest");
    let text = String::from_utf8_lossy(&negative.stdout).to_string();
    assert_eq!(negative.status.code(), Some(1), "{}", text);
    assert!(text.contains("COMMENT_OUTSIDE_HEADER"), "{}", text);
}

#[test]
fn selftest_finds_a_call_that_turns_a_missing_value_into_a_default() {
    let dir = std::env::temp_dir().join(format!("db_control_collapse_{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create the directory");
    let stem = format!("unwrap{}or", "_");
    fs::write(
        dir.join("collapse.rs"),
        format!(
            "// SPDX-License-Identifier: Apache-2.0\n// Copyright (c) 2026 Glovrex\n\npub fn value(input: Option<usize>) -> usize {{\n    input.{}(0)\n}}\n",
            stem
        ),
    )
    .expect("write the collapsing file");
    let outcome = Command::new(BIN)
        .arg("selftest")
        .arg("--path")
        .arg(&dir)
        .output()
        .expect("run selftest");
    let text = String::from_utf8_lossy(&outcome.stdout).to_string();
    assert_eq!(outcome.status.code(), Some(1), "{}", text);
    assert!(text.contains("UNKNOWN_COLLAPSED_TO_DEFAULT"), "{}", text);
}

#[test]
fn the_shipped_source_carries_no_comment_and_no_collapse() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let outcome = Command::new(BIN)
        .arg("selftest")
        .arg("--path")
        .arg(&src)
        .output()
        .expect("run selftest");
    let text = String::from_utf8_lossy(&outcome.stdout).to_string();
    assert_eq!(outcome.status.code(), Some(0), "{}", text);
    assert!(text.contains("G-C1"), "{}", text);
    assert!(text.contains("G-C2"), "{}", text);
}

fn field(body: &str, key: &str) -> String {
    let needle = format!("\"{}\":", key);
    let at = body.find(&needle).expect(&format!("{} in {}", key, body));
    let rest = body[at + needle.len()..].trim_start();
    let end = rest
        .find(|c: char| c == ',' || c == '\n' || c == '}')
        .expect("the value should end");
    rest[..end].trim().trim_matches('"').to_string()
}

#[test]
fn the_json_wire_carries_the_three_values_and_its_own_denominator() {
    let db = compiled("wire");
    let answered = run(&db, &["ls", "--band", "evidence", "--layer", "L2", "--json"]);
    assert_eq!(answered.exit, 0, "{}", answered.out);
    assert_eq!(field(&answered.out, "schema"), "1");
    assert_eq!(field(&answered.out, "verdict"), "TRUE");
    assert_eq!(field(&answered.out, "reason"), "ANSWERED");
    assert!(answered.says("\"matched\""), "the wire carries the denominator: {}", answered.out);
    assert!(answered.says("\"unscanned\""), "{}", answered.out);
    assert!(answered.says("\"UNKNOWN\""), "an undeclared attribute reaches the wire as UNKNOWN, not as a default: {}", answered.out);

    let empty = run(&db, &["ls", "--band", "arch", "--layer", "L2", "--json"]);
    assert_eq!(
        (empty.exit, field(&empty.out, "verdict"), field(&empty.out, "reason")),
        (2, "FALSE".to_string(), "EMPTY".to_string()),
        "an empty answer is FALSE on the wire and still exit 2: {}",
        empty.out
    );

    let refused = run(&db, &["ls", "--layer", "L9", "--json"]);
    assert_eq!(
        (refused.exit, field(&refused.out, "verdict")),
        (2, "UNKNOWN".to_string()),
        "a question that could not be asked is UNKNOWN, never FALSE: {}",
        refused.out
    );
}

#[test]
fn the_wire_adds_fields_as_the_lod_deepens_and_never_removes_one() {
    let db = compiled("wire_lod");
    let zero = run(&db, &["show", "Overview", "--lod", "0", "--json"]);
    let one = run(&db, &["show", "Overview", "--lod", "1", "--json"]);
    let two = run(&db, &["show", "Overview", "--lod", "2", "--json"]);
    for outcome in [&zero, &one, &two] {
        assert_eq!(outcome.exit, 0, "{}", outcome.out);
        assert!(outcome.says("\"line\""), "every level names the atom: {}", outcome.out);
    }
    assert!(!zero.says("\"content\""), "lod 0 is the headline only: {}", zero.out);
    assert!(one.says("\"content\""), "lod 1 adds the body: {}", one.out);
    assert!(!one.says("\"provenance\""), "lod 1 stops short of provenance: {}", one.out);
    assert!(two.says("\"content\""), "lod 2 keeps everything lod 1 said: {}", two.out);
    assert!(two.says("\"byte_start\""), "lod 2 adds provenance: {}", two.out);
    assert!(two.says("\"relations\""), "lod 2 adds relations: {}", two.out);
}

fn http_get(port: u16, target: &str, method: &str) -> Option<(u16, String)> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let request = format!("{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n", method, target);
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let status: u16 = raw.split_whitespace().nth(1)?.parse().ok()?;
    let body = match raw.find("\r\n\r\n") {
        Some(at) => raw[at + 4..].to_string(),
        None => String::new(),
    };
    Some((status, body))
}

#[test]
fn serve_answers_the_same_bytes_as_stdout_and_writes_nothing() {
    let db = compiled("serve");
    let port: u16 = 20000 + (std::process::id() % 9000) as u16;
    let mut child = Command::new(BIN)
        .arg("--db")
        .arg(&db)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("serve should start");

    let mut ready = None;
    for _ in 0..80 {
        if let Some(found) = http_get(port, "/v1/ls?band=evidence&layer=L2", "GET") {
            ready = Some(found);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let listed = match ready {
        Some(found) => found,
        None => {
            let _ = child.kill();
            panic!("serve never answered on port {}; this is UNTESTABLE, not a pass", port);
        }
    };

    let stdout = run(&db, &["ls", "--band", "evidence", "--layer", "L2", "--json"]);
    assert_eq!(listed.0, 200, "{}", listed.1);
    assert_eq!(
        listed.1.trim(),
        stdout.out.trim(),
        "one serialiser: the http body and the stdout json are the same bytes, or the face would have to know which transport it is on"
    );

    let refused = http_get(port, "/v1/ls", "POST").expect("a POST should still get an answer");
    assert_eq!(refused.0, 405, "serve is read only: {}", refused.1);
    assert!(refused.1.contains("METHOD_NOT_READ_ONLY"), "{}", refused.1);

    let unknown = http_get(port, "/nope", "GET").expect("an unknown route should still answer");
    assert_eq!(unknown.0, 404, "{}", unknown.1);

    let empty = http_get(port, "/v1/ls?band=arch&layer=L2", "GET").expect("an empty projection answers");
    assert_eq!(empty.0, 422, "an empty answer is not a 200: {}", empty.1);
    assert!(empty.1.contains("\"verdict\": \"FALSE\""), "{}", empty.1);

    let taken = Command::new(BIN)
        .arg("--db")
        .arg(&db)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .output()
        .expect("a second serve should run and fail");
    assert_eq!(
        taken.status.code(),
        Some(2),
        "a port already bound is UNTESTABLE, never a server that returns no rows: {}",
        String::from_utf8_lossy(&taken.stderr)
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn dump_commands_is_machine_read_from_clap() {
    let outcome = Command::new(BIN)
        .arg("--dump-commands")
        .output()
        .expect("run dump-commands");
    let text = String::from_utf8_lossy(&outcome.stdout).to_string();
    assert_eq!(outcome.status.code(), Some(0), "{}", text);
    for needle in ["\"compile\"", "\"push\"", "\"gate\"", "\"ls\"", "\"show\"", "\"find\"", "\"selftest\"", "\"serve\"", "exit_codes"] {
        assert!(text.contains(needle), "the dump should carry {}: {}", needle, text);
    }
    assert!(
        !text.contains("\"name\": \"help\""),
        "clap's own help argument is not part of the contract: {}",
        text
    );
    assert!(
        text.contains("\"long\": \"--layer\""),
        "a flag the README documents must come from clap, not from prose: {}",
        text
    );
}
