//! AC-021 (FR-021) — the log is append-only, and its public surface says so.
//!
//! AC-021 逐語: 「Given: 追記済みgx-log公開API表面。When: API一覧をスナップショットテストで走査。
//! Then: エントリ改変・削除用のpublic関数が存在しない（append/read系のみ）。」
//!
//! # Why a source scan rather than a doc test
//!
//! FR-021's claim is about what the crate *offers*, not about what any one caller does. A runtime
//! test can only show that the functions it happens to call behave; a function that edits an entry
//! would pass every one of them by never being called. So the surface itself is the subject, read
//! off the source the way `gx-canon`'s `ac_014.rs` reads the workspace, and asserted two ways:
//!
//! 1. **snapshot** — the set of public function names equals a list declared here. A new function
//!    fails this file until somebody writes its name down, which is the point: the moment where a
//!    mutating API would be added is the moment a human has to state that it is not one.
//! 2. **shape** — exactly one public function takes `&mut self`, and it is `append`. That is the
//!    stronger half. A name list is a promise about vocabulary; 「one writer, and it appends」 is a
//!    property of the type, and it is what makes 「改変・削除用の public 関数が存在しない」
//!    checkable rather than a matter of reading each signature and being satisfied.
//!
//! Neither is a claim about *storage*. Whether the bytes on disk can be edited is AC-069's and
//! hand 3's question (NFR-009); this file is about the API a caller is handed.

use std::path::{Path, PathBuf};

/// The whole public function surface of gx-log after hand 3, sorted.
///
/// Every name is append or read. Grouped by module in the comments, so a reviewer can see which
/// file each one comes from without opening it:
///
/// * `tile.rs` — `new`, `append`, `len`, `is_empty`, `entry`, `entries`, `leaf_hashes`, `root`,
///   `root_at`, `tile`, `leaf`, `leaf_hash`, `node_hash`
/// * `proof.rs` — `prove_inclusion`, `prove_inclusion_at`, `verify_inclusion`,
///   `verify_inclusion_of`, `prove_consistency`, `verify_consistency`, `checkpoint_signing_bytes`,
///   `unsigned_checkpoint`
/// * `store.rs` (hand 3, AC-069) — `open`, `append`, `log`, `path`, `recovery`, and
///   `AppendOutcome::entry`, whose name the tree already declares
///
/// Five names arrived with hand 3 and every one of them reads: `open` builds the value, `log` /
/// `path` / `recovery` answer questions about it. The one writer that file adds is `append`, which
/// is the subject of the shape assertion below.
///
/// Hand 5 adds one, `verify_inclusion_of`, and it reads: `verify_inclusion` became a wrapper around
/// it so that a third party verifying a `CommitReceipt` offline (AC-070) can ask about a
/// `LedgerLeaf` it rebuilt from the receipt, rather than about a `LedgerEntry` only the log can
/// produce. No behaviour moved with it — the two share one body — and nothing about this list's
/// promise changes: the surface is still append and read.
/// 🔴 M7 hand 5 adds one, `cached_subtree_roots`, and it **reads** (**FR-M7-2 案 A**). It answers with
/// the roots of the completed 256-leaf tiles the log has folded — derived state, never a second way
/// in.
/// It is public for a reason this suite is itself an instance of: a value nothing can look at is a
/// value nothing can check, and `tests/incremental_inclusion.rs` compares the vector and its length
/// against a fold of the leaves. The name carries no mutating verb and the function takes `&self`,
/// so the two shape assertions below hold without an exemption.
/// 🔴 **FR-M04** (M7 hand 6) adds four names, and all four are reads or appends in FR-021's sense:
///
/// * `audit_verdict_chain` — a **read** over a slice of checkpoints and two numbers a verifier
///   already holds. It touches no log, opens no file and decides nothing; it answers *what does
///   not add up*, which is the same standing as `verify_inclusion`.
/// * `checkpoints` — a read of what the parallel store replayed, the counterpart of `entries`.
/// * `unsigned_verdict_checkpoint` — a read of a tally and a tree head, folded into a statement
///   nobody has signed yet. The same shape, and the same non-signing, as `unsigned_checkpoint`.
/// * `verdict_checkpoint_signing_bytes` — the core a signature covers, beside
///   `checkpoint_signing_bytes` and for the same reason: a verifier needs to rebuild the message.
///
/// `open`, `append`, `path` and `recovery` are **not** new entries here even though
/// `VerdictCheckpointStore` offers all four — this list is by name and the names already exist.
/// What does move is [`EXPECTED_WRITERS`], because a second `&mut self` receiver is a second
/// writer whether or not it shares a spelling with the first.
const PUBLIC_FUNCTIONS: &[&str] = &[
    "append",
    "audit_verdict_chain",
    "cached_subtree_roots",
    "checkpoint_signing_bytes",
    "checkpoints",
    "entries",
    "entry",
    "is_empty",
    // 🔴 M6 hand 5, **M6-09**: `Error::kind()` answers which of `ERROR_KINDS` a refusal is. A
    // **read** in FR-021's sense and not an entry at all — it names a value this crate already
    // produced and touches no log. The array it answers from arrived for the same ruling: 44 §2.3's
    // twelve `gx_code`s need a denominator, and a refusal vocabulary nobody declared is a
    // vocabulary nobody can prove was covered (E-M2-23's 「crate 毎 Error 語彙表を 1 箇所宣言」,
    // unpaid here until now).
    "kind",
    "leaf",
    "leaf_hash",
    "leaf_hashes",
    "len",
    "log",
    "new",
    "node_hash",
    "open",
    "path",
    "prove_consistency",
    "prove_inclusion",
    "prove_inclusion_at",
    "recovery",
    "root",
    "root_at",
    "tile",
    "unsigned_checkpoint",
    "unsigned_verdict_checkpoint",
    "verdict_checkpoint_signing_bytes",
    "verify_consistency",
    "verify_inclusion",
    "verify_inclusion_of",
];

/// Verbs that would mean the log is not append-only.
///
/// Matched as substrings of a function name, so `remove_entry`, `truncate` and `set_root` are all
/// caught. The list is deliberately wider than anything anybody would plausibly write: a ban that
/// only names the API it already knows about bans nothing.
const MUTATING_VERBS: &[&str] = &[
    "remove",
    "delete",
    "truncate",
    "clear",
    "drain",
    "pop",
    "erase",
    "replace",
    "overwrite",
    "rewrite",
    "edit",
    "update",
    "set_",
    "insert_at",
    "amend",
    "purge",
    "prune",
    "rollback",
    "revoke",
];

/// Merkle libraries. M2H1-6 (`req/38_ERRATA_2026-08-07.md` §9) rules the tree self-written, so
/// none of these may appear in the manifest -- and none of their code appears anywhere, which is
/// the copy ban the same ruling restates and which a scan cannot prove.
const MERKLE_LIBRARIES: &[&str] = &[
    "rs_merkle",
    "rs-merkle",
    "merkle_light",
    "merkletree",
    "ct-merkle",
    "rct",
    "sha2",
];

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn sources(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return out;
    }
    for entry in std::fs::read_dir(dir).expect("readable") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(sources(&path));
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
    out
}

/// Code lines only. Doc comments in this crate quote 42 §3.11 and name the rulings; a scan that
/// read them would report the documentation of a rule as a breach of it (`ac_014.rs`, same fix).
fn code_lines(path: &Path) -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(path).expect("source is UTF-8");
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let t = line.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .map(|(n, line)| (n + 1, line.to_string()))
        .collect()
}

/// Every `pub fn` / `pub const fn` in `src/`, by name and by its **whole** signature.
///
/// `pub(crate) fn` is not matched, and that is correct: the merkle arithmetic below the surface is
/// crate-private on purpose, and AC-021 asks what the crate *offers*.
///
/// The signature is accumulated until its parentheses balance, because rustfmt wraps a long one
/// across lines and `append`'s is wrapped. A scan that read only the `pub fn` line saw
/// `pub fn append(` and reported **zero** mutable receivers -- a check against fail-open that was
/// itself fail-open, the same shape as the `ci.sh` member guard req/50 §5 found sitting after
/// `exit 0`. It is caught here rather than in review because
/// `ac_021_the_scan_actually_reads_a_surface` and the receiver count disagreed.
fn public_functions() -> Vec<(String, String)> {
    let src = crate_root().join("src");
    let files = sources(&src);
    assert!(
        files.len() >= 2,
        "found {} source files under {}; the scan is looking in the wrong place",
        files.len(),
        src.display()
    );

    let mut out = Vec::new();
    for path in files {
        let lines = code_lines(&path);
        for (position, (_, line)) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed
                .strip_prefix("pub fn ")
                .or_else(|| trimmed.strip_prefix("pub const fn "))
            else {
                continue;
            };
            let name = rest
                .split(['(', '<', ' '])
                .next()
                .unwrap_or_default()
                .to_string();

            // Gather until the argument list closes. `depth` starts at zero and the first line
            // always opens it, so a signature on one line ends on that line.
            let mut signature = String::new();
            let mut depth = 0i32;
            for (_, more) in lines.iter().skip(position) {
                signature.push_str(more.trim());
                signature.push(' ');
                depth += i32::try_from(more.matches('(').count()).expect("small")
                    - i32::try_from(more.matches(')').count()).expect("small");
                if depth <= 0 {
                    break;
                }
            }
            out.push((name, signature.trim().to_string()));
        }
    }
    out
}

#[test]
fn ac_021_the_public_surface_is_the_declared_one() {
    let mut names: Vec<String> = public_functions().into_iter().map(|(n, _)| n).collect();
    names.sort();
    names.dedup();

    let expected: Vec<String> = PUBLIC_FUNCTIONS.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(
        names, expected,
        "gx-log's public function surface changed. Every entry must be append or read (FR-021); \
         declare the new name in PUBLIC_FUNCTIONS and say in its doc which of the two it is."
    );
    println!("AC021_PUBLIC_FUNCTIONS={}", names.len());
}

#[test]
fn ac_021_no_public_function_is_named_for_a_mutation() {
    let mut violations = Vec::new();
    for (name, line) in public_functions() {
        for verb in MUTATING_VERBS {
            if name.contains(verb) {
                violations.push(format!("{name} (`{verb}`): {line}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "an append-only log offers no function that edits or removes an entry (FR-021, AC-021):\n{}",
        violations.join("\n")
    );
}

/// The shape assertion: every writer appends, and there is one per log type.
///
/// A `&mut self` method is the only way a caller can change what the log holds, so this is a check
/// on the *capability* rather than on the vocabulary.
///
/// # Why this reads 「every writer is `append`」 rather than 「there is one writer」
///
/// Hand 2 asserted `writers.len() == 1`, because gx-log held one type that could be written to.
/// Hand 3 adds a second -- `LedgerStore`, the durable log of AC-069, which wraps the in-memory
/// `TileLog` -- so a literal 「one」 would now be a count of types rather than a statement about
/// capability, and it would go on being satisfied by a crate that grew a `set_root` on a third
/// type as long as the second one lost its writer. The invariant hand 2 was reaching for is that
/// **the only thing any writer does is append**, so that is what is asserted, and the count is
/// pinned separately against a declared number so a new writer still has to be written down here.
///
/// This is a check answering the same question in a stronger form, not a check relaxed to pass:
/// the previous assertion is implied by this one for any crate with a single log type, and the new
/// one refuses a mutating method the old one would have refused only by accident of counting.
// 🔴 **FR-M04** (M7 hand 6): **three**. `VerdictCheckpointStore::append` is the third, and it is
// the one that says why this number is pinned separately from the name check above — it shares the
// spelling `append` with the other two, so a list keyed on names could not have noticed it arrive.
const EXPECTED_WRITERS: usize = 3; // TileLog::append, LedgerStore::append, VerdictCheckpointStore::append

#[test]
fn ac_021_every_public_function_with_a_mutable_receiver_appends() {
    let writers: Vec<(String, String)> = public_functions()
        .into_iter()
        .filter(|(_, line)| line.contains("&mut self"))
        .collect();

    let stray: Vec<&(String, String)> = writers.iter().filter(|(n, _)| n != "append").collect();
    assert!(
        stray.is_empty(),
        "an append-only log offers no writer but `append`; found:\n{}",
        stray
            .iter()
            .map(|(n, l)| format!("{n}: {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(
        writers.len(),
        EXPECTED_WRITERS,
        "gx-log declares {EXPECTED_WRITERS} writers and the scan found {}; a new one must be \
         written down here:\n{}",
        writers.len(),
        writers
            .iter()
            .map(|(n, l)| format!("{n}: {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The scan is not vacuous: it finds the surface it is supposed to be reading.
///
/// Without this, a `public_functions()` that silently returned nothing would make all three tests
/// above pass -- req/08 N-1's disease applied to a static check, and the same reason `ac_014.rs`
/// asserts its own scan finds the permitted uses.
#[test]
fn ac_021_the_scan_actually_reads_a_surface() {
    let found = public_functions();
    assert!(
        found.len() >= PUBLIC_FUNCTIONS.len(),
        "the scan found {} public functions but {} are declared; a check that reads nothing \
         reports nothing",
        found.len(),
        PUBLIC_FUNCTIONS.len()
    );
    assert!(
        found.iter().any(|(n, _)| n == "append"),
        "the scan did not find `append`, which is the one function AC-021 expects to exist"
    );
}

/// M2H1-6: the tree is gx's own, and the manifest shows it.
///
/// req/38 §9 rules 「手 2 は rs_merkle を使わず自前 merkle」 on three grounds -- gx hashes with
/// BLAKE3 (35 DR-3) while `rs_merkle` drags in a second SHA-256 implementation (req/50 §6 measured
/// the duplicated `sha2` subtree), 42 §3.11's domain separation is no library's default, and the
/// licence claim did not check out at three points. The dependency was dropped when the self-written
/// tree landed; this test is what keeps it dropped.
///
/// It does not and cannot prove the *copy* ban. No scan distinguishes code written from RFC 6962's
/// text from code transcribed out of a library; that ban is a discipline, recorded in
/// `Desktop/GitRepo/REFERENCES.md` and in req/51 §3.
#[test]
fn ac_021_no_merkle_library_is_declared() {
    let manifest = crate_root().join("Cargo.toml");
    let mut violations = Vec::new();
    for (n, line) in code_lines(&manifest) {
        let name = line
            .split(['=', '.', ' '])
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"');
        if MERKLE_LIBRARIES.contains(&name) {
            violations.push(format!("{}:{n}: {}", manifest.display(), line.trim()));
        }
    }
    assert!(
        violations.is_empty(),
        "M2H1-6 (req/38 §9) rules the merkle tree self-written; the manifest declares:\n{}",
        violations.join("\n")
    );
}
