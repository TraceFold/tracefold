// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-021 (FR-021) — the log is append-only, and its public surface says so.
//!
//! AC-021 verbatim: "Given: gx-log's public API surface, already appended-to. When: the API list
//! is walked by a snapshot test. Then: no public function exists for editing or deleting an entry
//! (append/read family only)." (sem: SEM-gx-log-103)
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
//!    stronger half. A name list is a promise about vocabulary; "one writer, and it appends" is a
//!    property of the type, and it is what makes "no public function exists for editing or deleting" (sem: SEM-gx-log-104)
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
///   `verify_inclusion_of`, `root_of_inclusion` (H-09), `prove_consistency`, `verify_consistency`,
///   `checkpoint_signing_bytes`, `unsigned_checkpoint`
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
/// 🔴 M7 hand 5 adds one, `cached_subtree_roots`, and it **reads** (**FR-M7-2 option A**). It answers with (sem: SEM-gx-log-105)
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
    // 🔴 **R6 / DR-43-11** (`src/head.rs`): `HeadStore::at` names the file and the origin its
    // checkpoints are signed under. A **constructor** in the sense `LedgerStore::open` is one, and
    // deliberately not an opener — it touches no filesystem until `read` or `write` is called, so
    // that a caller holding one has not yet decided whether this project keeps a head.
    "at",
    "audit_verdict_chain",
    "cached_subtree_roots",
    "checkpoint_signing_bytes",
    "checkpoints",
    // 🔴 **Phase B / `req/682`**: `witness_audit::classify_extension` answers whether an older
    // checkpoint is a prefix of a newer one, given the consistency proof over their two sizes. A
    // **read** in FR-021's sense — a pure function of two checkpoints and a proof, touching no log —
    // in `verify_consistency`'s standing: it names a fork rather than deciding storage.
    "classify_extension",
    // 🔴 **R6 / DR-43-11**: `head::compare` answers whether the project in front of the caller is
    // behind the head it has already published. A **read** in FR-021's sense and a pure function —
    // it takes numbers and returns a reason, touches no log, and is separate from the store so
    // that the comparison has one implementation for `open`, `catch_up` and `gx repair`.
    "compare",
    // 🔴 **R6 / DR-43-11**: `RolledBack::detail` is the one sentence every face prints. A **read**
    // of a value, in `Error::kind`'s standing — `req/38` §156 ruling 2(a) fixes the *code* at
    // `LEDGER_DISAGREES` on all five faces, so the difference has to live in the detail and the
    // detail has to have exactly one spelling.
    // 🔴 **R7 / `req/232` M-02**: `head::declaration_digest` is the digest a project's
    // `.gx/VERSION` is recorded under. A **read** in FR-021's sense — a pure function of some bytes
    // — and it lives here because 41 §6 puts every hash in gx-canon and gx-cli, which owns the
    // layout, deliberately does not name that crate.
    "declaration_digest",
    // 🔴 **R9 / `req/236` H-04**: `head::declaration_lines` splits `.gx/VERSION` into the lines
    // that are `key=value` and the lines that are not, after decoding (byte-order marks and UTF-16
    // included), trimming and dropping blanks. A **read** in FR-021's sense — a pure function of
    // some bytes — and it is public because the *parse* and the *digest* have to be one reading:
    // `req/236` H-04 measured them being two, with the parse in front and five byte shapes an
    // ordinary editor produces stopping every verb before the digest was ever taken.
    "declaration_lines",
    "detail",
    // 🔴 **Phase B / `req/682`**: `witness_audit::detect_equivocation` scans a set of signed
    // checkpoints for a matching `tree_size` under one origin with a differing root. A **read** in
    // FR-021's sense — a pure function over a slice, touching no log and deciding nothing about
    // storage — in `audit_verdict_chain`'s standing: it answers *what contradicts itself*.
    "detect_equivocation",
    "entries",
    "entry",
    // 🔴 **R6 / DR-43-11**: `PersistedHead::floor` takes the numbers a door compares off the
    // document. A **read**, and fallible because a `journal_head` this process cannot decode is a
    // corrupted detector rather than an absent one.
    "floor",
    // 🔴 **R6 / DR-43-11**: hex, both ways, for the 32-byte chain head the head file records. A
    // **read** in FR-021's sense (pure), and here rather than in `gx-canon` because it is a
    // rendering of bytes for a JSON document and not a canonical encode — 41 §6 governs the bytes a
    // digest is taken over, and nothing here is hashed.
    "from_hex",
    "is_empty",
    // 🔴 **DR-43-7** (`req/38` §153): whether this store came through `open_read_only`. A **read**
    // in FR-021's sense — it answers about the handle and touches no log — and it exists because
    // one `LedgerStore` field is handed to both readers and writers: `gx_engine`'s engine re-opens
    // a moved ledger read-only for a `GET` and has to notice, on the next write, that it is holding
    // the reader's door.
    "is_read_only",
    // 🔴 M6 hand 5, **M6-09**: `Error::kind()` answers which of `ERROR_KINDS` a refusal is. A
    // **read** in FR-021's sense and not an entry at all — it names a value this crate already
    // produced and touches no log. The array it answers from arrived for the same ruling: 44 §2.3's
    // twelve `gx_code`s need a denominator, and a refusal vocabulary nobody declared is a
    // vocabulary nobody can prove was covered (E-M2-23's "declare a per-crate Error vocabulary table in one place" (sem: SEM-gx-log-106),
    // unpaid here until now).
    "kind",
    "leaf",
    "leaf_hash",
    "leaf_hashes",
    "len",
    "log",
    "new",
    "node_hash",
    // 🔴 **R8 / `req/234` H-02**: the **read** the digest above is taken over --
    // `.gx/VERSION` reduced to what it declares (line endings, surrounding space, blank lines
    // and a BOM removed, `key = value` trimmed on both halves). A pure function of some bytes,
    // exactly as FR-021 uses the word: it opens nothing and appends nothing. It exists because
    // the eighth audit took a provably intact project offline with a trailing newline.
    "normalise_declaration",
    "open",
    // 🔴 **DR-43-7** (`req/38` §153, `req/215` H-03): `open` is a writer's door and repairs a torn
    // tail; this one does not create, does not truncate and does not repair. A **read**, and the
    // one `gx log proof`, `gx replay`, `gx verdict-checkpoint list` and `gx serve`'s start-up gate
    // now use — `req/215` measured all four shortening a ledger from 120 bytes to 0 on the way past.
    "open_read_only",
    // 🔴 **R5 / `req/227` M-04**: the same reader's door for a chain that is **not there**. A
    // **read** in FR-021's sense and, unlike `open_read_only`, one that cannot even open a file —
    // it answers "no file, no checkpoints" without creating one. It exists because `gx repair`'s
    // report opened on a narrower set of projects than `gx repair --yes` did: a project missing
    // `.gx/ledger/journal.verdicts` answered `INTERNAL` to the diagnosis and had the file grown
    // back by the repair.
    "open_read_only_or_absent",
    // 🔴 **R6 / DR-43-11**: the namespace `PersistedHead::checkpoint` is signed under, read off the
    // store. A **read**. It travels with the store rather than being a constant here because 42
    // §3.11 makes the origin "what stops a checkpoint of one log from verifying against another's
    // key", and an operator running two logs needs two namespaces.
    "origin",
    "path",
    // 🔴 **R6 / `req/229` M-02**: whether a `LedgerStore` has a file behind it. A **read**, and it
    // exists because `open_read_only_or_absent` can now answer with a store over a path that is not
    // there — `gx repair`'s report says `ledger_present: false` rather than refusing to open.
    "present",
    "prove_consistency",
    "prove_inclusion",
    "prove_inclusion_at",
    // 🔴 **DR-43-7** (`req/215` M-05): where `open` copied the file before it removed the torn
    // tail. A **read** — it answers about what a previous open did — and the consumer is `gx
    // serve`'s start-up line, which said nothing about a silent repair until now.
    "quarantined",
    // 🔴 **R6 / DR-43-11**: `HeadStore::read` returns the recorded head, or `None` for a project
    // that has never recorded one. A **read**, and the one place the difference between "absent"
    // and "malformed" is decided: absence is safe (this project made no statement about its past)
    // and a broken document is a refusal (somebody replaced the detector).
    "read",
    // 🔴 **DR-43-6** (`req/215` H-02): how many bytes of the file this store has turned into
    // leaves. A **read**, and the ledger's half of the change-detector that lets `Engine::catch_up`
    // notice a ledger that moved while the journal did not.
    "read_offset",
    "recovery",
    "root",
    "root_at",
    // 🔴 **H-09** (`req/222` §4 row 6): the root an inclusion proof and its leaf *reach*, which is
    // a **read** of the proof and touches no log. `verify_inclusion_of` became one equality over
    // it, so the surface grew by a name and not by a second walk. It exists because a receipt
    // older than the anchor needs its own root as the left-hand end of RFC 6962 §2.1.2's
    // consistency check, and computing it from the receipt is what keeps that end unforgeable.
    "root_of_inclusion",
    // 🔴 **R3 / `req/222` H-05** — a read: it re-reads the last framed record and compares.
    // 🔴 **R7 / DR-43-11 (b)**: `PersistedHead::stated` renders this document's own numbers as the
    // witness they are signed as. A **read** of a value — it hashes nothing and touches no log —
    // and it is the half of the signature check that makes an edited field fail by construction.
    "stated",
    "tail_unchanged",
    "tile",
    // 🔴 **R6 / DR-43-11**: see `from_hex`.
    "to_hex",
    "unsigned_checkpoint",
    "unsigned_verdict_checkpoint",
    "verdict_checkpoint_signing_bytes",
    "verify_consistency",
    "verify_inclusion",
    "verify_inclusion_of",
    // 🔴 **R7 / DR-43-11 (b)**: `PersistedHead::witness_payload` is the byte string
    // `witness_signature` covers. A **read**, and public for `checkpoint_signing_message`'s reason
    // one crate up: a verifier that has to rebuild a message from a doc comment is two
    // implementations checking two different things.
    "witness_payload",
    // 🔴 **R6 / DR-43-11**: `HeadStore::write` installs the recorded head. **The one entry in this
    // list that is neither an append nor a read**, and it is declared as the exception rather than
    // filed under a word that would not fit: `.gx/checkpoints/head.json` is a *replace*, done by
    // temporary file + fsync + rename + directory fsync, because a high-water mark is one value and
    // an append-only history of it would have the same prefix problem the mark exists to solve.
    // FR-021's append/read dichotomy is about the **log**, and this file is not the log — it is a
    // statement *about* the log, and `LedgerStore`/`TileLog` are untouched by it.
    "write",
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
/// # Why this reads "every writer is `append`" rather than "there is one writer" (sem: SEM-gx-log-107)
///
/// Hand 2 asserted `writers.len() == 1`, because gx-log held one type that could be written to.
/// Hand 3 adds a second -- `LedgerStore`, the durable log of AC-069, which wraps the in-memory
/// `TileLog` -- so a literal "one" (sem: SEM-gx-log-108) would now be a count of types rather than a statement about
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
/// req/38 §9 rules "hand 2 does not use rs_merkle; it is a homegrown merkle" (sem: SEM-gx-log-109) on three grounds -- gx hashes with
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
