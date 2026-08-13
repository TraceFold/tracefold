//! What hand 3 is allowed to be, measured from the source: one blob store, one second ceiling, one
//! reconstruction of Σ, and the two absences that make **E-M5-2** structural rather than promised.
//!
//! req/78 §6.2 手 3 is the load — 「store / escrow / replay(遷移では T-10b の器)+ AC-039」 — and
//! req/38 §37 settles the three rulings this file measures the shape of:
//!
//! > **M5-02 採(a)**=**E-M5-2**: replay は **Σ のみを再構成する read-only 操作**・AC-039 の「結果状態」
//! > =Σ(状態表+ledger root+escrow index)と読む。adapter は呼ばない(機械検査つき)。
//!
//! > **M5-05 採(a)**: CID キーの blob store 1 本が `PlannedDelta` と `inverse_delta` の両方を持つ
//! > (M4H6-3「既知 CID は参照のみ」を内包)。
//!
//! > **M5-20 採(a)+(c)**: engine 受取口の decode 前 byte 上限 1 箇所+契約行 1:1 probe(M4H2-8 形)。
//!
//! # Why these are scans and not behaviour
//!
//! Three of the claims above are about **absence and uniqueness** — one store, one ceiling per
//! receiving mouth, no adapter inside a replay — and a behavioural probe cannot see any of them: a
//! second store that nobody called, a second ceiling that agreed with the first by luck, and an
//! adapter call on a path no test walked would all pass a run. §30's lesson applies in the other
//! direction as well, so `tools/verify_m5h3.sh` §4 adds each of these presences by mutation and
//! prints which probe notices.
//!
//! The behavioural halves are `tests/blob_store.rs` (the store, both ends of its ceiling, the escrow
//! round-trip) and `tests/ac_039.rs` (Σ, bit-equality, and the control experiment).

mod support;

use support::{read_repo, repo_root};

/// The lines of a source file that are not comments (§30: 「invocation 行のみを対象にする」).
///
/// This crate's documentation discusses adapters, ceilings and journals at length, and a scan that
/// read prose would report the discussion as the thing. The same filter `engine_shape.rs` uses.
fn code_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && !l.starts_with("*") && !l.is_empty())
        .collect()
}

/// Every `.rs` file under a directory, recursively.
fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// The body of a function, from its declaration to the line that closes it at the same indent.
///
/// Written by indentation rather than by brace counting because the thing being asked is 「what does
/// this one function reach for」, and a brace counter that swallowed a nested closure would answer a
/// different question quietly. The declaration line is included so that a signature can be scanned
/// with the same helper.
fn function_body<'a>(text: &'a str, declaration: &str) -> Option<Vec<&'a str>> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.contains(declaration))?;
    let indent = lines[start].len() - lines[start].trim_start().len();
    let closing = format!("{}}}", " ".repeat(indent));
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| **l == closing)
        .map_or(lines.len(), |(i, _)| i);
    Some(lines[start..=end.min(lines.len() - 1)].to_vec())
}

// ---------------------------------------------------------------------------
// M5-05 採(a): one blob store
// ---------------------------------------------------------------------------

/// **M5-05 採(a)**: there is **one** content-addressed store, and it is in `store.rs`.
///
/// The ruling's word is 「blob store **1 本**」, and 「1 本」 is what makes M4H6-3 (「residual CID が
/// 既存 delta CID と一致するのは保管の一度性の裏返し」) an implementation rather than a coincidence:
/// two stores holding the same CID would each think they were the one keeping it. 41 §2 fixes the
/// module list, so 「in `store.rs`」 is not a preference — a blob store in `replay.rs` would make the
/// read-only side of **E-M5-2** own a writer.
#[test]
fn the_blob_store_is_declared_once_and_in_store_rs() {
    let mut declarations: Vec<String> = Vec::new();
    for file in walk(&repo_root().join("crates/gx-engine/src")) {
        let text = std::fs::read_to_string(&file).expect("a source file is readable");
        for line in code_lines(&text) {
            if line.starts_with("pub struct BlobStore") {
                declarations.push(format!(
                    "{}: {line}",
                    file.file_name().expect("a named file").to_string_lossy()
                ));
            }
        }
    }
    println!(
        "BLOB_STORE_DECLARATIONS={} {declarations:?}",
        declarations.len()
    );
    assert_eq!(
        declarations.len(),
        1,
        "M5-05 採(a) asks for one blob store and these exist: {declarations:?}"
    );
    assert!(
        declarations[0].starts_with("store.rs:"),
        "41 §2's four modules put storage in `store.rs`: {declarations:?}"
    );
}

// ---------------------------------------------------------------------------
// M5-20 採(a): the second ceiling, and the contract row that names it
// ---------------------------------------------------------------------------

/// 🔴 **M5-20 採(a)**: each receiving mouth declares its ceiling **once**.
///
/// The engine has two places bytes it did not write come back in: the journal (hand 1's
/// `MAX_RECORD_BYTES`) and the blob store (this hand's `MAX_BLOB_BYTES`). The ruling is 「decode 前
/// byte 上限 1 箇所」 **per mouth**, and the reason two constants are right where one might look
/// tidier is hand 1's: two files with different contents and different writers must be able to move
/// independently, and sharing a constant would make one ceiling a statement about the other.
///
/// What is being refused here is a *third* declaration — a local `const` inside a function, a second
/// copy in another module — because a ceiling that is declared twice is a ceiling that can be raised
/// in one place.
#[test]
fn each_receiving_mouth_declares_one_ceiling() {
    let mut found: Vec<(String, String)> = Vec::new();
    for file in walk(&repo_root().join("crates/gx-engine/src")) {
        let text = std::fs::read_to_string(&file).expect("a source file is readable");
        for line in code_lines(&text) {
            for name in ["MAX_RECORD_BYTES", "MAX_BLOB_BYTES"] {
                if line.starts_with(&format!("pub const {name}")) {
                    found.push((
                        name.to_string(),
                        format!(
                            "{}: {line}",
                            file.file_name().expect("a named file").to_string_lossy()
                        ),
                    ));
                }
            }
        }
    }
    println!("ENGINE_CEILINGS={found:?}");
    for name in ["MAX_RECORD_BYTES", "MAX_BLOB_BYTES"] {
        let hits: Vec<&(String, String)> = found.iter().filter(|(n, _)| n == name).collect();
        assert_eq!(
            hits.len(),
            1,
            "M5-20 採(a): `{name}` is declared once, and these exist: {hits:?}"
        );
    }
}

/// **M4H2-8** form: the contract row and the one declaration name each other.
///
/// 「契約表の行と定数宣言箇所の 1:1 probe」 (req/38 §30, and §37 asks for it again here). Neither end
/// is a contract on its own — a row naming a constant nobody declares is a promise with no
/// mechanism, and a constant no row mentions is a mechanism with no promise. The row that has to
/// name it is `get`'s, because `get` is the **decode 前** side: the ceiling is checked against the
/// size on disk before the bytes are read, which is the whole content of 「decode 前」.
#[test]
fn the_blob_contract_row_names_the_ceiling() {
    let store = read_repo("crates/gx-engine/src/store.rs");
    let row = store
        .lines()
        .find(|l| {
            l.trim_start().starts_with("//! | `get` |")
                || l.trim_start().starts_with("/// | `get` |")
        })
        .expect("the blob store's contract table has a `get` row");
    println!("BLOB_GET_CONTRACT_ROW={}", row.trim());
    assert!(
        row.contains("MAX_BLOB_BYTES"),
        "the `get` contract row does not name the constant that decides its refusal: {row}"
    );
    let put = store
        .lines()
        .find(|l| {
            l.trim_start().starts_with("//! | `put` |")
                || l.trim_start().starts_with("/// | `put` |")
        })
        .expect("the blob store's contract table has a `put` row");
    assert!(
        put.contains("MAX_BLOB_BYTES"),
        "the `put` row does not name it either, so the two ends of the ceiling are not one \
         statement: {put}"
    );
}

/// **M5H1-6's condition**: the store does not declare a third spelling of a recovery.
///
/// §38 ruled 採(a) -- 「`gx_log::Recovery` の再輸出=1 綴り維持」 -- **conditionally**: 「手 3 の store が
/// 3 つ目の失敗型を作らない事を条件に」. This is that condition, measured. It holds for a structural
/// reason rather than by restraint: a directory of whole files has no torn tail, so a blob is either
/// present and complete, absent, or the wrong size -- and the last two are refusals in
/// [`gx_engine::Error`] rather than a report about a damaged file. One idea, one word.
#[test]
fn the_store_declares_no_second_recovery() {
    let mut declarations: Vec<String> = Vec::new();
    for file in walk(&repo_root().join("crates/gx-engine/src")) {
        let text = std::fs::read_to_string(&file).expect("a source file is readable");
        for line in code_lines(&text) {
            if line.starts_with("pub struct") && line.contains("Recovery") {
                declarations.push(format!(
                    "{}: {line}",
                    file.file_name().expect("a named file").to_string_lossy()
                ));
            }
        }
    }
    println!("RECOVERY_DECLARATIONS_IN_ENGINE={declarations:?}");
    assert!(
        declarations.is_empty(),
        "M5H1-6's condition: the engine re-exports gx-log's `Recovery` and declares none of its \
         own: {declarations:?}"
    );
}

// ---------------------------------------------------------------------------
// E-M5-2: Σ is reconstructed in replay.rs, and nothing there can reach a substrate
// ---------------------------------------------------------------------------

/// **E-M5-2**: the reconstruction of Σ lives in `replay.rs`.
///
/// 42 §1.3-3 keys the engine's state table on `TransformationId` and 43 §7-1 makes the journal the
/// thing it is rebuilt from, so 「Σ を再構成する」 is the reading side of the same file that reads the
/// journal's bytes. A `Sigma` built in `pipeline.rs` would be Σ built by the code that also decides
/// transitions, and the bit-equality AC-039 asks for would then be a value compared with itself.
#[test]
fn sigma_is_reconstructed_in_replay_rs() {
    let replay = read_repo("crates/gx-engine/src/replay.rs");
    let lines = code_lines(&replay);
    let has_type = lines.iter().any(|l| l.starts_with("pub struct Sigma"));
    let has_fn = lines.iter().any(|l| l.starts_with("pub fn reconstruct("));
    println!("SIGMA_TYPE_IN_REPLAY={has_type} RECONSTRUCT_IN_REPLAY={has_fn}");
    assert!(
        has_type && has_fn,
        "E-M5-2 puts the Σ reconstruction in the read-only module: `pub struct Sigma` and \
         `pub fn reconstruct(` both belong in replay.rs"
    );
}

/// 🔴 **E-M5-2, first instrument**: the replay module cannot reach an adapter.
///
/// 「adapter は呼ばない(機械検査つき)」. The check has two halves and this is the structural one: no
/// code line in `replay.rs` names an adapter, and `reconstruct` takes journal records and nothing
/// else. A function that is not handed a substrate cannot call one, which is why the signature is
/// part of the claim rather than the call sites alone — a later hand that adds an
/// `adapter: &dyn SubstrateAdapter` parameter has broken E-M5-2 before writing a single call.
///
/// The behavioural half is `tests/ac_039.rs`, where a counting adapter is registered and its totals
/// are compared across a replay.
#[test]
fn replay_names_no_adapter_and_reconstruct_is_handed_none() {
    let replay = read_repo("crates/gx-engine/src/replay.rs");
    let offenders: Vec<&str> = code_lines(&replay)
        .into_iter()
        .filter(|l| {
            l.contains("Adapter") || l.contains("adapter") || l.contains("gx_substrate::Substrate")
        })
        .collect();
    println!("ADAPTER_MENTIONS_IN_REPLAY_CODE={}", offenders.len());
    assert!(
        offenders.is_empty(),
        "E-M5-2: replay is read-only and names an adapter: {offenders:?}"
    );

    let signature = function_body(&replay, "pub fn reconstruct(")
        .expect("replay.rs declares `pub fn reconstruct(`");
    let head: String = signature
        .iter()
        .take_while(|l| !l.trim_end().ends_with('{'))
        .chain(signature.iter().find(|l| l.trim_end().ends_with('{')))
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" ");
    println!("RECONSTRUCT_SIGNATURE={head}");
    assert!(
        head.contains("&[EngineJournalRecord]"),
        "`reconstruct` takes the records and nothing else: {head}"
    );
    assert!(
        !head.contains("Adapter") && !head.contains("adapter"),
        "an adapter reached the read-only side through a parameter: {head}"
    );
}

/// 🔴 The one probe that keeps AC-039 from being a value compared with itself.
///
/// AC-039 compares 「元の結果状態」 with 「再構成結果状態」. If the engine answered 「what is my state」
/// by replaying its own journal, both sides of that comparison would come from the same bytes and
/// the criterion would hold no matter what the reconstruction did. So the live Σ is built from the
/// engine's own tables and this probe is what says so: `Engine::sigma` does not touch
/// `self.journal`.
///
/// `tools/verify_m5h3.sh` §4 mutates `sigma` into a journal read and prints that this probe is the
/// one that notices — which is the difference between a claim and a measurement.
#[test]
fn the_engine_builds_sigma_from_its_tables_and_not_from_its_journal() {
    let pipeline = read_repo("crates/gx-engine/src/pipeline.rs");
    let body = function_body(&pipeline, "pub fn sigma(")
        .expect("pipeline.rs declares `pub fn sigma(` -- the live half of AC-039's comparison");
    let reads: Vec<&&str> = body
        .iter()
        .filter(|l| {
            let code = l.trim();
            !code.starts_with("//") && code.contains("self.journal")
        })
        .collect();
    println!(
        "SIGMA_BODY_LINES={} JOURNAL_READS={}",
        body.len(),
        reads.len()
    );
    assert!(
        reads.is_empty(),
        "`Engine::sigma` reads the journal, so AC-039 would compare the journal with itself: \
         {reads:?}"
    );
}
