// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R29 / `req/361` H-01, `req/38` §238 ruling 1** — the bed the twenty-eighth adversarial
//! audit drove, as a fixture two suites share rather than two suites each own a copy of.
//!
//! # The defect this module exists to close
//!
//! 43 T-10c sends the escrowed inverse back to the substrate when a commit's `apply` fails, and
//! until this lane `crates/gx-engine/src/pipeline.rs` decided what became of the **world** from
//! what the **call** answered: `Ok(_) => Rollback::Succeeded`. The audit showed what that word was
//! worth by driving a contract-conforming adapter whose `apply` performs the first `k` operations
//! of a delta and then returns `Err` — a shape `crates/gx-substrate/src/error.rs` permits in as
//! many words, since its `ApplyFailed` documentation says a non-atomic apply can fail halfway.
//! With a **relative** delta grammar the road `POST /candidates` to `verify` to `commit` to
//! `POST /transformations/{id}/undo` left the object at `A B D C D` where it had started at
//! `A B C D` — a record duplicated, the object further from home than when the roll-back began —
//! and put `"rollback":"Succeeded"` on the wire over it.
//!
//! Reproducing that needs three things a shipped adapter will not give: a delta grammar that can be
//! chosen (relative or absolute), an `apply` whose failure point is scripted rather than accidental,
//! and a world small enough to print in a refusal message. This module is those three things and
//! nothing else — it holds no probe of its own, because a fixture that also asserts is a fixture
//! whose failures are hard to attribute.
//!
//! # Why it is shared, and why it is shared this way
//!
//! Two suites drive the same corrupted world: `r29_rollback_is_verified.rs` reads what the undo's
//! own refusal says, and `r29_rollback_read_faces.rs` reads what the transformation's read faces
//! say about it afterwards. A second copy of the bed would be the drift class this whole lane is a
//! repair of — one road learning a fact the other road never hears. It is reached with the
//! `#[path]` spelling this directory already uses for `support/probe_counts.rs` and
//! `support/audit_p3_support.rs`, rather than through `support/mod.rs`, because `support/mod.rs` is
//! an existing file and surgical-change discipline says a new fixture arrives as a new file.
//!
//! Every item is `pub` and the module carries `#![allow(dead_code)]`, because each test binary uses
//! a different subset and Rust judges an unused item per binary.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gx_api::auth::Bearer;
use gx_api::state::{AppState, RequestEvidence, ServerKeys};
use gx_api::{ReceiptArchive, ReceiptSlot};
use gx_core::{
    Cid, Commutation, Fingerprint, Intent, ObjectId, ObjectSnapshot, ReprKind, SubstrateKind,
    Timestamp, TransformationId,
};
use gx_substrate::{AppliedDelta, PlannedDelta, SubstrateAdapter};
use gx_witness::{KeyPair, Receipt};
use serde_json::Value;
use tower::ServiceExt;

/// The bearer token every request in these suites carries. One constant, because 44 §2.5 puts the
/// whole router behind one guard and a fixture that varied the token would be measuring the guard.
pub const TOKEN: &str = "r29-scripted-token";

/// The substrate every fixture here speaks for.
///
/// `Fs` and not a custom kind, because the shipped fs pack (`policies/fs/deny-etc.cedar`) is what
/// makes these transformations decidable at all: its `fs-permit-default` statement admits any
/// locator that is not under `/etc`, so the verdict is an Admit for a scratch file and the road
/// reaches 43 T-9 through T-11 — which is the only part of the pipeline this lane is about.
/// Choosing a kind with no pack would make every probe fail at the gate, one transition too early.
pub const KIND: SubstrateKind = SubstrateKind::Fs;

// ---------------------------------------------------------------------------
// Measurements
// ---------------------------------------------------------------------------

/// Print a machine-readable measurement line and mirror it to `$R29_MEAS` when that is set.
///
/// The shape `crates/gx-cli/tests/r28_remedy_marker.rs` established, kept deliberately: a suite
/// whose findings exist only in a scrollback is a suite whose findings are re-derived by hand the
/// next time somebody asks. The mirror is silent when the variable is absent, so the fixture works
/// under a bare `cargo test` as well as under the runner script that sets it.
pub fn record(line: &str) {
    println!("{line}");
    let Ok(path) = std::env::var("R29_MEAS") else {
        return;
    };
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }
}

/// An empty directory under `CARGO_TARGET_TMPDIR`, cleared on entry and not on exit.
///
/// `CARGO_TARGET_TMPDIR` rather than `std::env::temp_dir`, for the reason `tests/support/mod.rs`
/// already writes out: on this project's WSL2 setup `/tmp` is cleared while the machine sits idle,
/// and a suite whose world file evaporates mid-run reports housekeeping as a substrate failure.
/// Cleared on entry so that a failing probe leaves its world on disk to be read.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("r29")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// Drive one future to completion on a current-thread runtime.
///
/// Written out rather than reached through the async test attribute, so that every probe in the
/// suites above wears the plain test attribute — which is what a registry counting probes reads
/// (`tests/support/probe_counts.rs` accepts both spellings; one spelling is still one fewer thing
/// for a reader to reconcile).
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime")
        .block_on(future)
}

// ---------------------------------------------------------------------------
// The digest
// ---------------------------------------------------------------------------

/// A 32-byte content digest for the fixture's world, built from four FNV-1a lanes.
///
/// 🔴 Deliberately **not** the XOR fold `gx-engine/tests/support/mod.rs` uses. That fold is
/// position-indexed modulo 32 and collides freely between strings that differ by a transposition —
/// which is exactly the difference this lane's finding turns on (`A B C D` against `A B D C D`). A
/// fixture whose digest could not tell a corrupted world from a restored one would leave
/// [`ScriptedAdapter`]'s `precondition` unable to falsify anything, and `gx-canon` is not available
/// here (`crates/gx-cli/Cargo.toml` forbids the edge, 41 §6 and Rule 1(i)), so the fixture carries
/// its own. Four independently seeded lanes rather than one, so that the value is 32 bytes of hash
/// rather than 8 bytes of hash and 24 bytes of padding.
pub fn digest_of(bytes: &[u8]) -> Cid {
    const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut out = [0u8; 32];
    for lane in 0u64..4 {
        let mut hash = BASIS ^ lane.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        // The length, folded in last: two worlds one of which is a prefix of the other must not
        // agree, and a plain FNV over the shorter one is a plain FNV over a prefix.
        hash ^= bytes.len() as u64;
        hash = hash.wrapping_mul(PRIME);
        let at = lane as usize * 8;
        out[at..at + 8].copy_from_slice(&hash.to_le_bytes());
    }
    Cid(out)
}

// ---------------------------------------------------------------------------
// The grammar, the script, and the operations
// ---------------------------------------------------------------------------

/// Which kind of sentence a `PlannedDelta` of this adapter is.
///
/// 🔴 This is the axis `req/361` H-01 turns on, and it is the reason the fixture is configurable
/// rather than fixed. A delta that says *the object's whole new content is X* is idempotent under
/// re-application and its inverse restores the object from any state; a delta that says *add these
/// records* is neither. Both are legitimate — 41 §4 leaves the payload opaque (P-6) and says
/// nothing about its grammar — so an engine that is correct for one and wrong for the other is an
/// engine whose correctness depends on which adapter a deployment happened to register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grammar {
    /// *Add these records, remove those.* The audit's grammar, and the one that produced
    /// `A B C D` to `A B D` to `A B D C D`.
    Relative,
    /// *The object's whole new content is this.* The negative control's grammar, under which the
    /// same half-failing script really does end with the object back where it started.
    Absolute,
}

/// What the adapter's `apply` does on one call.
///
/// 🔴 The middle variant is the contract-conforming failure `crates/gx-substrate/src/error.rs`
/// permits: its `ApplyFailed` documentation says a non-atomic apply can fail halfway, so an adapter
/// that performs part of a delta and then reports an error is inside the boundary contract and not
/// a broken implementation. Everything this lane repairs follows from taking that sentence
/// literally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behaviour {
    /// Perform every operation and answer `Ok`.
    Full,
    /// Perform the first `k` operations, leave the rest undone, and answer `Err(ApplyFailed)`.
    HalfThenFail(usize),
    /// Perform every operation, answer `Ok`, and then make every later read of the object fail.
    ///
    /// The case that separates *the object is home* from *this engine cannot tell*: the
    /// compensation's bytes landed and the read-back that would confirm it cannot be taken.
    FullThenUnreadable,
}

/// One operation of a delta of this adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Append one record to the end of the world.
    Add(String),
    /// Remove the first occurrence of one record from the world.
    Remove(String),
    /// Replace the world's whole content. An absolute delta is exactly one of these, which is what
    /// makes half of it and all of it the same world.
    Set(Vec<String>),
}

/// Encode a delta as bytes an adapter can read back. One header line naming the grammar, then one
/// line per operation.
pub fn encode(grammar: Grammar, ops: &[Op]) -> Vec<u8> {
    let mut text = String::new();
    text.push_str(match grammar {
        Grammar::Relative => "REL\n",
        Grammar::Absolute => "ABS\n",
    });
    for op in ops {
        match op {
            Op::Add(record) => {
                text.push('+');
                text.push_str(record);
                text.push('\n');
            }
            Op::Remove(record) => {
                text.push('-');
                text.push_str(record);
                text.push('\n');
            }
            Op::Set(records) => {
                for record in records {
                    text.push('=');
                    text.push_str(record);
                    text.push('\n');
                }
            }
        }
    }
    text.into_bytes()
}

/// Read a delta back out of its bytes.
///
/// An absolute payload decodes to **one** operation however many records it names, which is the
/// grammar's whole content: there is no such thing as half of "the content is this".
///
/// # Errors
/// [`gx_substrate::Error::PayloadUnreadable`] for bytes this adapter did not write — the arm 41 §4
/// reserves for a foreign payload, rather than a panic inside a fixture.
pub fn decode(payload: &[u8]) -> gx_substrate::Result<(Grammar, Vec<Op>)> {
    let text =
        std::str::from_utf8(payload).map_err(|e| gx_substrate::Error::PayloadUnreadable {
            detail: format!("this fixture writes UTF-8 deltas: {e}"),
        })?;
    let mut lines = text.lines();
    let grammar = match lines.next() {
        Some("REL") => Grammar::Relative,
        Some("ABS") => Grammar::Absolute,
        other => {
            return Err(gx_substrate::Error::PayloadUnreadable {
                detail: format!("a delta of this fixture opens REL or ABS, not {other:?}"),
            })
        }
    };
    let mut ops = Vec::new();
    let mut absolute = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (tag, rest) = line.split_at(1);
        match tag {
            "+" => ops.push(Op::Add(rest.to_string())),
            "-" => ops.push(Op::Remove(rest.to_string())),
            "=" => absolute.push(rest.to_string()),
            _ => {
                return Err(gx_substrate::Error::PayloadUnreadable {
                    detail: format!("a delta line of this fixture opens +, - or =, not {line:?}"),
                })
            }
        }
    }
    if grammar == Grammar::Absolute {
        ops.push(Op::Set(absolute));
    }
    Ok((grammar, ops))
}

/// The records a world's bytes hold: one per line, with the trailing newline of the last one
/// dropped rather than read as an empty record.
pub fn records_of(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

/// The bytes a list of records is written as: each record on its own line, every line terminated.
pub fn text_of(records: &[String]) -> String {
    let mut out = String::new();
    for record in records {
        out.push_str(record);
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// The adapter
// ---------------------------------------------------------------------------

/// A `SubstrateAdapter` whose world is a real file, whose grammar is chosen, and whose `apply`
/// fails where a script says it fails.
///
/// 🔴 **The fixture `req/361` H-01 was found with.** Three properties matter and each one is
/// load-bearing:
///
/// 1. **The world is on disk.** `snapshot` and `precondition` are real reads of a real file, so the
///    engine's new read-back is exercised as a read rather than as a lookup in the same memory the
///    fixture just wrote — which would make the probe an assertion about a variable.
/// 2. **The grammar is a parameter.** The same script drives a restored world under
///    [`Grammar::Absolute`] and a destroyed one under [`Grammar::Relative`], so the negative control
///    and the finding differ by exactly one value.
/// 3. **The script is per call, not per adapter.** 43 T-10c reaches `apply` a second time with the
///    escrowed inverse, and the finding is about what the *second* call does after the *first* one
///    half-failed. An adapter that failed every apply could not produce it.
pub struct ScriptedAdapter {
    /// The file the world lives in. Also the locator every transformation of this fixture names.
    path: PathBuf,
    grammar: Grammar,
    /// What each `apply` does, by call index. Calls past the end of the script are
    /// [`Behaviour::Full`].
    script: Mutex<Vec<Behaviour>>,
    /// How many times `apply` has been reached. The index into the script.
    applies: AtomicUsize,
    /// Set by [`Behaviour::FullThenUnreadable`]: every later `snapshot` and `precondition` fails.
    unreadable: AtomicBool,
    /// One line per `apply`, in order — what the call was told to do, what it did, and where the
    /// world came to rest. The account a failing probe prints instead of leaving a reader to guess.
    trace: Mutex<Vec<String>>,
    /// How many times `snapshot` has answered. The ordinal a read is named by in
    /// [`ScriptedAdapter::read_trace`].
    reads: AtomicUsize,
    /// One line per answered `snapshot`, in order — the ordinal, how many applies had been reached
    /// when it was taken, the bytes it answered about, and whether the third-party write below
    /// fired on it.
    read_trace: Mutex<Vec<String>>,
    /// 🔴 **R30 / `req/372` M-01** — the opt-in third-party hook, **off** unless armed.
    ///
    /// Zero means off, which is why the value is a count of applies rather than an index: `apply`
    /// is reached at least once on every road here, so no road can arm this by accident.
    third_party_after: AtomicUsize,
    /// What the third party writes when the hook fires.
    third_party_content: Mutex<String>,
    /// The read ordinal the hook fired on, or zero if it has not fired. One shot.
    third_party_at_read: AtomicUsize,
}

impl ScriptedAdapter {
    /// An adapter over a fresh world file holding `initial`, with no failures scripted.
    pub fn open(path: PathBuf, grammar: Grammar, initial: &[&str]) -> Arc<Self> {
        let records: Vec<String> = initial.iter().map(|r| (*r).to_string()).collect();
        std::fs::write(&path, text_of(&records)).expect("seed the world");
        Arc::new(Self {
            path,
            grammar,
            script: Mutex::new(Vec::new()),
            applies: AtomicUsize::new(0),
            unreadable: AtomicBool::new(false),
            trace: Mutex::new(Vec::new()),
            reads: AtomicUsize::new(0),
            read_trace: Mutex::new(Vec::new()),
            third_party_after: AtomicUsize::new(0),
            third_party_content: Mutex::new(String::new()),
            third_party_at_read: AtomicUsize::new(0),
        })
    }

    /// Fix what each `apply` will do, by call index.
    pub fn script(&self, script: &[Behaviour]) {
        let mut held = self.script.lock().expect("the script is not poisoned");
        held.clear();
        held.extend_from_slice(script);
    }

    /// Which grammar this adapter's deltas are written in.
    pub fn grammar(&self) -> Grammar {
        self.grammar
    }

    /// The locator every transformation of this fixture names.
    pub fn locator(&self) -> String {
        self.path.display().to_string()
    }

    /// The world's bytes, read straight off the file.
    ///
    /// Deliberately not routed through `snapshot`: a probe reading the world after a
    /// [`Behaviour::FullThenUnreadable`] needs the truth, and `snapshot` is under instruction to
    /// refuse.
    pub fn world(&self) -> String {
        std::fs::read_to_string(&self.path).unwrap_or_default()
    }

    /// The world as records.
    pub fn records(&self) -> Vec<String> {
        records_of(&self.world())
    }

    /// One line per `apply`, in order.
    pub fn trace(&self) -> Vec<String> {
        self.trace
            .lock()
            .expect("the trace is not poisoned")
            .clone()
    }

    /// How many times `apply` has been reached.
    pub fn applies(&self) -> usize {
        self.applies.load(Ordering::SeqCst)
    }

    /// Whether reads of the object are currently refusing.
    pub fn is_unreadable(&self) -> bool {
        self.unreadable.load(Ordering::SeqCst)
    }

    /// 🔴 **R30 / `req/372` M-01** — arm a **one-shot** third-party write, timed to land between
    /// the engine's own reads. **Off unless this is called**, so every existing user of this
    /// fixture drives exactly the road it drove before.
    ///
    /// # What it does, stated as the mechanism rather than as an intention
    ///
    /// Once `apply` has been reached `nth` times, the **next** `snapshot` answers from the world as
    /// it stands and *then* writes `content` over the world file, once. The answer the engine gets
    /// is therefore the pre-write world and the world the engine's *next* read finds is the
    /// post-write one — which is the only way to land a write strictly between two reads a caller
    /// does not control the scheduling of.
    ///
    /// # 🔴 Why the write is not an `apply`
    ///
    /// It goes straight to the file. A third party is not this engine's adapter call, and routing
    /// it through `apply` would move [`ScriptedAdapter::applies`] — the counter a probe uses to
    /// establish that the compensating inverse was **never sent**. That premise has to come from
    /// the fixture's own count of calls it received, not from gx's account of what it did.
    ///
    /// # What it deliberately does not do
    ///
    /// It does not decide which read it lands on. The ordinal it fired at is reported by
    /// [`ScriptedAdapter::third_party_fired_at_read`] and every read is on
    /// [`ScriptedAdapter::read_trace`], so a probe asserts what happened rather than assuming it.
    pub fn third_party_writes_after_apply(&self, nth: usize, content: &str) {
        *self
            .third_party_content
            .lock()
            .expect("the third-party content is not poisoned") = content.to_string();
        self.third_party_after.store(nth, Ordering::SeqCst);
    }

    /// The read ordinal the third-party write fired on, or zero if it never fired.
    pub fn third_party_fired_at_read(&self) -> usize {
        self.third_party_at_read.load(Ordering::SeqCst)
    }

    /// How many times `snapshot` has answered.
    ///
    /// A `snapshot` that **refused** (see [`Behaviour::FullThenUnreadable`]) is not counted: it
    /// answered nothing, so it names no world and carries no ordinal.
    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    /// One line per answered `snapshot`, in order.
    pub fn read_trace(&self) -> Vec<String> {
        self.read_trace
            .lock()
            .expect("the read trace is not poisoned")
            .clone()
    }

    fn refuse_reads(&self) -> gx_substrate::Result<()> {
        if self.unreadable.load(Ordering::SeqCst) {
            return Err(gx_substrate::Error::Unreadable {
                locator: self.locator(),
                detail: "this fixture was scripted to stop answering reads after the apply that \
                         landed (R29: a compensation whose bytes landed and whose read-back died)"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Perform the first `count` operations of `ops` against `records`.
    fn perform(records: &mut Vec<String>, ops: &[Op], count: usize) {
        for op in ops.iter().take(count) {
            match op {
                Op::Add(record) => records.push(record.clone()),
                Op::Remove(record) => {
                    if let Some(at) = records.iter().position(|held| held == record) {
                        records.remove(at);
                    }
                }
                Op::Set(content) => records.clone_from(content),
            }
        }
    }
}

impl SubstrateAdapter for ScriptedAdapter {
    fn kind(&self) -> SubstrateKind {
        KIND
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<ObjectSnapshot> {
        self.refuse_reads()?;
        let world =
            std::fs::read_to_string(&self.path).map_err(|e| gx_substrate::Error::Unreadable {
                locator: locator.to_string(),
                detail: e.to_string(),
            })?;
        let answer = ObjectSnapshot::new(
            ObjectId(digest_of(locator.as_bytes())),
            KIND,
            locator.to_string(),
            digest_of(world.as_bytes()),
            ReprKind::Bytes,
        );

        // 🔴 **R30 / `req/372` M-01** — the third-party write lands **after** this read has been
        // answered from `world` and before the caller can take its next one. The order is the whole
        // hook: an engine that reads once cannot see it, and an engine that reads twice does.
        let nth = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        let applies = self.applies.load(Ordering::SeqCst);
        let mut wrote: Option<String> = None;
        let armed = self.third_party_after.load(Ordering::SeqCst);
        if armed != 0 && applies >= armed && self.third_party_at_read.load(Ordering::SeqCst) == 0 {
            let content = self
                .third_party_content
                .lock()
                .expect("the third-party content is not poisoned")
                .clone();
            std::fs::write(&self.path, &content).map_err(|e| gx_substrate::Error::Unreadable {
                locator: locator.to_string(),
                detail: format!("this fixture's third party could not write its world: {e}"),
            })?;
            self.third_party_at_read.store(nth, Ordering::SeqCst);
            wrote = Some(content);
        }
        self.read_trace
            .lock()
            .expect("the read trace is not poisoned")
            .push(format!(
                "read#{nth} applies={applies} answered_about={world:?} third_party_wrote={wrote:?}"
            ));

        Ok(answer)
    }

    /// The goal bytes are the object's desired content; the delta is the difference, spelled in
    /// this adapter's grammar.
    ///
    /// The current content is read here rather than taken from `pre`, because 42 §3.3's
    /// `ObjectSnapshot` carries a digest and not a body — an adapter that needs the content to
    /// compute a relative delta has to read it, which is what the shipped fs adapter does as well.
    fn plan(&self, intent: &Intent, _pre: &ObjectSnapshot) -> gx_substrate::Result<PlannedDelta> {
        let goal = records_of(&String::from_utf8_lossy(&intent.goal().0));
        let ops = match self.grammar {
            Grammar::Absolute => vec![Op::Set(goal)],
            Grammar::Relative => {
                let mut remaining = goal;
                let mut removals = Vec::new();
                for held in self.records() {
                    match remaining.iter().position(|want| *want == held) {
                        Some(at) => {
                            remaining.remove(at);
                        }
                        None => removals.push(Op::Remove(held)),
                    }
                }
                let mut ops = removals;
                ops.extend(remaining.into_iter().map(Op::Add));
                ops
            }
        };
        PlannedDelta::new(KIND, encode(self.grammar, &ops))
    }

    fn precondition(&self, snap: &ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        self.refuse_reads()?;
        Ok(Fingerprint::new(
            KIND,
            snap.locator().to_string(),
            *snap.digest(),
        )?)
    }

    /// The one call that moves the world, under the script.
    ///
    /// 🔴 A [`Behaviour::HalfThenFail`] writes the partial world **and then** answers `Err`. That
    /// ordering is the finding: an adapter that failed without writing would leave the object where
    /// it was and 43 T-10c would have nothing to compensate for.
    /// `crates/gx-substrate/src/error.rs` says a non-atomic apply can fail halfway, and this is
    /// what halfway looks like on disk.
    fn apply(&self, delta: &PlannedDelta) -> gx_substrate::Result<AppliedDelta> {
        let nth = self.applies.fetch_add(1, Ordering::SeqCst);
        let behaviour = self
            .script
            .lock()
            .expect("the script is not poisoned")
            .get(nth)
            .copied()
            .unwrap_or(Behaviour::Full);
        let (_, ops) = decode(delta.payload())?;
        let performed = match behaviour {
            Behaviour::Full | Behaviour::FullThenUnreadable => ops.len(),
            Behaviour::HalfThenFail(k) => k.min(ops.len()),
        };
        let before = self.world();
        let mut records = records_of(&before);
        Self::perform(&mut records, &ops, performed);
        let after = text_of(&records);
        std::fs::write(&self.path, &after).map_err(|e| gx_substrate::Error::ApplyFailed {
            detail: format!("this fixture could not write its world: {e}"),
        })?;
        self.trace
            .lock()
            .expect("the trace is not poisoned")
            .push(format!(
                "apply#{} {behaviour:?} ops={} performed={performed} before={before:?} \
                 after={after:?}",
                nth + 1,
                ops.len(),
            ));
        if matches!(behaviour, Behaviour::HalfThenFail(_)) {
            return Err(gx_substrate::Error::ApplyFailed {
                detail: format!(
                    "this fixture performed {performed} of {} operations and then failed, which \
                     the ApplyFailed contract permits in as many words (43 T-10c)",
                    ops.len()
                ),
            });
        }
        if behaviour == Behaviour::FullThenUnreadable {
            self.unreadable.store(true, Ordering::SeqCst);
        }
        let digest = digest_of(after.as_bytes());
        Ok(AppliedDelta::new(
            delta.reference().clone(),
            Fingerprint::new(KIND, self.locator(), digest)?,
            digest,
            // E-M4-31: an adapter has no clock (41 §6 injects it at the engine boundary), so it
            // answers zero and the engine rebuilds the value with the moment it was given.
            Timestamp(0),
        ))
    }

    /// The inverse, in the same grammar the delta is written in.
    ///
    /// 🔴 The two arms are the two halves of the finding. A **relative** inverse is the syntactic
    /// negation of the delta, which is honest and complete and still cannot restore an object the
    /// forward apply moved only part of the way — applying *add C, add D* to `A B D` gives
    /// `A B D C D`. An **absolute** inverse names the content the object holds now, so it restores
    /// the object from wherever the failed apply left it. Neither is a broken adapter; 41 §4 leaves
    /// the payload opaque and neither grammar is privileged by anything in the canon.
    fn invert(
        &self,
        delta: &PlannedDelta,
        _pre: &ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::InvertOutcome> {
        let (grammar, ops) = decode(delta.payload())?;
        let inverse: Vec<Op> = match grammar {
            Grammar::Relative => ops
                .iter()
                .map(|op| match op {
                    Op::Add(record) => Op::Remove(record.clone()),
                    Op::Remove(record) => Op::Add(record.clone()),
                    Op::Set(content) => Op::Set(content.clone()),
                })
                .collect(),
            Grammar::Absolute => vec![Op::Set(self.records())],
        };
        Ok(gx_substrate::InvertOutcome::inverted(
            PlannedDelta::new(KIND, encode(grammar, &inverse))?,
            Vec::new(),
        ))
    }

    fn commutation(
        &self,
        _a: &PlannedDelta,
        _b: &PlannedDelta,
    ) -> gx_substrate::Result<Commutation> {
        Ok(Commutation::Commutes)
    }
}

// ---------------------------------------------------------------------------
// The server the adapter is registered into
// ---------------------------------------------------------------------------

/// The server's own key, and no ruler: nothing in these suites escalates.
struct OneKey(KeyPair);

impl ServerKeys for OneKey {
    fn signing(&self) -> &KeyPair {
        &self.0
    }

    fn ruler(&self, _key_id: &str) -> Option<&KeyPair> {
        None
    }
}

/// A receipt archive in memory.
///
/// 🔴 Not the no-archive default, and the difference is the whole undo road. `gx-api`'s
/// `undo_witness` answers `WitnessMissing::NoArchive` for a deployment that keeps no receipts, and
/// R3 (`req/38` §160 ruling 2) makes that a **refusal** — so a fixture with no archive would never
/// reach 43 T-9 through T-11 at all and every probe below would be green for the wrong reason. In
/// memory rather than in `.gx/receipts/` because these suites assert nothing about where a receipt
/// is filed.
struct MemoryArchive {
    held: Mutex<Vec<(TransformationId, ReceiptSlot, Receipt)>>,
}

impl ReceiptArchive for MemoryArchive {
    fn store(
        &self,
        id: &TransformationId,
        slot: ReceiptSlot,
        receipt: &Receipt,
    ) -> Result<(), String> {
        self.held
            .lock()
            .expect("the archive is not poisoned")
            .push((*id, slot, receipt.clone()));
        Ok(())
    }

    fn load_commit(&self, id: &TransformationId) -> Option<Receipt> {
        self.held
            .lock()
            .expect("the archive is not poisoned")
            .iter()
            .rev()
            .find(|(held, slot, _)| held == id && *slot == ReceiptSlot::Commit)
            .map(|(_, _, receipt)| receipt.clone())
    }
}

/// An engine with the scripted adapter registered, behind `gx_api::router`, driven in process.
///
/// The shape `crates/gx-cli/tests/ac_055.rs` established for 51 §7's axum test client (equivalent
/// to `tower::ServiceExt`), with one substitution: the adapter. Everything else — the shipped fs
/// policy pack, the real gate, the real engine, the real router and its Bearer guard — is what a
/// deployment runs, because a bed that stubbed any of them would be measuring the stub.
pub struct ScriptedServer {
    /// The state the router was built from, so a caller can reach the engine (and through it the
    /// journal, for `gx_api::stream::events_for`) without a second door.
    pub state: AppState,
    /// The router itself. `oneshot` clones it per request, as axum's own tests do.
    pub router: axum::Router,
    /// The adapter, for its script, its world and its trace.
    pub adapter: Arc<ScriptedAdapter>,
    /// The scratch directory this fixture owns.
    pub dir: PathBuf,
    /// The locator every transformation of this fixture names.
    pub locator: String,
    /// The key id the actor is named by. The server's own key, which is enough: 42 §3.2's `Actor`
    /// carries a key id and nothing on this road resolves it to a secret.
    pub key_id: String,
}

impl ScriptedServer {
    /// Stand up a server whose world starts at `initial`, under a scratch directory named `name`.
    pub fn start(name: &str, grammar: Grammar, initial: &[&str]) -> Self {
        let dir = scratch(name);
        let adapter = ScriptedAdapter::open(dir.join("world.txt"), grammar, initial);
        let locator = adapter.locator();

        let evidence = RequestEvidence::new();
        let gate =
            gx_gate::Gate::with_policies(gx_gate::packs::fs_pack().expect("the shipped fs pack"));
        let journal = dir.join("ledger").join("journal");
        std::fs::create_dir_all(journal.parent().expect("a parent"))
            .expect("create the ledger dir");
        let mut engine =
            gx_engine::Engine::open(&journal, gate, evidence.clone()).expect("the engine opens");
        engine.register_adapter(adapter.clone(), "gx-cli r29 scripted fixture");

        let key = KeyPair::from_seed("r29-server", &[29u8; 32]);
        let key_id = key.key_id().to_string();
        let state = AppState::new(
            engine,
            evidence,
            Arc::new(OneKey(key)),
            Bearer::new(TOKEN),
            dir.join("index"),
            None,
        )
        .expect("no recorded keyid to disagree with")
        .with_archive(Arc::new(MemoryArchive {
            held: Mutex::new(Vec::new()),
        }));
        let router = gx_api::router(state.clone());

        Self {
            state,
            router,
            adapter,
            dir,
            locator,
            key_id,
        }
    }

    /// The world's bytes, read straight off the file.
    pub fn world(&self) -> String {
        self.adapter.world()
    }

    /// One request through the router, with the token.
    pub async fn send(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let mut builder = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("Bearer {TOKEN}"));
        let request = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                builder
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&value).expect("serialises"),
                    ))
                    .expect("a request")
            }
            None => builder.body(axum::body::Body::empty()).expect("a request"),
        };
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("the router answers");
        let status = response.status().as_u16();
        let bytes = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .expect("a readable body");
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// `POST /candidates`, then `verify`, then `commit`, and the `TransformationId` that came back.
    ///
    /// Every status is asserted here rather than at each call site, because a suite whose bed
    /// half-fails silently is a suite whose findings are about the bed.
    pub async fn commit_goal(&self, goal: &str) -> String {
        let (status, created) = self
            .send(
                "POST",
                "/v1/candidates",
                Some(serde_json::json!({
                    "substrate": "fs",
                    "locator": self.locator,
                    "goal": goal,
                    "context": "Evidence",
                    "actor": { "Human": { "key": self.key_id } },
                })),
            )
            .await;
        assert_eq!(status, 201, "POST /v1/candidates: {created}");
        let id = created["id"].as_str().expect("an id").to_string();

        let (status, verified) = self
            .send("POST", &format!("/v1/candidates/{id}/verify"), None)
            .await;
        assert_eq!(status, 200, "verify: {verified}");
        let (status, committed) = self
            .send("POST", &format!("/v1/candidates/{id}/commit"), None)
            .await;
        assert_eq!(status, 200, "commit: {committed}");
        id
    }

    /// `POST /transformations/{id}/undo`, whatever it answers.
    pub async fn undo(&self, id: &str) -> (u16, Value) {
        self.send("POST", &format!("/v1/transformations/{id}/undo"), None)
            .await
    }
}
