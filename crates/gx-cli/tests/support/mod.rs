//! Fixtures for the M6 hand 2 suites: a project, a ledger, a journal, and receipts.
//!
//! A module beside the suites rather than a suite (cargo builds one target per file **directly**
//! under `tests/`, which is why `floor_doubt.rs` does not recurse when it counts them).
//!
//! # 🔴 These build receipts the way 43 T-11 builds them
//!
//! [`commit_receipt`] appends to a real `TileLog` under [`gx_witness::ReceiptPayload::ledger_digest`]
//! and only then completes and signs the payload, which is the order 43 T-11 fixes and the order
//! `verify_offline` depends on. A fixture that signed first and appended afterwards would produce
//! receipts that verify against nothing, and the suites would then be measuring the fixture.

#![allow(dead_code)] // Each suite uses a subset; the module is one file for one set of shapes.

use std::path::{Path, PathBuf};

use gx_core::{
    Actor, ChangeContext, Checkpoint, Cid, CompositionMetadata, DeltaRef, FingerprintBytes,
    InclusionProof, IntentId, ObjectId, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_log::{proof, TileLog};
use gx_witness::receipt::{Receipt, ReceiptKind, ReceiptPayload, VerdictSummary};
use gx_witness::KeyPair;

/// An empty directory under the cargo target directory.
///
/// `CARGO_TARGET_TMPDIR` rather than `std::env::temp_dir`, for the reason `gx_layout.rs` writes out:
/// on this project's WSL2 setup `/tmp` is cleared while the machine sits idle. Cleared on entry, not
/// on exit, so a failing test leaves its tree to be read.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// 🔴 A scratch directory on a filesystem that **has unix permissions**.
///
/// `CARGO_TARGET_TMPDIR` is under the repository, and on this machine the repository sits on drvfs
/// (`/mnt/c`), where every file reads `0777` no matter what `open(2)` was asked for. `KeyPair::save`
/// asks for `0o600` and `KeyPair::load` refuses anything looser, so a key fixture written there is
/// one the library will not read back — the same fact `gx_cli::keys::permission_warning` reports to
/// an operator (M6H2-10). Key material therefore goes to the system temporary directory, which is
/// ext4 here.
///
/// Not used for anything else: `gx_layout.rs` explains why the *durable* fixtures avoid `/tmp`
/// (it is cleared while the machine sits idle), and that reasoning still holds for them.
pub fn secure_scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("gx-m6h2").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// Thirty-two bytes of a `Cid`, from a seed. Not minted through gx-canon: these are **fixture
/// names**, and 則 1 (i) is a rule about the shipping crate rather than about a test's arithmetic —
/// but the shape is kept anyway, so nothing here resembles a mint.
pub fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

pub fn tid(seed: u64) -> TransformationId {
    TransformationId(cid(9_000_000 + seed))
}

pub fn iid(seed: u64) -> IntentId {
    IntentId(cid(7_000_000 + seed))
}

pub fn oid(seed: u64) -> ObjectId {
    ObjectId(cid(4_000_000 + seed))
}

pub fn fingerprint(seed: u8) -> FingerprintBytes {
    FingerprintBytes([seed; 32])
}

/// A key pair from a **known** seed.
///
/// Known on purpose: `key_surface.rs` looks for the actual secret bytes in `gx key gen`'s output
/// (M6-29), and a probe that grepped for a field name would pass on a leak with a different label.
pub fn keypair(seed: u8) -> KeyPair {
    KeyPair::from_seed(format!("key-{seed}"), &[seed; 32])
}

pub fn issued_at() -> Timestamp {
    Timestamp(1_754_600_000_000_000_000)
}

/// A `VerdictReceipt` payload for one of 43 T-4a/b/c's three verdicts (42 §3.10).
pub fn verdict_payload(kind: VerdictKind, key: &KeyPair, seed: u64) -> ReceiptPayload {
    let t = tid(seed);
    ReceiptPayload {
        key_id: key.key_id().clone(),
        verdict: Some(VerdictSummary {
            kind,
            proof_digest: cid(500_000 + seed),
        }),
        enforced: false,
        receipt_kind: ReceiptKind::VerdictReceipt,
        canonical_cid: t.0,
        inverse_delta: None,
        transformation: t,
        inclusion_proof: None,
        fail_posture_engaged: true,
        precondition_fingerprint: fingerprint(7),
        postcondition_fingerprint: None,
    }
}

/// A `CommitReceipt` payload, with whichever proof the caller is at.
pub fn commit_payload(key: &KeyPair, seed: u64, inclusion: InclusionProof) -> ReceiptPayload {
    let t = tid(seed);
    ReceiptPayload {
        key_id: key.key_id().clone(),
        verdict: Some(VerdictSummary {
            kind: VerdictKind::Admit,
            proof_digest: cid(600_000 + seed),
        }),
        enforced: true,
        receipt_kind: ReceiptKind::CommitReceipt,
        canonical_cid: t.0,
        inverse_delta: Some(cid(700_000 + seed)),
        transformation: t,
        inclusion_proof: Some(inclusion),
        fail_posture_engaged: false,
        precondition_fingerprint: fingerprint(11),
        postcondition_fingerprint: Some(fingerprint(12)),
    }
}

/// The placeholder a payload carries while its own digest is being taken (`ledger_digest` clears
/// the field before hashing, so this never reaches a signature).
pub fn empty_proof() -> InclusionProof {
    InclusionProof {
        leaf_index: 0,
        tree_size: 0,
        audit_path: Vec::new(),
    }
}

/// Sign a payload.
pub fn issue(payload: &ReceiptPayload, key: &KeyPair) -> Receipt {
    Receipt::issue(payload, issued_at(), key).expect("the fixture is a legal receipt")
}

/// A `CommitReceipt` whose inclusion proof was really produced by a log, plus that log's head.
///
/// `others` leaves land first, so the audit path is not the trivial empty one.
pub fn commit_receipt(key: &KeyPair, seed: u64, others: u64) -> (Receipt, Checkpoint, TileLog) {
    let mut log = TileLog::new();
    for i in 0..others {
        log.append(tid(900_000 + i), cid(910_000 + i), Timestamp(i as i64))
            .expect("canonical");
    }
    let staged = commit_payload(key, seed, empty_proof());
    let index = log.len();
    log.append(
        tid(seed),
        staged.ledger_digest().expect("canonical"),
        Timestamp(1),
    )
    .expect("canonical");
    let inclusion = proof::prove_inclusion(&log, index).expect("the entry is in the log");
    let receipt = issue(&commit_payload(key, seed, inclusion), key);
    let head = proof::unsigned_checkpoint(&log, "glovrex-ledger/v1", Timestamp(2))
        .expect("a non-empty log has a head");
    (receipt, head, log)
}

/// A `Transformation`, for the journal records the replay suite writes.
pub fn transformation(seed: u64) -> Transformation {
    Transformation::new(
        tid(seed),
        0,
        Subject::Object(oid(seed)),
        Some(cid(seed)),
        Vec::new(),
        CompositionMetadata {
            intent_id: iid(seed),
            delta: DeltaRef {
                substrate: SubstrateKind::Fs,
                cid: cid(2_000_000 + seed),
            },
            context: ChangeContext::Evidence,
            actor: Actor::Agent {
                key: "key-agent-1".to_string(),
                model: "claude-fable-5".to_string(),
            },
            created_at: Timestamp(1_754_000_000_000_000_000),
        },
    )
    .expect("order 0 is below MAX_ORDER")
}

/// 44 §1.2's `gx key gen` stdout, written to a file: what `--key` reads (M6H2-6).
pub fn write_public_key(dir: &Path, key: &KeyPair) -> PathBuf {
    let path = dir.join("key.pub.json");
    let doc = serde_json::json!({
        "key_id": key.key_id(),
        "public_key": gx_core::b64::encode(&key.public().to_bytes()),
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&doc).expect("serialises")).expect("write");
    path
}

/// Write a JSON value to a file and hand back the path.
pub fn write_json(path: &Path, value: &serde_json::Value) -> PathBuf {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create");
    }
    std::fs::write(path, serde_json::to_vec_pretty(value).expect("serialises")).expect("write");
    path.to_path_buf()
}

// ---------------------------------------------------------------------------
// The binary
// ---------------------------------------------------------------------------

/// What one `gx` invocation did.
pub struct Run {
    /// The process status. 44 §1.4's table.
    pub code: i32,
    /// 44 §1.3's single newline-terminated JSON object.
    pub stdout: String,
    /// 44 §1.3's problem object, and operator notes.
    pub stderr: String,
}

impl Run {
    /// stdout, parsed.
    ///
    /// # Panics
    /// If stdout is not one JSON object, which is itself 44 §1.3's contract and so is asserted here
    /// rather than in every suite.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(self.stdout.trim()).unwrap_or_else(|e| {
            panic!(
                "44 §1.3 asks for a single newline-terminated JSON object on stdout; got {:?} ({e})",
                self.stdout
            )
        })
    }
}

/// The `gx` binary, as cargo built it.
pub fn gx() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_gx"))
}

/// Run a prepared command.
pub fn run(cmd: &mut std::process::Command) -> Run {
    let out = cmd.output().expect("the gx binary runs");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

/// A project with a `.gx/` directory, and the layout that opened it.
pub fn project(name: &str) -> (PathBuf, gx_cli::layout::Layout) {
    let dir = scratch(name);
    let layout = gx_cli::layout::Layout::create(&dir).expect("create .gx/");
    (dir, layout)
}

/// A durable ledger inside a project's `.gx/`, holding `n` leaves plus the one named by `seed`.
///
/// Writes through `gx_log::LedgerStore` — the same file `gx log proof` will open — rather than
/// through a `TileLog` in memory, because the commands under test read a **file**.
pub fn seed_ledger(
    layout: &gx_cli::layout::Layout,
    key: &KeyPair,
    seed: u64,
    others: u64,
) -> (Receipt, u64) {
    let path = layout.ledger_path();
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create ledger dir");
    let mut store = gx_log::LedgerStore::open(&path).expect("open the ledger");
    for i in 0..others {
        store
            .append(tid(900_000 + i), cid(910_000 + i), Timestamp(i as i64))
            .expect("append");
    }
    let staged = commit_payload(key, seed, empty_proof());
    let index = store.log().len();
    store
        .append(
            tid(seed),
            staged.ledger_digest().expect("canonical"),
            Timestamp(1),
        )
        .expect("append");
    let inclusion = proof::prove_inclusion(store.log(), index).expect("in the log");
    (issue(&commit_payload(key, seed, inclusion), key), index)
}

/// Flip one bit of a receipt's JSON face, inside the signed material (AC-019, AC-057's second case).
///
/// The flip lands in the base64 `payload` — the bytes the signature covers — rather than in a
/// field beside it, because AC-019 asks for 「rのバイト列表現からランダムな1bitを反転」 and a flip in
/// an unsigned field would be a different experiment (and would pass).
pub fn tamper_payload_byte(receipt: &Receipt) -> Receipt {
    let mut tampered = receipt.clone();
    let last = tampered.envelope.payload.len() - 1;
    tampered.envelope.payload[last] ^= 0x01;
    tampered
}

// ---------------------------------------------------------------------------
// M6 hand 3: driving 44 §1.2's pipeline as four processes
// ---------------------------------------------------------------------------

/// A project, a key store, a target file and a goal file — everything `gx submit` needs.
///
/// # 🔴 Why the home directory is somewhere else
///
/// `KeyPair::save` asks for `0o600` and `KeyPair::load` refuses anything looser, and this
/// repository sits on drvfs where every file reads `0777` (M6H2-10). So the key store goes to
/// [`secure_scratch`] and the **project** stays under `CARGO_TARGET_TMPDIR`, which is where hand 2
/// put the durable fixtures for its own reason (`/tmp` is cleared while this machine sits idle).
/// The two directories are separate because req/56 §1 and §3 make them separate: 「秘密は project に
/// 置かない」.
pub struct Pipeline {
    /// The project whose `.gx/` the commands act on.
    pub project: PathBuf,
    /// `HOME`, holding `~/.gx/keys/` (req/56 §3).
    pub home: PathBuf,
    /// The file on the substrate the transformation is about.
    pub target: PathBuf,
    /// The key id `gx key gen` produced.
    pub key_id: String,
}

impl Pipeline {
    /// A `gx` invocation against this project, with this key store.
    ///
    /// `--project` rather than a working directory, because a test that changed directories would
    /// be changing it for every other test in the process (cargo runs them on threads).
    pub fn gx(&self) -> std::process::Command {
        let mut cmd = gx();
        cmd.env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .arg("--project")
            .arg(&self.project);
        cmd
    }

    /// `gx submit` with this fixture's substrate, locator, goal and actor.
    pub fn submit(&self, goal: &str) -> Run {
        let goal_file = self.project.join(format!("goal-{}.txt", goal.len()));
        std::fs::write(&goal_file, goal).expect("write the goal");
        run(self
            .gx()
            .arg("submit")
            .args(["--substrate", "fs"])
            .arg("--locator")
            .arg(&self.target)
            .arg("--intent")
            .arg(&goal_file)
            .args(["--context", "Evidence"])
            .args(["--actor-key", &self.key_id]))
    }

    /// 🔴 A **second** key in this project's store, and its id.
    ///
    /// 42 §3.13's `HumanDecision.actor` is the ruler and not the submitter, so a suite that ruled
    /// on an escalation with `key_id` would be unable to tell the correct implementation from one
    /// that signed the ruling with the key of the party being ruled on. Two keys make the
    /// difference observable.
    pub fn another_key(&self) -> String {
        let generated = run(self.gx().args(["key", "gen", "--json"]));
        assert_eq!(generated.code, 0, "a second key: {}", generated.stderr);
        generated.json()["key_id"]
            .as_str()
            .expect("44 §1.2's `gx key gen` prints a key_id")
            .to_string()
    }

    /// The contents of the file on the substrate.
    pub fn target_contents(&self) -> String {
        std::fs::read_to_string(&self.target).unwrap_or_default()
    }

    /// How many records the engine journal holds — Λ2's measurement.
    pub fn journal_records(&self) -> usize {
        self.journal().len()
    }

    /// 🔴 The journal's records, read off disk — no engine, no row.
    ///
    /// What 43 T-12's guard is checked against in `undo_cmd.rs`: 「`T_u.parents` が `T_o.id` を含む」
    /// became answerable from the log alone when **E-M5-13** put `parents` in the `Planned` record,
    /// and a helper that went through an `Engine` would be measuring the table instead.
    pub fn journal(&self) -> Vec<gx_engine::EngineJournalRecord> {
        let layout = gx_cli::layout::Layout::open(&self.project).expect("the project is open");
        let bytes = std::fs::read(layout.journal_path()).unwrap_or_default();
        gx_engine::replay(&bytes).records().to_vec()
    }
}

impl Pipeline {
    /// Drive `submit` → `plan` → `verify` → `commit` as four processes and hand back the id.
    ///
    /// The road hand 3 measured, packaged so that hand 4's suites can start from 「there is a
    /// committed transformation」 without restating it four times. Every step asserts its own status
    /// **by number**, so a fixture that silently stopped halfway is a failure here rather than a
    /// confusing one later.
    pub fn commit_one(&self, goal: &str) -> String {
        let submitted = self.submit(goal);
        assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
        let intent_id = submitted.json()["intent_id"]
            .as_str()
            .expect("an intent id")
            .to_string();
        let planned = run(self.gx().args(["plan", &intent_id]));
        assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
        let tid = planned.json()["transformation"]["id"]
            .as_str()
            .expect("a transformation id")
            .to_string();
        let verified = run(self.gx().args(["verify", &tid]));
        assert_eq!(verified.code, 0, "verify: {}", verified.stderr);
        let committed = run(self.gx().args(["commit", &tid]));
        assert_eq!(committed.code, 0, "commit: {}", committed.stderr);
        tid
    }

    /// `submit` → `plan` only: a `Candidate`, which is what 43 T-7's from-set starts at.
    pub fn planned_one(&self, goal: &str) -> String {
        let submitted = self.submit(goal);
        assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
        let intent_id = submitted.json()["intent_id"]
            .as_str()
            .expect("an intent id")
            .to_string();
        let planned = run(self.gx().args(["plan", &intent_id]));
        assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
        planned.json()["transformation"]["id"]
            .as_str()
            .expect("a transformation id")
            .to_string()
    }
}

/// 🔴 The test-only pack of **M6H3-9 採(a)** — a pack that denies a path a suite may write.
///
/// See the file itself for why it exists and why it is not under `policies/`. The path is absolute
/// so that it survives `--project` pointing somewhere else.
pub fn deny_writable_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("deny-writable.cedar")
}

/// 🔴 The fragment [`deny_writable_pack`]'s `forbid` matches on.
///
/// A name and not a path: the locator a suite denies lives in a temporary directory that does not
/// exist until the test runs, and Cedar's `like` is a string matcher over the attribute ASM-60-1
/// maps from `pre.locator()`.
pub const DENIED_FRAGMENT: &str = "gx-denied-target";

/// A file whose **old** content is large enough that its inverse cannot be built.
///
/// **E-M3-4** is the only rule that produces a `Verdict::Escalate` in v0.1 — 「the adapter could not
/// build an inverse」 — and the fs adapter's only `Ok(None)` is the escrow ceiling
/// (`MAX_INVERSE_PAYLOAD_BYTES`, one mebibyte, M4-21 採(a)). An inverse carries the **whole old
/// file**, so a change over a large file is the one an operator is asked about: it can be made and
/// it cannot be taken back. This is that file, and it is how an `Escalated` Given is built through
/// the binary rather than by injecting a stub adapter.
pub fn oversized_before() -> String {
    "x".repeat(1024 * 1024 + 4096)
}

/// Build a project with a key, a target file holding `before`, and nothing else.
pub fn pipeline(name: &str, before: &str) -> Pipeline {
    pipeline_named(name, before, "target.txt")
}

/// The same, with the target file's **name** chosen by the caller.
///
/// The name is what [`deny_writable_pack`] matches on, so a suite that wants a denied locator asks
/// for one containing [`DENIED_FRAGMENT`] rather than reaching for `/etc`.
pub fn pipeline_named(name: &str, before: &str, target_name: &str) -> Pipeline {
    let project = scratch(name);
    let home = secure_scratch(&format!("{name}-home"));
    let target = project.join(target_name);
    std::fs::write(&target, before).expect("write the target");

    let mut fixture = Pipeline {
        project,
        home,
        target,
        key_id: String::new(),
    };
    let generated = run(fixture.gx().args(["key", "gen", "--json"]));
    assert_eq!(
        generated.code, 0,
        "the fixture needs a key: {}",
        generated.stderr
    );
    fixture.key_id = generated.json()["key_id"]
        .as_str()
        .expect("44 §1.2's `gx key gen` prints a key_id")
        .to_string();
    fixture
}

/// 🔴 A locator the **shipped policy pack refuses** (`policies/fs/deny-etc.cedar`).
///
/// The pack's one forbid is 「`resource.locator == "/etc" || resource.locator like "/etc/*"`」, so
/// the CLI's only route to a `Verdict::Deny` in v0.1 runs through a real path under `/etc`. This
/// names one that exists on every Linux and is **read** and never written: `gx verify` reaches a
/// verdict without touching the substrate (43 T-3: 「verify自体は読み取り専用（substrate非破壊）」),
/// and `gx commit` of a `Denied` row is refused by T-8 **before** `apply` is reached.
///
/// 🔴 What this fixture deliberately does not do is `--record-only`. Under `EnforcementMode::
/// RecordOnly` T-8r opens and the commit proceeds to `apply`, which on this locator is a **write to
/// `/etc`** — harmless as a permission error for an ordinary user and destructive for a suite run
/// as root. So the record-only half of DR-2 is measured where it can be measured safely, in
/// `crates/gx-engine/tests/record_only_per_call.rs`, and the gap is raised as **M6H3-9**.
pub const DENIED_LOCATOR: &str = "/etc/hostname";
