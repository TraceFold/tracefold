// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1a** (`req/535` §3 R-1, §4 AC-1 / AC-2' / AC-3) — placing `.gx/` on a tree that is
//! already running, and saying what was placed.
//!
//! # What `req/535` §1 measured, and what this suite holds
//!
//! `req/535` §1 opened the `Command` enum and counted: twenty-two top-level verbs, and no `init`,
//! no `attach`, no `detach` among them. `.gx/` grew as a side effect of whichever verb ran first,
//! and there was no road by which a machine could be told **what that verb had made**. R-1a is the
//! repair: one operation, and an enumeration of all eleven rows of `gx_cli::layout::GX_PATHS` split
//! into what it created, what was already there, and what it did not place.
//!
//! Three arms carry the requirement and three more are their negative controls, because a probe
//! that only ever sees a healthy output is a probe that cannot tell a whole enumeration from a
//! short one. Each control feeds a **damaged** value to the same predicate the positive arm uses
//! and requires it to refuse.
//!
//! # 🔴 AC-2', not AC-2
//!
//! `req/535` §7 KA-1 is the assumption that a tree can be attached to without changing a byte of
//! it, and its own kill condition strengthened the acceptance: tracked files unchanged **and** the
//! additions to the tree appearing in the enumeration. The second half is the load-bearing one
//! here, and this suite measured why while it was being written: `DeclarationWriter::initialise`
//! writes `.gx/.gitignore` holding `*`, so the directory ignores itself and `git status` reports
//! **nothing at all** after an attach. The declaration is therefore not one of two places a reader
//! could learn what arrived — it is the only one.
//!
//! # What is deliberately not here
//!
//! R-3 (the coverage posture), R-5 (detach) and the registration of the wrap route as an attach
//! face (R-2) are `req/535` §8's P-1b and P-1c. This suite says nothing about any of them, and
//! `attach`'s own output names them as questions this face does not answer rather than leaving
//! them off.

mod support;

use std::path::Path;
use std::process::Command;

use serde_json::Value;
use support::{gx, run, scratch};

/// The three words `attach` may file a declared path under, and the only three.
const CLASSES: [&str; 3] = ["created", "already-present", "not-placed"];

// ---------------------------------------------------------------------------
// The predicate, written once so that the controls can attack it
// ---------------------------------------------------------------------------

/// 🔴 **AC-1** — is this enumeration whole?
///
/// A function returning a `Result` rather than a body of assertions, because the negative controls
/// below have to be able to *watch it refuse*. An assertion inside the positive arm cannot be run
/// against a damaged value without failing the arm that runs it.
///
/// Whole means four things at once, and the fourth is the one a short list passes without: every
/// row of `GX_PATHS` is present **by name**, no name appears twice, every row carries one of
/// [`CLASSES`], and the counts the answer states add up to the number of rows it printed.
fn enumeration_is_whole(v: &Value) -> Result<(), String> {
    let rows = v["placement"]
        .as_array()
        .ok_or_else(|| "`placement` is not an array".to_string())?;

    let mut seen: Vec<String> = Vec::new();
    for row in rows {
        let rel = row["rel"]
            .as_str()
            .ok_or_else(|| format!("a row carries no `rel`: {row}"))?;
        if seen.iter().any(|s| s == rel) {
            return Err(format!("`{rel}` is enumerated twice"));
        }
        seen.push(rel.to_string());
        let class = row["placement"]
            .as_str()
            .ok_or_else(|| format!("`{rel}` carries no `placement`: {row}"))?;
        if !CLASSES.contains(&class) {
            return Err(format!(
                "`{rel}` is filed under {class:?}, which is not one of {CLASSES:?}"
            ));
        }
        // R-1c: the nature travels with the row. A path whose loss cannot be undone and a path
        // that regenerates itself are two different facts about an attach, and an enumeration that
        // dropped the distinction would be a list of names.
        for key in ["shape", "nature"] {
            if !row[key].is_string() {
                return Err(format!("`{rel}` carries no `{key}`: {row}"));
            }
        }
    }

    // 🔴 The denominator is the table itself, read out of the library rather than transcribed. A
    // literal `11` here would be a second copy of `GX_PATHS`'s length, and the row this suite
    // exists to catch is exactly the row somebody forgot to copy.
    for declared in gx_cli::layout::GX_PATHS {
        if !seen.iter().any(|s| s == declared.rel) {
            return Err(format!(
                "`{}` is a row of GX_PATHS and the enumeration does not name it (it named {} of {})",
                declared.rel,
                seen.len(),
                gx_cli::layout::GX_PATHS.len()
            ));
        }
    }
    if seen.len() != gx_cli::layout::GX_PATHS.len() {
        return Err(format!(
            "the enumeration holds {} rows and GX_PATHS declares {}",
            seen.len(),
            gx_cli::layout::GX_PATHS.len()
        ));
    }

    let counts = &v["counts"];
    let total = counts["total"]
        .as_u64()
        .ok_or_else(|| "`counts.total` is not a number".to_string())?;
    let sum: u64 = CLASSES
        .iter()
        .map(|c| counts[c.replace('-', "_")].as_u64().unwrap_or(0))
        .sum();
    if total as usize != rows.len() || sum != total {
        return Err(format!(
            "the counts do not describe the rows: total={total}, sum of the three classes={sum}, \
             rows printed={}",
            rows.len()
        ));
    }
    Ok(())
}

/// 🔴 **AC-2'**, second half — every path that appeared in the tree is a path the answer named.
///
/// Equality in both directions rather than containment. A declaration that named more than it made
/// would be as wrong as one that named less, and only one of those two is caught by "the tree's
/// additions are a subset".
fn additions_are_declared(v: &Value, before: &[String], after: &[String]) -> Result<(), String> {
    let mut arrived: Vec<&String> = after.iter().filter(|p| !before.contains(p)).collect();
    arrived.sort();
    let mut declared: Vec<&str> = v["created_entries"]
        .as_array()
        .ok_or_else(|| "`created_entries` is not an array".to_string())?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    declared.sort();

    let missing: Vec<&&String> = arrived
        .iter()
        .filter(|p| !declared.contains(&p.as_str()))
        .collect();
    let invented: Vec<&&str> = declared
        .iter()
        .filter(|p| !arrived.iter().any(|a| a.as_str() == **p))
        .collect();
    if !missing.is_empty() || !invented.is_empty() {
        return Err(format!(
            "the tree gained {} entries and the answer named {}: not named={missing:?}, named and \
             not there={invented:?}",
            arrived.len(),
            declared.len()
        ));
    }
    Ok(())
}

/// Every file and directory under `root`, project-relative, sorted, with `/` separators.
///
/// Recursive, because the entries an attach adds are three levels down (`.gx/ledger/journal`) and a
/// shallow listing would let the enumeration name a directory and say nothing about what is in it.
fn walk(root: &Path) -> Vec<String> {
    fn inner(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            // `.git/` is the fixture's own bookkeeping and it moves on every command git runs; an
            // attach that added nothing would still look like it had if this walk read it.
            if rel == ".git" || rel.starts_with(".git/") {
                continue;
            }
            out.push(rel);
            if path.is_dir() {
                inner(root, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    inner(root, root, &mut out);
    out.sort();
    out
}

/// One git command, as text.
fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} runs: {e}"));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// 🔴 The tracked set with each file's **working-tree** digest, plus `HEAD`.
///
/// # Why not `git ls-files -s`
///
/// That is what this function was, and the control below caught it: `-s` prints the object name
/// the **index** holds, so a tracked file rewritten on disk and not staged comes back byte for byte
/// identical. The control wrote a different sentence into the committed file and the comparison
/// said nothing had changed — which means the positive arm, had it stayed, would have been green
/// against an attach that rewrote every tracked file in the tree.
///
/// `git hash-object <path>` reads the file, so the digest is of what is on the disk. The index
/// listing is kept beside it because AC-2 names the tracked **set** as well as its contents: a file
/// removed from the index and one rewritten in place are two different violations of R-1b.
fn tracked_state(dir: &Path) -> (String, String) {
    let names = git_out(dir, &["ls-files", "-z"]);
    let mut digests = String::new();
    for name in names.split('\0').filter(|s| !s.is_empty()) {
        digests.push_str(git_out(dir, &["hash-object", "--", name]).trim());
        digests.push(' ');
        digests.push_str(name);
        digests.push('\n');
    }
    (digests, git_out(dir, &["rev-parse", "HEAD"]))
}

/// A git repository holding one committed file.
fn git_fixture(name: &str) -> std::path::PathBuf {
    let dir = scratch(name);
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap_or_else(|e| panic!("git {args:?} runs: {e}"));
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "p1a@example.invalid"]);
    git(&["config", "user.name", "p1a"]);
    git(&["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join("app.txt"), "the work that was already running\n")
        .expect("write the tracked file");
    git(&["add", "app.txt"]);
    git(&["commit", "-q", "-m", "the tree before gx"]);
    dir
}

/// `gx attach --project <dir>`, parsed.
fn attach(dir: &Path) -> (support::Run, Value) {
    let out = run(gx().arg("attach").arg("--project").arg(dir));
    assert_eq!(
        out.code, 0,
        "`gx attach` on a directory it may write to is 44 §1.4's 0.\nstdout={}\nstderr={}",
        out.stdout, out.stderr
    );
    let json = out.json();
    (out, json)
}

// ---------------------------------------------------------------------------
// AC-1 — the enumeration, and the control that a short one is refused
// ---------------------------------------------------------------------------

/// 🔴 **AC-1** — one operation, and all eleven declared paths classified.
#[test]
fn ac1_attach_enumerates_every_declared_path_in_three_classes() {
    let dir = scratch("p1a_ac1");
    let (_, json) = attach(&dir);
    println!("AC1_ANSWER={json:#}");
    enumeration_is_whole(&json).unwrap_or_else(|why| panic!("🔴 AC-1: {why}\n{json:#}"));

    // The classification is not decoration: a fresh directory has the four `Nature::Source`
    // directories, the two `Nature::Meta` files and the rest **made**, so `created` cannot be zero
    // — an implementation that filed everything under one word would satisfy the shape check above
    // and say nothing.
    let created = json["counts"]["created"].as_u64().expect("counts.created");
    assert!(
        created >= 8,
        "a fresh directory should have most of GX_PATHS created for it; created={created}\n{json:#}"
    );
    // And the two rows nothing places — `LOCK` (a running process's exclusion) and
    // `ledger/*.torn.*` (a rule, not a name) — are the ones `Layout::create` skips by construction.
    let not_placed: Vec<&str> = json["placement"]
        .as_array()
        .expect("placement")
        .iter()
        .filter(|r| r["placement"] == "not-placed")
        .filter_map(|r| r["rel"].as_str())
        .collect();
    assert_eq!(
        not_placed,
        vec!["LOCK", "ledger/*.torn.*"],
        "the rows an attach does not place are the transient one and the pattern one, and the \
         answer has to say which rather than omitting them\n{json:#}"
    );
}

/// 🔴 The control for AC-1: drop one row and the same predicate must refuse.
///
/// `req/535` §4's negative column, run against the **real** answer with one member removed rather
/// than against a hand-written fixture, so what is being doubted is the predicate this suite
/// actually uses.
#[test]
fn ac1_control_an_enumeration_missing_one_row_is_refused() {
    let dir = scratch("p1a_ac1_control");
    let (_, json) = attach(&dir);
    enumeration_is_whole(&json).expect("the whole answer passes");

    for drop in 0..gx_cli::layout::GX_PATHS.len() {
        let mut damaged = json.clone();
        let rows = damaged["placement"].as_array_mut().expect("placement");
        let removed = rows.remove(drop);
        let name = removed["rel"].as_str().unwrap_or("?").to_string();
        let why = enumeration_is_whole(&damaged).expect_err(&format!(
            "🔴 an enumeration with `{name}` removed passed the wholeness predicate — the check is \
             hollow"
        ));
        assert!(
            why.contains(&name) || why.contains("rows"),
            "the refusal should name what is missing; got {why:?}"
        );
    }
    println!(
        "AC1_CONTROL_ROWS_DROPPED_AND_REFUSED={}",
        gx_cli::layout::GX_PATHS.len()
    );

    // The other half of the same control: a word outside the three classes.
    let mut damaged = json.clone();
    damaged["placement"][0]["placement"] = Value::from("done");
    enumeration_is_whole(&damaged)
        .expect_err("a class the answer invented has to be refused, not accepted");
}

// ---------------------------------------------------------------------------
// AC-2' — the tree, before and after
// ---------------------------------------------------------------------------

/// 🔴 **AC-2'** — no tracked file and no `HEAD` moved, and every entry the tree gained is named.
#[test]
fn ac2_attach_moves_no_tracked_file_and_names_what_it_added() {
    let dir = git_fixture("p1a_ac2");
    let before_state = tracked_state(&dir);
    let before_tree = walk(&dir);

    let (_, json) = attach(&dir);

    let after_state = tracked_state(&dir);
    let after_tree = walk(&dir);
    println!(
        "AC2_TRACKED_BEFORE={:?}\nAC2_TRACKED_AFTER={:?}",
        before_state.0, after_state.0
    );
    assert_eq!(
        before_state, after_state,
        "🔴 AC-2: `git ls-files -s` and `HEAD` are the tracked digests and the history, and an \
         attach may move neither (`req/535` R-1b / N-1)"
    );

    additions_are_declared(&json, &before_tree, &after_tree)
        .unwrap_or_else(|why| panic!("🔴 AC-2' second half: {why}\n{json:#}"));

    // 🔴 The measured reason AC-2 had to become AC-2': `.gx/.gitignore` holds `*`, so git reports
    // no untracked entry at all. This arm records that fact rather than assuming it — if a later
    // build stops writing that file, this line is where a reader learns the declaration is no
    // longer the only account of what arrived.
    let status = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(&dir)
        .output()
        .expect("git status runs");
    println!(
        "AC2_GIT_STATUS_AFTER_ATTACH={:?} DECLARED_ENTRIES={}",
        String::from_utf8_lossy(&status.stdout),
        json["created_entries"].as_array().map_or(0, Vec::len)
    );
    assert!(
        json["created_entries"]
            .as_array()
            .is_some_and(|a| a.len() >= 8),
        "the answer has to name what it added; it named {}\n{json:#}",
        json["created_entries"]
    );
}

/// 🔴 The control for AC-2': a run that did touch the tree must be refused by the same predicate.
#[test]
fn ac2_control_a_touched_tree_and_a_short_declaration_are_refused() {
    let dir = git_fixture("p1a_ac2_control");
    let before_state = tracked_state(&dir);
    let before_tree = walk(&dir);
    let (_, json) = attach(&dir);

    // (a) the tracked half — one byte written into a committed file, which is the shape R-1b
    // forbids, and the same comparison the positive arm makes has to notice it.
    std::fs::write(dir.join("app.txt"), "somebody wrote here\n").expect("touch the tracked file");
    let touched_state = tracked_state(&dir);
    assert_ne!(
        before_state.0, touched_state.0,
        "🔴 the tracked-state comparison did not notice a rewritten tracked file, so the positive \
         arm proves nothing"
    );

    // (b) the declaration half — the same answer with one created entry removed.
    let after_tree = walk(&dir);
    let mut damaged = json.clone();
    let entries = damaged["created_entries"].as_array_mut().expect("entries");
    let dropped = entries.remove(0);
    let why = additions_are_declared(&damaged, &before_tree, &after_tree)
        .expect_err("a declaration missing one entry it created has to be refused");
    println!("AC2_CONTROL_DROPPED={dropped} REFUSAL={why}");

    // (c) and the other direction — a name in the declaration that is not in the tree.
    let mut invented = json.clone();
    invented["created_entries"]
        .as_array_mut()
        .expect("entries")
        .push(Value::from(".gx/a-path-nothing-made"));
    additions_are_declared(&invented, &before_tree, &after_tree)
        .expect_err("a declaration naming a path it did not make has to be refused");
}

// ---------------------------------------------------------------------------
// AC-3 — the network
// ---------------------------------------------------------------------------

/// 🔴 **AC-3** — the whole operation inside a network namespace holding a down loopback.
///
/// `tools/e2e_p3.sh`'s own isolation (`unshare -rn`), reused rather than invented, and the
/// precondition is measured in the same namespace: a TCP connect from inside it has to fail, or
/// the arm below would exit 0 on a machine where the namespace did nothing.
#[cfg(target_os = "linux")]
#[test]
fn ac3_attach_completes_with_no_network_and_the_namespace_is_proven_empty() {
    let available = Command::new("unshare")
        .args(["-rn", "true"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(
        available,
        "🔴 `unshare -rn` is the instrument this arm measures with, and it is not available. This \
         is not a skip: an unmeasured network claim is the thing `req/535` AC-3 exists to stop"
    );

    // The negative control, and it is a control of the **namespace** rather than of gx: if a
    // connect succeeds in here, the positive arm below is measuring nothing.
    let reachable = Command::new("unshare")
        .args(["-rn", "bash", "-c", "exec 3<>/dev/tcp/1.1.1.1/443"])
        .output()
        .expect("bash runs");
    println!(
        "AC3_CONTROL_CONNECT_IN_NAMESPACE_RC={:?} STDERR={:?}",
        reachable.status.code(),
        String::from_utf8_lossy(&reachable.stderr)
    );
    assert!(
        !reachable.status.success(),
        "🔴 a TCP connect succeeded inside `unshare -rn`, so the namespace is not empty and the \
         arm below would prove nothing"
    );

    let dir = scratch("p1a_ac3");
    let out = Command::new("unshare")
        .arg("-rn")
        .arg(env!("CARGO_BIN_EXE_gx"))
        .arg("attach")
        .arg("--project")
        .arg(&dir)
        .output()
        .expect("gx runs under unshare");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "🔴 AC-3: `gx attach` did not complete inside a network namespace.\nstdout={stdout}\n\
         stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: Value = serde_json::from_str(stdout.trim()).expect("one JSON object on stdout");
    enumeration_is_whole(&json)
        .unwrap_or_else(|why| panic!("🔴 the network-free run's answer is not whole: {why}"));
    println!("AC3_NETWORK_FIELD={}", json["network"]);
}

// ---------------------------------------------------------------------------
// R-1d and idempotence
// ---------------------------------------------------------------------------

/// 🔴 **R-1d** — `--project` puts `.gx/` outside the tree, and the tree gains nothing at all.
///
/// `req/535` §7 KA-1's stated escape: a repository whose CI refuses an untracked entry cannot be
/// attached to in place, so the road that does not touch the tree has to exist and has to be
/// measured rather than asserted.
#[test]
fn r1d_a_project_outside_the_tree_leaves_the_tree_with_nothing_added() {
    let tree = git_fixture("p1a_r1d_tree");
    let elsewhere = scratch("p1a_r1d_elsewhere");
    let before_tree = walk(&tree);
    let before_state = tracked_state(&tree);

    let (_, json) = attach(&elsewhere);
    enumeration_is_whole(&json).unwrap_or_else(|why| panic!("🔴 R-1d: {why}\n{json:#}"));

    let after_tree = walk(&tree);
    assert_eq!(
        before_tree, after_tree,
        "🔴 R-1d: a `.gx/` placed outside a tree must leave that tree byte for byte as it was"
    );
    assert_eq!(before_state, tracked_state(&tree));
    assert!(
        elsewhere.join(".gx").is_dir(),
        "the directory `--project` named is where `.gx/` went"
    );
    println!("R1D_GX_DIR={}", json["gx_dir"]);
}

/// 🔴 Attaching twice creates nothing the second time, and says so.
///
/// R-2b writes idempotence for the wrap route and P-1b owns that; this is the same property one
/// layer down, on placement, and it is the difference between a verb an operator may re-run and a
/// verb that quietly makes a second copy of something.
#[test]
fn attach_twice_creates_nothing_the_second_time() {
    let dir = scratch("p1a_idempotent");
    let (_, first) = attach(&dir);
    let tree_after_first = walk(&dir);
    let (_, second) = attach(&dir);
    let tree_after_second = walk(&dir);

    enumeration_is_whole(&second).unwrap_or_else(|why| panic!("🔴 second run: {why}"));
    assert_eq!(
        tree_after_first, tree_after_second,
        "a second attach changed the directory"
    );
    assert_eq!(
        second["counts"]["created"].as_u64(),
        Some(0),
        "the second attach reported creating something\nfirst={first:#}\nsecond={second:#}"
    );
    assert_eq!(
        second["created_entries"].as_array().map(Vec::len),
        Some(0),
        "the second attach named entries it did not create\n{second:#}"
    );
    let already = second["counts"]["already_present"]
        .as_u64()
        .expect("already_present");
    assert!(
        already >= 8,
        "the second attach should find what the first one made: already_present={already}\n\
         {second:#}"
    );
}

/// 🔴 The twenty-two verbs `req/535` §1 counted are still twenty-two verbs, and `attach` is the
/// twenty-third.
///
/// The list is read off `clap` rather than transcribed, for `r21_help_is_user_facing`'s reason: a
/// verb renamed by this lane would otherwise be invisible to a check that compared a copy of the
/// list with itself.
#[test]
fn the_existing_verbs_are_untouched_and_attach_is_the_new_one() {
    let out = run(gx().arg("--help"));
    assert_eq!(out.code, 0, "{}", out.stderr);
    let verbs: Vec<String> = out
        .stdout
        .split("\nCommands:\n")
        .nth(1)
        .unwrap_or_default()
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .filter(|v| *v != "help")
        .map(str::to_string)
        .collect();
    println!("VERBS={} LIST={verbs:?}", verbs.len());

    // The twenty-two `req/535` §1 read out of the `Command` enum, by name. `__demo-notes-server` is
    // hidden from `--help` and is not in this list, which is why the number here is twenty-one
    // plus `attach`.
    for verb in [
        "submit",
        "plan",
        "verify",
        "commit",
        "wrap",
        "demo",
        "limits",
        "confine",
        "verdict-checkpoint",
        "receipt",
        "log",
        "checkpoint",
        "key",
        "undo",
        "cancel",
        "escalation",
        "policy",
        "repair",
        "replay",
        "draft",
        "serve",
    ] {
        assert!(
            verbs.iter().any(|v| v == verb),
            "🔴 `{verb}` is a verb this repository had before P-1a and `--help` no longer lists it"
        );
    }
    assert!(
        verbs.iter().any(|v| v == "attach"),
        "🔴 R-1a asks for one declarative operation, and `gx attach` is it: {verbs:?}"
    );
}

// ---------------------------------------------------------------------------
// R44 lane B, item 2 — `walk`'s honesty about a subtree it could not read
// ---------------------------------------------------------------------------
//
// `req/591` §4 measured the shape of the gap: a `read_dir` failure inside `walk` returns silently,
// so a directory this process cannot list is dropped from both the "before" and the "after"
// enumeration of `.gx/` and never shows up in `created_entries`'s diff. `docs/LIMITS.md`'s standing
// disclosure names the failure as "silent about a path it could not examine", not a false answer —
// these two arms are the honest word `unreadable_entries` gives it.

/// A directory `chmod 000`'d so that listing it fails — skipped, printing why, on a machine where
/// permission bits do not bind a directory lookup (root, or an equivalent uid).
#[cfg(unix)]
fn unreadable_dir_or_skip(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let bound = std::fs::read_dir(path).is_err();
    if !bound {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod 755");
        println!(
            "R44B_ITEM2 SKIP: {} still lists under mode 000 (euid 0?); this arm measures nothing \
             here",
            path.display()
        );
    }
    bound
}

/// 🔴 The positive arm — a subtree under `.gx/` that `walk` cannot list is named, not dropped.
///
/// The bed: attach once (so `.gx/receipts/` exists), put a nested directory under it with a file
/// inside, then take away this process's ability to list that nested directory. A second attach is
/// the "after" walk `req/591` measured as losing the subtree on both sides of the diff — this arm's
/// claim is that the loss is now named rather than silent.
#[cfg(unix)]
#[test]
fn item2_unreadable_subtree_is_named_rather_than_silently_dropped() {
    let dir = scratch("r44b_item2_positive");
    let (_, first) = attach(&dir);
    assert_eq!(
        first["unreadable_entries"].as_array().map(Vec::len),
        Some(0),
        "a freshly attached project has nothing unreadable yet\n{first:#}"
    );

    let deep = dir.join(".gx").join("receipts").join("deep");
    std::fs::create_dir(&deep).expect("a nested directory under a declared one");
    std::fs::write(deep.join("inside.txt"), b"content walk must not see\n")
        .expect("a file the blinded walk will lose");

    if !unreadable_dir_or_skip(&deep) {
        return;
    }
    let out = run(gx().arg("attach").arg("--project").arg(&dir).arg("--json"));
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&deep, std::fs::Permissions::from_mode(0o755))
            .expect("restore mode");
    }
    assert_eq!(
        out.code, 0,
        "🔴 req/38 §148: a subtree this process cannot list does not move the exit — the second \
         attach still succeeds and says so in the field, not in the status\nstdout={}\nstderr={}",
        out.stdout, out.stderr
    );
    let second = out.json();
    println!("R44B_ITEM2_POSITIVE={second:#}");

    let unreadable: Vec<&str> = second["unreadable_entries"]
        .as_array()
        .unwrap_or_else(|| panic!("🔴 `unreadable_entries` is not an array: {second:#}"))
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        unreadable.contains(&".gx/receipts/deep"),
        "🔴 the blinded subtree `.gx/receipts/deep` is not named in `unreadable_entries`={unreadable:?} \
         — this is the exact silent loss `req/591` §4 measured\n{second:#}"
    );

    // 🔴 The symmetric-loss half of req/591's finding: the file inside the blinded directory
    // reaches neither walk, so it is absent from `created_entries` on both runs — and that
    // omission is now explained by `unreadable_entries` rather than left to look like nothing was
    // ever there.
    let created: Vec<&str> = second["created_entries"]
        .as_array()
        .expect("created_entries")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        !created.iter().any(|e| e.contains("inside.txt")),
        "the file inside a directory this process could not list should not appear as created: \
         {created:?}"
    );
}

/// 🔴 The negative control — a run that has nothing unreadable must not invent a name.
///
/// Without this, the positive arm above could pass against an implementation that always fills
/// `unreadable_entries` with something, which is the mirror failure of the one being fixed.
#[test]
fn item2_control_a_fully_readable_tree_names_nothing_as_unreadable() {
    let dir = scratch("r44b_item2_control");
    let (_, first) = attach(&dir);
    let (_, second) = attach(&dir);
    for (label, json) in [("first", &first), ("second", &second)] {
        let unreadable = json["unreadable_entries"].as_array().unwrap_or_else(|| {
            panic!("🔴 `unreadable_entries` is not an array ({label}): {json:#}")
        });
        assert!(
            unreadable.is_empty(),
            "🔴 the {label} attach touched nothing unreadable and still named {unreadable:?}\n{json:#}"
        );
    }
}
