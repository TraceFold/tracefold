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

/// Runs the binary with its working directory set to `cwd` and no `--db`,
/// so the upward search from `cwd` is what resolves the DB, exactly as a
/// caller sitting in a project subdirectory would invoke it.
fn run_in(cwd: &Path, args: &[&str]) -> Run {
    let mut command = Command::new(BIN);
    command.current_dir(cwd);
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

fn scratch_dir(label: &str) -> PathBuf {
    let serial = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("db_scratch_{}_{}_{}", std::process::id(), serial, label));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear the previous copy");
    }
    dir
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

fn full_id(db: &Path, args: &[&str]) -> String {
    let mut argv: Vec<&str> = args.to_vec();
    argv.push("--json");
    let wired = run(db, &argv);
    assert_eq!(wired.exit, 0, "the wire should answer so an id can be read from it: {}", wired.out);
    let at = wired.out.find("\"id\":").expect("a row carries an id");
    let rest = wired.out[at + 5..].trim_start();
    let id: String = rest.trim_start_matches('"').chars().take(64).collect();
    assert_eq!(id.len(), 64, "an atom id is the whole digest: {}", id);
    id
}

fn layer_at(line: &str) -> Option<usize> {
    let words: Vec<&str> = line.split_whitespace().collect();
    for index in [0usize, 1usize] {
        let layer = match words.get(index) {
            Some(word) => word,
            None => continue,
        };
        if !["L0", "L1", "L2", "UNKNOWN"].contains(layer) {
            continue;
        }
        if let Some(address) = words.get(index + 1) {
            if address.contains('/') && address.contains('#') {
                return Some(index);
            }
        }
    }
    None
}

fn row_lines(out: &str) -> Vec<String> {
    out.split('\n')
        .filter(|line| layer_at(line).is_some())
        .map(|line| line.to_string())
        .collect()
}

fn address_of(line: &str) -> String {
    let at = layer_at(line).expect("the line is a row");
    line.split_whitespace().nth(at + 1).expect("a row prints its address").to_string()
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

const LEGACY_LINE: &str = "{\"seq\":9001,\"id\":\"0f5c1a2b3c4d5e6f70819293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7\",\"executor\":\"tool\",\"prev_hash\":\"\",\"supersedes\":[],\"band\":\"arch\",\"kind\":\"spec\",\"text\":\"a record the previous engine wrote\",\"address\":[],\"body\":[],\"evidence\":{},\"gate\":{},\"source\":\"\",\"tags\":[]}";

fn settle(db: &Path) {
    mutate(&db.join("bands").join("evidence").join("run.md"), |body| {
        format!("{}\n[MEASURED] one more line, so that push has an atom to admit.\n", body.trim_end())
    });
    let pushed = run(db, &["push"]);
    assert_eq!(
        pushed.exit, 0,
        "push rewrites HEAD over every line it read, which is how a journal that holds an older record gets a HEAD that covers it: {}",
        pushed.out
    );
}

fn edit_last_line(path: &Path, edit: impl Fn(String) -> String) {
    let text = fs::read_to_string(path).expect("read the journal");
    let mut lines: Vec<String> = text
        .trim_end_matches('\n')
        .split('\n')
        .map(|line| line.to_string())
        .collect();
    let last = lines.pop().expect("the journal holds a last line");
    let changed = edit(last.clone());
    assert_ne!(last, changed, "the tamper changed nothing, so it would have measured an untampered journal");
    lines.push(changed);
    fs::write(path, format!("{}\n", lines.join("\n"))).expect("write the tampered journal");
}

fn flip_hex_after(line: &str, key: &str) -> String {
    let needle = format!("\"{}\":\"", key);
    let at = line.find(&needle).expect("the record carries that key") + needle.len();
    let mut bytes = line.as_bytes().to_vec();
    bytes[at] = if bytes[at] == b'0' { b'1' } else { b'0' };
    String::from_utf8(bytes).expect("a journal record is ascii json")
}

fn journal_lines(path: &Path) -> usize {
    fs::read_to_string(path)
        .expect("read the journal")
        .split('\n')
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn append_line(path: &Path, line: &str) {
    let mut text = fs::read_to_string(path).expect("read the journal");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(line);
    text.push('\n');
    fs::write(path, text).expect("write the journal");
}

#[test]
fn a_record_from_the_previous_engine_is_read_counted_and_never_mistaken_for_a_break() {
    let db = admitted("legacy");
    let journal = db.join("journal").join("semantic.journal.jsonl");
    let before = run(&db, &["gate"]);
    assert!(
        before.says("G-S5") && !before.says("CHAIN_NOT_RECOMPUTABLE"),
        "before the old record is added, nothing is unrecomputable: {}",
        before.out
    );

    append_line(&journal, LEGACY_LINE);
    let built = run(&db, &["compile"]);
    assert_eq!(
        built.exit, 0,
        "a record in the format this engine replaced does not stop compile: {}",
        built.out
    );
    assert!(
        built.says("1 record(s) in the format this engine replaced"),
        "the denominator counts the two shapes apart rather than reporting one number: {}",
        built.out
    );

    let interposed = run(&db, &["gate"]);
    assert_eq!(
        (interposed.exit, interposed.pair("HEAD_MISMATCH")),
        (1, "(1, HEAD_MISMATCH)".to_string()),
        "a line appended without rewriting HEAD is a line added after the fold was recorded, and that is a break, not an unknown: {}",
        interposed.out
    );

    settle(&db);
    let checked = run(&db, &["gate"]);
    assert_eq!(
        (checked.exit, checked.says("LEGACY_UNVERIFIABLE")),
        (2, true),
        "the previous engine folded its chain a different way, so that link is UNKNOWN, never a chain this engine found broken: {}",
        checked.out
    );
    assert!(
        !checked.says("CHAIN_BREAK") && !checked.says("HEAD_MISMATCH"),
        "an unverifiable link must not be reported as a break, and HEAD folds over the old line as it stands: {}",
        checked.out
    );
    assert!(
        checked.says("HEAD equals the fold of all"),
        "the UNKNOWN says what it did check, not only what it could not: {}",
        checked.out
    );

    append_line(&journal, "{\"seq\": 9002, \"this\": \"is not either shape\"}");
    let refused = run(&db, &["compile"]);
    assert_eq!(
        (refused.exit, refused.says("1 UNKNOWN")),
        (2, true),
        "a line of neither shape is counted as UNKNOWN and refuses; it is never silently dropped: {}",
        refused.out
    );
}

#[test]
fn the_two_journal_shapes_do_not_answer_to_each_other() {
    let db = admitted("shapes");
    let journal = db.join("journal").join("semantic.journal.jsonl");
    let text = fs::read_to_string(&journal).expect("read the journal");
    let current = text
        .split('\n')
        .find(|line| !line.trim().is_empty())
        .expect("push wrote at least one record")
        .to_string();
    assert!(
        current.contains("\"atom_id\"") && current.contains("\"lineage\""),
        "a record this engine wrote names atom_id and lineage: {}",
        current
    );
    assert!(
        !current.contains("\"id\":") && !LEGACY_LINE.contains("\"atom_id\""),
        "the two shapes are disjoint, so the reader cannot take one for the other: {} / {}",
        current,
        LEGACY_LINE
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

    for line in listed.out.split('\n').skip(1) {
        let word = match line.split_whitespace().next() {
            Some(word) => word,
            None => continue,
        };
        assert!(
            !(word.len() == 12 && word.chars().all(|c| c.is_ascii_hexdigit())),
            "no row prints a twelve character prefix of an id: every command that takes an id takes the whole one, so a token that looks like an id and is refused as one is never printed: {}",
            line
        );
    }
    let full = full_id(&db, &["ls", "--band", "evidence", "--layer", "L2"]);
    let first: String = full.chars().take(12).collect();

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
fn gap_atoms_are_excluded_by_default_counted_out_loud_and_shown_on_request() {
    let db = compiled("gaps");
    let plain = run(&db, &["ls", "--layer", "L2", "--cursor", "begin"]);
    assert_eq!(plain.exit, 0, "{}", plain.out);
    let with_gaps = run(&db, &["ls", "--layer", "L2", "--cursor", "begin", "--include-gaps"]);
    assert_eq!(with_gaps.exit, 0, "{}", with_gaps.out);

    let without = row_lines(&plain.out).len();
    let within = row_lines(&with_gaps.out).len();
    assert!(
        within > without,
        "--include-gaps must return strictly more rows, or the exclusion never happened: {} with gaps against {} without",
        within,
        without
    );
    assert!(
        plain.says("gaps_excluded: "),
        "the exclusion is stated on the denominator line, never made silently: {}",
        plain.out
    );
    assert!(
        !with_gaps.says("gaps_excluded: "),
        "nothing is excluded when they are asked for, so there is no count to state: {}",
        with_gaps.out
    );
    assert!(
        !plain.says("[gap]"),
        "a default projection carries no gap atom: {}",
        plain.out
    );
    assert!(
        with_gaps.says("[gap]"),
        "a gap atom names itself a gap rather than wearing the anchor of the heading above it: {}",
        with_gaps.out
    );

    let wired = run(&db, &["ls", "--layer", "L2", "--cursor", "begin", "--json"]);
    assert!(
        wired.says("\"gaps_excluded\""),
        "the wire carries the same count as the text, so a face can show it: {}",
        wired.out
    );

    let searched = run(&db, &["find", "milliseconds"]);
    assert_eq!(searched.exit, 0, "find still answers with gaps excluded: {}", searched.out);
}

#[test]
fn paging_walks_a_whole_projection_without_repeating_or_skipping_a_row() {
    let db = fresh_db("paging");
    mutate(&db.join("db.toml"), |text| {
        text.replace("L0 = 100", "L0 = 1").replace("L1 = 30", "L1 = 1").replace("L2 = 8", "L2 = 1")
    });
    let built = run(&db, &["compile"]);
    assert_eq!(built.exit, 0, "{}", built.out);
    let mut seen: Vec<String> = Vec::new();
    let mut cursor = String::from("begin");
    let mut declared = 0usize;
    let mut pages = 0usize;
    for _ in 0..64 {
        let page = run(&db, &["ls", "--cursor", &cursor]);
        assert_eq!(page.exit, 0, "a page of a non empty projection answers: {}", page.out);
        pages += 1;
        let header = page.out.split('\n').next().expect("a header line").to_string();
        declared = header
            .split(" of ")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|word| word.parse::<usize>().ok())
            .expect("the header carries the denominator");
        for line in row_lines(&page.out) {
            seen.push(address_of(&line));
        }
        let next = page
            .out
            .split('\n')
            .find(|line| line.starts_with("next: db ls --cursor "))
            .map(|line| line.trim_start_matches("next: db ls --cursor ").trim().to_string());
        match next {
            Some(found) => cursor = found,
            None => break,
        }
    }
    assert!(
        pages > 1 && declared > 1,
        "the cap was lowered so that this projection needs more than one page; {} page(s) over {} row(s) means the walk never paged and the control was never asked, which is UNTESTABLE rather than a pass",
        pages,
        declared
    );
    let mut distinct = seen.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        (seen.len(), distinct.len()),
        (declared, declared),
        "walking the cursor must emit every row of the projection exactly once; the sort key is total, so a page can neither repeat a row nor step over one"
    );
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

fn hits(out: &str) -> usize {
    out.split(": ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|word| word.parse::<usize>().ok())
        .expect("the first line counts the hits")
}

#[test]
fn find_narrows_in_sql_and_binds_an_exit_and_a_reason_to_each_control() {
    let db = compiled("find_filters");
    for (needle, home, away) in [
        ("regenerable", "arch", "evidence"),
        ("milliseconds", "evidence", "arch"),
    ] {
        let everywhere = run(&db, &["find", needle]);
        assert_eq!(everywhere.exit, 0, "{}", everywhere.out);
        let total = hits(&everywhere.out);
        assert!(total > 0, "the needle must be in the corpus: {}", everywhere.out);

        let inside = run(&db, &["find", needle, "--band", home]);
        assert_eq!(
            (inside.exit, hits(&inside.out)),
            (0, total),
            "every hit for {:?} is in {}, so naming that band keeps them all: {}",
            needle,
            home,
            inside.out
        );

        let outside = run(&db, &["find", needle, "--band", away]);
        assert_eq!(
            (outside.exit, outside.pair("EMPTY")),
            (2, "(2, EMPTY)".to_string()),
            "no hit for {:?} is in {}, so naming that band must exclude every one; if the filter never reached the query this returns {} hits instead: {}",
            needle,
            away,
            total,
            outside.out
        );
    }

    let controls: Vec<(&str, Vec<&str>, i32, &str)> = vec![
        ("negative", vec!["find", "regenerable", "--band", "nowhere"], 2, "UNKNOWN_FILTER_VALUE"),
        ("negative", vec!["find", "regenerable", "--layer", "L9"], 2, "UNKNOWN_FILTER_VALUE"),
        ("negative", vec!["find", "regenerable", "--limit", "999"], 2, "OVER_CAP"),
        ("vacuous", vec!["find", "regenerable", "--layer", "L2", "--limit", "5"], 2, "EMPTY"),
        ("positive", vec!["find", "regenerable", "--layer", "L0"], 0, "hit(s)"),
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
fn an_address_that_names_two_atoms_is_refused_rather_than_answered_with_the_first() {
    let db = fresh_db("ambiguous");
    mutate(&db.join("bands").join("evidence").join("run.md"), |text| {
        format!("{}\n## Overview\n\nA second document now claims that anchor.\n", text.trim_end())
    });
    let built = run(&db, &["compile"]);
    assert_eq!(built.exit, 0, "{}", built.out);

    let ambiguous = run(&db, &["show", "Overview"]);
    assert_eq!(
        (ambiguous.exit, ambiguous.pair("AMBIGUOUS_ADDRESS")),
        (2, "(2, AMBIGUOUS_ADDRESS)".to_string()),
        "two atoms answer to that anchor, and the first is not the answer: {}",
        ambiguous.out
    );
    assert!(
        ambiguous.says("names 2 atoms"),
        "the refusal counts them exactly, not \"more than one\": {}",
        ambiguous.out
    );

    let exact = run(&db, &["show", "arch/01_overview.md#Overview"]);
    assert_eq!(
        exact.exit, 0,
        "the longer address still selects one atom: {}",
        exact.out
    );
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
    let carried = run(&db, &["ls", "--band", "evidence", "--layer", "L2", "--include-gaps", "--json"]);
    assert_eq!(carried.exit, 0, "{}", carried.out);
    assert!(
        carried.says("\"UNKNOWN\""),
        "an undeclared attribute reaches the wire as UNKNOWN, not as a default; in this fixture every claiming atom declares all three, so the atoms that carry an UNKNOWN are the gaps, and they are asked for by name: {}",
        carried.out
    );
    assert!(
        answered.says("\"gaps_excluded\""),
        "and the default projection says how many it left out rather than hiding them silently: {}",
        answered.out
    );

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

#[test]
fn the_gate_wire_carries_every_line_and_its_own_breakdown() {
    let db = admitted("gate_wire");
    let table = run(&db, &["gate"]);
    let wired = run(&db, &["gate", "--json"]);
    assert_eq!(
        table.exit, wired.exit,
        "the same run answers with the same exit on both spellings: {}",
        wired.out
    );
    assert_eq!(field(&wired.out, "schema"), "1");
    let wanted = match table.exit {
        0 => "TRUE",
        1 => "FALSE",
        _ => "UNKNOWN",
    };
    assert!(
        wired.says(&format!("\n  \"verdict\": \"{}\"", wanted)),
        "exit 0 is TRUE, a counted failure is FALSE, and a gate that could not be asked is UNKNOWN; the envelope verdict sits at the top level, where a row verdict is a different question: {}",
        wired.out
    );
    for needle in ["\"cmd\": \"gate\"", "\"matched\"", "\"breakdown\"", "\"G-S1\""] {
        assert!(wired.says(needle), "the gate wire carries {}: {}", needle, wired.out);
    }
    assert!(
        wired.says("\"cap\": null"),
        "a gate run has no row cap, and null says so rather than a zero that reads like one: {}",
        wired.out
    );

    let broken = fresh_db("gate_wire_undeclared");
    mutate(&broken.join("bands").join("arch").join("band.toml"), |text| {
        text.replace("executor = \"owner\"\n", "")
    });
    let built = run(&broken, &["compile"]);
    assert_eq!(built.exit, 0, "{}", built.out);
    let counted = run(&broken, &["gate", "--json"]);
    assert!(
        counted.says("UNDECLARED_ATTRIBUTE") && counted.says("\"attribute\": \"executor\""),
        "an atom with no declared executor reaches the wire counted by attribute and document: {}",
        counted.out
    );
    let spelled = run(&broken, &["gate", "--detail"]);
    assert!(
        spelled.says("attribute") && spelled.says("executor"),
        "--detail prints the same tally as a table: {}",
        spelled.out
    );
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

    let served_gate = http_get(port, "/v1/gate", "GET").expect("the gate route should answer");
    let stdout_gate = run(&db, &["gate", "--json"]);
    assert_eq!(
        served_gate.1.trim(),
        stdout_gate.out.trim(),
        "one serialiser for the gate too: the http body and the stdout json are the same bytes"
    );
    assert_eq!(
        served_gate.0,
        if stdout_gate.exit == 0 { 200 } else { 422 },
        "a gate run that did not pass is not a 200: {}",
        served_gate.1
    );

    let served_bands = http_get(port, "/v1/bands", "GET").expect("the band route should answer");
    let stdout_bands = run(&db, &["bands", "--json"]);
    assert_eq!(
        served_bands.1.trim(),
        stdout_bands.out.trim(),
        "one serialiser for the band listing too, so a face reads the bands from the engine and not from whatever host is serving it"
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

#[test]
fn init_writes_the_four_entry_skeleton_and_its_gate_is_untestable() {
    let dir = scratch_dir("init_fresh");
    let created = run(&dir, &["init", dir.to_str().expect("utf8 path")]);
    assert_eq!(created.exit, 0, "init over a directory with none of its targets should answer: {}", created.out);

    assert!(dir.join("db.toml").is_file(), "root manifest");
    assert!(dir.join("bands").join("decisions").join("band.toml").is_file(), "band manifest");
    let heading_path = dir.join("bands").join("decisions").join("01_DECISIONS.md");
    assert!(heading_path.is_file(), "the placeholder document");
    assert_eq!(
        fs::read_to_string(&heading_path).expect("read the placeholder"),
        "# Decisions\n",
        "init writes the bare heading only, no D-entry"
    );
    assert!(dir.join("journal").join("semantic.journal.jsonl").is_file(), "journal file");
    assert_eq!(
        fs::read(dir.join("journal").join("semantic.journal.jsonl")).expect("read the journal"),
        Vec::<u8>::new(),
        "a freshly initialized journal admits nothing yet"
    );
    assert!(dir.join("journal").join("HEAD").is_file(), "HEAD");
    assert!(dir.join("build").is_dir(), "build/ exists and is empty");
    assert_eq!(
        fs::read_dir(dir.join("build")).expect("read build/").count(),
        0,
        "build/ is empty right after init"
    );
    assert!(dir.join(".gitignore").is_file(), ".gitignore");

    // contrast: the DB principle says an empty DB's gate is UNTESTABLE, never a silent pass,
    // and it must be UNTESTABLE for the reason init actually produced: nothing admitted yet.
    let gated = run(&dir, &["gate"]);
    assert_eq!(gated.exit, 2, "a DB with 0 admission events must not gate green: {}", gated.out);
    assert!(gated.says("G-S5"), "the journal gate must be the one naming the empty chain: {}", gated.out);
    assert!(gated.says("JOURNAL_ABSENT"), "the reason must be JOURNAL_ABSENT, not folded into a generic fail: {}", gated.out);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn init_refuses_to_overwrite_a_hand_planted_target() {
    let dir = scratch_dir("init_occupied");
    fs::create_dir_all(&dir).expect("create the directory ahead of init");
    fs::write(dir.join("db.toml"), b"already real content, not init's to touch\n").expect("plant db.toml");

    let refused = run(&dir, &["init", dir.to_str().expect("utf8 path")]);
    assert_eq!(refused.exit, 2, "init over an existing db.toml must refuse rather than overwrite: {}", refused.out);
    assert!(refused.says("INIT_REFUSED_EXISTS"));
    assert!(refused.says("db.toml"), "the enumeration must name the target that blocked it: {}", refused.out);
    assert!(
        !dir.join("bands").exists(),
        "a refused init must not have written any of the other targets either"
    );
    assert_eq!(
        fs::read_to_string(dir.join("db.toml")).expect("read the planted file"),
        "already real content, not init's to touch\n",
        "the planted file must be untouched"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn init_run_twice_refuses_the_second_time_over_its_own_output() {
    let dir = scratch_dir("init_twice");
    let first = run(&dir, &["init", dir.to_str().expect("utf8 path")]);
    assert_eq!(first.exit, 0, "the first init should answer: {}", first.out);

    let second = run(&dir, &["init", dir.to_str().expect("utf8 path")]);
    assert_eq!(second.exit, 2, "init is not idempotent by overwrite; a second call must refuse: {}", second.out);
    assert!(second.says("INIT_REFUSED_EXISTS"));
    assert!(
        second.says("7 of the 7 target(s)") || second.says("7 target(s)"),
        "the second call must see every one of the 7 targets the first call wrote: {}",
        second.out
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_db_root_outside_any_db_prints_the_search_trail() {
    let dir = scratch_dir("search_outside");
    fs::create_dir_all(&dir).expect("an ordinary directory with no db.toml anywhere above it");

    let lost = run_in(&dir, &["gate"]);
    assert_eq!(lost.exit, 2, "with no db.toml reachable upward, resolution must refuse: {}", lost.out);
    assert!(lost.says("NO_DB_ROOT"));
    assert!(lost.says("db.toml"), "the candidate list it walked must be printed: {}", lost.out);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_subdirectory_of_a_db_resolves_upward_to_its_root() {
    let dir = fresh_db("search_subdir");
    let sub = dir.join("bands").join("arch");
    let compiled = run_in(&sub, &["compile"]);
    assert_eq!(
        compiled.exit, 0,
        "run with no --db from inside bands/arch, the upward walk must still find db.toml: {}",
        compiled.out
    );
    assert!(
        compiled.says("2 band(s)"),
        "it must be this DB's own bands that compiled, not an unrelated one: {}",
        compiled.out
    );
}

#[test]
fn a_nested_db_resolves_to_the_nearest_one_not_the_outer_one() {
    let outer = fresh_db("search_nest_outer");
    let inner = outer.join("bands").join("arch").join("nested_db");
    copy_tree(&fixture(), &inner);

    let deep = inner.join("bands").join("arch");
    let compiled = run_in(&deep, &["compile"]);
    assert_eq!(
        compiled.exit, 0,
        "resolution from deep inside the inner DB must still find its own, nearer db.toml: {}",
        compiled.out
    );
    assert!(
        inner.join("build").join("index").join("semantic.sqlite").is_file(),
        "the inner DB is the one that got compiled"
    );
    assert!(
        !outer.join("build").exists(),
        "the outer DB, further up the walk, must not have been touched at all"
    );
}

fn legacy_bed(label: &str) -> (PathBuf, PathBuf) {
    let db = admitted(label);
    let journal = db.join("journal").join("semantic.journal.jsonl");
    append_line(&journal, LEGACY_LINE);
    settle(&db);
    let hold = run(&db, &["gate"]);
    assert_eq!(
        (hold.exit, hold.says("LEGACY_UNVERIFIABLE")),
        (2, true),
        "the bed for the tamper controls is a journal that holds an older record and a HEAD that covers it: {}",
        hold.out
    );
    (db, journal)
}

#[test]
fn g_s5_still_checks_head_when_the_journal_holds_a_record_it_cannot_recompute() {
    let (base, base_journal) = legacy_bed("chain_t0");
    let lines = journal_lines(&base_journal);
    let held = run(&base, &["gate"]);
    assert!(
        held.says(&format!("HEAD equals the fold of all {} line(s)", lines)),
        "the untampered bed states what it checked and over how many lines: {}",
        held.out
    );
    assert!(
        held.says(&format!("{} line(s) in the current format carry a prev_hash this engine recomputed and hold", lines - 1)),
        "the count of checked lines is the count it recomputed, never the whole file: {}",
        held.out
    );

    let (one, one_journal) = legacy_bed("chain_t1a");
    edit_last_line(&one_journal, |line| flip_hex_after(&line, "atom_id"));
    let tampered = run(&one, &["gate"]);
    assert_eq!(
        (tampered.exit, tampered.pair("HEAD_MISMATCH")),
        (1, "(1, HEAD_MISMATCH)".to_string()),
        "T1a: a field of the last record changed, the record still parses, and the fold no longer matches HEAD: {}",
        tampered.out
    );

    let (two, two_journal) = legacy_bed("chain_t1b");
    edit_last_line(&two_journal, |line| flip_hex_after(&line, "prev_hash"));
    let relinked = run(&two, &["gate"]);
    assert_eq!(
        (relinked.exit, relinked.pair("CHAIN_BREAK")),
        (1, "(1, CHAIN_BREAK)".to_string()),
        "T1b: the last record now claims a predecessor it does not have: {}",
        relinked.out
    );

    let (three, three_journal) = legacy_bed("chain_t2");
    edit_last_line(&three_journal, |line| line[1..].to_string());
    let unreadable = run(&three, &["gate"]);
    assert_eq!(
        (unreadable.exit, unreadable.pair("CHAIN_BREAK")),
        (1, "(1, CHAIN_BREAK)".to_string()),
        "T2: a line of neither shape is a break in the chain, never a line to skip: {}",
        unreadable.out
    );

    let (four, _) = legacy_bed("chain_t4");
    mutate(&four.join("journal").join("HEAD"), |text| {
        let mut bytes = text.trim().to_string().into_bytes();
        bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
        format!("{}\n", String::from_utf8(bytes).expect("a head is hex, and hex is ascii"))
    });
    let moved = run(&four, &["gate"]);
    assert_eq!(
        (moved.exit, moved.pair("HEAD_MISMATCH")),
        (1, "(1, HEAD_MISMATCH)".to_string()),
        "T4: one byte of HEAD is one byte of the only thing that protects the last record: {}",
        moved.out
    );

    let (five, five_journal) = legacy_bed("chain_t5");
    let before = journal_lines(&five_journal);
    let text = fs::read_to_string(&five_journal).expect("read the journal");
    let kept: Vec<&str> = text.trim_end_matches('\n').split('\n').collect();
    fs::write(&five_journal, format!("{}\n", kept[..kept.len() - 1].join("\n"))).expect("write the shortened journal");
    let after = journal_lines(&five_journal);
    assert_eq!(after, before - 1, "the control removed exactly one line");
    let truncated = run(&five, &["gate"]);
    assert_eq!(
        (truncated.exit, truncated.pair("HEAD_MISMATCH")),
        (1, "(1, HEAD_MISMATCH)".to_string()),
        "T5: the last record is gone and HEAD is the fold that still counts it: {}",
        truncated.out
    );
    assert!(
        truncated.says(&format!("the {} line(s) on disk", after)),
        "the denominator is the number of lines it actually read, printed, never quietly shrunk: {}",
        truncated.out
    );
}

#[test]
fn g_s5_says_head_absent_rather_than_passing_over_an_unprotected_tail() {
    let (db, _) = legacy_bed("chain_head_absent");
    fs::remove_file(db.join("journal").join("HEAD")).expect("remove HEAD");
    let bare = run(&db, &["gate"]);
    assert_eq!(
        (bare.exit, bare.pair("HEAD_ABSENT")),
        (2, "(2, HEAD_ABSENT)".to_string()),
        "with no HEAD the last record has nothing to be checked against, which is UNKNOWN and never a pass: {}",
        bare.out
    );
}

#[test]
fn a_query_refuses_to_answer_from_an_index_the_source_has_moved_past() {
    let db = admitted("stale");
    let fresh = run(&db, &["find", "regenerable"]);
    assert_eq!(fresh.exit, 0, "positive control: a fresh index answers: {}", fresh.out);

    mutate(&db.join("bands").join("arch").join("01_overview.md"), |body| {
        format!("{}\n\n## D-0002 a decision written after the index was built\n\nchartreuse\n", body.trim_end())
    });

    let stale = run(&db, &["find", "chartreuse"]);
    assert_eq!(
        (stale.exit, stale.pair("STALE_INDEX")),
        (2, "(2, STALE_INDEX)".to_string()),
        "the answer is on disk and not in the index, so EMPTY would be a false answer: {}",
        stale.out
    );
    assert!(stale.says("Run db compile"), "the refusal says what to run: {}", stale.out);

    for command in [
        vec!["ls", "--band", "arch", "--layer", "L1"],
        vec!["show", "Overview"],
    ] {
        let refused = run(&db, &command);
        assert_eq!(
            (refused.exit, refused.pair("STALE_INDEX")),
            (2, "(2, STALE_INDEX)".to_string()),
            "every query path is gated on the same freshness, not only find: {:?} {}",
            command,
            refused.out
        );
    }

    let strictly = run(&db, &["--strict", "find", "chartreuse"]);
    assert_eq!(
        (strictly.exit, strictly.pair("STALE_INDEX")),
        (2, "(2, STALE_INDEX)".to_string()),
        "--strict compares the bytes and reaches the same verdict: {}",
        strictly.out
    );

    let rebuilt = run(&db, &["push"]);
    assert_eq!(rebuilt.exit, 0, "{}", rebuilt.out);
    let answered = run(&db, &["find", "chartreuse"]);
    assert_eq!(
        (answered.exit, answered.says("hit(s)")),
        (0, true),
        "after compile the same question is answered rather than refused: {}",
        answered.out
    );
}

#[test]
fn a_source_file_whose_stamp_moved_but_whose_bytes_did_not_is_not_called_stale() {
    let db = admitted("stamp_moved");
    let document = db.join("bands").join("arch").join("01_overview.md");
    let bytes = fs::read(&document).expect("read the document");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(&document, &bytes).expect("write the same bytes back");
    assert_eq!(bytes, fs::read(&document).expect("read it again"), "the control rewrote the same bytes");

    let answered = run(&db, &["find", "regenerable"]);
    assert_eq!(
        (answered.exit, answered.says("hit(s)")),
        (0, true),
        "the cheap stamp moved and the digest did not, so the index is fresh; the stamp is a hint, never the verdict: {}",
        answered.out
    );
}

#[test]
fn every_band_carries_an_l0_layer_because_a_heading_is_one() {
    let db = compiled("headings");
    for band in ["arch", "evidence"] {
        let listed = run(&db, &["ls", "--band", band, "--layer", "L0", "--cursor", "begin"]);
        assert_eq!(
            listed.exit, 0,
            "a heading is the header tier whatever role its document carries, so every band has an L0: {} {}",
            band, listed.out
        );
        assert!(
            !row_lines(&listed.out).is_empty(),
            "the L0 page of {} carries rows: {}",
            band,
            listed.out
        );
    }

    let tagged = run(&db, &["ls", "--band", "arch", "--layer", "L1", "--cursor", "begin"]);
    assert_eq!(tagged.exit, 0, "{}", tagged.out);
    assert!(
        tagged.says("#Layers"),
        "a heading that declares a layer keeps it; the heading default applies only where nothing was declared: {}",
        tagged.out
    );
}

#[test]
fn the_budget_pages_instead_of_refusing_every_page_the_same_way() {
    let db = fresh_db("budget");
    mutate(&db.join("db.toml"), |text| text.replace("budget_tokens = 8000", "budget_tokens = 120"));
    let built = run(&db, &["compile"]);
    assert_eq!(built.exit, 0, "{}", built.out);

    let page = run(&db, &["ls", "--band", "arch", "--layer", "L1", "--lod", "2", "--cursor", "begin"]);
    assert_eq!(
        page.exit, 0,
        "a page over the budget is cut to the rows that fit and says so, rather than refusing every cursor alike: {}",
        page.out
    );
    assert!(page.says("cut to"), "the cut is stated, never silent: {}", page.out);
    let next = page
        .out
        .split('\n')
        .find(|line| line.starts_with("next: db ls --cursor "))
        .map(|line| line.trim_start_matches("next: db ls --cursor ").trim().to_string())
        .expect("a cut page carries the cursor that advances it");
    let second = run(&db, &["ls", "--band", "arch", "--layer", "L1", "--lod", "2", "--cursor", &next]);
    assert_eq!(second.exit, 0, "the cursor from a cut page advances: {}", second.out);
    assert_ne!(
        row_lines(&page.out).first(),
        row_lines(&second.out).first(),
        "the second page is a different page"
    );

    let starved = fresh_db("budget_starved");
    mutate(&starved.join("db.toml"), |text| text.replace("budget_tokens = 8000", "budget_tokens = 4"));
    let rebuilt = run(&starved, &["compile"]);
    assert_eq!(rebuilt.exit, 0, "{}", rebuilt.out);
    let refused = run(&starved, &["ls", "--band", "arch", "--layer", "L1", "--lod", "2", "--cursor", "begin"]);
    assert_eq!(
        (refused.exit, refused.pair("OVER_BUDGET")),
        (2, "(2, OVER_BUDGET)".to_string()),
        "when one row alone is over the whole budget there is no page to cut to, and an atom is never printed in half: {}",
        refused.out
    );
}

#[test]
fn advice_printed_beside_a_refusal_is_advice_that_changes_the_answer() {
    let db = fresh_db("advice");
    mutate(&db.join("db.toml"), |text| text.replace("L1 = 30", "L1 = 1"));
    let built = run(&db, &["compile"]);
    assert_eq!(built.exit, 0, "{}", built.out);

    let refused = run(&db, &["ls", "--band", "arch", "--layer", "L1"]);
    assert_eq!(
        (refused.exit, refused.pair("OVER_CAP")),
        (2, "(2, OVER_CAP)".to_string()),
        "{}",
        refused.out
    );
    assert!(
        refused.says("no filter narrows this projection"),
        "every row of this projection carries one band, one role and one layer, so it must say that no filter narrows it rather than offering three that do not: {}",
        refused.out
    );
    assert!(
        !refused.says("narrow by --"),
        "an axis with a single value is not offered as a way to narrow, because it returns the same rows again: {}",
        refused.out
    );
    assert!(refused.says("page with --cursor begin"), "{}", refused.out);
}

#[test]
fn a_refusal_carries_the_same_denominator_the_answer_would_have() {
    let db = fresh_db("refusal_wire");
    mutate(&db.join("db.toml"), |text| text.replace("L1 = 30", "L1 = 1"));
    let built = run(&db, &["compile"]);
    assert_eq!(built.exit, 0, "{}", built.out);

    let refused = run(&db, &["ls", "--band", "arch", "--layer", "L1", "--json"]);
    assert_eq!(refused.exit, 2, "{}", refused.out);
    assert_eq!(field(&refused.out, "reason"), "OVER_CAP");
    assert_eq!(field(&refused.out, "cmd"), "ls");
    assert_eq!(field(&refused.out, "band"), "arch");
    assert_eq!(field(&refused.out, "rows"), "1", "the cap that refused is on the envelope");
    let matched: usize = field(&refused.out, "matched").parse().expect("a number");
    let returned: usize = field(&refused.out, "returned").parse().expect("a number");
    let withheld: usize = field(&refused.out, "withheld").parse().expect("a number");
    assert!(matched > 1, "a refusal that counted the rows says how many: {}", refused.out);
    assert_eq!((returned, withheld), (0, matched), "withheld is what matched and did not come back");

    let unknown = run(&db, &["ls", "--layer", "L9", "--json"]);
    assert_eq!(unknown.exit, 2, "{}", unknown.out);
    assert_eq!(field(&unknown.out, "reason"), "UNKNOWN_FILTER_VALUE");
    assert_eq!(field(&unknown.out, "cmd"), "ls", "a refused question still names the command it refused");
    assert_eq!(field(&unknown.out, "verdict"), "UNKNOWN", "0 matched under UNKNOWN is not 0 matched under FALSE");

    let answered = run(&db, &["ls", "--band", "arch", "--layer", "L1", "--cursor", "begin", "--json"]);
    assert_eq!(answered.exit, 0, "{}", answered.out);
    let matched: usize = field(&answered.out, "matched").parse().expect("a number");
    let returned: usize = field(&answered.out, "returned").parse().expect("a number");
    let withheld: usize = field(&answered.out, "withheld").parse().expect("a number");
    assert_eq!(withheld, matched - returned, "withheld is matched less returned on an answer too");
}

#[test]
fn find_prints_an_address_show_accepts_and_the_line_that_matched() {
    let db = fresh_db("find_address");
    mutate(&db.join("bands").join("evidence").join("run.md"), |body| {
        format!(
            "{}\n\n[MEASURED] a paragraph whose first line names nothing\nand whose second line names chartreuse.\n",
            body.trim_end()
        )
    });
    let built = run(&db, &["compile"]);
    assert_eq!(built.exit, 0, "{}", built.out);

    let hit = run(&db, &["find", "chartreuse"]);
    assert_eq!(hit.exit, 0, "{}", hit.out);
    let row = row_lines(&hit.out).first().expect("a hit row").clone();
    let address = address_of(&row);
    assert!(
        !address.chars().all(|value| value.is_ascii_hexdigit()),
        "find prints an address, not a prefix of a digest: {}",
        row
    );
    assert!(
        row.contains("and whose second line names chartreuse"),
        "the line printed is the line that matched, not the first line of the atom: {}",
        row
    );
    assert!(row.contains("+1"), "the offset of the matching line inside the atom is named: {}", row);

    let shown = run(&db, &["show", &address]);
    assert_eq!(
        shown.exit, 0,
        "the address find printed is the address show takes; nothing has to be reconstructed by hand: {}",
        shown.out
    );
}

#[test]
fn show_prints_a_single_line_atom_once() {
    let db = compiled("show_once");
    let shown = run(&db, &["show", "Overview", "--lod", "1"]);
    assert_eq!(shown.exit, 0, "{}", shown.out);
    assert_eq!(
        shown.out.matches("# Overview").count(),
        1,
        "at lod 1 an atom whose body is its own headline is printed once, not twice: {}",
        shown.out
    );

    let deeper = run(&db, &["show", "Overview", "--lod", "2"]);
    assert_eq!(deeper.exit, 0, "{}", deeper.out);
    let said = shown.out.split('\n').next().expect("a first line").to_string();
    assert!(
        deeper.says(&said),
        "lod 2 still contains everything lod 1 said: {}",
        deeper.out
    );
}

#[test]
fn init_declares_the_document_it_writes() {
    let dir = scratch_dir("init_declared");
    let made = run_in(&std::env::temp_dir(), &["init", dir.to_str().expect("a path")]);
    assert_eq!(made.exit, 0, "{}", made.out);
    let gated = run(&dir, &["gate"]);
    assert!(
        !gated.says("ORPHAN_MD"),
        "init declares the document it writes, so a DB it has just made carries no orphan: {}",
        gated.out
    );
    assert!(
        !gated.says("GRANULARITY_UNDECLARED"),
        "and it declares the floor and ceiling G-S6 needs, or every DB it makes starts UNKNOWN against a rule it never wrote: {}",
        gated.out
    );
    assert_eq!(
        (gated.exit, gated.says("JOURNAL_ABSENT")),
        (2, true),
        "a DB with nothing admitted is still UNTESTABLE, never a pass: {}",
        gated.out
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn bands_counts_every_declared_band_and_keeps_gaps_beside_the_claims() {
    let db = compiled("bands");
    let listed = run(&db, &["bands"]);
    assert_eq!(listed.exit, 0, "{}", listed.out);
    for band in ["arch", "evidence"] {
        assert!(listed.says(band), "every band db.toml declares is listed: {}", listed.out);
    }

    let wired = run(&db, &["bands", "--json"]);
    assert_eq!(wired.exit, 0, "{}", wired.out);
    assert_eq!(field(&wired.out, "cmd"), "bands");
    assert_eq!(field(&wired.out, "verdict"), "TRUE");
    assert!(
        wired.says("\"cap\": null"),
        "a band listing has no row cap, and null says so rather than a zero that reads like one: {}",
        wired.out
    );

    let counted = run(&db, &["ls", "--include-gaps", "--cursor", "begin", "--json"]);
    assert_eq!(counted.exit, 0, "{}", counted.out);
    let whole: usize = field(&counted.out, "total").parse().expect("a whole corpus count");
    let mut summed = 0usize;
    for line in wired.out.split('\n') {
        let trimmed = line.trim();
        for key in ["\"atoms\":", "\"gaps\":"] {
            if trimmed.starts_with(key) {
                let value = trimmed[key.len()..].trim().trim_end_matches(',');
                summed += value.parse::<usize>().expect("a whole band count");
            }
        }
    }
    assert_eq!(
        summed, whole,
        "the atoms and gaps of every band add up to the corpus the wire reports as its total, or one of the two counts is hiding inside the other:\n{}\n{}",
        wired.out, counted.out
    );

    let blind = fresh_db("bands_no_index");
    let asked = run(&blind, &["bands"]);
    assert_eq!(
        (asked.exit, asked.says("UNTESTABLE")),
        (2, true),
        "a DB with no index answers UNTESTABLE, never an empty band list that reads like a DB with no bands: {}",
        asked.out
    );
}

#[test]
fn the_wire_carries_the_corpus_it_drew_from_and_says_null_when_it_never_counted() {
    let db = compiled("total");
    let answered = run(&db, &["ls", "--band", "arch", "--json"]);
    assert_eq!(answered.exit, 0, "{}", answered.out);
    let total: usize = field(&answered.out, "total").parse().expect("a whole number");
    let matched: usize = field(&answered.out, "matched").parse().expect("a whole number");
    assert!(
        total >= matched && matched > 0,
        "total is the corpus the projection was drawn from, so it is never below matched: total {} matched {}",
        total,
        matched
    );

    let over = run(&db, &["ls", "--include-gaps", "--json"]);
    assert_eq!(
        (over.exit, field(&over.out, "reason")),
        (2, "OVER_CAP".to_string()),
        "the whole corpus is over the strictest cap, which is the refusal this control wants: {}",
        over.out
    );
    assert_eq!(
        field(&over.out, "total"),
        total.to_string(),
        "a refusal that did count carries the same total the answer would have, and the total is a property of the index rather than of the filter: {}",
        over.out
    );

    let refused = run(&db, &["ls", "--layer", "L9", "--json"]);
    assert_eq!(refused.exit, 2, "{}", refused.out);
    assert!(
        refused.says("\"total\": null"),
        "a filter refused before anything was counted reports null, not a zero that reads like an empty corpus: {}",
        refused.out
    );
}

#[test]
fn g_s6_sizes_atoms_against_a_declared_floor_and_ceiling() {
    let clean = admitted("gran_clean");
    let held = run(&clean, &["gate"]);
    assert!(
        held.says("G-S6") && held.says("none carries two kinds of evidence marker"),
        "the fixture declares [granularity] and holds against it, or the negatives below prove nothing: {}",
        held.out
    );

    let coarse = fresh_db("gran_coarse");
    mutate(&coarse.join("bands").join("evidence").join("run.md"), |text| {
        format!("{}\n[DERIVED] and the same paragraph also says [MEASURED], so one atom carries two claims.\n", text)
    });
    let widened = run(&coarse, &["gate"]);
    assert!(
        widened.says("GRANULARITY_COARSE"),
        "an atom carrying two kinds of evidence marker is over-coarse and must be named: {}",
        widened.out
    );

    let fine = fresh_db("gran_fine");
    fs::write(
        fine.join("bands").join("evidence").join("run.md"),
        "\n# S\n\nx\n\n",
    )
    .expect("write the shredded document");
    let split = run(&fine, &["gate"]);
    assert!(
        split.says("GRANULARITY_FINE") && split.says("byte floor"),
        "a document whose claiming atoms are under the floor and whose separators outnumber them past the ceiling is over-fine and must be named: {}",
        split.out
    );

    let silent = fresh_db("gran_silent");
    mutate(&silent.join("db.toml"), |text| {
        text.replace("[granularity]\nmin_mean_bytes = 40\nmax_gap_ratio = 0.55\n\n", "")
    });
    let unmeasured = run(&silent, &["gate"]);
    assert!(
        unmeasured.says("GRANULARITY_UNDECLARED"),
        "a DB that declares no floor is UNKNOWN against it, never a pass: {}",
        unmeasured.out
    );
    assert!(
        !unmeasured.says("GRANULARITY_COARSE") && !unmeasured.says("GRANULARITY_FINE"),
        "and an undeclared rule is not quietly replaced by one this engine chose: {}",
        unmeasured.out
    );

    let bad = fresh_db("gran_bad");
    mutate(&bad.join("db.toml"), |text| text.replace("min_mean_bytes = 40", "min_mean_bytes = 0"));
    let refused = run(&bad, &["gate"]);
    assert_eq!(
        (refused.exit, refused.says("UNTESTABLE")),
        (2, true),
        "a floor below one byte can never be crossed, so it is refused rather than accepted and never fired: {}",
        refused.out
    );
}
